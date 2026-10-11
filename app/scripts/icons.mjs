// Derive every brand asset from the same vector; no platform icon is hand edited.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const app = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const root = dirname(app);
const source = readFileSync(join(root, 'ui/src/assets/brand-mark.svg'), 'utf8');
assert.match(source, /viewBox="0 0 64 64"/);
const tile = source.replace('viewBox="0 0 64 64"', 'viewBox="-10 -10 84 84"').replace(
  /(<svg[^>]*>)/,
  '$1<rect x="-6" y="-6" width="76" height="76" rx="18" fill="#141518" stroke="#34332f" stroke-width=".6"/>',
);
const tray = source.replaceAll('#e8e6e1', '#000000').replaceAll('#e8a33d', '#000000').replace(' opacity=".5"', '');
const sandbox = join(homedir(), 'Desktop/test');
mkdirSync(sandbox, { recursive: true });
const temporary = mkdtempSync(join(sandbox, 'antiquota-icons-'));
const icons = join(app, 'src-tauri/icons');
const cli = join(app, 'node_modules/@tauri-apps/cli/tauri.js');
function generate(name, svg, sizes = []) {
  const input = join(temporary, `${name}.svg`);
  const output = join(temporary, name);
  writeFileSync(input, svg);
  execFileSync(process.execPath, [cli, 'icon', input, '-o', output, ...sizes.flatMap(size => ['-p', String(size)])], { stdio: 'inherit' });
  return output;
}
function copyPng(directory, size, destination) {
  const file = join(directory, `${size}x${size}.png`);
  const bytes = readFileSync(file);
  assert.equal(bytes.subarray(0, 8).toString('hex'), '89504e470d0a1a0a');
  assert.equal(bytes.readUInt32BE(16), size);
  assert.equal(bytes.readUInt32BE(20), size);
  copyFileSync(file, join(icons, destination));
}
function orderedIcns(bytes) {
  assert.equal(bytes.subarray(0, 4).toString(), 'icns');
  assert.equal(bytes.readUInt32BE(4), bytes.length);
  const chunks = [];
  for (let at = 8; at < bytes.length;) {
    const size = bytes.readUInt32BE(at + 4);
    assert(size >= 8 && at + size <= bytes.length);
    chunks.push(bytes.subarray(at, at + size));
    at += size;
  }
  // Tauri emits identical ICNS images in random map order; sort their type tags.
  chunks.sort((a, b) => Buffer.compare(a.subarray(0, 4), b.subarray(0, 4)));
  assert.equal(new Set(chunks.map(chunk => chunk.subarray(0, 4).toString())).size, chunks.length);
  return Buffer.concat([bytes.subarray(0, 8), ...chunks]);
}
try {
  const platform = generate('platform', tile);
  writeFileSync(join(icons, 'icon.icns'), orderedIcns(readFileSync(join(platform, 'icon.icns'))));
  copyFileSync(join(platform, 'icon.ico'), join(icons, 'icon.ico'));
  assert.equal(readFileSync(join(icons, 'icon.ico')).subarray(0, 4).toString('hex'), '00000100');
  const pngs = generate('png', tile, [32, 64, 128, 256, 512, 1024]);
  for (const [size, name] of [[32, '32x32.png'], [64, '64x64.png'], [128, '128x128.png'], [256, '128x128@2x.png'], [512, 'icon.png'], [1024, 'app-icon.png']]) copyPng(pngs, size, name);
  const templates = generate('tray', tray, [44, 88]);
  copyPng(templates, 44, 'tray.png');
  copyPng(templates, 88, 'tray@2x.png');
  mkdirSync(join(root, 'ui/public/assets'), { recursive: true });
  writeFileSync(join(root, 'ui/public/assets/favicon.svg'), tile);
  const splash = join(app, 'splash/index.html');
  const html = readFileSync(splash, 'utf8');
  const marker = /<!-- brand:start -->[\s\S]*?<!-- brand:end -->/;
  assert.match(html, marker);
  writeFileSync(splash, html.replace(marker, `<!-- brand:start -->\n${source.trim()}\n<!-- brand:end -->`));
  console.log('Generated favicon, splash, PNG, ICO, ICNS and monochrome tray assets.');
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
