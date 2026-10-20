# Zavik site

Static landing page for **Zavik**, served at
`https://jhongamers.github.io/zavik/` — a **project site of this repo**
(`Jhongamers/zavik`), published from this orphan `gh-pages` branch.

Plain HTML + inline CSS, no build step, `.nojekyll` present (served as-is).

## Publishing

The repo is private until launch (free-plan Pages needs a public repo).
On launch day, next to opening the repo:

1. Settings → Pages → Source: **Deploy from a branch**.
2. Branch: `gh-pages` / root (save).

The site goes live at `https://jhongamers.github.io/zavik/` (name verified:
GitHub serves project sites at `<owner>.github.io/<repo>` — docs + a live
control case, not assumed).

> Note: a separate repo named `zavik.github.io` would have published at
> `.../zavik.github.io/`, which is NOT the registered ADR URL — that is why
> the site lives on this branch instead.

## Placeholders for the founder

- **Demo GIF**: drop a `demo.gif` at the branch root and replace the
  `.demo` div in `index.html` with
  `<img src="demo.gif" alt="zavik up demo" width="100%">`.
- **LICENSE legal name**: the footer says "MIT License"; add the holder name
  once it's decided.
- Copy mirrors the root `README.md` (install by OS, honest limitations);
  re-sync at launch milestones, not every commit.
