#!/usr/bin/env python3
"""Verify every guaranteed colour pair in tokens.json, in every theme.

Usage: python3 tools/check_contrast.py [path/to/tokens.json]
Exits 1 and lists failures if any pair drops below its ratio. Run it in CI
whenever tokens.json changes; fix the token, never relax the check.
"""
import json, sys

PATH = sys.argv[1] if len(sys.argv) > 1 else 'tokens/tokens.json'
T = json.load(open(PATH))
THEMES = [t['id'] for t in T['color']['themes']]
CT = {t['name']: t['value'] for t in T['color']['tokens']}
AAA_THEMES = {'cvd', 'hc'}

def raw(n, th):
    v = CT[n]
    return v if isinstance(v, str) else v.get(th, v[THEMES[0]])
def res(n, th):
    v = raw(n, th)
    while v.startswith('{'):
        v = raw(v[1:-1], th)
    return v
def rgba(h):
    h = h.lstrip('#'); a = int(h[6:8], 16) / 255 if len(h) == 8 else 1.0
    return [int(h[i:i + 2], 16) for i in (0, 2, 4)], a
def blend(fg, bg):
    (c, a), (b, _) = rgba(fg), rgba(bg)
    return '#' + ''.join('%02x' % round(c[i] * a + b[i] * (1 - a)) for i in range(3))
def lum(h):
    c, _ = rgba(h)
    f = lambda x: x / 12.92 if x <= .03928 else ((x + .055) / 1.055) ** 2.4
    r, g, b = [f(x / 255) for x in c]
    return .2126 * r + .7152 * g + .0722 * b
def ratio(a, b):
    x, y = sorted([lum(a), lum(b)], reverse=True)
    return (x + .05) / (y + .05)

GROUNDS = ['bg-void', 'bg-base', 'bg-panel', 'bg-raised', 'bg-field', 'bg-hover']
fails = []
def need(th, fg, bg, minimum, label):
    r = ratio(fg, bg)
    if r < minimum:
        fails.append(f'{th:9} {label:40} {r:5.2f} < {minimum}')

for th in THEMES:
    R = lambda n: res(n, th)
    text = 7 if th in AAA_THEMES else 4.5
    for ink in ['ink', 'ink-muted', 'ink-subtle']:
        for g in GROUNDS:
            need(th, R(ink), R(g), text, f'{ink} on {g}')
    for s in ['success', 'warning', 'danger', 'info', 'cyan', 'magenta']:
        soft = {'info': 'cyan-soft', 'cyan': 'cyan-soft', 'magenta': 'magenta-soft'}.get(s, s + '-soft')
        for g in ['bg-panel', 'bg-raised', soft]:
            need(th, R(s), R(g), text, f'{s} on {g}')
    need(th, R('on-cyan'), R('cyan'), text, 'on-cyan on cyan')
    for g in ['bg-panel', 'bg-raised', 'bg-field']:
        need(th, R('line-control'), R(g), 3, f'line-control on {g}')
    for i in range(1, 11):
        need(th, R(f'series-{i}'), R('plot-bg'), 3, f'series-{i} on plot-bg')
    need(th, R('plot-cursor'), R('plot-bg'), 3, 'plot-cursor on plot-bg')
    plate = R('cell-plate')
    for k, n in (('seq', 7), ('div', 5)):
        for i in range(n):
            fill = R(f'{k}-{i}')
            under = blend(plate, fill) if rgba(plate)[1] > 0 else fill
            need(th, R(f'on-{k}-{i}'), under, text, f'on-{k}-{i} on {k}-{i} (+plate)')

if fails:
    print('Contrast failures:'); print('\n'.join(fails)); sys.exit(1)
print(f'All pairs pass in {len(THEMES)} themes: {", ".join(THEMES)}')
