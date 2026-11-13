// One wording for logging in, everywhere in the launcher's own words (audit 12 Nov: the title bar said "Log In" beside
// Settings' "Log in", and some lines said "Sign in"): "Log in" as the verb, "signed in" only as a state. Reads the UI's
// strings (comments left out), no browser.
//
//   node --test launcher/tests/wording.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const UI = path.join(__dirname, '..', 'ui');
function words(f) {
  let t = fs.readFileSync(path.join(UI, f), 'utf8');
  if (f.endsWith('.html')) return t.replace(/<!--[\s\S]*?-->/g, '');
  t = t.replace(/\/\*[\s\S]*?\*\//g, '');
  return t.split('\n').map((l) => l.replace(/(^|[^:"'`\\])\/\/.*$/, '$1')).join('\n');
}

test('"Log in", never "Log In" or "Sign in", in what players read', () => {
  const bad = [];
  for (const f of fs.readdirSync(UI).filter((x) => /\.(js|html)$/.test(x))) {
    words(f).split('\n').forEach((l, i) => {
      if (/\bLog In\b|\bSign [iI]n\b|\bSigning in\b/.test(l)) bad.push(`${f}:${i + 1}: ${l.trim().slice(0, 120)}`);
    });
  }
  assert.deepEqual(bad, []);
});
