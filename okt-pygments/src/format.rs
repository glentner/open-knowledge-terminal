//! `HtmlFormatter(nowrap=True)` from pygments/formatters/html.py (2.21.0).
//!
//! With `nowrap` the formatter output is just `_format_lines`: per token, the value is
//! escaped (`html.escape(value, quote=False)`: only `&`, `<`, `>`), split at newlines, and
//! wrapped in `<span class="...">` — adjacent tokens with the same CSS class share one span,
//! spans are closed at the end of every line and reopened on the next, empty parts never
//! produce empty spans. Token types whose class is `''` (`Token.Text`, ...) get no span.

use crate::data::Data;
use crate::engine::Tok;

/// The formatter's lines, without the trailing line separator (`\n`) each one ends with.
pub(crate) fn format_lines(data: &Data, toks: &[Tok]) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    // css class of the span that is open on `line` ('' = none)
    let mut lspan: &str = "";
    let mut line = String::new();
    let mut escaped = String::new();
    for t in toks {
        let cspan: &str = &data.tokens[t.ty as usize].css;
        escaped.clear();
        escape_into(&t.val, &mut escaped);
        let mut parts = escaped.split('\n');
        let mut last = parts.next().unwrap_or("");
        // for all but the last line
        for next in parts {
            let part = last;
            last = next;
            if !line.is_empty() {
                if lspan != cspan && !part.is_empty() {
                    close(&mut line, lspan);
                    open(&mut line, cspan);
                    line.push_str(part);
                    close(&mut line, cspan);
                } else {
                    line.push_str(part);
                    close(&mut line, lspan);
                }
                lines.push(std::mem::take(&mut line));
            } else if !part.is_empty() {
                let mut l = String::with_capacity(part.len() + 32);
                open(&mut l, cspan);
                l.push_str(part);
                close(&mut l, cspan);
                lines.push(l);
            } else {
                lines.push(String::new());
            }
        }
        // for the last line
        if !line.is_empty() && !last.is_empty() {
            if lspan != cspan {
                close(&mut line, lspan);
                open(&mut line, cspan);
                line.push_str(last);
                lspan = cspan;
            } else {
                line.push_str(last);
            }
        } else if !last.is_empty() {
            open(&mut line, cspan);
            line.push_str(last);
            lspan = cspan;
        }
    }
    if !line.is_empty() {
        close(&mut line, lspan);
        lines.push(line);
    }
    lines
}

fn open(out: &mut String, css: &str) {
    if !css.is_empty() {
        out.push_str("<span class=\"");
        out.push_str(css);
        out.push_str("\">");
    }
}

fn close(out: &mut String, css: &str) {
    if !css.is_empty() {
        out.push_str("</span>");
    }
}

/// `html.escape(s, quote=False)`.
fn escape_into(s: &str, out: &mut String) {
    let mut last = 0;
    for (i, b) in s.bytes().enumerate() {
        let rep = match b {
            b'&' => "&amp;",
            b'<' => "&lt;",
            b'>' => "&gt;",
            _ => continue,
        };
        out.push_str(&s[last..i]);
        out.push_str(rep);
        last = i + 1;
    }
    out.push_str(&s[last..]);
}

/// `html.escape(s)` (quote=True), used for the no-lexer fallback.
pub(crate) fn escape_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last = 0;
    for (i, b) in s.bytes().enumerate() {
        let rep = match b {
            b'&' => "&amp;",
            b'<' => "&lt;",
            b'>' => "&gt;",
            b'"' => "&quot;",
            b'\'' => "&#x27;",
            _ => continue,
        };
        out.push_str(&s[last..i]);
        out.push_str(rep);
        last = i + 1;
    }
    out.push_str(&s[last..]);
    out
}
