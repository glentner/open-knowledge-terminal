#!/usr/bin/env python3
"""Compare okt-pygments with the reference `highlight_lines` (terminal.py, Pygments 2.21.0).

Run with a Python that has the reference's dependencies, after building the CLI::

    cargo build --release -p okt-pygments --example hl
    uv run --python 3.13 --with pygments==2.21.0 --with markdown-it-py --with mdit-py-plugins \\
        --with linkify-it-py --with pyyaml --with nh3 python tools/pygments/check.py \\
        [--kb] [--examples DIR] [--snippets DIR] [--synthetic] [--hl PATH] [-v]

The reference is $OKT_KB/bin/terminal.py (OKT_KB defaults to ~/KB).

Corpora:
  --kb          every file of the KB git repo that the reference highlights (its own `classify`
                and the rule in its file view: code/html kinds, plus .json/.jsonl/.geojson/.yaml/
                .yml/.toml text; .jsonl with lang='json')
  --examples    Pygments' tests/examplefiles/<alias>/* (lang=<alias>) for supported lexers
  --snippets    Pygments' tests/snippets/<alias>/*.txt input sections (lang=<alias>)
  --synthetic   edge cases: CRLF, CR, BOM, tabs, empty files, no trailing newline, unmatched
                characters, non-ASCII identifiers, very long lines, ...
  --files LIST  arbitrary files (one path per line), lexer chosen from the file name
  --cross       additionally run every case through every supported lexer (lang=<alias>),
                which exercises error recovery and odd input
  --selection   compare only the lexer selection, for all ~600 Pygments lexers: names made
                from every registry glob, 20000 real file names, #! lines, every alias
  --fuzz N      additionally N mutated variants of the collected cases (random insertions of
                syntax-significant snippets, deletions, duplications), each through its own
                lexer and one random other supported lexer (--seed for reproducibility)

Prints per-lexer pass counts and, for each failure, the first differing line.
"""

import argparse
import collections
import importlib.util
import json
import os
import subprocess
import sys
import signal
import tempfile
import time

HERE = os.path.dirname(os.path.abspath(__file__))
OKT = os.path.dirname(os.path.dirname(HERE))
KB = os.environ.get('OKT_KB', os.path.expanduser('~/KB'))
TERMINAL = os.path.join(KB, 'bin', 'terminal.py')
DEFAULT_HL = os.path.join(OKT, 'target', 'release', 'examples', 'hl')


def load_reference():
    spec = importlib.util.spec_from_file_location('terminal', TERMINAL)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


T = load_reference()

from pygments.lexers import get_lexer_by_name, get_lexer_for_filename  # noqa: E402
from pygments.util import ClassNotFound  # noqa: E402


def supported():
    with open(os.path.join(OKT, 'okt-pygments/data/pygments.json'), encoding='utf-8') as fh:
        return set(json.load(fh)['lexers'])


def lexer_class(text, path, lang):
    """Which lexer the reference picks (mirrors highlight_lines)."""
    try:
        if lang:
            return type(get_lexer_by_name(lang)).__name__
        name = path.rsplit('/', 1)[-1]
        try:
            return type(get_lexer_for_filename(name)).__name__
        except ClassNotFound:
            first = text.split('\n', 1)[0]
            if first.startswith('#!'):
                for key, lx in (('python', 'python'), ('node', 'javascript'), ('bash', 'bash'),
                                ('/sh', 'bash'), ('zsh', 'bash'), ('ruby', 'ruby'), ('perl', 'perl'),
                                ('awk', 'awk')):
                    if key in first:
                        return type(get_lexer_by_name(lx)).__name__
    except ClassNotFound:
        pass
    return '(none)'


class RefTimeout(Exception):
    pass


class Case:
    __slots__ = ('corpus', 'file', 'path', 'lang', 'text')

    def __init__(self, corpus, file, path, lang):
        self.corpus, self.file, self.path, self.lang = corpus, file, path, lang
        with open(file, 'rb') as fh:
            data = fh.read(T.MAX_RENDER_BYTES + 1)
        self.text = data[:T.MAX_RENDER_BYTES].decode('utf-8', 'replace')


