// The browser game's page files (www-files/*.js, *.html) never put a game brand word or a client version / build number
// in a sentence a player reads (docs/WORDS.md; the same rule as tools/audit/common.py BANNED and VERSION). Build
// badges, data fields and Lua/XML the game runs are not sentences: a string counts as prose when it holds two words
// with a space between. Comments are skipped.
//
//   node --test tests/prose.page.test.cjs          (from the stream folder)
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const ROOT = path.join(__dirname, '..', 'www-files');
const BANNED = /world of warcraft|\bwow\b|warcraft|lich king|blizzard/i;
const VERSION = /\b(?:3\.3\.5a?|1\.12(?:\.[0-9])?|2\.4\.3|[0-9]\.[0-9]{1,2}\.[0-9][a-z]?|12340|5875|8606|15595|18414)\b/;
// Known and left to the owner (see /home/claude/review/stream.md): chain.js's refusals name the folder a player
// dropped and the one the page needs (the tests in ownfiles-layer.test.cjs pin that wording); wu-addons-core.js's
// .toc check is for addon authors (## Interface must be 30300).
const OWNER = [/^chain\.js:/, /^wu-addons-core\.js:/];

function prose(file) {
  const out = [];
  const lines = fs.readFileSync(path.join(ROOT, file), 'utf8').split('\n');
  let block = false;
  lines.forEach((line, i) => {
    let code = line;
    if (block) { const e = code.indexOf('*/'); if (e < 0) return; code = code.slice(e + 2); block = false; }
    code = code.replace(/\/\*.*?\*\//g, '');
    const s = code.indexOf('/*');
    if (s >= 0) { block = true; code = code.slice(0, s); }
    code = code.replace(/^\s*\/\/.*$/, '').replace(/\s\/\/\s.*$/, '').replace(/^\s*--.*$/, '');
    const re = /(['"`])((?:\\.|(?!\1).)*)\1/g;
    let m;
    while ((m = re.exec(code))) {
      const str = m[2];
      if (!/[A-Za-z]{2,} [A-Za-z]{2,}/.test(str)) continue;
      if (BANNED.test(str) || VERSION.test(str)) out.push(`${file}:${i + 1}: ${str.slice(0, 160)}`);
    }
  });
  return out;
}

test('no brand word or client version in the page files\' player sentences', () => {
  const files = fs.readdirSync(ROOT).filter((f) => /\.(js|html)$/.test(f));
  assert.ok(files.length > 10, 'the page files are there');
  const found = files.flatMap(prose);
  const owner = found.filter((l) => OWNER.some((r) => r.test(l)));
  const bad = found.filter((l) => !owner.includes(l));
  if (owner.length) console.log(`# left to the owner (${owner.length}):\n#   ` + owner.join('\n#   '));
  assert.deepEqual(bad, []);
});

test('the gamedata-off note and the game-folder card say "game folder" without a version', () => {
  const early = fs.readFileSync(path.join(ROOT, 'wrath-early.js'), 'utf8');
  assert.match(early, /This realm needs your own game folder\./);
  const note = early.slice(early.indexOf('p.textContent = '), early.indexOf("var a = document.createElement('a')"));
  const lines = note.match(/'[^']{20,}'/g) || [];
  assert.ok(lines.length >= 2, 'the computer and the phone wording');
  for (const l of lines) assert.doesNotMatch(l, VERSION);
  assert.match(note, /works on a computer/, 'a phone is told plainly that the folder needs a computer');
  const page = fs.readFileSync(path.join(ROOT, 'wrath-page.js'), 'utf8');
  assert.match(page, /label: 'Classic' \}/, 'the dock\'s Interface menu: Forever UI or Classic');
});
