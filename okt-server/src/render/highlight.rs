//! Source highlighting into per-line HTML, byte-identical to Pygments' output (see the
//! `okt-pygments` crate). For the rare file whose Pygments lexer isn't ported, the lines are shown
//! escaped but uncolored, as for a file no lexer matches.

use okt_pygments::Highlight;

/// Highlight to one HTML string per line (balanced per line).
pub fn highlight_lines(text: &str, path: Option<&str>, lang: Option<&str>) -> Vec<String> {
    match okt_pygments::highlight_lines(text, path, lang) {
        Highlight::Lines(lines) => lines,
        Highlight::Unsupported { .. } => {
            text.split('\n').map(crate::util::py_html_escape).collect()
        }
    }
}

/// Decode the lexer tables up front so the first code view doesn't pay for it.
pub fn preload() {
    okt_pygments::preload();
}
