# zavik

**Supply-chain audit for your dependencies.** Zavik inspects what your
package manager is about to install — install scripts, network/shell access,
typosquats, unpinned versions — and proposes fixes. Honest about what it
cannot see. MIT, open source: <https://github.com/Jhongamers/zavik>

```bash
npm install -g zavik
zavik audit
```

## How this package installs (read this — it's different on purpose)

`npm install -g zavik` **does not download or execute anything**. It copies a
small JavaScript shim, nothing else. There is **no install script** — the
class of mechanism our own `audit` classifies as a supply-chain risk.

On the **first `zavik` run**, the shim downloads the official release binary
for your platform from the GitHub Releases page, **verifies its sha256**
against the published checksum (a mismatch is refused — there is no skip
flag), and caches it under `~/.cache/zavik/` (`$XDG_CACHE_HOME` respected).
Later runs reuse the cache. The binary runs as your user, not as root at
install time.

## Supported platforms

| Platform | Status |
| --- | --- |
| Linux x64 / arm64 | ✅ release binary |
| macOS arm64 (Apple Silicon) | ✅ release binary |
| macOS Intel (x64) | ❌ no binary built — `cargo install zavik` |
| Windows | ❌ use the `.zip` from [Releases](https://github.com/Jhongamers/zavik/releases) |

## Overrides

- `ZAVIK_VERSION=vX.Y.Z` — install a specific release (default: the version
  of this npm package, which always matches a published release).
- `ZAVIK_CACHE_DIR` — cache location (default: `$XDG_CACHE_HOME/zavik` or
  `~/.cache/zavik`).

## Alternatives (no Node.js involved)

- `curl -fsSL https://raw.githubusercontent.com/Jhongamers/zavik/main/install.sh | sh`
  (Linux/macOS — read the script first; we mean it)
- `cargo install zavik` (Rust toolchain)

## License

MIT
