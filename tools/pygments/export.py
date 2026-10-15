#!/usr/bin/env python3
"""Export Pygments lexer data for the `okt-pygments` crate.

Run with the Pygments and the Python minor version okt-pygments matches (Python's own
Unicode tables decide what ``\\w``, ``\\d`` and ``\\s`` match)::

    uv run --python 3.13 --with pygments==2.21.0 python tools/pygments/export.py \\
        okt-pygments/data/pygments.json

What gets exported (all as one compact JSON document):

``tokens``
    Every token type the exported lexers can produce, as ``[dotted name, css class]``.
    The class string is exactly ``HtmlFormatter()._get_css_classes(ttype)`` ('' = no span).

``regexes``
    Every regex used by an exported lexer, *already translated* from Python ``re`` syntax to
    the syntax accepted by fancy-regex 0.19. Translation works on Python's own parse tree
    (``re._parser``), so escapes, verbose mode, literal ``{`` and scoped flags are handled
    exactly as Python does. Character classes are emitted as explicit code-point sets computed
    with Python itself (exact ``\\w``/``\\d``/``\\s`` for Unicode 15.1 and exact IGNORECASE
    folding), so no ``(?i)`` or ``\\w`` reaches the Rust engine. Big sets are emitted as
    private-use marker characters that the Rust side expands (see ``MARKERS``). ``\\b``/``\\B``
    become markers too: Rust expands them to its native word boundary when the text has no
    character on which Python's and Rust's notion of "word character" differ, and to an
    exact lookaround form otherwise. Each regex also carries a first-character filter.

``lexers``
    The processed token tables (``cls._tokens`` after instantiation, i.e. with includes,
    ``inherit`` and ``combined`` states resolved) of every supported lexer and every lexer
    they delegate to with ``using()``, plus extra data the hand-ported parts need.

``registry``
    All Pygments lexers in ``LEXERS`` order (class name, aliases, filename globs, priority), so
    ``get_lexer_by_name``/``get_lexer_for_filename`` can be replicated exactly.

The script fails loudly when a supported lexer uses a callback it does not know.
"""

import bisect
import json
import re
import sys
from re import _constants as C
from re import _parser as sre_parse

import _sre
from re._casefix import _EXTRA_CASES

import pygments
from pygments import lexer as plexer
from pygments.formatters import HtmlFormatter
from pygments.lexers import find_lexer_class
from pygments.lexers._mapping import LEXERS
from pygments.token import Token, _TokenType

assert pygments.__version__ == '2.21.0', pygments.__version__

# --------------------------------------------------------------------------------------------
# Which lexers to export
# --------------------------------------------------------------------------------------------

# Lexers the reference can reach (see terminal.py `classify` + `highlight_lines`), in rough
# priority order. Lexers they delegate to via using() are added automatically.
SUPPORTED = [
    'PythonLexer', 'BashLexer', 'JsonLexer', 'YamlLexer', 'TOMLLexer', 'HtmlLexer',
    'JavascriptLexer', 'TypeScriptLexer', 'TsxLexer', 'JsxLexer', 'CssLexer', 'ScssLexer',
    'TransactSqlLexer', 'RustLexer', 'GoLexer', 'CLexer', 'CppLexer', 'MakefileLexer',
    'DockerLexer', 'RubyLexer', 'PerlLexer', 'SLexer', 'RebolLexer', 'JuliaLexer', 'LuaLexer',
    'JavaLexer', 'KotlinLexer', 'SwiftLexer', 'PhpLexer', 'PowerShellLexer', 'XmlLexer',
    'AwkLexer', 'FishShellLexer', 'VueLexer', 'PrologLexer', 'IniLexer', 'TextLexer',
]

# Lexers whose get_tokens_unprocessed is overridden; the Rust crate has a hand port for each.
# Value: name of the Rust implementation (`custom` in the JSON).
CUSTOM = {
    'JsonLexer': 'json',
    'TextLexer': 'text',
    'YamlLexer': 'yaml',
    'MakefileLexer': 'makefile',
    'CLexer': 'cfamily',
    'CppLexer': 'cfamily',
    'LuaLexer': 'lua',
    'SwiftLexer': 'swift',
    'PhpLexer': 'php',
}

