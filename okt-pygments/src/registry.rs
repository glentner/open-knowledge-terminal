//! Lexer selection, as `highlight_lines` in the reference does it:
//!
//! * `get_lexer_by_name(alias)`: the first entry of `LEXERS` (in mapping order) whose aliases
//!   contain `alias.lower()`;
//! * `get_lexer_for_filename(name)` without code: every `(lexer, glob)` whose fnmatch glob
//!   matches the basename (case-sensitively) is rated `priority + (0.5 if '*' not in glob)`,
//!   ties broken by class name, and the highest wins;
//! * otherwise, for a `#!` first line, a fixed list of interpreter substrings.

use crate::data::{Data, GlobKind, RegEntry};

/// `get_lexer_by_name(alias)`; `None` is `ClassNotFound`.
pub(crate) fn by_name<'d>(data: &'d Data, alias: &str) -> Option<&'d RegEntry> {
    if alias.is_empty() {
        return None;
    }
    let alias = alias.to_lowercase();
    data.registry.iter().find(|e| e.aliases.contains(&alias))
}

/// `get_lexer_for_filename(name)` (no code given, so no `analyse_text`).
pub(crate) fn for_filename<'d>(data: &'d Data, name: &str) -> Option<&'d RegEntry> {
    let mut best: Option<(f64, &'d RegEntry)> = None;
    for e in &data.registry {
        for g in &e.globs {
            let hit = match &g.kind {
                GlobKind::Suffix(s) => name.ends_with(s.as_str()),
                GlobKind::Exact(s) => name == s,
                GlobKind::Regex(rx) => data.regexes[*rx as usize]
                    .match_end(name, 0, true)
                    .ok()
                    .flatten()
                    .is_some(),
            };
            if !hit {
                continue;
            }
            // explicit patterns get a bonus
            let rating = e.priority + if g.pattern.contains('*') { 0.0 } else { 0.5 };
            // `matches.sort(key=(rating, cls.__name__))` then the last one: the maximum, and
            // among equal keys (same class) the later one.
            let better = match best {
                None => true,
                Some((r, b)) => rating > r || (rating == r && e.class.as_str() >= b.class.as_str()),
            };
            if better {
                best = Some((rating, e));
            }
        }
    }
    best.map(|(_, e)| e)
}

/// The reference's fallback for files no glob matches: guess from a `#!` line.
pub(crate) fn for_shebang<'d>(data: &'d Data, text: &str) -> Option<&'d RegEntry> {
    let first = text.split('\n').next().unwrap_or("");
    if !first.starts_with("#!") {
        return None;
    }
    const KEYS: [(&str, &str); 8] = [
        ("python", "python"),
        ("node", "javascript"),
        ("bash", "bash"),
        ("/sh", "bash"),
        ("zsh", "bash"),
        ("ruby", "ruby"),
        ("perl", "perl"),
        ("awk", "awk"),
    ];
    KEYS.iter()
        .find(|(key, _)| first.contains(key))
        .and_then(|(_, lx)| by_name(data, lx))
}
