//! JavaScript semantics, where the reference client's output depends on them: number to string,
//! `toFixed`, `toLocaleString('en-US')`, `String(value)`, `JSON.stringify`, `parseFloat`, string
//! whitespace, UTF-16 lengths and slices, object key order, dates in local time, and collation
//! (`localeCompare`). The views are ported line for line, and these keep their text identical.

use std::collections::HashMap;

use serde_json::{Map, Value};
use wasm_bindgen::{JsCast, JsValue};

// ---- escaping -------------------------------------------------------------------------------

/// `esc()` of the reference client: `& < > " '`.
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

// ---- numbers -------------------------------------------------------------------------------

/// `Number.prototype.toString()` (radix 10): shortest round-trip digits, exponent form outside
/// [1e-6, 1e21).
pub fn num(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x == 0.0 {
        return "0".into();
    }
    if x.is_infinite() {
        return if x > 0.0 {
            "Infinity".into()
        } else {
            "-Infinity".into()
        };
    }
    let neg = x < 0.0;
    let a = x.abs();
    // shortest digits and exponent from Rust's `{:e}` (also shortest round-trip)
    let e = format!("{a:e}");
    let (mant, exp) = e.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i32;
    let n = exp + 1; // position of the decimal point relative to the digits
    let mut s = String::new();
    if neg {
        s.push('-');
    }
    if k <= n && n <= 21 {
        s.push_str(&digits);
        for _ in 0..(n - k) {
            s.push('0');
        }
    } else if 0 < n && n <= 21 {
        s.push_str(&digits[..n as usize]);
        s.push('.');
        s.push_str(&digits[n as usize..]);
    } else if -6 < n && n <= 0 {
        s.push_str("0.");
        for _ in 0..(-n) {
            s.push('0');
        }
        s.push_str(&digits);
    } else {
        let e = n - 1;
        s.push_str(&digits[..1]);
        if k > 1 {
            s.push('.');
            s.push_str(&digits[1..]);
        }
        s.push('e');
        s.push(if e < 0 { '-' } else { '+' });
        s.push_str(&e.abs().to_string());
    }
    s
}

/// `Number.prototype.toFixed(d)`: rounds the exact binary value, ties to the larger magnitude.
pub fn to_fixed(x: f64, d: usize) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.abs() >= 1e21 {
        return num(x);
    }
    let neg = x < 0.0;
    let a = x.abs();
    // exact decimal expansion (enough digits for any double >= 1e-30 in this range)
    let exact = format!("{a:.1100}");
    let (int, frac) = exact.split_once('.').unwrap();
    let mut digits: Vec<u8> = int
        .bytes()
        .chain(frac.bytes().take(d))
        .map(|b| b - b'0')
        .collect();
    let rest = &frac.as_bytes()[d.min(frac.len())..];
    let round_up = match rest.first() {
        Some(b) if *b > b'5' => true,
        Some(b'5') => true, // exactly half or more: ties go to the larger n
        _ => false,
    };
    if round_up {
        let mut i = digits.len();
        loop {
            if i == 0 {
                digits.insert(0, 1);
                break;
            }
            i -= 1;
            if digits[i] == 9 {
                digits[i] = 0;
            } else {
                digits[i] += 1;
                break;
            }
        }
    }
    let int_len = digits.len() - d;
    let mut s = String::new();
    if neg {
        s.push('-');
    }
    for &dg in &digits[..int_len] {
        s.push((b'0' + dg) as char);
    }
    if d > 0 {
        s.push('.');
        for &dg in &digits[int_len..] {
            s.push((b'0' + dg) as char);
        }
    }
    s
}

/// `(n ?? 0).toLocaleString('en-US')`.
pub fn fmt_n(x: f64) -> String {
    if x.fract() == 0.0 && x.abs() < 9.0e15 {
        let v = x as i64;
        let digits = v.unsigned_abs().to_string();
        let mut out = String::new();
        if v < 0 {
            out.push('-');
        }
        let n = digits.len();
        for (i, c) in digits.chars().enumerate() {
            if i > 0 && (n - i) % 3 == 0 {
                out.push(',');
            }
            out.push(c);
        }
        return out;
    }
    let n = js_sys::Number::from(x);
    n.to_locale_string("en-US")
        .as_string()
        .unwrap_or_else(|| num(x))
}