def kb_cases():
    files = subprocess.run(['git', '-C', KB, 'ls-files', '-z'], capture_output=True, check=True).stdout
    out = []
    for rel in files.decode().split('\0'):
        if not rel:
            continue
        full = os.path.join(KB, rel)
        if os.path.islink(full) or not os.path.isfile(full):
            continue
        with open(full, 'rb') as fh:
            head = fh.read(8192)
        kind = T.classify(rel, head, False)
        ext = T.ext_of(rel)
        if kind in ('code', 'html') or (kind == 'text' and ext in T.STRUCTURED_EXT):
            out.append(Case('kb', full, rel, 'json' if ext == '.jsonl' else None))
    return out


def example_cases(root, sup):
    out = []
    for alias in sorted(os.listdir(root)):
        d = os.path.join(root, alias)
        if not os.path.isdir(d):
            continue
        try:
            cls = type(get_lexer_by_name(alias)).__name__
        except ClassNotFound:
            continue
        if cls not in sup:
            continue
        for dirpath, _dirs, files in os.walk(d):
            for f in sorted(files):
                if f.endswith('.output'):
                    continue
                full = os.path.join(dirpath, f)
                out.append(Case('examples', full, full, alias))
    return out


def snippet_cases(root, sup, tmp):
    out = []
    for alias in sorted(os.listdir(root)):
        d = os.path.join(root, alias)
        if not os.path.isdir(d):
            continue
        try:
            cls = type(get_lexer_by_name(alias)).__name__
        except ClassNotFound:
            continue
        if cls not in sup:
            continue
        for f in sorted(os.listdir(d)):
            if not f.endswith('.txt'):
                continue
            with open(os.path.join(d, f), encoding='utf-8') as fh:
                content = fh.read()
            # Pygments' snippet format: ---input--- ... ---tokens--- ...
            if '---input---\n' not in content:
                continue
            inp = content.split('---input---\n', 1)[1].split('\n---tokens---\n', 1)[0]
            target = os.path.join(tmp, f'snippet-{alias}-{f}')
            with open(target, 'w', encoding='utf-8', newline='') as fh:
                fh.write(inp)
            out.append(Case('snippets', target, f'{alias}/{f}', alias))
    return out


SYNTHETIC = {
    'empty.py': '',
    'nl.py': '\n',
    'nonl.py': 'x = 1',
    'crlf.py': 'def f():\r\n    return "a"\r\n',
    'cr.py': 'a = 1\rb = 2\r',
    'mixed.sh': 'echo a\r\necho b\recho c\n',
    'bom.py': '\ufeffimport os\nprint(os)\n',
    'bom.json': '\ufeff{"a": [1, 2.5e3, true, null]}\n',
    'tabs.py': 'if x:\n\tif y:\n\t\treturn\t# c\n',
    'tabs.mk': 'all:\n\techo "$(CC)"\n\t@ls\n',
    'error.py': 'x = $ ? ` ¿\n',
    'error.json': '{"a": @, ~}\n{"b": "unterminated\n',
    'unicode.py': 'café = "naïve"\nπ = 3.14\n名前 = 1\nx² = 2\nä̈ = 3\n',
    'unicode.js': 'const café = "ünïcödé"; // ☃\nlet ℮ = 1;\n',
    'unicode.go': 'package main\nvar 世界 = "hello"\n',
    'long.py': 'x = [' + ', '.join(str(i) for i in range(20000)) + ']\n',
    'longstr.js': 'var s = "' + 'ab\\"c' * 20000 + '";\n',
    'deep.json': '[' * 2000 + ']' * 2000 + '\n',
    'nul.py': 'a = "\\x00"\n',
    'html.html': '<!DOCTYPE html>\n<html><head><style>\nb { color: red }\n</style>\n'
                 '<script>var a = 1 < 2 && "x";</script></head>\n<body onload="f()">&amp; é</body></html>',
    'yaml.yaml': 'a: 1\nb:\n  - c\n  - d: |\n      text\n       more\n\n    e\nf: "q\\"x"\n# c\n---\n...\n',
    'toml.toml': '[a]\nb = 1\nc = "x"\n[[d]]\ne = 1979-05-27T07:32:00Z\n',
    'shebang': '#!/usr/bin/env python3\nprint("hi")\n',
    'shebang-node': '#!/usr/bin/env node\nconsole.log(1)\n',
    'shebang-sh': '#!/bin/sh\necho $HOME\n',
    'Makefile': 'CC ?= gcc\n.PHONY: all\nifeq ($(X),1)\nY = 2\nendif\nall: a.o \\\n  b.o\n\t$(CC) -o $@ $^\n# done\n',
    'Dockerfile': 'FROM python:3.12 AS base\nRUN apt-get update && \\\n    apt-get install -y git\nCMD ["python", "-m", "x"]\n',
    'trailing-ws.py': 'x = 1   \n\n   \n',
    'only-ws.sh': '   \n\t\n',
    'weird.rb': "x = <<~EOS\n  hi #{name}\nEOS\nputs %w[a b]\n",
    'weird.lua': 'local t = {1,2}\nprint(string.format("%d", #t))\n--[[ c ]]\n',
    'weird.c': '#include <stdio.h>\nint main(void) { size_t n = 0; return (int)n; }\n',
    'weird.php': '<?php\necho strlen("x");\n$a = <<<EOT\nhi\nEOT;\n?>\n<b>html</b>\n',
    'weird.r': 'x <- c(1, 2)\nprint(x)\n',
    'weird.R': 'x <- c(1, 2)\nprint(x)\n',
    'weird.pl': 'foo(X) :- bar(X).\n',
    'weird.jsonl': '{"a": 1}\n{"b": [2]}\n',
}


