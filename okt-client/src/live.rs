//! Live updates: the server's event stream, and reloading the index when files or commits change.

use std::cell::RefCell;

use serde::Deserialize;
use wasm_bindgen::JsCast;
use web_sys::{EventSource, MessageEvent};

use crate::api::{self, spawn};
use crate::chrome::{self, toast};
use crate::dom;
use crate::fmt::short;
use crate::graph;
use crate::insp;
use crate::model::{Commit, Idx, TableData};
use crate::nav;
use crate::state::{self, Live, boot, s, s_mut};
use crate::views;

#[derive(Deserialize, Default, Clone, Debug)]
pub struct Change {
    #[serde(default)]
    pub version: Option<u64>,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub changed: Vec<String>,
    #[serde(default)]
    pub added: Vec<String>,
    #[serde(default)]
    pub removed: Vec<String>,
}

thread_local! {
    /// `reloadBusy`: Some while a reload runs; the inner Option is a queued follow-up.
    static BUSY: RefCell<Option<Option<Change>>> = const { RefCell::new(None) };
}

/// `reloadIndex(change)`
pub fn reload_index(change: Option<Change>) {
    let queued = BUSY.with(|b| {
        let mut b = b.borrow_mut();
        if let Some(again) = b.as_mut() {
            *again = Some(change.clone().or_else(|| again.clone()).unwrap_or_default());
            true
        } else {
            *b = Some(None);
            false
        }
    });
    if queued {
        return;
    }
    spawn(async move {
        if let Err(msg) = reload(change).await {
            toast("Index reload failed", &msg, true);
        }
        let again = BUSY.with(|b| b.borrow_mut().take().flatten());
        if let Some(a) = again {
            reload_index(Some(a));
        }
    });
}

async fn reload(change: Option<Change>) -> Result<(), String> {
    let text = api::get_text("/api/index").await.map_err(|e| e.message())?;
    let idx = Idx::parse(&text)?;
    let flash: Vec<String> = change
        .as_ref()
        .map(|c| c.changed.iter().chain(c.added.iter()).cloned().collect())
        .unwrap_or_default();
    {
        let mut st = s_mut();
        st.idx = Some(idx.clone());
        st.files.clear();
    }
    let reason = change.as_ref().map(|c| c.reason.as_str()).unwrap_or("");
    if reason == "commit" || reason == "rebuild" {
        {
            let mut st = s_mut();
            st.hist.clear();
            st.log_data = None;
        }
        views::log::ensure(true).await.map_err(|e| e.message())?;
        if s().view == "log" {
            views::log::render();
        }
    }
    if let Some(sel) = state::sel() {
        if !idx.has(&sel) {
            toast("Removed", &sel, false);
            s_mut().sel = None;
        }
    }
    graph::build(&flash);
    if s().view == "files" {
        views::tree::render();
    }
    let (view, folder, rec) = {
        let st = s();
        (st.view, st.table_folder.clone(), st.table_recursive)
    };
    if view == "table" {
        if let Some(f) = folder {
            let url = format!(
                "/api/table?folder={}&recursive={}",
                crate::js::encode_uri_component(&f),
                if rec { 1 } else { 0 }
            );
            if let Ok(t) = api::get::<TableData>(&url).await {
                s_mut().table = Some(std::rc::Rc::new(t));
                views::table::render();
            }
        }
    }
    nav::render_crumbs();
    let sel = state::sel();
    let touches = match &sel {
        None => true,
        Some(x) => {
            flash.contains(x)
                || change
                    .as_ref()
                    .map(|c| c.removed.contains(x))
                    .unwrap_or(false)
                || reason == "commit"
                || reason == "rebuild"
        }
    };
    if touches && s().commit.is_none() {
        insp::render();
    }
    if let Some(c) = &change {
        if c.reason == "files" {
            let all: Vec<&String> = c
                .changed
                .iter()
                .chain(c.added.iter())
                .chain(c.removed.iter())
                .collect();
            if !all.is_empty() {
                let title = if all.len() == 1 {
                    if !c.added.is_empty() {
                        "NEW FILE".to_string()
                    } else if !c.removed.is_empty() {
                        "DELETED".to_string()
                    } else {
                        "SAVED".to_string()
                    }
                } else {
                    format!("{} FILES CHANGED", all.len())
                };
                let body = all
                    .iter()
                    .take(3)
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
                    + if all.len() > 3 { " …" } else { "" };
                toast(&title, &body, false);
            }
        }
    }
    if s().live == Live::Busy {
        chrome::set_live(Live::Ok);
    }
    chrome::render_status();
    Ok(())
}