# Callbacks (by __qualname__) that the Rust crate implements. Closure variables are exported
# as arguments.
KNOWN_CALLBACKS = {
    'YamlLexer.something.<locals>.callback': 'yaml_something',
    'YamlLexer.reset_indent.<locals>.callback': 'yaml_reset_indent',
    'YamlLexer.save_indent.<locals>.callback': 'yaml_save_indent',
    'YamlLexer.set_indent.<locals>.callback': 'yaml_set_indent',
    'YamlLexer.set_block_scalar_indent.<locals>.callback': 'yaml_set_block_scalar_indent',
    'YamlLexer.parse_block_scalar_empty_line.<locals>.callback': 'yaml_parse_block_scalar_empty_line',
    'YamlLexer.parse_block_scalar_indent.<locals>.callback': 'yaml_parse_block_scalar_indent',
    'YamlLexer.parse_plain_scalar_indent.<locals>.callback': 'yaml_parse_plain_scalar_indent',
    'RubyLexer.heredoc_callback': 'ruby_heredoc',
    'RubyLexer.gen_rubystrings_rules.<locals>.intp_regex_callback': 'ruby_intp_regex',
    'RubyLexer.gen_rubystrings_rules.<locals>.intp_string_callback': 'ruby_intp_string',
    'RebolLexer.word_callback': 'rebol_word',
}

# Lexer options the reference passes (highlight_lines: stripnl=False, ensurenl=False).
OPTIONS = {'stripnl': False, 'ensurenl': False}

# Token types created at lex time by code we hand-port (so they don't exist after import).
EXTRA_TOKENS = [Token.Text.Whitespace.Error]

MAXCP = 0x10FFFF
SURROGATES = (0xD800, 0xDFFF)

# --------------------------------------------------------------------------------------------
# Code-point range sets: sorted lists of disjoint, non-adjacent inclusive (lo, hi) tuples.
# --------------------------------------------------------------------------------------------


def rs_from_points(points):
    out = []
    for cp in sorted(set(points)):
        if out and out[-1][1] + 1 == cp:
            out[-1] = (out[-1][0], cp)
        else:
            out.append((cp, cp))
    return out


def rs_normalize(ranges):
    out = []
    for lo, hi in sorted(ranges):
        if out and lo <= out[-1][1] + 1:
            if hi > out[-1][1]:
                out[-1] = (out[-1][0], hi)
        else:
            out.append((lo, hi))
    return out


def rs_union(*sets):
    return rs_normalize([r for s in sets for r in s])


def rs_complement(a):
    out, nxt = [], 0
    for lo, hi in a:
        if lo > nxt:
            out.append((nxt, lo - 1))
        nxt = hi + 1
    if nxt <= MAXCP:
        out.append((nxt, MAXCP))
    return out


def rs_intersect(a, b):
    out, i, j = [], 0, 0
    while i < len(a) and j < len(b):
        lo, hi = max(a[i][0], b[j][0]), min(a[i][1], b[j][1])
        if lo <= hi:
            out.append((lo, hi))
        if a[i][1] < b[j][1]:
            i += 1
        else:
            j += 1
    return out


def rs_diff(a, b):
    return rs_intersect(a, rs_complement(b))


def rs_contains(a, cp):
    i = bisect.bisect_right(a, (cp, MAXCP + 1)) - 1
    return i >= 0 and a[i][0] <= cp <= a[i][1]


def rs_without_surrogates(a):
    return rs_diff(a, [SURROGATES])


def rs_is_subset(a, b):
    return rs_diff(a, b) == []


ALL = [(0, MAXCP)]


def py_set(regex):
    """The exact set of code points matched by a single-character Python regex."""
    m = re.compile(regex).match
    return rs_from_points(cp for cp in range(MAXCP + 1) if m(chr(cp)))


print('computing unicode sets ...', file=sys.stderr)
UNI_WORD = py_set(r'\w')
UNI_DIGIT = py_set(r'\d')
UNI_SPACE = py_set(r'\s')
ASCII_WORD = py_set(r'(?a)\w')
ASCII_DIGIT = py_set(r'(?a)\d')
ASCII_SPACE = py_set(r'(?a)\s')

# Named sets the Rust side knows; a pattern refers to them with one private-use character
# (always inside a class: `[]`), which Rust expands to the range list.
MARKERS = {
    'W': ('', rs_without_surrogates(UNI_WORD)),
    'D': ('', rs_without_surrogates(UNI_DIGIT)),
    'S': ('', rs_without_surrogates(UNI_SPACE)),
    'L': ('', rs_without_surrogates(rs_diff(UNI_WORD, UNI_DIGIT))),   # [^\W\d]
}
MARK_WB = ''      # \b (Unicode, Python semantics)
MARK_NWB = ''     # \B

# Code points whose IGNORECASE behaviour may differ from plain matching: everything that is
# cased, every lowercase image, and Python's extra case fixes. For any other code point,
# matching under IGNORECASE is identical to matching without it.
_lower = _sre.unicode_tolower
CASE_CANDIDATES = set()
for _cp in range(MAXCP + 1):
    if _sre.unicode_iscased(_cp):
        CASE_CANDIDATES.add(_cp)
        CASE_CANDIDATES.add(_lower(_cp))
