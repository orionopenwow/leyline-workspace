// ui/addresses.js (window.LeylineAddresses) says the same names as tools/domain/addresses.json, the one list
// (docs/DOMAIN-MOVE.md).   node launcher/tests/addresses.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const ROOT = path.join(__dirname, '..', '..');
const win = {};
vm.runInNewContext(fs.readFileSync(path.join(ROOT, 'launcher', 'ui', 'addresses.js'), 'utf8'), { window: win });
const A = win.LeylineAddresses;
const J = JSON.parse(fs.readFileSync(path.join(ROOT, 'tools', 'domain', 'addresses.json'), 'utf8'));

test('the names match the one list', () => {
  assert.equal(A.SITE_HOST, J.site.new);
  assert.equal(A.OLD_SITE_HOST, J.site.old);
  assert.equal(A.GAME_HOST, J.game.new);
  assert.equal(A.OLD_GAME_HOST, J.game.old);
  assert.equal(A.RELAY_HOST, J.relay.new);
  assert.equal(A.OLD_RELAY_HOST, J.relay.old);
  assert.equal(A.site('/wrath'), 'https://' + J.site.new + '/wrath');
});

test('both names count', () => {
  assert.ok(A.isOfficialRealm('play.openvanilla.app') && A.isOfficialRealm('LeylineLauncher.com:3724'));
  assert.ok(!A.isOfficialRealm('relay.leylinelauncher.com:40002'));
  assert.ok(A.isSiteHost('leylinelauncher.com') && A.isSiteHost('play.openvanilla.app') && !A.isSiteHost('evil.example'));
  assert.equal(A.oldName('https://leylinelauncher.com/wrath/art/x.webp'), 'https://play.openvanilla.app/wrath/art/x.webp');
  assert.equal(A.oldName('https://elsewhere.example/x'), 'https://elsewhere.example/x');
  assert.ok(Object.isFrozen(A));
});
