import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const WINDOWS = process.platform === 'win32';
const COMSPEC = process.env.ComSpec || process.env.COMSPEC || 'C:\\Windows\\System32\\cmd.exe';
// Use the host's command interpreter as a harmless Windows executable.
// procable runs offline: every test also listens on a loopback server and
// asserts that the launcher never sends it a request.
const PAYLOAD = WINDOWS ? fs.readFileSync(COMSPEC) : Buffer.from('#!/bin/sh\necho verified-engine\n');
const HASH = createHash('sha256').update(PAYLOAD).digest('hex');
const BIN = WINDOWS ? 'impeccable.exe' : 'impeccable';
// The release asset this host's launcher asks for (the cmd tests pin AMD64).
const ASSET = `impeccable-${WINDOWS ? 'windows' : process.platform === 'darwin' ? 'darwin' : 'linux'}-${!WINDOWS && process.arch === 'arm64' ? 'arm64' : 'x64'}${WINDOWS ? '.exe' : ''}`;


async function exercise(t, scenario) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'procable-launcher-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const scripts = path.join(root, 'skill scripts');
  const home = path.join(root, 'home');
  const cache = path.join(root, 'cache');
  fs.mkdirSync(scripts);
  fs.mkdirSync(home);
  fs.writeFileSync(path.join(scripts, 'VERSION'), '0.0.0-test\n');
  const name = WINDOWS ? 'impeccable.cmd' : 'impeccable';
  const launcher = path.join(scripts, name);
  fs.copyFileSync(path.join(ROOT, 'skill/scripts', name), launcher);
  const digest = scenario === 'cached-stamp-trusted' ? '1'.repeat(64) : HASH;
  const entry = `0.0.0-test ${ASSET} ${digest}`;
  const manifest = scenario === 'cached-crlf-manifest' ? `# test manifest\r\n\r\n${entry}` : `# test manifest\n\n${entry}\n`;
  fs.writeFileSync(path.join(scripts, 'engine-checksums.txt'), manifest);
  const cacheDir = path.join(cache, 'bin', '0.0.0-test');
  if (scenario.startsWith('cached-')) {
    fs.mkdirSync(cacheDir, { recursive: true });
    const tampered = Buffer.from('#!/bin/sh\necho tampered-engine of another size\n');
    const cachedBin = path.join(cacheDir, BIN);
    const swapped = ['cached-tampered', 'cached-replaced', 'cached-moved-older'].includes(scenario);
    fs.writeFileSync(cachedBin, swapped ? tampered : PAYLOAD, { mode: 0o755 });
    const stamp = {
      'cached-empty-stamp': '',
      'cached-foreign-stamp': `${'0'.repeat(64)} ${PAYLOAD.length}`,
      'cached-digest-only-stamp': HASH,
      'cached-replaced': `${HASH} ${tampered.length}`,
      'cached-moved-older': `${HASH} ${PAYLOAD.length}`,
      'cached-stamp-trusted': `${'1'.repeat(64)} ${PAYLOAD.length}\n`,
    }[scenario];
    if (stamp !== undefined) {
      fs.writeFileSync(`${cachedBin}.verified`, stamp);
      const past = new Date(Date.now() - 60000);
      if (scenario === 'cached-replaced') fs.utimesSync(`${cachedBin}.verified`, past, past);
      if (['cached-moved-older', 'cached-stamp-trusted'].includes(scenario)) fs.utimesSync(cachedBin, past, past);
    }
  }
  const tools = path.join(root, 'tools');
  fs.mkdirSync(tools);
  const requests = [];
  const server = http.createServer((req, res) => {
    requests.push(req.url);
    res.end(PAYLOAD);
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  t.after(() => new Promise(resolve => server.close(resolve)));
  // The upstream download variable is set on purpose: procable must ignore it.
  const env = {
    PATH: WINDOWS ? `${process.env.SystemRoot}\\System32;${process.env.SystemRoot}` : `${tools}:/usr/bin:/bin`,
    HOME: home, USERPROFILE: home, TEMP: root, TMP: root,
    IMPECCABLE_HOME: cache,
    IMPECCABLE_DOWNLOAD_BASE: `http://127.0.0.1:${server.address().port}`,
    ...(WINDOWS ? { SystemRoot: process.env.SystemRoot, ComSpec: COMSPEC, PROCESSOR_ARCHITECTURE: 'AMD64' } : {}),
  };
  const run = () => new Promise((resolve, reject) => {
    const child = WINDOWS
      ? spawn(COMSPEC, ['/d', '/s', '/c', `""${launcher}" /d /c echo verified-engine"`], { env, cwd: root, windowsVerbatimArguments: true, timeout: 20000 })
      : spawn('/bin/sh', [launcher], { env, cwd: root, timeout: 20000 });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', data => { stdout += data; });
    child.stderr.on('data', data => { stderr += data; });
    child.on('error', reject);
    child.on('close', (status, signal) => resolve({ status, signal, stdout, stderr }));
  });
  const result = await run();
  assert.equal(result.signal, null, JSON.stringify(result));
  assert.deepEqual(requests, [], 'procable never downloads an engine');
  return { ...result, files: fs.existsSync(cacheDir) ? fs.readdirSync(cacheDir).sort() : [], requests, cacheDir };
}

test('launcher with no engine fails offline and sends no request', async t => {
  const result = await exercise(t, 'no-engine');
  assert.equal(result.status, 127, JSON.stringify(result));
  assert.doesNotMatch(result.stdout, /verified-engine/);
  assert.match(result.stderr, /runs offline and does not download its engine/);
  assert.match(result.stderr, /IMPECCABLE_BIN/);
  assert.deepEqual(result.files, []);
});

test('launcher hashes an unstamped cached engine once, then trusts the stamp', async t => {
  const result = await exercise(t, 'cached-unstamped');
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /verified-engine/);
  assert.equal(result.stderr, '');
  assert.deepEqual(result.files, [BIN, `${BIN}.verified`]);
  assert.equal(fs.readFileSync(path.join(result.cacheDir, `${BIN}.verified`), 'utf8').trim(), `${HASH} ${PAYLOAD.length}`);
});

