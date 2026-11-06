// ahead.js: the terrain look-ahead's budget follows the line (review 4 Nov: 100 MB even on mobile data), and
// sw-register.js's status() drops its listener on a timeout. Run: node stream/tests/ahead-budget.test.cjs
const fs = require('fs'), os = require('os'), path = require('path');
const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'ahead-'));
const www = path.join(__dirname, '..', 'www-files');
fs.writeFileSync(path.join(dir, 'ahead.mjs'), fs.readFileSync(path.join(www, 'ahead.js'), 'utf8'));
let fails = 0;
const ok = (c, m, x) => { console.log((c ? 'PASS ' : 'FAIL ') + m + (c || x === undefined ? '' : ' ' + JSON.stringify(x))); if (!c) fails++; };
(async () => {
  const { aheadBudget } = await import('file://' + path.join(dir, 'ahead.mjs'));
  const MB = 1024 * 1024;
  ok(aheadBudget(undefined) === 100 * MB, 'no connection info: 100 MB');
  ok(aheadBudget({ type: 'wifi', effectiveType: '4g' }) === 100 * MB, 'wifi: 100 MB');
  ok(aheadBudget({ type: 'cellular', effectiveType: '4g' }) === 20 * MB, 'mobile data on 4g: 20 MB');
  const sw = fs.readFileSync(path.join(www, 'sw-register.js'), 'utf8');
  const st = sw.slice(sw.indexOf('status: () =>'), sw.indexOf('clear: () =>'));
  ok((st.match(/removeEventListener\('message', ch\)/g) || []).length === 2, "status(): the listener is removed on an answer and on the timeout");
  process.exit(fails ? 1 : 0);
})();
