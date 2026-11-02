/**
 * Locate the procable engine binary for tests that drive verbs end to end.
 *
 * Order: $PROCABLE_BIN, then skill/scripts/bin/<os>-<arch>/procable[.exe]
 * (what `bun run fetch:engine` writes), then target/release/procable[.exe]
 * (what `cargo build --release -p procable` writes, so a local source build
 * is picked up without any extra step). Returns null when none exists so a
 * suite can skip cleanly instead of failing on a machine without the engine.
 */
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');

export function engineTarget() {
  const platform = { darwin: 'darwin', linux: 'linux', win32: 'windows' }[os.platform()] || 'unknown';
  const arch = { arm64: 'arm64', x64: 'x64' }[os.arch()] || 'unknown';
  return `${platform}-${arch}`;
}

export function findEngineBinary() {
  const fromEnv = process.env.PROCABLE_BIN;
  if (fromEnv && fs.existsSync(fromEnv)) return path.resolve(fromEnv);
  const target = engineTarget();
  const exe = target.startsWith('windows-') ? 'procable.exe' : 'procable';
  const candidates = [
    path.join(REPO_ROOT, 'skill', 'scripts', 'bin', target, exe),
    path.join(REPO_ROOT, 'target', 'release', exe),
  ];
  return candidates.find((p) => fs.existsSync(p)) || null;
}

export const ENGINE_MISSING_MESSAGE =
  'engine binary not found: run `cargo build --release -p procable` or `bun run fetch:engine`, or set PROCABLE_BIN';

/** Environment the launcher would export for the binary when run from this repo's skill dir. */
export function engineEnv(bin, extra = {}) {
  return {
    ...process.env,
    PROCABLE_SKILL_DIR: path.join(REPO_ROOT, 'skill'),
    PROCABLE_SELF: bin,
    ...extra,
    // Any roll API request a test makes is tagged as test traffic. Set last,
    // so neither the shell nor a test's extras can untag it.
    PROCABLE_TELEMETRY_CHANNEL: 'test',
  };
}
