//! The exported Pygments data (`data/pygments.json`, written by `tools/pygments/export.py`),
//! decoded into the structures the lexing engine runs on.
//!
//! Mirrors, per lexer, `cls._tokens` after `RegexLexerMeta` processed the token definitions:
//! every state is a flat rule list (includes and `inherit` resolved, `combined(...)` states
//! present as `_tmp_N`), and every rule is `(regex, action, new_state)`:
//!
//! * actions: a token type, `None`, `bygroups(...)`, `using(...)`, or one of the callbacks
//!   that are hand-ported in [`crate::lexers`];
//! * new states: `None`, a tuple of state names / `#pop` / `#push`, `-n` (pop n), `#push`.

use std::collections::{HashMap, HashSet};

use once_cell::sync::{Lazy, OnceCell};
use serde_json::Value;

use crate::regex::Rx;

/// Index into [`Data::tokens`].
pub(crate) type TokId = u16;
/// Index into [`Data::lexers`].
pub(crate) type LexId = u16;
/// Index into [`LexerDef::states`].
pub(crate) type StateId = u16;

static RAW: &str = include_str!("../data/pygments.json");

/// All exported data, decoded on first use.
pub(crate) static DATA: Lazy<Data> = Lazy::new(|| Data::load(RAW));

pub(crate) struct TokenType {
    /// Dotted name, e.g. `Token.Name.Builtin`.
    pub name: String,
    /// `HtmlFormatter()._get_css_classes(ttype)`; empty means "no span".
    pub css: String,
}

pub(crate) struct Data {
    pub markers: Markers,
    pub tokens: Vec<TokenType>,
    tok_by_name: HashMap<String, TokId>,
    pub regexes: Vec<Rx>,
    pub lexers: Vec<LexerDef>,
    lexer_by_name: HashMap<String, LexId>,
    pub registry: Vec<RegEntry>,
    pub t_text: TokId,
    pub t_whitespace: TokId,
    pub t_error: TokId,
}

/// How a lexer produces tokens.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    /// `RegexLexer.get_tokens_unprocessed`.
    Regex,
    /// `ExtendedRegexLexer.get_tokens_unprocessed` (works on a `LexerContext`).
    Extended,
    /// Not regex based: only usable through a hand port.
    Other,
}

/// Hand-ported `get_tokens_unprocessed` overrides (see [`crate::lexers`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Custom {
    Json,
    Text,
    Yaml,
    Makefile,
    CFamily,
    Lua,
    Swift,
    Php,
}

pub(crate) struct LexerDef {
    pub name: String,
    pub kind: Kind,
    pub custom: Option<Custom>,
    pub states: Vec<StateDef>,
    pub state_ids: HashMap<String, StateId>,
    /// The `root` state (every RegexLexer has one).
    pub root: StateId,
    /// Extra data for hand-ported code (sets of names, auxiliary regexes, token ids).
    pub data: Value,
    /// Name set used by the hand-ported token filters, built from `data` on first use.
    pub names: OnceCell<HashSet<String>>,
}

pub(crate) struct StateDef {
    pub rules: Vec<Rule>,
}

pub(crate) struct Rule {
    pub rx: u32,
    pub action: Action,
    pub new_state: NewState,
}

pub(crate) enum Action {
    None,
    Token(TokId),
    /// `bygroups(*args)`: one action per group (None, token type or callable).
    ByGroups(Vec<Action>),
    Using(Using),
    Callback(Callback),
}

/// `using(_other, state=..., **kwargs)`.
pub(crate) struct Using {
    /// `None` is `this`.
    pub target: Option<LexId>,
    /// The initial state stack (`state=` argument), as states of the target lexer.
    pub stack: Option<Vec<StateId>>,
    /// The only lexer keyword argument used by the supported lexers: PhpLexer's
    /// `_startinline`. `None` when no keyword arguments were given (then `using(this)`
    /// reuses the very same lexer instance).
    pub startinline: Option<bool>,
}