for _k, _vs in _EXTRA_CASES.items():
    CASE_CANDIDATES.add(_k)
    CASE_CANDIDATES.update(_vs)
for _cp in list(CASE_CANDIDATES):
    CASE_CANDIDATES.add(_lower(_cp))
CASE_CANDIDATES = sorted(CASE_CANDIDATES)
CASE_CANDIDATES_RS = rs_from_points(CASE_CANDIDATES)

# --------------------------------------------------------------------------------------------
# Regex translation (Python re -> fancy-regex), working on Python's parse tree
# --------------------------------------------------------------------------------------------

F_I, F_M, F_S, F_A = C.SRE_FLAG_IGNORECASE, C.SRE_FLAG_MULTILINE, C.SRE_FLAG_DOTALL, C.SRE_FLAG_ASCII
MAXREPEAT = C.MAXREPEAT


class Untranslatable(Exception):
    pass


def esc_char(cp, in_class):
    """Escape one code point for fancy-regex (pattern stays pure ASCII)."""
    c = chr(cp)
    if c.isascii() and (c.isalnum() or c == '_'):
        return c
    if c in '<>':           # never `\<` / `\>`: those are word-boundary assertions in Rust
        return c
    if c == ' ' and not in_class:
        return c
    if 0x21 <= cp <= 0x7E:  # other ASCII punctuation: backslash escape is always literal
        return '\\' + c
    return '\\x{%X}' % cp


def class_body(ranges):
    out = []
    for lo, hi in ranges:
        if lo == hi:
            out.append(esc_char(lo, True))
        elif hi == lo + 1:
            out.append(esc_char(lo, True) + esc_char(hi, True))
        else:
            out.append(esc_char(lo, True) + '-' + esc_char(hi, True))
    return ''.join(out)


def decompose(ranges):
    """Express a set as marker sets plus a remainder; return the class body text."""
    rest = ranges
    parts = []
    for key in ('W', 'L', 'D', 'S'):
        mark, mset = MARKERS[key]
        if mset and rs_is_subset(mset, rest):
            parts.append(mark)
            rest = rs_diff(rest, mset)
    return ''.join(parts) + class_body(rest)


def emit_set(ranges):
    """Emit a set of code points as one atom."""
    ranges = rs_without_surrogates(ranges)
    full = rs_without_surrogates(ALL)
    if ranges == full:
        return '(?s:.)'
    if ranges == rs_diff(full, [(10, 10)]):
        return '[^\\n]'
    if len(ranges) == 1 and ranges[0][0] == ranges[0][1]:
        return esc_char(ranges[0][0], False)
    if not ranges:
        return '[^\\x{0}-\\x{10FFFF}]'
    pos = decompose(ranges)
    neg = decompose(rs_without_surrogates(rs_complement(ranges)))
    if len(neg) + 1 < len(pos) and neg:
        return '[^' + neg + ']'
    return '[' + pos + ']'


def category_set(cat, flags):
    ascii_ = flags & F_A
    if cat == C.CATEGORY_DIGIT:
        return ASCII_DIGIT if ascii_ else UNI_DIGIT
    if cat == C.CATEGORY_NOT_DIGIT:
        return rs_complement(ASCII_DIGIT if ascii_ else UNI_DIGIT)
    if cat == C.CATEGORY_SPACE:
        return ASCII_SPACE if ascii_ else UNI_SPACE
    if cat == C.CATEGORY_NOT_SPACE:
        return rs_complement(ASCII_SPACE if ascii_ else UNI_SPACE)
    if cat == C.CATEGORY_WORD:
        return ASCII_WORD if ascii_ else UNI_WORD
    if cat == C.CATEGORY_NOT_WORD:
        return rs_complement(ASCII_WORD if ascii_ else UNI_WORD)
    raise Untranslatable(f'category {cat}')


PY_CAT_SRC = {
    C.CATEGORY_DIGIT: r'\d', C.CATEGORY_NOT_DIGIT: r'\D', C.CATEGORY_SPACE: r'\s',
    C.CATEGORY_NOT_SPACE: r'\S', C.CATEGORY_WORD: r'\w', C.CATEGORY_NOT_WORD: r'\W',
}


def class_items_plain(items, flags):
    """Set matched by an IN item list, ignoring IGNORECASE."""
    negate = False
    parts = []
    for op, av in items:
        if op is C.NEGATE:
            negate = True
        elif op is C.LITERAL:
            parts.append([(av, av)])
        elif op is C.RANGE:
            parts.append([(av[0], av[1])])
        elif op is C.CATEGORY:
            parts.append(category_set(av, flags))
        else:
            raise Untranslatable(f'class item {op}')
    s = rs_union(*parts)
    return rs_complement(s) if negate else s