def synthetic_cases(tmp):
    out = []
    d = os.path.join(tmp, 'synthetic')
    os.makedirs(d, exist_ok=True)
    for name, content in SYNTHETIC.items():
        p = os.path.join(d, name)
        with open(p, 'w', encoding='utf-8', newline='') as fh:
            fh.write(content)
        lang = 'json' if name.endswith('.jsonl') else None
        out.append(Case('synthetic', p, name, lang))
    # invalid UTF-8 (decoded with 'replace' by the reference)
    p = os.path.join(d, 'invalid.py')
    with open(p, 'wb') as fh:
        fh.write(b'x = "\xff\xfe\xc3("\n# \xe2\x82\n')
    out.append(Case('synthetic', p, 'invalid.py', None))
    return out


def file_list_cases(listfile, sup):
    out = []
    with open(listfile, encoding='utf-8') as fh:
        for line in fh:
            p = line.rstrip('\n')
            if not p or not os.path.isfile(p):
                continue
            c = Case('files', p, p, None)
            if lexer_class(c.text, c.path, None) in sup:
                out.append(c)
    return out


def cross_cases(cases, sup):
    """Every case again, through every supported lexer (one alias each)."""
    from pygments.lexers._mapping import LEXERS
    aliases = []
    for _mod, _name, al, _f, _m in LEXERS.values():
        if al:
            try:
                cls = type(get_lexer_by_name(al[0])).__name__
            except ClassNotFound:
                continue
            if cls in sup:
                aliases.append(al[0])
    out = []
    for c in cases:
        for a in aliases:
            n = Case.__new__(Case)
            n.corpus, n.file, n.path, n.lang, n.text = 'cross', c.file, c.path, a, c.text
            out.append(n)
    return out


FUZZ_SNIPPETS = [
    '"', "'", '`', '\\', '\n', '\r', '\r\n', '\t', ' ', '#', '//', '/*', '*/', '<!--', '-->', '<?php',
    '?>', '"' * 3, "'" * 3, '${', '#{', '}', '{', '(', ')', '[', ']', '<<EOF\n', '<<-EOS\n', 'EOF\n',
    '<', '>', '&', ';', ':', ',', '.', '=', '%', '@', '$', '!', '?', '|', '^', '~', '*', '+', '-',
    '0x', '1e10', '.5', '__init__', 'if ', 'def ', 'class ', 'end', 'begin', '---\n', '...\n',
    '- ', ': ', '|\n', '>-\n', '&anchor ', '*alias', '!tag ', 'é', 'é', '²',
    '٣', ' ', ' ', '　', '\x0b', '\x0c', '\x1c', '\x00', '�',
    '\U0001F600', '世界', 'İ', 'ı', 'K', 'ſ', 'r"', "b'",
    'f"{', '\\(', '\\n', '\\u12', '<script>', '</script>', '<style>', '</style>', '<![CDATA[',
]


