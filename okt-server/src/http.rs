//! HTTP: the page, its assets, the JSON API, raw files, server-sent events, and the few POST
//! actions (rebuild, open in an app, pause watching). Loopback only by default; every request's
//! Host header must name this server, and POSTs need a custom header and a same-origin Origin.

use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::Response;
use base64::Engine;
use once_cell::sync::Lazy;
use regex::Regex;
use rust_embed::RustEmbed;
use serde_json::{Value, json};
use sha2::Digest;

use crate::app::App;
use crate::index::Index;
use crate::launch;
use crate::pwa;
use crate::render::{RenderError, guess_mime, py_quote};
use crate::util::*;

#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/../okt-client/dist"]
#[allow_missing = true]
pub struct ClientDist;

pub fn router(app: Arc<App>) -> Router {
    Router::new().fallback(dispatch).with_state(app)
}

fn qs_parse(query: &str) -> HashMap<String, String> {
    // urllib.parse.parse_qs, first value wins, blank values dropped
    let mut out = HashMap::new();
    for part in query.split('&') {
        if part.is_empty() {
            continue;
        }
        let (k, v) = part.split_once('=').unwrap_or((part, ""));
        let dec = |s: &str| {
            percent_encoding::percent_decode_str(&s.replace('+', " "))
                .decode_utf8_lossy()
                .to_string()
        };
        let (k, v) = (dec(k), dec(v));
        if v.is_empty() {
            continue;
        }
        out.entry(k).or_insert(v);
    }
    out
}

struct Resp {
    status: u16,
    ctype: String,
    data: Vec<u8>,
    extra: Vec<(&'static str, String)>,
    compress: bool,
}

impl Resp {
    fn json(v: &Value) -> Self {
        Resp {
            status: 200,
            ctype: "application/json; charset=utf-8".into(),
            data: serde_json::to_vec(v).unwrap(),
            extra: vec![],
            compress: true,
        }
    }
    fn json_bytes(b: Vec<u8>) -> Self {
        Resp {
            status: 200,
            ctype: "application/json; charset=utf-8".into(),
            data: b,
            extra: vec![],
            compress: true,
        }
    }
    fn fail(status: u16, msg: &str) -> Self {
        let mut r = Self::json(&json!({"error": msg}));
        r.status = status;
        r
    }
    fn bytes(data: Vec<u8>, ctype: &str) -> Self {
        Resp {
            status: 200,
            ctype: ctype.into(),
            data,
            extra: vec![],
            compress: true,
        }
    }
}

fn finish(req_headers: &HeaderMap, head_only: bool, r: Resp) -> Response {
    let gzip_ok = req_headers
        .get("accept-encoding")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("gzip"))
        .unwrap_or(false);
    let mut data = r.data;
    let mut extra = r.extra;
    let media = ["image/", "video/", "audio/", "application/pdf"];
    if r.compress && data.len() > 2048 && gzip_ok && !media.iter().any(|m| r.ctype.starts_with(m)) {
        let mut z = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::new(5));
        z.write_all(&data).unwrap();
        data = z.finish().unwrap();
        extra.push(("Content-Encoding", "gzip".into()));
        extra.push(("Vary", "Accept-Encoding".into()));
    }
    let mut b = Response::builder().status(r.status);
    let h = b.headers_mut().unwrap();
    h.insert("Content-Type", HeaderValue::from_str(&r.ctype).unwrap());
    h.insert("Content-Length", HeaderValue::from(data.len()));
    h.insert(
        "X-Content-Type-Options",
        HeaderValue::from_static("nosniff"),
    );
    h.insert("Referrer-Policy", HeaderValue::from_static("no-referrer"));
    h.insert(
        "Cross-Origin-Resource-Policy",
        HeaderValue::from_static("same-origin"),
    );
    h.insert("Cache-Control", HeaderValue::from_static("no-store"));
    for (k, v) in extra {
        if let Ok(v) = HeaderValue::from_str(&v) {
            h.insert(k, v);
        }
    }
    b.body(if head_only {
        Body::empty()
    } else {
        Body::from(data)
    })
    .unwrap()
}

fn host_ok(app: &App, headers: &HeaderMap) -> bool {
    let host = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();
    app.allowed_hosts.read().unwrap().contains(&host)
}

