//! Small helpers. Several mirror Python's string semantics exactly, so the JSON this server
//! produces stays byte-stable.

use std::path::Path;

use serde_json::Value;

/// Python's `str.isspace` set: Unicode White_Space plus the four C0 separators \x1c-\x1f.
pub fn is_py_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// Python `str.strip()`.
pub fn py_strip(s: &str) -> &str {
    s.trim_matches(is_py_space)
}

/// Python `str.split()` with no arguments.
pub fn py_split_ws(s: &str) -> impl Iterator<Item = &str> {
    s.split(is_py_space).filter(|w| !w.is_empty())
}

/// Python `html.escape(s, quote=True)`.
pub fn py_html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + s.len() / 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(c),
        }
    }
    out
}

/// The first `n` characters (code points) of `s`, like Python's `s[:n]`.
pub fn take_chars(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

/// `s[a:b]` in code points.
pub fn slice_chars(s: &str, a: usize, b: usize) -> &str {
    let byte_at = |n: usize| s.char_indices().nth(n).map(|(i, _)| i).unwrap_or(s.len());
    let start = byte_at(a);
    let end = byte_at(b.max(a));
    &s[start..end]
}

/// Number of code points before byte offset `byte` in `s`.
pub fn char_index(s: &str, byte: usize) -> usize {
    s[..byte].chars().count()
}

/// Normalize a repo-relative posix path; None if it escapes the root.
pub fn norm_rel(p: &str) -> Option<String> {
    let trailing = p.ends_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for part in p.split('/') {
        match part {
            "" | "." => continue,
            ".." => {
                parts.pop()?;
            }
            _ => parts.push(part),
        }
    }
    let out = parts.join("/");
    Some(if trailing && !out.is_empty() {
        out + "/"
    } else {
        out
    })
}

/// 'a/b/c.md' -> 'a/b/'; 'a/b/' -> 'a/'; top level -> '.' (the root).
pub fn parent_dir(path: &str) -> String {
    let p = path.trim_end_matches('/');
    match p.rfind('/') {
        None => ".".to_string(),
        Some(i) => format!("{}/", &p[..i]),
    }
}

pub fn top_folder(path: &str) -> &str {
    match path.find('/') {
        Some(i) => &path[..i],
        None => "",
    }
}

/// The `[prefix]` of a commit subject (`^\[([^\]]+)\]`).
pub fn prefix_of(subject: &str) -> String {
    if let Some(rest) = subject.strip_prefix('[') {
        if let Some(end) = rest.find(']') {
            if end > 0 {
                return rest[..end].to_string();
            }
        }
    }
    String::new()
}

/// Extension including the dot, lowercased: '.md'; dotfiles with a single dot are their own
/// extension ('.gitignore').
pub fn ext_of(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    if name.starts_with('.') && name.matches('.').count() == 1 {
        return name.to_lowercase();
    }
    match name.rfind('.') {
        Some(i) => format!(".{}", name[i + 1..].to_lowercase()),
        None => String::new(),
    }
}

pub fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

pub fn posix_dirname(p: &str) -> &str {
    // posixpath.dirname
    let i = p.rfind('/').map(|i| i + 1).unwrap_or(0);
    let head = &p[..i];
    if !head.is_empty() && head.chars().any(|c| c != '/') {
        head.trim_end_matches('/')
    } else {
        head
    }
}

/// posixpath.join for two parts.
pub fn posix_join(a: &str, b: &str) -> String {
    if b.starts_with('/') || a.is_empty() {
        b.to_string()
    } else if a.ends_with('/') {
        format!("{a}{b}")
    } else {
        format!("{a}/{b}")
    }
}

pub fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(std::path::PathBuf::from)
}

pub fn display_path(p: &Path) -> String {
    if let Some(home) = home_dir() {
        if let Ok(rel) = p.strip_prefix(&home) {
            return format!("~/{}", rel.display());
        }
    }
    p.display().to_string()
}

/// Values that mean 'not filled in yet'.
pub fn is_todo(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => {
            let s = py_strip(s).to_lowercase();
            s.is_empty()
                || ["todo", "tbd", "n/a", "none", "unknown"]
                    .iter()
                    .any(|p| s.starts_with(p))
                || s == "-"
                || s == "—"
                || s == "?"
        }
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
        _ => false,
    }
}

