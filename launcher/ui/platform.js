// Leyline Launcher: what the launcher runs on (docs/ANDROID.md). The core's `platform` command answers (lib.rs platform():
// mobile.rs on a phone, app.rs on a computer); an older core, or a preview page with no core, counts as a computer.
//
// On a phone: <html> gets the class "phone" and data-platform="android" (phone.css lays the launcher out for a phone and
// hides everything that needs a computer), the page may draw under the system bars (viewport-fit=cover; phone.css keeps
// clear of them with the safe-area insets), and "leyline:platform" is sent once phone.js and the others can act on it.
//
//   window.LeylinePlatform = { os, mobile, node, ready }   ready: a promise of the same three, once the core answered
//
// Loaded first, right after addresses.js, so the class is on before the other scripts lay anything out.
(function () {
  "use strict";
  const desktop = { os: "desktop", mobile: false, node: true };
  const P = (window.LeylinePlatform = Object.assign({}, desktop));
  const invoke = window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke;

  function apply(p) {
    const os = typeof p.os === "string" && /^[a-z]{2,12}$/.test(p.os) ? p.os : "desktop";
    Object.assign(P, { os, mobile: !!p.mobile, node: p.node !== false && !p.mobile });
    const root = document.documentElement;
    root.dataset.platform = os;
    root.classList.toggle("phone", P.mobile);
    if (P.mobile) {
      const vp = document.querySelector('meta[name="viewport"]');
      if (vp && !/viewport-fit/.test(vp.content)) vp.content += ", viewport-fit=cover";
    }
    document.dispatchEvent(new CustomEvent("leyline:platform", { detail: { os: P.os, mobile: P.mobile, node: P.node } }));
    return { os: P.os, mobile: P.mobile, node: P.node };
  }

  P.ready = (async () => {
    if (!invoke) return apply(desktop);
    try { return apply((await invoke("platform")) || desktop); }
    catch (_) { return apply(desktop); }   // an older core: a computer
  })();
})();
