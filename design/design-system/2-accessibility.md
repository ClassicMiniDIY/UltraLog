# Accessibility

There is no data on which kinds of colour vision deficiency UltraLog users have, so the color-blind safe theme is built for all of them (protan, deutan and tritan) and to WCAG 2.2 **AAA** where AAA applies. High contrast goes further for low vision.

## Every theme (the floor)

- Text 4.5:1 on every ground; borders of controls, the focus ring, icons and data lines 3:1.
- Every status has an icon shape and a word: check, triangle, octagon, circle-i.
- Confidence is a fill style (filled / outlined / dashed), never a colour.
- Empty cells are hatched, not just dark.
- Sequential maps rise monotonically in lightness, so they read in greyscale.
- Hovering a legend entry solos that series; every chart can be explored by interaction as well as colour.
- Keyboard: every control reachable, `focus-ring` visible, no keyboard traps in floating windows (Esc closes).

## Color-blind safe (`cvd`), the Color blind mode setting

| Role | Night Drive | Color-blind safe |
| --- | --- | --- |
| Series 1-8 | Neon set | Okabe-Ito / Wong, lightened for a dark ground: sky blue, orange, bluish green, yellow, blue, vermillion, reddish purple, grey |
| Series 9-10 | Coral, lilac | Repeat series 1-2, **dashed 6/4** (swatches dashed too) |
| Series identity | Legend | Legend **plus direct labels** at each line's end; legend moves top-left |
| Cursor | `cyan` | `ink` (white), since series-1 is sky blue |
| Sequential map | Sunset | Cividis-style blue → yellow |
| Diverging map | Cyan ↔ orange | Blue ↔ amber |
| Cell values | Text straight on the fill | Text on a solid `cell-plate`, 7:1+ on every stop |
| Text | AA | **AAA**: `ink-muted` and `ink-subtle` raised to 7:1+ on every ground |
| Success / danger | Green / red | Sky blue / vermillion-orange, off the red-green axis |
| Warning / info | Amber / cyan | Yellow / lavender (info never reads as success) |
| Bloom, plot floor | Follow intensity | Forced off |

Okabe-Ito is distinguishable for all three common deficiency types; the dashes and direct labels cover the cases where two lines still converge, and full achromatopsia.

## High contrast (`hc`)

- True black grounds, white `ink`, `ink-muted` and `ink-subtle` all 7:1+ (most 12:1+).
- Surfaces separated by borders (`line` 6:1, `line-control` 12:1), not by shade.
- Focus ring 5px.
- Uses the color-blind safe data palette, brightened, with direct labels and value plates.
- All intensity effects off; the Ultra intensity slider is disabled with an explanation.

## Checks

The token build verifies every pair above in every theme. A token change that breaks a pair fails the build; fix the token, don't relax the check.
