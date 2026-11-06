// The race/class switches in the page env (docs/RACE-CLASS-COMBOS.md), in node: /__env.js as the portal writes it
// (stream/wrath_portal.py env_js, for the realm's "all" preset), run with different page addresses. The env object
// outranks the query string in the client (benilla_app::webenv), so the env's own copy step is what lets
// ?moderncombos=0 turn the extra pairs off.
//
//   node --test tests/combos-env.page.test.cjs          (from the stream folder)
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const vm = require('node:vm');
const { execFileSync } = require('node:child_process');

const PORTAL = path.join(__dirname, '..', 'wrath_portal.py');

// /__env.js from the portal itself, with the realm's preset file saying `preset`.
function envJs(preset, sess = { user: 'WR1', pass: 'pw', slice: 'mini', tier: 2, mode: 'stream' }) {
  const data = fs.mkdtempSync(path.join(os.tmpdir(), 'combos-env-'));
  fs.mkdirSync(path.join(data, 'leyline'));
  fs.writeFileSync(path.join(data, 'leyline', 'race-class-combos.txt'), preset + '\n');
  const py = [
    'import importlib.util, json, sys',
    'spec = importlib.util.spec_from_file_location("wp", sys.argv[1])',
    'wp = importlib.util.module_from_spec(spec); spec.loader.exec_module(wp)',
    'sys.stdout.write(wp.env_js(json.loads(sys.argv[2])).decode())',
  ].join('\n');
  const env = Object.assign({}, process.env, {
    SOAP_USER: 'WUSOAP', SOAP_PASS: 'x', AC_SOAP_URL: 'http://127.0.0.1:9/', PORTAL_URL: 'http://127.0.0.1:9',
    WU_STREAM: '0', STREAM_HOST: 'wrath.example', PUBLIC_HOST: 'play.example', WU_DL_PROBE: '0', WU_PORTAL_DATA: data,
    WU_RELAY_LISTING: 'http://127.0.0.1:9/api/listing/realms.json', WU_ART_DIR: path.join(data, 'art'),
  });
  delete env.WU_RACE_CLASS_COMBOS;
  return execFileSync('python3', ['-I', '-c', py, PORTAL, JSON.stringify(sess)], { encoding: 'utf8', env });
}

// What the client reads (webenv::var: the env object first) after the page ran /__env.js at `search`.
function run(js, search) {
  const win = { location: { search } };
  win.window = win;
  vm.runInNewContext(js, Object.assign(win, { URLSearchParams }));
  return win.__wenilla_env;
}

const ALL = envJs('all');
const MODERN = envJs('modern');

test('the realm\'s preset turns the pairs on with no switch on the address', () => {
  assert.equal(run(MODERN, '').moderncombos, '1');
  assert.equal(run(MODERN, '').allcombos, undefined);
  const e = run(ALL, '');
  assert.equal(e.moderncombos, '1');
  assert.equal(e.allcombos, '1');
});

test('?moderncombos=0 turns both off; ?allcombos=0 keeps the modern pairs', () => {
  const off = run(ALL, '?moderncombos=0');
  assert.equal(off.moderncombos, '0');
  assert.equal(off.allcombos, '0');
  const modern = run(ALL, '?allcombos=0&cinematics=1');
  assert.equal(modern.moderncombos, '1');
  assert.equal(modern.allcombos, '0');
  assert.equal(run(MODERN, '?foreverui=0').foreverui, '0', 'the Forever UI switch still works beside them');
});

test('a community realm gets none, but the page can still ask', () => {
  const hop = envJs('all', { user: 'WR1', pass: 'pw', slice: 'mini', tier: 2, mode: 'stream',
                             hop: { id: 'abcdefghijklmnop', name: 'Frost', home: false, pre: true } });
  const plain = run(hop, '');
  assert.equal(plain.moderncombos, undefined);
  assert.equal(plain.allcombos, undefined);
  assert.equal(run(hop, '?moderncombos=1').moderncombos, '1');
});

test('a broken address never stops the page', () => {
  const js = MODERN;
  const win = { location: { get search() { throw new Error('no'); } } };
  win.window = win;
  assert.doesNotThrow(() => vm.runInNewContext(js, Object.assign(win, { URLSearchParams })));
  assert.equal(win.__wenilla_env.moderncombos, '1');
});
