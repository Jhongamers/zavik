#!/usr/bin/env node
'use strict';
// Zavik npm shim: resolves the release binary (download + sha256 on first
// run, cached afterwards) and executes it with the exact arguments, exit
// code and signals of a native install.

const os = require('node:os');
const { spawnSync } = require('node:child_process');
const pkg = require('../package.json');
const { ensureBinary, resolveVersion } = require('../lib/ensure');

ensureBinary({ version: resolveVersion(pkg.version) })
  .then((binary) => {
    const result = spawnSync(binary, process.argv.slice(2), { stdio: 'inherit' });
    if (result.error) {
      console.error(`zavik: ${result.error.message}`);
      process.exit(1);
    }
    if (result.signal) {
      // Mirror the native binary: 128 + signal number (e.g. 130 for SIGINT).
      const number = os.constants.signals[result.signal];
      process.exit(number ? 128 + number : 1);
    }
    process.exit(result.status === null ? 1 : result.status);
  })
  .catch((error) => {
    console.error(error.message);
    process.exit(1);
  });
