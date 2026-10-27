'use strict';
// Zavik npm package — release-binary resolver.
//
// Zero npm dependencies on purpose (house supply-chain stance: we audit what
// we ship). The npm surface is a thin shim: `npm install` copies files and
// does NOTHING else — no install script, no network, no execution. The
// official GitHub release binary is downloaded and sha256-verified on the
// first `zavik` run, as the invoking user, into a user cache dir. See
// docs/design-npm.md for the decision record.

const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const http = require('node:http');
const https = require('node:https');
const crypto = require('node:crypto');
const { execFileSync } = require('node:child_process');

const REPO = 'Jhongamers/zavik';

// Platform -> release.yml asset. Must match the release.yml target matrix.
function resolveTarget(platform = process.platform, arch = process.arch) {
  if (platform === 'linux' && arch === 'x64') {
    return { target: 'x86_64-unknown-linux-gnu', ext: 'tar.gz' };
  }
  if (platform === 'linux' && arch === 'arm64') {
    return { target: 'aarch64-unknown-linux-gnu', ext: 'tar.gz' };
  }
  if (platform === 'darwin' && arch === 'arm64') {
    return { target: 'aarch64-apple-darwin', ext: 'tar.gz' };
  }
  if (platform === 'darwin' && arch === 'x64') {
    throw new Error(
      'zavik: no release binary for macOS Intel (x64) — the release builds macOS arm64 only. ' +
        'Install with `cargo install zavik` instead (https://crates.io/crates/zavik).'
    );
  }
  if (platform === 'win32') {
    throw new Error(
      'zavik: the npm package supports Linux and macOS only — on Windows grab the .zip from ' +
        'https://github.com/Jhongamers/zavik/releases.'
    );
  }
  throw new Error(
    `zavik: unsupported platform ${platform}/${arch} — see every install option at ` +
      'https://github.com/Jhongamers/zavik#install.'
  );
}

// Explicit pin (same override as install.sh) > the published package version
// (kept in lockstep with the release by the release_pins cargo test).
function resolveVersion(pkgVersion) {
  return process.env.ZAVIK_VERSION || pkgVersion;
}

function cacheDir(tag, target) {
  const base =
    process.env.ZAVIK_CACHE_DIR ||
    path.join(process.env.XDG_CACHE_HOME || path.join(os.homedir(), '.cache'), 'zavik');
  return path.join(base, `npm-${tag}`, target);
}

// GET with redirect following (release downloads 302 to
// objects.githubusercontent.com). Resolves the response body as a Buffer.
function get(url, redirectsLeft = 5, redirects = 0) {
  return new Promise((resolve, reject) => {
    const mod = url.startsWith('http:') ? http : https;
    const request = mod.get(url, (response) => {
      const status = response.statusCode || 0;
      if (status >= 300 && status < 400 && response.headers.location) {
        response.resume();
        if (redirectsLeft <= 0) {
          reject(new Error(`too many redirects fetching ${url}`));
          return;
        }
        resolve(get(response.headers.location, redirectsLeft - 1, redirects + 1));
        return;
      }
      if (status !== 200) {
        response.resume();
        reject(new Error(`HTTP ${status} fetching ${url}`));
        return;
      }
      const chunks = [];
      response.on('data', (chunk) => chunks.push(chunk));
      response.on('end', () => resolve(Buffer.concat(chunks)));
    });
    request.on('error', reject);
  });
}

// Downloads the release tarball, verifies its sha256 (no skip flag — same
// rule as install.sh) and extracts the binary into the user cache.
// Returns the absolute path of the ready-to-run binary.
async function ensureBinary(options = {}) {
  const version = options.version;
  if (!version) throw new Error('zavik: ensureBinary requires a version');
  const tag = `v${version}`;
  const { target } = resolveTarget();
  const dir = cacheDir(tag, target);
  const binary = path.join(dir, 'zavik');
  if (fs.existsSync(binary)) return binary;

  const base =
    process.env.ZAVIK_BASE_URL || `https://github.com/${REPO}/releases/download/${tag}`;
  const asset = `zavik-${tag}-${target}.tar.gz`;

  const tarball = await get(`${base}/${asset}`);
  const checksumLine = (await get(`${base}/${asset}.sha256`)).toString('utf8');
  const expected = checksumLine.trim().split(/\s+/)[0];
  const actual = crypto.createHash('sha256').update(tarball).digest('hex');
  if (actual !== expected) {
    throw new Error(
      `zavik: sha256 mismatch for ${asset} — refusing to install (corrupted download or tampered asset?)`
    );
  }

  // Extract to a sibling temp dir, then rename into place: a concurrent first
  // run downloads identical, checksum-verified bytes, so last rename wins.
  fs.mkdirSync(dir, { recursive: true });
  const stage = fs.mkdtempSync(path.join(dir, '.stage-'));
  try {
    const tarballPath = path.join(stage, asset);
    fs.writeFileSync(tarballPath, tarball);
    execFileSync('tar', ['-xzf', tarballPath, '-C', stage]);
    const staged = path.join(stage, 'zavik');
    if (!fs.existsSync(staged)) {
      throw new Error(`zavik: release archive ${asset} did not contain a 'zavik' binary`);
    }
    fs.chmodSync(staged, 0o755);
    fs.renameSync(staged, binary);
  } finally {
    fs.rmSync(stage, { recursive: true, force: true });
  }
  return binary;
}

module.exports = { REPO, resolveTarget, resolveVersion, cacheDir, ensureBinary };