def class_items_pysrc(items):
    """Python source of an equivalent single-character class (for IGNORECASE evaluation)."""
    out = ['[']
    for op, av in items:
        if op is C.NEGATE:
            out.append('^')
        elif op is C.LITERAL:
            out.append('\\U%08x' % av)
        elif op is C.RANGE:
            out.append('\\U%08x-\\U%08x' % av)
        elif op is C.CATEGORY:
            out.append(PY_CAT_SRC[av])
        else:
            raise Untranslatable(f'class item {op}')
    out.append(']')
    return ''.join(out)


_ci_cache = {}


def exact_set(items, flags):
    """Exact set of code points matched by a one-character construct given as IN items."""
    plain = class_items_plain(items, flags)
    if not flags & F_I:
        return plain
    src = class_items_pysrc(items)
    key = (src, flags & (F_I | F_A))
    if key not in _ci_cache:
        pyflags = re.IGNORECASE | (re.ASCII if flags & F_A else 0)
        m = re.compile(src, pyflags).match
        hits = [cp for cp in CASE_CANDIDATES if m(chr(cp))]
        _ci_cache[key] = rs_union(rs_diff(plain, CASE_CANDIDATES_RS), rs_from_points(hits))
    return _ci_cache[key]


def atom_set(op, av, flags):
    if op is C.LITERAL:
        return exact_set([(C.LITERAL, av)], flags)
    if op is C.NOT_LITERAL:
        return exact_set([(C.NEGATE, None), (C.LITERAL, av)], flags)
    if op is C.IN:
        return exact_set(av, flags)
    if op is C.ANY:
        return ALL if flags & F_S else rs_diff(ALL, [(10, 10)])
    raise AssertionError(op)


def combine_flags(flags, add, rem):
    if add & sre_parse.TYPE_FLAGS:
        flags &= ~sre_parse.TYPE_FLAGS
    return (flags | add) & ~rem


class First:
    """First-character information: can the regex match empty, and which chars can start it."""

    __slots__ = ('nullable', 'ascii', 'nonascii')

    def __init__(self, nullable=True, ascii_=0, nonascii=False):
        self.nullable, self.ascii, self.nonascii = nullable, ascii_, nonascii

    @staticmethod
    def of_set(ranges):
        mask = 0
        for lo, hi in rs_intersect(ranges, [(0, 127)]):
            for cp in range(lo, hi + 1):
                mask |= 1 << cp
        return First(False, mask, bool(rs_intersect(ranges, [(128, MAXCP)])))

    @staticmethod
    def any():
        return First(True, (1 << 128) - 1, True)

    def union(self, other):
        return First(self.nullable or other.nullable, self.ascii | other.ascii,
                     self.nonascii or other.nonascii)


def first_seq(firsts):
    out = First(True, 0, False)
    for f in firsts:
        out.ascii |= f.ascii
        out.nonascii = out.nonascii or f.nonascii
        if not f.nullable:
            out.nullable = False
            break
    return out


