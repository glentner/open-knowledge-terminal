//! The lexing engine.
//!
//! Faithful ports of, from `pygments/lexer.py`:
//!
//! * `RegexLexer.get_tokens_unprocessed` ([`Runner::regex`]), including the fallback when no
//!   rule matches: at `\n` the state stack resets to `root` and a `Whitespace` token is
//!   emitted, otherwise one character becomes an `Error` token;
//! * `ExtendedRegexLexer.get_tokens_unprocessed` and `LexerContext` ([`Runner::extended`],
//!   [`Ctx`]): same loop on a context object, `endpos`-limited matching, `Text` instead of
//!   `Whitespace` for the newline fallback, and callbacks that move `ctx.pos` themselves;
//! * `bygroups`, `using` (including `using(this, state=...)`) and `default` (an empty regex
//!   with no action);
//! * the dispatch that `get_tokens_unprocessed` overrides perform, to the hand ports in
//!   [`crate::lexers`].
//!
//! Positions are byte offsets (Pygments uses code points); every place that depends on a
//! length (`len(text)` in callbacks, `do_insertions`) is handled where it occurs.

use std::borrow::Cow;
use std::cell::Cell;

use crate::LexError;
use crate::data::{Action, Custom, Data, Kind, LexId, NewState, StateId, StateOp, TokId};
use crate::lexers;
use crate::regex::Groups;

/// One token: `(index, tokentype, value)`.
pub(crate) struct Tok<'t> {
    pub pos: usize,
    pub ty: TokId,
    pub val: Cow<'t, str>,
}

impl<'t> Tok<'t> {
    pub fn new(pos: usize, ty: TokId, val: &'t str) -> Tok<'t> {
        Tok {
            pos,
            ty,
            val: Cow::Borrowed(val),
        }
    }
}

/// A lexer instance: the class plus the options that change its behaviour. okt passes only
/// `stripnl`/`ensurenl` (handled in preprocessing), so the only option that matters is
/// PhpLexer's `startinline`, which `using(this, _startinline=True)` sets.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Inst {
    pub id: LexId,
    pub startinline: bool,
}

impl Inst {
    pub fn new(id: LexId) -> Inst {
        Inst {
            id,
            startinline: false,
        }
    }
}

/// `pygments.lexer.LexerContext`, plus the attributes `YamlLexerContext` and RubyLexer's
/// heredoc callback (`ctx.__dict__.setdefault('heredocstack', [])`) add.
pub(crate) struct Ctx {
    pub pos: usize,
    pub end: usize,
    pub stack: Vec<StateId>,
    pub indent_stack: Vec<i64>,
    pub indent: i64,
    pub next_indent: i64,
    pub block_scalar_indent: Option<i64>,
    pub heredocstack: Vec<(bool, String)>,
}

impl Ctx {
    /// `LexerContext(text, pos, stack, end)` / `YamlLexerContext(text, 0)`.
    pub fn new(pos: usize, end: usize, stack: Vec<StateId>) -> Ctx {
        Ctx {
            pos,
            end,
            stack,
            indent_stack: Vec::new(),
            indent: -1,
            next_indent: 0,
            block_scalar_indent: None,
            heredocstack: Vec::new(),
        }
    }
}

/// How many consecutive iterations without progress we tolerate before giving up. Pygments
/// would loop forever (or exhaust memory) in such a case.
const STALL_LIMIT: u32 = 100_000;

/// Maximum nesting of lexer invocations (`using()`, Ruby heredocs and interpolation). Python
/// raises `RecursionError` at about 1000 frames; a few hundred levels here is far beyond what
/// real input produces and keeps us clear of a stack overflow on small thread stacks.
const MAX_DEPTH: u32 = 200;

/// Per-call lexing state.
pub(crate) struct Runner<'d> {
    pub data: &'d Data,
    /// Use the exact (lookaround) `\b`/`\B` variants; see [`crate::regex`].
    pub exact: bool,
    depth: Cell<u32>,
}

/// Counts one level of lexer nesting for as long as it lives.
struct DepthGuard<'a>(&'a Cell<u32>);

impl Drop for DepthGuard<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() - 1);
    }
}

