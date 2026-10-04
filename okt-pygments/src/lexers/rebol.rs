//! `RebolLexer.word_callback` (pygments/lexers/rebol.py): classify a word by trying a chain
//! of `re.match(pattern, word)` tests. The chain is transcribed from the Pygments source by
//! the exporter (`data.chain`: `[kind, regex, token]`, the last entry being the `else`).

use crate::LexError;
use crate::engine::{Runner, Tok};
use crate::regex::Groups;

pub(crate) fn word_callback<'t>(
    run: &Runner,
    text: &'t str,
    m: &Groups,
    out: &mut Vec<Tok<'t>>,
    base: usize,
) -> Result<(), LexError> {
    let word = &text[m.start()..m.end()];
    let lx = &run.data.lexers[run
        .data
        .lexer_id("RebolLexer")
        .expect("RebolLexer exported") as usize];
    let exact = run.exact || word.is_empty();
    for step in lx.data["chain"].as_array().expect("chain") {
        let tok = step[2].as_u64().unwrap() as u16;
        let hit = match step[0].as_str().unwrap() {
            "else" => true,
            "match" => {
                let rx = &run.data.regexes[step[1].as_u64().unwrap() as usize];
                rx.match_end(word, 0, exact)?.is_some()
            }
            _ => return Err(LexError::Unsupported),
        };
        if hit {
            out.push(Tok::new(base + m.start(), tok, word));
            return Ok(());
        }
    }
    Ok(())
}
