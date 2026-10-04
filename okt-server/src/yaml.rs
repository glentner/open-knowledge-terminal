//! Frontmatter YAML with PyYAML's `SafeLoader` semantics (YAML 1.1 implicit typing: yes/no/on/off
//! booleans, sexagesimal and octal ints, dates and timestamps), converted to JSON the way the
//! reference implementation's `jsonable` does: dates become ISO strings, keys become `str(key)`.

use std::rc::Rc;

use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::{Map, Number, Value};
use yaml_rust2::parser::{Event, Parser, Tag};
use yaml_rust2::scanner::TScalarStyle;

use crate::util::{py_float_repr, py_repr_str};

#[derive(Debug)]
enum Node {
    Scalar {
        value: String,
        style: TScalarStyle,
        tag: Option<Tag>,
    },
    Seq {
        items: Vec<Rc<Node>>,
        tag: Option<Tag>,
    },
    Map {
        pairs: Vec<(Rc<Node>, Rc<Node>)>,
        tag: Option<Tag>,
    },
}

/// A constructed Python value.
#[derive(Clone, Debug)]
enum Py {
    None,
    Bool(bool),
    Int(i128),
    Float(f64),
    Str(String),
    Date(i32, u32, u32),
    DateTime {
        date: (i32, u32, u32),
        time: (u32, u32, u32, u32),
        tz: Option<i32>,
    },
    Bytes(Vec<u8>),
    List(Vec<Py>),
    Dict(Vec<(Py, Py)>),
}

const YAML: &str = "tag:yaml.org,2002:";

fn tag_name(t: &Tag) -> String {
    format!("{}{}", t.handle, t.suffix)
}

struct Loader<'a> {
    parser: Parser<std::str::Chars<'a>>,
    anchors: std::collections::HashMap<usize, Rc<Node>>,
}

impl<'a> Loader<'a> {
    fn next(&mut self) -> Result<Event, String> {
        self.parser
            .next_token()
            .map(|(e, _)| e)
            .map_err(|e| e.info().to_string())
    }

    fn node(&mut self, ev: Event) -> Result<Rc<Node>, String> {
        match ev {
            Event::Alias(id) => self
                .anchors
                .get(&id)
                .cloned()
                .ok_or_else(|| "found undefined alias".to_string()),
            Event::Scalar(value, style, anchor, tag) => {
                let n = Rc::new(Node::Scalar { value, style, tag });
                if anchor > 0 {
                    self.anchors.insert(anchor, n.clone());
                }
                Ok(n)
            }
            Event::SequenceStart(anchor, tag) => {
                let mut items = Vec::new();
                loop {
                    let ev = self.next()?;
                    if matches!(ev, Event::SequenceEnd) {
                        break;
                    }
                    items.push(self.node(ev)?);
                }
                let n = Rc::new(Node::Seq { items, tag });
                if anchor > 0 {
                    self.anchors.insert(anchor, n.clone());
                }
                Ok(n)
            }
            Event::MappingStart(anchor, tag) => {
                let mut pairs = Vec::new();
                loop {
                    let ev = self.next()?;
                    if matches!(ev, Event::MappingEnd) {
                        break;
                    }
                    let k = self.node(ev)?;
                    let ev = self.next()?;
                    let v = self.node(ev)?;
                    pairs.push((k, v));
                }
                let n = Rc::new(Node::Map { pairs, tag });
                if anchor > 0 {
                    self.anchors.insert(anchor, n.clone());
                }
                Ok(n)
            }
            other => Err(format!("unexpected event {other:?}")),
        }
    }
}

