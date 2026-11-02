#!/usr/bin/env node
// `procable` npm shim: finds the platform binary and execs it with argv.
// Order: $PROCABLE_BIN, then the version-pinned user cache (~/.procable/bin/<version>/). procable
// runs offline: the shim never downloads an engine. A cached binary is
// verified against skill/scripts/engine-checksums.txt (shipped in the
// package); a <binary>.verified stamp holding the manifest digest and the
// binary's size records that a cached binary passed, until either changes or
// the binary is newer than the stamp.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import { createRequire } from 'node:module';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const pkg = require('../../package.json');
const OS = { darwin: 'darwin', linux: 'linux', win32: 'windows' }[process.platform] || process.platform;
const ARCH = { arm64: 'arm64', x64: 'x64' }[process.arch] || process.arch;
const TARGET = `${OS}-${ARCH}`;
const EXE = OS === 'windows' ? 'procable.exe' : 'procable';
const SKILL_SCRIPTS = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', '..', 'skill', 'scripts');
// The engine version is the one the skill pins (skill/scripts/VERSION).
const VERSION = (() => { try { return fs.readFileSync(path.join(SKILL_SCRIPTS, 'VERSION'), 'utf8').trim(); } catch { return ''; } })();
const CACHE_ROOT = process.env.PROCABLE_HOME || path.join(os.homedir(), '.procable');
const CACHED = path.join(CACHE_ROOT, 'bin', VERSION, EXE);
const ASSET = `procable-${TARGET}${OS === 'windows' ? '.exe' : ''}`;
const MANIFEST = path.join(SKILL_SCRIPTS, 'engine-checksums.txt');
const STAMP = `${CACHED}.verified`;

function exists(p) { try { return !!p && fs.statSync(p).isFile(); } catch { return false; } }
// A stamp counts only while it holds an accepted digest and the binary's size,
// and the binary has not changed since it was written.
function stampValid(accepted) {
  try {
    const [digest, size] = fs.readFileSync(STAMP, 'utf8').trim().split(/\s+/);
    const bin = fs.statSync(CACHED);
    return accepted.includes(digest) && size === String(bin.size) && bin.mtimeMs <= fs.statSync(STAMP).mtimeMs;
  } catch { return false; }
}
// The digest MANIFEST records for this version and asset, or '' when none.
function expectedDigest(wanted = ASSET) {
  try {
    for (const line of fs.readFileSync(MANIFEST, 'utf8').split(/\r?\n/)) {
      const [version, asset, sum] = line.trim().split(/\s+/);
      if (version === VERSION && asset === wanted) return (sum || '').toLowerCase();
    }
  } catch { /* no manifest: nothing verifies */ }
  return '';
}
async function locate() {
  const envBin = process.env.PROCABLE_BIN;
  if (exists(envBin)) return envBin;
  if (exists(CACHED)) {
    // Windows on ARM runs x64 binaries, and the launchers fall back to that asset.
    const accepted = [expectedDigest(), TARGET === 'windows-arm64' && expectedDigest('procable-windows-x64.exe')].filter(Boolean);
    if (stampValid(accepted)) return CACHED;
    let digest = '';
    try { digest = createHash('sha256').update(fs.readFileSync(CACHED)).digest('hex'); } catch { /* unreadable: unverified */ }
    if (digest && accepted.includes(digest)) {
      try { fs.writeFileSync(STAMP, `${digest} ${fs.statSync(CACHED).size}\n`); } catch { /* read-only cache: verified this run */ }
      return CACHED;
    }
    process.stderr.write(`procable: cannot verify the cached engine ${CACHED} against ${MANIFEST}; not using it\n`);
  }
  return null;
}

// `--version` / `-v` is answered by the shim itself: the number users mean
// is this npm package's version, not the engine's (docs/CLI-CONTRACT.md).
const argv = process.argv.slice(2);
if (argv[0] === '--version' || argv[0] === '-v') {
  process.stdout.write(`${pkg.version}\n`);
  process.exit(0);
}

const bin = await locate();
if (!bin) {
  process.stderr.write(
    `procable: no engine binary for ${TARGET}. procable runs offline and does not download its engine. `
    + `Reinstall with the prompt-orchestrator installer, or set PROCABLE_BIN.\n`,
  );
  process.exit(127);
}
const result = spawnSync(bin, argv, {
  stdio: 'inherit',
  env: { PROCABLE_SELF: 'npx procable', ...process.env },
});
if (result.error) {
  process.stderr.write(`procable: failed to run ${bin}: ${result.error.message}\n`);
  process.exit(127);
}
process.exit(result.status === null ? 1 : result.status);
