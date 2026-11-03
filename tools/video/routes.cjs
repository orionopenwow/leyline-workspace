// What the launcher fetches from the site, answered from the repo's own art (nothing else leaves the machine)
'use strict';
const path = require('node:path');
const { ROOT, UI } = require('./lib.cjs');
const ART = path.join(ROOT, 'stream', 'www-files', 'art');
// the Home gallery's pictures (the site's home.json): the owner's screenshots in stream/www-files/art
const HOME = { slides: { hangout: [{ file: 'leyline-shot-dalaran.webp', alt: '', credit: '' }], host: [{ file: 'leyline-shot-city.webp', alt: '', credit: '' }],
  browser: [{ file: 'leyline-shot-start.webp', alt: '', credit: '' }], news: [{ file: 'leyline-shot-coa-ultrahd.webp', alt: '', credit: '' }] } };
async function routes(page) {
  await page.route(/^https:\/\//, (r) => {
    const u = r.request().url();
    if (/\/wrath\/art\/home\/home\.json$/.test(u)) return r.fulfill({ contentType: 'application/json', headers: { 'Access-Control-Allow-Origin': '*' }, body: JSON.stringify(HOME) });
    const m = /\/wrath\/art\/home\/([a-z0-9-]+\.webp)$/.exec(u);
    if (m) return r.fulfill({ path: path.join(ART, m[1]), contentType: 'image/webp' });
    if (/leyline-bg|backdrop/.test(u)) return r.fulfill({ path: path.join(UI, 'art', 'leyline-bg.webp'), contentType: 'image/webp' });
    return r.abort();
  });
}
module.exports = { routes };
