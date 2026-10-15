//! Opening things on the desktop: the terminal itself (the installed web app when there is one),
//! files in their default app or the user's editor, and folders in Finder.

use std::path::Path;
use std::process::{Command, Stdio};

use crate::git::which;

pub fn default_editor() -> Vec<String> {
    if let Ok(env) = std::env::var("OKT_EDITOR").or_else(|_| std::env::var("KB_TERMINAL_EDITOR")) {
        if !env.trim().is_empty() {
            return shell_words(&env);
        }
    }
    if which("code").is_some() {
        return vec!["code".into(), "--goto".into(), "{path}:{line}".into()];
    }
    if cfg!(target_os = "macos") {
        return vec!["open".into(), "-t".into(), "{path}".into()];
    }
    if which("xdg-open").is_some() {
        return vec!["xdg-open".into(), "{path}".into()];
    }
    vec![]
}

/// A small `shlex.split`: whitespace separated, with single and double quotes.
pub fn shell_words(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut has = false;
    let mut chars = s.chars().peekable();
    let mut quote: Option<char> = None;
    while let Some(c) = chars.next() {
        match quote {
            Some(q) if c == q => quote = None,
            Some('"') if c == '\\' => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            }
            Some(_) => cur.push(c),
            None => match c {
                '\'' | '"' => {
                    quote = Some(c);
                    has = true;
                }
                '\\' => {
                    if let Some(n) = chars.next() {
                        cur.push(n);
                        has = true;
                    }
                }
                c if c.is_whitespace() => {
                    if has || !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                        has = false;
                    }
                }
                c => cur.push(c),
            },
        }
    }
    if has || !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn spawn(cmd: &[String]) -> std::io::Result<()> {
    let mut c = Command::new(&cmd[0]);
    c.args(&cmd[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        c.process_group(0);
    }
    c.spawn().map(|_| ())
}

/// Open the terminal: the installed web app when there is one (macOS), else a browser tab.
///
/// The web app is found first by the address it was installed from, then by `open -a <app name>`.
/// Safari names a Dock app after the manifest's short name, or whatever was typed in its dialog,
/// so the name alone often misses and a tab opens instead.
pub fn open_ui(url: &str, how: &str, app_name: &str) -> String {
    if how == "none" {
        return "nothing".into();
    }
    if (how == "auto" || how == "app") && cfg!(target_os = "macos") {
        if let Some(app) = find_web_app(url, app_name) {
            if open_app(&app.path.to_string_lossy()) {
                return format!("the {} app", app.name);
            }
        }
        let short = crate::pwa::short_name(app_name);
        let mut names = vec![app_name];
        if short != app_name {
            names.push(&short);
        }
        for name in names {
            if !name.is_empty() && open_app(name) {
                return format!("the {name} app");
            }
        }
        if how == "app" {
            println!(
                "no app for {url} yet; opening a browser tab. Install it from there (Safari: File > Add to Dock; Chrome: Install)."
            );
        }
    }
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = spawn(&[opener.to_string(), url.to_string()]);
    "a browser tab".into()
}

/// `open -a <name or path>`: launches the app, or brings it to the front when it is running.
fn open_app(app: &str) -> bool {
    Command::new("open")
        .args(["-a", app])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// An installed web app: a Safari "Add to Dock" app or a Chrome shortcut app.
#[derive(Debug)]
pub struct WebApp {
    pub path: std::path::PathBuf,
    pub name: String,
    modified: std::time::SystemTime,
}

/// The web app installed from `url`'s origin, looked up in ~/Applications and one folder below it
/// (Chrome keeps its apps in `Chrome Apps.localized`). With several, the one named `prefer` wins,
/// else the newest.
fn find_web_app(url: &str, prefer: &str) -> Option<WebApp> {
    let home = std::env::var_os("HOME")?;
    find_web_app_in(&Path::new(&home).join("Applications"), url, prefer)
}

fn find_web_app_in(apps: &Path, url: &str, prefer: &str) -> Option<WebApp> {
    let mut dirs = vec![apps.to_path_buf()];
    let mut found = Vec::new();
    let mut i = 0;
    while i < dirs.len() {
        let Ok(rd) = std::fs::read_dir(&dirs[i]) else {
            i += 1;
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "app") {
                if let Some(app) = web_app_at(&p, url) {
                    found.push(app);
                }
            } else if i == 0 && p.is_dir() {
                dirs.push(p);
            }
        }
        i += 1;
    }
    pick_web_app(found, prefer)
}

fn pick_web_app(mut found: Vec<WebApp>, prefer: &str) -> Option<WebApp> {
    if let Some(i) = found.iter().position(|a| a.name == prefer) {
        return Some(found.swap_remove(i));
    }
    found.into_iter().max_by_key(|a| a.modified)
}

/// The app bundle at `path` when its Info.plist records an address with `url`'s origin.
fn web_app_at(path: &Path, url: &str) -> Option<WebApp> {
    let info = path.join("Contents").join("Info.plist");
    let plist = plist::Value::from_file(&info).ok()?;
    let d = plist.as_dictionary()?;
    if !web_app_urls(d).iter().any(|u| same_origin(u, url)) {
        return None;
    }
    let name = d
        .get("CFBundleName")
        .and_then(|v| v.as_string())
        .map(str::to_string)
        .or_else(|| Some(path.file_stem()?.to_string_lossy().into_owned()))?;
    let modified = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .unwrap_or(std::time::UNIX_EPOCH);
    Some(WebApp {
        path: path.to_path_buf(),
        name,
        modified,
    })
}

/// The addresses a browser records in a web app's Info.plist: Safari's manifest URL, manifest id
/// and start URL; Chrome's shortcut URL.
fn web_app_urls(d: &plist::Dictionary) -> Vec<String> {
    let s = |v: Option<&plist::Value>| v.and_then(|v| v.as_string()).map(str::to_string);
    let sub = |k: &str, kk: &str| {
        s(d.get(k)
            .and_then(|v| v.as_dictionary())
            .and_then(|m| m.get(kk)))
    };
    [
        s(d.get("WKManifestURL")),
        sub("WKPushBundleMetadata", "manifestId"),
        sub("Manifest", "start_url"),
        s(d.get("CrAppModeShortcutURL")),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Same scheme, host and port; the loopback names count as one host (an app installed from
/// localhost opens the server on 127.0.0.1 just as well).
fn same_origin(a: &str, b: &str) -> bool {
    fn parts(u: &str) -> Option<(String, String, u16)> {
        let (scheme, rest) = u.split_once("://")?;
        let scheme = scheme.to_ascii_lowercase();
        let auth = rest.split(['/', '?', '#']).next()?;
        let auth = auth.rsplit_once('@').map_or(auth, |(_, h)| h);
        let default = if scheme == "https" { 443 } else { 80 };
        let (host, port) = if let Some(v6) = auth.strip_prefix('[') {
            let (h, after) = v6.split_once(']')?;
            match after.strip_prefix(':') {
                Some(p) => (h, p.parse().ok()?),
                None => (h, default),
            }
        } else {
            match auth.rsplit_once(':') {
                Some((h, p)) => (h, p.parse().ok()?),
                None => (auth, default),
            }
        };
        let host = host.to_ascii_lowercase();
        let host = match host.as_str() {
            "localhost" | "127.0.0.1" | "::1" => "loopback".to_string(),
            _ => host,
        };
        Some((scheme, host, port))
    }
    matches!((parts(a), parts(b)), (Some(x), Some(y)) if x == y)
}

/// Hand a path to the desktop: `editor`, `text`, `reveal`, or the default `app`.
pub fn launch(how: &str, abs_path: &Path, line: u32, editor: &[String]) -> Result<String, String> {
    let p = abs_path.display().to_string();
    let cmd: Vec<String> = match how {
        "editor" => {
            if editor.is_empty() {
                return Err("no editor configured; pass --editor".into());
            }
            let mut cmd: Vec<String> = editor
                .iter()
                .map(|part| {
                    part.replace("{path}", &p)
                        .replace("{line}", &line.max(1).to_string())
                })
                .collect();
            if !editor.iter().any(|part| part.contains("{path}")) {
                cmd.push(p.clone());
            }
            cmd
        }
        "text" => {
            // scripts and plain text open as text, never through a handler that might run them
            if cfg!(target_os = "macos") {
                vec!["open".into(), "-t".into(), p.clone()]
            } else if !editor.is_empty() {
                return launch("editor", abs_path, line, editor);
            } else {
                vec!["xdg-open".into(), p.clone()]
            }
        }
        "reveal" => {
            if cfg!(target_os = "macos") {
                vec!["open".into(), "-R".into(), p.clone()]
            } else {
                vec![
                    "xdg-open".into(),
                    abs_path
                        .parent()
                        .map(|x| x.display().to_string())
                        .unwrap_or(p.clone()),
                ]
            }
        }
        _ => {
            if cfg!(target_os = "macos") {
                vec!["open".into(), p.clone()]
            } else {
                vec!["xdg-open".into(), p.clone()]
            }
        }
    };
    spawn(&cmd).map_err(|e| e.to_string())?;
    Ok(cmd.iter().take(2).cloned().collect::<Vec<_>>().join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origins() {
        let u = "http://127.0.0.1:6580/";
        assert!(same_origin("http://127.0.0.1:6580/manifest.webmanifest", u));
        assert!(same_origin("http://localhost:6580", u));
        assert!(same_origin("http://[::1]:6580/#v=graph", u));
        assert!(!same_origin("http://127.0.0.1:8376/", u));
        assert!(!same_origin("https://127.0.0.1:6580/", u));
        assert!(!same_origin("http://example.com:6580/", u));
        assert!(same_origin(
            "http://example.com/",
            "http://example.com:80/x"
        ));
        assert!(!same_origin("not a url", u));
    }

    // the shape of a Safari "Add to Dock" Info.plist (macOS 14), trimmed
    const SAFARI: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleIdentifier</key>
	<string>com.apple.Safari.WebApp.00000000-0000-0000-0000-000000000000</string>
	<key>CFBundleName</key>
	<string>Knowledge Terminal</string>
	<key>Manifest</key>
	<dict>
		<key>name</key>
		<string>Open Knowledge Terminal</string>
		<key>short_name</key>
		<string>Knowledge Terminal</string>
		<key>start_url</key>
		<string>http://127.0.0.1:6580/</string>
	</dict>
	<key>WKManifestURL</key>
	<string>http://127.0.0.1:6580/manifest.webmanifest</string>
	<key>WKPushBundleMetadata</key>
	<dict>
		<key>manifestId</key>
		<string>http://127.0.0.1:6580/</string>
	</dict>
</dict>
</plist>"#;

    #[test]
    fn safari_app_urls() {
        let v = plist::Value::from_reader_xml(SAFARI.as_bytes()).unwrap();
        let urls = web_app_urls(v.as_dictionary().unwrap());
        assert_eq!(urls.len(), 3);
        assert!(
            urls.iter()
                .all(|u| same_origin(u, "http://127.0.0.1:6580/"))
        );
    }

    /// `OKT_TEST_APPLICATIONS=<a copy of ~/Applications> cargo test -p okt-server -- --ignored`
    #[test]
    #[ignore]
    fn finds_installed_app() {
        let dir = std::env::var("OKT_TEST_APPLICATIONS").expect("OKT_TEST_APPLICATIONS");
        let url = std::env::var("OKT_TEST_URL").unwrap_or("http://127.0.0.1:6580/".into());
        let app = find_web_app_in(Path::new(&dir), &url, "Open Knowledge Terminal");
        println!("{app:?}");
        assert!(app.is_some());
    }

    #[test]
    fn picks_preferred_then_newest() {
        let app = |name: &str, secs: u64| WebApp {
            path: format!("/x/{name}.app").into(),
            name: name.into(),
            modified: std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs),
        };
        let both = || vec![app("OK Terminal", 1), app("Knowledge Terminal", 2)];
        assert_eq!(
            pick_web_app(both(), "Open Knowledge Terminal")
                .unwrap()
                .name,
            "Knowledge Terminal"
        );
        assert_eq!(
            pick_web_app(both(), "OK Terminal").unwrap().name,
            "OK Terminal"
        );
        assert!(pick_web_app(vec![], "OK Terminal").is_none());
    }
}