fn token_ok(app: &App, headers: &HeaderMap) -> bool {
    let Some(t) = &app.token else { return true };
    let cookie = headers
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    cookie.contains(&format!("okt={t}"))
}

async fn dispatch(State(app): State<Arc<App>>, req: Request) -> Response {
    let method = req.method().clone();
    let headers = req.headers().clone();
    let uri = req.uri().clone();
    let path = uri.path().to_string();
    let query = uri.query().unwrap_or("").to_string();
    let head_only = method == Method::HEAD;
    if method == Method::POST {
        let body = axum::body::to_bytes(req.into_body(), 1 << 20)
            .await
            .unwrap_or_default();
        let r = post(app, &headers, &path, body).await;
        return finish(&headers, false, r);
    }
    if method != Method::GET && method != Method::HEAD {
        return finish(&headers, false, Resp::fail(405, "method not allowed"));
    }
    if !host_ok(&app, &headers) {
        return finish(&headers, head_only, Resp::fail(403, "bad host header"));
    }
    let qs = qs_parse(&query);
    if let Some(t) = &app.token {
        if path == "/" && qs.get("token") == Some(t) {
            return Response::builder()
                .status(303)
                .header(
                    "Set-Cookie",
                    format!("okt={t}; HttpOnly; SameSite=Strict; Path=/"),
                )
                .header("Location", "/")
                .header("Content-Length", "0")
                .body(Body::empty())
                .unwrap();
        }
    }
    if path == "/api/hello" {
        let r = Resp::json(
            &json!({"app": crate::APP, "version": crate::VERSION, "root": app.root.display().to_string(), "pid": std::process::id(), "build": app.build, "exe": app.exe}),
        );
        return finish(&headers, head_only, r);
    }
    if let Some(r) = pwa_asset(&app, &path) {
        return finish(&headers, head_only, r);
    }
    if !token_ok(&app, &headers) {
        let mut r = Resp::bytes(b"<!doctype html><title>Open Knowledge Terminal</title><p>Open the link printed in your terminal.</p>".to_vec(), "text/html; charset=utf-8");
        r.status = 401;
        return finish(&headers, head_only, r);
    }
    if path == "/api/events" {
        return events(app);
    }
    let app2 = app.clone();
    let path2 = path.clone();
    let r = tokio::task::spawn_blocking(move || get(&app2, &path2, &qs))
        .await
        .unwrap_or_else(|e| Resp::fail(500, &format!("{e}")));
    finish(&headers, head_only, r)
}

fn page_csp(scripts: &[String]) -> String {
    let hashes: String = scripts.iter().map(|h| format!(" 'sha256-{h}'")).collect();
    format!(
        "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'{hashes}; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; \
         font-src 'self' data:; connect-src 'self'; frame-src 'self'; media-src 'self'; object-src 'none'; \
         base-uri 'none'; form-action 'none'; frame-ancestors 'none'"
    )
}

/// The page template and the hashes of its inline scripts (the wasm loader Trunk writes).
pub fn page_template() -> (String, Vec<String>) {
    let html = ClientDist::get("index.html")
        .map(|f| String::from_utf8_lossy(&f.data).into_owned())
        .unwrap_or_else(|| FALLBACK_PAGE.to_string());
    static SCRIPT: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?s)<script(?P<attrs>[^>]*)>(?P<body>.*?)</script>").unwrap());
    let mut hashes = Vec::new();
    for m in SCRIPT.captures_iter(&html) {
        let attrs = &m["attrs"];
        if attrs.contains("src=") || attrs.contains("application/json") {
            continue;
        }
        let digest = sha2::Sha256::digest(m["body"].as_bytes());
        hashes.push(base64::engine::general_purpose::STANDARD.encode(digest));
    }
    (html, hashes)
}

const FALLBACK_PAGE: &str = "<!doctype html><html><head><meta charset=\"utf-8\"><title>__APPNAME__</title></head><body style=\"background:#000;color:#ffa028;font:14px monospace;padding:2em\"><p>The client has not been built. Run <code>make client</code> (trunk build) and rebuild the server.</p><script id=\"boot\" type=\"application/json\">__BOOT__</script></body></html>";

pub fn shell_assets() -> Vec<String> {
    let mut v = vec!["/".to_string()];
    for f in ClientDist::iter() {
        if f != "index.html" {
            v.push(format!("/{f}"));
        }
    }
    v.extend(["/favicon.svg", "/icon.svg", "/manifest.webmanifest"].map(String::from));
    v
}