def mutate(text, rnd):
    text = text[:4000]
    for _ in range(rnd.randint(1, 6)):
        op = rnd.random()
        pos = rnd.randint(0, len(text))
        if op < 0.6:
            text = text[:pos] + rnd.choice(FUZZ_SNIPPETS) + text[pos:]
        elif op < 0.8 and text:
            end = min(len(text), pos + rnd.randint(1, 40))
            text = text[:pos] + text[end:]
        elif text:
            end = min(len(text), pos + rnd.randint(1, 80))
            text = text[:pos] + text[pos:end] * 2 + text[end:]
    return text


def supported_aliases(sup):
    from pygments.lexers._mapping import LEXERS
    aliases = []
    for _mod, _name, al, _f, _m in LEXERS.values():
        if al:
            try:
                if type(get_lexer_by_name(al[0])).__name__ in sup:
                    aliases.append(al[0])
            except ClassNotFound:
                pass
    return aliases


def fuzz_cases(cases, n, seed, sup, tmp):
    import random
    rnd = random.Random(seed)
    aliases = supported_aliases(sup)
    d = os.path.join(tmp, 'fuzz')
    os.makedirs(d, exist_ok=True)
    out = []
    for i in range(n):
        src = rnd.choice(cases)
        text = mutate(src.text, rnd)
        p = os.path.join(d, f'{i}')
        with open(p, 'w', encoding='utf-8', newline='') as fh:
            fh.write(text)
        for lang in (src.lang, rnd.choice(aliases)):
            c = Case.__new__(Case)
            c.corpus, c.file, c.path, c.lang, c.text = 'fuzz', p, src.path, lang, text
            out.append(c)
    return out


def glob_examples(glob):
    """A few file names matching an fnmatch glob."""
    out = set()
    for star in ('', 'x', 'a.b-c'):
        name, i = '', 0
        while i < len(glob):
            ch = glob[i]
            if ch == '*':
                name += star
            elif ch == '?':
                name += 'q'
            elif ch == '[':
                j = glob.find(']', i + 2)
                if j < 0:
                    name += ch
                else:
                    body = glob[i + 1:j]
                    if body.startswith('!'):
                        name += 'Z' if 'Z' not in body else 'Y'
                    else:
                        name += body[0]
                    i = j
            else:
                name += ch
            i += 1
        out.add(name)
    return out


def selection_check(hl, tmp):
    """Compare lexer selection (all ~600 lexers, by file name, #! line and alias)."""
    import random
    from pygments.lexers._mapping import LEXERS
    names = set()
    for _mod, _name, aliases, filenames, _m in LEXERS.values():
        for g in filenames:
            names |= glob_examples(g)
            names |= {n.upper() for n in glob_examples(g)}
    real = set()
    for root, dirs, files in os.walk('/usr'):
        real.update(files)
    rnd = random.Random(0)
    names |= set(rnd.sample(sorted(real), min(20000, len(real))))
    rows = []   # (text, path, lang)
    for n in sorted(names):
        rows.append(('', n, None))
    shebangs = ['#!/usr/bin/env python3', '#!/usr/bin/node', '#!/bin/bash', '#!/bin/sh', '#!/bin/zsh',
                '#!/usr/bin/ruby', '#!/usr/bin/perl -w', '#!/usr/bin/awk -f', '#!/usr/bin/tclsh',
                '#! /bin/sh', '#!/usr/bin/env -S deno run', 'no shebang', '#!pythonperl']
    for sb in shebangs:
        for n in ('script', 'x.unknownext', 'x.txt', 'x.conf', 'Justfile', 'x.json'):
            rows.append((sb + '\nbody\n', n, None))
    for _mod, _name, aliases, _f, _m in LEXERS.values():
        for a in aliases:
            rows.append(('', 'x.py', a))
            rows.append(('', 'x.py', a.upper()))
    for a in ('', 'nonexistent', 'JSON', 'Python3', 'py3'):
        rows.append(('', 'x.py', a))
    d = os.path.join(tmp, 'sel')
    os.makedirs(d, exist_ok=True)
    lines, files = [], {}
    for text, path, lang in rows:
        if text not in files:
            files[text] = os.path.join(d, str(len(files)))
            with open(files[text], 'w', encoding='utf-8') as fh:
                fh.write(text)
        lines.append(f'{files[text]}\t{path}\t{lang or ""}')
    proc = subprocess.run([hl, '--batch', '--which'], input='\n'.join(lines) + '\n',
                          capture_output=True, text=True, check=True)
    got = [json.loads(x) for x in proc.stdout.split('\n') if x]
    bad = 0
    for (text, path, lang), g in zip(rows, got):
        want = lexer_class(text, path, lang)
        want = None if want == '(none)' else want
        if want != g:
            bad += 1
            if bad <= 20:
                print(f'  selection differs for path={path!r} lang={lang!r}: want {want} got {g}')
    print(f'selection: {len(rows) - bad}/{len(rows)} identical')
    return bad