/// `parseFloat(s)`: the longest prefix (after leading whitespace) that is a decimal literal.
pub fn parse_float(s: &str) -> f64 {
    let t = s.trim_start_matches(is_js_space);
    let b = t.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    if t[i..].starts_with("Infinity") {
        return if b.first() == Some(&b'-') {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
    }
    let start_digits = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let mut frac_digits = 0;
    if i < b.len() && b[i] == b'.' {
        let j = i + 1;
        let mut k = j;
        while k < b.len() && b[k].is_ascii_digit() {
            k += 1;
        }
        frac_digits = k - j;
        if i > start_digits || frac_digits > 0 {
            i = k;
        }
    }
    if i == start_digits || (i == start_digits + 1 && b[start_digits] == b'.' && frac_digits == 0) {
        return f64::NAN;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut k = i + 1;
        if k < b.len() && (b[k] == b'+' || b[k] == b'-') {
            k += 1;
        }
        let ds = k;
        while k < b.len() && b[k].is_ascii_digit() {
            k += 1;
        }
        if k > ds {
            i = k;
        }
    }
    t[..i].parse::<f64>().unwrap_or(f64::NAN)
}

/// `parseInt(s, 10)`.
pub fn parse_int(s: &str) -> Option<i64> {
    let t = s.trim_start_matches(is_js_space);
    let (neg, rest) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let v: i64 = digits.parse().unwrap_or(i64::MAX);
    Some(if neg { -v } else { v })
}

// ---- strings -------------------------------------------------------------------------------

/// JS `\s` (WhiteSpace + LineTerminator).
pub fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\x0b' | '\x0c' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

/// `String.prototype.trim()`.
pub fn trim(s: &str) -> &str {
    s.trim_matches(is_js_space)
}

/// `s.split(/\s+/)` (leading/trailing whitespace gives empty first/last parts, like JS).
pub fn split_ws(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_ws = false;
    for c in s.chars() {
        if is_js_space(c) {
            if !in_ws {
                out.push(std::mem::take(&mut cur));
                in_ws = true;
            }
        } else {
            in_ws = false;
            cur.push(c);
        }
    }
    out.push(cur);
    out
}

/// `s.split(/\s+/).filter(Boolean)`.
pub fn words(s: &str) -> Vec<String> {
    split_ws(s).into_iter().filter(|w| !w.is_empty()).collect()
}

/// `String.prototype.toLowerCase()`.
pub fn lower(s: &str) -> String {
    s.to_lowercase()
}

/// `s.length` (UTF-16 code units).
pub fn len16(s: &str) -> usize {
    s.encode_utf16().count()
}

/// `s.slice(a, b)` on UTF-16 code units (negative and out-of-range indices like JS).
pub fn slice16(s: &str, a: i64, b: Option<i64>) -> String {
    let u: Vec<u16> = s.encode_utf16().collect();
    let n = u.len() as i64;
    let norm = |i: i64| if i < 0 { (n + i).max(0) } else { i.min(n) };
    let a = norm(a);
    let b = norm(b.unwrap_or(n));
    if a >= b {
        return String::new();
    }
    String::from_utf16_lossy(&u[a as usize..b as usize])
}

/// `hay.indexOf(needle)` in UTF-16 code units, or -1.
pub fn index_of16(hay: &[u16], needle: &[u16]) -> i64 {
    if needle.is_empty() {
        return 0;
    }
    if needle.len() > hay.len() {
        return -1;
    }
    for i in 0..=(hay.len() - needle.len()) {
        if &hay[i..i + needle.len()] == needle {
            return i as i64;
        }
    }
    -1
}

pub fn u16s(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// `str.padStart(2, '0')` for small non-negative numbers.
pub fn pad2(n: u32) -> String {
    format!("{n:02}")
}

// ---- values (JSON from the server, seen the way JS sees it) ----------------------------------

/// Whether `k` is an array index ("0".."4294967294" in canonical form): JS orders such keys first.
fn is_array_index(k: &str) -> bool {
    if k.is_empty() || k.len() > 10 || !k.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    if k.len() > 1 && k.starts_with('0') {
        return false;
    }
    k.parse::<u64>().map(|v| v < 4294967295).unwrap_or(false)
}

/// `Object.entries(o)` order: array-index keys ascending, then the rest in insertion order.
pub fn entries(m: &Map<String, Value>) -> Vec<(&String, &Value)> {
    let mut idx: Vec<(&String, &Value)> = m.iter().filter(|(k, _)| is_array_index(k)).collect();
    if idx.is_empty() {
        return m.iter().collect();
    }
    idx.sort_by_key(|(k, _)| k.parse::<u64>().unwrap());
    idx.extend(m.iter().filter(|(k, _)| !is_array_index(k)));
    idx
}

/// The same order for keys of a map we build ourselves (e.g. counts by type).
pub fn order_keys<T: Clone>(m: &indexmap::IndexMap<String, T>) -> Vec<(String, T)> {
    let mut idx: Vec<(String, T)> = m
        .iter()
        .filter(|(k, _)| is_array_index(k))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    idx.sort_by_key(|(k, _)| k.parse::<u64>().unwrap());
    idx.extend(
        m.iter()
            .filter(|(k, _)| !is_array_index(k))
            .map(|(k, v)| (k.clone(), v.clone())),
    );
    idx
}

/// `String(v)`.
pub fn string(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => num(n.as_f64().unwrap_or(f64::NAN)),
        Value::String(s) => s.clone(),
        Value::Array(a) => a
            .iter()
            .map(|x| {
                if x.is_null() {
                    String::new()
                } else {
                    string(x)
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".into(),
    }
}

/// `JSON.stringify(v)`.
pub fn json_stringify(v: &Value) -> String {
    let mut out = String::new();
    stringify_into(v, &mut out);
    out
}

fn stringify_into(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            let x = n.as_f64().unwrap_or(f64::NAN);
            out.push_str(&if x.is_finite() { num(x) } else { "null".into() })
        }
        Value::String(s) => quote_json(s, out),
        Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                stringify_into(x, out);
            }
            out.push(']');
        }
        Value::Object(m) => {
            out.push('{');
            for (i, (k, x)) in entries(m).into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                quote_json(k, out);
                out.push(':');
                stringify_into(x, out);
            }
            out.push('}');
        }
    }
}