fn page(app: &App) -> Resp {
    let a = &app.args;
    let boot = json!({
        "version": crate::VERSION, "theme": a.theme, "accent": a.accent, "collapse": a.collapse,
        "watch": app.watching, "interval": a.interval, "editor": app.editor.first(), "platform": std::env::consts::OS.replace("macos", "darwin"),
        "kb": app.root.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(), "app": a.app, "font": a.font, "build": app.build,
    });
    let (tpl, hashes) = page_template();
    let html = tpl.replace("__APPNAME__", &py_html_escape(&a.app)).replace(
        "__BOOT__",
        &serde_json::to_string(&boot).unwrap().replace("</", "<\\/"),
    );
    let mut r = Resp::bytes(html.into_bytes(), "text/html; charset=utf-8");
    r.extra.push(("Content-Security-Policy", page_csp(&hashes)));
    r
}

fn pwa_asset(app: &App, path: &str) -> Option<Resp> {
    let a = &app.args;
    match path {
        "/manifest.webmanifest" => Some(Resp::bytes(
            serde_json::to_vec_pretty(&pwa::manifest(&a.app, &a.theme, &display_path(&app.root)))
                .unwrap(),
            "application/manifest+json",
        )),
        "/sw.js" => {
            let mut r = Resp::bytes(
                pwa::service_worker(&app.build, &shell_assets()).into_bytes(),
                "text/javascript; charset=utf-8",
            );
            r.extra.push(("Service-Worker-Allowed", "/".into()));
            Some(r)
        }
        "/icon.svg" | "/favicon.svg" => Some(Resp::bytes(
            pwa::icon_svg(&a.accent, path == "/favicon.svg").into_bytes(),
            "image/svg+xml",
        )),
        p => pwa::ICON_PNGS
            .iter()
            .find(|(n, _)| *n == p)
            .map(|(_, size)| Resp::bytes(pwa::icon_png(*size, &a.accent), "image/png")),
    }
}

fn not_found(msg: &str) -> Resp {
    Resp::fail(
        404,
        &if msg.contains(' ') {
            msg.to_string()
        } else {
            format!("not found: {msg}")
        },
    )
}

fn render_err(e: RenderError) -> Resp {
    match e {
        RenderError::NotFound(m) => not_found(&m),
        RenderError::Git(m) => Resp::fail(500, &format!("git: {m}")),
    }
}

