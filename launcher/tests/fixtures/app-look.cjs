// The launcher's whole look for a test page that draws one part on its own (friends.js, worldfriends.js…), so its
// screenshots show what the app shows: every stylesheet index.html links, in its order, served from launcher/ui (so
// the art they use, art/stone-side.webp and the rest, loads too), the Dark or Light base, and the part set in a page
// panel (.panel.wide) like the Single player page's.
//
//   const { appLook } = require('./fixtures/app-look.cjs');
//   await appLook(page, { root: '#auto', panel: 'spf' });   // after the part has drawn; before the screenshot
'use strict';
const fs = require('node:fs');
const path = require('node:path');

const UI = path.join(__dirname, '..', '..', 'ui');
const ORIGIN = 'http://leyline-ui.test/';
const TYPES = { '.css': 'text/css', '.webp': 'image/webp', '.png': 'image/png', '.svg': 'image/svg+xml', '.woff2': 'font/woff2', '.jpg': 'image/jpeg' };

function sheets() {
  const html = fs.readFileSync(path.join(UI, 'index.html'), 'utf8');
  return [...html.matchAll(/<link rel="stylesheet" href="([^"]+)"/g)].map((m) => m[1]);
}

async function appLook(page, o = {}) {
  await page.route(ORIGIN + '**', (r) => {
    const rel = decodeURIComponent(new URL(r.request().url()).pathname).replace(/^\/+/, '');
    const file = path.join(UI, rel);
    if (!file.startsWith(UI) || !fs.existsSync(file) || !fs.statSync(file).isFile()) return r.fulfill({ status: 404, body: '' });
    return r.fulfill({ path: file, contentType: TYPES[path.extname(file)] || 'application/octet-stream' });
  });
  await page.evaluate(async ({ hrefs, origin, o }) => {
    const root = document.documentElement;
    root.dataset.theme = o.light ? 'light' : 'dark';
    root.dataset.base = o.light ? 'light' : 'dark';
    if (o.phone) root.classList.add('phone');
    await Promise.all(hrefs.map((h) => new Promise((ok) => {
      const l = document.createElement('link');
      l.rel = 'stylesheet';
      l.href = origin + h;
      l.onload = l.onerror = ok;
      document.head.append(l);
    })));
    document.body.classList.add('ly');
    Object.assign(document.body.style, { overflow: 'auto', height: 'auto', padding: '16px', backgroundImage: 'none' });
    // the part in a page panel, like on the Single player page (the Friends area is .panel.wide.spf)
    const el = o.root && document.querySelector(o.root);
    if (el) {
      let panel = el.closest('.panel');
      if (el.matches('.spf') || !o.panel) panel = panel || el;   // the Friends area is the panel itself
      if (!panel) {
        const had = document.activeElement;   // moving a part drops its focus: put it back
        panel = document.createElement('div');
        el.replaceWith(panel);
        panel.append(el);
        if (had && had !== document.body && had.isConnected) had.focus();
      }
      panel.classList.add('panel', 'wide', ...(o.panel ? [o.panel] : []));
      panel.style.maxWidth = (o.width || 820) + 'px';
    }
    await document.fonts.ready;
  }, { hrefs: sheets(), origin: ORIGIN, o });
  await page.waitForTimeout(150);
}

module.exports = { appLook, sheets };