fn quote_json(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

// ---- dates (local time, like `new Date(t * 1000)`) -------------------------------------------

pub struct LocalTime {
    pub year: i32,
    pub month: u32, // 0-based
    pub day: u32,
    pub weekday: u32,
    pub hours: u32,
    pub minutes: u32,
    pub seconds: u32,
}

pub fn local_ms(ms: f64) -> LocalTime {
    let d = js_sys::Date::new(&JsValue::from_f64(ms));
    LocalTime {
        year: d.get_full_year() as i32,
        month: d.get_month(),
        day: d.get_date(),
        weekday: d.get_day(),
        hours: d.get_hours(),
        minutes: d.get_minutes(),
        seconds: d.get_seconds(),
    }
}

pub fn local(t: f64) -> LocalTime {
    local_ms(t * 1000.0)
}

pub fn now_ms() -> f64 {
    js_sys::Date::now()
}

/// `new Date(t * 1000).toDateString()`.
pub fn date_string(t: f64) -> String {
    js_sys::Date::new(&JsValue::from_f64(t * 1000.0))
        .to_date_string()
        .into()
}

/// Local midnight (ms) of the day containing `ms` (`d.setHours(0, 0, 0, 0)`).
pub fn local_midnight(ms: f64) -> f64 {
    let d = js_sys::Date::new(&JsValue::from_f64(ms));
    d.set_hours(0);
    d.set_minutes(0);
    d.set_seconds(0);
    d.set_milliseconds(0);
    d.get_time()
}

// ---- collation (`localeCompare`) -------------------------------------------------------------

/// Ranks of `items` in the order `Intl.Collator(undefined, {numeric})` sorts them; strings the
/// collator considers equal share a rank, so a stable sort by rank reproduces a stable JS sort
/// with `localeCompare`.
pub fn collation_ranks<'a>(
    items: impl IntoIterator<Item = &'a str>,
    numeric: bool,
) -> HashMap<String, u32> {
    let mut uniq: Vec<&str> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for s in items {
        if seen.insert(s) {
            uniq.push(s);
        }
    }
    let arr = js_sys::Array::new();
    for s in &uniq {
        arr.push(&JsValue::from_str(s));
    }
    let opts = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&opts, &"numeric".into(), &JsValue::from_bool(numeric));
    let coll = js_sys::Intl::Collator::new(&js_sys::Array::new(), &opts);
    let compare: js_sys::Function = coll.compare();
    if let Ok(sort) = js_sys::Reflect::get(&arr, &JsValue::from_str("sort"))
        .and_then(|f| f.dyn_into::<js_sys::Function>())
    {
        let _ = sort.call1(&arr, &compare);
    }
    let mut ranks = HashMap::with_capacity(uniq.len());
    let mut rank = 0u32;
    let mut prev: Option<JsValue> = None;
    for v in arr.iter() {
        if let Some(p) = &prev {
            let c = compare
                .call2(&JsValue::NULL, p, &v)
                .ok()
                .and_then(|x| x.as_f64())
                .unwrap_or(1.0);
            if c != 0.0 {
                rank += 1;
            }
        }
        if let Some(s) = v.as_string() {
            ranks.insert(s, rank);
        }
        prev = Some(v);
    }
    ranks
}

