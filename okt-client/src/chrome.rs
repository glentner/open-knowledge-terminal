//! Status bar, ticker, toasts, settings, help, and the app-install / offline screens.

use std::cell::RefCell;

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Element, Event, MouseEvent};

use crate::actions;
use crate::api::{self, spawn};
use crate::dom;
use crate::fmt::{self, short};
use crate::go::{self, CMDS};
use crate::graph;
use crate::insp;
use crate::js::{self, esc, fmt_n, num, pad2};
use crate::state::{self, Live, boot, cfg, cfg_mut, s, s_mut, save_settings};
use crate::theme::{self, ACCENTS};
use crate::views;

thread_local! {
    static CONN_TIMER: std::cell::Cell<i32> = const { std::cell::Cell::new(0) };
    static INSTALL: RefCell<Option<JsValue>> = const { RefCell::new(None) };
}

/// `setLive(state)`
pub fn set_live(state: Live) {
    let was = s().live;
    s_mut().live = state;
    if state != Live::Err {
        CONN_TIMER.with(|t| dom::clear_timeout(t.get()));
        dom::set_hidden(&dom::id("offbar"), true);
    } else if was != Live::Err {
        CONN_TIMER.with(|t| {
            dom::clear_timeout(t.get());
            t.set(dom::timeout(2500, || {
                if s().live == Live::Err {
                    dom::set_hidden(&dom::id("offbar"), false);
                }
            }));
        });
    }
    let el = dom::id("live");
    dom::toggle(&el, "off", state == Live::Off);
    dom::toggle(&el, "err", state == Live::Err);
    dom::toggle(&el, "busy", state == Live::Busy);
    if let Some(sp) = dom::q_in(&el, "span") {
        dom::set_text(
            &sp,
            match state {
                Live::Err => "OFFLINE",
                Live::Off => "PAUSED",
                Live::Busy => "INDEXING",
                Live::Ok => "LIVE",
            },
        );
    }
    render_status();
}

pub fn is_standalone() -> bool {
    let mq = dom::win()
        .match_media("(display-mode: standalone)")
        .ok()
        .flatten()
        .map(|m| m.matches())
        .unwrap_or(false);
    let ios = js_sys::Reflect::get(&dom::win().navigator(), &"standalone".into())
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    mq || ios
}

fn is_safari() -> bool {
    let ua = dom::win().navigator().user_agent().unwrap_or_default();
    js::regexp(
        "^((?!chrome|chromium|android|crios|fxios|edg).)*safari",
        "i",
    )
    .map(|re| re.test(&ua))
    .unwrap_or(false)
}

