//! `RubyLexer` callbacks (pygments/lexers/ruby.py): `heredoc_callback`, and the
//! `intp_regex_callback` / `intp_string_callback` closures of `gen_rubystrings_rules`.

use crate::engine::{Ctx, Inst, Runner, Tok};
use crate::regex::Groups;
use crate::{LexError, py_rstrip, py_strip};

fn group(m: &Groups, i: usize) -> Result<(usize, usize), LexError> {
    // Python would yield `None` values (and crash in the formatter) for a group that did
    // not participate; these regexes always set every group.
    m.get(i)?
        .ok_or(LexError::Python("group did not participate"))
}

fn tok(run: &Runner, inst: Inst, key: &str) -> u16 {
    run.data.lexers[inst.id as usize].data[key]
        .as_u64()
        .expect("ruby token") as u16
}

/// `heredoc_callback(self, match, ctx)`.
/// Groups: 1 = `<<[-~]?`, 2 = quote?, 3 = name, 4 = quote again, 5 = rest of line.
pub(crate) fn heredoc_callback<'t>(
    run: &Runner,
    inst: Inst,
    text: &'t str,
    m: &Groups,
    ctx: &mut Ctx,
    out: &mut Vec<Tok<'t>>,
    base: usize,
) -> Result<(), LexError> {
    let (operator, heredoc) = (tok(run, inst, "operator"), tok(run, inst, "heredoc"));
    let (delimiter, error) = (tok(run, inst, "delimiter"), tok(run, inst, "error"));
    let g: Vec<(usize, usize)> = (1..=5).map(|i| group(m, i)).collect::<Result<_, _>>()?;
    let s = |i: usize| &text[g[i - 1].0..g[i - 1].1];
    out.push(Tok::new(base + g[0].0, operator, s(1)));
    out.push(Tok::new(base + g[1].0, heredoc, s(2)));
    out.push(Tok::new(base + g[2].0, delimiter, s(3)));
    out.push(Tok::new(base + g[3].0, heredoc, s(4)));

    let outermost = ctx.heredocstack.is_empty();
    ctx.heredocstack
        .push((s(1) == "<<-" || s(1) == "<<~", s(3).to_string()));

    ctx.pos = g[4].0;
    ctx.end = g[4].1;
    // this may find other heredocs, so limit the recursion depth
    if ctx.heredocstack.len() < 100 {
        run.extended(inst, text, ctx, out, base)?;
    } else {
        out.push(Tok::new(base + ctx.pos, heredoc, s(5)));
    }
    ctx.pos = m.end();

    if outermost {
        // this is the outer heredoc again, now we can process them all
        let stack = std::mem::take(&mut ctx.heredocstack);
        for (tolerant, hdname) in &stack {
            // `for match in line_re.finditer(ctx.text, ctx.pos)`: complete lines only.
            let mut lines: Vec<(usize, usize)> = Vec::new();
            let mut found = false;
            let mut p = ctx.pos;
            while let Some(nl) = text[p..].find('\n') {
                let (ls, le) = (p, p + nl + 1);
                let line = &text[ls..le];
                let check = if *tolerant {
                    py_strip(line)
                } else {
                    py_rstrip(line)
                };
                if check == hdname {
                    for &(a, b) in &lines {
                        out.push(Tok::new(base + a, heredoc, &text[a..b]));
                    }
                    out.push(Tok::new(base + ls, delimiter, line));
                    ctx.pos = le;
                    found = true;
                    break;
                }
                lines.push((ls, le));
                p = le;
            }
            if !found {
                // end of heredoc not found -- error!
                for &(a, b) in &lines {
                    out.push(Tok::new(base + a, error, &text[a..b]));
                }
                if let Some(&(_, b)) = lines.last() {
                    ctx.pos = b;
                }
            }
        }
        ctx.end = text.len();
    }
    Ok(())
}

/// `intp_regex_callback` (`regex == true`) and `intp_string_callback`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn intp_callback<'t>(
    run: &Runner,
    inst: Inst,
    text: &'t str,
    m: &Groups,
    ctx: &mut Ctx,
    out: &mut Vec<Tok<'t>>,
    base: usize,
    regex: bool,
) -> Result<(), LexError> {
    let (t, state) = if regex {
        (tok(run, inst, "regex"), "interpolated-regex")
    } else {
        (tok(run, inst, "other"), "interpolated-string")
    };
    let (s1, e1) = group(m, 1)?;
    out.push(Tok::new(base + s1, t, &text[s1..e1]));
    let (s3, e3) = group(m, 3)?;
    let sub = &text[s3..e3];
    let lx = &run.data.lexers[inst.id as usize];
    let mut nctx = Ctx::new(0, sub.len(), vec![lx.state_ids[state]]);
    run.extended(inst, sub, &mut nctx, out, base + s3)?;
    let (s4, e4) = group(m, 4)?;
    out.push(Tok::new(base + s4, t, &text[s4..e4]));
    ctx.pos = m.end();
    Ok(())
}