/// Callbacks with a hand port in [`crate::lexers`]; fields are the closure variables.
pub(crate) enum Callback {
    YamlSomething(TokId),
    YamlResetIndent(TokId),
    YamlSaveIndent { tok: TokId, start: bool },
    YamlSetIndent { tok: TokId, implicit: bool },
    YamlSetBlockScalarIndent(TokId),
    YamlParseBlockScalarEmptyLine { indent: TokId, content: TokId },
    YamlParseBlockScalarIndent(TokId),
    YamlParsePlainScalarIndent(TokId),
    RubyHeredoc,
    RubyIntpRegex,
    RubyIntpString,
    RebolWord,
}

pub(crate) enum NewState {
    None,
    /// A tuple of operations (state names, `#pop`, `#push`).
    Ops(Vec<StateOp>),
    /// `-n`: pop n states (keeping at least one).
    Pop(usize),
    /// The bare string `#push`.
    Push,
}

#[derive(Clone, Copy)]
pub(crate) enum StateOp {
    Enter(StateId),
    Pop,
    Push,
}

/// One row of `pygments.lexers._mapping.LEXERS`.
pub(crate) struct RegEntry {
    /// Lexer class name, e.g. `PythonLexer`.
    pub class: String,
    pub aliases: Vec<String>,
    pub globs: Vec<Glob>,
    pub priority: f64,
}

pub(crate) struct Glob {
    /// The fnmatch pattern as written in Pygments (used for the `*`-bonus).
    pub pattern: String,
    pub kind: GlobKind,
}

pub(crate) enum GlobKind {
    /// `*` followed by a literal: `name.endswith(literal)`.
    Suffix(String),
    /// No wildcard at all.
    Exact(String),
    /// Anything else: `fnmatch.translate(pattern)`, matched at the start of the name.
    Regex(u32),
}

impl Data {
    fn load(raw: &str) -> Data {
        let v: Value = serde_json::from_str(raw).expect("pygments.json is valid JSON");
        let markers = Markers::load(&v);

        let mut tokens = Vec::new();
        let mut tok_by_name = HashMap::new();
        for (i, t) in arr(&v["tokens"]).iter().enumerate() {
            let name = str_(&t[0]).to_string();
            tok_by_name.insert(name.clone(), i as TokId);
            tokens.push(TokenType {
                name,
                css: str_(&t[1]).to_string(),
            });
        }

        let regexes = arr(&v["regexes"])
            .iter()
            .map(|r| {
                let first = if r[1].is_null() {
                    None
                } else {
                    let lo = r[1][0].as_u64().unwrap() as u128;
                    let hi = r[1][1].as_u64().unwrap() as u128;
                    Some((lo | (hi << 64), r[1][2].as_u64().unwrap() != 0))
                };
                Rx::new(str_(&r[0]), first, &markers)
            })
            .collect();

        // Lexer ids are assigned up front so `using()` targets can be resolved.
        let lex_obj = v["lexers"].as_object().expect("lexers");
        let lexer_by_name: HashMap<String, LexId> = lex_obj
            .keys()
            .enumerate()
            .map(|(i, k)| (k.clone(), i as LexId))
            .collect();
        let tok_of = |name: &str| -> TokId {
            *tok_by_name
                .get(name)
                .unwrap_or_else(|| panic!("token type {name} not exported"))
        };
        let t_text = tok_of("Token.Text");
        let t_whitespace = tok_of("Token.Text.Whitespace");
        let t_error = tok_of("Token.Error");

        // First pass: state names of every lexer (needed to resolve `using(X, state=...)`).
        let mut state_ids_all: Vec<HashMap<String, StateId>> = Vec::new();
        for (_name, lx) in lex_obj {
            let mut ids = HashMap::new();
            if let Some(states) = lx.get("states").and_then(Value::as_object) {
                for (i, k) in states.keys().enumerate() {
                    ids.insert(k.clone(), i as StateId);
                }
            }
            state_ids_all.push(ids);
        }

        let mut lexers = Vec::new();
        for (li, (name, lx)) in lex_obj.iter().enumerate() {
            let kind = match str_(&lx["kind"]) {
                "regex" => Kind::Regex,
                "extended" => Kind::Extended,
                _ => Kind::Other,
            };
            let custom = lx.get("custom").and_then(Value::as_str).map(|c| match c {
                "json" => Custom::Json,
                "text" => Custom::Text,
                "yaml" => Custom::Yaml,
                "makefile" => Custom::Makefile,
                "cfamily" => Custom::CFamily,
                "lua" => Custom::Lua,
                "swift" => Custom::Swift,
                "php" => Custom::Php,
                other => panic!("unknown custom lexer {other}"),
            });
            let state_ids = state_ids_all[li].clone();
            let mut states = Vec::new();
            if let Some(st) = lx.get("states").and_then(Value::as_object) {
                for (_sname, rules) in st {
                    let rules = arr(rules)
                        .iter()
                        .map(|r| Rule {
                            rx: r[0].as_u64().unwrap() as u32,
                            action: parse_action(&r[1], li, &lexer_by_name, &state_ids_all),
                            new_state: parse_new_state(&r[2], &state_ids),
                        })
                        .collect();
                    states.push(StateDef { rules });
                }
            }
            let root = state_ids.get("root").copied().unwrap_or(0);
            lexers.push(LexerDef {
                name: name.clone(),
                kind,
                custom,
                states,
                state_ids,
                root,
                data: lx.get("data").cloned().unwrap_or(Value::Null),
                names: OnceCell::new(),
            });
        }

        let registry = arr(&v["registry"])
            .iter()
            .map(|e| RegEntry {
                class: str_(&e[0]).to_string(),
                aliases: arr(&e[1]).iter().map(|a| str_(a).to_string()).collect(),
                globs: arr(&e[2])
                    .iter()
                    .map(|g| Glob {
                        pattern: str_(&g[0]).to_string(),
                        kind: match str_(&g[1]) {
                            "suffix" => GlobKind::Suffix(str_(&g[2]).to_string()),
                            "exact" => GlobKind::Exact(str_(&g[2]).to_string()),
                            _ => GlobKind::Regex(g[2].as_u64().unwrap() as u32),
                        },
                    })
                    .collect(),
                priority: e[3].as_f64().unwrap(),
            })
            .collect();

        Data {
            markers,
            tokens,
            tok_by_name,
            regexes,
            lexers,
            lexer_by_name,
            registry,
            t_text,
            t_whitespace,
            t_error,
        }
    }