/// `renderStatus()`
pub fn render_status() {
    let Some(idx) = state::idx() else { return };
    let kb = &idx.kb;
    let files = idx.files().count();
    let dirty = kb.dirty.len();
    let b = &idx.built;
    let b_ = boot();
    let live = s().live;
    let live_label = match live {
        Live::Ok => {
            if b_.watch {
                format!("LIVE · {}s", num(b_.interval))
            } else {
                "MANUAL".into()
            }
        }
        Live::Err => "OFFLINE".into(),
        Live::Off => "PAUSED".into(),
        Live::Busy => "INDEXING".into(),
    };
    let html = format!(
        "<span class=\"seg {}\"><b>{}</b></span><span class=\"seg\">{}</span><button class=\"seg acc\" data-view=\"log\" title=\"Branch\">{}{}{}</button><button class=\"seg acc\" data-commit=\"{}\" title=\"Show HEAD\">HEAD <b>{}</b></button><span class=\"seg\"><b>{}</b> files</span><span class=\"seg\"><b>{}</b> edges</span><span class=\"seg\"><b>{}</b> commits</span><button class=\"seg{}\" data-list=\"BROKEN\"><b>{}</b> broken</button><button class=\"seg{}\" data-list=\"DIRTY\"><b>{}</b> dirty</button><span class=\"seg\" title=\"Last index build\">index <b>{}</b> ms · {}</span><span class=\"seg right\" id=\"clock\"></span><button class=\"seg\" id=\"helpseg\" style=\"border-right:0\"><b>?</b> help</button>",
        if live == Live::Err { "bad" } else { "acc" },
        esc(&live_label),
        esc(&kb.display),
        esc(kb
            .branch
            .as_deref()
            .filter(|x| !x.is_empty())
            .unwrap_or("detached")),
        if kb.ahead != 0 {
            format!(" <b style=\"color:var(--warn)\">↑{}</b>", kb.ahead)
        } else {
            String::new()
        },
        if kb.behind != 0 {
            format!(" <b style=\"color:var(--warn)\">↓{}</b>", kb.behind)
        } else {
            String::new()
        },
        esc(kb.head.as_deref().unwrap_or("")),
        short(kb.head.as_deref().unwrap_or("")),
        fmt_n(files as f64),
        fmt_n(idx.edges.len() as f64),
        fmt_n(idx.commits),
        if idx.broken.is_empty() { "" } else { " bad" },
        idx.broken.len(),
        if dirty > 0 { " warn" } else { "" },
        dirty,
        num((b.ms + 0.5).floor()),
        fmt::time(b.at),
    );
    dom::set_html(&dom::id("status"), &html);
    tick_clock();
    let nav = dom::win().navigator();
    if js_sys::Reflect::has(&nav, &"setAppBadge".into()).unwrap_or(false) && is_standalone() {
        let n = if cfg().badge == Some(false) { 0 } else { dirty };
        let name = if n > 0 {
            "setAppBadge"
        } else {
            "clearAppBadge"
        };
        if let Ok(f) =
            js_sys::Reflect::get(&nav, &name.into()).and_then(|f| f.dyn_into::<js_sys::Function>())
        {
            let r = if n > 0 {
                f.call1(&nav, &JsValue::from_f64(n as f64))
            } else {
                f.call0(&nav)
            };
            if let Ok(p) = r.and_then(|p| p.dyn_into::<js_sys::Promise>()) {
                let noop = wasm_bindgen::closure::Closure::<dyn FnMut(JsValue)>::new(|_| {});
                let _ = p.catch(&noop);
                noop.forget();
            }
        }
    }
    dom::set_text(
        &dom::id("headchip"),
        &format!(
            "{} · {}",
            kb.branch
                .as_deref()
                .filter(|x| !x.is_empty())
                .unwrap_or("HEAD"),
            short(kb.head.as_deref().unwrap_or(""))
        ),
    );
    dom::set_text(&dom::id("kbname"), &kb.display);
}

/// `tickClock()`
pub fn tick_clock() {
    if let Some(c) = dom::try_id("clock") {
        let d = js::local_ms(js::now_ms());
        dom::set_html(
            &c,
            &format!(
                "<b>{}:{}:{}</b>",
                pad2(d.hours),
                pad2(d.minutes),
                pad2(d.seconds)
            ),
        );
    }
}

/// `renderTicker()`
pub fn render_ticker() {
    let tr = dom::id("ttrack");
    let data = s().log_data.clone();
    let Some(data) = data.filter(|_| cfg().ticker) else {
        dom::set_html(&tr, "");
        return;
    };
    let newc = s().new_commits.clone();
    let html: Vec<String> = data
        .commits
        .iter()
        .take(24)
        .map(|c| {
            format!(
                "<button data-commit=\"{}\">{}<span class=\"h\">{}</span><span class=\"x\">{}</span><span class=\"s\">{}</span><span class=\"a\">{}</span></button>",
                c.h,
                if newc.contains(&c.h) { "<span class=\"new\">NEW</span>" } else { "" },
                short(&c.h),
                match &c.x {
                    Some(x) if !x.is_empty() => format!("[{}]", esc(x)),
                    _ => String::new(),
                },
                esc(&views::log::strip_prefix(&c.s)),
                fmt::ago_text(c.t)
            )
        })
        .collect();
    dom::set_html(&tr, &html.join("<span class=\"a\">·</span>"));
    dom::raf(move |_| {
        let w = tr.scroll_width() as f64;
        dom::set_style(&tr, "--tdur", &format!("{}s", num((w / 55.0).max(40.0))));
    });
}

