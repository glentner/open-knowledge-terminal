//! `JsonLexer.get_tokens_unprocessed` (pygments/lexers/data.py), a hand-written scanner.
//!
//! Strings are queued until the next significant character: a `:` turns queued
//! `String.Double` tokens into `Name.Tag` (object keys), anything else flushes them as they
//! are. Ported statement by statement.

use crate::data::{Data, LexerDef, TokId};
use crate::engine::Tok;

struct Toks {
    string: TokId,
    ws: TokId,
    constant: TokId,
    float: TokId,
    integer: TokId,
    punct: TokId,
    single: TokId,
    multi: TokId,
    error: TokId,
    tag: TokId,
}

fn is_hex(c: char) -> bool {
    c.is_ascii_hexdigit()
}

fn is_integer(c: char) -> bool {
    c == '-' || c.is_ascii_digit()
}

fn is_float(c: char) -> bool {
    matches!(c, '.' | 'e' | 'E' | '+')
}

fn is_constant(c: char) -> bool {
    // set('truefalsenull')
    matches!(c, 't' | 'r' | 'u' | 'e' | 'f' | 'a' | 'l' | 's' | 'n')
}

fn is_punct(c: char) -> bool {
    matches!(c, '{' | '}' | '[' | ']' | ',')
}

fn is_ws(c: char) -> bool {
    matches!(c, ' ' | '\n' | '\r' | '\t')
}

pub(crate) fn lex<'t>(
    _data: &Data,
    lx: &LexerDef,
    text: &'t str,
    out: &mut Vec<Tok<'t>>,
    base: usize,
) {
    let t = |k: &str| lx.data[k].as_u64().expect("json token") as TokId;
    let k = Toks {
        string: t("string"),
        ws: t("ws"),
        constant: t("constant"),
        float: t("float"),
        integer: t("integer"),
        punct: t("punct"),
        single: t("single"),
        multi: t("multi"),
        error: t("error"),
        tag: t("tag"),
    };

    let mut in_string = false;
    let mut in_escape = false;
    let mut in_unicode_escape = 0u32;
    let mut in_whitespace = false;
    let mut in_constant = false;
    let mut in_number = false;
    let mut in_float = false;
    let mut in_punctuation = false;
    let mut in_comment_single = false;
    let mut in_comment_multiline = false;
    let mut expecting_second_comment_opener = false; // // or /*
    let mut expecting_second_comment_closer = false; // */
    let mut start = 0usize;
    // (start, token, text)
    let mut queue: Vec<Tok<'t>> = Vec::new();
    let emit = |out: &mut Vec<Tok<'t>>, pos: usize, ty: TokId, s: &'t str| {
        out.push(Tok::new(base + pos, ty, s))
    };
    let flush = |out: &mut Vec<Tok<'t>>, queue: &mut Vec<Tok<'t>>| out.append(queue);

    for (stop, character) in text.char_indices() {
        let next = stop + character.len_utf8(); // `stop + 1` in characters
        if in_string {
            if in_unicode_escape > 0 {
                if is_hex(character) {
                    in_unicode_escape -= 1;
                    if in_unicode_escape == 0 {
                        in_escape = false;
                    }
                } else {
                    in_unicode_escape = 0;
                    in_escape = false;
                }
            } else if in_escape {
                if character == 'u' {
                    in_unicode_escape = 4;
                } else {
                    in_escape = false;
                }
            } else if character == '\\' {
                in_escape = true;
            } else if character == '"' {
                queue.push(Tok::new(base + start, k.string, &text[start..next]));
                in_string = false;
                in_escape = false;
                in_unicode_escape = 0;
            }
            continue;
        } else if in_whitespace {
            if is_ws(character) {
                continue;
            }
            if !queue.is_empty() {
                queue.push(Tok::new(base + start, k.ws, &text[start..stop]));
            } else {
                emit(out, start, k.ws, &text[start..stop]);
            }
            in_whitespace = false;
            // Fall through so the new character can be evaluated.
        } else if in_constant {
            if is_constant(character) {
                continue;
            }
            emit(out, start, k.constant, &text[start..stop]);
            in_constant = false;
        } else if in_number {
            if is_integer(character) {
                continue;
            } else if is_float(character) {
                in_float = true;
                continue;
            }
            if in_float {
                emit(out, start, k.float, &text[start..stop]);
            } else {
                emit(out, start, k.integer, &text[start..stop]);
            }
            in_number = false;
            in_float = false;
        } else if in_punctuation {
            if is_punct(character) {
                continue;
            }
            emit(out, start, k.punct, &text[start..stop]);
            in_punctuation = false;
        } else if in_comment_single {
            if character != '\n' {
                continue;
            }
            if !queue.is_empty() {
                queue.push(Tok::new(base + start, k.single, &text[start..stop]));
            } else {
                emit(out, start, k.single, &text[start..stop]);
            }
            in_comment_single = false;
        } else if in_comment_multiline {
            if character == '*' {
                expecting_second_comment_closer = true;
            } else if expecting_second_comment_closer {
                expecting_second_comment_closer = false;
                if character == '/' {
                    if !queue.is_empty() {
                        queue.push(Tok::new(base + start, k.multi, &text[start..next]));
                    } else {
                        emit(out, start, k.multi, &text[start..next]);
                    }
                    in_comment_multiline = false;
                }
            }
            continue;
        } else if expecting_second_comment_opener {
            expecting_second_comment_opener = false;
            if character == '/' {
                in_comment_single = true;
                continue;
            } else if character == '*' {
                in_comment_multiline = true;
                continue;
            }
            // Exhaust the queue. Accept the existing token types.
            flush(out, &mut queue);
            emit(out, start, k.error, &text[start..stop]);
        }

        start = stop;

        if character == '"' {
            in_string = true;
        } else if is_ws(character) {
            in_whitespace = true;
        } else if matches!(character, 'f' | 'n' | 't') {
            // The first letters of true|false|null
            flush(out, &mut queue);
            in_constant = true;
        } else if is_integer(character) {
            flush(out, &mut queue);
            in_number = true;
        } else if character == ':' {
            // Yield from the queue. Replace string token types.
            for mut q in queue.drain(..) {
                if q.ty == k.string {
                    q.ty = k.tag;
                }
                out.push(q);
            }
            in_punctuation = true;
        } else if is_punct(character) {
            flush(out, &mut queue);
            in_punctuation = true;
        } else if character == '/' {
            // This is the beginning of a comment.
            expecting_second_comment_opener = true;
        } else {
            flush(out, &mut queue);
            emit(out, start, k.error, &text[start..next]);
        }
    }

    // Yield any remaining text.
    flush(out, &mut queue);
    let rest = &text[start..];
    if in_string {
        emit(out, start, k.error, rest);
    } else if in_float {
        emit(out, start, k.float, rest);
    } else if in_number {
        emit(out, start, k.integer, rest);
    } else if in_constant {
        emit(out, start, k.constant, rest);
    } else if in_whitespace {
        emit(out, start, k.ws, rest);
    } else if in_punctuation {
        emit(out, start, k.punct, rest);
    } else if in_comment_single {
        emit(out, start, k.single, rest);
    } else if in_comment_multiline || expecting_second_comment_opener {
        emit(out, start, k.error, rest);
    }
}
