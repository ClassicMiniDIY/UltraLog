# HeatmapTable

Binned tables: Histogram (Average Z / Hit count), Lambda Delay, Accel Enrichment, and the scatter density plot share it.

- Fill from the sequential map `seq-0`…`seq-6` for magnitudes (hits, dead time, RPM), or the diverging map `div-0`…`div-4` for signed values centred on a target (AFR vs target, suggested correction %).
- Value text uses the matching `on-seq-N` / `on-div-N` token, set on a `cell-plate`. The plate is invisible in Night Drive, Stealth, Ultra and Paper and becomes a solid dark plate in Color-blind safe, High contrast and Graphite, so every value reaches 7:1 whatever the cell colour (no colormap can give 7:1 on its mid stops by itself).
- Empty cells use `cell-empty` with a 45° hatch, so "no data" never looks like "low value".
- Confidence is shown by the event-count badge's fill, not its hue: **filled** = high, **outlined** = medium, **dashed** = low. "Blank low-confidence cells" renders low cells as empty.
- The selected cell gets a 2px `cyan` ring with a `bg-void` gap; its statistics appear below the table.
- Axis labels in `plot-axis`, axis titles in `overline`.