pub fn encode_uri_component(s: &str) -> String {
    js_sys::encode_uri_component(s).into()
}

pub fn decode_uri_component(s: &str) -> Option<String> {
    js_sys::decode_uri_component(s).ok().map(|v| v.into())
}

/// `CSS.escape(s)`.
pub fn css_escape(s: &str) -> String {
    web_sys::css::escape(s)
}

/// `new RegExp(pattern, flags)`, or None where the constructor throws (no `eval` needed, so
/// it works under the page's CSP).
pub fn regexp(pattern: &str, flags: &str) -> Option<js_sys::RegExp> {
    let ctor = js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("RegExp"))
        .ok()?
        .dyn_into::<js_sys::Function>()
        .ok()?;
    let args = js_sys::Array::of2(&JsValue::from_str(pattern), &JsValue::from_str(flags));
    js_sys::Reflect::construct(&ctor, &args)
        .ok()?
        .dyn_into::<js_sys::RegExp>()
        .ok()
}

/// A JS exception's message, for error toasts.
pub fn err_message(e: &JsValue) -> String {
    if let Some(err) = e.dyn_ref::<js_sys::Error>() {
        return err.message().into();
    }
    e.as_string().unwrap_or_else(|| "error".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_to_string() {
        assert_eq!(num(3.0), "3");
        assert_eq!(num(-3.5), "-3.5");
        assert_eq!(num(42.666666666666664), "42.666666666666664");
        assert_eq!(num(1e21), "1e+21");
        assert_eq!(num(1.5e-7), "1.5e-7");
        assert_eq!(num(0.000001), "0.000001");
        assert_eq!(num(123456789012345680000.0), "123456789012345680000");
        assert_eq!(num(0.1 + 0.2), "0.30000000000000004");
    }

    #[test]
    fn fixed() {
        assert_eq!(to_fixed(2.5, 0), "3");
        assert_eq!(to_fixed(0.25, 1), "0.3");
        assert_eq!(to_fixed(1.005, 2), "1.00"); // 1.00499999...
        assert_eq!(to_fixed(-2.5, 0), "-3");
        assert_eq!(to_fixed(-0.04, 1), "-0.0");
        assert_eq!(to_fixed(123.456, 1), "123.5");
        assert_eq!(to_fixed(9.99, 1), "10.0");
        assert_eq!(to_fixed(0.0, 2), "0.00");
    }

    #[test]
    fn floats() {
        assert_eq!(parse_float("12%"), 12.0);
        assert_eq!(parse_float("  -.5e-3x"), -0.0005);
        assert!(parse_float("abc").is_nan());
        assert!(parse_float(".").is_nan());
        assert_eq!(parse_float("1e"), 1.0);
        assert_eq!(parse_float("3."), 3.0);
    }

    #[test]
    fn grouping() {
        assert_eq!(fmt_n(1234567.0), "1,234,567");
        assert_eq!(fmt_n(-1234.0), "-1,234");
        assert_eq!(fmt_n(12.0), "12");
    }

    #[test]
    fn splitting() {
        assert_eq!(split_ws(" a  b "), vec!["", "a", "b", ""]);
        assert_eq!(split_ws(""), vec![""]);
        assert_eq!(slice16("héllo", 1, Some(3)), "él");
    }
}
