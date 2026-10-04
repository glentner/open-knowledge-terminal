//! The Open Knowledge Terminal: a live, Bloomberg-style browser terminal for a markdown knowledge
//! base kept in git.
//!
//! It indexes every file git would track (honoring .gitignore), parses frontmatter, links, and
//! mentions into a graph, reads history straight from git, and serves a single-page interface on
//! localhost. A file watcher notices saved files and new commits and rebuilds only what changed;
//! the page updates itself over server-sent events.
//!
//! ```text
//! okt                 # index ~/KB (or the repo you are in) and open it
//! okt --detach        # serve in the background and return (Raycast, launchers)
//! okt --stop          # stop the background server
//! okt --no-browser    # just serve
//! okt --check         # build the index, print a summary, exit
//! ```
//!
//! The terminal never writes to the KB. The only state it keeps is the browser's own preferences.

mod app;
mod config;
mod git;
mod http;
mod index;
mod launch;
mod parse;
mod pwa;
mod render;
mod util;
mod watch;
mod yaml;

use std::collections::HashSet;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use clap::Parser;
use indexmap::IndexMap;
use serde_json::Value;

use crate::app::App;
use crate::git::Git;
use crate::index::Index;
use crate::util::display_path;

pub const APP: &str = "okt";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DEFAULT_PORT: u16 = 6580; // O-K-T on a phone keypad

#[derive(Parser, Clone, Debug)]
#[command(
    name = "okt",
    args_override_self = true,
    version,
    about = "The Open Knowledge Terminal: a live, read-only browser terminal for a markdown KB in git.",
    after_help = "keys: / or : command line · g graph · f files · l log · t table · o/c/h inspector tabs · ? help\nThe page reconnects on its own if you restart the server."
)]
pub struct Args {
    /// the KB repo (default: the git repo you are in, else ~/KB)
    #[arg(long, value_name = "PATH")]
    pub kb: Option<PathBuf>,
    /// interface to bind (loopback only by default)
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
    /// port (the next free one if busy; 0 = any)
    #[arg(long, default_value_t = DEFAULT_PORT)]
    pub port: u16,
    /// what to open once serving: auto = the installed app if there is one, else a browser tab
    #[arg(long, default_value = "auto", value_parser = ["auto", "app", "browser", "none"])]
    pub open: String,
    /// open nothing (same as --open none)
    #[arg(long)]
    pub no_browser: bool,
    /// name of the installed web app, for its manifest and for --open
    #[arg(long, default_value = "Open Knowledge Terminal", value_name = "NAME")]
    pub app: String,
    /// serve in the background and return once ready; if it is already running, just open it
    #[arg(long)]
    pub detach: bool,
    /// stop the running server for this KB
    #[arg(long)]
    pub stop: bool,
    /// polling interval in seconds, used only where file-system events are unavailable
    #[arg(long, default_value_t = 2.0, value_name = "SEC")]
    pub interval: f64,
    /// disable watching; use REFRESH in the UI to rebuild
    #[arg(long)]
    pub no_watch: bool,
    /// graph: folders with more than N direct files start collapsed (0 = never)
    #[arg(long, default_value_t = 25, value_name = "N")]
    pub collapse: i64,
    /// command for EDIT, with {path} and {line} placeholders (default: $OKT_EDITOR, else VS Code, else the OS default)
    #[arg(long, value_name = "CMD")]
    pub editor: Option<String>,
    /// initial theme
    #[arg(long, default_value = "terminal", value_parser = ["terminal", "dark", "light"])]
    pub theme: String,
    /// initial terminal accent color, also the app icon
    #[arg(long, default_value = "#ffa028", value_name = "HEX")]
    pub accent: String,
    /// monospace font to put first (default: Berkeley Mono when installed, then JetBrains Mono, SF Mono, Menlo)
    #[arg(long, value_name = "FAMILY")]
    pub font: Option<String>,
    /// require a one-time link token (automatic when binding beyond loopback)
    #[arg(long)]
    pub require_token: bool,
    /// build the index, print a summary and any broken links, then exit
    #[arg(long)]
    pub check: bool,
    /// print only the startup banner
    #[arg(short, long)]
    pub quiet: bool,
    /// log every request
    #[arg(short, long)]
    pub verbose: bool,
}

