// The launcher's dormant server code (ui/lite.js SERVER_FEATURES, src-tauri/src/addresses.rs; docs/LAUNCHER-LITE.md):
// Leyline has no server since 26 Nov 2026, so the shipped app runs with the switch off. A test file of a part that
// needed the server (the sign-in, accounts, the forum, the official realm, hosting through the relay…) requires this
// first: every page it opens runs with the switch on (window.__LEYLINE_SERVER_FEATURES__), so the code that could come
// back stays tested. The lite app itself is tested in lite.browser.test.cjs.
//
//   require('./fixtures/server-on.cjs');   // before the tests open a page
'use strict';
const pw = require('playwright');
const ON = () => { window.__LEYLINE_SERVER_FEATURES__ = true; };
if (!pw.chromium.__serverOn) {
  const launch = pw.chromium.launch.bind(pw.chromium);
  pw.chromium.launch = async (...a) => {
    const b = await launch(...a);
    const newContext = b.newContext.bind(b);
    b.newContext = async (...o) => { const c = await newContext(...o); await c.addInitScript(ON); return c; };
    const newPage = b.newPage.bind(b);
    b.newPage = async (...o) => { const p = await newPage(...o); await p.addInitScript(ON); return p; };
    return b;
  };
  pw.chromium.__serverOn = true;
}
module.exports = { ON };
