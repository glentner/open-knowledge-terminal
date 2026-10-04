#!/usr/bin/env python3
"""Compare /raw/<path> (bytes and the headers that matter) and the static/PWA routes between the
reference and okt."""
import json
import sys
import urllib.parse
import urllib.request

REF, OKT = (sys.argv[1:3] + ['http://127.0.0.1:8376', 'http://127.0.0.1:6580'][len(sys.argv[1:3]):])[:2]
KEYS = ('Content-Type', 'Content-Security-Policy', 'Content-Disposition', 'Cache-Control', 'X-Content-Type-Options')


def fetch(base, path, enc=True):
    req = urllib.request.Request(base + path, headers={'Accept-Encoding': 'identity'})
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return r.status, {k: r.headers.get(k) for k in KEYS}, r.read()
    except urllib.error.HTTPError as e:
        return e.code, {k: e.headers.get(k) for k in KEYS}, e.read()


idx = json.loads(fetch(REF, '/api/index')[2])
files = [n['id'] for n in idx['nodes'] if n['t'] == 'file']
fails = 0
for p in files + ['missing/file.md', '../etc/passwd', '.git/config']:
    q = '/raw/' + urllib.parse.quote(p)
    a, b = fetch(REF, q), fetch(OKT, q)
    if a != b:
        fails += 1
        print('DIFF', q, a[0], b[0])
        for k in KEYS:
            if a[1][k] != b[1][k]:
                print('   ', k, repr(a[1][k]), '|', repr(b[1][k]))
        if a[2] != b[2]:
            print('    body differs', len(a[2]), len(b[2]), a[2][:80], b[2][:80])
print('raw files compared:', len(files) + 3, 'FAILURES:', fails)