/// `toast(title, body, err)`
pub fn toast(title: &str, body: &str, err: bool) -> Element {
    let el = dom::doc().create_element("div").unwrap();
    el.set_class_name(if err { "toast err" } else { "toast" });
    dom::set_html(
        &el,
        &format!(
            "<div class=\"tt\">{}</div><div class=\"tb\">{}</div>",
            esc(title),
            esc(body)
        ),
    );
    let host = dom::id("toasts");
    let _ = host.append_child(&el);
    // the toast comes back as currentTarget: a handler that captured `el` would hold it, and
    // through it this listener, for the life of the page
    dom::on(el.as_ref(), "click", |e: MouseEvent| {
        if let Some(t) = e
            .current_target()
            .and_then(|t| t.dyn_into::<Element>().ok())
        {
            t.remove();
        }
    });
    let e3 = el.clone();
    dom::timeout(if err { 6000 } else { 3200 }, move || e3.remove());
    while host.children().length() > 5 {
        if let Some(f) = host.first_element_child() {
            f.remove();
        }
    }
    el
}

/// `renderSettings()`
pub fn render_settings() {
    let el = dom::id("settings");
    let c = cfg().clone();
    let t = c.theme.as_str();
    let rd = c.reader_font();
    let b = boot();
    let opt = |list: &[&str], cur: &str, attr: &str| -> String {
        list.iter()
            .map(|x| {
                format!(
                    "<button class=\"opt{}\" data-{attr}=\"{x}\">{}</button>",
                    if cur == *x { " on" } else { "" },
                    x.to_uppercase()
                )
            })
            .collect()
    };
    let swatches: String = ACCENTS
        .iter()
        .map(|(k, v)| format!("<button class=\"sw{}\" data-accent=\"{v}\" title=\"{k}\" style=\"background:{v}\"></button>", if c.accent == *v { " on" } else { "" }))
        .collect();
    let live = s().live;
    let has_install = INSTALL.with(|i| i.borrow().is_some());
    let app_row = if is_standalone() {
        format!(
            "<button class=\"opt{}\" data-badge=\"1\" title=\"Dock badge counts uncommitted files\">DOCK BADGE</button><span class=\"note\" style=\"margin:0\">running as the {} app</span>",
            if c.badge == Some(false) { "" } else { " on" },
            esc(b.app_name())
        )
    } else if has_install {
        "<button class=\"opt\" data-install=\"1\">INSTALL AS AN APP</button><span class=\"note\" style=\"margin:0\">keeps it in the Dock</span>".to_string()
    } else {
        format!(
            "<span class=\"note\" style=\"margin:0\">{} to keep it in the Dock as its own app</span>",
            if is_safari() {
                "Safari › File › Add to Dock…"
            } else {
                "Install it from the address bar or the browser menu"
            }
        )
    };
    let html = format!(
        "<h4>THEME</h4><div class=\"row\">{}</div>\n    <h4>ACCENT</h4><div class=\"row\">{swatches}</div>\n    <div class=\"row\" style=\"margin-top:6px\"><input type=\"color\" id=\"acpick\" value=\"{}\"><input class=\"hex\" id=\"achex\" value=\"{}\" spellcheck=\"false\"><span class=\"note\" style=\"margin:0\">{}</span></div>\n    <h4>READER FONT</h4><div class=\"row\">{}</div>\n    <h4>GRAPH</h4><div class=\"row\"><span class=\"note\" style=\"margin:0\">collapse folders over</span><input class=\"hex\" id=\"colN\" style=\"width:54px\" value=\"{}\"><span class=\"note\" style=\"margin:0\">files</span><button class=\"opt\" data-reset=\"1\">RESET EXPANSIONS</button></div>\n    <h4>DISPLAY</h4><div class=\"row\"><button class=\"opt{}\" data-ticker=\"1\">COMMIT TICKER</button><button class=\"opt{}\" data-labels=\"1\">ALL LABELS</button>{}</div>\n    <h4>APP</h4><div class=\"row\">{app_row}</div>\n    <div class=\"note\">{} {} · editor: {} · read-only</div>",
        opt(&["terminal", "dark", "light"], t, "theme"),
        esc(&c.accent),
        esc(&c.accent),
        if t == "terminal" {
            "drives the whole terminal theme"
        } else {
            "UI accent; families keep their colors"
        },
        opt(&["mono", "sans", "serif"], &rd, "reader"),
        state::collapse_n(),
        if c.ticker { " on" } else { "" },
        if c.labels { " on" } else { "" },
        if b.watch {
            format!(
                "<button class=\"opt{}\" data-pause=\"1\">PAUSE WATCH</button>",
                if live == Live::Off { " on" } else { "" }
            )
        } else {
            String::new()
        },
        esc(b.app_name()),
        esc(&b.version),
        esc(b.editor.as_deref().unwrap_or("none")),
    );
    dom::set_html(&el, &html);
}

