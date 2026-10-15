#!/usr/bin/env python3
"""Side-by-side table of two tests/perf/profile.mjs runs.

    node tests/perf/profile.mjs http://127.0.0.1:6580/ before
    node tests/perf/profile.mjs http://127.0.0.1:6581/ after
    python3 tests/perf/compare.py results-before.json results-after.json

Lower is better everywhere: main-thread busy time, frame interval percentiles (16.7 ms is a
locked 60 fps), frames over 33 ms, long-task time, and input events that took longer than 16 ms.
"""

import json
import sys

COLS = [
    ('main_busy_ms', 'busy ms'),
    ('frame_p50', 'frame p50'),
    ('frame_p95', 'frame p95'),
    ('over33', '>33ms'),
    ('longtask_ms', 'longtask ms'),
    ('slow_events', 'slow ev'),
    ('event_max', 'ev max'),
]


def main(a_path, b_path):
    a = {r['scenario']: r for r in json.load(open(a_path))}
    b = {r['scenario']: r for r in json.load(open(b_path))}
    names = [n for n in a if n in b] + [n for n in b if n not in a]
    head = f"{'scenario':<14}" + ''.join(f'{label:>22}' for _, label in COLS)
    print(head)
    print(f"{'':<14}" + ''.join(f"{'a':>11}{'b':>11}" for _ in COLS))
    for n in names:
        ra, rb = a.get(n, {}), b.get(n, {})
        row = f'{n:<14}'
        for key, _ in COLS:
            row += f"{ra.get(key, '-'):>11}{rb.get(key, '-'):>11}"
        print(row)
    draws = [(n, b[n]) for n in names if n in b and b[n].get('draws')]
    if draws:
        print('\ncanvas draw() in b, ms (p50 / p95 / max):')
        for n, r in draws:
            print(f"  {n:<14}{r['draw_p50']:>8}{r['draw_p95']:>8}{r['draw_max']:>8}  ({r['draws']} draws)")


if __name__ == '__main__':
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    main(sys.argv[1], sys.argv[2])