test('launcher reads a CRLF manifest whose last line has no newline', async t => {
  const result = await exercise(t, 'cached-crlf-manifest');
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /verified-engine/);
});

for (const [scenario, how] of [['cached-tampered', 'that does not match the manifest'], ['cached-replaced', 'written after its stamp'], ['cached-moved-older', 'moved in with an older mtime']]) {
  test(`launcher refuses a cached engine ${how} and downloads nothing`, async t => {
    const result = await exercise(t, scenario);
    assert.equal(result.status, 127, JSON.stringify(result));
    assert.doesNotMatch(result.stdout, /tampered-engine|verified-engine/);
    assert.match(result.stderr, /cannot verify the cached engine/);
  });
}

for (const [scenario, stamp] of [['cached-empty-stamp', 'is empty'], ['cached-foreign-stamp', 'does not hold the manifest digest'], ['cached-digest-only-stamp', 'predates the size field']]) {
  test(`launcher rehashes a cached engine whose stamp ${stamp}`, async t => {
    const result = await exercise(t, scenario);
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /verified-engine/);
    assert.equal(result.stderr, '');
    assert.equal(fs.readFileSync(path.join(result.cacheDir, `${BIN}.verified`), 'utf8').trim(), `${HASH} ${PAYLOAD.length}`, 'the stamp is rewritten');
  });
}

test('launcher runs a cached engine on a good stamp without hashing it', async t => {
  const result = await exercise(t, 'cached-stamp-trusted');
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /verified-engine/);
  assert.equal(result.stderr, '');
});

test('launchers carry no download path', () => {
  for (const name of ['impeccable', 'impeccable.cmd']) {
    const source = fs.readFileSync(path.join(ROOT, 'skill/scripts', name), 'utf8');
    assert.doesNotMatch(source, /curl|wget|IMPECCABLE_DOWNLOAD_BASE|releases\/download/, name);
  }
});

test('cmd launcher is checked out with CRLF line endings', () => {
  const source = fs.readFileSync(path.join(ROOT, 'skill/scripts/impeccable.cmd'));
  const withoutCrlf = Buffer.from(source.toString('binary').replaceAll('\r\n', ''));
  assert.equal(withoutCrlf.includes(10), false, 'cmd.exe batch labels require CRLF line endings');
});

test('cmd launcher forwards engine exit code through cmd /c', { skip: WINDOWS ? false : 'Windows-only cmd /c exit-code forwarding' }, async t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'impeccable-launcher-exit-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const home = path.join(root, 'home');
  fs.mkdirSync(home);
  const launcher = path.join(root, 'impeccable.cmd');
  fs.copyFileSync(path.join(ROOT, 'skill/scripts/impeccable.cmd'), launcher);
  const env = {
    PATH: `${process.env.SystemRoot}\\System32;${process.env.SystemRoot}`,
    HOME: home, USERPROFILE: home, TEMP: root, TMP: root,
    IMPECCABLE_HOME: path.join(root, 'cache'),
    IMPECCABLE_BIN: COMSPEC,
    SystemRoot: process.env.SystemRoot,
    ComSpec: COMSPEC,
    PROCESSOR_ARCHITECTURE: 'AMD64',
  };
  const run = (args) => new Promise((resolve, reject) => {
    const child = spawn(COMSPEC, ['/d', '/s', '/c', `""${launcher}" ${args}"`], { env, cwd: root, windowsVerbatimArguments: true, timeout: 20000 });
    child.on('error', reject);
    child.on('close', (status, signal) => resolve({ status, signal }));
  });
  for (const [args, expected] of [['/c exit 2', 2], ['/c exit 1', 1], ['/c exit 0', 0]]) {
    const result = await run(args);
    assert.equal(result.signal, null, JSON.stringify(result));
    assert.equal(result.status, expected, args);
  }
});