class Translator:
    """Translate one Python pattern into fancy-regex syntax plus a first-character filter."""

    def __init__(self, pattern, flags):
        self.pattern = pattern
        self.warnings = []
        self.group_chars = {}
        p = sre_parse.parse(pattern, flags)
        self.flags = p.state.flags
        self.ngroups = p.state.groups - 1
        self.out, self.first = self.seq(p, self.flags, in_loop=False)

    def seq(self, items, flags, in_loop):
        strs, firsts = [], []
        for op, av in items:
            s, f, _ = self.item(op, av, flags, in_loop)
            strs.append(s)
            firsts.append(f)
        return ''.join(strs), first_seq(firsts)

    def item(self, op, av, flags, in_loop):
        """Returns (text, First, is_atom)."""
        if op in (C.LITERAL, C.NOT_LITERAL, C.IN, C.ANY):
            s = atom_set(op, av, flags)
            return emit_set(s), First.of_set(s), True
        if op is C.AT:
            return self.at(av, flags), First(True), False
        if op is C.BRANCH:
            parts, first = [], None
            for alt in av[1]:
                s, f = self.seq(alt, flags, in_loop)
                parts.append(s)
                first = f if first is None else first.union(f)
            return '(?:' + '|'.join(parts) + ')', first, True
        if op is C.SUBPATTERN:
            group, add, rem, p = av
            nf = combine_flags(flags, add, rem)
            s, f = self.seq(p, nf, in_loop)
            if group:
                if in_loop and f.nullable:
                    # Python's sre and Rust's regex crate may disagree on what such a group
                    # captures in the final (empty) iteration.
                    self.warnings.append('capture group with nullable body inside a loop')
                self.group_chars[group] = self.chars_of(p, nf)
                return '(' + s + ')', f, True
            return '(?:' + s + ')', f, True
        if op in (C.MAX_REPEAT, C.MIN_REPEAT, C.POSSESSIVE_REPEAT):
            lo, hi, sub = av
            s, f = self.seq(sub, flags, in_loop=in_loop or hi > 1)
            if not (len(sub) == 1 and self.is_atom(sub[0])):
                s = '(?:' + s + ')'
            if (lo, hi) == (0, MAXREPEAT):
                q = '*'
            elif (lo, hi) == (1, MAXREPEAT):
                q = '+'
            elif (lo, hi) == (0, 1):
                q = '?'
            elif lo == hi:
                q = '{%d}' % lo
            elif hi == MAXREPEAT:
                q = '{%d,}' % lo
            else:
                q = '{%d,%d}' % (lo, hi)
            if op is C.MIN_REPEAT:
                q += '?'
            text = s + q
            if op is C.POSSESSIVE_REPEAT:
                text = '(?>' + text + ')'
            nullable = lo == 0 or f.nullable or hi == 0
            if hi == 0:
                f = First(True)
            return text, First(nullable, f.ascii, f.nonascii), op is C.POSSESSIVE_REPEAT
        if op in (C.ASSERT, C.ASSERT_NOT):
            direction, p = av
            s, _ = self.seq(p, flags, in_loop)
            if op is C.ASSERT:
                head = '(?=' if direction == 1 else '(?<='
            else:
                head = '(?!' if direction == 1 else '(?<!'
            return head + s + ')', First(True), True
        if op is C.GROUPREF:
            if flags & F_I and rs_intersect(self.group_chars.get(av, ALL), CASE_CANDIDATES_RS):
                # fancy-regex compares by simple case folding with equal byte length; Python
                # compares lower() per character. Identical for ASCII text.
                self.warnings.append('case-insensitive backreference to a group with cased chars')
                return '(?i:\\%d)' % av, First.any(), True
            return '(?:\\%d)' % av, First.any(), True
        if op is C.GROUPREF_EXISTS:
            g, yes, no = av
            ys, yf = self.seq(yes, flags, in_loop)
            if no is not None:
                ns, nf = self.seq(no, flags, in_loop)
            else:
                ns, nf = '', First(True)
            return '(?(%d)%s|%s)' % (g, ys, ns), yf.union(nf), True
        if op is C.ATOMIC_GROUP:
            s, f = self.seq(av, flags, in_loop)
            return '(?>' + s + ')', f, True
        if op is C.FAILURE:
            return '(?!)', First(True), True
        raise Untranslatable(f'opcode {op}')

    def chars_of(self, items, flags):
        """Superset of the characters a sub-pattern can consume."""
        out = []
        for op, av in items:
            if op in (C.LITERAL, C.NOT_LITERAL, C.IN, C.ANY):
                out.append(atom_set(op, av, flags))
            elif op is C.BRANCH:
                out.extend(self.chars_of(alt, flags) for alt in av[1])
            elif op is C.SUBPATTERN:
                out.append(self.chars_of(av[3], combine_flags(flags, av[1], av[2])))
            elif op in (C.MAX_REPEAT, C.MIN_REPEAT, C.POSSESSIVE_REPEAT):
                out.append(self.chars_of(av[2], flags))
            elif op is C.ATOMIC_GROUP:
                out.append(self.chars_of(av, flags))
            elif op is C.GROUPREF_EXISTS:
                out.append(self.chars_of(av[1], flags))
                if av[2] is not None:
                    out.append(self.chars_of(av[2], flags))
            elif op is C.GROUPREF:
                out.append(self.group_chars.get(av, ALL))
        return rs_union(*out)

    @staticmethod
    def is_atom(item):
        op, av = item
        if op in (C.LITERAL, C.NOT_LITERAL, C.IN, C.ANY, C.BRANCH, C.SUBPATTERN, C.ATOMIC_GROUP,
                  C.GROUPREF):
            return True
        return False

    def at(self, code, flags):
        if code == C.AT_BEGINNING:
            return '(?m:^)' if flags & F_M else '\\A'
        if code == C.AT_BEGINNING_STRING:
            return '\\A'
        if code == C.AT_END:
            return '(?m:$)' if flags & F_M else '(?=\\n?\\z)'
        if code == C.AT_END_STRING:
            return '\\z'
        if code in (C.AT_BOUNDARY, C.AT_NON_BOUNDARY):
            if flags & F_A:
                w = '[' + class_body(ASCII_WORD) + ']'
                if code == C.AT_BOUNDARY:
                    return f'(?:(?<={w})(?!{w})|(?<!{w})(?={w}))'
                return f'(?:(?<={w})(?={w})|(?<!{w})(?!{w})(?:(?<=[\\s\\S])|(?=[\\s\\S])))'
            return MARK_WB if code == C.AT_BOUNDARY else MARK_NWB
        raise Untranslatable(f'at code {code}')


