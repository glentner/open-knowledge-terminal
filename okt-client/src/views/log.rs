//! LOG: repository history with a commits-per-day chart and prefix chips.

use std::rc::Rc;

use indexmap::IndexMap;
use wasm_bindgen::JsCast;
use web_sys::{KeyboardEvent, MouseEvent};

use crate::api::{self, ApiError};
use crate::chrome;
use crate::dom;
use crate::fmt::{self, MON, day_key, short};
use crate::insp;
use crate::js::{self, esc, fmt_n, num, pad2, to_fixed};
use crate::model::{Commit, LogData};
use crate::panes;
use crate::state::{s, s_mut};

/// `ensureLog(force)`
pub async fn ensure(force: bool) -> Result<Rc<LogData>, ApiError> {
    if !force {
        if let Some(d) = s().log_data.clone() {
            return Ok(d);
        }
    }
    let d: LogData = api::get("/api/log")
        .await
        .inspect_err(|e| dom::log_err("log", &e.message()))?;
    let d = Rc::new(d);
    s_mut().log_data = Some(d.clone());
    chrome::render_ticker();
    Ok(d)
}

/// `[x] subject` without the prefix.
pub fn strip_prefix(s: &str) -> String {
    // /^\[[^\]]+\]\s*/
    if let Some(rest) = s.strip_prefix('[') {
        if let Some(end) = rest.find(']') {
            if end > 0 {
                return rest[end + 1..]
                    .trim_start_matches(js::is_js_space)
                    .to_string();
            }
        }
    }
    s.to_string()
}

fn matches(c: &Commit) -> bool {
    let st = s();
    if let Some(p) = &st.log_prefix {
        if c.x.as_deref() != Some(p.as_str()) {
            return false;
        }
    }
    if let Some(d) = &st.log_day {
        if day_key(c.t) != *d {
            return false;
        }
    }
    let q = js::lower(js::trim(&st.log_filter));
    if q.is_empty() {
        return true;
    }
    let subj = js::lower(&c.s);
    js::split_ws(&q).iter().all(|w| {
        c.h.starts_with(w.as_str())
            || subj.contains(w.as_str())
            || c.f
                .iter()
                .any(|f| js::lower(f.get(1).map(|s| s.as_str()).unwrap_or("")).contains(w.as_str()))
    })
}

/// Commits per day, in the order the days are first met.
fn per_day(all: &[Commit]) -> IndexMap<String, f64> {
    let mut days: IndexMap<String, f64> = IndexMap::new();
    for c in all {
        *days.entry(day_key(c.t)).or_insert(0.0) += 1.0;
    }
    days
}