static RE_BOOL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?:yes|Yes|YES|no|No|NO|true|True|TRUE|false|False|FALSE|on|On|ON|off|Off|OFF)$")
        .unwrap()
});
static RE_FLOAT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?:[-+]?(?:[0-9][0-9_]*)\.[0-9_]*(?:[eE][-+][0-9]+)?|\.[0-9][0-9_]*(?:[eE][-+][0-9]+)?|[-+]?[0-9][0-9_]*(?::[0-5]?[0-9])+\.[0-9_]*|[-+]?\.(?:inf|Inf|INF)|\.(?:nan|NaN|NAN))$").unwrap()
});
static RE_INT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?:[-+]?0b[0-1_]+|[-+]?0[0-7_]+|[-+]?(?:0|[1-9][0-9_]*)|[-+]?0x[0-9a-fA-F_]+|[-+]?[1-9][0-9_]*(?::[0-5]?[0-9])+)$").unwrap()
});
static RE_NULL: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(?:~|null|Null|NULL|)$").unwrap());
static RE_TS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?:[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]|[0-9][0-9][0-9][0-9]-[0-9][0-9]?-[0-9][0-9]?(?:[Tt]|[ \t]+)[0-9][0-9]?:[0-9][0-9]:[0-9][0-9](?:\.[0-9]*)?(?:[ \t]*(?:Z|[-+][0-9][0-9]?(?::[0-9][0-9])?))?)$").unwrap()
});
static RE_TS_PARTS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?P<year>[0-9][0-9][0-9][0-9])-(?P<month>[0-9][0-9]?)-(?P<day>[0-9][0-9]?)(?:(?:[Tt]|[ \t]+)(?P<hour>[0-9][0-9]?):(?P<minute>[0-9][0-9]):(?P<second>[0-9][0-9])(?:\.(?P<fraction>[0-9]*))?(?:[ \t]*(?P<tz>Z|(?P<tz_sign>[-+])(?P<tz_hour>[0-9][0-9]?)(?::(?P<tz_minute>[0-9][0-9]))?))?)?$").unwrap()
});

/// The implicit tag PyYAML's resolver gives a plain scalar.
fn resolve_plain(v: &str) -> &'static str {
    let first = v.chars().next();
    let cands: &[&str] = match first {
        None => &["null"],
        Some(c) if "yYnNtTfFoO".contains(c) => {
            if "nN".contains(c) {
                &["bool", "null"]
            } else {
                &["bool"]
            }
        }
        Some(c) if c.is_ascii_digit() => &["float", "int", "timestamp"],
        Some('-') | Some('+') => &["float", "int"],
        Some('.') => &["float"],
        Some('<') => &["merge"],
        Some('~') => &["null"],
        Some('=') => &["value"],
        _ => &[],
    };
    for c in cands {
        let hit = match *c {
            "bool" => RE_BOOL.is_match(v),
            "float" => RE_FLOAT.is_match(v),
            "int" => RE_INT.is_match(v),
            "null" => RE_NULL.is_match(v),
            "timestamp" => RE_TS.is_match(v),
            "merge" => v == "<<",
            "value" => v == "=",
            _ => false,
        };
        if hit {
            return c;
        }
    }
    "str"
}

fn scalar_tag(value: &str, style: TScalarStyle, tag: &Option<Tag>) -> String {
    match tag {
        Some(t) if t.handle == "!" && t.suffix.is_empty() => format!("{YAML}str"),
        Some(t) => tag_name(t),
        None => {
            if style == TScalarStyle::Plain {
                format!("{YAML}{}", resolve_plain(value))
            } else {
                format!("{YAML}str")
            }
        }
    }
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                29
            } else {
                28
            }
        }
    }
}