fn tty() -> bool {
    unsafe { libc::isatty(1) == 1 }
}

fn plain(s: &str) -> String {
    if tty() {
        return s.to_string();
    }
    static ANSI: once_cell::sync::Lazy<regex::Regex> =
        once_cell::sync::Lazy::new(|| regex::Regex::new(r"\x1b\[[0-9;]*m").unwrap());
    ANSI.replace_all(s, "").into_owned()
}

pub fn say(s: &str) {
    println!("{}", plain(s));
    let _ = std::io::stdout().flush();
}

fn clock() -> String {
    let t = unsafe { libc::time(std::ptr::null_mut()) };
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&t, &mut tm) };
    format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec)
}

pub fn log(msg: &str) {
    say(&format!("\x1b[2m{}\x1b[0m {msg}", clock()));
}

/// GET /api/hello on a local port; None when nothing answers like us.
fn probe(host: &str, port: u16) -> Option<Value> {
    let addr: SocketAddr = (host, port).to_socket_addrs().ok()?.next()?;
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_millis(600)).ok()?;
    s.set_read_timeout(Some(Duration::from_millis(600))).ok()?;
    s.set_write_timeout(Some(Duration::from_millis(600))).ok()?;
    write!(
        s,
        "GET /api/hello HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n"
    )
    .ok()?;
    let mut buf = Vec::new();
    let _ = s.read_to_end(&mut buf);
    let text = String::from_utf8_lossy(&buf);
    let body = text.split("\r\n\r\n").nth(1)?;
    let v: Value = serde_json::from_str(body.trim()).ok()?;
    if v.get("app").and_then(|a| a.as_str()) == Some(APP) {
        Some(v)
    } else {
        Some(serde_json::json!({"app": "other"}))
    }
}

fn port_free(host: &str, port: u16) -> bool {
    TcpListener::bind((host, port)).is_ok()
}

/// port if it's free, else the next free one within 20, else 0 (the OS picks).
fn pick_port(host: &str, port: u16) -> u16 {
    for p in port..=port.saturating_add(20) {
        if port_free(host, p) {
            return p;
        }
    }
    0
}

fn log_file() -> PathBuf {
    let home = util::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let base = if cfg!(target_os = "macos") {
        home.join("Library").join("Logs")
    } else {
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local").join("state"))
    };
    let _ = std::fs::create_dir_all(&base);
    base.join("okt.log")
}

fn opened_note(what: &str) -> String {
    if what == "nothing" {
        String::new()
    } else {
        format!("; opened {what}")
    }
}

fn find_kb(arg: &Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(p) = arg {
        let p = std::fs::canonicalize(expand_home(p))
            .map_err(|e| format!("okt: {}: {e}", p.display()))?;
        return Git::toplevel(&p)
            .ok_or_else(|| format!("okt: {} is not inside a git work tree", p.display()));
    }
    if let Ok(cwd) = std::env::current_dir() {
        if let Some(top) = Git::toplevel(&cwd) {
            return Ok(top);
        }
    }
    if let Some(home) = util::home_dir() {
        let d = home.join("KB");
        if d.exists() {
            if let Some(top) = Git::toplevel(&d) {
                return Ok(top);
            }
        }
    }
    Err("okt: could not find the KB; run inside it or pass --kb PATH".into())
}

fn expand_home(p: &Path) -> PathBuf {
    let s = p.display().to_string();
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(h) = util::home_dir() {
            return h.join(rest);
        }
    }
    p.to_path_buf()
}

