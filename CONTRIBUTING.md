# Contributing to Zavik

Thanks for wanting to help. This document is the honest contract between
you and this project — read it before opening a PR and nobody wastes time.

## Ways to contribute (no PR needed)

- **Bug reports** — open an issue with: `zavik --version`, OS, steps to
  reproduce, expected vs. actual (JSON output helps a lot).
- **Docs** — typos, unclear explanations, missing examples: just open a PR.
- **Ideas / roadmap discussion** — issues are open; say what you would use
  and for what.

## Before you build code: open an issue first

For anything beyond typos/docs/tests, **propose before you build** — open
an issue describing the change and wait for a reaction. PRs sent without
prior discussion may be closed, not because they are bad, but because the
project follows a written roadmap (`ROADMAP.md`) and an uncoordinated PR
can conflict with work already in progress.

Honest expectation: this is a **solo-maintained** project. Reviews and
merges happen, but they can take days. No SLA is promised.

## Ground rules for code PRs

1. **Tests are mandatory.** A PR that changes behavior without a test that
   fails without the change will not merge. CI enforces
   `cargo test --workspace` and `cargo clippy --all-targets -- -D warnings`.
2. **Small, focused diffs.** One concern per PR. Large PRs will be asked to
   split.
3. **No new dependencies without prior discussion** (in the issue, before
   `cargo add`). This is a supply-chain security tool — every dependency it
   ships is attack surface, and we apply that standard to ourselves.
4. **English everywhere in the repo**: identifiers, comments, rustdoc, docs.
   User-facing CLI strings go through the i18n catalog (`pt-BR` + `en`);
   **JSON, SARIF and exit codes are never translated** — that contract is
   frozen in English.
5. **Zavik only proposes.** PRs that add automatic *execution* of fixes are
   out of scope by design — the tool proposes, the human executes.
6. **Don't touch pre-existing formatting drift** outside your changed lines;
   keep new lines `rustfmt`-clean.

## Review process

1. Open a PR against `main`; CI must be green.
2. Maintainer review — expect questions; they are a sign of care, not
   rejection.
3. Merge when both sides are satisfied. Merged code is code one person is
   willing to maintain.

## Security vulnerabilities

**Do not open a public issue.** Use GitHub's private vulnerability
reporting (Security tab → Report a vulnerability) — details in
[SECURITY.md](SECURITY.md).

## License

By contributing you agree your contribution is released under the
[MIT License](LICENSE), the same license as the project.
