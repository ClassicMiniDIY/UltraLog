# ChannelCard

One card per selected channel in the Selected Channels strip: name, min and max with jump-to buttons, remove.

The series colour appears twice: a 2px strip along the card's top edge and the line swatch beside the name, the same swatch the legend uses. The name itself stays `ink` (series colours are lines, not text). Values are `font-mono` 15px with the unit in `ink-muted`. In Color-blind safe, High contrast and exports, series 9 and 10 show a dashed swatch (`ul-swatch--cvd-dash`) to match their dashed lines. Consumer provides: channel name, series index (1-10), min, max, unit.