def translate(pattern, flags):
    t = Translator(pattern, flags)
    f = t.first
    if f.nullable:
        first = None
    else:
        first = [f.ascii & ((1 << 64) - 1), f.ascii >> 64, int(f.nonascii)]
    return t.out, first, t.warnings


# --------------------------------------------------------------------------------------------
# Export state
# --------------------------------------------------------------------------------------------


class Exporter:
    def __init__(self):
        self.token_ids = {}
        self.tokens = []
        self.regex_ids = {}
        self.regexes = []
        self.lexers = {}
        self.pending = []
        self.problems = []
        self.warnings = {}

    # tokens -------------------------------------------------------------------------------
    def tok(self, ttype):
        assert type(ttype) is _TokenType, ttype
        if ttype not in self.token_ids:
            self.token_ids[ttype] = len(self.tokens)
            self.tokens.append(ttype)
        return self.token_ids[ttype]

    # regexes ------------------------------------------------------------------------------
    def rx(self, pattern, flags, where=''):
        key = (pattern, flags)
        if key not in self.regex_ids:
            try:
                out, first, warns = translate(pattern, flags)
            except Untranslatable as e:
                self.problems.append(f'{where}: untranslatable regex {pattern!r}: {e}')
                out, first, warns = '(?!)', None, []
            for w in warns:
                self.warnings.setdefault(w, []).append(f'{where}: {pattern!r}')
            self.regex_ids[key] = len(self.regexes)
            self.regexes.append([out, first])
        return self.regex_ids[key]

    # lexers -------------------------------------------------------------------------------
    def want(self, clsname):
        if clsname not in self.lexers and clsname not in self.pending:
            self.pending.append(clsname)

    def run(self):
        for name in SUPPORTED:
            self.want(name)
        while self.pending:
            name = self.pending.pop(0)
            self.export_lexer(name)

    def export_lexer(self, clsname):
        cls = lexer_class(clsname)
        entry = {}
        self.lexers[clsname] = entry
        gtu = cls.get_tokens_unprocessed
        if clsname in CUSTOM:
            entry['custom'] = CUSTOM[clsname]
        elif gtu is plexer.RegexLexer.get_tokens_unprocessed:
            pass
        elif gtu is plexer.ExtendedRegexLexer.get_tokens_unprocessed:
            pass
        else:
            self.problems.append(f'{clsname}: overrides get_tokens_unprocessed, no hand port')
        if cls(**OPTIONS).filters:
            # get_tokens() would run these; the Rust side does not model filters
            self.problems.append(f'{clsname}: installs default token filters')
        if issubclass(cls, plexer.RegexLexer):
            inst = cls(**OPTIONS)
            entry['kind'] = 'extended' if issubclass(cls, plexer.ExtendedRegexLexer) else 'regex'
            states = {}
            for state, rules in inst._tokens.items():
                out = []
                for rexmatch, action, new_state in rules:
                    pat = rexmatch.__self__
                    rid = self.rx(pat.pattern, pat.flags, f'{clsname}:{state}')
                    out.append([rid, self.action(action, clsname, state), new_state_json(new_state)])
                states[state] = out
            entry['states'] = states
        else:
            entry['kind'] = 'other'
        extra = EXTRA_DATA.get(clsname)
        if extra:
            entry['data'] = extra(self, cls)

    def action(self, action, clsname, state):
        if action is None:
            return None
        if type(action) is _TokenType:
            return self.tok(action)
        qual = getattr(action, '__qualname__', '')
        cells = closure(action)
        if qual == 'bygroups.<locals>.callback':
            return ['g', [self.action(a, clsname, state) for a in cells['args']]]
        if qual == 'using.<locals>.callback':
            other = cells.get('_other', plexer.this)  # the `this` variant doesn't close over it
            gt = cells.get('gt_kwargs', {})
            kwargs = cells.get('kwargs', {})
            if other is plexer.this:
                target = None
            else:
                target = other.__name__
                self.want(target)
            stack = list(gt['stack']) if 'stack' in gt else None
            return ['u', target, stack, dict(kwargs)]
        if qual in KNOWN_CALLBACKS:
            args = {}
            for k, v in cells.items():
                if type(v) is _TokenType:
                    args[k] = self.tok(v)
                elif isinstance(v, (bool, int, str)) or v is None:
                    args[k] = v
                else:
                    self.problems.append(f'{clsname}:{state}: callback {qual} arg {k}={v!r}')
            return ['c', KNOWN_CALLBACKS[qual], args]
        self.problems.append(f'{clsname}:{state}: unknown callback {qual or action!r}')
        return ['c', 'unknown:' + (qual or repr(action)), {}]


