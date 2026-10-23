# Plot

The time-series chart at the heart of Log Viewer (and each panel of Stacked Plots).

- Canvas `plot-bg`; minor grid `plot-grid`, major `plot-grid-major`; ticks `plot-axis` in `data-sm`.
- Series take `series-1`…`series-10` in selection order, 1.5px (2px on hover/solo). In Color-blind safe, series 9-10 are dashed 6/4.
- The playhead is a full-height `plot-cursor` line, 1.5px. No series ever uses the cursor colour.
- Drag-to-zoom and lap ranges fill with `plot-selection`.
- **Direct labels:** in Color-blind safe and High contrast each series is also labelled with its name at its right-hand end, and the legend moves to the top-left. Labels dodge each other vertically.
- **Intensity:** at Neon and above, series lines get `bloom-series` (a soft glow in the line's own colour) and Ultra draws the `plot-floor` perspective grid behind the data. Both are off in Color-blind safe, High contrast and exports.
- The floating legend (`plot-legend-bg`) lists Cursor first with its time, then each channel: swatch, name, current value right-aligned in mono.

Consumer provides: series (name, unit, samples, index), cursor time, visible range.