/// `onCommits(commits)`
fn on_commits(commits: Vec<Commit>) {
    {
        let mut st = s_mut();
        for c in &commits {
            st.new_commits.insert(c.h.clone());
        }
    }
    for c in commits.iter().take(3) {
        let t = toast(&format!("NEW COMMIT {}", short(&c.h)), &c.s, false);
        let h = c.h.clone();
        dom::on(t.as_ref(), "click", move |_: web_sys::MouseEvent| {
            insp::open_commit(&h)
        });
    }
    if commits.len() > 3 {
        toast("NEW COMMITS", &format!("{} more", commits.len() - 3), false);
    }
}

#[derive(Deserialize)]
struct Hello {
    version: Option<u64>,
    started: Option<f64>,
    build: Option<String>,
    #[serde(default)]
    watch: bool,
}

#[derive(Deserialize)]
struct Commits {
    #[serde(default)]
    commits: Vec<Commit>,
}

fn data_of(e: &MessageEvent) -> String {
    e.data().as_string().unwrap_or_default()
}

/// `connect()`
pub fn connect() {
    let es = match EventSource::new("/api/events") {
        Ok(es) => es,
        Err(_) => {
            chrome::set_live(Live::Err);
            return;
        }
    };
    let et: &web_sys::EventTarget = es.as_ref();
    dom::on(et, "hello", |e: MessageEvent| {
        let Ok(d) = serde_json::from_str::<Hello>(&data_of(&e)) else {
            return;
        };
        let b = boot();
        if let Some(build) = d.build.as_ref().filter(|x| !x.is_empty()) {
            if !b.build.is_empty() && *build != b.build {
                let _ = dom::win().location().reload(); // the server was upgraded
                return;
            }
        }
        let restarted = s().started.is_some() && d.started != s().started;
        s_mut().started = d.started;
        chrome::set_live(if d.watch || !b.watch {
            Live::Ok
        } else {
            Live::Off
        });
        if let Some(idx) = state::idx() {
            if restarted || d.version != Some(idx.version) {
                reload_index(Some(Change {
                    reason: "rebuild".into(),
                    ..Default::default()
                }));
            }
        }
    });
    dom::on(et, "index", |e: MessageEvent| {
        let Ok(d) = serde_json::from_str::<Change>(&data_of(&e)) else {
            return;
        };
        let stale = match state::idx() {
            None => true,
            Some(idx) => d.version != Some(idx.version),
        };
        if stale {
            reload_index(Some(d));
        }
    });
    dom::on(et, "commit", |e: MessageEvent| {
        let d: Commits = serde_json::from_str(&data_of(&e)).unwrap_or(Commits { commits: vec![] });
        on_commits(d.commits);
    });
    let onerr = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::Event)>::new(|_| {
        if s().live != Live::Off {
            chrome::set_live(Live::Err);
        }
    });
    es.set_onerror(Some(onerr.as_ref().unchecked_ref()));
    onerr.forget();
    let onopen = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::Event)>::new(|_| {
        if s().live == Live::Err {
            chrome::set_live(Live::Ok);
        }
    });
    es.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    onopen.forget();
    std::mem::forget(es);
}
