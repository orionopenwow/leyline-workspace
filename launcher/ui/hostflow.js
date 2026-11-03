// Leyline: the Host page's flow (owner 3 Nov: "Private / Public (as simple as that), then an extremely well thought out
// list of templates, a panel of sliders, a place to implement Claude into the server creation process, and a full guide;
// make this area way more streamlined"). Four steps on the Start tab while no realm is set up on this PC:
//   1. Private or Public (one line each), with one status line for the Host pack and Docker and its one fix button;
//   2. a template (ui/hostflow-catalog.js: Pure vanilla, Vanilla+, ... Blank), each with its pitch, key numbers and
//      "best for";
//   3. the settings panel (grouped sliders, switches and choices; a change shows "Custom (from X)"; a live summary;
//      save as my template; share as a code) with Ask Claude (the player's own assistant through the Agents page, or
//      a copy-paste prompt for any AI chat and a paste-back box: a JSON block of known keys, checked and clamped here
//      and again by the core);
//   4. Host (what happens, then the steps), then the running realm's card.
// Once a realm runs on this PC the console (hostconsole.js) shows, with this page's "Edit settings" bar on top.
// The core: src-tauri/src/host/flow.rs (host_flow_apply, host_flow_current, host_plan_ask, host_plan_take) and the
// usual host commands. Game modes, Fork and Play solo stay one click away (hostmodes.js, the More menu).
// Loaded after hostmodes.js: it puts itself on top of the Start tab on each "leyline:kit".
"use strict";
(() => {
  const C = window.LeylineHostCatalog;
  const call = window.__TAURI__ && window.__TAURI__.core
    ? window.__TAURI__.core.invoke
    : async () => { throw "This is a preview page: open it in the Leyline app."; };
  const oops = (e, fb) => (window.LeylineFriendly ? window.LeylineFriendly(e, fb) : (fb || "Something went wrong. Try again."));
  const byId = (id) => document.getElementById(id);
  const mk = (tag, cls, text) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  };
  const btn = (label, cls, on) => { const b = mk("button", cls || "btn small", label); b.type = "button"; if (on) b.addEventListener("click", on); return b; };
  const page = byId("page-host");
  const main = byId("hc-pane-main");
  const guidePane = byId("hc-pane-guide");
  if (!C || !page || !main) return;

  // ---------------------------------------------------------------- the catalog
  const S = Object.fromEntries(C.settings.map((s) => [s.key, s]));
  const W = Object.fromEntries(C.worlds.map((w) => [w.id, w]));
  const T = Object.fromEntries(C.templates.map((t) => [t.id, t]));
  const tplValues = (id) => Object.assign({}, C.base, (T[id] || {}).set || {});
  const NOT_FROM_AI = ["signup"];
  const NAME_OK = /^[A-Za-z0-9 _'-]{1,32}$/;
  const GM_OK = /^[A-Za-z0-9]{3,16}$/;
  const BRAND = /world\s*of\s*warcraft|warcraft|lich\s*king|lichking|blizzard|\bwow\b/i;
  const needsGm = !/Windows/i.test(navigator.userAgent || "");
  const SHARE = (rid) => window.LeylineAddresses.site(`/wrath/r/${rid}`);
  const SAVED_KEY = "leyline.hostTemplates";
  const CODE_PREFIX = "LLR1.";
  const store = {
    get: (k) => { try { return localStorage.getItem(k); } catch (_) { return null; } },
    set: (k, v) => { try { localStorage.setItem(k, v); } catch (_) { /* private window */ } },
  };

  // ---------------------------------------------------------------- our own icons (no game emblems)
  const svg = (inner, vb) => `<svg viewBox="${vb || "0 0 24 24"}" width="22" height="22" aria-hidden="true" focusable="false" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">${inner}</svg>`;
  const ICON = {
    private: svg('<path d="M4 11.5 12 5l8 6.5"/><path d="M6.5 10v9h11v-9"/><rect x="10" y="13" width="4" height="6" rx="1"/>'),
    public: svg('<circle cx="12" cy="12" r="2.2"/><path d="M7.8 7.8a6 6 0 0 0 0 8.4M16.2 7.8a6 6 0 0 1 0 8.4M5 5a10 10 0 0 0 0 14M19 5a10 10 0 0 1 0 14"/>'),
    "pure-vanilla": svg('<path d="M12 20V9"/><path d="M12 13c-4 0-6-3-6-7 4 0 6 3 6 7z"/><path d="M12 11c3 0 5-2 5-5-3 0-5 2-5 5z"/>'),
    "vanilla-plus": svg('<path d="M10 20V10"/><path d="M10 14c-3.5 0-5-2.6-5-6 3.5 0 5 2.6 5 6z"/><path d="M17 6v6M14 9h6"/>'),
    original: svg('<path d="M4 19c3-1 4-4 8-5s5-4 8-6"/><circle cx="4" cy="19" r="1.4"/><circle cx="12" cy="14" r="1.4"/><circle cx="20" cy="8" r="1.4"/>'),
    classic: svg('<path d="M5 19h14"/><path d="M8 19v-5M12 19V9M16 19V5"/>'),
    fun: svg('<path d="M12 3l2.2 5 5.3.5-4 3.6 1.2 5.3L12 14.8 7.3 17.4l1.2-5.3-4-3.6 5.3-.5z"/>'),
    rp: svg('<path d="M18 4c-6 1-10 6-11 13l-2 3"/><path d="M18 4c0 6-3 10-9 11"/><path d="M9 11h5"/>'),
    hardcore: svg('<path d="M12 20s-7-4.4-7-10a4 4 0 0 1 7-2.6A4 4 0 0 1 19 10c0 5.6-7 10-7 10z"/><path d="M12 10v4"/>'),
    "solo-bots": svg('<circle cx="12" cy="7" r="2.6"/><path d="M7.5 19a4.5 4.5 0 0 1 9 0"/><rect x="2.5" y="12" width="4" height="4" rx="1"/><rect x="17.5" y="12" width="4" height="4" rx="1"/>'),
    pvp: svg('<path d="M5 4l9 9M19 4l-9 9"/><path d="M4 15l5 5M20 15l-5 5"/><path d="M7 17l-2 2M17 17l2 2"/>'),
    "event-night": svg('<path d="M9 4h6M10 4v2M14 4v2"/><path d="M7 8h10l-1 10H8z"/><path d="M12 11v4"/>'),
    blank: svg('<rect x="5" y="5" width="14" height="14" rx="2" stroke-dasharray="3 2.4"/>'),
    mine: svg('<path d="M6 4h12v16l-6-4-6 4z"/>'),
    claude: svg('<path d="M12 3v4M12 17v4M3 12h4M17 12h4M5.6 5.6l2.8 2.8M15.6 15.6l2.8 2.8M5.6 18.4l2.8-2.8M15.6 8.4l2.8-2.8"/>'),
  };
  const icon = (k, cls) => { const s = mk("span", cls || "hst-ico"); s.innerHTML = ICON[k] || ICON.blank; return s; };

  // ---------------------------------------------------------------- the checks (host/flow.rs check_values, the same rules)
  const rawNumber = (v) => {
    const n = typeof v === "number" ? v : typeof v === "string" ? Number(v.trim().replace(/x$/i, "").trim()) : NaN;
    return Number.isFinite(n) && !(typeof v === "string" && v.trim() === "") ? n : null;
  };
  function snap(s, n) {
    const step = s.step || 1;
    const c = Math.min(Math.max(n, s.min), s.max);
    return Math.round(Math.min(s.min + Math.round((c - s.min) / step) * step, s.max) * 100) / 100;
  }
  const asToggle = (v) => {
    if (typeof v === "boolean") return v;
    if (v === 0 || v === 1) return v === 1;
    if (typeof v === "string") { const t = v.trim().toLowerCase(); if (["on", "yes", "true", "1"].includes(t)) return true; if (["off", "no", "false", "0"].includes(t)) return false; }
    return null;
  };
  function checkValues(start, given, fromAi) {
    const out = { values: Object.assign({}, start), changed: [], leftOut: [] };
    for (const [k, v] of Object.entries(given || {})) {
      const short = String(k).slice(0, 32);
      const s = S[k];
      if (!s) { out.leftOut.push(`${short}: not a setting`); continue; }
      if (fromAi && NOT_FROM_AI.includes(k)) { out.leftOut.push(`${short}: you choose who can join yourself`); continue; }
      if (s.kind === "range") {
        const raw = rawNumber(v);
        if (raw === null) { out.leftOut.push(`${s.label}: not a number`); continue; }
        const n = snap(s, raw);
        if (Math.abs(raw - n) > 1e-9) out.changed.push(`${s.label}: ${String(v).slice(0, 12)} to ${n} (its range is ${s.min} to ${s.max})`);
        out.values[k] = n;
      } else if (s.kind === "toggle") {
        const b = asToggle(v);
        if (b === null) out.leftOut.push(`${s.label}: on or off`); else out.values[k] = b;
      } else if (s.kind === "choice") {
        if (s.options.some((o) => o[0] === v)) out.values[k] = v; else out.leftOut.push(`${s.label}: not one of its choices`);
      } else out.leftOut.push(`${s.label}: coming, not yet`);
    }
    return out;
  }
  const applies = (s, v) => (!s.only || s.only.includes(v.world)) && (!s.needs || v[s.needs] === true);
  const whyNot = (s, v) => (s.only && !s.only.includes(v.world) ? `Not on "${W[v.world].label}"` : s.needs && v[s.needs] !== true ? `Needs: ${S[s.needs].label}` : "");

  // a chat answer's JSON block: a ```json fence first, else the outermost braces
  function jsonBlock(text) {
    if (typeof text !== "string" || text.length > 65536) return null;
    const parts = text.split("```");
    let cand = null;
    for (let i = 1; i < parts.length; i += 2) { const b = parts[i].replace(/^json/, "").trim(); if (b.startsWith("{")) { cand = b; break; } }
    if (!cand) { const a = text.indexOf("{"), z = text.lastIndexOf("}"); if (a < 0 || z <= a) return null; cand = text.slice(a, z + 1); }
    try { const v = JSON.parse(cand); return v && typeof v === "object" && !Array.isArray(v) ? v : null; } catch (_) { return null; }
  }
  function answerFrom(v) {
    if (!v || typeof v.settings !== "object" || !v.settings || Array.isArray(v.settings)) throw new Error('The answer has no "settings" in it.');
    if (Object.keys(v.settings).length > 64) throw new Error("Too many settings in that answer.");
    const known = typeof v.template === "string" && T[v.template];
    const t = known ? v.template : "classic";
    const c = checkValues(tplValues(t), v.settings, true);
    if (typeof v.template === "string" && !known) c.leftOut.unshift(`template ${v.template.slice(0, 24)}: not one of ours, so Classic levelling`);
    const w = {};
    for (const [k, line] of Object.entries(v.why && typeof v.why === "object" ? v.why : {}).slice(0, 64)) {
      if (typeof line !== "string" || !S[k]) continue;
      const l = line.split(/\s+/).filter(Boolean).join(" ");
      if (l && l.length <= 160 && !BRAND.test(l) && !/[\u0000-\u001f]/.test(l)) w[k] = l;
    }
    const name = typeof v.name === "string" && NAME_OK.test(v.name.trim()) && !BRAND.test(v.name) ? v.name.trim() : null;
    return { template: t, values: c.values, changed: c.changed, leftOut: c.leftOut, why: w, name };
  }
  function parseAnswer(text) {
    const v = jsonBlock(text);
    if (!v) throw new Error("No settings block in that answer: paste the whole reply, with its { } block.");
    return answerFrom(v);
  }

  // ---------------------------------------------------------------- share codes and my templates
  const b64url = (s) => btoa(unescape(encodeURIComponent(s))).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  const unb64url = (s) => decodeURIComponent(escape(atob(s.replace(/-/g, "+").replace(/_/g, "/"))));
  function diffOf(v, t) { const base = tplValues(t); const d = {}; for (const k of Object.keys(v)) if (v[k] !== base[k]) d[k] = v[k]; return d; }
  const shareCode = () => CODE_PREFIX + b64url(JSON.stringify({ t: baseTid(), v: diffOf(vals, baseTid()) }));
  function readCode(code) {
    const c = String(code || "").trim();
    if (!c.startsWith(CODE_PREFIX) || c.length > 4000) throw new Error("That isn't a realm settings code.");
    let doc;
    try { doc = JSON.parse(unb64url(c.slice(CODE_PREFIX.length))); } catch (_) { throw new Error("That code is damaged: copy it again."); }
    return answerFrom({ template: doc && doc.t, settings: (doc && doc.v) || {} });
  }
  function savedList() {
    try { const l = JSON.parse(store.get(SAVED_KEY) || "[]"); return Array.isArray(l) ? l.filter((x) => x && typeof x.id === "string" && T[x.base]).slice(0, 24) : []; } catch (_) { return []; }
  }
  const saveList = (l) => store.set(SAVED_KEY, JSON.stringify(l.slice(0, 24)));

  // ---------------------------------------------------------------- state
  let kit = null;          // the last "leyline:kit"
  let pk = null, dock = null;
  let vis = null;          // "private" | "public"
  let tid = null;          // a template id, or "mine:<id>"
  let vals = null;         // the panel's values
  let why = {};            // key -> a line from Claude
  let ownRules = [];       // the host's own house rules
  let signupTouched = false;
  let view = "flow";       // flow | run | edit | away (Game modes or the ruleset picker shows) | console
  let job = null, jobTimer = null, live = null, liveTimer = null, askTimer = null;
  let lastAnswerNote = null;
  let askOpen = false;     // Ask Claude's box stays open across repaints
  let askWords = "";       // and keeps what was typed
  let realmName = "";
  const ui = {};

  const baseTid = () => (tid && tid.startsWith("mine:") ? (savedList().find((x) => "mine:" + x.id === tid) || {}).base || "classic" : tid || "classic");
  const startValues = () => {
    if (tid && tid.startsWith("mine:")) { const m = savedList().find((x) => "mine:" + x.id === tid); if (m) return checkValues(tplValues(m.base), m.values, false).values; }
    return tplValues(tid || "classic");
  };
  const isCustom = () => !!vals && JSON.stringify(sorted(vals)) !== JSON.stringify(sorted(startValues()));
  const sorted = (o) => Object.fromEntries(Object.keys(o).sort().map((k) => [k, o[k]]));
  const tplName = () => { if (tid && tid.startsWith("mine:")) { const m = savedList().find((x) => "mine:" + x.id === tid); return m ? m.name : "My template"; } return (T[tid] || {}).name || ""; };
  const title = () => (isCustom() ? `Custom (from ${tplName()})` : tplName());

  const root = mk("section", "hst");
  root.id = "hst";
  root.setAttribute("aria-label", "Host a realm");
  // the page's status line; a line said just before a repaint shows once after it
  let flash = null;
  const say = (t, kind) => {
    flash = t ? [t, kind] : null;
    const s = root.querySelector(".hst-status-line");
    if (s) { s.textContent = t || ""; s.className = "status hst-status-line" + (kind ? " " + kind : ""); }
  };
  function statusLine() {
    const s = mk("p", "status hst-status-line" + (flash && flash[1] ? " " + flash[1] : ""), flash ? flash[0] : "");
    flash = null;
    return s;
  }

  // ---------------------------------------------------------------- where it sits
  function place() {
    const setup = !(kit && kit.realm);
    let want;
    if (view === "run") want = true;
    else if (view === "away") want = false;
    else if (setup) { if (view === "console" || view === "edit") view = "flow"; want = true; }
    else { if (view === "flow") view = "console"; want = true; }   // a realm here: the console, with the Edit settings bar
    main.classList.toggle("hst-on", want && view !== "console" && view !== "edit");
    main.classList.toggle("hst-edit", want && view === "edit");
    if (!want) { root.remove(); return; }
    if (main.firstChild !== root) main.prepend(root);
  }
  document.addEventListener("leyline:kit", (e) => {
    kit = e.detail || null;
    if (view === "flow" || view === "console" || view === "edit") { if (kit && kit.realm && view === "flow") view = "console"; }
    place();
    paint();
    loadReady();
  });

  // the play bar's My Den / Solo leveling asks the Host page for its ruleset (hostconsole.js applyWant): the ruleset
  // picker shows it with its own facts, so this flow steps aside
  document.addEventListener("leyline:host-ruleset", () => {
    if (!(kit && kit.realm) && view === "flow") away(() => window.LeylineHostModes && window.LeylineHostModes.rulesets());
  });

  // ---------------------------------------------------------------- the one status line: the Host pack and Docker
  const DOCK_FIX = { missing: "Get Docker", stopped: "Start Docker", desktop_stopped: "Start Docker", no_compose: "Update Docker", no_permission: "Let Docker in" };
  let dockTimer = null;
  async function loadReady() {
    const [p, d] = await Promise.all([call("hostpack_info").catch(() => null), call("docker_info").catch(() => null)]);
    pk = p; dock = d;
    paintReady();
    if (dock && dock.state !== "ready" && !dockTimer) dockTimer = setInterval(recheckDocker, 5000);
  }
  async function recheckDocker() {
    if (!page.classList.contains("on") || document.visibilityState === "hidden") return;
    let d = null;
    try { d = await call("docker_info"); } catch (_) { return; }
    dock = d;
    if (d && d.state === "ready") { clearInterval(dockTimer); dockTimer = null; }
    paintReady();
  }
  const haveKit = () => !!(kit && kit.is_kit);
  const dockOk = () => !dock || dock.state === "ready";
  function paintReady() {
    const box = ui.ready;
    if (!box) return;
    box.replaceChildren();
    const ok = haveKit() && dockOk();
    box.className = "hst-ready " + (ok ? "ok" : "warn");
    const dot = mk("span", "hst-dot");
    dot.setAttribute("aria-hidden", "true");
    let text, fix = null;
    if (!haveKit()) {
      const inst = pk && pk.installed;
      text = inst ? "The Host pack is downloaded: one click to use it." : "This PC needs the Host pack: your realm's software, one download, no game files.";
      fix = btn(inst ? "Use the Host pack" : "Get the Host pack", "btn small primary hst-fix", (ev) => getPack(ev.currentTarget, inst && pk.kit_dir));
      if (!inst && !(pk && pk.available)) fix.disabled = true;
    } else if (!dockOk()) {
      text = `Docker (free) runs your realm. ${dock.help || "It isn't answering yet."}`;
      if (dock.can_start) fix = btn("Start Docker", "btn small primary hst-fix", (ev) => startDocker(ev.currentTarget));
      else if (DOCK_FIX[dock.state] && dock.state !== "starting") fix = btn(DOCK_FIX[dock.state], "btn small hst-fix", () => call("open_link", { name: (dock.get_url || "").includes("engine") ? "dockerengine" : "docker" }).catch(() => {}));
    } else {
      const v = pk && pk.installed && pk.installed.version;
      text = `Ready to host on this PC: the Host pack${v ? " " + v : ""} and Docker are set.`;
    }
    box.append(dot, mk("span", "hst-ready-t", text));
    if (fix) { fix.id = "hst-fix"; box.append(fix); }
  }
  async function getPack(b, dir) {
    b.disabled = true;
    b.textContent = dir ? "Setting up…" : "Downloading…";
    try {
      if (!dir) { const r = await call("hostpack_install"); dir = r[1]; }
      await call("set_host_kit", { path: dir });
      if (typeof loadHost === "function") loadHost();   // app.js: re-read the kit ("leyline:kit")
    } catch (e) { say(oops(e), "bad"); b.disabled = false; b.textContent = "Try again"; }
  }
  async function startDocker(b) {
    b.disabled = true;
    try { say(await call("docker_start")); } catch (e) { say(oops(e, "Docker did not start. Open Docker Desktop yourself: this page sees it."), "warn"); b.disabled = false; return; }
    dock = Object.assign({}, dock, { state: "starting", can_start: false, help: "Starting: this usually takes under a minute." });
    paintReady();
    if (!dockTimer) dockTimer = setInterval(recheckDocker, 3000);
  }

  // ---------------------------------------------------------------- paint
  function paint() {
    root.replaceChildren();
    root.dataset.v = view;
    Object.keys(ui).forEach((k) => delete ui[k]);
    if (view === "run") return paintRun();
    if (view === "console") return paintEditBar();
    if (view === "edit") return paintEdit();
    if (view === "away") return;
    paintFlow();
  }

  const stepHead = (n, t, line, extra) => {
    const h = mk("div", "hst-sh");
    const tt = mk("div", "hst-sh-t");
    tt.append(mk("h3", "", t));
    if (line) tt.append(mk("p", "", line));
    if (n) { const num = mk("span", "hst-n", String(n)); num.setAttribute("aria-hidden", "true"); h.append(num); }
    h.append(tt);
    if (extra) h.append(extra);
    return h;
  };

  function paintFlow() {
    const ready = mk("div", "hst-ready");
    ready.setAttribute("role", "status");
    ui.ready = ready;
    root.append(ready);
    paintReady();

    // 1. Private or Public
    const s1 = mk("section", "hst-step hst-s1");
    s1.append(stepHead(1, "Who is it for?", null));
    const pick = mk("div", "hst-vis");
    pick.setAttribute("role", "radiogroup");
    pick.setAttribute("aria-label", "Who is it for");
    for (const [v, name, line, fine] of [
      ["private", "Private", "Just me and friends I invite.", "Runs on this PC. Friends join with your invite link."],
      ["public", "Public", "On the Community realms list: anyone can join.", "Through the Leyline relay: you open no ports, nobody sees your address."]]) {
      const b = btn("", "hst-vis-b" + (vis === v ? " on" : ""), () => setVis(v));
      b.dataset.vis = v;
      b.setAttribute("role", "radio");
      b.setAttribute("aria-checked", String(vis === v));
      const t = mk("span", "hst-vis-t");
      t.append(mk("b", "", name), mk("span", "", line), mk("small", "", fine));
      b.append(icon(v, "hst-vis-ico"), t);
      pick.append(b);
    }
    s1.append(pick);
    if (vis === "public") {
      const node = mk("p", "hst-fine hst-node");
      node.append(document.createTextNode("Optional: help carry other players' connections from this PC. "));
      node.append(btn("Be a node ›", "hc-more hst-node-b", () => { const t = page.querySelector('.page-tab[data-hc="network"]'); if (t) t.click(); else call("open_link", { name: "beanode" }).catch(() => {}); }));
      s1.append(node);
    }
    root.append(s1);

    // 2. a template
    const s2 = mk("section", "hst-step hst-s2" + (vis ? "" : " dim"));
    s2.append(stepHead(2, "Pick a template", "Each one fills in the settings below. Change anything after."));
    const grid = mk("div", "hst-grid");
    grid.setAttribute("role", "radiogroup");
    grid.setAttribute("aria-label", "Templates");
    for (const t of C.templates) grid.append(tplCard(t.id, t.name, t.pitch, t.best_for, tplValues(t.id), t.id));
    for (const m of savedList()) grid.append(tplCard("mine:" + m.id, m.name, `Yours, from ${T[m.base].name}.`, "Yours", checkValues(tplValues(m.base), m.values, false).values, "mine", m));
    s2.append(grid);
    const code = mk("div", "hst-code-in");
    const ci = mk("input", "hc-token");
    ci.id = "hst-code-in"; ci.placeholder = "Paste a settings code from a friend"; ci.autocomplete = "off"; ci.spellcheck = false;
    code.append(ci, btn("Use code", "btn small", () => {
      try { const a = readCode(ci.value); useAnswer(a, "From a code."); ci.value = ""; } catch (e) { say(e.message, "warn"); }
    }));
    s2.append(code);
    root.append(s2);

    // 3. the settings panel, 4. Host
    if (tid) { paintPanel(); paintHost(); }

    // more: game modes, Fork and Play solo; other rulesets; Survivors, coming
    const more = mk("details", "hst-more");
    more.append(mk("summary", "", "More ways to host"));
    const ml = mk("div", "hst-more-l");
    ml.append(btn("Game modes: host one, fork it or play it solo ›", "hc-more hst-modes", () => away(() => window.LeylineHostModes && window.LeylineHostModes.grid())));
    ml.append(btn("Other rulesets and older game versions ›", "hc-more hst-rulesets", () => away(() => window.LeylineHostModes && window.LeylineHostModes.rulesets())));
    ml.append(btn("Put a server I already run on Leyline ›", "hc-more hst-link", () => away(() => window.LeylineHostModes && window.LeylineHostModes.rulesets())));
    more.append(ml);
    const soon = mk("p", "hst-fine hst-soon");
    soon.append(mk("em", "hx-chip", "Coming soon"), document.createTextNode(" Survivors: hold out against the waves. "),
      btn("Game modes ›", "hc-more", () => window.LeylineNav && window.LeylineNav.show("mods", "gamemodes")));
    root.append(more, soon, statusLine());
  }

  function keyNumbers(v) {
    const w = W[v.world];
    const out = [];
    out.push(v.world === "top" ? "Start at 80" : `XP ${v.xp}x`);
    out.push(`To ${w.max_level}`);
    out.push(v.world === "solo" ? `${v.helpers} helpers` : !applies(S.bots, v) || !v.bots ? "No bots" : `${v.bots} bots`);
    out.push({ normal: "PvE", pvp: "PvP", rp: "RP", rppvp: "RP-PvP" }[v.realm_type]);
    if (applies(S.boost, v) && v.boost) out.push("Boost");
    return out;
  }
  function tplCard(id, name, pitch, bestFor, v, ico, mine) {
    const priv = W[v.world].private_only;
    const b = btn("", "hst-tpl" + (tid === id ? " on" : "") + (priv && vis === "public" ? " off" : ""), () => chooseTemplate(id));
    b.dataset.tpl = id;
    b.setAttribute("role", "radio");
    b.setAttribute("aria-checked", String(tid === id));
    const head = mk("span", "hst-tpl-h");
    head.append(icon(ico, "hst-tpl-ico"), mk("b", "", name));
    const tags = mk("span", "hst-tpl-tags");
    tags.append(mk("em", "hst-best", bestFor));
    if (priv) tags.append(mk("em", "hst-priv", "Private only"));
    head.append(tags);
    const nums = mk("span", "hst-nums");
    for (const n of keyNumbers(v)) nums.append(mk("i", "", n));
    b.append(head, mk("span", "hst-pitch", pitch), nums);
    if (mine) {
      const x = mk("span", "hst-tpl-x", "Remove");
      x.setAttribute("role", "button");
      x.tabIndex = 0;
      const rm = (ev) => { ev.stopPropagation(); saveList(savedList().filter((s) => s.id !== mine.id)); if (tid === id) { tid = null; vals = null; } paint(); };
      x.addEventListener("click", rm);
      x.addEventListener("keydown", (ev) => { if (ev.key === "Enter" || ev.key === " ") rm(ev); });
      b.append(x);
    }
    return b;
  }

  function setVis(v) {
    vis = v;
    if (!signupTouched && vals) vals.signup = v === "public" ? "open" : "invite";
    if (v === "public" && vals && W[vals.world].private_only) { vals.world = "whole"; say("Just me, with helper bots is private only: the world is now The whole world.", "warn"); }
    paint();
    const t = root.querySelector(".hst-s2");
    if (t && !tid) t.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }
  function chooseTemplate(id) {
    tid = id;
    vals = startValues();
    why = {};
    lastAnswerNote = null;
    if (W[vals.world].private_only && vis !== "private") { vis = "private"; say("Solo with bots is private only: switched to Private.", "warn"); }
    if (!vis) vis = "private";
    vals.signup = vis === "public" ? "open" : "invite";
    signupTouched = false;
    paint();
    const p = root.querySelector(".hst-s3");
    if (p) p.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }
  function useAnswer(a, note) {
    tid = a.template;
    const keepSignup = vals ? vals.signup : (vis === "public" ? "open" : "invite");
    vals = Object.assign({}, a.values, { signup: keepSignup });
    why = a.why || {};
    if (!vis) vis = "private";
    if (W[vals.world].private_only && vis !== "private") vis = "private";
    if (a.name) realmName = a.name;
    lastAnswerNote = { note, changed: a.changed || [], leftOut: a.leftOut || [] };
    paint();
  }

  // ---------------------------------------------------------------- 3. the settings panel
  function control(s) {
    const row = mk("div", "hst-row hst-" + s.kind);
    row.dataset.key = s.key;
    const off = s.kind !== "coming" && !applies(s, vals);
    if (off) row.classList.add("na");
    const id = "hst-in-" + s.key;
    const lab = mk("label", "hst-lab", s.label);
    lab.htmlFor = id;
    const note = mk("span", "hst-help", off ? whyNot(s, vals) : (s.help || ""));
    if (why[s.key]) { const c = mk("span", "hst-why"); c.append(icon("claude", "hst-why-ico"), document.createTextNode(why[s.key])); row.append(c); }
    if (s.kind === "coming") {
      row.append(lab, mk("em", "hx-chip", "Coming"), note);
      lab.removeAttribute("for");
      return row;
    }
    let input;
    if (s.kind === "range") {
      input = mk("input");
      input.type = "range"; input.min = String(s.min); input.max = String(s.max); input.step = String(s.step || 1);
      input.value = String(vals[s.key]);
      const out = mk("output", "hst-val", fmtVal(s, vals[s.key]));
      out.htmlFor = id;
      input.addEventListener("input", () => { vals[s.key] = snap(s, Number(input.value)); out.textContent = fmtVal(s, vals[s.key]); delete why[s.key]; changed(false); });
      input.addEventListener("change", () => changed(true));
      const wrap = mk("div", "hst-slide");
      wrap.append(input, out);
      row.append(lab, wrap, note);
    } else if (s.kind === "toggle") {
      input = mk("input");
      input.type = "checkbox";
      input.setAttribute("role", "switch");
      input.checked = !!vals[s.key];
      input.addEventListener("change", () => { vals[s.key] = input.checked; delete why[s.key]; changed(true); });
      const sw = mk("span", "hst-switch");
      sw.append(input, mk("i"));
      row.append(lab, sw, note);
    } else {
      input = mk("select", "hc-hours");
      for (const [v, l] of s.options) { const o = mk("option", "", l); o.value = v; input.append(o); }
      input.value = vals[s.key];
      input.addEventListener("change", () => {
        vals[s.key] = input.value;
        if (s.key === "signup") signupTouched = true;
        if (s.key === "world" && W[input.value].private_only && vis !== "private") { vis = "private"; say("Just me, with helper bots is private only: switched to Private.", "warn"); }
        delete why[s.key]; changed(true);
      });
      row.append(lab, input, note);
    }
    input.id = id;
    input.dataset.key = s.key;
    input.disabled = off;
    return row;
  }
  function fmtVal(s, v) {
    if (s.key === "max_players" && !v) return "Ruleset's own";
    if (s.key === "bots" && !v) return "None";
    if (s.unit === "x") return `${v}x`;
    return `${v} ${s.unit || ""}`.trim();
  }
  // a change: the summary and the title now; the whole panel again when what applies may have changed (a switch, a choice)
  function changed(repaint) {
    if (repaint) {
      const y = window.scrollY;
      const focusKey = document.activeElement && document.activeElement.dataset ? document.activeElement.dataset.key : null;
      paint();
      window.scrollTo(0, y);
      if (focusKey) { const f = root.querySelector(`[data-key="${focusKey}"] input, [data-key="${focusKey}"] select`); if (f) f.focus(); }
      return;
    }
    paintSummary();
  }

  function paintPanel(edit) {
    const s3 = mk("section", "hst-step hst-s3");
    const ask = btn("", "btn small hst-ask-open", () => { askOpen = askBox.hidden; askBox.hidden = !askOpen; ask.setAttribute("aria-expanded", String(askOpen)); if (askOpen) askBox.querySelector("textarea").focus(); });
    ask.append(icon("claude", "hst-ask-ico"), document.createTextNode("Ask Claude"));
    s3.append(stepHead(edit ? "" : 3, edit ? "Your realm's settings" : "Settings", "Every slider and switch is a real setting of your realm.", ask));
    const askBox = askClaude();
    askBox.hidden = !askOpen;
    ask.setAttribute("aria-expanded", String(askOpen));
    s3.append(askBox);
    if (lastAnswerNote) {
      const n = mk("div", "hst-answer");
      n.append(mk("b", "", lastAnswerNote.note + " Check them, then host."));
      if (lastAnswerNote.changed.length) n.append(mk("p", "hst-changed", "Changed to fit: " + lastAnswerNote.changed.join("; ") + "."));
      if (lastAnswerNote.leftOut.length) n.append(mk("p", "hst-left", "Left out: " + lastAnswerNote.leftOut.join("; ") + "."));
      s3.append(n);
    }
    const body = mk("div", "hst-body");
    const groups = mk("div", "hst-groups");
    for (const g of C.groups) {
      const box = mk("fieldset", "hst-group");
      box.dataset.group = g.id;
      box.append(mk("legend", "", g.label));
      for (const s of C.settings.filter((x) => x.group === g.id)) box.append(control(s));
      if (g.id === "rules") {
        const l = mk("label", "hst-row hst-rules");
        const ta = mk("textarea", "hc-token");
        ta.id = "hst-own-rules"; ta.rows = 3; ta.maxLength = 1300; ta.spellcheck = true;
        ta.placeholder = "Your own house rules, one per line (up to 10)";
        ta.value = ownRules.join("\n");
        ta.addEventListener("input", () => { ownRules = ta.value.split("\n").map((x) => x.trim()).filter(Boolean).slice(0, 10); paintSummary(); });
        l.append(mk("span", "hst-lab", "Your house rules"), ta, mk("span", "hst-help", "Shown on the realm list and the join page."));
        box.append(l);
      }
      groups.append(box);
    }
    const side = mk("aside", "hst-side");
    side.setAttribute("aria-label", "Summary");
    ui.side = side;
    body.append(groups, side);
    s3.append(body);
    root.append(s3);
    paintSummary();
  }

  function summaryLines(v) {
    const w = W[v.world];
    const L = [];
    L.push(["World", w.label]);
    L.push(["Start", v.world === "top" ? "Everyone at 80" : applies(S.boost, v) && v.boost ? `Level 1, or a one-time boost to 80${v.mount ? "; a mount at 1" : ""}` : "Level 1"]);
    L.push(["Rates", `XP ${v.xp}x · drops ${v.drops}x · gold ${v.gold}x · rep ${v.rep}x · honor ${v.honor}x`]);
    L.push(["Bots", v.world === "solo" ? `${v.helpers} helpers per player, ${v.bots} roaming` : applies(S.bots, v) && v.bots ? `${v.bots} roaming` : "None"]);
    L.push(["Type", S.realm_type.options.find((o) => o[0] === v.realm_type)[1]]);
    const fun = [v.events && "events", v.events && v.trivia && "trivia", v.events && v.hide && "hide and seek", v.duel_ladder && "duel ladder", applies(S.minigolf, v) && v.minigolf && "minigolf"].filter(Boolean);
    L.push(["Extras", fun.length ? fun.join(", ") : "None"]);
    L.push(["Players", `${v.max_players ? "Up to " + v.max_players : "The ruleset's own limit"} · ${S.signup.options.find((o) => o[0] === v.signup)[1].toLowerCase()}`]);
    const r = [v.rp_names && "RP names", v.one_life && "hardcore welcome", ownRules.length && `${ownRules.length} of your own`].filter(Boolean);
    if (r.length) L.push(["Rules", r.join(", ")]);
    if (v.loadout !== "none") L.push(["Addons", S.loadout.options.find((o) => o[0] === v.loadout)[1]]);
    return L;
  }
  function paintSummary() {
    const side = ui.side;
    if (!side || !vals) return;
    side.replaceChildren();
    const h = mk("div", "hst-sum-h");
    const custom = isCustom();
    h.append(mk("span", "hc-k", custom ? "Custom" : "Template"), mk("b", "hst-sum-t", title()));
    side.append(h);
    const who = mk("p", "hst-sum-who", vis === "public" ? "Public: on the Community realms list." : "Private: you and friends with your invite link.");
    side.append(who);
    const dl = mk("dl", "hst-sum");
    for (const [k, v] of summaryLines(vals)) dl.append(mk("dt", "", k), mk("dd", "", v));
    side.append(dl);
    const acts = mk("div", "hst-sum-acts");
    if (custom) acts.append(btn("Back to the template", "hc-more hst-reset", () => { vals = Object.assign(startValues(), { signup: vals.signup }); why = {}; lastAnswerNote = null; paint(); }));
    acts.append(btn("Save as my template", "btn small hst-save-tpl", () => saveMine(acts)));
    acts.append(btn("Share as a code", "btn small hst-share", async (ev) => {
      const c = shareCode();
      ui.code = c;
      const out = side.querySelector(".hst-code") || mk("code", "hc-link hst-code");
      out.textContent = c;
      side.append(out);
      try { await navigator.clipboard.writeText(c); ev.currentTarget.textContent = "Code copied"; } catch (_) { /* shown to copy by hand */ }
    }));
    side.append(acts);
  }
  function saveMine(acts) {
    if (acts.querySelector(".hst-mine-name")) return;
    const row = mk("div", "hst-mine");
    const i = mk("input", "hc-token hst-mine-name");
    i.maxLength = 32; i.placeholder = "Name it"; i.value = isCustom() ? "" : `My ${tplName()}`.slice(0, 32);
    const ok = btn("Save", "btn small primary", () => {
      const name = i.value.trim();
      if (!NAME_OK.test(name) || BRAND.test(name)) { say("A name: up to 32 letters, digits and spaces.", "warn"); i.focus(); return; }
      const l = savedList();
      const id = "t" + Date.now().toString(36);
      l.push({ id, name, base: baseTid(), values: diffOf(vals, baseTid()) });
      saveList(l);
      tid = "mine:" + id;
      say(`Saved "${name}" with your templates.`, "ok");
      paint();
    });
    row.append(i, ok);
    acts.after(row);
    i.focus();
  }

  // ---------------------------------------------------------------- Ask Claude
  function promptText(words) {
    const lines = [
      "I'm setting up a game realm in the Leyline launcher. Fill in its settings for what I want, and explain each choice in one short line.",
      `What I want: ${words || "(describe it here)"}`,
      "",
      "Answer with one JSON block like this, using only the keys below:",
      '{"template": "<template id>", "settings": {"<key>": <value>}, "why": {"<key>": "<one short line>"}, "name": "<optional realm name>"}',
      "",
      "Templates (pick the closest, then change settings):",
      ...C.templates.map((t) => `- ${t.id}: ${t.name}. ${t.pitch}`),
      "",
      "Settings:",
    ];
    for (const s of C.settings) {
      if (s.kind === "coming" || NOT_FROM_AI.includes(s.key)) continue;
      let what = s.kind === "range" ? `a number from ${s.min} to ${s.max} in steps of ${s.step || 1}${s.unit ? " (" + s.unit + ")" : ""}`
        : s.kind === "toggle" ? "true or false" : "one of " + s.options.map((o) => `"${o[0]}" (${o[1]})`).join(", ");
      if (s.only) what += `; only for world ${s.only.join(" or ")}`;
      if (s.needs) what += `; needs ${s.needs}`;
      lines.push(`- ${s.key}: ${s.label}. ${what}.${s.help ? " " + s.help : ""}`);
    }
    lines.push("", `Not possible yet: ${C.settings.filter((s) => s.kind === "coming").map((s) => s.label).join(", ")}.`,
      "Leave out who can join (private or public, sign-up): I choose that myself.");
    return lines.join("\n");
  }
  function askClaude() {
    const box = mk("div", "hst-askbox");
    box.id = "hst-askbox";
    const ta = mk("textarea", "hc-token");
    ta.id = "hst-ask-words"; ta.rows = 2; ta.maxLength = 600;
    ta.placeholder = "Describe your realm: a cozy RP realm for 10 friends, slow levelling, no PvP";
    ta.value = askWords;
    ta.addEventListener("input", () => { askWords = ta.value; });
    box.append(mk("label", "hst-lab", "Describe the realm you want"), ta);
    const ways = mk("div", "hst-ways");
    // the player's own assistant (agents.rs host_plan_request / host_plan_propose): connected on the Agents page
    const a = mk("div", "hst-way");
    a.append(mk("b", "", "Your own assistant"), mk("span", "hst-help", "Claude on this PC, connected on the Agents page. You approve its answer, then review it here."));
    const send = btn("Ask my assistant", "btn small primary hst-ask-send", async () => {
      const words = ta.value.trim();
      if (!words) { ta.focus(); return; }
      let st = null;
      try { st = await call("agents_state"); } catch (_) { /* older core */ }
      if (!st || !st.allow) { askSay("Your assistant isn't connected yet: set it up once on the Agents page, or use any AI chat below.", "warn", true); return; }
      try { await call("host_plan_ask", { text: words }); } catch (e) { askSay(oops(e), "bad"); return; }
      askSay('Sent. In Claude, say: "Fill my Leyline realm settings". Approve its card, and the settings land here.', "ok");
      waitAnswer(Date.now());
    });
    a.append(send);
    // any AI chat: copy the prompt, paste the answer back
    const b = mk("div", "hst-way");
    b.append(mk("b", "", "Any AI chat"), mk("span", "hst-help", "Copy the prompt into any AI chat, then paste its whole answer here. Only your words and the settings list are in it."));
    const copy = btn("Copy the prompt", "btn small hst-ask-copy", async (ev) => {
      const p = promptText(ta.value.trim());
      ui.prompt = p;
      try { await navigator.clipboard.writeText(p); ev.currentTarget.textContent = "Prompt copied"; } catch (_) { pre.hidden = false; }
      pre.textContent = p;
    });
    const pre = mk("pre", "hst-prompt");
    pre.id = "hst-prompt"; pre.hidden = true;
    const show = btn("Show it", "hc-more", () => { pre.textContent = promptText(ta.value.trim()); pre.hidden = !pre.hidden; });
    const back = mk("textarea", "hc-token");
    back.id = "hst-paste"; back.rows = 3; back.placeholder = "Paste the AI's answer here";
    const fill = btn("Fill the settings", "btn small hst-ask-fill", () => {
      try { useAnswer(parseAnswer(back.value), "From your AI chat."); } catch (e) { askSay(e.message, "warn"); }
    });
    const row = mk("div", "hst-way-row");
    row.append(copy, show);
    b.append(row, pre, back, fill);
    ways.append(a, b);
    box.append(ways, mk("p", "status hst-ask-status"));
    return box;
  }
  function askSay(t, kind, agentsLink) {
    const s = root.querySelector(".hst-ask-status");
    if (!s) return;
    s.replaceChildren(document.createTextNode(t || ""));
    s.className = "status hst-ask-status" + (kind ? " " + kind : "");
    if (agentsLink) s.append(" ", btn("Agents page ›", "hc-more", () => window.LeylineNav && window.LeylineNav.show("agents")));
  }
  function waitAnswer(t0) {
    clearTimeout(askTimer);
    askTimer = setTimeout(async () => {
      let a = null;
      try { a = await call("host_plan_take"); } catch (_) { /* again */ }
      if (a && a.values) {
        useAnswer({ template: a.template, values: a.values, why: a.why, name: a.name, changed: a.changed, leftOut: a.left_out }, "From your assistant.");
        return;
      }
      if (Date.now() - t0 < 10 * 60 * 1000) waitAnswer(t0);
      else askSay("No answer from your assistant yet. Ask it again, or use any AI chat.", "warn");
    }, 3000);
  }

  // ---------------------------------------------------------------- 4. Host
  function paintHost() {
    const s4 = mk("section", "hst-step hst-s4");
    s4.append(stepHead(4, "Host it", null));
    const form = mk("div", "hst-form");
    const nl = mk("label", "hc-field");
    const name = mk("input", "hc-token");
    name.id = "hst-name"; name.type = "text"; name.maxLength = 32; name.spellcheck = false; name.autocomplete = "off";
    name.value = realmName || (isCustom() ? "My Realm" : tplName()).replace(/[^A-Za-z0-9 _'-]/g, "").slice(0, 32) || "My Realm";
    name.addEventListener("input", () => { realmName = name.value; });
    nl.append(mk("span", "", "Realm name"), name);
    form.append(nl);
    let gm = null;
    if (needsGm) {
      const gl = mk("label", "hc-field");
      gm = mk("input", "hc-token");
      gm.id = "hst-gm"; gm.type = "text"; gm.maxLength = 16; gm.spellcheck = false; gm.autocomplete = "off"; gm.placeholder = "3 to 16 letters or digits";
      gl.append(mk("span", "", "Your account name (it gets GM)"), gm);
      form.append(gl);
    }
    const what = mk("ol", "hst-what");
    what.setAttribute("aria-label", "What happens");
    const steps = [
      "Your settings are saved in the Host pack on this PC.",
      `Docker starts your realm's servers here: the database, the login server and the world server. World: ${W[vals.world].label.toLowerCase()}.`,
      "The game data it needs is read from your own game folder, on this PC. Nothing is downloaded from us.",
      "It connects out to the Leyline relay. You open no ports.",
      vis === "public" ? "It shows on the Community realms list within a minute." : "You get an invite link. Friends open it once and make their own account.",
      "The first start takes 5 to 20 minutes. Leave this PC on while people play.",
    ];
    for (const t of steps) what.append(mk("li", "", t));
    const go = mk("div", "hc-go hst-go-row");
    const host = btn(`Host ${title()}`, "btn primary hst-go", () => hostIt(name, gm, host));
    host.id = "hst-go";
    if (!haveKit() || !dockOk()) host.title = "Set up the line at the top first";
    go.append(host);
    s4.append(form, mk("p", "hc-bh", "What happens"), what, go);
    root.append(s4);
  }
  async function hostIt(nameIn, gmIn, b) {
    const name = nameIn.value.trim();
    if (!vis) { say("Pick Private or Public first.", "warn"); return; }
    if (!NAME_OK.test(name) || BRAND.test(name)) { say("Give your realm a name: letters, digits and spaces, no game names.", "warn"); nameIn.focus(); return; }
    const gm = gmIn ? gmIn.value.trim() : null;
    if (gmIn && !GM_OK.test(gm)) { say("Type your account name: 3 to 16 letters or digits.", "warn"); gmIn.focus(); return; }
    if (!haveKit()) { say("Get the Host pack first: the line at the top.", "warn"); root.querySelector(".hst-ready").scrollIntoView({ block: "nearest" }); return; }
    if (!dockOk()) { say("Docker first: the line at the top.", "warn"); root.querySelector(".hst-ready").scrollIntoView({ block: "nearest" }); return; }
    b.disabled = true;
    say("Getting your realm ready…");
    try {
      const done = await call("host_flow_apply", { template: baseTid(), values: vals, visibility: vis, rules: ownRules });
      const prof = await call("host_profiles").catch(() => null);
      const o = Array.isArray(prof) && Array.isArray(prof[0]) ? prof[0].find((x) => x && x.id === done.profile) : null;
      if (o && o.note) throw o.note;
      await call("host_choose", { profile: done.profile, clientId: o && o.clients && o.clients[0] ? o.clients[0].id : null });
      await call("host_start_quiet", { answers: { name, visibility: done.visibility, gm } });
      job = { running: true, step: "Getting ready", n: 0, of: 8, secs: 0, code: null, problem: null };
      live = { name, title: title(), rid: null, online: false, link: null, visibility: done.visibility, players: 0 };
      view = "run";
      place(); paint();
      follow();
    } catch (e) { say(oops(e), "bad"); b.disabled = false; }
  }

  // ---------------------------------------------------------------- the start, then the running realm's card
  const fmt = (s) => { const m = Math.floor(s / 60), r = s % 60; return m ? `${m} min ${String(r).padStart(2, "0")} s` : `${r} s`; };
  function paintRun() {
    const sw = mk("div", "hst-run");
    if (live && live.online) {
      const box = mk("div", "hc-live on hst-live");
      const t = mk("div", "hc-live-t");
      t.append(mk("b", "", "You're live"));
      const st = mk("p", "hst-live-st");
      st.append(mk("span", "hst-dot on"), document.createTextNode(` ${live.name} is online · ${live.players || 0} playing · ${live.title}`));
      t.append(st, mk("span", "", live.visibility === "private"
        ? "Private. Send friends your invite link: they make their account there, then play."
        : "Public. It's on the Community realms list. Share its page with anyone."));
      const code = mk("code", "hc-link hst-share", live.link || "");
      code.id = "hst-share";
      t.append(code);
      box.append(t);
      const acts = mk("div", "hst-live-acts");
      acts.append(btn(live.visibility === "private" ? "Copy invite link" : "Copy its page", "btn primary", async (ev) => {
        try { await navigator.clipboard.writeText(live.link); ev.currentTarget.textContent = "Copied"; } catch (_) { say("Select the link and copy it.", "warn"); }
      }));
      acts.append(btn("GM command center", "btn small", () => window.LeylineGM && window.LeylineGM.open("realm")));
      acts.append(btn("Edit settings", "btn small hst-edit-b", () => openEdit()));
      acts.append(btn("Stop", "btn small hst-stop", (ev) => stopIt(ev.currentTarget)));
      acts.append(btn("Open the console", "hc-more", () => { view = "console"; live = null; place(); paint(); }));
      box.append(acts);
      sw.append(box);
    } else {
      const failed = job && !job.running && job.code !== 0 && job.code !== undefined && job.code !== null;
      const box = mk("div", "hc-prog" + (failed ? " bad" : job && !job.running ? " ok" : ""));
      const h = mk("div", "hc-prog-h");
      h.append(mk("b", "", failed ? "Setting up stopped" : job && job.running ? job.step : "Connecting to the network"),
        mk("span", "hc-fine", job ? (job.running ? `Step ${Math.max(1, job.n)} of ${job.of} · ${fmt(job.secs)}` : fmt(job.secs)) : ""));
      const bar = mk("div", "hc-bar");
      const fill = mk("i");
      fill.style.width = `${Math.round((job && job.running ? Math.max(job.n - 0.5, 0.3) : (job ? job.of : 1)) / Math.max(job ? job.of : 1, 1) * 100)}%`;
      bar.append(fill);
      bar.setAttribute("role", "progressbar");
      box.append(h, bar, mk("p", "hc-fine", failed ? (job.problem || "Something went wrong.") : `Hosting ${live ? live.title : "your realm"}. The first start takes 5 to 20 minutes. You can use the rest of Leyline meanwhile.`));
      if (failed) box.append(btn("Back to the settings", "btn small", () => { view = kit && kit.realm ? "console" : "flow"; live = null; place(); paint(); }));
      sw.append(box);
    }
    sw.append(statusLine());
    root.append(sw);
  }
  async function follow() {
    clearTimeout(jobTimer);
    let p = null;
    try { p = await call("host_progress"); } catch (_) { /* again */ }
    if (p) job = p;
    if (view !== "run") return;
    paint();
    if (job && job.running) { jobTimer = setTimeout(follow, 2000); return; }
    if (job && job.code === 0) { if (typeof loadHost === "function") loadHost(); watchLive(); }
  }
  async function watchLive() {
    clearTimeout(liveTimer);
    if (!live || view !== "run") return;
    let info = null;
    try { info = await call("host_info"); } catch (_) { /* again */ }
    const rid = info && info.realm && info.realm.realm_id;
    if (rid) {
      live.rid = rid;
      let st = null;
      try { st = await call("host_status", { realmId: rid }); } catch (_) { /* again */ }
      if (st && st.online) {
        live.online = true;
        live.players = st.players || 0;
        if (live.visibility === "private") { try { live.link = await call("host_invite"); } catch (_) { live.link = SHARE(rid); } }
        else live.link = SHARE(rid);
        paint();
        liveTimer = setTimeout(watchLive, 30000);   // the players line stays fresh
        return;
      }
    }
    liveTimer = setTimeout(watchLive, 8000);
  }
  async function stopIt(b) {
    if (!window.confirm("Stop your realm? Anyone playing is dropped. Characters, accounts and your invite link are kept.")) return;
    b.disabled = true;
    try { say(await call("host_stop"), "ok"); live = null; view = "console"; place(); paint(); if (typeof loadHost === "function") loadHost(); }
    catch (e) { say(oops(e), "bad"); b.disabled = false; }
  }

  // ---------------------------------------------------------------- a realm on this PC: Edit settings
  function paintEditBar() {
    const bar = mk("div", "hst-editbar");
    const r = kit && kit.realm;
    bar.append(icon("classic", "hst-ico"), mk("span", "", r && r.name ? `${r.name}: its settings` : "Your realm's settings"));
    bar.append(btn("Edit settings", "btn small hst-edit-open", () => openEdit()));
    root.append(bar);
  }
  async function openEdit() {
    let cur = null;
    try { cur = await call("host_flow_current"); } catch (_) { /* none */ }
    const r = kit && kit.realm;
    vis = (cur && cur.visibility) || (r && r.visibility) || "private";
    if (cur && T[cur.template]) { tid = cur.template; vals = checkValues(tplValues(tid), cur.values || {}, false).values; ownRules = Array.isArray(cur.rules) ? cur.rules.slice(0, 10) : []; lastAnswerNote = null; }
    else { tid = "classic"; vals = tplValues("classic"); vals.signup = vis === "public" ? "open" : "invite"; lastAnswerNote = { note: "These settings weren't made on this page: they start from Classic levelling.", changed: [], leftOut: [] }; }
    why = {};
    view = "edit";
    live = null;
    place(); paint();
    root.scrollIntoView({ block: "start" });
  }
  function paintEdit() {
    const top = mk("div", "hst-edit-top");
    top.append(btn("‹ Back to the console", "hc-more hst-edit-back", () => { view = "console"; place(); paint(); }));
    root.append(top);
    paintPanel(true);
    const go = mk("div", "hst-step hst-edit-go");
    go.append(mk("p", "hst-fine", "Saving needs a restart of your realm (about a minute; anyone playing is dropped). Your house rules show on the list after it."),
      mk("p", "hst-fine", `Who can join stays ${vis === "public" ? "Public" : "Private"}: change that in Realm tools.`));
    const row = mk("div", "hc-go");
    const now = btn("Save and restart", "btn primary hst-save-restart", (ev) => saveEdit(true, ev.currentTarget));
    const later = btn("Save for the next start", "btn small hst-save-later", (ev) => saveEdit(false, ev.currentTarget));
    row.append(now, later);
    go.append(row, statusLine());
    root.append(go);
  }
  async function saveEdit(restart, b) {
    b.disabled = true;
    try {
      await call("host_flow_apply", { template: baseTid(), values: vals, visibility: vis, rules: ownRules });
      if (!restart) { say("Saved. They apply the next time your realm starts.", "ok"); b.disabled = false; return; }
      await call("host_start_quiet", { answers: {} });
      job = { running: true, step: "Restarting with the new settings", n: 0, of: 8, secs: 0, code: null, problem: null };
      live = { name: (kit && kit.realm && kit.realm.name) || "Your realm", title: title(), rid: null, online: false, link: null, visibility: vis, players: 0 };
      view = "run";
      place(); paint();
      follow();
    } catch (e) { say(oops(e), "bad"); b.disabled = false; }
  }

  // ---------------------------------------------------------------- Game modes or the ruleset picker: this flow steps aside
  function away(fn) {
    view = "away";
    place();
    if (fn) fn();
  }
  function back() {
    view = kit && kit.realm ? "console" : "flow";
    place(); paint();
    if (window.LeylineNav) window.LeylineNav.show("host");
  }

  // ---------------------------------------------------------------- the Guide tab: start to finish (docs/leyline/host-a-realm.md)
  function guide() {
    if (!guidePane || byId("hst-guide")) return;
    const box = mk("section", "hst-guide");
    box.id = "hst-guide";
    const head = mk("div", "hst-guide-h");
    const full = btn("The full guide on the website", "hc-more", () => call("open_link", { name: "hostarealm" }).catch(() => {}));
    full.dataset.link = "hostarealm";
    head.append(mk("h3", "", "Host a realm, start to finish"), full);
    box.append(head);
    const ch = (title, ...parts) => {
      const d = mk("details", "hst-g");
      d.append(mk("summary", "", title));
      for (const p of parts) d.append(typeof p === "string" ? mk("p", "", p) : p);
      box.append(d);
      return d;
    };
    const list = (rows) => { const ul = mk("ul", "hst-g-l"); for (const [b, t] of rows) { const li = mk("li"); li.append(mk("b", "", b), document.createTextNode(" " + t)); ul.append(li); } return ul; };
    ch("What the server software is",
      "Your realm runs on an open-source realm server (AzerothCore), packed by us into a realm image with its modules and our realm scripts. Docker, a free program, runs it on your PC in its own space.",
      list([["Database:", "accounts, characters and the world."], ["Login server:", "checks a player's account."], ["World server:", "the game world itself."],
        ["Leyline agent:", "connects out to the Leyline relay on port 443. You open no ports and nobody sees your address."]]),
      "The Host pack is the folder with all of this and its tools. The game data the server needs is read from your own game folder, on your PC. Leyline never sends game files.");
    ch("Private or Public",
      "Private: just you and friends you invite. Friends open your invite link once and make their own account. Nobody else can find it.",
      "Public: on the Community realms list, anyone can join. Change it any time in Realm tools.");
    const tl = list(C.templates.map((t) => [t.name + ":", `${t.pitch} Best for: ${t.best_for.toLowerCase()}.`]));
    ch("The templates", tl);
    const sl = mk("div");
    for (const g of C.groups) {
      sl.append(mk("p", "hc-bh", g.label));
      sl.append(list(C.settings.filter((s) => s.group === g.id).map((s) => [s.label + ":", s.kind === "coming" ? (s.help || "Coming.") : (s.help || describe(s))])));
    }
    ch("Every setting", sl);
    ch("Ask Claude",
      "Describe the realm you want in plain words. Your own assistant (connected on the Agents page) fills the settings, and you approve its card first. Or copy the prompt into any AI chat and paste its answer back.",
      "Either way the settings land in the panel for you to check before you host. Unknown settings are left out, numbers are kept inside their range, and who can join is always your own choice. Nothing secret goes into the prompt.");
    ch("Invite friends", "Private realms: copy your invite link from the realm's card or Realm tools > Invite links. Friends open it once and make their own account there. Leyline never makes accounts or sees passwords.");
    ch("Go public", "Realm tools > Public or private. A public realm is on the Community realms list within a minute. Claim it with your Hero Key so it shows as yours.");
    ch("Be a node", "Optional: let this PC carry other players' connections to the network. Host > Network, or the website's guide.");
    ch("Backups and updates", "Stop the realm, then copy the Host pack folder: its state folder holds your realm's key and links. When a new Host pack is out, update it under Host pack & settings, then start again: your realm and its links are kept.");
    ch("When something's wrong", "Run the health check on the console: each line says what to fix. Docker not running? The line at the top of Host starts it. Server logs are in Realm tools.");
    guidePane.prepend(box);
  }
  function describe(s) {
    if (s.kind === "range") return `${s.min} to ${s.max}${s.unit === "x" ? "x" : s.unit ? " " + s.unit : ""}.`;
    if (s.kind === "choice") return s.options.map((o) => o[1]).join(", ") + ".";
    return "On or off.";
  }
  guide();

  window.LeylineHostFlow = { show: back, hide: () => away(), parseAnswer, checkValues, promptText, readCode, catalog: C,
    state: () => ({ vis, tid, vals, view, custom: isCustom(), title: title() }) };
  place();
  paint();
})();