fn construct_int(v: &str) -> Result<Py, String> {
    let mut v = v.replace('_', "");
    let mut sign: i128 = 1;
    if v.starts_with('-') {
        sign = -1;
    }
    if v.starts_with('-') || v.starts_with('+') {
        v = v[1..].to_string();
    }
    let bad = || format!("invalid literal for int(): '{v}'");
    let n: i128 = if v == "0" {
        0
    } else if let Some(b) = v.strip_prefix("0b") {
        i128::from_str_radix(b, 2).map_err(|_| bad())?
    } else if let Some(h) = v.strip_prefix("0x") {
        i128::from_str_radix(h, 16).map_err(|_| bad())?
    } else if v.starts_with('0') {
        i128::from_str_radix(&v, 8).map_err(|_| bad())?
    } else if v.contains(':') {
        let mut total: i128 = 0;
        let mut base: i128 = 1;
        for part in v.split(':').rev() {
            total += part.parse::<i128>().map_err(|_| bad())? * base;
            base *= 60;
        }
        total
    } else {
        v.parse::<i128>().map_err(|_| bad())?
    };
    Ok(Py::Int(sign * n))
}

fn construct_float(v: &str) -> Result<Py, String> {
    let mut v = v.replace('_', "").to_lowercase();
    let mut sign = 1.0;
    if v.starts_with('-') {
        sign = -1.0;
    }
    if v.starts_with('-') || v.starts_with('+') {
        v = v[1..].to_string();
    }
    if v == ".inf" {
        return Ok(Py::Float(sign * f64::INFINITY));
    }
    if v == ".nan" {
        return Ok(Py::Float(f64::NAN));
    }
    if v.contains(':') {
        let mut total = 0.0;
        let mut base = 1.0;
        for part in v.split(':').rev() {
            total += part
                .parse::<f64>()
                .map_err(|_| format!("could not convert string to float: '{part}'"))?
                * base;
            base *= 60.0;
        }
        return Ok(Py::Float(sign * total));
    }
    let f: f64 = v
        .parse()
        .map_err(|_| format!("could not convert string to float: '{v}'"))?;
    Ok(Py::Float(sign * f))
}

fn construct_timestamp(v: &str) -> Result<Py, String> {
    let m = RE_TS_PARTS
        .captures(v)
        .ok_or_else(|| format!("invalid timestamp '{v}'"))?;
    let year: i32 = m["year"].parse().unwrap();
    let month: u32 = m["month"].parse().unwrap();
    let day: u32 = m["day"].parse().unwrap();
    if year < 1 {
        return Err(format!("year {year} is out of range"));
    }
    if !(1..=12).contains(&month) {
        return Err("month must be in 1..12".into());
    }
    if day < 1 || day > days_in_month(year, month) {
        return Err("day is out of range for month".into());
    }
    let Some(hour) = m.name("hour") else {
        return Ok(Py::Date(year, month, day));
    };
    let hour: u32 = hour.as_str().parse().unwrap();
    let minute: u32 = m["minute"].parse().unwrap();
    let second: u32 = m["second"].parse().unwrap();
    if hour > 23 {
        return Err("hour must be in 0..23".into());
    }
    if minute > 59 {
        return Err("minute must be in 0..59".into());
    }
    if second > 59 {
        return Err("second must be in 0..59".into());
    }
    let mut fraction = 0u32;
    if let Some(f) = m.name("fraction") {
        if !f.as_str().is_empty() {
            let mut s: String = f.as_str().chars().take(6).collect();
            while s.len() < 6 {
                s.push('0');
            }
            fraction = s.parse().unwrap_or(0);
        }
    }
    let tz = if let Some(sign) = m.name("tz_sign") {
        let th: i32 = m["tz_hour"].parse().unwrap();
        let tm: i32 = m
            .name("tz_minute")
            .map(|x| x.as_str().parse().unwrap())
            .unwrap_or(0);
        let mins = th * 60 + tm;
        Some(if sign.as_str() == "-" { -mins } else { mins })
    } else if m.name("tz").is_some() {
        Some(0)
    } else {
        None
    };
    Ok(Py::DateTime {
        date: (year, month, day),
        time: (hour, minute, second, fraction),
        tz,
    })
}

