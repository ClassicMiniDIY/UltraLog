# Handoff: ultralog.co (Nuxt + Vue + TypeScript on Cloudflare)

Rebuild ultralog.co, currently a single GitHub Pages page, as a Nuxt site deployed on Cloudflare, with download and release content generated from the app's GitHub releases. Structure, look and behaviour are in `design-system/5-website.md`; components in `design-system/components/{SiteHero,FeatureCard,DownloadCard,ReleaseCard}`, with renders in `reference/`.

## Stack

- Latest stable Nuxt, Vue 3, TypeScript (strict). Nuxt Content for docs pages if the wiki is imported.
- Styling: `design/tokens/tokens.css` imported globally (it declares every token as a CSS variable, with `[data-theme="…"]` overrides) plus scoped component CSS, or Tailwind/UnoCSS whose theme maps to the variables (`colors: { cyan: 'var(--cyan)', … }`). Never hard-code colours.
- Fonts: self-host `design/fonts/woff2/` (the `@font-face` rules are already in `tokens.css`; adjust the URL path to where the files are served).
- `design/tokens/tokens.ts` gives typed, resolved values for anything that needs colours in JS (OG images, charts).

## Pages

`/` (hero, features, mode screenshots, supported ECUs, downloads, latest release), `/download`, `/releases`, `/docs` (start by linking to the GitHub wiki; optionally import it into Nuxt Content), `/ecus/[slug]`. Copy existing text from the current ultralog.co where it still applies; follow the voice rules in `design-system/README.md`.

## Release-driven content

- A server/build utility fetches `GET https://api.github.com/repos/{owner}/{repo}/releases` (use a `GITHUB_TOKEN` to avoid rate limits). Map assets to platforms by file name (`.dmg` → macOS + arch, `.msi`/`.exe` → Windows, `.AppImage`/`.deb` → Linux), sizes from the asset, notes from the release body (render Markdown).
- Prerender these pages at build. Rebuild when a release is published: in the **app** repo, add a GitHub Actions workflow on `release: published` that sends a `repository_dispatch` to the **site** repo (or calls a Cloudflare deploy hook); the site's deploy workflow listens for it.
- DownloadCard picks the visitor's OS/architecture client-side (user agent / `navigator.userAgentData`) and marks it "Your system"; without JS, all three cards show equally.

## Themes and accessibility

- Default `data-theme` is `night`. Footer offers **Color blind mode** (sets `cvd`) and optionally an Ultra intensity toggle (`stealth` / `night` / `ultra`), remembered in `localStorage`, applied before first paint (inline script in `app.vue` head) to avoid a flash.
- Respect `prefers-contrast: more` → `hc`, and `prefers-reduced-motion`.
- Meet WCAG AA everywhere (AAA in `cvd`/`hc`, as the tokens guarantee); real alt text on screenshots; visible focus (`--focus-ring`).
- Social/OG image: `design/logos/hero-sunset-dash.png`; favicon from `ultralog-mark.svg`.

## Cloudflare deployment

- Deploy to Cloudflare (Workers with static assets, or Pages; follow Nuxt's current Cloudflare deployment docs for the right Nitro preset). The site can be fully prerendered (`nuxt generate`) unless server routes are wanted.
- CI: a GitHub Actions workflow builds and deploys with `cloudflare/wrangler-action` on push to `main`, on `repository_dispatch` (new app release), and on a manual trigger. Secrets: `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID`, `GITHUB_TOKEN` (read-only for releases).
- Domain: move ultralog.co DNS to Cloudflare (or add the zone), attach the custom domain to the project, then retire the GitHub Pages site and its CNAME. Keep old URLs working with redirects if any existed.

## Acceptance criteria

- [ ] Lighthouse: performance ≥ 90, accessibility 100 on `/`.
- [ ] Publishing a GitHub release updates `/`, `/download` and `/releases` without a manual commit.
- [ ] Theme toggles persist and don't flash on load; `cvd` and `hc` match the app.
- [ ] No hard-coded colours; all from tokens.
- [ ] Hero, feature cards, downloads and release cards match `reference/` renders.
