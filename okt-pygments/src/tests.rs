//! Unit tests. The byte-for-byte comparison against Pygments itself lives in
//! `tools/pygments/check.py` (it needs Python with Pygments 2.21.0).

use super::*;

/// Every exported regex compiles, in both `\b` variants.
#[test]
fn all_regexes_compile() {
    let mut failures = Vec::new();
    for (i, rx) in DATA.regexes.iter().enumerate() {
        for exact in [false, true] {
            if let Err(e) = rx.compiled(exact) {
                failures.push(format!("#{i} exact={exact}: {e:?}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} regexes failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// `py_isspace` is exactly Python's `\s` as exported.
#[test]
fn py_isspace_matches_export() {
    let (_, body) = DATA
        .markers
        .sets
        .iter()
        .find(|(m, _)| *m == '\u{e003}')
        .expect("S marker");
    let re = fancy_regex::Regex::new(&format!("^[{body}]$")).unwrap();
    for cp in 0..=0x10FFFFu32 {
        let Some(c) = char::from_u32(cp) else {
            continue;
        };
        let mut buf = [0u8; 4];
        let s = c.encode_utf8(&mut buf);
        assert_eq!(re.is_match(s).unwrap(), py_isspace(c), "U+{cp:04X}");
    }
}

/// The engine's `\b` agrees with its `\w` class (the ambiguity check relies on it).
#[test]
fn rust_word_boundary_matches_word_class() {
    let w = fancy_regex::Regex::new(r"^\w$").unwrap();
    let b = fancy_regex::Regex::new(r"^\b").unwrap();
    for cp in 0..=0x10FFFFu32 {
        let Some(c) = char::from_u32(cp) else {
            continue;
        };
        let mut buf = [0u8; 4];
        let s = c.encode_utf8(&mut buf);
        assert_eq!(w.is_match(s).unwrap(), b.is_match(s).unwrap(), "U+{cp:04X}");
    }
}

/// Texts that contain a character Python and Rust classify differently need exact `\b`.
#[test]
fn ambiguous_word_characters() {
    use crate::regex::needs_exact_word_boundaries as exact;
    assert!(!exact("plain ascii text_42"));
    assert!(
        !exact("café naïve 名前 Ж"),
        "letters are word chars for both"
    );
    assert!(
        exact("x\u{b2}"),
        "superscript two: Python word char (No), Rust not"
    );
    assert!(
        exact("e\u{301}"),
        "combining acute: Rust word char (Mn), Python not"
    );
    assert!(
        exact("a\u{203f}b"),
        "undertie: Rust word char (Pc), Python not"
    );
}

fn lines(text: &str, path: &str) -> Vec<String> {
    match highlight_lines(text, Some(path), None) {
        Highlight::Lines(l) => l,
        other => panic!("{other:?}"),
    }
}

#[test]
fn fallback_escapes_quotes() {
    assert_eq!(
        highlight_lines("a<'\"&>\nb", Some("x.unknownext"), None),
        Highlight::Lines(vec!["a&lt;&#x27;&quot;&amp;&gt;".into(), "b".into()])
    );
}

#[test]
fn python_basics() {
    // Expected output taken from Pygments 2.21.0 (`highlight_lines` in check.py).
    assert_eq!(
        lines("def f(x):\n    return x  # c\n", "a.py"),
        [
            r#"<span class="k">def</span><span class="w"> </span><span class="nf">f</span><span class="p">(</span><span class="n">x</span><span class="p">):</span>"#,
            r#"    <span class="k">return</span> <span class="n">x</span>  <span class="c1"># c</span>"#,
            "",
        ]
    );
}

#[test]
fn crlf_bom_and_line_count() {
    // CR alone creates lines Pygments sees but `text.count('\n')` does not: truncated.
    assert_eq!(lines("\u{feff}a\r\nb\rc", "x.txt"), ["a", "b"]);
}

#[test]
fn empty_text() {
    assert_eq!(lines("", "a.py"), vec![String::new()]);
    assert_eq!(lines("", "a.json"), vec![String::new()]);
}

#[test]
fn unsupported_lexer() {
    assert_eq!(
        highlight_lines("x", Some("a.cs"), None),
        Highlight::Unsupported {
            lexer: "CSharpLexer".into()
        }
    );
}
