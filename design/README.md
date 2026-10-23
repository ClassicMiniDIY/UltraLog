# UltraLog design handoff

Everything needed to implement the UltraLog "Night Drive" design system in the desktop app (Rust + egui) and to rebuild ultralog.co (Nuxt on Cloudflare). Hand this folder to Claude Code; it is written to be read by Claude Code as well as by you.

## What's inside

| Folder | Contents |
| --- | --- |
| `HANDOFF-APP.md` | Task brief for the desktop app: phases, file-by-file guidance, acceptance criteria |
| `HANDOFF-WEBSITE.md` | Task brief for the Nuxt + Cloudflare website |
| `design-system/` | The full brand book: `README.md` (principles, colour, type, logo, voice), numbered sections (intensity, accessibility, data viz, exports, website, implementation), and every component's guidelines + HTML/CSS preview |
| `tokens/` | `tokens.json` (source of truth, 7 themes), `tokens.css` (web), `tokens.ts` (typed, resolved hex per theme) |
| `rust/` | `ultralog_theme.rs` (all tokens as egui `Color32` consts) and `theme.rs` (settings → live tokens, OKLab blending for the intensity slider, egui `Visuals` mapping, glow helper, tests). Compiles and passes tests against egui 0.31 |
| `fonts/` | `woff2/` for the web, `ttf/` for egui (Latin subsets; see the app brief for full-coverage files) |
| `logos/` | Lockups (dark, paper, mono), mark (colour, mono ink, mono white), app icon (SVG + 1024 PNG), original badge, hero art |
| `reference/` | PNG renders of every component in Night Drive, plus key components in Color-blind safe, High contrast, Ultra, Stealth and Paper. Claude Code can open these to compare its work |
| `tools/check_contrast.py` | Verifies every guaranteed colour pair in every theme; run in CI |

## Starting in Claude Code

1. **Install Claude Code** (once). macOS / Linux / WSL: `curl -fsSL https://claude.ai/install.sh | bash` (or `brew install --cask claude-code`). Windows PowerShell: `irm https://claude.ai/install.ps1 | iex`. Then open a new terminal and check `claude --version`. Docs: https://code.claude.com/docs/en/overview
2. **Copy this folder into the repo** at `design/` (so the app repo has `design/HANDOFF-APP.md`, `design/tokens/…`). Commit it; it's the reference for future work too.
3. **Open the repo and start a session:**
   ```bash
   cd path/to/ultralog
   claude
   ```
   The Claude desktop app's **Code** tab works too, and so does the VS Code extension.
4. **Paste a kickoff prompt** (below). Ask it to plan first; review the plan; then let it work phase by phase, one PR or commit per phase.

### Kickoff prompt: desktop app

```
Read design/README.md, then design/HANDOFF-APP.md in full, then design/design-system/README.md and the numbered sections it references. Explore this codebase and report: egui/eframe/egui_plot versions, where Visuals/fonts are set today, how the color blind mode and settings are stored, how plots, heatmaps and the PNG/PDF export are drawn. Then write an implementation plan following the phases in HANDOFF-APP.md, adapted to what you found. Don't change code until I approve the plan.
```

### Kickoff prompt: website

Run this in a new repo (or a `site/` folder) with this bundle copied to `design/`:

```
Read design/README.md, design/HANDOFF-WEBSITE.md and design/design-system/5-website.md. Then scaffold the Nuxt site described there, using design/tokens/tokens.css and the component guidelines in design/design-system/components/ (SiteHero, FeatureCard, DownloadCard, ReleaseCard). Start with a plan and the Cloudflare deployment setup; ask me for the GitHub repo owner/name and the Cloudflare account details you need.
```

### Tips

- Add a short `CLAUDE.md` to each repo root pointing at `design/` ("UI work follows design/design-system; never hard-code colours, use the theme tokens"). Claude Code reads it every session.
- If the design changes later, update the design system page first, export new tokens, and rerun `python3 design/tools/check_contrast.py design/tokens/tokens.json`.