fn get(app: &App, path: &str, qs: &HashMap<String, String>) -> Resp {
    let q = |k: &str| qs.get(k).map(|s| s.as_str());
    match path {
        "/" => page(app),
        "/api/index" => Resp::json_bytes(app.index.read().unwrap().snapshot().as_ref().clone()),
        "/api/file" => {
            let p = q("path").unwrap_or("");
            let rev = q("rev");
            let idx = app.index.read().unwrap();
            if rev.is_none() && !idx.files.contains_key(p) {
                return Resp::fail(404, &format!("not in the index: {p}"));
            }
            let mut out = match app.render.payload(&idx, p, rev) {
                Ok(o) => o,
                Err(e) => return render_err(e),
            };
            if rev.is_none() {
                let hist = idx.hist.get(p).cloned().unwrap_or_default();
                let mut g = serde_json::Map::new();
                g.insert("count".into(), json!(hist.len()));
                if !hist.is_empty() {
                    g.insert(
                        "first".into(),
                        Value::Object(Index::commit_summary(&idx.commits[hist[hist.len() - 1].0])),
                    );
                    g.insert(
                        "last".into(),
                        Value::Object(Index::commit_summary(&idx.commits[hist[0].0])),
                    );
                }
                out.as_object_mut()
                    .unwrap()
                    .insert("git".into(), Value::Object(g));
            }
            Resp::json(&out)
        }
        "/api/log" => {
            let idx = app.index.read().unwrap();
            let p = q("path");
            let limit: usize = q("limit").and_then(|l| l.parse().ok()).unwrap_or(0);
            match p {
                None | Some("") | Some(".") => {
                    let commits = if limit > 0 {
                        &idx.commits[..limit.min(idx.commits.len())]
                    } else {
                        &idx.commits[..]
                    };
                    let out: Vec<Value> = commits
                        .iter()
                        .map(|c| {
                            let mut m = Index::commit_summary(c);
                            m.insert("f".into(), json!(c.f));
                            Value::Object(m)
                        })
                        .collect();
                    Resp::json(
                        &json!({"path": null, "commits": out, "total": idx.commits.len(), "head": idx.head}),
                    )
                }
                Some(p) if p.ends_with('/') => Resp::json(
                    &json!({"path": p, "commits": idx.folder_history(p, 400), "head": idx.head}),
                ),
                Some(p) => Resp::json(
                    &json!({"path": p, "commits": idx.file_history(p), "head": idx.head}),
                ),
            }
        }
        "/api/blame" => blame(app, q("path").unwrap_or(""), q("rev")),
        "/api/table" => {
            let idx = app.index.read().unwrap();
            Resp::json(&idx.table(q("folder").unwrap_or("."), q("recursive") == Some("1")))
        }
        "/api/search" => {
            let idx = app.index.read().unwrap();
            Resp::json(&idx.search(
                q("q").unwrap_or(""),
                q("re") == Some("1"),
                q("case") == Some("1"),
                400,
            ))
        }
        p if p.starts_with("/api/commit/") => {
            static SHA: Lazy<Regex> =
                Lazy::new(|| Regex::new(r"^(?:[0-9a-fA-F]{4,40}|HEAD(~\d+)?)$").unwrap());
            let sha = percent_encoding::percent_decode_str(&p["/api/commit/".len()..])
                .decode_utf8_lossy()
                .to_string();
            if !SHA.is_match(&sha) {
                return Resp::fail(400, "bad sha");
            }
            match app.commit_detail(&sha) {
                Ok(v) => Resp::json(&v),
                Err((s, m)) => Resp::fail(s, &m),
            }
        }
        p if p.starts_with("/raw/") => {
            let rel = percent_encoding::percent_decode_str(&p[5..])
                .decode_utf8_lossy()
                .to_string();
            raw(app, &rel, q("rev"))
        }
        p => {
            let name = p.trim_start_matches('/');
            if !name.is_empty() && name != "index.html" {
                if let Some(f) = ClientDist::get(name) {
                    let ctype = match name.rsplit('.').next() {
                        Some("wasm") => "application/wasm".to_string(),
                        Some("js") => "text/javascript; charset=utf-8".to_string(),
                        Some("css") => "text/css; charset=utf-8".to_string(),
                        _ => guess_mime(name).unwrap_or_else(|| "application/octet-stream".into()),
                    };
                    return Resp::bytes(f.data.into_owned(), &ctype);
                }
            }
            Resp::fail(404, "not found")
        }
    }
}

fn blame(app: &App, path: &str, rev: Option<&str>) -> Resp {
    let idx = app.index.read().unwrap();
    if rev.is_none() && !idx.files.contains_key(path) {
        return Resp::fail(404, "not in the index");
    }
    let to_json =
        |r: Result<(Vec<usize>, Vec<crate::git::BlameCommit>), crate::git::GitError>| -> Resp {
            match r {
                Ok((lines, commits)) => {
                    let commits: Vec<Value> = commits
                        .into_iter()
                        .map(|c| {
                            let mut m = serde_json::Map::new();
                            m.insert("h".into(), json!(c.h));
                            m.insert("t".into(), json!(c.t));
                            m.insert("s".into(), json!(c.s));
                            m.insert("an".into(), json!(c.an));
                            if let Some(x) = c.x {
                                m.insert("x".into(), json!(x));
                            }
                            Value::Object(m)
                        })
                        .collect();
                    Resp::json(&json!({"lines": lines, "commits": commits}))
                }
                Err(e) => Resp::fail(500, &format!("git: {e}")),
            }
        };
    if let Some(rev) = rev {
        let Some(sha) = idx.git.resolve(rev) else {
            return Resp::fail(404, "unknown revision");
        };
        let Some(at) = idx.path_at(path, &sha) else {
            return Resp::fail(404, &format!("{path} did not exist at {}", &sha[..7]));
        };
        return to_json(idx.git.blame(&at, Some(&sha)));
    }
    if !idx.hist.contains_key(path) {
        let n = idx
            .files
            .get(path)
            .map(|e| e.lines)
            .filter(|n| *n > 0)
            .unwrap_or(1);
        return Resp::json(
            &json!({"lines": vec![0; n], "commits": [{"h": "0".repeat(40), "t": 0, "s": "Not committed yet", "an": ""}]}),
        );
    }
    to_json(idx.git.blame(path, None))
}

