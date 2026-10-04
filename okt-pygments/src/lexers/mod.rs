//! Hand ports of the Pygments code that is not table driven.
//!
//! * [`json`] — `JsonLexer.get_tokens_unprocessed` (a hand-written scanner);
//! * [`makefile`] — `MakefileLexer.get_tokens_unprocessed` and `do_insertions`;
//! * [`postprocess`] — the token filters in the `get_tokens_unprocessed` overrides of
//!   `CFamilyLexer` (C, C++), `SwiftLexer`, `PhpLexer` and `LuaLexer`;
//! * [`yaml`] — the `YamlLexer` callbacks (`something`, `save_indent`, ...), which drive the
//!   indentation state of `YamlLexerContext`;
//! * [`ruby`] — `RubyLexer.heredoc_callback` and the interpolated string/regex callbacks;
//! * [`rebol`] — `RebolLexer.word_callback`.

pub(crate) mod json;
pub(crate) mod makefile;
pub(crate) mod postprocess;
pub(crate) mod rebol;
pub(crate) mod ruby;
pub(crate) mod yaml;

use crate::LexError;
use crate::data::Callback;
use crate::engine::{Ctx, Inst, Runner, Tok};
use crate::regex::Groups;

/// Run a hand-ported callback (`callback(lexer, match[, ctx])`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn callback<'t>(
    run: &Runner,
    inst: Inst,
    cb: &Callback,
    text: &'t str,
    m: &Groups,
    ctx: Option<&mut Ctx>,
    out: &mut Vec<Tok<'t>>,
    base: usize,
) -> Result<(), LexError> {
    fn need_ctx(ctx: Option<&mut Ctx>) -> Result<&mut Ctx, LexError> {
        ctx.ok_or(LexError::Python("callback needs a lexer context"))
    }
    match cb {
        Callback::RebolWord => rebol::word_callback(run, text, m, out, base),
        Callback::RubyHeredoc => {
            ruby::heredoc_callback(run, inst, text, m, need_ctx(ctx)?, out, base)
        }
        Callback::RubyIntpRegex => {
            ruby::intp_callback(run, inst, text, m, need_ctx(ctx)?, out, base, true)
        }
        Callback::RubyIntpString => {
            ruby::intp_callback(run, inst, text, m, need_ctx(ctx)?, out, base, false)
        }
        yaml_cb => yaml::callback(run, yaml_cb, text, m, need_ctx(ctx)?, out, base),
    }
}
