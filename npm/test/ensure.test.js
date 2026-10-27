'use strict';
// Tests for the npm package's binary resolver (node:test, zero dependencies
// — the package itself ships none, and the tests honor the same stance).
//
// The E2E cases fake a whole release (tarball + sha256) served over local
// HTTP, mirroring crates/cli/tests/installer.rs: happy path installs a
// working binary and caches it; a tampered archive is refused.

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const http = require('node:http');
const crypto = require('node:crypto');
const { execFileSync } = require('node:child_process');

const { resolveTarget, resolveVersion, ensureBinary } = require('../lib/ensure');

// --- platform mapping --------------------------------------------------------

test('linux x64/arm64 and macOS arm64 map to release targets', () => {
  assert.equal(
    resolveTarget('linux', 'x64').target,
    'x86_64-unknown-linux-gnu'
  );
  assert.equal(
    resolveTarget('linux', 'arm64').target,
    'aarch64-unknown-linux-gnu'
  );
  assert.equal(resolveTarget('darwin', 'arm64').target, 'aarch64-apple-darwin');
});

test('macOS Intel fails honestly, pointing at cargo install', () => {
  assert.throws(() => resolveTarget('darwin', 'x64'), /cargo install zavik/);
});

test('Windows fails honestly, pointing at the release .zip', () => {
  assert.throws(() => resolveTarget('win32', 'x64'), /\.zip/);
});

test('unknown platforms fail with the install-docs link', () => {
  assert.throws(() => resolveTarget('freebsd', 'x64'), /#install/);
});

// --- version resolution ------------------------------------------------------

test('ZAVIK_VERSION pins the tag (same override as install.sh)', () => {
  process.env.ZAVIK_VERSION = 'v9.9.9';
  try {
    assert.equal(resolveVersion('0.1.4'), 'v9.9.9');
  } finally {
    delete process.env.ZAVIK_VERSION;
  }
});

test('without a pin the package version is used', () => {
  assert.equal(resolveVersion('0.1.4'), '0.1.4');
});

// --- download + verify + cache (fake release over local HTTP) ----------------

const ASSET_TAG = 'v0.0.0-test';

function fakeRelease() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'zavik-npm-release-'));
  const stage = fs.mkdtempSync(path.join(os.tmpdir(), 'zavik-npm-stage-'));
  const binary = path.join(stage, 'zavik');
  fs.writeFileSync(binary, '#!/bin/sh\necho zavik npm-test-binary\n');
  fs.chmodSync(binary, 0o755);
  const asset = `zavik-${ASSET_TAG}-x86_64-unknown-linux-gnu.tar.gz`;
  const tarballPath = path.join(dir, asset);
  execFileSync('tar', ['-czf', tarballPath, '-C', stage, 'zavik']);
  const tarball = fs.readFileSync(tarballPath);
  const sha256 = crypto.createHash('sha256').update(tarball).digest('hex');
  return { dir, asset, tarball, sha256 };
}

/// Serves the fixture release; `sabotage` flips a tarball byte AFTER the
/// checksum was computed (the tamper scenario).
function serveRelease(release, { sabotage = false } = {}) {
  const hits = [];
  const server = http.createServer((request, response) => {
    hits.push(request.url);
    if (request.url === `/${release.asset}.sha256`) {
      response.end(`${release.sha256}  ${release.asset}\n`);
      return;
    }
    if (request.url === `/${release.asset}`) {
      const body = sabotage
        ? Buffer.from(
            release.tarball.map((byte, index) =>
              index === release.tarball.length - 1 ? byte ^ 0xff : byte
            )
          )
        : release.tarball;
      response.end(body);
      return;
    }
    response.statusCode = 404;
    response.end('not found');
  });
  return new Promise((resolve) => {
    server.listen(0, '127.0.0.1', () => {
      resolve({ server, hits, url: `http://127.0.0.1:${server.address().port}` });
    });
  });
}

function withEnv(overrides, body) {
  const saved = {};
  for (const [key, value] of Object.entries(overrides)) {
    saved[key] = process.env[key];
    process.env[key] = value;
  }
  return Promise.resolve()
    .then(body)
    .finally(() => {
      for (const [key, value] of Object.entries(saved)) {
        if (value === undefined) delete process.env[key];
        else process.env[key] = value;
      }
    });
}

test('first run installs the checksum-verified binary, second run is cached', async () => {
  const release = fakeRelease();
  const { server, hits, url } = await serveRelease(release);
  const cache = fs.mkdtempSync(path.join(os.tmpdir(), 'zavik-npm-cache-'));
  try {
    await withEnv(
      {
        ZAVIK_VERSION: ASSET_TAG,
        ZAVIK_BASE_URL: url,
        ZAVIK_CACHE_DIR: cache,
      },
      async () => {
        const binary = await ensureBinary({ version: '0.0.0-test' });
        assert.ok(fs.existsSync(binary), 'binary lands in the cache');

        const output = execFileSync(binary).toString();
        assert.match(output, /npm-test-binary/, 'installed binary runs');

        // Cached: a second resolve makes no new network requests.
        const before = hits.length;
        const again = await ensureBinary({ version: '0.0.0-test' });
        assert.equal(again, binary, 'same cached path');
        assert.equal(hits.length, before, 'no extra requests when cached');
      }
    );
  } finally {
    server.close();
  }
});

test('a tampered archive is refused and nothing is installed', async () => {
  const release = fakeRelease();
  const { server, url } = await serveRelease(release, { sabotage: true });
  const cache = fs.mkdtempSync(path.join(os.tmpdir(), 'zavik-npm-cache-'));
  try {
    await withEnv(
      {
        ZAVIK_VERSION: ASSET_TAG,
        ZAVIK_BASE_URL: url,
        ZAVIK_CACHE_DIR: cache,
      },
      async () => {
        await assert.rejects(
          ensureBinary({ version: '0.0.0-test' }),
          /sha256 mismatch/,
          'checksum divergence is refused, same rule as install.sh'
        );
        const { resolveTarget } = require('../lib/ensure');
        const { target } = resolveTarget('linux', 'x64');
        const binary = path.join(cache, `npm-${ASSET_TAG}`, target, 'zavik');
        assert.ok(!fs.existsSync(binary), 'nothing installed on tamper');
      }
    );
  } finally {
    server.close();
  }
});
