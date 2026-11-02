#!/usr/bin/env node
// Rename transform: impeccable -> procable, across every tracked file and path.
//
// procable is a fork of pbakaus/impeccable. Upstream fixes are merged in as
// impeccable-named code, so this transform is re-run after each upstream sync
// instead of renaming by hand. It is idempotent: a second run changes nothing.
//
//   node scripts/fork/rename-procable.mjs            # dry run: counts only
//   node scripts/fork/rename-procable.mjs --apply    # rewrite files, git mv paths
//
// Kept as upstream wrote them:
// - the attribution files (LICENSE, NOTICE.md, FORK.md) and this script;
// - references to the upstream project: `pbakaus/impeccable`, `impeccable.style`
//   and the phrase "upstream Impeccable";
// - binary files, and the compiled in-page bundle, which `cargo xtask bundle`
//   regenerates from the renamed crates.
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

const APPLY = process.argv.includes('--apply');
const ROOT = execFileSync('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8' }).trim();
process.chdir(ROOT);

const SKIP_FILES = new Set([
  'LICENSE',
  'NOTICE.md',
  'FORK.md',
  'scripts/fork/rename-procable.mjs',
  'crates/live/assets/detect-antipatterns-browser.js',
]);
const BINARY_EXT = /\.(png|jpe?g|gif|webp|avif|ico|icns|woff2?|ttf|otf|eot|zip|gz|tgz|wasm|pdf|mp4|webm|mov|bin|exe|dll|so|dylib)$/i;

// Upstream references that must survive the rename verbatim.
const PROTECT = [
  /\[Impeccable\]\(https:\/\/github\.com\/pbakaus\/impeccable\)/g,
  /Impeccable by Paul Bakaus/g,
  /github\.com\/pbakaus\/impeccable/g,
  /pbakaus\/impeccable/g,
  /impeccable\.style/g,
  /\b[Uu]pstream Impeccable\b/g,
];
const MAP = [
  [/IMPECCABLE/g, 'PROCABLE'],
  [/Impeccable/g, 'Procable'],
  [/impeccable/g, 'procable'],
];

function renameText(text) {
  const kept = [];
  let out = text;
  for (const re of PROTECT) {
    out = out.replace(re, (m) => `\u0000${kept.push(m) - 1}\u0000`);
  }
  for (const [re, to] of MAP) out = out.replace(re, to);
  return out.replace(/\u0000(\d+)\u0000/g, (_, i) => kept[Number(i)]);
}

function renamePath(p) {
  return p.split('/').map((seg) => MAP.reduce((s, [re, to]) => s.replace(re, to), seg)).join('/');
}

function isBinary(file, buf) {
  if (BINARY_EXT.test(file)) return true;
  return buf.subarray(0, 8000).includes(0);
}

const files = execFileSync('git', ['ls-files', '-z'], { encoding: 'utf8' }).split('\0').filter(Boolean);
let contentChanged = 0;
let occurrences = 0;
const moves = [];

for (const file of files) {
  if (!fs.existsSync(file) || fs.lstatSync(file).isSymbolicLink()) continue;
  if (!SKIP_FILES.has(file)) {
    const buf = fs.readFileSync(file);
    if (!isBinary(file, buf)) {
      const text = buf.toString('utf8');
      const next = renameText(text);
      if (next !== text) {
        contentChanged += 1;
        occurrences += (text.match(/impeccable/gi) || []).length - (next.match(/impeccable/gi) || []).length;
        if (APPLY) fs.writeFileSync(file, next);
      }
    }
  }
  const target = renamePath(file);
  if (target !== file && !SKIP_FILES.has(file)) moves.push([file, target]);
}

for (const [from, to] of moves) {
  if (!APPLY) continue;
  fs.mkdirSync(path.dirname(to), { recursive: true });
  execFileSync('git', ['mv', '-f', from, to]);
}
if (APPLY) {
  // Drop directories the moves emptied.
  const dirs = [...new Set(moves.map(([from]) => path.dirname(from)))].sort((a, b) => b.length - a.length);
  for (const d of dirs) {
    let cur = d;
    while (cur && cur !== '.' && fs.existsSync(cur) && fs.readdirSync(cur).length === 0) {
      fs.rmdirSync(cur);
      cur = path.dirname(cur);
    }
  }
}

console.log(`${APPLY ? 'renamed' : 'would rename'}: ${occurrences} occurrences in ${contentChanged} files, ${moves.length} paths`);