/// The commits-per-day chart alone, drawn for the chart's own width: the rows fit by CSS, so a
/// change of the pane's width redraws only this.
pub fn render_chart() {
    let Some(data) = s().log_data.clone() else {
        return;
    };
    let all = &data.commits;
    let days = per_day(all);
    let chart = dom::id("lchart");
    let log_day = s().log_day.clone();
    if !all.is_empty() {
        let t1 = all[0].t;
        let t0 = all[all.len() - 1].t;
        let d0 = js::local_midnight(t0 * 1000.0);
        let d1 = js::local_midnight((t1 * 1000.0).max(js::now_ms()));
        let n = (((d1 - d0) / 86400000.0) + 0.5).floor() as i64 + 1;
        let cw = chart.client_width();
        let w = (if cw != 0 { cw as f64 } else { 800.0 }).max(200.0);
        let (h, pad_l, pad_b, pad_t) = (92.0, 28.0, 16.0, 8.0);
        let max = days.values().cloned().fold(f64::NEG_INFINITY, f64::max);
        let bw = (w - pad_l - 8.0) / n as f64;
        let mut svg = format!(
            "<svg viewBox=\"0 0 {} {}\" preserveAspectRatio=\"none\">",
            num(w),
            num(h)
        );
        for v in [max, (max / 2.0 + 0.5).floor()] {
            let y = pad_t + (h - pad_t - pad_b) * (1.0 - v / max);
            svg.push_str(&format!(
                "<line class=\"grid\" x1=\"{}\" x2=\"{}\" y1=\"{}\" y2=\"{}\"/><text class=\"axis\" x=\"{}\" y=\"{}\" text-anchor=\"end\">{}</text>",
                num(pad_l),
                num(w - 8.0),
                num(y),
                num(y),
                num(pad_l - 4.0),
                num(y + 3.0),
                num(v)
            ));
        }
        for i in 0..n {
            let d = js::local_ms(d0 + i as f64 * 86400000.0);
            let key = format!("{}-{}-{}", d.year, pad2(d.month + 1), pad2(d.day));
            let v = days.get(&key).copied().unwrap_or(0.0);
            let x = pad_l + i as f64 * bw;
            let lab = if n > 120 {
                d.day == 1
            } else {
                (d.weekday == 1 && (i > 3 || n < 14))
                    || (i == 0 && d.weekday >= 1 && d.weekday <= 4)
            };
            if lab && x + 40.0 < w {
                svg.push_str(&format!(
                    "<text class=\"axis\" x=\"{}\" y=\"{}\">{} {}</text>",
                    num(x + 1.0),
                    num(h - 4.0),
                    MON[d.month as usize],
                    d.day
                ));
            }
            if v == 0.0 {
                continue;
            }
            let hh = (h - pad_t - pad_b) * v / max;
            let dim = matches!(&log_day, Some(ld) if *ld != key);
            svg.push_str(&format!(
                "<rect class=\"bar{}\" data-day=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"><title>{} {} · {} commit{}</title></rect>",
                if dim { " dim" } else { "" },
                key,
                to_fixed(x + 0.5, 1),
                to_fixed(h - pad_b - hh, 1),
                to_fixed((bw - 1.0).max(1.0), 1),
                to_fixed(hh, 1),
                MON[d.month as usize],
                d.day,
                num(v),
                if v == 1.0 { "" } else { "s" }
            ));
        }
        dom::set_html(&chart, &(svg + "</svg>"));
    } else {
        dom::set_html(&chart, "<div class=\"empty\">No commits yet.</div>");
    }
}

/// `renderLog()`
pub fn render() {
    let Some(data) = s().log_data.clone() else {
        return;
    };
    let all = &data.commits;
    let list: Vec<&Commit> = all.iter().filter(|c| matches(c)).collect();
    let days = per_day(all);
    let log_day = s().log_day.clone();
    render_chart();
    // prefix chips
    let mut px: IndexMap<String, f64> = IndexMap::new();
    for c in all {
        let k =
            c.x.clone()
                .filter(|x| !x.is_empty())
                .unwrap_or_else(|| "—".into());
        *px.entry(k).or_insert(0.0) += 1.0;
    }
    let mut pxs: Vec<(String, f64)> = px.into_iter().collect();
    pxs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let prefix = s().log_prefix.clone();
    let mut chips: String = pxs
        .iter()
        .map(|(p, c)| {
            format!(
                "<button class=\"pchip{}\" data-px=\"{}\">{}<b>{}</b></button>",
                if prefix.as_deref() == Some(p.as_str()) {
                    " on"
                } else {
                    ""
                },
                esc(p),
                esc(p),
                num(*c)
            )
        })
        .collect();
    if let Some(ld) = &log_day {
        chips.push_str(&format!(
            "<button class=\"pchip on\" data-clearday=\"1\">{} ✕</button>",
            esc(ld)
        ));
    }
    dom::set_html(&dom::id("lchips"), &chips);
    // rows grouped by day
    let mut html = String::new();
    let mut last_day = String::new();
    let authors: std::collections::HashSet<&str> =
        all.iter().take(400).map(|c| c.an.as_str()).collect();
    let multi = authors.len() > 1;
    let (commit, new_commits) = {
        let st = s();
        (st.commit.clone(), st.new_commits.clone())
    };
    for c in list.iter().take(1500) {
        let dk = day_key(c.t);
        if dk != last_day {
            html.push_str(&format!(
                "<div class=\"dayrule\">{} · {}</div>",
                esc(&js::date_string(c.t)),
                days.get(&dk)
                    .map(|v| num(*v))
                    .unwrap_or_else(|| "undefined".into())
            ));
            last_day = dk;
        }
        let is_new = new_commits.contains(&c.h);
        let first = c.an.split(' ').next().unwrap_or("");
        html.push_str(&format!(
            "<div class=\"crow{}\" data-h=\"{}\"><span class=\"h\">{}{}</span><span class=\"dt\" title=\"{}\">{}{}</span><span class=\"px\">{}</span><span class=\"sj\">{}</span><span class=\"nf\">{}</span><span class=\"ad\"><span class=\"a\">+{}</span> <span class=\"d\">−{}</span></span></div>",
            if commit.as_deref() == Some(c.h.as_str()) { " on" } else { "" },
            c.h,
            short(&c.h),
            if is_new { " <b style=\"color:var(--accent-ink);background:var(--accent);padding:0 3px\">NEW</b>" } else { "" },
            esc(&c.an),
            fmt::time(c.t),
            if multi { format!(" {}", esc(&js::slice16(first, 0, Some(8)))) } else { String::new() },
            match &c.x { Some(x) if !x.is_empty() => format!("[{}]", esc(x)), _ => String::new() },
            esc(&strip_prefix(&c.s)),
            num(c.n),
            fmt_n(c.a),
            fmt_n(c.d),
        ));
    }
    dom::set_html(
        &dom::id("ltable"),
        if html.is_empty() {
            "<div class=\"empty\">No commits match.</div>"
        } else {
            &html
        },
    );
    dom::set_text(
        &dom::id("lmeta"),
        &format!(
            "{} of {} commits",
            fmt_n(list.len() as f64),
            fmt_n(all.len() as f64)
        ),
    );
}

