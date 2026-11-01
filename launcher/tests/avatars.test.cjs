// The player portraits (ui/avatars.js, ui/art/avatars/): every listed portrait ships as a small SVG with no script, the
// list matches the files and the website's copies, and the default from a name is the website's (stream/profiles.py
// default_portrait: the same FNV-1a, the same vectors as stream/tests/test_profiles.py).
//
//   node --test launcher/tests/avatars.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const UI = path.join(__dirname, '..', 'ui');
const SITE = path.join(__dirname, '..', '..', 'stream', 'www-files', 'art', 'avatars');
const win = {};
vm.runInNewContext(fs.readFileSync(path.join(UI, 'avatars.js'), 'utf8'), { window: win, TextEncoder });
const A = win.LeylineAvatars;

test('24 portraits, each a small SVG without script, the same file on the website', () => {
  const ids = JSON.parse(JSON.stringify(A.list().map((x) => x[0])));   // out of the vm's realm
  assert.equal(ids.length, 24);
  assert.equal(new Set(ids).size, 24);
  assert.deepEqual(fs.readdirSync(path.join(UI, 'art', 'avatars')).filter((f) => f.endsWith('.svg')).sort(), ids.map((i) => i + '.svg').sort());
  for (const id of ids) {
    const body = fs.readFileSync(path.join(UI, 'art', 'avatars', id + '.svg'), 'utf8');
    assert.ok(body.startsWith('<svg') && !/<script|on[a-z]+=|href=/i.test(body), id);
    assert.ok(body.length < 4096, `${id}: ${body.length} bytes`);
    assert.equal(body, fs.readFileSync(path.join(SITE, id + '.svg'), 'utf8'), id + ' matches the website copy');
  }
});

test('the default from a name: stable, the website\'s, never a letter; old shape ids map to portraits', () => {
  for (const [name, id] of [['Groenz', 'monk'], ['groenz ', 'monk'], ['Alice', 'wolf'], ['Orion', 'brute'], ['Bob', 'warrior'],
    ['Ünïcode', 'wanderer'], ['', 'wanderer'], ['?', 'wanderer']]) assert.equal(A.pick(name), id, name);
  assert.equal(A.known('star'), 'wizard');
  assert.equal(A.known('nope'), '');
  assert.equal(A.src('../x'), 'art/avatars/wanderer.svg');
  assert.equal(A.fromUrl('https://play.openvanilla.app/wrath/avatar/p/moon.svg'), 'witch');
  assert.equal(A.fromUrl('https://play.openvanilla.app/wrath/avatar/u/aaaaaaaaaa.svg?v=2'), '');
});