def run_rust(hl, cases):
    """Run the CLI once in batch mode; returns one parsed result per case."""
    with tempfile.TemporaryDirectory() as tmp:
        lines = []
        for c in cases:
            src = c.file
            if os.path.getsize(src) > T.MAX_RENDER_BYTES:
                src = os.path.join(tmp, str(len(lines)))
                with open(src, 'wb') as fh:
                    fh.write(c.text.encode('utf-8'))
            lines.append(f'{src}\t{c.path}\t{c.lang or ""}')
        proc = subprocess.run([hl, '--batch'], input='\n'.join(lines) + '\n', capture_output=True,
                              text=True, check=True)
    # split on '\n' only: raw U+2028 / U+0085 may appear inside JSON strings
    results = [json.loads(line) for line in proc.stdout.split('\n') if line]
    assert len(results) == len(cases), (len(results), len(cases))
    return results


def first_diff(a, b):
    for i, (x, y) in enumerate(zip(a, b)):
        if x != y:
            return i, x, y
    return min(len(a), len(b)), (a[len(b)] if len(a) > len(b) else '<end>'), \
        (b[len(a)] if len(b) > len(a) else '<end>')


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--kb', action='store_true')
    ap.add_argument('--examples')
    ap.add_argument('--snippets')
    ap.add_argument('--synthetic', action='store_true')
    ap.add_argument('--files')
    ap.add_argument('--cross', action='store_true')
    ap.add_argument('--fuzz', type=int, default=0)
    ap.add_argument('--selection', action='store_true',
                    help='compare lexer selection for all Pygments lexers instead of output')
    ap.add_argument('--seed', type=int, default=0)
    ap.add_argument('--hl', default=DEFAULT_HL)
    ap.add_argument('-v', '--verbose', action='store_true')
    ap.add_argument('--only', help='only lexers with this class name (comma separated)')
    ap.add_argument('--timeout', type=int, default=20,
                    help='seconds before giving up on the reference for one case')
    args = ap.parse_args()
    sup = supported()
    if args.selection:
        with tempfile.TemporaryDirectory() as tmp:
            return 1 if selection_check(args.hl, tmp) else 0

    with tempfile.TemporaryDirectory() as tmp:
        cases = []
        if args.kb:
            cases += kb_cases()
        if args.examples:
            cases += example_cases(args.examples, sup)
        if args.snippets:
            cases += snippet_cases(args.snippets, sup, tmp)
        if args.synthetic:
            cases += synthetic_cases(tmp)
        if args.files:
            cases += file_list_cases(args.files, sup)
        if args.cross:
            cases += cross_cases(list(cases), sup)
        if args.fuzz:
            base = [c for c in cases if c.corpus != 'cross']
            cases += fuzz_cases(base, args.fuzz, args.seed, sup, tmp)
        if args.only:
            only = set(args.only.split(','))
            cases = [c for c in cases if lexer_class(c.text, c.path, c.lang) in only]
        return compare_all(args, sup, cases)


def chunked(cases, results_of, size=200):
    """Yield (case, rust result) pairs, running the CLI on a few cases at a time."""
    for i in range(0, len(cases), size):
        chunk = cases[i:i + size]
        yield from zip(chunk, results_of(chunk))


