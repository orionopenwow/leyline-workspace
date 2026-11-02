// Leyline's public addresses for the window (docs/DOMAIN-MOVE.md): the same names as src-tauri/src/addresses.rs and
// tools/domain/addresses.json. The new names come first; the old ones keep answering forever, so a saved address on an
// old name (a realm link, a picture, the official realm in settings) still counts as ours. window.LeylineAddresses.
(function () {
  "use strict";
  const SITE_HOST = "leylinelauncher.com";
  const OLD_SITE_HOST = "play.openvanilla.app";
  const A = {
    SITE_HOST, OLD_SITE_HOST,
    GAME_HOST: "play.leylinelauncher.com", OLD_GAME_HOST: "wrath.openvanilla.app",
    RELAY_HOST: "relay.leylinelauncher.com", OLD_RELAY_HOST: "relay.openvanilla.app",
    SITE: "https://" + SITE_HOST,
    OLD_SITE: "https://" + OLD_SITE_HOST,
    // the official realm (desktop realmlist) on either name
    OFFICIAL_REALMS: [SITE_HOST, OLD_SITE_HOST],
    site: (path) => "https://" + SITE_HOST + (path || ""),
    isSiteHost: (h) => [SITE_HOST, OLD_SITE_HOST].includes(String(h || "").toLowerCase()),
    isOfficialRealm: (a) => [SITE_HOST, OLD_SITE_HOST].includes(String(a || "").trim().toLowerCase().replace(/:3724$/, "")),
    // a picture on the site, on the old name when the new one does not load (one retry; the window's CSP allows both)
    oldName: (url) => String(url || "").replace(/^https:\/\/leylinelauncher\.com\//, "https://" + OLD_SITE_HOST + "/"),
  };
  window.LeylineAddresses = Object.freeze(A);
})();
