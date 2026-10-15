//! The client's formatting helpers (dates in local time, sizes, ages).

use crate::js::{self, num, pad2, to_fixed};

pub const MON: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// `fmtDate(t)`: "Mon DD"
pub fn date(t: f64) -> String {
    if t == 0.0 || t.is_nan() {
        return String::new();
    }
    let d = js::local(t);
    format!("{} {}", MON[d.month as usize], pad2(d.day))
}

/// `fmtTime(t)`: "HH:MM"
pub fn time(t: f64) -> String {
    if t == 0.0 || t.is_nan() {
        return String::new();
    }
    let d = js::local(t);
    format!("{}:{}", pad2(d.hours), pad2(d.minutes))
}

/// `fmtDT(t)`
pub fn dt(t: f64) -> String {
    if t == 0.0 || t.is_nan() {
        return String::new();
    }
    format!("{} {}", date(t), time(t))
}

/// `fmtSize(b)`
pub fn size(b: Option<f64>) -> String {
    let Some(b) = b else { return String::new() };
    if b < 1024.0 {
        format!("{} B", num(b))
    } else if b < 1048576.0 {
        format!(
            "{} K",
            to_fixed(b / 1024.0, if b < 10240.0 { 1 } else { 0 })
        )
    } else {
        format!("{} M", to_fixed(b / 1048576.0, 1))
    }
}

/// `ago(t)`
pub fn ago(t: f64) -> String {
    if t == 0.0 || t.is_nan() {
        return String::new();
    }
    let s = js::now_ms() / 1000.0 - t;
    if s < 60.0 {
        "now".into()
    } else if s < 3600.0 {
        format!("{}m", num((s / 60.0).floor()))
    } else if s < 86400.0 {
        format!("{}h", num((s / 3600.0).floor()))
    } else if s < 86400.0 * 60.0 {
        format!("{}d", num((s / 86400.0).floor()))
    } else if s < 86400.0 * 730.0 {
        format!("{}mo", num((s / (86400.0 * 30.0)).floor()))
    } else {
        format!("{}y", num((s / (86400.0 * 365.0)).floor()))
    }
}

/// `agoText(t)`
pub fn ago_text(t: f64) -> String {
    let a = ago(t);
    if a == "now" {
        "just now".into()
    } else if !a.is_empty() {
        format!("{a} ago")
    } else {
        String::new()
    }
}

/// `short(h)`
pub fn short(h: &str) -> String {
    js::slice16(h, 0, Some(7))
}

/// `dayKey(t)`: local YYYY-MM-DD
pub fn day_key(t: f64) -> String {
    let d = js::local(t);
    format!("{}-{}-{}", d.year, pad2(d.month + 1), pad2(d.day))
}