fn b64_decode(s: &str) -> Result<Vec<u8>, String> {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0;
    for c in s.bytes() {
        if c == b'=' {
            break;
        }
        let Some(v) = T.iter().position(|x| *x == c) else {
            if c.is_ascii_whitespace() {
                continue;
            }
            return Err("failed to decode base64 data: Incorrect padding".into());
        };
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

fn flatten(pairs: &[(Rc<Node>, Rc<Node>)]) -> Result<Vec<(Rc<Node>, Rc<Node>)>, String> {
    let mut merge: Vec<(Rc<Node>, Rc<Node>)> = Vec::new();
    let mut rest = Vec::new();
    for (k, v) in pairs {
        let is_merge = matches!(&**k, Node::Scalar { value, style, tag } if scalar_tag(value, *style, tag) == format!("{YAML}merge"));
        if is_merge {
            match &**v {
                Node::Map { pairs, .. } => merge.extend(flatten(pairs)?),
                Node::Seq { items, .. } => {
                    let mut sub = Vec::new();
                    for item in items {
                        match &**item {
                            Node::Map { pairs, .. } => sub.push(flatten(pairs)?),
                            _ => return Err("while constructing a mapping".into()),
                        }
                    }
                    sub.reverse();
                    for s in sub {
                        merge.extend(s);
                    }
                }
                _ => return Err("while constructing a mapping".into()),
            }
        } else {
            rest.push((k.clone(), v.clone()));
        }
    }
    merge.extend(rest);
    Ok(merge)
}

fn construct(n: &Node) -> Result<Py, String> {
    match n {
        Node::Scalar { value, style, tag } => {
            let t = scalar_tag(value, *style, tag);
            let Some(short) = t.strip_prefix(YAML) else {
                return Err(format!(
                    "could not determine a constructor for the tag {}",
                    py_repr_str(&t)
                ));
            };
            match short {
                "null" => Ok(Py::None),
                "bool" => Ok(Py::Bool(matches!(
                    value.to_lowercase().as_str(),
                    "yes" | "true" | "on"
                ))),
                "int" => construct_int(value),
                "float" => construct_float(value),
                "timestamp" => construct_timestamp(value),
                "str" | "value" | "merge" => Ok(Py::Str(value.clone())),
                "binary" => Ok(Py::Bytes(b64_decode(value)?)),
                _ => Err(format!(
                    "could not determine a constructor for the tag {}",
                    py_repr_str(&t)
                )),
            }
        }
        Node::Seq { items, tag } => {
            let t = tag.as_ref().map(tag_name);
            match t.as_deref().and_then(|t| t.strip_prefix(YAML)) {
                None | Some("seq") | Some("set") => Ok(Py::List(
                    items
                        .iter()
                        .map(|i| construct(i))
                        .collect::<Result<_, _>>()?,
                )),
                Some("omap") | Some("pairs") => {
                    let mut out = Vec::new();
                    for item in items {
                        match &**item {
                            Node::Map { pairs, .. } if pairs.len() == 1 => {
                                out.push(Py::List(vec![
                                    construct(&pairs[0].0)?,
                                    construct(&pairs[0].1)?,
                                ]));
                            }
                            _ => return Err("while constructing an ordered map".into()),
                        }
                    }
                    Ok(Py::List(out))
                }
                _ => Err(format!(
                    "could not determine a constructor for the tag {}",
                    py_repr_str(t.as_deref().unwrap_or(""))
                )),
            }
        }
        Node::Map { pairs, tag } => {
            let t = tag.as_ref().map(tag_name);
            match t.as_deref().and_then(|t| t.strip_prefix(YAML)) {
                None | Some("map") => {}
                Some("set") => {
                    let mut out = Vec::new();
                    for (k, _) in pairs {
                        out.push(construct(k)?);
                    }
                    return Ok(Py::List(out));
                }
                _ => {
                    return Err(format!(
                        "could not determine a constructor for the tag {}",
                        py_repr_str(t.as_deref().unwrap_or(""))
                    ));
                }
            }
            let mut out: Vec<(Py, Py)> = Vec::new();
            for (k, v) in flatten(pairs)? {
                let key = construct(&k)?;
                if matches!(key, Py::List(_) | Py::Dict(_)) {
                    return Err("while constructing a mapping".into());
                }
                let val = construct(&v)?;
                let ks = key_str(&key);
                if let Some(slot) = out
                    .iter_mut()
                    .find(|(k2, _)| key_str(k2) == ks && same_kind(k2, &key))
                {
                    slot.1 = val;
                } else {
                    out.push((key, val));
                }
            }
            Ok(Py::Dict(out))
        }
    }
}

fn same_kind(a: &Py, b: &Py) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

fn date_iso(d: (i32, u32, u32)) -> String {
    format!("{:04}-{:02}-{:02}", d.0, d.1, d.2)
}

fn time_iso(t: (u32, u32, u32, u32), tz: Option<i32>) -> String {
    let mut s = format!("{:02}:{:02}:{:02}", t.0, t.1, t.2);
    if t.3 != 0 {
        s.push_str(&format!(".{:06}", t.3));
    }
    if let Some(m) = tz {
        let sign = if m < 0 { '-' } else { '+' };
        s.push_str(&format!("{}{:02}:{:02}", sign, m.abs() / 60, m.abs() % 60));
    }
    s
}

fn bytes_repr(b: &[u8]) -> String {
    let has_sq = b.contains(&b'\'');
    let has_dq = b.contains(&b'"');
    let q = if has_sq && !has_dq { '"' } else { '\'' };
    let mut s = format!("b{q}");
    for &c in b {
        match c {
            b'\\' => s.push_str("\\\\"),
            b'\n' => s.push_str("\\n"),
            b'\r' => s.push_str("\\r"),
            b'\t' => s.push_str("\\t"),
            c if c as char == q => {
                s.push('\\');
                s.push(c as char);
            }
            0x20..=0x7e => s.push(c as char),
            c => s.push_str(&format!("\\x{c:02x}")),
        }
    }
    s.push(q);
    s
}

/// `str(key)` in Python.
fn key_str(k: &Py) -> String {
    match k {
        Py::None => "None".into(),
        Py::Bool(b) => if *b { "True" } else { "False" }.into(),
        Py::Int(i) => i.to_string(),
        Py::Float(f) => py_float_repr(*f),
        Py::Str(s) => s.clone(),
        Py::Date(y, m, d) => date_iso((*y, *m, *d)),
        Py::DateTime { date, time, tz } => format!("{} {}", date_iso(*date), time_iso(*time, *tz)),
        Py::Bytes(b) => bytes_repr(b),
        Py::List(_) | Py::Dict(_) => String::new(),
    }
}

/// The reference implementation's `jsonable`.
fn jsonable(v: &Py) -> Value {
    match v {
        Py::None => Value::Null,
        Py::Bool(b) => Value::Bool(*b),
        Py::Int(i) => {
            if let Ok(x) = i64::try_from(*i) {
                Value::Number(Number::from(x))
            } else if let Ok(x) = u64::try_from(*i) {
                Value::Number(Number::from(x))
            } else {
                Value::String(i.to_string())
            }
        }
        Py::Float(f) => {
            if f.is_finite() {
                Number::from_f64(*f)
                    .map(Value::Number)
                    .unwrap_or(Value::Null)
            } else {
                Value::String(py_float_repr(*f))
            }
        }
        Py::Str(s) => Value::String(s.clone()),
        Py::Date(y, m, d) => Value::String(date_iso((*y, *m, *d))),
        Py::DateTime { date, time, tz } => {
            Value::String(format!("{}T{}", date_iso(*date), time_iso(*time, *tz)))
        }
        Py::Bytes(b) => Value::String(bytes_repr(b)),
        Py::List(items) => Value::Array(items.iter().map(jsonable).collect()),
        Py::Dict(pairs) => {
            let mut m = Map::new();
            for (k, v) in pairs {
                m.insert(key_str(k), jsonable(v));
            }
            Value::Object(m)
        }
    }
}

/// Parse frontmatter text: (mapping as JSON or None, error message or None).
pub fn parse_frontmatter(text: &str) -> (Option<Map<String, Value>>, Option<String>) {
    match load(text) {
        Err(e) => {
            let first = e.split('\n').next().unwrap_or("").to_string();
            (None, Some(crate::util::take_chars(&first, 240).to_string()))
        }
        Ok(None) => (Some(Map::new()), None),
        Ok(Some(Py::Dict(pairs))) => match jsonable(&Py::Dict(pairs)) {
            Value::Object(m) => (Some(m), None),
            _ => unreachable!(),
        },
        Ok(Some(_)) => (None, Some("frontmatter is not a mapping".into())),
    }
}

fn load(text: &str) -> Result<Option<Py>, String> {
    let mut l = Loader {
        parser: Parser::new_from_str(text),
        anchors: Default::default(),
    };
    let mut doc: Option<Rc<Node>> = None;
    let mut docs = 0;
    loop {
        let ev = l.next()?;
        match ev {
            Event::StreamStart | Event::Nothing | Event::DocumentEnd => continue,
            Event::StreamEnd => break,
            Event::DocumentStart => {
                docs += 1;
                if docs > 1 {
                    return Err("expected a single document in the stream".into());
                }
                let ev = l.next()?;
                if matches!(ev, Event::DocumentEnd) {
                    continue;
                }
                doc = Some(l.node(ev)?);
            }
            other => {
                // documents without an explicit start
                docs += 1;
                if docs > 1 {
                    return Err("expected a single document in the stream".into());
                }
                doc = Some(l.node(other)?);
            }
        }
    }
    match doc {
        None => Ok(None),
        Some(n) => {
            // a lone empty plain scalar is a null document
            if let Node::Scalar {
                value,
                style: TScalarStyle::Plain,
                tag: None,
            } = &*n
            {
                if value.is_empty() {
                    return Ok(None);
                }
            }
            Ok(Some(construct(&n)?))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fm(s: &str) -> String {
        let (m, e) = parse_frontmatter(s);
        match (m, e) {
            (Some(m), None) => serde_json::to_string(&Value::Object(m)).unwrap(),
            (_, Some(e)) => format!("ERR {e}"),
            _ => "?".into(),
        }
    }

    #[test]
    fn yaml11() {
        assert_eq!(
            fm("a: yes\nb: off\nc: 'yes'\nd: y"),
            r#"{"a":true,"b":false,"c":"yes","d":"y"}"#
        );
        assert_eq!(
            fm("t: 1:30\no: 0755\nh: 0x1f\nf: 1.5\ne: 1e3\ng: 1.0e+3"),
            r#"{"t":90,"o":493,"h":31,"f":1.5,"e":"1e3","g":1000.0}"#
        );
        assert_eq!(
            fm("d: 2026-09-30\nn: ~\nm:\nz: 2026-09-30 10:20:30Z"),
            r#"{"d":"2026-09-30","n":null,"m":null,"z":"2026-09-30T10:20:30+00:00"}"#
        );
        assert_eq!(fm("2026: x\ntrue: y"), r#"{"2026":"x","True":"y"}"#);
        assert_eq!(
            fm("a: &x {k: 1}\nb:\n  <<: *x\n  j: 2"),
            r#"{"a":{"k":1},"b":{"k":1,"j":2}}"#
        );
        assert_eq!(fm("- a\n- b"), "ERR frontmatter is not a mapping");
        assert_eq!(fm(""), "{}");
        assert_eq!(fm("# just a comment"), "{}");
        assert_eq!(fm("d: 2026-02-30"), "ERR day is out of range for month");
        assert_eq!(
            fm("asana: 646041972178665 (glentner@x.edu; seat)"),
            r#"{"asana":"646041972178665 (glentner@x.edu; seat)"}"#
        );
    }
}
