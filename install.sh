#!/bin/sh
# Zavik installer — https://github.com/Jhongamers/zavik
#
# Detects your OS/arch, downloads the release tarball, verifies its sha256
# and installs the `zavik` binary. Non-interactive, POSIX sh.
#
# Honest notes:
#   - Binaries exist from release v0.1.0 on (see the Releases page).
#   - `curl | sh` is a risk class our own `audit` flags: prefer downloading
#     this script, reading it, then running it.
#   - Windows is not supported by this script: use the .zip from Releases.
#   - The script pins a release version; bump via ZAVIK_VERSION.
#
# Overrides (mirrors, air-gapped, tests):
#   ZAVIK_VERSION       release tag to install (default: v0.1.3)
#   ZAVIK_INSTALL_DIR   install dir (default: $HOME/.local/bin)
#   ZAVIK_BASE_URL      download base (default: GitHub release download URL)
set -eu

REPO="Jhongamers/zavik"
VERSION="${ZAVIK_VERSION:-v0.1.3}"
INSTALL_DIR="${ZAVIK_INSTALL_DIR:-${HOME}/.local/bin}"
BASE_URL="${ZAVIK_BASE_URL:-https://github.com/${REPO}/releases/download/${VERSION}}"

say() { printf '%s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

# --- OS/arch -> Rust target triple (must match release.yml) ------------------
os="$(uname -s | tr '[:upper:]' '[:lower:]')"
machine="$(uname -m)"

case "$machine" in
  x86_64 | amd64) arch="x86_64" ;;
  aarch64 | arm64) arch="aarch64" ;;
  *) die "unsupported architecture: $machine" ;;
esac

case "$os" in
  linux) target="${arch}-unknown-linux-gnu" ;;
  darwin) target="${arch}-apple-darwin" ;;
  *)
    die "unsupported operating system: $os (supported: Linux, macOS; Windows: use the .zip from https://github.com/${REPO}/releases)"
    ;;
esac

asset="zavik-${VERSION}-${target}.tar.gz"
url="${BASE_URL}/${asset}"

# --- downloader --------------------------------------------------------------
if command -v curl >/dev/null 2>&1; then
  download() { curl -fsSL "$1" -o "$2"; }
elif command -v wget >/dev/null 2>&1; then
  download() { wget -qO "$2" "$1"; }
else
  die "need curl or wget to download ${url}"
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT INT TERM

say "downloading ${url}"
download "$url" "${tmp}/${asset}" ||
  die "download failed: ${url} (does this release/asset exist?)"
download "${url}.sha256" "${tmp}/${asset}.sha256" ||
  die "download failed: ${url}.sha256"

# --- checksum verification (anti-tamper; there is no skip flag) --------------
say "verifying sha256"
if command -v sha256sum >/dev/null 2>&1; then
  (cd "$tmp" && sha256sum -c "${asset}.sha256" >/dev/null) ||
    die "checksum mismatch — refusing to install (corrupted download or tampered asset?)"
elif command -v shasum >/dev/null 2>&1; then
  (cd "$tmp" && shasum -a 256 -c "${asset}.sha256" >/dev/null) ||
    die "checksum mismatch — refusing to install (corrupted download or tampered asset?)"
else
  die "need sha256sum or shasum to verify the download"
fi

# --- extract + install -------------------------------------------------------
tar -xzf "${tmp}/${asset}" -C "$tmp" || die "could not extract ${asset}"
[ -f "${tmp}/zavik" ] || die "archive did not contain a 'zavik' binary"

mkdir -p "$INSTALL_DIR"
mv "${tmp}/zavik" "${INSTALL_DIR}/zavik"
chmod +x "${INSTALL_DIR}/zavik"

say ""
say "installed: ${INSTALL_DIR}/zavik ($("${INSTALL_DIR}/zavik" --version 2>/dev/null || echo "${VERSION}"))"
case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) ;;
  *)
    say "note: ${INSTALL_DIR} is not on your PATH — add it:"
    say "  export PATH=\"${INSTALL_DIR}:\$PATH\""
    ;;
esac
say "next: cd your-project && zavik audit"