impl<'d> Runner<'d> {
    pub fn new(data: &'d Data, exact: bool) -> Runner<'d> {
        Runner {
            data,
            exact,
            depth: Cell::new(0),
        }
    }

    fn enter(&self) -> Result<DepthGuard<'_>, LexError> {
        if self.depth.get() >= MAX_DEPTH {
            return Err(LexError::Python("maximum recursion depth exceeded"));
        }
        self.depth.set(self.depth.get() + 1);
        Ok(DepthGuard(&self.depth))
    }

    /// `lexer.get_tokens_unprocessed(text[, stack])`, with token positions offset by `base`.
    pub fn tokens<'t>(
        &self,
        inst: Inst,
        text: &'t str,
        stack: Option<&[StateId]>,
        out: &mut Vec<Tok<'t>>,
        base: usize,
    ) -> Result<(), LexError> {
        let _depth = self.enter()?;
        let lx = &self.data.lexers[inst.id as usize];
        let no_stack = || match stack {
            None => Ok(()),
            Some(_) => Err(LexError::Python(
                "get_tokens_unprocessed() got an unexpected keyword argument 'stack'",
            )),
        };
        match lx.custom {
            Some(Custom::Json) => {
                no_stack()?;
                lexers::json::lex(self.data, lx, text, out, base);
                Ok(())
            }
            Some(Custom::Text) => {
                no_stack()?;
                out.push(Tok::new(base, self.data.t_text, text));
                Ok(())
            }
            Some(Custom::Yaml) => {
                no_stack()?;
                let mut ctx = Ctx::new(0, text.len(), vec![lx.root]);
                self.extended(inst, text, &mut ctx, out, base)
            }
            Some(Custom::Makefile) => {
                no_stack()?;
                lexers::makefile::lex(self, lx, text, out, base)
            }
            Some(Custom::CFamily) => {
                let start = out.len();
                self.regex(inst, text, stack.unwrap_or(&[lx.root]), out, base)?;
                lexers::postprocess::cfamily(self.data, lx, &mut out[start..]);
                Ok(())
            }
            Some(Custom::Swift) => {
                no_stack()?;
                let start = out.len();
                self.regex(inst, text, &[lx.root], out, base)?;
                lexers::postprocess::swift(self.data, lx, &mut out[start..]);
                Ok(())
            }
            Some(Custom::Php) => {
                no_stack()?;
                lexers::postprocess::php_guard(text)?;
                let mut st = vec![lx.root];
                if inst.startinline {
                    st.push(lx.state_ids["php"]);
                }
                let start = out.len();
                self.regex(inst, text, &st, out, base)?;
                lexers::postprocess::php(self.data, lx, &mut out[start..]);
                Ok(())
            }
            Some(Custom::Lua) => {
                no_stack()?;
                let start = out.len();
                self.regex(inst, text, &[lx.root], out, base)?;
                lexers::postprocess::lua(self.data, lx, out, start)
            }
            None => match lx.kind {
                Kind::Regex => self.regex(inst, text, stack.unwrap_or(&[lx.root]), out, base),
                Kind::Extended => {
                    no_stack()?;
                    let mut ctx = Ctx::new(0, text.len(), vec![lx.root]);
                    self.extended(inst, text, &mut ctx, out, base)
                }
                Kind::Other => Err(LexError::Unsupported),
            },
        }
    }

    /// `RegexLexer.get_tokens_unprocessed(self, text, stack)`.
    pub fn regex<'t>(
        &self,
        inst: Inst,
        text: &'t str,
        stack: &[StateId],
        out: &mut Vec<Tok<'t>>,
        base: usize,
    ) -> Result<(), LexError> {
        let lx = &self.data.lexers[inst.id as usize];
        let exact = self.exact || text.is_empty();
        let mut pos = 0usize;
        let mut statestack: Vec<StateId> = stack.to_vec();
        let mut cur = *statestack
            .last()
            .ok_or(LexError::Python("empty state stack"))?;
        let mut guard = Guard::new();
        'outer: loop {
            guard.step(pos)?;
            for rule in &lx.states[cur as usize].rules {
                let rx = &self.data.regexes[rule.rx as usize];
                if !rx.may_start(text, pos) {
                    continue;
                }
                match &rule.action {
                    Action::None => match rx.match_end(text, pos, exact)? {
                        None => continue,
                        Some(end) => pos = end,
                    },
                    Action::Token(t) => match rx.match_end(text, pos, exact)? {
                        None => continue,
                        Some(end) => {
                            out.push(Tok::new(base + pos, *t, &text[pos..end]));
                            pos = end;
                        }
                    },
                    action => match rx.captures(text, pos, exact)? {
                        None => continue,
                        Some(m) => {
                            self.action(inst, action, text, &m, None, out, base)?;
                            pos = m.end();
                        }
                    },
                }
                if !matches!(rule.new_state, NewState::None) {
                    apply_new_state(&rule.new_state, &mut statestack)?;
                    cur = *statestack
                        .last()
                        .ok_or(LexError::Python("empty state stack"))?;
                }
                continue 'outer;
            }
            // No rule matched.
            let Some(&b) = text.as_bytes().get(pos) else {
                break;
            };
            if b == b'\n' {
                statestack.clear();
                statestack.push(lx.root);
                cur = lx.root;
                out.push(Tok::new(base + pos, self.data.t_whitespace, "\n"));
                pos += 1;
            } else {
                let n = utf8_len(b);
                out.push(Tok::new(base + pos, self.data.t_error, &text[pos..pos + n]));
                pos += n;
            }
        }
        Ok(())
    }

    /// `ExtendedRegexLexer.get_tokens_unprocessed(self, text, context)`; `text` is `ctx.text`.
    pub fn extended<'t>(
        &self,
        inst: Inst,
        text: &'t str,
        ctx: &mut Ctx,
        out: &mut Vec<Tok<'t>>,
        base: usize,
    ) -> Result<(), LexError> {
        let _depth = self.enter()?;
        let lx = &self.data.lexers[inst.id as usize];
        let mut cur = *ctx
            .stack
            .last()
            .ok_or(LexError::Python("empty state stack"))?;
        let mut guard = Guard::new();
        'outer: loop {
            guard.step(ctx.pos)?;
            // `rexmatch(text, ctx.pos, ctx.end)`: endpos == slicing the string.
            let hay = text.get(..ctx.end).ok_or(LexError::Python("bad endpos"))?;
            if ctx.pos > hay.len() {
                // Python would still let empty patterns match at pos > endpos; no supported
                // lexer gets here (callbacks keep pos <= end), so refuse rather than diverge.
                return Err(LexError::Python("pos beyond endpos"));
            }
            let exact = self.exact || hay.is_empty();
            for rule in &lx.states[cur as usize].rules {
                let rx = &self.data.regexes[rule.rx as usize];
                if !rx.may_start(hay, ctx.pos) {
                    continue;
                }
                match &rule.action {
                    // An action-less rule (`default(...)`) does not move ctx.pos.
                    Action::None => {
                        if rx.match_end(hay, ctx.pos, exact)?.is_none() {
                            continue;
                        }
                    }
                    Action::Token(t) => match rx.match_end(hay, ctx.pos, exact)? {
                        None => continue,
                        Some(end) => {
                            out.push(Tok::new(base + ctx.pos, *t, &text[ctx.pos..end]));
                            ctx.pos = end;
                        }
                    },
                    action => match rx.captures(hay, ctx.pos, exact)? {
                        None => continue,
                        Some(m) => {
                            self.action(inst, action, text, &m, Some(ctx), out, base)?;
                            if new_state_falsy(&rule.new_state) {
                                // the callback may have altered the state stack
                                cur = *ctx
                                    .stack
                                    .last()
                                    .ok_or(LexError::Python("empty state stack"))?;
                            }
                        }
                    },
                }
                if !matches!(rule.new_state, NewState::None) {
                    apply_new_state(&rule.new_state, &mut ctx.stack)?;
                    cur = *ctx
                        .stack
                        .last()
                        .ok_or(LexError::Python("empty state stack"))?;
                }
                continue 'outer;
            }
            if ctx.pos >= ctx.end {
                break;
            }
            let b = text.as_bytes()[ctx.pos];
            if b == b'\n' {
                ctx.stack = vec![lx.root];
                cur = lx.root;
                out.push(Tok::new(base + ctx.pos, self.data.t_text, "\n"));
                ctx.pos += 1;
            } else {
                let n = utf8_len(b);
                out.push(Tok::new(
                    base + ctx.pos,
                    self.data.t_error,
                    &text[ctx.pos..ctx.pos + n],
                ));
                ctx.pos += n;
            }
        }
        Ok(())
    }

    /// Run a callable action (`bygroups`, `using`, or a hand-ported callback) on a match.
    #[allow(clippy::too_many_arguments)]
    pub fn action<'t>(
        &self,
        inst: Inst,
        action: &Action,
        text: &'t str,
        m: &Groups,
        mut ctx: Option<&mut Ctx>,
        out: &mut Vec<Tok<'t>>,
        base: usize,
    ) -> Result<(), LexError> {
        match action {
            Action::None => Ok(()),
            Action::Token(t) => {
                out.push(Tok::new(base + m.start(), *t, &text[m.start()..m.end()]));
                Ok(())
            }
            Action::ByGroups(args) => {
                for (i, a) in args.iter().enumerate() {
                    match a {
                        Action::None => {}
                        Action::Token(t) => {
                            // `if data:` -- empty and non-participating groups yield nothing
                            if let Some((s, e)) = m.get(i + 1)?
                                && e > s
                            {
                                out.push(Tok::new(base + s, *t, &text[s..e]));
                            }
                        }
                        other => {
                            // `if data is not None:` -- callables also see empty groups
                            if let Some((s, e)) = m.get(i + 1)? {
                                if let Some(c) = ctx.as_deref_mut() {
                                    c.pos = s;
                                }
                                let pm = Groups::pseudo(s, e);
                                self.action(inst, other, text, &pm, ctx.as_deref_mut(), out, base)?;
                            }
                        }
                    }
                }
                if let Some(c) = ctx {
                    c.pos = m.end();
                }
                Ok(())
            }
            Action::Using(u) => {
                let (s, e) = (m.start(), m.end());
                let target = match u.target {
                    // `lx = lexer` without keyword arguments, else a new instance.
                    None => Inst {
                        id: inst.id,
                        startinline: u.startinline.unwrap_or(inst.startinline),
                    },
                    Some(t) => Inst {
                        id: t,
                        startinline: u.startinline.unwrap_or(false),
                    },
                };
                self.tokens(target, &text[s..e], u.stack.as_deref(), out, base + s)?;
                if let Some(c) = ctx {
                    c.pos = e;
                }
                Ok(())
            }
            Action::Callback(cb) => lexers::callback(self, inst, cb, text, m, ctx, out, base),
        }
    }
}