fn raw(app: &App, path: &str, rev: Option<&str>) -> Resp {
    let idx = app.index.read().unwrap();
    let data = if let Some(rev) = rev {
        let Some(sha) = idx.git.resolve(rev) else {
            return Resp::fail(404, "unknown revision");
        };
        let Some(at) = idx.path_at(path, &sha) else {
            return Resp::fail(404, "not in that revision");
        };
        match idx.git.cat(&sha, &at) {
            Ok(d) => d,
            Err(_) => return Resp::fail(404, "not in that revision"),
        }
    } else {
        if !idx.files.contains_key(path) {
            return Resp::fail(404, "not in the index");
        }
        let full = app.root.join(path);
        let real = std::fs::canonicalize(&full);
        let root = std::fs::canonicalize(&app.root).unwrap_or(app.root.clone());
        match real {
            Ok(r) if r.starts_with(&root) => {}
            _ => return Resp::fail(403, "outside the KB"),
        }
        match std::fs::read(&full) {
            Ok(d) => d,
            Err(e) => return Resp::fail(500, &format!("OSError: {e}")),
        }
    };
    let mut ctype = guess_mime(path).unwrap_or_else(|| "application/octet-stream".into());
    if ctype.starts_with("text/")
        || ctype == "application/json"
        || ctype == "application/javascript"
    {
        ctype.push_str("; charset=utf-8");
    }
    let mut r = Resp::bytes(data, &ctype);
    r.extra.push(("X-Frame-Options", "SAMEORIGIN".into()));
    r.extra.push(("Content-Disposition", "inline".into()));
    let e = ext_of(path);
    if [".html", ".htm", ".svg", ".xml"].contains(&e.as_str()) {
        r.extra.push(("Content-Security-Policy", "sandbox allow-scripts allow-popups allow-popups-to-escape-sandbox; frame-ancestors 'self'".into()));
    }
    r
}

fn events(app: Arc<App>) -> Response {
    let mut rx = app.bus.subscribe();
    let hello = json!({
        "version": app.index.read().unwrap().version, "head": app.index.read().unwrap().head, "started": app.started, "build": app.build,
        "watch": app.watching && !app.paused.load(Ordering::Relaxed), "interval": app.args.interval,
    });
    let first = format!(
        "retry: 2000\nevent: hello\ndata: {}\n\n",
        serde_json::to_string(&hello).unwrap()
    );
    let stream = async_stream(first, move |tx| {
        let app = app.clone();
        async move {
            loop {
                if app.stopping.load(Ordering::Relaxed) {
                    break;
                }
                match tokio::time::timeout(Duration::from_secs(15), rx.recv()).await {
                    Err(_) => {
                        if tx.send(Bytes::from_static(b": ping\n\n")).await.is_err() {
                            break;
                        }
                    }
                    Ok(Ok((kind, data))) => {
                        if kind == "bye" {
                            break;
                        }
                        if tx
                            .send(Bytes::from(format!("event: {kind}\ndata: {data}\n\n")))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => continue,
                    Ok(Err(_)) => break,
                }
            }
        }
    });
    Response::builder()
        .status(200)
        .header("Content-Type", "text/event-stream; charset=utf-8")
        .header("Cache-Control", "no-store")
        .header("X-Accel-Buffering", "no")
        .header("X-Content-Type-Options", "nosniff")
        .body(Body::from_stream(stream))
        .unwrap()
}

/// A body stream fed by a task through a channel, starting with `first`.
fn async_stream<F, Fut>(
    first: String,
    f: F,
) -> impl futures_util::Stream<Item = Result<Bytes, std::io::Error>>
where
    F: FnOnce(tokio::sync::mpsc::Sender<Bytes>) -> Fut,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let (tx, rx) = tokio::sync::mpsc::channel::<Bytes>(64);
    let _ = tx.try_send(Bytes::from(first));
    tokio::spawn(f(tx));
    futures_util::stream::unfold(
        rx,
        |mut rx| async move { rx.recv().await.map(|b| (Ok(b), rx)) },
    )
}