def closure(fn):
    code = getattr(fn, '__code__', None)
    if code is None or not fn.__closure__:
        return {}
    return {name: cell.cell_contents for name, cell in zip(code.co_freevars, fn.__closure__)}


def new_state_json(ns):
    if ns is None:
        return None
    if isinstance(ns, tuple):
        return list(ns)
    if isinstance(ns, int):
        return ns
    if ns == '#push':
        return '#push'
    raise AssertionError(ns)


def lexer_class(clsname):
    for _key, (_mod, name, _a, _f, _m) in LEXERS.items():
        cls = find_lexer_class(name)
        if cls.__name__ == clsname:
            return cls
    # not in the registry (e.g. BaseMakefileLexer is, but helper classes may not be)
    for mod in {m for (m, *_rest) in LEXERS.values()}:
        module = __import__(mod, None, None, ['__all__'])
        if hasattr(module, clsname):
            return getattr(module, clsname)
    raise KeyError(clsname)


# --------------------------------------------------------------------------------------------
# Extra data for hand-ported lexers
# --------------------------------------------------------------------------------------------


def data_cfamily(ex, cls):
    return {'sets': [sorted(cls.stdlib_types), sorted(cls.c99_types), sorted(cls.c11_atomic_types),
                     sorted(cls.linux_types)],
            'name': ex.tok(Token.Name), 'type': ex.tok(Token.Keyword.Type)}


def data_lua(ex, cls):
    inst = cls(**OPTIONS)
    return {'functions': sorted(inst._functions), 'builtin': ex.tok(Token.Name.Builtin),
            'name': ex.tok(Token.Name), 'punct': ex.tok(Token.Punctuation)}


def data_swift(ex, cls):
    from pygments.lexers._cocoa_builtins import COCOA_INTERFACES, COCOA_PROTOCOLS, COCOA_PRIMITIVES
    names = set(COCOA_INTERFACES) | set(COCOA_PROTOCOLS) | set(COCOA_PRIMITIVES)
    return {'names': sorted(names), 'match': [ex.tok(Token.Name), ex.tok(Token.Name.Class)],
            'to': ex.tok(Token.Name.Builtin.Pseudo)}


def data_php(ex, cls):
    inst = cls(**OPTIONS)
    # Note: the heredoc rule uses a case-insensitive backreference, which the Rust side guards
    # (see `php_guard` in okt-pygments/src/lexers/postprocess.rs).
    return {'functions': sorted(inst._functions), 'other': ex.tok(Token.Name.Other),
            'builtin': ex.tok(Token.Name.Builtin)}


def data_makefile(ex, cls):
    ex.want('BaseMakefileLexer')
    return {'r_special': ex.rx(cls.r_special.pattern, cls.r_special.flags, 'MakefileLexer'),
            'r_comment': ex.rx(cls.r_comment.pattern, cls.r_comment.flags, 'MakefileLexer'),
            'preproc': ex.tok(Token.Comment.Preproc), 'comment': ex.tok(Token.Comment)}


def data_json(ex, cls):
    from pygments.token import String, Whitespace, Keyword, Number, Punctuation, Comment, Error, Name
    assert cls.integers == set('-0123456789') and cls.floats == set('.eE+')
    assert cls.constants == set('truefalsenull') and cls.punctuations == set('{}[],')
    assert cls.hexadecimals == set('0123456789abcdefABCDEF')
    assert cls.whitespaces == {' ', '\n', '\r', '\t'}
    return {k: ex.tok(v) for k, v in {
        'string': String.Double, 'ws': Whitespace, 'constant': Keyword.Constant,
        'float': Number.Float, 'integer': Number.Integer, 'punct': Punctuation,
        'single': Comment.Single, 'multi': Comment.Multiline, 'error': Error, 'tag': Name.Tag,
    }.items()}


def data_text(ex, cls):
    return {'text': ex.tok(Token.Text)}


def data_yaml(ex, cls):
    return {'ws_error': ex.tok(Token.Text.Whitespace.Error)}


def data_ruby(ex, cls):
    from pygments.token import Operator, String, Error
    return {k: ex.tok(v) for k, v in {
        'operator': Operator, 'heredoc': String.Heredoc, 'delimiter': String.Delimiter,
        'error': Error, 'regex': String.Regex, 'other': String.Other,
    }.items()} | {'line_re': ex.rx(plexer.line_re.pattern, plexer.line_re.flags, 'RubyLexer')}


