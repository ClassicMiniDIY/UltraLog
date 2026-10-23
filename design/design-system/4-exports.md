# Exports

PNG and PDF exports leave the app: they get printed, emailed to a customer, posted to a forum, pasted into a tune log. They follow their own rules.

- **Paper is the default.** White page, ink type, `ultralog-lockup-paper`. Every token has a `paper` value.
- **Graphite is the dark option.** A mid-grey page (`#24222c` plot, `#2a2833` page) instead of black: still reads as "dark mode" on screen, uses far less toner or ink than a black page, and keeps a white margin in PDFs so a printer never floods the sheet edge-to-edge.
- **Always color-blind safe.** Both export themes use the Okabe-Ito series, cividis maps and the blue/orange diverging map whatever the user's in-app setting, with series 9-10 dashed. The reader's colour vision is unknown.
- **No intensity effects.** No glow, bloom or grid floor in exports; the sunset redline along the top edge is the only brand flourish.
- **Paper maps run light to dark.** On paper, low values are pale (`seq-0` cream) and high values dark (`seq-6` navy), so sparse tables cost almost no ink.

## Layout (ExportSheet)

1. Redline across the top edge.
2. Lockup, then the title in `headline` (default: date + ECU + file name; editable).
3. Time range and record/channel counts, right-aligned in mono.
4. The chart or table at full width; plot frame in `plot-grid-major`.
5. Legend with every series (swatch + name + unit), or the ColorScale for maps.
6. Footer: ultralog.co · version · export date, in `data-sm`.

PNG exports are the same layout at 2× for crisp screenshots; "Copy for paste" copies a PNG of the chart only, without header and footer.
