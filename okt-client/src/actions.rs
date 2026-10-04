//! Actions the reference exposes as commands and buttons: open with an app, rebuild, time
//! travel, copy, blame, accent, expand/collapse.

use serde_json::json;

use crate::api::{self, spawn};
use crate::chrome::{self, toast};
use crate::dom;
use crate::graph;
use crate::insp;
use crate::js;
use crate::nav;
use crate::state::{self, Live, cfg_mut, s, s_mut, save_settings};
use crate::theme::{self, ACCENTS};

/// `openWith(how, path)`
pub fn open_with(how: &str, path: Option<&str>) {
    let path = path
        .filter(|p| !p.is_empty())
        .map(String::from)
        .or_else(state::sel)
        .unwrap_or_else(|| ".".into());
    let line = s().jump.unwrap_or(1);
    let how = how.to_string();
    spawn(async move {
        match api::post(
            "/api/open",
            &json!({"path": path, "how": how, "line": line}),
        )
        .await
        {
            Ok(_) => {
                toast(
                    match how.as_str() {
                        "editor" => "EDIT",
                        "reveal" => "REVEAL",
                        _ => "OPEN",
                    },
                    &path,
                    false,
                );
            }
            Err(e) => {
                toast("Could not open", &e.message(), true);
            }
        }
    });
}

/// `rebuild()`
pub fn rebuild() {
    chrome::set_live(Live::Busy);
    spawn(async {
        match api::post("/api/rebuild", &json!({})).await {
            Ok(r) => {
                let ms = r.get("ms").and_then(|v| v.as_f64()).unwrap_or(0.0);
                toast(
                    "REFRESH",
                    &format!("Rebuilt the index in {} ms", js::num((ms + 0.5).floor())),
                    false,
                );
            }
            Err(e) => {
                toast("REFRESH failed", &e.message(), true);
            }
        }
    });
}

/// `travel(rev)`
pub fn travel(rev: Option<&str>) {
    let is_file = state::idx()
        .zip(state::sel())
        .and_then(|(i, s)| i.node(&s).map(|n| n.t == "file"))
        .unwrap_or(false);
    if !is_file {
        toast("AT", "Select a file first", true);
        return;
    }
    {
        let mut st = s_mut();
        st.rev = rev.filter(|r| !r.is_empty()).map(String::from);
        st.tab = "content".into();
    }
    insp::render();
    nav::nav(true);
}

/// `copyText(t)`
pub fn copy_text(t: &str) {
    let t = t.to_string();
    let Some(clip) = Some(dom::win().navigator().clipboard()) else {
        return;
    };
    let p = clip.write_text(&t);
    spawn(async move {
        match wasm_bindgen_futures::JsFuture::from(p).await {
            Ok(_) => {
                toast("Copied", &t, false);
            }
            Err(_) => {
                toast("Copy failed", &t, true);
            }
        }
    });
}

/// `toggleBlame()`
pub fn toggle_blame() {
    let is_file = state::idx()
        .zip(state::sel())
        .and_then(|(i, s)| i.node(&s).map(|n| n.t == "file"))
        .unwrap_or(false);
    if !is_file {
        toast("BLAME", "Select a text file first", true);
        return;
    }
    {
        let mut st = s_mut();
        st.blame = !st.blame;
        if st.tab != "content" {
            st.tab = "content".into();
        }
    }
    insp::render();
}

/// `setAccent(a)`
pub fn set_accent(a: &str) {
    let mut v = js::lower(js::trim(a));
    if let Some((_, hex)) = ACCENTS.iter().find(|(k, _)| *k == v) {
        v = hex.to_string();
    }
    if v.len() == 6
        && v.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        v = format!("#{v}");
    }
    let ok = v.len() == 7
        && v.starts_with('#')
        && v[1..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if !ok {
        toast("ACCENT", "Use #rrggbb or a preset name", true);
        return;
    }
    cfg_mut().accent = v;
    save_settings();
    theme::apply_theme();
    chrome::render_settings();
}

/// `expandCmd(a, collapse)`
pub fn expand_cmd(a: &str, collapse: bool) {
    let Some(idx) = state::idx() else { return };
    if a.to_uppercase() == "ALL" {
        {
            let mut c = cfg_mut();
            if collapse {
                c.collapsed.clear();
            } else {
                for n in &idx.nodes {
                    if n.t == "dir" {
                        c.collapsed.insert(n.id.clone(), false);
                    }
                }
            }
        }
        save_settings();
        graph::build(&[]);
        toast(
            if collapse {
                "COLLAPSE ALL"
            } else {
                "EXPAND ALL"
            },
            if collapse {
                "Back to defaults"
            } else {
                "Every folder expanded"
            },
            false,
        );
        return;
    }
    let mut id = if !a.is_empty() {
        if idx.has(a) {
            a.to_string()
        } else if a.ends_with('/') {
            a.to_string()
        } else {
            format!("{a}/")
        }
    } else {
        state::current_folder()
    };
    if let Some(sel) = state::sel() {
        let r = graph::rep_of(&sel);
        if a.is_empty() && r != sel {
            id = r;
        }
    }
    if !idx.is_dir(&id) {
        toast(
            if collapse { "COLLAPSE" } else { "EXPAND" },
            &format!("No folder {a}"),
            true,
        );
        return;
    }
    graph::toggle_collapse(&id, Some(collapse));
}
