# Ultra intensity

One slider in Settings → Display, labelled **Ultra intensity**, 0-100, default 35. It lets people who find neon distracting turn it down to a clean dark tool, and lets people who want the full synthwave cockpit turn it all the way up. It is a single dial so it stays simple; the levels are names on the track, not separate settings.

| | Stealth · 0 | Night Drive · 35 (default) | Neon · 70 | Ultra · 100 |
| --- | --- | --- | --- | --- |
| Grounds (`bg-*`, `line-*`) | Neutral greys | Indigo-black | Toward purple | Deep purple |
| `cyan` accent | Softened `#5cc8dc` | `#2de2ff` | Interpolated | Hotter `#3df0ff` |
| Sunset redline (`redline-*`) | Flat grey hairline | Full gradient | Full gradient | Full gradient |
| `glow-cyan` (primary hover, active tab) | none | 16px soft | Wider | 22px + 48px halo |
| `focus-ring` | Solid ring | Solid ring | Solid ring | Solid ring + glow |
| Grid floor (`grid-floor`) on drop zone | Barely there | `line` | Magenta tint | Hot magenta |
| Series bloom (`bloom-series`) | 0 | 0 | 2px | 4px |
| Plot floor (`plot-floor`) | off | off | faint | Perspective grid behind data |
| Data colours, text, contrast | **unchanged** | **unchanged** | **unchanged** | **unchanged** |

## How it works

- `stealth`, `night` and `ultra` are anchor themes with the same token names. The app interpolates every colour between the two nearest anchors in OKLab, and every glow radius and alpha linearly, so any value of the slider is valid. Neon (70) sits just past the midpoint between Night Drive and Ultra.
- Text contrast is tested across the whole slider: a unit test sweeps intensity 0-100 (with and without Color blind mode) and fails if any `ink-*` / `bg-*` pair drops below its guaranteed ratio.
- Series, colour maps and status colours never change with intensity; the cursor follows the accent tint. Bloom is drawn *under* the line in the line's own colour, so the line itself stays crisp.

## Guard rails

- **Color blind mode on:** intensity still changes chrome, but `bloom-series` and `plot-floor` are forced off.
- **High contrast on:** the slider is disabled at its High contrast values (no glows, no gradients, no floor) and its caption says why.
- **Exports:** ignore intensity entirely; Paper and Graphite have their own fixed values.
- **Reduced motion / low power:** glows render without blur animation; on integrated GPUs Ultra may drop the plot floor if frame time exceeds budget.
