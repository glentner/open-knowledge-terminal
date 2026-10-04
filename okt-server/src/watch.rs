//! Watching the KB: file-system events (FSEvents on macOS, inotify on Linux), debounced, trigger
//! an incremental re-index; a slow safety poll catches anything the events miss. Where events are
//! unavailable it falls back to polling every `--interval` seconds, like the reference watcher.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use notify::{RecursiveMode, Watcher};
use serde_json::{Value, json};

use crate::app::App;

const DEBOUNCE: Duration = Duration::from_millis(200);
const SAFETY_POLL: Duration = Duration::from_secs(60);

/// Paths whose changes can matter: anything outside .git, and the few files in .git that move
/// HEAD, branches, or the index (status).
fn relevant(root: &Path, p: &Path) -> bool {
    let Ok(rel) = p.strip_prefix(root) else {
        return true;
    };
    let mut comps = rel.components();
    let first = comps
        .next()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .unwrap_or_default();
    if first == ".git" {
        let rest: Vec<String> = comps
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect();
        return match rest.first().map(|s| s.as_str()) {
            Some("HEAD") | Some("index") | Some("packed-refs") | Some("refs")
            | Some("ORIG_HEAD") | Some("MERGE_HEAD") => {
                !rest.last().map(|l| l.ends_with(".lock")).unwrap_or(false)
            }
            _ => false,
        };
    }
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    !(name == ".DS_Store" || first == ".obsidian" || name.ends_with('~') || name.ends_with(".swp"))
}

pub fn spawn(app: Arc<App>) {
    let (tx, rx) = mpsc::channel::<()>();
    let root = app.root.clone();
    let root2 = root.clone();
    let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(ev) = res {
            if ev.paths.iter().any(|p| relevant(&root2, p)) {
                let _ = tx.send(());
            }
        }
    })
    .and_then(|mut w| w.watch(&root, RecursiveMode::Recursive).map(|_| w));
    let poll = match &watcher {
        Ok(_) => SAFETY_POLL,
        Err(e) => {
            crate::log(&format!(
                "file events unavailable ({e}); polling every {}s",
                app.args.interval
            ));
            Duration::from_secs_f64(app.args.interval.max(0.5))
        }
    };
    std::thread::Builder::new()
        .name("okt-watcher".into())
        .spawn(move || {
            let _keep = watcher; // dropping the watcher stops the events
            let mut ticks: u64 = 0;
            loop {
                if app.stopping.load(Ordering::Relaxed) {
                    break;
                }
                let got = rx.recv_timeout(poll);
                if let Err(mpsc::RecvTimeoutError::Disconnected) = got {
                    std::thread::sleep(poll);
                }
                if got.is_ok() {
                    // debounce: let a burst of saves settle
                    let deadline = Instant::now() + DEBOUNCE;
                    loop {
                        let now = Instant::now();
                        if now >= deadline {
                            break;
                        }
                        match rx.recv_timeout(deadline - now) {
                            Ok(()) => continue,
                            Err(_) => break,
                        }
                    }
                }
                if app.paused.load(Ordering::Relaxed) {
                    continue;
                }
                ticks += 1;
                if let Err(e) = tick(&app, got.is_ok() || ticks % 5 == 0) {
                    crate::log(&format!("watcher: {e}"));
                }
            }
        })
        .expect("watcher thread");
}

/// One watch step: re-scan, re-parse what changed, refresh status, and tell the pages.
fn tick(app: &App, check_status: bool) -> Result<(), String> {
    let git = app.git();
    let head = git.head();
    let (old_head, head_moved, old_status) = {
        let idx = app.index.read().unwrap();
        (idx.head.clone(), head != idx.head, idx.status.clone())
    };
    let mut change = {
        let mut idx = app.index.write().unwrap();
        idx.update(if head_moved { "commit" } else { "files" }, head_moved)
            .map_err(|e| e.to_string())?
    };
    if change.is_none() && check_status {
        let mut idx = app.index.write().unwrap();
        idx.refresh_status();
        let st = &idx.status;
        if st.files != old_status.files
            || st.ahead != old_status.ahead
            || st.behind != old_status.behind
            || st.branch != old_status.branch
        {
            idx.version += 1;
            *idx.json_cache.lock().unwrap() = None;
            let c = crate::index::Change {
                reason: "status".into(),
                ..Default::default()
            };
            idx.last_change = Some(c.clone());
            change = Some(c);
        }
    }
    let Some(change) = change else { return Ok(()) };
    if head_moved {
        let new = app.new_commits(old_head.as_deref(), head.as_deref());
        if !new.is_empty() {
            app.publish("commit", &json!({"commits": new}));
            if !app.args.quiet {
                for c in new.iter().take(5) {
                    crate::log(&format!(
                        "\x1b[33mnew commit\x1b[0m {} {}",
                        &c["h"].as_str().unwrap_or("")[..7],
                        c["s"].as_str().unwrap_or("")
                    ));
                }
            }
        }
    }
    let n = change.changed.len() + change.added.len() + change.removed.len();
    let (version, ms, head) = {
        let idx = app.index.read().unwrap();
        (
            idx.version,
            crate::util::py_round1(idx.built_ms),
            idx.head.clone(),
        )
    };
    if n > 0 && !app.args.quiet {
        crate::log(&format!(
            "reindexed {n} file{} in {:.0} ms",
            if n != 1 { "s" } else { "" },
            ms
        ));
    }
    let mut ev = serde_json::Map::new();
    ev.insert("version".into(), json!(version));
    if let Value::Object(m) = change.to_json() {
        for (k, v) in m {
            ev.insert(k, v);
        }
    }
    ev.insert("ms".into(), json!(ms));
    ev.insert("head".into(), json!(head));
    app.publish("index", &Value::Object(ev));
    Ok(())
}