/// `if not new_state:` in ExtendedRegexLexer.
fn new_state_falsy(ns: &NewState) -> bool {
    match ns {
        NewState::None => true,
        NewState::Ops(ops) => ops.is_empty(),
        NewState::Pop(n) => *n == 0,
        NewState::Push => false,
    }
}

/// The state transition part of the lexing loop.
fn apply_new_state(ns: &NewState, stack: &mut Vec<StateId>) -> Result<(), LexError> {
    match ns {
        NewState::None => {}
        NewState::Ops(ops) => {
            for op in ops {
                match op {
                    StateOp::Pop => {
                        if stack.len() > 1 {
                            stack.pop();
                        }
                    }
                    StateOp::Push => {
                        let top = *stack.last().ok_or(LexError::Python("empty state stack"))?;
                        stack.push(top);
                    }
                    StateOp::Enter(s) => stack.push(*s),
                }
            }
        }
        NewState::Pop(n) => {
            // pop, but keep at least one state on the stack
            if *n >= stack.len() {
                stack.truncate(1);
            } else {
                stack.truncate(stack.len() - n);
            }
        }
        NewState::Push => {
            let top = *stack.last().ok_or(LexError::Python("empty state stack"))?;
            stack.push(top);
        }
    }
    Ok(())
}

/// Detects a lexing loop that makes no progress.
struct Guard {
    last: usize,
    stall: u32,
}

impl Guard {
    fn new() -> Guard {
        Guard {
            last: usize::MAX,
            stall: 0,
        }
    }

    #[inline]
    fn step(&mut self, pos: usize) -> Result<(), LexError> {
        if pos == self.last {
            self.stall += 1;
            if self.stall > STALL_LIMIT {
                return Err(LexError::Stall);
            }
        } else {
            self.last = pos;
            self.stall = 0;
        }
        Ok(())
    }
}

/// Length of the UTF-8 sequence starting with byte `b` (one Python character).
#[inline]
pub(crate) fn utf8_len(b: u8) -> usize {
    match b {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    }
}
