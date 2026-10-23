# TextField

Text input, number field and select (channel picker). Inputs sit in a `bg-field` well with a `line-control` border and `radius-sm`, 28px tall.

- Focus: border turns `cyan` plus `focus-ring`.
- Required channel mappings mark the label with a magenta `*`.
- Number fields (bins, thresholds, sensitivity) use `font-mono`, tabular figures, right aligned, and show their unit inside the field ("30 s").
- An unmapped optional channel shows the placeholder "Not logged — optional" in `ink-subtle`, plus a warning badge on the row if results depend on it.
- Error: `danger` border and an inline status below with the octagon icon.

Forms lay out as a two-column grid: labels (`label` style, `ink-muted`) left, controls right.