/// `wireSettings()`
pub fn wire_settings() {
    let el = dom::id("settings");
    dom::on_id("setbtn", "click", |e: MouseEvent| {
        e.stop_propagation();
        let el = dom::id("settings");
        let h = !dom::hidden(&el);
        dom::set_hidden(&el, h);
        if !h {
            render_settings();
        }
    });
    let el2 = el.clone();
    dom::on(dom::doc().as_ref(), "mousedown", move |e: MouseEvent| {
        let setbtn = dom::id("setbtn");
        let on_btn = e
            .target()
            .map(|t| &t == AsRef::<web_sys::EventTarget>::as_ref(&setbtn))
            .unwrap_or(false);
        if !dom::hidden(&el2) && !dom::contains(&el2, &e.target()) && !on_btn {
            dom::set_hidden(&el2, true);
        }
    });
    dom::on(el.as_ref(), "click", |e: MouseEvent| {
        let Some(b) = dom::closest(&e.target(), "button") else {
            return;
        };
        if let Some(t) = dom::data(&b, "theme") {
            cfg_mut().theme = t;
            theme::apply_theme();
            graph::build(&[]);
            insp::render();
        }
        if let Some(a) = dom::data(&b, "accent") {
            actions::set_accent(&a);
        }
        if let Some(r) = dom::data(&b, "reader") {
            cfg_mut().reader = Some(r);
            theme::apply_theme();
            insp::render();
        }
        if dom::data(&b, "ticker").is_some() {
            {
                let mut c = cfg_mut();
                c.ticker = !c.ticker;
            }
            theme::apply_theme();
            render_ticker();
        }
        if dom::data(&b, "labels").is_some() {
            graph::toggle_labels();
        }
        if dom::data(&b, "reset").is_some() {
            cfg_mut().collapsed.clear();
            graph::build(&[]);
        }
        if dom::data(&b, "badge").is_some() {
            {
                let mut c = cfg_mut();
                c.badge = Some(c.badge == Some(false));
            }
            render_status();
        }
        if dom::data(&b, "install").is_some() {
            if let Some(ev) = INSTALL.with(|i| i.borrow_mut().take()) {
                if let Ok(f) = js_sys::Reflect::get(&ev, &"prompt".into())
                    .and_then(|f| f.dyn_into::<js_sys::Function>())
                {
                    let _ = f.call0(&ev);
                }
            }
        }
        if dom::data(&b, "pause").is_some() {
            let paused = s().live != Live::Off;
            spawn(async move {
                match api::post("/api/watch", &serde_json::json!({"paused": paused})).await {
                    Ok(_) => set_live(if paused { Live::Off } else { Live::Ok }),
                    Err(err) => {
                        toast("Watch", &err.message(), true);
                    }
                }
                save_settings();
                render_settings();
            });
            return;
        }
        save_settings();
        render_settings();
    });
    dom::on(el.as_ref(), "input", |e: Event| {
        let Some(t) = dom::target_el(&e) else { return };
        if t.id() == "acpick" {
            let v = t.unchecked_ref::<web_sys::HtmlInputElement>().value();
            cfg_mut().accent = v.clone();
            theme::apply_theme();
            dom::input("achex").set_value(&v);
            save_settings();
        }
    });
    dom::on(el.as_ref(), "change", |e: Event| {
        let Some(t) = dom::target_el(&e) else { return };
        let v = t
            .dyn_ref::<web_sys::HtmlInputElement>()
            .map(|i| i.value())
            .unwrap_or_default();
        if t.id() == "achex" {
            actions::set_accent(&v);
        }
        if t.id() == "colN" {
            if let Some(n) = js::parse_int(&v) {
                if n >= 0 {
                    cfg_mut().collapse = Some(n);
                    save_settings();
                    graph::build(&[]);
                }
            }
        }
    });
    // installable app
    dom::on(dom::win().as_ref(), "beforeinstallprompt", |e: Event| {
        e.prevent_default();
        INSTALL.with(|i| *i.borrow_mut() = Some(e.into()));
    });
    dom::on(dom::win().as_ref(), "appinstalled", |_: Event| {
        INSTALL.with(|i| *i.borrow_mut() = None);
        toast(
            "INSTALLED",
            &format!(
                "{} is an app now; okt opens it from here on",
                boot().app_name()
            ),
            false,
        );
    });
}