def data_rebol(ex, cls):
    # RebolLexer.word_callback is a chain of re.match(pattern, word) tests; transcribe it from
    # the source and verify the transcription against the real callback below.
    import inspect
    import ast
    src = inspect.getsource(cls.word_callback)
    src = '\n'.join(line[4:] for line in src.splitlines())
    tree = ast.parse(src)
    chain = []
    from pygments import token as tokmod

    def tokval(node):
        names = []
        while isinstance(node, ast.Attribute):
            names.append(node.attr)
            node = node.value
        names.append(node.id)
        names.reverse()
        t = getattr(tokmod, names[0])
        for n in names[1:]:
            t = getattr(t, n)
        return t

    node = tree.body[0].body[1]  # the if/elif chain
    while True:
        if isinstance(node, ast.If):
            test = node.test
            assert isinstance(test, ast.Call) and test.func.attr in ('match', 'search'), ast.dump(test)
            pat = ast.literal_eval(test.args[0])
            fn = test.func.attr
            yld = node.body[0].value
            assert isinstance(yld, ast.Yield)
            t = tokval(yld.value.elts[1])
            chain.append([fn, pat, t])
            if node.orelse and len(node.orelse) == 1 and isinstance(node.orelse[0], ast.If):
                node = node.orelse[0]
                continue
            if node.orelse:
                yld = node.orelse[0].value
                chain.append(['else', None, tokval(yld.value.elts[1])])
            break
        break
    out = []
    for fn, pat, t in chain:
        if fn == 'else':
            out.append(['else', None, ex.tok(t)])
        else:
            # re.match(p, word) == compiled.match; re.search == unanchored search.
            out.append([fn, ex.rx(pat, 0, 'RebolLexer.word_callback'), ex.tok(t)])
    return {'chain': out, 'chain_src': [[fn, pat, str(t)] for fn, pat, t in chain]}


EXTRA_DATA = {
    'CLexer': data_cfamily, 'CppLexer': data_cfamily, 'LuaLexer': data_lua,
    'SwiftLexer': data_swift, 'PhpLexer': data_php, 'MakefileLexer': data_makefile,
    'JsonLexer': data_json, 'TextLexer': data_text, 'YamlLexer': data_yaml,
    'RubyLexer': data_ruby, 'RebolLexer': data_rebol,
}

# --------------------------------------------------------------------------------------------
# Registry and CSS
# --------------------------------------------------------------------------------------------


def export_registry(ex):
    out = []
    for _key, (_mod, name, aliases, filenames, _mimes) in LEXERS.items():
        cls = find_lexer_class(name)
        globs = []
        for g in filenames:
            special = any(ch in g for ch in '*?[')
            if g.startswith('*') and not any(ch in g[1:] for ch in '*?['):
                globs.append([g, 'suffix', g[1:]])
            elif not special:
                globs.append([g, 'exact', g])
            else:
                import fnmatch
                globs.append([g, 'regex', ex.rx(fnmatch.translate(g), 0, f'glob {g}')])
        out.append([cls.__name__, list(aliases), globs, float(cls.priority)])
    return out


def all_token_types():
    seen = []
    stack = [Token]
    while stack:
        t = stack.pop()
        seen.append(t)
        stack.extend(sorted(t.subtypes))
    return seen


def main(path):
    ex = Exporter()
    ex.run()
    registry = export_registry(ex)
    for t in EXTRA_TOKENS:
        ex.tok(t)
    fmt = HtmlFormatter(nowrap=True)
    tokens = [['.'.join(('Token',) + tuple(t)) if t else 'Token', fmt._get_css_classes(t)]
              for t in ex.tokens]
    for w, where in sorted(ex.warnings.items()):
        print(f'warning: {w}: {len(where)} regexes, e.g. {where[0]}', file=sys.stderr)
    if ex.problems:
        print('PROBLEMS:', file=sys.stderr)
        for p in ex.problems:
            print('  ' + p, file=sys.stderr)
    data = {
        'pygments': pygments.__version__,
        'unicode': {k: [m, rs_without_surrogates(s)] for k, (m, s) in MARKERS.items()},
        'markers': {'wb': MARK_WB, 'nwb': MARK_NWB},
        'tokens': tokens,
        'regexes': ex.regexes,
        'lexers': ex.lexers,
        'registry': registry,
    }
    with open(path, 'w', encoding='utf-8') as fh:
        json.dump(data, fh, ensure_ascii=False, separators=(',', ':'))
    print(f'wrote {path}: {len(ex.lexers)} lexers, {len(ex.regexes)} regexes, '
          f'{len(ex.tokens)} token types', file=sys.stderr)
    return 1 if ex.problems else 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else 'pygments.json'))
