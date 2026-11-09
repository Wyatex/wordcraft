#!/usr/bin/env node

const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');

const APP_ROOT = path.resolve(__dirname, '../..');
const NPM_ROOT = path.resolve(__dirname, '..');
const DIST = path.join(NPM_ROOT, 'dist');
const TRUNK_DIST = path.resolve(APP_ROOT, '../../dist/preview');
const repoRoot = path.resolve(APP_ROOT, '../..');

function commandAvailable(command, args) {
  try { execFileSync(command, args, { stdio: 'ignore' }); return true; } catch (_) { return false; }
}

if (!commandAvailable('trunk', ['--version'])) {
  console.error('wordcraft-preview: trunk is required (cargo install trunk)');
  process.exit(1);
}
if (!commandAvailable('rustup', ['target', 'list', '--installed'])) {
  console.error('wordcraft-preview: rustup is required');
  process.exit(1);
}
const targets = execFileSync('rustup', ['target', 'list', '--installed'], { encoding: 'utf8' });
if (!targets.includes('wasm32-unknown-unknown')) {
  console.error('wordcraft-preview: install wasm32-unknown-unknown with rustup target add wasm32-unknown-unknown');
  process.exit(1);
}

if (process.env.WORDCRAFT_PREVIEW_SKIP_TRUNK === '1') {
  console.log('wordcraft-preview: reusing existing Trunk output (WORDCRAFT_PREVIEW_SKIP_TRUNK=1)');
} else {
  execFileSync('trunk', ['build', '--release'], { cwd: APP_ROOT, stdio: 'inherit' });
}
if (!fs.existsSync(TRUNK_DIST)) throw new Error(`Trunk did not create ${TRUNK_DIST}`);
fs.rmSync(DIST, { recursive: true, force: true });
fs.mkdirSync(DIST, { recursive: true });

const topLevel = fs.readdirSync(TRUNK_DIST);
const jsFiles = topLevel.filter(name => name.endsWith('.js'));
const wasmFiles = topLevel.filter(name => name.endsWith('.wasm'));
for (const name of [...jsFiles, ...wasmFiles]) fs.copyFileSync(path.join(TRUNK_DIST, name), path.join(DIST, name));

function copyTree(source, destination) {
  fs.mkdirSync(destination, { recursive: true });
  for (const entry of fs.readdirSync(source, { withFileTypes: true })) {
    const from = path.join(source, entry.name);
    const to = path.join(destination, entry.name);
    if (entry.isDirectory()) copyTree(from, to); else fs.copyFileSync(from, to);
  }
}
if (fs.existsSync(path.join(TRUNK_DIST, 'snippets'))) copyTree(path.join(TRUNK_DIST, 'snippets'), path.join(DIST, 'snippets'));

const generated = jsFiles.find(name => name.startsWith('wordcraft-preview') && !name.endsWith('_bg.js')) || jsFiles[0];
if (!generated) throw new Error('Trunk did not emit a wasm-bindgen JavaScript module');
fs.writeFileSync(path.join(DIST, 'index.js'), `export * from './${generated}';\nexport { default } from './${generated}';\n`);
fs.copyFileSync(path.join(NPM_ROOT, 'index.d.ts'), path.join(DIST, 'index.d.ts'));
for (const license of ['LICENSE-MIT', 'LICENSE-APACHE']) {
  const source = path.join(repoRoot, license);
  if (fs.existsSync(source)) fs.copyFileSync(source, path.join(NPM_ROOT, license));
}
const bytes = wasmFiles.reduce((total, name) => total + fs.statSync(path.join(DIST, name)).size, 0);
console.log(`wordcraft-preview built: ${(bytes / 1024 / 1024).toFixed(2)} MiB wasm`);