/// `showHelp()`
pub fn show_help() {
    let el = dom::id("help");
    #[rustfmt::skip]
    let keys: &[(&str, &str)] = &[
        ("/ or :", "focus the GO bar"),
        ("g f l t", "graph, files, log, table"),
        ("o c h", "overview, content, history"),
        ("s b", "source on/off, blame on/off"),
        (", .", "step a file back or forward through its history"),
        ("j k", "graph: step through the selection's connections"),
        ("⏎ space", "graph: go to the one marked; or open/close the folder"),
        ("e", "expand or collapse the folder in the graph"),
        ("u", "up to the parent folder; in files, fold everything under it"),
        ("z", "zoom the graph to the selection and its neighbors"),
        ("J K", "scroll the inspector down, up"),
        ("n N", "next, previous FIND match"),
        ("y", "copy the path"),
        ("m 1–9", "pin the selection; go to a pin"),
        ("E O F", "edit, open, reveal"),
        ("[ ]", "back, forward"),
        ("↑ ↓ ⏎", "move and open in files and the log (j k move too)"),
        ("space", "files: open or close a folder, or show a file in the graph"),
        ("+ − 0", "zoom in, out, fit"),
        ("R", "rebuild the index"),
        ("esc", "close, then clear the selection"),
        ("?", "this help"),
    ];
    // the arrangement: a row per preset key, the cycle, and the pane leader's keys
    #[rustfmt::skip]
    let pane_keys: &[(&str, &str)] = &[
        (";", "classic: views left, Inspector right"),
        ("'", "swapped: Inspector left, views right"),
        ("\\", "stacked: views on top, Inspector below"),
        ("`", "three: the Graph's bar on top, Inspector left, Files right"),
        ("p", "cycle the presets"),
        ("w", "then a pane key, acting on the focused pane (a click in a pane focuses it)"),
        ("w w  w W", "focus the next, previous pane"),
        ("w d", "collapse the focused pane to its title bar, or expand it"),
        ("w x", "swap it with its neighbour, each keeping its views"),
        ("w r", "turn its split between side by side and stacked"),
        ("w q", "close it; its views go to its neighbour, which takes the space"),
        ("w v  w V", "move its view to its neighbour; moving the last closes it"),
    ];
    let pd: String = pane_keys
        .iter()
        .map(|(k, d)| format!("<dt><kbd>{}</kbd></dt><dd>{}</dd>", esc(k), esc(d)))
        .collect();
    let pins: String = {
        let p = cfg().pins.clone();
        if p.is_empty() {
            "<dt>m</dt><dd style=\"color:var(--dim)\">pins the selection; then 1–9 go there</dd>"
                .into()
        } else {
            p.iter()
                .enumerate()
                .map(|(i, id)| format!("<dt><kbd>{}</kbd></dt><dd>{}</dd>", i + 1, esc(id)))
                .collect()
        }
    };
    let kd: String = keys
        .iter()
        .map(|(k, d)| format!("<dt><kbd>{}</kbd></dt><dd>{}</dd>", esc(k), esc(d)))
        .collect();
    let cd: String = CMDS
        .iter()
        .map(|c| {
            format!(
                "<dt>{}{}</dt><dd>{}</dd>",
                esc(c.k),
                c.arg
                    .map(|a| format!(" <span style=\"color:var(--dim)\">{}</span>", esc(a)))
                    .unwrap_or_default(),
                esc(&go::desc(c))
            )
        })
        .collect();
    dom::set_html(
        &el,
        &format!(
            "<div class=\"helpbox\"><h3>OK TERMINAL · HELP</h3>\n    <div><h4>KEYS</h4><dl>{kd}</dl>\n    <h4>PANES</h4><dl>{pd}</dl>\n    <h4>GRAPH</h4><dl><dt>click</dt><dd>shine a light on a node and everything it touches</dd><dt>double-click</dt><dd>expand or collapse a folder; empty space fits</dd><dt>drag, wheel</dt><dd>pan, zoom</dd><dt>accent arrows</dt><dd>links out; pale lines are links in</dd><dt>dashed</dt><dd>connections to external systems</dd></dl>\n    <h4>PINS</h4><dl>{pins}</dl></div>\n    <div><h4>COMMANDS</h4><dl>{cd}</dl></div>\n    <div class=\"full\">Anything else you type opens the best-matching file or folder; a commit hash opens that commit; if nothing matches it runs FIND. The terminal is read-only; EDIT, OPEN and REVEAL hand the file to your apps.</div></div>"
        ),
    );
    dom::set_hidden(&el, false);
}

