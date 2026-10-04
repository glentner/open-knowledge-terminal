#!/usr/bin/env python3
"""Compare the JSON API of okt with the reference implementation (bin/terminal.py), endpoint by
endpoint, over every file in the KB. Both servers must be serving the same KB.

    python3 tests/parity/api_diff.py --ref http://127.0.0.1:8376 --okt http://127.0.0.1:6580 [--only file]

Volatile fields (index version, build timings) are ignored; everything else must match exactly.
"""
import argparse
import difflib
import json
import sys
import urllib.parse
import urllib.request


def get(base, path):
    req = urllib.request.Request(base + path, headers={'Host': urllib.parse.urlsplit(base).netloc})
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return r.status, json.loads(r.read().decode())
    except urllib.error.HTTPError as e:
        return e.code, json.loads(e.read().decode() or '{}')


def norm_index(d):
    d = dict(d)
    d.pop('version', None)
    d.pop('built', None)
    d.pop('change', None)
    return d


def show_diff(a, b, label, limit=40):
    sa = json.dumps(a, indent=1, ensure_ascii=False, sort_keys=False).splitlines()
    sb = json.dumps(b, indent=1, ensure_ascii=False, sort_keys=False).splitlines()
    d = list(difflib.unified_diff(sa, sb, 'ref', 'okt', n=1, lineterm=''))
    print(f'=== {label}: {len(d)} diff lines')
    for line in d[:limit]:
        print('   ', line[:300])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--ref', default='http://127.0.0.1:8376')
    ap.add_argument('--okt', default='http://127.0.0.1:6580')
    ap.add_argument('--only', default='')
    ap.add_argument('--limit', type=int, default=40)
    ap.add_argument('--max-files', type=int, default=0)
    args = ap.parse_args()
    only = set(args.only.split(',')) if args.only else None
    fails = 0

    def check(label, path, norm=lambda x: x):
        nonlocal fails
        s1, a = get(args.ref, path)
        s2, b = get(args.okt, path)
        a, b = norm(a), norm(b)
        if s1 != s2 or a != b:
            fails += 1
            print(f'status ref={s1} okt={s2}' if s1 != s2 else '', end='')
            show_diff(a, b, f'{label} {path}', args.limit)
            return False
        return True

    _, idx = get(args.ref, '/api/index')
    files = [n['id'] for n in idx['nodes'] if n['t'] == 'file']
    dirs = [n['id'] for n in idx['nodes'] if n['t'] == 'dir']
    if args.max_files:
        files = files[:args.max_files]

    if not only or 'index' in only:
        check('index', '/api/index', norm_index)
    if not only or 'log' in only:
        check('log', '/api/log')
        for p in files[::7] + dirs:
            check('log', '/api/log?path=' + urllib.parse.quote(p))
    if not only or 'file' in only:
        ok = 0
        for p in files:
            if check('file', '/api/file?path=' + urllib.parse.quote(p)):
                ok += 1
        print(f'files: {ok}/{len(files)} identical')
    if not only or 'table' in only:
        for d in ['.'] + dirs:
            for rec in ('0', '1'):
                check('table', f'/api/table?folder={urllib.parse.quote(d)}&recursive={rec}')
    if not only or 'search' in only:
        for q in ['pearc', 'Geoffrey', 'TODO', 'slack', '/rcac-[a-z]+/', 'é', 'the']:
            if q.startswith('/'):
                check('search', '/api/search?re=1&q=' + urllib.parse.quote(q[1:-1]))
            else:
                check('search', '/api/search?q=' + urllib.parse.quote(q))
    if not only or 'blame' in only:
        for p in files[::25]:
            check('blame', '/api/blame?path=' + urllib.parse.quote(p))
    if not only or 'commit' in only:
        _, log = get(args.ref, '/api/log?limit=40')
        for c in log['commits'][:40]:
            check('commit', '/api/commit/' + c['h'])
    if not only or 'rev' in only:
        _, log = get(args.ref, '/api/log?limit=60')
        for c in log['commits'][5:60:6]:
            for f in c['f'][:2]:
                p = f[1]
                check('rev', f'/api/file?path={urllib.parse.quote(p)}&rev={c["h"]}')
    print('FAILURES:', fails)
    sys.exit(1 if fails else 0)


if __name__ == '__main__':
    main()