for (const [probeStatus, trusted] of [[0, true], [1, false]]) {
  test(`sh launcher ${trusted ? 'runs' : 'skips'} a home binary whose engine-probe exits ${probeStatus}`, { skip: WINDOWS ? 'POSIX launcher probe' : false }, async t => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'impeccable-launcher-probe-'));
    t.after(() => fs.rmSync(root, { recursive: true, force: true }));
    const scripts = path.join(root, 'scripts');
    const home = path.join(root, 'home');
    const homeBin = path.join(home, '.impeccable', 'bin');
    fs.mkdirSync(scripts);
    fs.mkdirSync(homeBin, { recursive: true });
    fs.writeFileSync(path.join(scripts, 'VERSION'), '0.0.0-test\n');
    fs.writeFileSync(path.join(scripts, 'engine-checksums.txt'), `0.0.0-test ${ASSET} ${HASH}\n`);
    const launcher = path.join(scripts, 'impeccable');
    fs.copyFileSync(path.join(ROOT, 'skill/scripts/impeccable'), launcher);
    // Prints the handshake marker either way; only the exit status differs.
    fs.writeFileSync(path.join(homeBin, 'impeccable'),
      `#!/bin/sh\nif [ "$1" = engine-probe ]; then echo impeccable-engine 0.0.0-test; exit ${probeStatus}; fi\necho home-engine-ran\n`,
      { mode: 0o755 });
    const requests = [];
    const server = http.createServer((req, res) => {
      requests.push(req.url);
      res.writeHead(404);
      res.end();
    });
    await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
    t.after(() => new Promise(resolve => server.close(resolve)));
    const env = {
      PATH: '/usr/bin:/bin',
      HOME: home,
      IMPECCABLE_HOME: path.join(root, 'cache'),
      IMPECCABLE_DOWNLOAD_BASE: `http://127.0.0.1:${server.address().port}`,
    };
    const result = await new Promise((resolve, reject) => {
      const child = spawn('/bin/sh', [launcher, 'detect'], { env, cwd: root, timeout: 20000 });
      let stdout = '';
      child.stdout.on('data', data => { stdout += data; });
      child.on('error', reject);
      child.on('close', (status, signal) => resolve({ status, signal, stdout }));
    });
    assert.equal(result.signal, null, JSON.stringify(result));
    if (trusted) {
      assert.equal(result.status, 0, JSON.stringify(result));
      assert.match(result.stdout, /home-engine-ran/);
      assert.equal(requests.length, 0, 'a trusted home binary needs no download');
    } else {
      assert.equal(result.status, 127, JSON.stringify(result));
      assert.doesNotMatch(result.stdout, /home-engine-ran/);
      assert.equal(requests.length, 0, 'a failed probe never falls through to a download');
    }
  });
}

// The sibling engine slot is gitignored, so a skill upgrade that copies files
// over it leaves the old engine behind. POSIX: a sh stub. Windows: a copy of
// node.exe that answers `engine-probe` through engine-probe.js in the cwd.
const SIBLING_DIR = ASSET.slice('impeccable-'.length).replace(/\.exe$/, '');
// Args for a run that reaches the cached engine (a copy of cmd.exe on Windows).
const CACHED_ARGS = WINDOWS ? ['/d', '/c', 'echo verified-engine'] : ['engine-probe'];