/// Python truthiness of a JSON-ified value.
pub fn py_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// Python `repr(float)`.
pub fn py_float_repr(f: f64) -> String {
    if f.is_nan() {
        return "nan".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "inf".into() } else { "-inf".into() };
    }
    if f == 0.0 {
        return if f.is_sign_negative() {
            "-0.0".into()
        } else {
            "0.0".into()
        };
    }
    // shortest round-trip digits, then Python's formatting rules (repr uses 'r' format: scientific
    // when exponent < -4 or >= 16)
    let s = format!("{:e}", f); // e.g. 1.5e-5, 1e16
    let (mant, exp) = s.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    let neg = mant.starts_with('-');
    let digits: String = mant.chars().filter(|c| c.is_ascii_digit()).collect();
    if exp < -4 || exp >= 16 {
        let mut m = digits[..1].to_string();
        if digits.len() > 1 {
            m.push('.');
            m.push_str(&digits[1..]);
        }
        let sign = if exp < 0 { '-' } else { '+' };
        return format!(
            "{}{}e{}{:02}",
            if neg { "-" } else { "" },
            m,
            sign,
            exp.abs()
        );
    }
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    if exp < 0 {
        out.push_str("0.");
        for _ in 0..(-exp - 1) {
            out.push('0');
        }
        out.push_str(&digits);
    } else {
        let int_len = exp as usize + 1;
        if digits.len() <= int_len {
            out.push_str(&digits);
            for _ in 0..(int_len - digits.len()) {
                out.push('0');
            }
            out.push_str(".0");
        } else {
            out.push_str(&digits[..int_len]);
            out.push('.');
            out.push_str(&digits[int_len..]);
        }
    }
    out
}

/// Python `repr(str)`.
pub fn py_repr_str(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::new();
    out.push(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\x{:02x}", c as u32))
            }
            c if (0x80..0xa0).contains(&(c as u32)) => {
                out.push_str(&format!("\\x{:02x}", c as u32))
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

fn py_repr(v: &Value) -> String {
    match v {
        Value::String(s) => py_repr_str(s),
        other => py_str(other),
    }
}

/// Python `str(x)` of a JSON-ified value.
pub fn py_str(v: &Value) -> String {
    match v {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.to_string()
            } else if let Some(u) = n.as_u64() {
                u.to_string()
            } else {
                py_float_repr(n.as_f64().unwrap_or(0.0))
            }
        }
        Value::String(s) => s.clone(),
        Value::Array(a) => format!("[{}]", a.iter().map(py_repr).collect::<Vec<_>>().join(", ")),
        Value::Object(o) => format!(
            "{{{}}}",
            o.iter()
                .map(|(k, v)| format!("{}: {}", py_repr_str(k), py_repr(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// Python's `json.dumps(obj, separators=(',', ':'), ensure_ascii=False)`.
/// Python's `round(x, 1)` (correctly rounded, ties to even on the exact binary value).
pub fn py_round1(x: f64) -> f64 {
    format!("{x:.1}").parse().unwrap_or(x)
}

pub fn now_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths() {
        assert_eq!(norm_rel("a/./b/../c.md").as_deref(), Some("a/c.md"));
        assert_eq!(norm_rel("../x"), None);
        assert_eq!(norm_rel("a/b/").as_deref(), Some("a/b/"));
        assert_eq!(parent_dir("a/b/c.md"), "a/b/");
        assert_eq!(parent_dir("a/b/"), "a/");
        assert_eq!(parent_dir("x.md"), ".");
        assert_eq!(ext_of("x/.gitignore"), ".gitignore");
        assert_eq!(ext_of("x/a.TAR.GZ"), ".gz");
        assert_eq!(ext_of("Makefile"), "");
        assert_eq!(prefix_of("[memory] inbox"), "memory");
        assert_eq!(prefix_of("no prefix"), "");
    }

    #[test]
    fn floats() {
        assert_eq!(py_float_repr(1.5), "1.5");
        assert_eq!(py_float_repr(1.0), "1.0");
        assert_eq!(py_float_repr(1e16), "1e+16");
        assert_eq!(py_float_repr(1.5e-5), "1.5e-05");
        assert_eq!(py_float_repr(0.0001), "0.0001");
        assert_eq!(py_float_repr(123456.789), "123456.789");
    }

    #[test]
    fn strs() {
        assert_eq!(py_str(&serde_json::json!(["a", 1, true])), "['a', 1, True]");
        assert_eq!(slice_chars("héllo", 1, 3), "él");
        assert_eq!(take_chars("héllo", 2), "hé");
    }
}