/// `waitForServer()`: the app shell came from the service worker and the server is down.
pub fn wait_for_server() {
    set_live(Live::Err);
    dom::set_hidden(&dom::id("offbar"), true);
    let el = dom::id("offline");
    let host = dom::win().location().host().unwrap_or_default();
    dom::set_html(
        &el,
        &format!(
            "<div class=\"offbox\"><img src=\"/favicon.svg\" alt=\"\" width=\"56\" height=\"56\"><h3>{} isn't running</h3>\n    <p>Start it from your launcher, or run <code>okt --detach</code> in the KB. This window connects as soon as the server answers.</p>\n    <p class=\"offwait\"><i></i>waiting for {}</p></div>",
            esc(boot().app_name()),
            esc(&host)
        ),
    );
    dom::set_hidden(&el, false);
    fn tick() {
        dom::timeout(1500, || {
            spawn(async {
                if api::hello_ok().await {
                    let _ = dom::win().location().reload();
                } else {
                    tick();
                }
            })
        });
    }
    tick();
}

pub fn register_sw() {
    let w = dom::win();
    if js_sys::Reflect::has(&w.navigator(), &"serviceWorker".into()).unwrap_or(false)
        && w.is_secure_context()
    {
        let p = w.navigator().service_worker().register("/sw.js");
        let noop = wasm_bindgen::closure::Closure::<dyn FnMut(JsValue)>::new(|_| {});
        let _ = p.catch(&noop);
        noop.forget();
    }
}
