// Leyline Launcher: the guide. Two small things for new players, both shown once and kept in the launcher's own
// settings (src-tauri settings.rs Settings::seen, through the core's guide_seen; never localStorage):
//   * the first-run tour: 3 or 4 short steps pointing at the real controls (Play Wrath Unleashed, Your Den, Host a
//     server, and Find your game files when no game folder is added yet). Skippable at every step; Help › Show me
//     again runs it any time.
//   * encouragement moments: one short line in a small toast, the first time something good happens (your first
//     server is live, a friend joins it, your first mod goes in). Each once, never twice. The in-game ones (first
//     time in your Den, first piece, first save, first grow) are Den Lite's (labs/den-lite/src/cheers.rs).
// An older core (or a preview page) without Settings::seen shows neither, so nothing appears unasked.
// The other scripts call window.LeylineGuide.moment(key) where the thing happens (app.js, hostconsole.js, den.js).
// Loaded last (after app.js: window.LeylineApp, window.LeylineNav).
"use strict";
(() => {
  // ---------------------------------------------------------------- the words: one list, the owner edits them here
  // TODO(owner): a voice pass on every line below (the tour and the moments), his words
  const TOUR = [
    { key: "play", target: "#play-go", fallback: "#pb-realm-btn", title: "Play Wrath Unleashed",
      text: "The official realm: hang out, level up, meet people. Press PLAY, or pick Play in your browser in the realm list." },
    { key: "den", target: ".nav-den", title: "Your Den",
      text: "Your own small white room, on this PC. Build in it, save it, grow it. Invite a friend when you're ready." },
    { key: "host", target: '.side .nav[data-page="host"]', title: "Host a server",
      text: "Run your own realm from a template, in one click. Friends join with your invite link." },
    { key: "files", target: "#pb-client", title: "Find your game files", needsNoClient: true,
      text: "Leyline plays with a game folder you already have. Add it here, or see Find your game files for where it usually is." },
  ];
  const MOMENTS = {
    "cheer-hosted": { line: "You're live — share this link and friends can join.", action: "Copy invite link" },
    "cheer-friend": { line: "A friend just joined your server. Say hello!" },
    "cheer-mod": { line: "Your first mod is in. It's there the next time you press PLAY." },
  };
  const SKIP = "Skip tour", NEXT = "Next", DONE = "Done";

  // ---------------------------------------------------------------- the core
  const call = window.__TAURI__ && window.__TAURI__.core
    ? window.__TAURI__.core.invoke
    : async () => { throw "This is a preview page: open it in the Leyline app."; };
  const byId = (id) => document.getElementById(id);
  const mk = (tag, cls, text) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  };
  let seen = null;   // the keys already shown (Settings::seen); null: unknown, so nothing shows
  const app = () => (window.LeylineApp && window.LeylineApp.state ? window.LeylineApp.state() : null);
  function readSeen() {
    const s = app();
    if (s && s.settings && Array.isArray(s.settings.seen)) seen = s.settings.seen.slice();
    return seen;
  }
  function mark(key) {
    if (!seen) return;
    if (!seen.includes(key)) seen.push(key);
    const s = app();
    if (s && s.settings && Array.isArray(s.settings.seen) && !s.settings.seen.includes(key)) s.settings.seen.push(key);
    call("guide_seen", { key }).catch((e) => console.warn("[leyline] guide", e));
  }

  // ---------------------------------------------------------------- the tour
  let tour = null;   // { steps, i, card, ring }
  const visible = (el) => { if (!el) return false; const r = el.getBoundingClientRect(); return r.width > 0 && r.height > 0; };
  function find(step) {
    const a = document.querySelector(step.target);
    if (visible(a)) return a;
    const b = step.fallback && document.querySelector(step.fallback);
    return visible(b) ? b : null;
  }
  function steps() {
    const s = app();
    const noClient = !s || !s.settings || !(s.settings.clients || []).length;
    return TOUR.filter((t) => (!t.needsNoClient || noClient) && find(t));
  }
  function place() {
    if (!tour) return;
    const step = tour.steps[tour.i];
    const el = find(step);
    const { card, ring } = tour;
    if (!el) { ring.hidden = true; return; }
    const r = el.getBoundingClientRect();
    const pad = 4;
    Object.assign(ring.style, { left: `${r.left - pad}px`, top: `${r.top - pad}px`, width: `${r.width + pad * 2}px`, height: `${r.height + pad * 2}px` });
    ring.hidden = false;
    const W = window.innerWidth, H = window.innerHeight, gap = 14;
    const cw = card.offsetWidth, ch = card.offsetHeight;
    let x, y, side;
    if (el.closest(".side") && r.right + gap + cw <= W - 8) { x = r.right + gap; y = r.top + r.height / 2 - ch / 2; side = "right"; }   // the side menu
    else { x = r.left + r.width / 2 - cw / 2; y = r.top - gap - ch; side = "above"; }                                                // the play bar
    if (y + ch > H - 8) y = H - 8 - ch;
    x = Math.max(8, Math.min(x, W - 8 - cw));
    y = Math.max(8, y);
    card.style.left = `${Math.round(x)}px`;
    card.style.top = `${Math.round(y)}px`;
    card.dataset.side = side;
  }
  function paintStep() {
    const step = tour.steps[tour.i];
    const { card } = tour;
    card.dataset.step = step.key;
    byId("gd-tour-n").textContent = `${tour.i + 1} of ${tour.steps.length}`;
    byId("gd-tour-title").textContent = step.title;
    byId("gd-tour-text").textContent = step.text;
    const next = byId("gd-tour-next");
    next.textContent = tour.i === tour.steps.length - 1 ? DONE : NEXT;
    place();
    next.focus({ preventScroll: true });
  }
  function startTour() {
    if (tour) return;
    if (window.LeylineNav && window.LeylineNav.current().page !== "play") window.LeylineNav.show("play");
    const list = steps();
    if (!list.length) return;
    const ring = mk("div", "gd-ring");
    ring.id = "gd-ring";
    ring.setAttribute("aria-hidden", "true");
    const card = mk("div", "dlg gd-tour");
    card.id = "gd-tour";
    card.setAttribute("role", "dialog");
    card.setAttribute("aria-modal", "false");
    card.setAttribute("aria-labelledby", "gd-tour-title");
    card.setAttribute("aria-describedby", "gd-tour-text");
    const n = mk("span", "gd-tour-n"); n.id = "gd-tour-n";
    const h = mk("h3", ""); h.id = "gd-tour-title";
    const p = mk("p", ""); p.id = "gd-tour-text";
    const row = mk("div", "row gd-tour-row");
    const skip = mk("button", "btn small", SKIP); skip.type = "button"; skip.id = "gd-tour-skip";
    const next = mk("button", "btn small primary", NEXT); next.type = "button"; next.id = "gd-tour-next";
    skip.addEventListener("click", () => endTour());
    next.addEventListener("click", () => { if (tour.i < tour.steps.length - 1) { tour.i++; paintStep(); } else endTour(); });
    row.append(skip, next);
    card.append(n, h, p, row);
    document.body.append(ring, card);
    tour = { steps: list, i: 0, card, ring };
    paintStep();
    window.addEventListener("resize", place);
    document.addEventListener("keydown", onKey, true);
  }
  function onKey(e) { if (e.key === "Escape" && tour) { e.preventDefault(); endTour(); } }
  function endTour() {
    if (!tour) return;
    tour.card.remove();
    tour.ring.remove();
    tour = null;
    window.removeEventListener("resize", place);
    document.removeEventListener("keydown", onKey, true);
    mark("tour");
    nextToast();
  }

  // ---------------------------------------------------------------- the moments: one small toast at a time
  const queue = [];
  let toast = null;
  let toastTimer = null;
  // quiet: the page already says it (the Host page's "You're live" box), so it only counts as shown
  function moment(key, quiet) {
    const m = MOMENTS[key] ? key : MOMENTS["cheer-" + key] ? "cheer-" + key : null;
    if (!m || !readSeen() || seen.includes(m) || queue.includes(m) || (toast && toast.dataset.key === m)) return false;
    mark(m);   // once, even if the window closes before it fades
    if (quiet) return false;
    queue.push(m);
    nextToast();
    return true;
  }
  function nextToast() {
    if (toast || tour || !queue.length) return;
    const key = queue.shift();
    const m = MOMENTS[key];
    toast = mk("div", "gd-toast");
    toast.id = "gd-toast";
    toast.dataset.key = key;
    toast.setAttribute("role", "status");
    const mark0 = mk("span", "gd-toast-ic");
    mark0.setAttribute("aria-hidden", "true");
    const line = mk("span", "gd-toast-t", m.line);
    toast.append(mark0, line);
    if (m.action && key === "cheer-hosted") {
      const b = mk("button", "btn small primary", m.action); b.type = "button"; b.id = "gd-toast-go";
      b.addEventListener("click", async () => {
        let link = null;
        try { link = await call("host_invite"); } catch (_) { /* no kit */ }
        if (!link) { line.textContent = "Your invite link is on the Host page."; return; }
        try { await navigator.clipboard.writeText(link); line.textContent = "Invite link copied. Send it to a friend."; }
        catch (_) { line.textContent = link; }
        b.remove();
        hold();
      });
      toast.append(b);
    }
    const x = mk("button", "gd-toast-x", "×"); x.type = "button"; x.setAttribute("aria-label", "Close");
    x.addEventListener("click", () => closeToast());
    toast.append(x);
    toast.addEventListener("mouseenter", () => clearTimeout(toastTimer));
    toast.addEventListener("mouseleave", hold);
    toast.addEventListener("focusin", () => clearTimeout(toastTimer));
    document.body.append(toast);
    hold();
  }
  function hold() { clearTimeout(toastTimer); toastTimer = setTimeout(closeToast, 9000); }
  function closeToast() {
    clearTimeout(toastTimer);
    if (toast) toast.remove();
    toast = null;
    setTimeout(nextToast, 400);
  }

  // ---------------------------------------------------------------- start: once the launcher has its state
  async function boot() {
    for (let i = 0; i < 50 && !app(); i++) await new Promise((r) => setTimeout(r, 100));
    if (!readSeen()) return;   // an older core: no guide
    if (!seen.includes("tour")) setTimeout(startTour, 500);
  }
  const again = byId("guide-again");
  if (again) again.addEventListener("click", () => { if (readSeen()) startTour(); });
  window.LeylineGuide = { moment, tour: startTour, endTour, seen: () => (seen ? seen.slice() : null), TOUR, MOMENTS };
  boot();
})();
