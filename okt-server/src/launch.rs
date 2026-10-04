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
pub fn open_ui(url: &str, how: &str, app_name: &str) -> String {
    if how == "none" {
        return "nothing".into();
    }
    if (how == "auto" || how == "app") && cfg!(target_os = "macos") && !app_name.is_empty() {
        let ok = Command::new("open")
            .args(["-a", app_name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            return format!("the {app_name} app");
        }
        if how == "app" {
            println!(
                "no app named \"{app_name}\" yet; opening a browser tab. Install it from there (Safari: File > Add to Dock; Chrome: Install)."
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