fn start_background(args: &Args, root: &Path) -> i32 {
    let port = if args.port == 0 {
        DEFAULT_PORT
    } else {
        args.port
    };
    for p in port..=port + 20 {
        let info = probe("127.0.0.1", p);
        if let Some(info) = &info {
            if info.get("root").and_then(|r| r.as_str()) == Some(&root.display().to_string()) {
                let url = format!("http://127.0.0.1:{p}/");
                say(&format!(
                    "Open Knowledge Terminal is running at {url}{}",
                    opened_note(&launch::open_ui(&url, &args.open, &args.app))
                ));
                return 0;
            }
        }
        if info.is_none() && p > port {
            break;
        }
    }
    let port = pick_port(&args.host, port);
    if port == 0 {
        eprintln!("okt: no free port near {}", args.port);
        return 1;
    }
    let logf = log_file();
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("okt"));
    let mut child_args: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| a != "--detach")
        .collect();
    child_args.extend([
        "--kb".into(),
        root.display().to_string(),
        "--port".into(),
        port.to_string(),
        "--open".into(),
        "none".into(),
    ]);
    let mut fh = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&logf)
    {
        Ok(f) => f,
        Err(e) => {
            eprintln!("okt: cannot open {}: {e}", logf.display());
            return 1;
        }
    };
    let _ = writeln!(
        fh,
        "\n--- {} starting: {} {}",
        clock(),
        exe.display(),
        child_args.join(" ")
    );
    let out = fh.try_clone().unwrap();
    let mut cmd = std::process::Command::new(&exe);
    cmd.args(&child_args)
        .stdin(std::process::Stdio::null())
        .stdout(out)
        .stderr(fh)
        .current_dir(root);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut proc = match cmd.spawn() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("okt: could not start the server: {e}");
            return 1;
        }
    };
    let deadline = Instant::now() + Duration::from_secs(90);
    while Instant::now() < deadline {
        if let Ok(Some(status)) = proc.try_wait() {
            eprintln!("okt: the server exited ({status}); see {}", logf.display());
            return 1;
        }
        if let Some(info) = probe("127.0.0.1", port) {
            if info.get("root").and_then(|r| r.as_str()) == Some(&root.display().to_string()) {
                let url = format!("http://127.0.0.1:{port}/");
                say(&format!(
                    "Open Knowledge Terminal started at {url} (pid {}){}",
                    proc.id(),
                    opened_note(&launch::open_ui(&url, &args.open, &args.app))
                ));
                return 0;
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    eprintln!(
        "okt: the server did not answer within 90 s; see {}",
        logf.display()
    );
    1
}

fn stop_background(args: &Args, root: &Path) -> i32 {
    let port = if args.port == 0 {
        DEFAULT_PORT
    } else {
        args.port
    };
    for p in port..=port + 20 {
        let info = probe("127.0.0.1", p);
        if let Some(info) = &info {
            if info.get("root").and_then(|r| r.as_str()) == Some(&root.display().to_string()) {
                if let Some(pid) = info.get("pid").and_then(|v| v.as_i64()) {
                    unsafe { libc::kill(pid as i32, libc::SIGTERM) };
                    for _ in 0..50 {
                        if probe("127.0.0.1", p).is_none() {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    say(&format!(
                        "Open Knowledge Terminal stopped (pid {pid}, port {p})"
                    ));
                    return 0;
                }
            }
        }
        if info.is_none() && p > port {
            break;
        }
    }
    say(&format!(
        "Open Knowledge Terminal is not running for {}",
        display_path(root)
    ));
    1
}

fn print_summary(idx: &Index, root: &Path, elapsed: f64) {
    let st = &idx.status;
    let head: String = idx
        .head
        .as_deref()
        .unwrap_or("no commits")
        .chars()
        .take(7)
        .collect();
    let mut kinds: IndexMap<&str, usize> = IndexMap::new();
    for e in idx.files.values() {
        *kinds.entry(e.kind.as_str()).or_insert(0) += 1;
    }
    let mut kinds: Vec<(&str, usize)> = kinds.into_iter().collect();
    kinds.sort_by(|a, b| b.1.cmp(&a.1));
    let mut ek: IndexMap<&str, usize> = IndexMap::new();
    for e in &idx.edges {
        *ek.entry(e.k).or_insert(0) += 1;
    }
    let mut ek: Vec<(&str, usize)> = ek.into_iter().collect();
    ek.sort_by(|a, b| b.1.cmp(&a.1));
    let mut line = format!(
        "\x1b[1;33mOPEN KNOWLEDGE TERMINAL\x1b[0m {VERSION}  ·  {}  ·  {} @ {head}",
        display_path(root),
        st.branch.as_deref().unwrap_or("?")
    );
    if st.ahead > 0 {
        line.push_str(&format!("  ↑{}", st.ahead));
    }
    if st.behind > 0 {
        line.push_str(&format!(" ↓{}", st.behind));
    }
    say(&line);
    say(&format!(
        "indexed {} files in {} folders · {} edges ({}) · {} commits · {:.2} s",
        idx.files.len(),
        idx.dirs.len(),
        idx.edges.len(),
        ek.iter()
            .map(|(k, v)| format!("{v} {k}"))
            .collect::<Vec<_>>()
            .join(", "),
        idx.commits.len(),
        elapsed
    ));
    say(&format!(
        "kinds: {}",
        kinds
            .iter()
            .map(|(k, v)| format!("{v} {k}"))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    if !idx.broken.is_empty() {
        let n = idx.broken.len();
        say(&format!(
            "\x1b[31m{n} broken link{}\x1b[0m",
            if n != 1 { "s" } else { "" }
        ));
    }
    if !st.files.is_empty() {
        let shown: Vec<String> = st
            .files
            .iter()
            .take(6)
            .map(|(p, c)| format!("{c} {p}"))
            .collect();
        say(&format!(
            "{} uncommitted: {}{}",
            st.files.len(),
            shown.join(", "),
            if st.files.len() > 6 { " …" } else { "" }
        ));
    }
}

fn build_hash(accent: &str) -> String {
    let mut h = sha1_smol::Sha1::new();
    let mut names: Vec<String> = http::ClientDist::iter().map(|s| s.to_string()).collect();
    names.sort();
    for n in names {
        if let Some(f) = http::ClientDist::get(&n) {
            h.update(n.as_bytes());
            h.update(&f.data);
        }
    }
    h.update(pwa::icon_svg(accent, false).as_bytes());
    h.update(VERSION.as_bytes());
    h.digest().to_string()[..10].to_string()
}

fn main() {
    let mut args = Args::parse();
    if args.no_browser {
        args.open = "none".into();
    }
    let acc = args.accent.trim_start_matches('#').to_lowercase();
    if acc.len() != 6 || !acc.chars().all(|c| c.is_ascii_hexdigit()) {
        eprintln!("okt: --accent must be a hex color like #ffa028");
        std::process::exit(2);
    }
    args.accent = format!("#{acc}");
    if let Some(f) = &args.font {
        if f.is_empty()
            || f.len() > 64
            || !f
                .chars()
                .all(|c| c.is_alphanumeric() || " .+-_".contains(c))
        {
            eprintln!("okt: --font takes a font family name, e.g. \"Berkeley Mono\"");
            std::process::exit(2);
        }
    }
    let root = match find_kb(&args.kb) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let loopback = ["127.0.0.1", "localhost", "::1"].contains(&args.host.as_str());
    if !loopback {
        args.require_token = true;
    }
    if args.stop {
        std::process::exit(stop_background(&args, &root));
    }
    if args.detach {
        if !loopback {
            eprintln!("okt: --detach serves on loopback only");
            std::process::exit(2);
        }
        std::process::exit(start_background(&args, &root));
    }

    let git = Git::new(&root);
    let mut idx = Index::new(&root, git);
    idx.collapse = args.collapse.max(0);
    let t0 = Instant::now();
    if let Err(e) = idx.build("start") {
        eprintln!("okt: {e}");
        std::process::exit(1);
    }
    print_summary(&idx, &root, t0.elapsed().as_secs_f64());
    if args.check {
        for (p, target, line, kind) in &idx.broken {
            say(&format!("  broken {kind}: {p}:{line} -> {target}"));
        }
        std::process::exit(if idx.broken.is_empty() { 0 } else { 1 });
    }

    // single instance: if a terminal for this KB is already serving, just open it
    let mut port = args.port;
    if port != 0 {
        let probe_host = if loopback { "127.0.0.1" } else { "127.0.0.1" };
        if let Some(existing) = probe(probe_host, port) {
            if existing.get("root").and_then(|r| r.as_str()) == Some(&root.display().to_string()) {
                let url = format!(
                    "http://{}:{port}/",
                    if loopback {
                        "127.0.0.1"
                    } else {
                        args.host.as_str()
                    }
                );
                say(&format!(
                    "already running at {url}{}",
                    opened_note(&launch::open_ui(&url, &args.open, &args.app))
                ));
                return;
            }
        }
        let wanted = port;
        port = pick_port(&args.host, port);
        if port != wanted {
            say(&format!(
                "\x1b[33mport {wanted} is busy; serving on {}. An installed app expects the port it was installed from, so free {wanted} or install again from this one.\x1b[0m",
                if port == 0 {
                    "a random port".to_string()
                } else {
                    port.to_string()
                }
            ));
        }
    }
    let listener = match std::net::TcpListener::bind((args.host.as_str(), port)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("okt: cannot bind {}:{port}: {e}", args.host);
            std::process::exit(1);
        }
    };
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(port);
    listener.set_nonblocking(true).unwrap();
    let host_disp = if args.host.contains(':') {
        format!("[{}]", args.host)
    } else {
        args.host.clone()
    };
    let hostnames: HashSet<String> = [
        "127.0.0.1".to_string(),
        "localhost".to_string(),
        "[::1]".to_string(),
        host_disp,
    ]
    .into_iter()
    .collect();
    let allowed: HashSet<String> = hostnames.iter().map(|h| format!("{h}:{port}")).collect();
    let token = if args.require_token {
        Some(random_token())
    } else {
        None
    };
    let editor = args
        .editor
        .as_deref()
        .map(launch::shell_words)
        .unwrap_or_else(launch::default_editor);
    let (bus, _) = tokio::sync::broadcast::channel(256);
    let app = Arc::new(App {
        root: root.clone(),
        build: build_hash(&args.accent),
        args: args.clone(),
        index: RwLock::new(idx),
        render: render::Renderer::new(),
        bus,
        editor,
        started: util::now_secs(),
        token: token.clone(),
        allowed_hosts: RwLock::new(allowed),
        commit_cache: Mutex::new(IndexMap::new()),
        watching: !args.no_watch,
        paused: AtomicBool::new(false),
        stopping: AtomicBool::new(false),
    });
    if !args.no_watch {
        watch::spawn(app.clone());
    }
    std::thread::spawn(render::highlight::preload);
    {
        // build the markdown machinery (linkify's regexes, the sanitizer) before the first view
        let app = app.clone();
        std::thread::spawn(move || {
            let idx = app.index.read().unwrap();
            let _ = app.render.md.render(
                &idx,
                "warmup.md",
                "# Warm\n\nSee www.example.com, [a](b.md), `c/d.md`, [[e]].\n\n- [ ] task\n",
                false,
            );
        });
    }
    let shown = if loopback {
        "127.0.0.1".to_string()
    } else {
        args.host.clone()
    };
    let url = format!(
        "http://{shown}:{port}/{}",
        token
            .as_ref()
            .map(|t| format!("?token={t}"))
            .unwrap_or_default()
    );
    let watch_note = if args.no_watch {
        "manual refresh only".to_string()
    } else {
        "watching for changes".to_string()
    };
    say(&format!(
        "serving \x1b[4m{url}\x1b[0m  ·  {watch_note}  ·  Ctrl-C to stop"
    ));
    if !loopback {
        say("\x1b[31mwarning: bound beyond loopback; anyone with the link can read the KB\x1b[0m");
    }
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async move {
        let listener = tokio::net::TcpListener::from_std(listener).unwrap();
        let router = http::router(app.clone());
        if args.open != "none" {
            let (u, how, name) = (url.clone(), args.open.clone(), args.app.clone());
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(300)).await;
                launch::open_ui(&u, &how, &name);
            });
        }
        let app2 = app.clone();
        let shutdown = async move {
            let mut term =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {},
                _ = term.recv() => {},
            }
            app2.stopping.store(true, Ordering::Relaxed);
            app2.publish("bye", &serde_json::json!({}));
        };
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(shutdown)
            .await;
    });
    say("\nstopped");
}

fn random_token() -> String {
    let mut buf = [0u8; 24];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut buf);
    }
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf)
}
