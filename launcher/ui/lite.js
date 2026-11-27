// Leyline as a launcher aggregator (owner, 26 Nov 2026: Leyline's only server is shut down for good; docs/LAUNCHER-LITE.md).
// The website comes back as a static site at the same address (GitHub Pages): its read-only files stay (the mods
// catalog, news, flags, modes, the realm list of other servers, the docs). Gone with the server: the sign-in (Hero Key,
// Google), accounts and profiles, the official realm, the browser game, the relay (community realms through it, invite
// links through it, the friends' mailbox, sharing a Den or a world over the internet), the forum, social and presence,
// the official leaderboards, the Den market, bug reports to the server, listing submissions and Survivors.
//
// One switch, SERVER_FEATURES (the same as src-tauri/src/addresses.rs): false, and every part of the window that needed
// the server is hidden (html.lite and [data-srv], lite.css) or skips its call; the code stays so it could come back.
// The tests of that dormant code turn it back on for their page only (window.__LEYLINE_SERVER_FEATURES__, set before
// this file runs); nothing in the app sets it. Loaded right after addresses.js, before every other script.
(function () {
  "use strict";
  const SERVER_FEATURES = false;
  const on = SERVER_FEATURES || window.__LEYLINE_SERVER_FEATURES__ === true;
  const L = {
    SERVER_FEATURES: on,
    lite: !on,
    // the Community card (Discord and Reddit are the allowed links). TODO(owner): Leyline's Reddit address; until it is
    // set, the card shows the Discord only (and the core's LINKS needs a "reddit" entry too: src-tauri/src/lib.rs)
    REDDIT: null,
    // Help › Found a bug?: the launcher's public repo's issues (the core's LINKS "issues")
    ISSUES: "https://github.com/orionopenwow/Leyline/issues",
  };
  // with the switch on (the tests of the dormant code), the lite-only parts are taken out, so the page is as it was
  if (on) document.addEventListener("DOMContentLoaded", () => document.querySelectorAll("[data-lite]").forEach((e) => e.remove()));
  if (!on) {
    document.documentElement.classList.add("lite");
    // the words that named what is gone, in the few inputs that had them (data-lite-placeholder)
    document.addEventListener("DOMContentLoaded", () => {
      document.querySelectorAll("[data-lite-placeholder]").forEach((i) => { i.placeholder = i.dataset.litePlaceholder; });
    });
  }
  window.LeylineLite = Object.freeze(L);
})();