    pub fn lexer_id(&self, class: &str) -> Option<LexId> {
        self.lexer_by_name.get(class).copied()
    }

    pub fn tok_id(&self, name: &str) -> Option<TokId> {
        self.tok_by_name.get(name).copied()
    }
}

fn arr(v: &Value) -> &Vec<Value> {
    v.as_array().expect("array")
}

fn str_(v: &Value) -> &str {
    v.as_str().expect("string")
}

fn parse_action(
    v: &Value,
    this: usize,
    lexer_by_name: &HashMap<String, LexId>,
    state_ids_all: &[HashMap<String, StateId>],
) -> Action {
    match v {
        Value::Null => Action::None,
        Value::Number(n) => Action::Token(n.as_u64().unwrap() as TokId),
        Value::Array(a) => match str_(&a[0]) {
            "g" => Action::ByGroups(
                arr(&a[1])
                    .iter()
                    .map(|x| parse_action(x, this, lexer_by_name, state_ids_all))
                    .collect(),
            ),
            "u" => {
                let target = a[1].as_str().map(|t| {
                    *lexer_by_name
                        .get(t)
                        .unwrap_or_else(|| panic!("using() target {t} not exported"))
                });
                let target_idx = target.map(|t| t as usize).unwrap_or(this);
                let stack = a[2].as_array().map(|names| {
                    names
                        .iter()
                        .map(|n| {
                            *state_ids_all[target_idx]
                                .get(str_(n))
                                .unwrap_or_else(|| panic!("unknown state {n}"))
                        })
                        .collect()
                });
                let kwargs = a[3].as_object().expect("kwargs");
                let mut startinline = None;
                for (k, val) in kwargs {
                    match k.as_str() {
                        "_startinline" => startinline = Some(val.as_bool().unwrap()),
                        other => panic!("unsupported using() keyword argument {other}"),
                    }
                }
                Action::Using(Using {
                    target,
                    stack,
                    startinline,
                })
            }
            "c" => {
                let args = &a[2];
                let tok = |k: &str| args[k].as_u64().unwrap() as TokId;
                let flag = |k: &str| args[k].as_bool().unwrap();
                Action::Callback(match str_(&a[1]) {
                    "yaml_something" => Callback::YamlSomething(tok("token_class")),
                    "yaml_reset_indent" => Callback::YamlResetIndent(tok("token_class")),
                    "yaml_save_indent" => Callback::YamlSaveIndent {
                        tok: tok("token_class"),
                        start: flag("start"),
                    },
                    "yaml_set_indent" => Callback::YamlSetIndent {
                        tok: tok("token_class"),
                        implicit: flag("implicit"),
                    },
                    "yaml_set_block_scalar_indent" => {
                        Callback::YamlSetBlockScalarIndent(tok("token_class"))
                    }
                    "yaml_parse_block_scalar_empty_line" => {
                        Callback::YamlParseBlockScalarEmptyLine {
                            indent: tok("indent_token_class"),
                            content: tok("content_token_class"),
                        }
                    }
                    "yaml_parse_block_scalar_indent" => {
                        Callback::YamlParseBlockScalarIndent(tok("token_class"))
                    }
                    "yaml_parse_plain_scalar_indent" => {
                        Callback::YamlParsePlainScalarIndent(tok("token_class"))
                    }
                    "ruby_heredoc" => Callback::RubyHeredoc,
                    "ruby_intp_regex" => Callback::RubyIntpRegex,
                    "ruby_intp_string" => Callback::RubyIntpString,
                    "rebol_word" => Callback::RebolWord,
                    other => panic!("unknown callback {other}"),
                })
            }
            other => panic!("unknown action kind {other}"),
        },
        _ => panic!("bad action {v}"),
    }
}

