# Button

Triggers an action: load a file, re-run an analysis, apply a template, export.

- **primary** (`cyan` fill, `on-cyan` label): one per view, the action that moves the task forward: *Select a file*, *Save*, *Run & Chart*. Hover adds `glow-cyan`.
- **secondary** (default, `bg-raised` with a `line-control` border): everything else: *Re-run current file*, *Apply*, *Validate*, *PNG*, *PDF*.
- **ghost**: low-weight actions next to a primary (*Cancel*, *Copy for paste*, *+ New* in a list header).
- **danger** (outlined in `danger`): removes data the user loaded (*Remove log*, *Delete template*). Never filled.
- **sm** (24px): inside dense rows (library items, results toolbars).
- **Icon button** (28px, 20px `--sm`): close, more, jump-to-min/max. Always give it an `aria-label` / egui hover text.

Consumer provides: label (Title Case, 1-3 words, verb first), optional leading 16px icon, `disabled` state.
Don't: colour export buttons by file type (the old blue PNG / red PDF), put two primaries side by side, or use `magenta` for buttons in the app.
