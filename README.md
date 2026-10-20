# Zavik site

Static landing page for **Zavik**, served at
`https://jhongamers.github.io/zavik/` — a **project site of this repo**
(`Jhongamers/zavik`), published from this orphan `gh-pages` branch.

Plain HTML + inline CSS, no build step, `.nojekyll` present (served as-is).

## Publishing

The repo is public and Pages is live (enabled Oct 20, 2026: Settings →
Pages → Deploy from a branch → `gh-pages` / root). The site serves at
`https://jhongamers.github.io/zavik/` (name verified: GitHub serves
project sites at `<owner>.github.io/<repo>` — docs + a live control
case, not assumed).

To publish a change: edit `index.html`, commit on this branch, push —
GitHub Pages republishes in ~1 minute.

> Note: a separate repo named `zavik.github.io` would have published at
> `.../zavik.github.io/`, which is NOT the registered ADR URL — that is why
> the site lives on this branch instead.

## Brand assets

Official symbol = the split-Z with the ✦ spark (founder-confirmed,
fire-tested at 16px/grayscale/circle). Files live in `~/zavik-brand/`
(outside the repo — brand binaries do not belong in the code repo):

- `zavik-symbol.jpg` — master icon (dark tile)
- `zavik-lockup.jpg` — horizontal reference (wordmark rendered as image;
  site header uses `zavik-mark.png` + HTML text instead)
- `zavik-mark.png` — transparent background, for dark UIs
- `favicon.ico` / `favicon-32.png` / `apple-touch-icon.png`
- `og.png` — 1200×630 link preview (PH/social)

## Placeholders for the founder

- **Demo GIF**: drop a `demo.gif` at the branch root and replace the
  `.demo` div in `index.html` with
  `<img src="demo.gif" alt="zavik up demo" width="100%">`.
- **LICENSE legal name**: the footer says "MIT License"; add the holder name
  once it's decided.
- Copy mirrors the root `README.md` (install by OS, honest limitations);
  re-sync at launch milestones, not every commit.
