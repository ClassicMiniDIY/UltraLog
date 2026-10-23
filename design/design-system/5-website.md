# Website

ultralog.co moves from a single GitHub Pages page to a Nuxt site that builds its release-driven content from the GitHub API.

## Structure

- **/** Hero (SiteHero) · three FeatureCards · screenshot band per mode (Log Viewer, Scatter, Histogram, Lambda Delay, Accel Enrichment) · supported ECUs grid · DownloadCards · latest ReleaseCard · footer.
- **/download** All builds for the latest release, checksums, older versions.
- **/releases** Every ReleaseCard, generated from GitHub release notes.
- **/docs** Wiki content rendered with Nuxt Content.
- **/ecus** One page per supported ECU/format: file types, channels mapped by field normalization, sample log.

## Programmatic behaviour

- At build (and on a release webhook), fetch `/repos/{owner}/ultralog/releases`; generate DownloadCards from each release's assets (OS from the file name, size from the asset), ReleaseCards from the body, and the version shown in the hero.
- Detect the visitor's OS and architecture to choose the primary download; the others stay secondary.
- Offer the same Color blind mode toggle as the app (sets `data-theme="cvd"`, remembered in localStorage); respect `prefers-contrast: more` with `hc` and `prefers-reduced-motion`.
- Optional: an intensity toggle in the footer for the full Ultra look.

## Look

- `bg-void` page, sections separated by `space-12` and `line-subtle` rules.
- One SiteHero per page, with the striped sun and the perspective floor.
- Feature and ECU cards use the chamfered hexagon `ul-hex` frame; buttons, downloads and releases use `radius-lg` cards.
- Screenshots of the app sit on `bg-panel` with a 1px `line` frame and the redline on top, never with fake window chrome beyond the app's own.
- The hero art (`hero-sunset-dash.png`) is for social cards; the live hero is built from tokens so it follows the theme.