static FILTER: dom::Debounce = dom::Debounce::new(100);
static FIT: dom::Debounce = dom::Debounce::new(100);

/// `wireLog()`
pub fn wire() {
    dom::on_id("lfilter", "input", |_: web_sys::Event| {
        FILTER.call(|| {
            s_mut().log_filter = dom::input("lfilter").value();
            render();
        })
    });
    dom::on_id("lchips", "click", |e: MouseEvent| {
        let Some(b) = dom::closest(&e.target(), ".pchip") else {
            return;
        };
        if dom::data(&b, "clearday").is_some() {
            s_mut().log_day = None;
        } else {
            let p = dom::data(&b, "px");
            let mut st = s_mut();
            st.log_prefix = if st.log_prefix == p { None } else { p };
        }
        render();
    });
    dom::on_id("lchart", "click", |e: MouseEvent| {
        let Some(r) = dom::closest(&e.target(), "rect.bar") else {
            return;
        };
        let d = dom::data(&r, "day");
        {
            let mut st = s_mut();
            st.log_day = if st.log_day == d { None } else { d };
        }
        render();
    });
    dom::on_id("ltable", "click", |e: MouseEvent| {
        if let Some(row) = dom::closest(&e.target(), ".crow") {
            insp::open_commit(&dom::data(&row, "h").unwrap_or_default());
        }
    });
    // the chart follows its pane's width, whatever changed it (a drag, a preset, a neighbour
    // collapsing, the window)
    let fit = wasm_bindgen::closure::Closure::<dyn FnMut(js_sys::Array)>::new(
        move |_entries: js_sys::Array| {
            FIT.call(|| {
                if panes::visible("log") {
                    render_chart();
                }
            })
        },
    );
    if let Ok(ro) = web_sys::ResizeObserver::new(fit.as_ref().unchecked_ref()) {
        ro.observe(&dom::id("lchart"));
    }
    fit.forget();
}

/// `logKey(e)`
pub fn key(e: &KeyboardEvent) -> bool {
    let rows = dom::qa("#ltable .crow");
    if rows.is_empty() {
        return false;
    }
    let mut i: i64 = rows
        .iter()
        .position(|r| dom::has_class(r, "on"))
        .map(|x| x as i64)
        .unwrap_or(-1);
    let n = rows.len() as i64;
    match e.key().as_str() {
        "ArrowDown" | "j" => i = (i + 1).min(n - 1),
        "ArrowUp" | "k" => i = (i - 1).max(0),
        _ => return false,
    }
    let row = &rows[i.max(0) as usize];
    insp::open_commit(&dom::data(row, "h").unwrap_or_default());
    dom::scroll_into_view_nearest(row);
    true
}