def compare_all(args, sup, cases):
    stats = collections.defaultdict(lambda: collections.Counter())
    failures = []
    timeouts = []
    refused = []

    def on_alarm(_sig, _frame):
        raise RefTimeout()

    signal.signal(signal.SIGALRM, on_alarm)
    t0 = time.time()
    rust_time = 0.0

    def results_of(chunk):
        nonlocal rust_time
        t = time.time()
        res = run_rust(args.hl, chunk)
        rust_time += time.time() - t
        return res

    for n, (c, got) in enumerate(chunked(cases, results_of)):
        if args.verbose and n % 500 == 0:
            print(f'{n}/{len(cases)} ({time.time() - t0:.0f}s, rust {rust_time:.1f}s)', file=sys.stderr)
        cls = lexer_class(c.text, c.path, c.lang)
        key = (c.corpus, cls)
        signal.alarm(args.timeout)
        try:
            want = T.highlight_lines(c.text, c.path, c.lang)
        except RefTimeout:
            stats[key]['ref timeout'] += 1
            timeouts.append((c, cls, isinstance(got, dict)))
            continue
        except Exception as e:  # the reference itself fails
            want = None
            err = f'{type(e).__name__}: {e}'
        finally:
            signal.alarm(0)
        if isinstance(got, dict) and 'unsupported' in got:
            if want is None:
                stats[key]['ok (both fail)'] += 1
            elif cls in sup:
                # a supported lexer that declined this input (it could not guarantee identical
                # output, e.g. the PHP heredoc guard): not a mismatch, but listed
                stats[key]['refused'] += 1
                refused.append((c, cls))
            else:
                stats[key]['unsupported'] += 1
            continue
        if want is None:
            stats[key]['FAIL'] += 1
            failures.append((c, cls, f'reference raised {err}', None))
            continue
        if got == want:
            stats[key]['ok'] += 1
        else:
            stats[key]['FAIL'] += 1
            failures.append((c, cls, 'differs', first_diff(want, got)))

    print(f'rust: {len(cases)} cases in {rust_time:.1f}s', file=sys.stderr)
    by_corpus = collections.defaultdict(collections.Counter)
    print(f'{"corpus":10} {"lexer":28} {"ok":>6} {"fail":>6} {"unsup":>6} {"refused":>7}')
    for (corpus, cls), cnt in sorted(stats.items()):
        print(f'{corpus:10} {cls:28} {cnt["ok"] + cnt["ok (both fail)"]:6} {cnt["FAIL"]:6} '
              f'{cnt["unsupported"]:6} {cnt["refused"]:7}'
              + (f'  ({cnt["ref timeout"]} ref timeouts)' if cnt['ref timeout'] else ''))
        by_corpus[corpus].update(cnt)
    print()
    for corpus, cnt in by_corpus.items():
        ok = cnt['ok'] + cnt['ok (both fail)']
        total = sum(cnt.values())
        sup_total = total - cnt['unsupported'] - cnt['ref timeout'] - cnt['refused']
        print(f'{corpus}: {ok}/{sup_total} identical ({cnt["unsupported"]} unsupported-lexer files skipped, '
              f'{cnt["refused"]} refused)')
    if refused:
        print(f'\n{len(refused)} cases a supported lexer declined (returned Unsupported):')
        for c, cls in refused:
            print(f'  [{c.corpus}] {cls} {c.file} (lang={c.lang})')
    if timeouts:
        print(f'\n{len(timeouts)} cases where the reference did not finish within {args.timeout}s:')
        for c, cls, gave_up in timeouts:
            what = 'gave up' if gave_up else 'produced output'
            print(f'  [{c.corpus}] {cls} {c.file} (lang={c.lang}); rust {what}')
    if failures:
        print(f'\n{len(failures)} failures:')
        for c, cls, why, diff in failures:
            print(f'  [{c.corpus}] {cls} {c.file}: {why}')
            if diff:
                i, want, got = diff
                print(f'    line {i + 1}:\n      want {want[:300]!r}\n      got  {got[:300]!r}')
    return 1 if failures else 0


if __name__ == '__main__':
    sys.exit(main())
