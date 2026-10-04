//! A port of linkify-it (the Python edition markdown-it-py uses): schema links (http, https, ftp,
//! `//`, mailto) and "fuzzy" links and emails without a schema (example.com, a@b.org), with the
//! same regular expressions and the same candidate merging, so documents linkify identically.

use fancy_regex::Regex;
use once_cell::sync::Lazy;

use super::ucre_data::*;

/// Python's `$` (no MULTILINE): end of string, or just before a final newline.
const END: &str = r"(?:\n?\z)";

fn src_zpcc() -> String {
    format!("{SRC_Z}|{SRC_P}|{SRC_CC}")
}
fn src_zcc() -> String {
    format!("{SRC_Z}|{SRC_CC}")
}
const TEXT_SEPARATORS: &str = "[><\u{ff5c}]";

struct Re {
    src_tlds: String,
    email_fuzzy: Regex,
    link_no_ip_fuzzy: Regex,
    http: Regex,
    not_http: Regex,
    mailto: Regex,
    schema_search: Regex,
    schema_at_start: Regex,
    pretest: Regex,
}

fn build() -> Re {
    let zpcc = src_zpcc();
    let zcc = src_zcc();
    let pseudo_letter = format!("(?:(?!{TEXT_SEPARATORS}|{zpcc}){SRC_ANY})");
    let src_ip4 =
        r"(?:(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)";
    let src_auth = format!(r"(?:(?:(?!{zcc}|[@/\[\]()]).){{1,50}}@)?");
    let src_port = r"(?::(?:6(?:[0-4]\d{3}|5(?:[0-4]\d{2}|5(?:[0-2]\d|3[0-5])))|[1-5]?\d{1,4}))?";
    let src_email_name = r#"[\-;:&=\+\$,\.a-zA-Z0-9_][\-;:&=\+\$,"\.a-zA-Z0-9_]{0,63}"#;
    let src_xn = r"xn--[a-z0-9\-]{1,59}";
    let src_domain_root = format!("(?:{src_xn}|{pseudo_letter}{{1,63}})");
    let src_domain = format!(
        "(?:{src_xn}|(?:{pseudo_letter})|(?:{pseudo_letter}(?:-|{pseudo_letter}){{0,61}}{pseudo_letter}))"
    );
    let src_host = format!(r"(?:(?:(?:{src_domain})\.)*{src_domain})");
    let tpl_host_fuzzy = format!(r"(?:{src_ip4}|(?:(?:(?:{src_domain})\.)+(?:%TLDS%)))");
    let tpl_host_no_ip_fuzzy = format!(r"(?:(?:(?:{src_domain})\.)+(?:%TLDS%))");
    let tpl_host_fuzzy_test =
        format!(r"localhost|www\.|\.\d{{1,3}}\.|(?:\.(?:%TLDS%)(?:{zpcc}|>|{END}))");
    let host_terminator =
        format!(r"(?={END}|{TEXT_SEPARATORS}|{zpcc})(?!-|_|:\d|\.-|\.(?!{END}|{zpcc}))");
    let src_path = {
        let z = &zcc;
        let mut p = String::new();
        p.push_str("(?:[/?#](?:");
        p.push_str(&format!(
            "(?!{z}|{TEXT_SEPARATORS}|[()\\[\\]{{}}.,\"'?!\\-;]).|"
        ));
        p.push_str(&format!("\\[(?:(?!{z}|\\]).)*\\]|"));
        p.push_str(&format!("\\((?:(?!{z}|[)]).)*\\)|"));
        p.push_str(&format!("\\{{(?:(?!{z}|[}}]).)*\\}}|"));
        p.push_str(&format!("\\\"(?:(?!{z}|[\"]).)+\\\"|"));
        p.push_str(&format!("\\'(?:(?!{z}|[']).)+\\'|"));
        p.push_str(&format!("\\'(?={pseudo_letter}|[-])|"));
        p.push_str("\\.{2,}[a-zA-Z0-9%/&]|");
        p.push_str(&format!("\\.(?!{z}|[.]|{END})|"));
        p.push_str("\\-+|");
        p.push_str(&format!(",(?!{z}|{END})|"));
        p.push_str(&format!(";(?!{z}|{END})|"));
        p.push_str(&format!("\\!+(?!{z}|[!]|{END})|"));
        p.push_str(&format!("\\?(?!{z}|[?]|{END})"));
        p.push_str(")+|\\/)?");
        p
    };
    let src_host_strict = format!("{src_host}{host_terminator}");
    let tpl_host_fuzzy_strict = format!("{tpl_host_fuzzy}{host_terminator}");
    let src_host_port_strict = format!("{src_host}{src_port}{host_terminator}");
    let tpl_host_port_no_ip_fuzzy_strict =
        format!("{tpl_host_no_ip_fuzzy}{src_port}{host_terminator}");
    let tpl_email_fuzzy =
        format!(r#"(^|{TEXT_SEPARATORS}|"|\(|{zcc})({src_email_name}@{tpl_host_fuzzy_strict})"#);
    let tpl_link_no_ip_fuzzy = format!(
        "(^|(?![.:/\\-_@])(?:[$+<=>^`|\u{ff5c}]|{zpcc}))((?![$+<=>^`|\u{ff5c}]){tpl_host_port_no_ip_fuzzy_strict}{src_path})"
    );
    // tlds: the default list, plus any two-letter zone, plus punycode zones
    let tlds_2ch = "a[cdefgilmnoqrstuwxz]|b[abdefghijmnorstvwyz]|c[acdfghiklmnoruvwxyz]|d[ejkmoz]|e[cegrstu]|f[ijkmor]|g[abdefghilmnpqrstuwy]|h[kmnrtu]|i[delmnoqrst]|j[emop]|k[eghimnprwyz]|l[abcikrstuvy]|m[acdeghklmnopqrstuvwxyz]|n[acefgilopruz]|om|p[aefghklmnrstwy]|qa|r[eosuw]|s[abcdeghijklmnortuvxyz]|t[cdfghjklmnortvwz]|u[agksyz]|v[aceginu]|w[fs]|y[et]|z[amw]";
    let src_tlds = format!(
        "biz|com|edu|gov|net|org|pro|web|xxx|aero|asia|coop|info|museum|name|shop|рф|{tlds_2ch}|{src_xn}"
    );
    let untpl = |t: &str| t.replace("%TLDS%", &src_tlds);
    let ci = |p: String| Regex::new(&format!("(?i){p}")).unwrap();
    let schema_test = format!("(^|(?!_)(?:[><\u{ff5c}]|{zpcc}))(http:|https:|ftp:|//|mailto:)");
    Re {
        email_fuzzy: ci(untpl(&tpl_email_fuzzy)),
        link_no_ip_fuzzy: ci(untpl(&tpl_link_no_ip_fuzzy)),
        http: ci(format!(
            r"^(?://{src_auth}{src_host_port_strict}{src_path})"
        )),
        not_http: ci(format!(
            r"^(?:{src_auth}(?:localhost|(?:(?:{src_domain})\.)+{src_domain_root}){src_port}{host_terminator}{src_path})"
        )),
        mailto: ci(format!("^(?:{src_email_name}@{src_host_strict})")),
        schema_search: ci(schema_test.clone()),
        schema_at_start: ci(format!("^{schema_test}")),
        pretest: ci(format!(
            "({schema_test})|({})|@",
            untpl(&tpl_host_fuzzy_test)
        )),
        src_tlds,
    }
}

static RE: Lazy<Re> = Lazy::new(build);

#[derive(Clone, Debug)]
#[allow(dead_code)] // linkify-it's full match record
pub struct Match {
    pub schema: String,
    pub index: usize,
    pub last_index: usize,
    pub raw: String,
    pub text: String,
    pub url: String,
}

fn validate(schema: &str, text: &str, pos: usize) -> usize {
    let tail = &text[pos..];
    let re = &*RE;
    match schema.to_lowercase().as_str() {
        "http:" | "https:" | "ftp:" => re
            .http
            .find(tail)
            .ok()
            .flatten()
            .map(|m| m.end())
            .unwrap_or(0),
        "//" => {
            let Some(m) = re.not_http.find(tail).ok().flatten() else {
                return 0;
            };
            let before: Vec<char> = text[..pos].chars().rev().take(3).collect();
            if before.len() >= 3 && (before[2] == ':' || before[2] == '/') {
                return 0;
            }
            m.end()
        }
        "mailto:" => re
            .mailto
            .find(tail)
            .ok()
            .flatten()
            .map(|m| m.end())
            .unwrap_or(0),
        _ => 0,
    }
}

fn create_match(text: &str, schema: &str, index: usize, last_index: usize) -> Match {
    let raw = text[index..last_index].to_string();
    let schema = schema.to_lowercase();
    let mut url = raw.clone();
    if schema.is_empty() {
        url = format!("http://{url}");
    }
    if schema == "mailto:" && !url.to_lowercase().starts_with("mailto:") {
        url = format!("mailto:{url}");
    }
    Match {
        schema,
        index,
        last_index,
        text: raw.clone(),
        raw,
        url,
    }
}

pub fn pretest(text: &str) -> bool {
    RE.pretest.is_match(text).unwrap_or(false)
}

pub fn test(text: &str) -> bool {
    !matches(text).is_empty()
}

/// All links in `text`, left to right, non-overlapping.
pub fn matches(text: &str) -> Vec<Match> {
    if text.is_empty() {
        return vec![];
    }
    let re = &*RE;
    struct Cand {
        schema: String,
        index: usize,
        last: usize,
    }
    let mut schemed = Vec::new();
    for m in re.schema_search.captures_iter(text).flatten() {
        let g1 = m.get(1).unwrap();
        let g2 = m.get(2).unwrap();
        let whole = m.get(0).unwrap();
        let len = validate(g2.as_str(), text, whole.end());
        if len > 0 {
            schemed.push(Cand {
                schema: g2.as_str().to_string(),
                index: whole.start() + g1.as_str().len(),
                last: whole.end() + len,
            });
        }
    }
    let mut fuzzy_link = Vec::new();
    for m in re.link_no_ip_fuzzy.captures_iter(text).flatten() {
        let g1 = m.get(1).unwrap();
        let whole = m.get(0).unwrap();
        fuzzy_link.push(Cand {
            schema: String::new(),
            index: whole.start() + g1.as_str().len(),
            last: whole.end(),
        });
    }
    let mut fuzzy_email = Vec::new();
    for m in re.email_fuzzy.captures_iter(text).flatten() {
        let g1 = m.get(1).unwrap();
        let whole = m.get(0).unwrap();
        fuzzy_email.push(Cand {
            schema: "mailto:".into(),
            index: whole.start() + g1.as_str().len(),
            last: whole.end(),
        });
    }
    // merge the three streams, each already sorted by position, dropping overlaps
    let lists = [&schemed, &fuzzy_email, &fuzzy_link];
    let mut idx = [0usize; 3];
    let mut out = Vec::new();
    let mut last_index = 0;
    loop {
        let mut best: Option<usize> = None;
        for (li, list) in lists.iter().enumerate() {
            let Some(c) = list.get(idx[li]) else { continue };
            best = match best {
                None => Some(li),
                Some(bi) => {
                    let b = &lists[bi][idx[bi]];
                    if c.index < b.index || (c.index == b.index && c.last > b.last) {
                        Some(li)
                    } else {
                        Some(bi)
                    }
                }
            };
        }
        let Some(bi) = best else { break };
        let c = &lists[bi][idx[bi]];
        idx[bi] += 1;
        if c.index < last_index {
            continue;
        }
        out.push(create_match(text, &c.schema, c.index, c.last));
        last_index = c.last;
    }
    out
}

/// A schema link starting exactly at the beginning of `text`.
pub fn match_at_start(text: &str) -> Option<Match> {
    if text.is_empty() {
        return None;
    }
    let m = RE.schema_at_start.captures(text).ok().flatten()?;
    let g1 = m.get(1).unwrap();
    let g2 = m.get(2).unwrap();
    let whole = m.get(0).unwrap();
    let len = validate(g2.as_str(), text, whole.end());
    if len == 0 {
        return None;
    }
    Some(create_match(
        text,
        g2.as_str(),
        whole.start() + g1.as_str().len(),
        whole.end() + len,
    ))
}

#[allow(dead_code)]
pub fn tlds() -> &'static str {
    &RE.src_tlds
}

#[cfg(test)]
mod tests {
    use super::*;

    fn urls(t: &str) -> Vec<(String, String)> {
        matches(t).into_iter().map(|m| (m.text, m.url)).collect()
    }

    #[test]
    fn fuzzy() {
        assert_eq!(
            urls("see docs.rcac.purdue.edu (x)"),
            vec![(
                "docs.rcac.purdue.edu".into(),
                "http://docs.rcac.purdue.edu".into()
            )]
        );
        assert_eq!(
            urls("mail glentner@purdue.edu now"),
            vec![(
                "glentner@purdue.edu".into(),
                "mailto:glentner@purdue.edu".into()
            )]
        );
        assert_eq!(urls("bin/terminal.py"), vec![]);
        assert_eq!(
            urls("go https://example.org/a?b=1."),
            vec![(
                "https://example.org/a?b=1".into(),
                "https://example.org/a?b=1".into()
            )]
        );
        assert!(pretest("x.com"));
    }

    #[test]
    fn at_start() {
        let m = match_at_start("https://a.org/x y").unwrap();
        assert_eq!(m.raw, "https://a.org/x");
    }
}