async fn post(app: Arc<App>, headers: &HeaderMap, path: &str, body: Bytes) -> Resp {
    if !host_ok(&app, headers) || !token_ok(&app, headers) {
        return Resp::fail(403, "forbidden");
    }
    let marked = headers.get("x-okt").map(|v| v == "1").unwrap_or(false)
        || headers
            .get("x-kb-terminal")
            .map(|v| v == "1")
            .unwrap_or(false);
    if !marked {
        return Resp::fail(403, "missing X-OKT header");
    }
    if let Some(origin) = headers.get("origin").and_then(|v| v.to_str().ok()) {
        let host = origin
            .split("://")
            .nth(1)
            .unwrap_or("")
            .split('/')
            .next()
            .unwrap_or("")
            .to_lowercase();
        if !app.allowed_hosts.read().unwrap().contains(&host) {
            return Resp::fail(403, "bad origin");
        }
    }
    let req: Value = if body.is_empty() {
        json!({})
    } else {
        match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(_) => return Resp::fail(400, "bad json"),
        }
    };
    let path = path.to_string();
    tokio::task::spawn_blocking(move || post_blocking(&app, &path, &req))
        .await
        .unwrap_or_else(|e| Resp::fail(500, &format!("{e}")))
}

fn post_blocking(app: &App, path: &str, req: &Value) -> Resp {
    match path {
        "/api/rebuild" => {
            let t0 = Instant::now();
            let change = {
                let mut idx = app.index.write().unwrap();
                match idx.build("rebuild") {
                    Ok(c) => c,
                    Err(e) => return Resp::fail(500, &format!("git: {e}")),
                }
            };
            app.render.clear();
            let ms = py_round1(t0.elapsed().as_secs_f64() * 1000.0);
            if !app.args.quiet {
                crate::log(&format!("full rebuild in {:.0} ms", ms));
            }
            let (version, head) = {
                let idx = app.index.read().unwrap();
                (idx.version, idx.head.clone())
            };
            let mut ev = change.to_json();
            let m = ev.as_object_mut().unwrap();
            let mut out = serde_json::Map::new();
            out.insert("version".into(), json!(version));
            for (k, v) in m.iter() {
                out.insert(k.clone(), v.clone());
            }
            out.insert("ms".into(), json!(ms));
            out.insert("head".into(), json!(head));
            app.publish("index", &Value::Object(out));
            Resp::json(&json!({"ok": true, "version": version, "ms": ms}))
        }
        "/api/open" => {
            let p = req.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let idx = app.index.read().unwrap();
            let is_dir = idx.dirs.contains(&format!("{}/", p.trim_end_matches('/')));
            if !idx.files.contains_key(p) && !is_dir && p != "." && !p.is_empty() {
                return Resp::fail(404, "not in the index");
            }
            let target = if p == "." || p.is_empty() {
                app.root.clone()
            } else {
                app.root.join(p.trim_end_matches('/'))
            };
            let mut how = req
                .get("how")
                .and_then(|v| v.as_str())
                .unwrap_or("app")
                .to_string();
            if !["app", "reveal", "editor"].contains(&how.as_str()) {
                return Resp::fail(400, "bad how");
            }
            if how == "app" {
                if let Some(e) = idx.files.get(p) {
                    let exec = {
                        use std::os::unix::fs::PermissionsExt;
                        std::fs::metadata(&target)
                            .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
                            .unwrap_or(false)
                    };
                    if e.kind == "code" || e.kind == "text" || exec {
                        how = "text".into();
                    }
                }
            }
            let line = req.get("line").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
            match launch::launch(&how, &target, line, &app.editor) {
                Ok(cmd) => Resp::json(&json!({"ok": true, "cmd": cmd})),
                Err(e) => Resp::fail(500, &format!("RuntimeError: {e}")),
            }
        }
        "/api/watch" => {
            let paused = req.get("paused").map(py_truthy).unwrap_or(false);
            if app.watching {
                app.paused.store(paused, Ordering::Relaxed);
            }
            Resp::json(
                &json!({"ok": true, "paused": app.watching && app.paused.load(Ordering::Relaxed)}),
            )
        }
        _ => Resp::fail(404, "not found"),
    }
}

#[allow(dead_code)]
pub fn quote(s: &str) -> String {
    py_quote(s)
}

#[allow(dead_code)]
fn status_of(r: &Response) -> StatusCode {
    r.status()
}
