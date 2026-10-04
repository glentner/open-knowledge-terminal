#!/usr/bin/env python3
"""Drive file-system and git changes in a scratch clone of the KB while both servers watch it, and
compare what they publish (SSE events) and serve afterwards.

    python3 tests/parity/watch_diff.py --kb /path/to/clone --ref http://127.0.0.1:8378 --okt http://127.0.0.1:6581
"""
import argparse
import json
import subprocess
import threading
import time
import urllib.request
from pathlib import Path


def get(base, path):
    with urllib.request.urlopen(base + path, timeout=60) as r:
        return json.loads(r.read().decode())


class Stream(threading.Thread):
    def __init__(self, base):
        super().__init__(daemon=True)
        self.base, self.events = base, []

    def run(self):
        r = urllib.request.urlopen(self.base + '/api/events', timeout=3600)
        kind = None
        for raw in r:
            line = raw.decode().rstrip('\n')
            if line.startswith('event: '):
                kind = line[7:]
            elif line.startswith('data: '):
                self.events.append((time.time(), kind, json.loads(line[6:])))


def norm_event(kind, d):
    d = dict(d)
    for k in ('version', 'ms', 'started', 'build', 'interval', 'watch', 't'):
        d.pop(k, None)
    return kind, d


def norm_index(d):
    d = dict(d)
    for k in ('version', 'built', 'change'):
        d.pop(k, None)
    return d


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--kb', required=True)
    ap.add_argument('--ref', default='http://127.0.0.1:8378')
    ap.add_argument('--okt', default='http://127.0.0.1:6581')
    ap.add_argument('--settle', type=float, default=5.0)
    args = ap.parse_args()
    kb = Path(args.kb)
    git = lambda *a: subprocess.run(['git', '-C', str(kb), '-c', 'user.name=T', '-c', 'user.email=t@x', *a],
                                    check=True, capture_output=True, text=True).stdout
    streams = {n: Stream(b) for n, b in (('ref', args.ref), ('okt', args.okt))}
    for s in streams.values():
        s.start()
    time.sleep(1.0)
    fails = 0

    def step(label, action):
        nonlocal fails
        marks = {n: len(s.events) for n, s in streams.items()}
        t0 = time.time()
        action()
        time.sleep(args.settle)
        evs = {n: [norm_event(k, d) for _, k, d in s.events[marks[n]:] if k != 'ping'] for n, s in streams.items()}
        lat = {n: [round(t - t0, 2) for t, k, _ in s.events[marks[n]:] if k in ('index', 'commit')] for n, s in streams.items()}
        a, b = norm_index(get(args.ref, '/api/index')), norm_index(get(args.okt, '/api/index'))
        same_ev, same_idx = evs['ref'] == evs['okt'], a == b
        print(f'{label:<34} events {"same" if same_ev else "DIFF"}  index {"same" if same_idx else "DIFF"}  latency ref={lat["ref"]} okt={lat["okt"]}')
        if not same_ev:
            fails += 1
            print('   ref:', json.dumps(evs['ref'])[:1500])
            print('   okt:', json.dumps(evs['okt'])[:1500])
        if not same_idx:
            fails += 1
            for k in a:
                if a[k] != b.get(k):
                    print(f'   index[{k}] differs: ref {str(a[k])[:300]} / okt {str(b.get(k))[:300]}')

    some = sorted(p for p in kb.rglob('*.md') if '.git' not in p.parts)
    target, other = some[3], some[7]
    rel = lambda p: str(p.relative_to(kb))

    step('edit: append a link', lambda: target.write_text(target.read_text() + f'\n\nSee [other]({Path("/").joinpath(rel(other))}).\n'.replace('(/', '(../' * (len(Path(rel(target)).parts) - 1) or '(')))
    step('create: new note with links', lambda: (kb / 'idea').mkdir(exist_ok=True) or (kb / 'idea' / 'zz-watch-test.md').write_text(
        '---\ntitle: Watch test\ntype: idea\n---\n# Watch test\n\nLinks to [[' + other.stem + ']] and [a broken one](nowhere.md).\n'))
    step('edit: twice quickly', lambda: [(kb / 'idea' / 'zz-watch-test.md').write_text(f'# Watch test {i}\n') or time.sleep(0.05) for i in range(3)])
    step('rename a file', lambda: (kb / 'idea' / 'zz-watch-test.md').rename(kb / 'idea' / 'zz-watch-test-2.md'))
    step('stage (status only)', lambda: git('add', '-A'))
    step('commit', lambda: git('commit', '-q', '-m', '[idea] watch test'))
    step('delete + commit', lambda: (kb / 'idea' / 'zz-watch-test-2.md').unlink() or git('commit', '-qam', '[idea] remove watch test'))
    step('new folder with files', lambda: (kb / 'zz-folder').mkdir() or [(kb / 'zz-folder' / f'n{i}.md').write_text(f'# N{i}\n\n[[n{(i + 1) % 3}]]\n') for i in range(3)])
    step('remove folder files', lambda: [(kb / 'zz-folder' / f'n{i}.md').unlink() for i in range(3)])
    step('reset to HEAD~2 (history rewrite)', lambda: git('reset', '-q', '--hard', 'HEAD~2'))
    print('FAILURES:', fails)


if __name__ == '__main__':
    main()
