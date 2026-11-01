// Leyline: modes anyone can host and fork (docs/MODES.md, "Mode packages"): the Host page's first screen while no realm
// is set up on this PC. A grid of mode cards (picture, name, one line, who made it, players on realms running it now);
// pick one: a short form (realm name, who can join, the mode's own settings); Host: the host flow with the mode's
// ruleset, scripts, mods and Den (host/modes.rs use_mode, then host_choose and host_start_quiet as the ruleset picker
// does); then "You're live" with the share link. "Fork this mode" on any card, and on any listed realm that publishes its
// mode (the Realms page: realmRow below), copies the mode into the Host pack as yours, based_on set, and opens its settings.
// Commands: src-tauri/src/p2p/cmds.rs host_mode*. Loaded after hostconsole.js (it rebuilds the Start tab on each
// "leyline:kit"; this puts its screen back on top). Everything from a mode or the realm list is shown with textContent.
"use strict";
(() => {
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
  if (!page || !main) return;

  // the website's share link for a realm (stream/links.py /wrath/r/<id>: the full id, or its short form once that ships)
  const SHARE = (rid) => `https://play.openvanilla.app/wrath/r/${rid}`;
  // the realms made for this PC alone are always private (hostconsole.js LOCAL_IDS, host/profiles.rs private_profile)
  const PRIVATE = ["den", "solo"];
  const NAME_OK = /^[A-Za-z0-9 _'-]{1,32}$/;
  const GM_OK = /^[A-Za-z0-9]{3,16}$/;
  const needsGm = !/Windows/i.test(navigator.userAgent || "");

  const root = mk("section", "hx");
  root.id = "hx";
  root.setAttribute("aria-label", "Host a mode");
  let modes = null;        // host_modes
  let kit = null;          // the last "leyline:kit"
  let view = "grid";       // grid | host | edit | run | off (the ruleset picker underneath)
  let cur = null;          // the mode on the form
  let job = null, jobTimer = null, live = null, liveTimer = null;

  const realms = () => { try { return Array.isArray(realmsCache) ? realmsCache : []; } catch (_) { return []; } };   // app.js
  // a mode's name: from the cards, else from a listed realm that runs it
  const nameOf = (id) => {
    const m = (modes || []).find((x) => x.id === id);
    if (m) return m.name;
    const r = realms().find((x) => x.mode && x.mode.id === id && x.mode.name);
    return r ? r.mode.name : id;
  };
  function stats(id) {
    const rs = realms().filter((r) => r.mode && r.mode.id === id);
    const on = rs.filter((r) => r.state === "live");
    return { realms: rs.length, live: on.length, players: on.reduce((n, r) => n + (r.players || 0), 0) };
  }
  // "by Y", or "based on X by Y · by Z" for a fork
  function credit(m) {
    const p = mk("p", "hx-by");
    if (m.based_on) p.append(document.createTextNode(`based on ${nameOf(m.based_on.id)} by ${m.based_on.author || "someone"} · `));
    p.append(document.createTextNode(`by ${m.author || "someone"}`));
    return p;
  }
  function pic(m, cls) {
    const box = mk("div", cls || "hx-pic");
    box.setAttribute("aria-hidden", "true");
    if (m.picture) { const img = mk("img"); img.src = m.picture; img.alt = ""; img.width = 320; img.height = 180; img.decoding = "async"; box.append(img); }
    return box;
  }
  const say = (t, kind) => { const s = root.querySelector(".hx-status"); if (s) { s.textContent = t || ""; s.className = "status hx-status" + (kind ? " " + kind : ""); } };

  // ---------------------------------------------------------------- where it sits
  function place() {
    // no modes (an older core, or none to show): the ruleset picker as before
    const setup = !(kit && kit.realm) && Array.isArray(modes) && modes.length > 0;
    const shown = setup || (view === "run" && live);
    main.classList.toggle("hx-on", !!setup && view !== "off" && view !== "run");
    root.classList.toggle("hx-bar", view === "off");
    if (view === "off" && root.dataset.v !== "off") paint();
    if (!shown) { root.remove(); return; }
    if (main.firstChild !== root) main.prepend(root);
  }
  document.addEventListener("leyline:kit", (e) => {
    kit = e.detail || null;
    if (kit && kit.realm && view !== "run") view = "grid";   // a realm on this PC: the console (the grid is for the next one)
    place();
    if (!modes) load();
  });
  document.addEventListener("leyline:realms", () => { if (view === "grid") paint(); });

  async function load(focusId) {
    try { modes = await call("host_modes"); } catch (e) { modes = []; console.error("[leyline] modes", e); }
    if (!Array.isArray(modes)) modes = [];
    if (focusId) { const m = modes.find((x) => x.id === focusId); if (m) { cur = m; view = "edit"; } }
    place();
    paint();
  }

  function paint() {
    root.replaceChildren();
    root.dataset.v = view;
    if (view === "off") {   // the ruleset picker underneath: one line back to the modes
      const bar = mk("div", "hx-back-bar");
      bar.append(mk("span", "", "Picking a ruleset yourself."), btn("‹ Host a mode instead", "hc-more hx-back", () => { view = "grid"; place(); paint(); }));
      root.append(bar);
      return;
    }
    if (view === "host" && cur) return paintHost();
    if (view === "edit" && cur) return paintEdit();
    if (view === "run") return paintRun();
    paintGrid();
  }

  // ---------------------------------------------------------------- the grid
  function paintGrid() {
    const sw = mk("div", "hc-sw hx-sw");
    const top = mk("div", "hx-top");
    const id = mk("div", "hc-id");
    id.append(mk("p", "hc-t", "What will you host?"));
    // a plain ruleset: no mode (the kit's state/mode.json goes, so the realm list shows none)
    const other = btn("Pick a ruleset yourself", "hc-more hx-other", () => { view = "off"; place(); paint(); call("host_mode_use", { id: null, values: {} }).catch(() => {}); });
    top.append(id, other);
    sw.append(top);   // owner 1 Nov 01:04: "less info panels, straight to the point": no lede, small template tiles
    const grid = mk("div", "hx-grid");
    grid.setAttribute("role", "list");
    for (const m of modes || []) grid.append(cardOf(m));
    if (!(modes || []).length) grid.append(mk("p", "hc-fine", "Loading the modes…"));
    sw.append(grid, mk("p", "status hx-status"));
    root.append(sw);
  }
  function cardOf(m) {
    const c = mk("article", "hx-card" + (m.own ? " own" : "") + (m.status === "coming" ? " soon" : ""));
    c.setAttribute("role", "listitem");
    c.dataset.mode = m.id;
    const b = mk("div", "hx-b");
    const h = mk("h3", "hx-name", m.name);
    if (m.own) h.append(mk("em", "hx-chip", "Yours"));
    if (m.status === "coming") h.append(mk("em", "hx-chip", "Coming soon"));
    b.append(h, mk("p", "hx-line", m.line));
    // who made it: only when it isn't one of ours as shipped (a fork, or someone else's)
    if (m.based_on || (m.author && m.author !== "Leyline")) b.append(credit(m));
    const st = stats(m.id);
    if (st.realms) {
      const s = mk("p", "hx-live" + (st.live ? " on" : ""));
      s.textContent = st.live ? `${st.live} realm${st.live === 1 ? "" : "s"} live · ${st.players} playing` : `${st.realms} realm${st.realms === 1 ? "" : "s"}, offline now`;
      b.append(s);
    }
    const acts = mk("div", "hx-acts");
    if (m.status === "coming") {
      if (m.credit) b.append(mk("p", "hx-by", `By ${m.credit.by}`));
    } else {
      acts.append(btn("Host", "btn small primary hx-host", () => { cur = m; view = "host"; paint(); }));
      acts.append(btn("Fork", "btn small hx-fork", (ev) => forkIt(m.id, null, ev.currentTarget)));
      if (m.own) {
        acts.append(btn("Change", "btn small hx-change", () => { cur = m; view = "edit"; paint(); }));
        acts.append(btn("Remove", "hc-more hx-remove", () => removeIt(m)));
      }
    }
    c.append(pic(m), b, acts);
    return c;
  }
  async function forkIt(id, listed, b) {
    if (!(kit && kit.is_kit)) {
      view = "grid"; show(); paint();
      say("Get the Host pack first: a fork lives in it. Pick a ruleset yourself below to get ready.", "warn");
      return;
    }
    if (b) b.disabled = true;
    try {
      const f = await call("host_mode_fork", { id, listed: listed || null, author: null });
      await load(f.id);
      show();
      say(`Forked: ${f.name} is yours. Change it, then host it.`, "ok");
    } catch (e) { say(oops(e), "bad"); if (b) b.disabled = false; }
  }
  async function removeIt(m) {
    if (!window.confirm(`Remove ${m.name}? Realms already running it keep running.`)) return;
    try { await call("host_mode_remove", { id: m.id }); await load(); say(`Removed ${m.name}.`, "ok"); } catch (e) { say(oops(e), "bad"); }
  }
  function show() {
    if (window.LeylineNav) window.LeylineNav.show("host");
    const t = byId("hc-tab-main");
    if (t && t.getAttribute("aria-selected") !== "true") t.click();
    place();
  }

  // ---------------------------------------------------------------- a setting's field
  function field(s, value) {
    const l = mk("label", "hc-field hx-f");
    l.dataset.key = s.key;
    const lab = mk("span", "hx-l", s.label);
    let input;
    if (s.type === "toggle") {
      l.classList.add("hx-tog");
      input = mk("input");
      input.type = "checkbox";
      input.checked = !!value;
      l.append(input, lab);
    } else if (s.type === "choice") {
      input = mk("select", "hc-hours");
      for (const o of s.options) { const op = mk("option", "", o.label); op.value = o.value; input.append(op); }
      input.value = value;
      l.append(lab, input);
    } else {
      const row = mk("span", "hx-num");
      input = mk("input", "hc-token");
      input.type = "number"; input.min = String(s.min); input.max = String(s.max); input.step = "1"; input.value = String(value);
      input.inputMode = "numeric";
      row.append(input, mk("span", "hc-fine", `${s.unit ? s.unit + " · " : ""}${s.min} to ${s.max}`));
      l.append(lab, row);
    }
    input.dataset.key = s.key;
    input.dataset.type = s.type;
    if (s.help) l.append(mk("span", "hc-fine hx-help", s.help));
    return l;
  }
  function values(box) {
    const out = {};
    for (const i of box.querySelectorAll("[data-key][data-type]")) {
      out[i.dataset.key] = i.dataset.type === "toggle" ? i.checked : i.dataset.type === "number" ? Number(i.value) : i.value;
    }
    return out;
  }
  function badValue(m, vals) {
    for (const s of m.settings) {
      const v = vals[s.key];
      if (s.type === "number" && !(Number.isInteger(v) && v >= s.min && v <= s.max)) return `${s.label}: ${s.min} to ${s.max}.`;
    }
    return null;
  }
  function head(m, kicker) {
    const top = mk("div", "hx-head");
    const back = btn("‹ All modes", "hc-more hx-back", () => { view = "grid"; paint(); });
    top.append(back);
    const row = mk("div", "hx-mrow");
    const t = mk("div", "hx-mt");
    t.append(mk("p", "hc-k", kicker), mk("p", "hc-t", m.name), mk("p", "hx-line", m.line), credit(m));
    row.append(pic(m, "hx-pic small"), t);
    top.append(row);
    return top;
  }

  // ---------------------------------------------------------------- host it: the short form
  let vis = "private";
  function paintHost() {
    const m = cur;
    const sw = mk("div", "hc-sw hx-sw");
    sw.append(head(m, "Host this mode"));
    const form = mk("div", "hx-form");
    const nl = mk("label", "hc-field");
    const name = mk("input", "hc-token");
    name.id = "hx-name"; name.type = "text"; name.maxLength = 32; name.spellcheck = false; name.autocomplete = "off";
    name.value = m.name.replace(/[^A-Za-z0-9 _'-]/g, "").slice(0, 32) || "My Realm";
    nl.append(mk("span", "", "Realm name"), name);
    form.append(nl);
    if (!PRIVATE.includes(m.profile)) {
      const vl = mk("div", "hc-field");
      vl.append(mk("span", "", "Who can join"));
      const seg = mk("div", "hc-seg");
      seg.id = "hx-vis";
      for (const [v, label] of [["private", "Friends with my link"], ["public", "Everyone"]]) {
        const b = btn(label, "hc-seg-b", () => { vis = v; seg.querySelectorAll(".hc-seg-b").forEach((x) => x.setAttribute("aria-pressed", String(x.dataset.value === vis))); });
        b.dataset.value = v;
        b.setAttribute("aria-pressed", String(v === vis));
        seg.append(b);
      }
      vl.append(seg);
      form.append(vl);
    }
    let gm = null;
    if (needsGm) {
      const gl = mk("label", "hc-field");
      gm = mk("input", "hc-token");
      gm.id = "hx-gm"; gm.type = "text"; gm.maxLength = 16; gm.spellcheck = false; gm.autocomplete = "off"; gm.placeholder = "3 to 16 letters or digits";
      gl.append(mk("span", "", "Your account name (it gets GM)"), gm);
      form.append(gl);
    }
    const sets = mk("div", "hx-sets");
    if (m.settings.length) sets.append(mk("p", "hc-bh", "This mode's settings"));
    for (const s of m.settings) sets.append(field(s, s.default));
    form.append(sets);
    sw.append(form);
    const go = mk("div", "hc-go");
    const ready = kit && kit.is_kit;
    const host = btn(ready ? `Host ${m.name}` : "Get the Host pack first", "btn primary hx-go", () => ready ? hostIt(m, name, gm, sets, host) : getReady());
    host.id = "hx-go";
    go.append(host, mk("span", "hc-fine", PRIVATE.includes(m.profile) ? "Private: only friends with your invite link. The first start takes 5 to 20 minutes."
      : "The first start takes 5 to 20 minutes. Leave this PC on while people play."));
    sw.append(go, mk("p", "status hx-status"));
    root.append(sw);
    name.focus();
  }
  // no Host pack or Docker yet: the ruleset picker's own steps get this PC ready (hostconsole.js paintReady)
  function getReady() { view = "off"; place(); paint(); const r = main.querySelector(".hc-ready, .hc-pre"); if (r) r.scrollIntoView({ block: "nearest" }); }

  async function hostIt(m, nameIn, gmIn, sets, b) {
    const name = nameIn.value.trim();
    if (!NAME_OK.test(name)) { say("Give your realm a name: letters, digits and spaces.", "warn"); nameIn.focus(); return; }
    const gm = gmIn ? gmIn.value.trim() : null;
    if (gmIn && !GM_OK.test(gm)) { say("Type your account name: 3 to 16 letters or digits.", "warn"); gmIn.focus(); return; }
    const vals = values(sets);
    const why = badValue(m, vals);
    if (why) { say(why, "warn"); return; }
    b.disabled = true;
    say("Getting your realm ready…");
    try {
      const u = await call("host_mode_use", { id: m.id, values: vals });
      const prof = await call("host_profiles").catch(() => null);
      const o = Array.isArray(prof) && Array.isArray(prof[0]) ? prof[0].find((x) => x && x.id === u.profile) : null;
      if (o && o.note) throw o.note;
      await call("host_choose", { profile: u.profile, clientId: o && o.clients && o.clients[0] ? o.clients[0].id : null });
      await call("host_start_quiet", { answers: { name, visibility: PRIVATE.includes(u.profile) ? "private" : vis, gm } });
      job = { running: true, step: "Getting ready", n: 0, of: 8, secs: 0, code: null, problem: null };
      live = { mode: m, name, rid: null, online: false, link: null, visibility: PRIVATE.includes(u.profile) ? "private" : vis };
      view = "run";
      paint();
      follow();
    } catch (e) { say(oops(e), "bad"); b.disabled = false; }
  }

  // ---------------------------------------------------------------- the start, then "You're live"
  const fmt = (s) => { const m = Math.floor(s / 60), r = s % 60; return m ? `${m} min ${String(r).padStart(2, "0")} s` : `${r} s`; };
  function paintRun() {
    const sw = mk("div", (live && live.online ? "" : "hc-sw ") + "hx-sw hx-run");
    if (live && live.online) {
      const box = mk("div", "hc-live on hx-live-box");
      const t = mk("div", "hc-live-t");
      t.append(mk("b", "", "You're live"), mk("span", "", live.visibility === "private"
        ? `${live.name} runs ${live.mode.name}. Send friends your invite link: they make their account there, then play.`
        : `${live.name} runs ${live.mode.name} and is on the realm list. Send the link to anyone.`));
      const code = mk("code", "hc-link hx-share", live.link || "");
      code.id = "hx-share";
      t.append(code);
      box.append(t);
      const acts = mk("div", "hx-acts");
      acts.append(btn("Copy link", "btn primary", async (ev) => {
        try { await navigator.clipboard.writeText(live.link); ev.currentTarget.textContent = "Copied"; } catch (_) { say("Select the link and copy it.", "warn"); }
      }));
      acts.append(btn("Open the console", "btn small", () => { view = "grid"; live = null; place(); }));
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
      box.append(h, bar, mk("p", "hc-fine", failed ? (job.problem || "Something went wrong.") : `Hosting ${live ? live.mode.name : "your mode"}. The first start takes 5 to 20 minutes. You can use the rest of Leyline meanwhile.`));
      if (failed) box.append(btn("Back to the modes", "btn small", () => { view = "grid"; live = null; paint(); }));
      sw.append(box);
    }
    sw.append(mk("p", "status hx-status"));
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
        if (live.visibility === "private") { try { live.link = await call("host_invite"); } catch (_) { live.link = SHARE(rid); } }
        else live.link = SHARE(rid);
        place(); paint();
        return;
      }
    }
    liveTimer = setTimeout(watchLive, 8000);
  }

  // ---------------------------------------------------------------- your fork's settings
  function paintEdit() {
    const m = cur;
    const sw = mk("div", "hc-sw hx-sw");
    sw.append(head(m, "Your mode"));
    const form = mk("div", "hx-form");
    const text = (id, label, value, max, ph) => {
      const l = mk("label", "hc-field");
      const i = mk("input", "hc-token");
      i.id = id; i.type = "text"; i.maxLength = max; i.value = value || ""; i.autocomplete = "off";
      if (ph) i.placeholder = ph;
      l.append(mk("span", "", label), i);
      form.append(l);
      return i;
    };
    const name = text("hx-e-name", "Name", m.name, 32);
    const line = text("hx-e-line", "One line about it", m.line, 120);
    const author = text("hx-e-author", "Your name (shown as who made it)", m.author === "Host" ? "" : m.author, 32, "How people know you");
    const jam = text("hx-e-jam", "Mode jam (optional)", m.jam || "", 32, "the jam's short name, like first-jam");
    const sets = mk("div", "hx-sets");
    if (m.settings.length) sets.append(mk("p", "hc-bh", "Its settings (what a host starts with)"));
    for (const s of m.settings) sets.append(field(s, s.default));
    form.append(sets);
    sw.append(form);
    const go = mk("div", "hc-go");
    const save = async (thenHost, b) => {
      const vals = values(sets);
      const why = badValue(m, vals);
      if (why) { say(why, "warn"); return; }
      if (!author.value.trim()) { say("Type your name: it shows as who made it.", "warn"); author.focus(); return; }
      b.disabled = true;
      try {
        const n = await call("host_mode_save", { id: m.id, edits: { name: name.value, line: line.value, author: author.value, jam: jam.value.trim(), defaults: vals } });
        modes = await call("host_modes");
        cur = modes.find((x) => x.id === n.id) || n;
        view = thenHost ? "host" : "grid";
        paint();
        say(thenHost ? "" : `Saved ${n.name}.`, "ok");
      } catch (e) { say(oops(e), "bad"); b.disabled = false; }
    };
    const sb = btn("Save and host", "btn primary hx-save-host", (ev) => save(true, ev.currentTarget));
    const s2 = btn("Save", "btn small hx-save", (ev) => save(false, ev.currentTarget));
    go.append(sb, s2, mk("span", "hc-fine", "Based on another mode: its card says so, and so does your realm on the list."));
    sw.append(go, mk("p", "status hx-status"));
    root.append(sw);
  }

  // ---------------------------------------------------------------- the Realms page: a realm's mode, and Fork this mode
  function realmRow(r, mainBox, acts) {
    const m = r && r.mode;
    if (!m || !m.id) return;
    const line = mk("div", "rl-mode");
    line.append(document.createTextNode(`Mode: ${m.name || nameOf(m.id)}${m.author ? " by " + m.author : ""}`));
    if (m.based_on) line.append(mk("span", "rl-mode-b", ` · based on ${nameOf(m.based_on.id)} by ${m.based_on.author || "someone"}`));
    mainBox.append(line);
    if (m.forkable && m.recipe && acts) {
      const f = btn("Fork this mode", "btn small rl-fork", (ev) => forkIt(m.id, m.recipe, ev.currentTarget));
      f.title = "Copy this realm's mode into your Host pack as yours, and change it";
      acts.append(f);
    }
  }

  window.LeylineHostModes = { realmRow, reload: () => load(), open: (id) => { view = "grid"; show(); if (id) { const m = (modes || []).find((x) => x.id === id); if (m) { cur = m; view = "host"; } } paint(); } };
  load();
})();
