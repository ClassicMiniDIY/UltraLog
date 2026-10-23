# FormulaEditor

The formula field of a computed channel, with syntax colouring, insert chips, live validation and a value preview.

Syntax roles: channel names `cyan`, numbers `gold`, functions (`abs`, `sqrt`) `violet`, time shifts (`@-0.1s`, `[-1]`) `orange`, operators `ink-muted`. Validation shows below the field as an inline status with icon and words ("Formula valid", "Unknown channel …"); the preview row prints min / avg / max in `font-mono`. Colour is never the only signal of a token's role, so the editor stays readable in Color-blind safe.