fn parse_new_state(v: &Value, ids: &HashMap<String, StateId>) -> NewState {
    let op = |name: &str| match name {
        "#pop" => StateOp::Pop,
        "#push" => StateOp::Push,
        n => StateOp::Enter(*ids.get(n).unwrap_or_else(|| panic!("unknown state {n}"))),
    };
    match v {
        Value::Null => NewState::None,
        Value::String(s) if s == "#push" => NewState::Push,
        Value::Number(n) => NewState::Pop(n.as_i64().unwrap().unsigned_abs() as usize),
        Value::Array(a) => NewState::Ops(a.iter().map(|x| op(str_(x))).collect()),
        _ => panic!("bad new state {v}"),
    }
}

/// The private-use marker characters the exporter writes for big Unicode sets and `\b`/`\B`,
/// and their expansions.
pub(crate) struct Markers {
    /// (marker, class body) for the sets that only appear inside `[...]`.
    pub sets: Vec<(char, String)>,
    pub wb: char,
    pub nwb: char,
    /// Class body of Python's `\w` (Unicode).
    pub word: String,
}

impl Markers {
    fn load(v: &Value) -> Markers {
        let mut sets = Vec::new();
        let mut word = String::new();
        for (key, val) in v["unicode"].as_object().expect("unicode") {
            let marker = str_(&val[0]).chars().next().unwrap();
            let mut body = String::new();
            for r in arr(&val[1]) {
                let lo = r[0].as_u64().unwrap() as u32;
                let hi = r[1].as_u64().unwrap() as u32;
                push_class_char(&mut body, lo);
                if hi != lo {
                    if hi > lo + 1 {
                        body.push('-');
                    }
                    push_class_char(&mut body, hi);
                }
            }
            if key == "W" {
                word = body.clone();
            }
            sets.push((marker, body));
        }
        let wb = str_(&v["markers"]["wb"]).chars().next().unwrap();
        let nwb = str_(&v["markers"]["nwb"]).chars().next().unwrap();
        Markers {
            sets,
            wb,
            nwb,
            word,
        }
    }
}

/// Same escaping as the exporter's `esc_char(cp, in_class=True)`.
fn push_class_char(out: &mut String, cp: u32) {
    let c = char::from_u32(cp).expect("no surrogates in exported sets");
    if c.is_ascii_alphanumeric() || c == '_' || c == '<' || c == '>' {
        out.push(c);
    } else if (0x21..=0x7e).contains(&cp) {
        out.push('\\');
        out.push(c);
    } else {
        out.push_str(&format!("\\x{{{cp:X}}}"));
    }
}