async function runWithSibling(t, { version, probeStatus = 0, mode = 0o755, cached = false, args = ['engine-probe'] }) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'impeccable-launcher-sibling-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  // A space and a parenthesis, as in C:\\Program Files (x86).
  const scripts = path.join(root, 'skill (scripts)');
  const home = path.join(root, 'home');
  const cache = path.join(root, 'ca (che)');
  const sibling = path.join(scripts, 'bin', SIBLING_DIR, BIN);
  fs.mkdirSync(path.dirname(sibling), { recursive: true });
  fs.mkdirSync(home);
  fs.writeFileSync(path.join(scripts, 'VERSION'), '0.0.0-test\n');
  // A digest the cached engine does not hash to: running it proves the stamp
  // was honored in a path with a space and a parenthesis, without a rehash.
  const digest = '1'.repeat(64);
  fs.writeFileSync(path.join(scripts, 'engine-checksums.txt'), `0.0.0-test ${ASSET} ${digest}\n`);
  const name = WINDOWS ? 'impeccable.cmd' : 'impeccable';
  const launcher = path.join(scripts, name);
  fs.copyFileSync(path.join(ROOT, 'skill/scripts', name), launcher);
  if (WINDOWS) {
    fs.copyFileSync(process.execPath, sibling);
    fs.writeFileSync(path.join(root, 'engine-probe.js'), `console.log('impeccable-engine ${version}');\nprocess.exitCode = ${probeStatus};\n`);
  } else {
    fs.writeFileSync(sibling, `#!/bin/sh\necho impeccable-engine ${version}\nexit ${probeStatus}\n`, { mode });
  }
  if (cached) {
    // A trusted cache entry: a stamp holding the manifest digest and size, newer than the binary.
    const cachedBin = path.join(cache, 'bin', '0.0.0-test', BIN);
    fs.mkdirSync(path.dirname(cachedBin), { recursive: true });
    fs.writeFileSync(cachedBin, PAYLOAD, { mode: 0o755 });
    fs.writeFileSync(`${cachedBin}.verified`, `${digest} ${PAYLOAD.length}\n`);
    const past = new Date(Date.now() - 60000);
    fs.utimesSync(cachedBin, past, past);
  }
  const requests = [];
  const server = http.createServer((req, res) => {
    requests.push(req.url);
    res.writeHead(404);
    res.end();
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  t.after(() => new Promise(resolve => server.close(resolve)));
  const env = {
    PATH: WINDOWS ? `${process.env.SystemRoot}\\System32;${process.env.SystemRoot}` : '/usr/bin:/bin',
    HOME: home,
    IMPECCABLE_HOME: cache,
    IMPECCABLE_DOWNLOAD_BASE: `http://127.0.0.1:${server.address().port}`,
    ...(WINDOWS ? { USERPROFILE: home, TEMP: root, TMP: root, SystemRoot: process.env.SystemRoot, ComSpec: COMSPEC, PROCESSOR_ARCHITECTURE: 'AMD64' } : {}),
  };
  const result = await new Promise((resolve, reject) => {
    const child = WINDOWS
      ? spawn(COMSPEC, ['/d', '/s', '/c', `""${launcher}" ${args.join(' ')}"`], { env, cwd: root, windowsVerbatimArguments: true, timeout: 20000 })
      : spawn('/bin/sh', [launcher, ...args], { env, cwd: root, timeout: 20000 });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', data => { stdout += data; });
    child.stderr.on('data', data => { stderr += data; });
    child.on('error', reject);
    child.on('close', (status, signal) => resolve({ status, signal, stdout, stderr }));
  });
  assert.equal(result.signal, null, JSON.stringify(result));
  return { ...result, requests };
}

test('launcher runs a sibling engine that reports the pinned version', async t => {
  const result = await runWithSibling(t, { version: '0.0.0-test' });
  assert.equal(result.status, 0, JSON.stringify(result));
  assert.match(result.stdout, /impeccable-engine 0\.0\.0-test/);
  assert.equal(result.stderr, '');
  assert.deepEqual(result.requests, []);
});

test('launcher skips a sibling engine from another version and runs the cached one', async t => {
  const result = await runWithSibling(t, { version: '0.0.0-old', cached: true, args: CACHED_ARGS });
  assert.equal(result.status, 0, JSON.stringify(result));
  assert.match(result.stdout, /verified-engine/);
  assert.doesNotMatch(result.stdout, /impeccable-engine/);
  for (const part of [path.join('bin', SIBLING_DIR, BIN), '0.0.0-old', '0.0.0-test']) {
    assert.ok(result.stderr.includes(part), result.stderr);
  }
  assert.deepEqual(result.requests, []);
});

test('sh launcher skips a sibling engine whose probe fails', { skip: WINDOWS ? 'POSIX launcher sibling stub' : false }, async t => {
  const result = await runWithSibling(t, { version: '0.0.0-test', probeStatus: 1, cached: true, args: CACHED_ARGS });
  assert.equal(result.status, 0, JSON.stringify(result));
  assert.match(result.stdout, /verified-engine/);
  assert.doesNotMatch(result.stdout, /impeccable-engine/);
  assert.match(result.stderr, /reports engine unknown/);
  assert.deepEqual(result.requests, []);
});

test('sh launcher runs a current sibling engine that lost its executable bit', { skip: WINDOWS ? 'POSIX file modes' : false }, async t => {
  const result = await runWithSibling(t, { version: '0.0.0-test', mode: 0o644 });
  assert.equal(result.status, 0, JSON.stringify(result));
  assert.match(result.stdout, /impeccable-engine 0\.0\.0-test/);
  assert.equal(result.stderr, '');
  assert.deepEqual(result.requests, []);
});
