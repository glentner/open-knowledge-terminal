#!/usr/bin/env python3
"""Pixel-compare ref vs okt screenshots: per scenario, the share of differing pixels and the
bounding boxes of differing regions; writes <scenario>-diff.png (okt dimmed, differences red)."""
import sys
from pathlib import Path

import numpy as np
from PIL import Image

d = Path(sys.argv[1])
names = sorted({p.name.rsplit('-', 1)[0] for p in d.glob('*-ref.png')})
for n in names:
    a = np.asarray(Image.open(d / f'{n}-ref.png').convert('RGB')).astype(int)
    b = np.asarray(Image.open(d / f'{n}-okt.png').convert('RGB')).astype(int)
    if a.shape != b.shape:
        print(n, 'shape differs', a.shape, b.shape)
        continue
    diff = np.abs(a - b).sum(axis=2) > 30
    share = diff.mean() * 100
    # coarse regions: 40px tiles with any difference
    th, tw = 40, 40
    tiles = []
    H, W = diff.shape
    for y in range(0, H, th):
        for x in range(0, W, tw):
            c = diff[y:y + th, x:x + tw].sum()
            if c > 4:
                tiles.append((x, y, c))
    print(f'{n:<10} {share:6.2f}% differing; {len(tiles)} tiles', end='')
    if tiles:
        xs = [t[0] for t in tiles]; ys = [t[1] for t in tiles]
        print(f'  bbox x {min(xs)}-{max(xs) + tw} y {min(ys)}-{max(ys) + th}')
    else:
        print()
    out = (b * 0.35).astype(np.uint8)
    out[diff] = [255, 0, 0]
    Image.fromarray(out).save(d / f'{n}-diff.png')
