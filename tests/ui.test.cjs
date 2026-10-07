const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { join } = require('node:path');
const { test } = require('node:test');
const vm = require('node:vm');

const source = readFileSync(join(__dirname, '../antiquota/ui/app.js'), 'utf8').split('async function api(')[0];

function page({ native = true, embed = false, zoom = 1.25 } = {}) {
  const calls = [], saved = new Map(), properties = new Map();
  const tauri = native ? { webview: { getCurrentWebview: () => ({ setZoom: z => {
    calls.push(z);
    return Promise.resolve();
  } }) } } : null;
  const parent = { __TAURI__: tauri, setZoom: z => calls.push(['parent', z]), zoomBy: d => calls.push(['delta', d]) };
  const root = { classList: { add() {} }, style: { setProperty: (k, v) => properties.set(k, v) } };
  const context = vm.createContext({
    window: { __TAURI__: tauri }, parent, location: { search: embed ? '?embed=1' : '' }, URLSearchParams,
    Element: class {}, DocumentFragment: class {},
    document: { documentElement: root, querySelector: () => ({ content: 'test' }) },
    localStorage: { getItem: k => k === 'antiquota.prefs' ? JSON.stringify({ zoom }) : null,
      setItem: (k, v) => saved.set(k, v) },
    toast() {}, fail: assert.fail,
  });
  vm.runInContext(source, context);
  return { context, calls, saved, properties, root };
}

test('desktop zoom uses the webview once and persists each supported scale', () => {
  const p = page();
  assert.deepEqual(p.calls, [1.25]);
  for (const z of [0.8, 1, 1.25, 1.5]) {
    p.context.setZoom(z);
    assert.equal(p.calls.at(-1), z);
    assert.equal(JSON.parse(p.saved.get('antiquota.prefs')).zoom, z);
    assert.equal(p.properties.get('--ui-zoom'), z);
    assert.equal(p.root.style.zoom, undefined);
  }
  p.context.zoomBy(-0.1);
  assert.equal(p.calls.at(-1), 1.4);
  p.context.zoomBy(0);
  assert.equal(p.calls.at(-1), 1);
});

test('embedded sessions inherit zoom and delegate controls to their parent', () => {
  const p = page({ embed: true });
  assert.deepEqual(p.calls, []);
  assert.equal(p.saved.size, 0);
  p.context.setZoom(0.8);
  p.context.zoomBy(0.1);
  assert.deepEqual(p.calls, [['parent', 0.8], ['delta', 0.1]]);
  assert.equal(p.root.style.zoom, undefined);
});

test('browser layout leaves scaling to browser zoom', () => {
  const p = page({ native: false });
  p.context.setZoom(1.5);
  assert.deepEqual(p.calls, []);
  assert.equal(p.saved.size, 0);
  assert.equal(p.root.style.zoom, undefined);
});

test('invalid and extreme desktop scales stay within the supported range', () => {
  const p = page({ zoom: 'invalid' });
  assert.deepEqual(p.calls, [1]);
  p.context.setZoom(10);
  assert.equal(p.calls.at(-1), 1.6);
  p.context.setZoom(0);
  assert.equal(p.calls.at(-1), 0.7);
});
