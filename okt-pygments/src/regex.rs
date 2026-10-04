//! Python `re` matching semantics on top of fancy-regex.
//!
//! Pygments calls `compiled.match(text, pos)` (and `match(text, pos, endpos)` for
//! `ExtendedRegexLexer`): an anchored match at `pos` that can still look behind `pos`. That is
//! `RegexInput::new(text).from_pos(pos).anchored(true)` in fancy-regex; `endpos` is handled by
//! the caller slicing the haystack, which is exactly Python's "as if the string were `endpos`
//! characters long".
//!
//! Patterns arrive translated by `tools/pygments/export.py`. They still contain private-use
//! marker characters:
//!
//! * set markers (Python's exact `\w`, `\d`, `\s`, `[^\W\d]`), always inside `[...]`, expanded
//!   here to explicit ranges;
//! * `\b` / `\B` markers. Python decides word boundaries with its own `\w`; the Rust engines
//!   with theirs, and the two disagree on a few thousand code points (combining marks, `²`,
//!   ...). A pattern is therefore compiled in two variants: *fast* (native `\b`/`\B`) and
//!   *exact* (lookarounds over Python's `\w`). The exact variant is used whenever the text
//!   contains a code point on which the two definitions differ, or is empty (Python's `\B`
//!   never matches an empty string; Rust's does). Otherwise both give identical results.

use fancy_regex::{Regex, RegexBuilder, RegexInput};
use once_cell::sync::OnceCell;

use crate::LexError;
use crate::data::{DATA, Markers};

/// One exported regex, compiled lazily.
pub(crate) struct Rx {
    src: Box<str>,
    /// First-character filter: `None` if the pattern can match the empty string, else a
    /// bitmap of ASCII characters that can start a match and whether non-ASCII ones can.
    first: Option<(u128, bool)>,
    has_wb: bool,
    fast: OnceCell<Result<Regex, String>>,
    exact: OnceCell<Result<Regex, String>>,
}

/// Group spans of a match (index 0 is the whole match), as byte offsets into the haystack.
pub(crate) struct Groups {
    spans: Vec<Option<(usize, usize)>>,
}

impl Groups {
    /// Python's `_PseudoMatch(start, text)`, which `bygroups` hands to nested callbacks.
    pub fn pseudo(start: usize, end: usize) -> Groups {
        Groups {
            spans: vec![Some((start, end))],
        }
    }

    /// `match.span(i)` if the group participated; `Err` mirrors Python's IndexError for a
    /// group that does not exist.
    pub fn get(&self, i: usize) -> Result<Option<(usize, usize)>, LexError> {
        self.spans
            .get(i)
            .copied()
            .ok_or(LexError::Python("no such group"))
    }

    pub fn start(&self) -> usize {
        self.spans[0].unwrap().0
    }

    pub fn end(&self) -> usize {
        self.spans[0].unwrap().1
    }
}

const BACKTRACK_LIMIT: usize = 50_000_000;

impl Rx {
    pub fn new(src: &str, first: Option<(u128, bool)>, markers: &Markers) -> Rx {
        let has_wb = src.contains(markers.wb) || src.contains(markers.nwb);
        Rx {
            src: src.into(),
            first,
            has_wb,
            fast: OnceCell::new(),
            exact: OnceCell::new(),
        }
    }

    /// Can a match start at `pos`? (A cheap, exact pre-check: never false when the regex
    /// would match.)
    #[inline]
    pub fn may_start(&self, text: &str, pos: usize) -> bool {
        match self.first {
            None => true,
            Some((mask, nonascii)) => match text.as_bytes().get(pos) {
                None => false,
                Some(&b) if b < 128 => (mask >> b) & 1 == 1,
                Some(_) => nonascii,
            },
        }
    }

    pub fn pattern(&self, exact: bool) -> String {
        expand(&self.src, exact && self.has_wb, &DATA.markers)
    }

    pub fn compiled(&self, exact: bool) -> Result<&Regex, LexError> {
        let exact = exact && self.has_wb;
        let cell = if exact { &self.exact } else { &self.fast };
        cell.get_or_init(|| {
            RegexBuilder::new(&self.pattern(exact))
                .backtrack_limit(BACKTRACK_LIMIT)
                .build()
                .map_err(|e| format!("{e}"))
        })
        .as_ref()
        .map_err(|e| LexError::Regex(format!("cannot compile /{}/: {e}", self.src)))
    }

    /// `compiled.match(text, pos)` → end of the match.
    #[inline]
    pub fn match_end(
        &self,
        text: &str,
        pos: usize,
        exact: bool,
    ) -> Result<Option<usize>, LexError> {
        let re = self.compiled(exact)?;
        match re.find_input(RegexInput::new(text).from_pos(pos).anchored(true)) {
            Ok(m) => Ok(m.map(|m| {
                debug_assert_eq!(m.start(), pos);
                m.end()
            })),
            Err(e) => Err(LexError::Regex(format!("{e}"))),
        }
    }

    /// `compiled.match(text, pos)` → all group spans.
    pub fn captures(
        &self,
        text: &str,
        pos: usize,
        exact: bool,
    ) -> Result<Option<Groups>, LexError> {
        let re = self.compiled(exact)?;
        match re.captures_input(RegexInput::new(text).from_pos(pos).anchored(true)) {
            Ok(Some(c)) => {
                let spans = (0..c.len())
                    .map(|i| c.get(i).map(|m| (m.start(), m.end())))
                    .collect();
                Ok(Some(Groups { spans }))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(LexError::Regex(format!("{e}"))),
        }
    }
}

/// Expand the exporter's marker characters.
fn expand(src: &str, exact: bool, markers: &Markers) -> String {
    let mut out = String::with_capacity(src.len());
    for c in src.chars() {
        if c.is_ascii() {
            out.push(c);
        } else if c == markers.wb {
            if exact {
                let w = &markers.word;
                out.push_str(&format!("(?:(?<=[{w}])(?![{w}])|(?<![{w}])(?=[{w}]))"));
            } else {
                out.push_str("\\b");
            }
        } else if c == markers.nwb {
            if exact {
                let w = &markers.word;
                out.push_str(&format!(
                    "(?:(?<=[{w}])(?=[{w}])|(?<![{w}])(?![{w}])(?:(?<=[\\s\\S])|(?=[\\s\\S])))"
                ));
            } else {
                out.push_str("\\B");
            }
        } else if let Some((_, body)) = markers.sets.iter().find(|(m, _)| *m == c) {
            out.push_str(body);
        } else {
            unreachable!("exported patterns are ASCII apart from markers");
        }
    }
    out
}

/// Does `text` need the exact `\b`/`\B` variants? True if it contains a code point that is a
/// word character for exactly one of Python and Rust. Uses a single class built with the
/// regex engine's own `\w` (symmetric difference with Python's set), so it is exact for the
/// engine actually doing the matching.
pub(crate) fn needs_exact_word_boundaries(text: &str) -> bool {
    static AMBIGUOUS: OnceCell<Regex> = OnceCell::new();
    if text.is_ascii() {
        return false;
    }
    let re = AMBIGUOUS.get_or_init(|| {
        let pat = format!("[\\w~~[{}]]", DATA.markers.word);
        Regex::new(&pat).expect("ambiguous-word-char class compiles")
    });
    re.is_match(text).unwrap_or(true)
}
