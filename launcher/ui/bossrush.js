// Leyline: Boss Rush (docs/BOSS-RUSH.md; owner 5 Nov: "its own launched mini game mode ... select any fight any raid",
// "in the actual raid ... exactly as-is", "1:1 to the game"). Pick a fight, a difficulty and a hero, one click, and
// the game opens on that fight's own platform.
//
// Two engines behind the one page (host/bossrush.rs):
//   * Exact (first, recommended): the real fight, played by the game server's own boss script, on this PC: the
//     Built-in engine's Boss Rush install (host/engine.rs Kind::BossRush: only the raid maps read from the game folder,
//     its own small database). Set up once (bossrush_setup, its progress from engine_progress kind "bossrush"), then
//     Fight (bossrush_play: the hero made at once, the game signed in to it, onto the platform).
//   * Light (labelled "light version, not exact"): Den Lite's own recreation at the same real spot. No set-up beyond
//     Your Den's own small program; bossrush_play writes the fight for Den Lite, then Your Den's one-click start
//     (den_play_lite) opens it.
// Under them: the local board (bossrush_board: best times per fight, the last runs, a new best marked), the result of
// the last run with Try again, and the fight as a code to share (or a friend's code pasted in: bossrush_code).
//
// Loaded after app.js. Nothing is asked of the core until the page (or Home's row) is opened.
"use strict";
(() => {
  const call = window.__TAURI__ && window.__TAURI__.core
    ? window.__TAURI__.core.invoke
    : async () => { throw "This is a preview page: open it in the Leyline app."; };
  const oops = (e, fb) => (window.LeylineFriendly ? window.LeylineFriendly(e, fb) : (fb || "Something went wrong. Try again."));
  const mk = (tag, cls, text) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  };
  const btn = (label, cls, on) => { const b = mk("button", cls || "btn small", label); b.type = "button"; if (on) b.addEventListener("click", on); return b; };
  const byId = (id) => document.getElementById(id);
  const page = byId("page-bossrush");
  const root = byId("br");
  if (!page || !root) return;

  const BUILD = "3.3.5a";
  const NAME = "Boss Rush";
  // TODO(owner): the final line, his words
  const TITLE = "Any raid's big fight, straight onto its platform: a ready hero, bot companions, the clock running";
  const KEY = "leyline.bossrush.pick";
  const store = { get: (k) => { try { return localStorage.getItem(k); } catch (_) { return null; } },
    set: (k, v) => { try { localStorage.setItem(k, v); } catch (_) { /* private window */ } } };

  let featured = null;     // this week's featured fight (boards.js, from boards_get: the catalog in turn, one a week)
  let shareMsg = ["", ""];  // the last Share's answer
  let cat = null;          // bossrush_catalog
  let board = null;        // bossrush_board
  let job = null;          // engine_progress while Boss Rush's exact engine sets up or starts
  let jobTimer = null;
  let boardTimer = null;
  let busy = false;
  let last = null;         // the last play's answer (the login to type, the code)
  let lastSeen = null;     // the newest run on the board when Fight was pressed (the result card waits for a newer one)
  let line = ["", ""];
  let pick = (() => {
    try { const p = JSON.parse(store.get(KEY) || "null"); if (p && p.fight) return p; } catch (_) { /* default */ }
    return { fight: "frozen-throne", diff: "normal", cls: "mage", spec: "fire", engine: "exact" };
  })();

  const appState = () => (window.LeylineApp && window.LeylineApp.state ? window.LeylineApp.state() : null);
  const folder = () => {
    const s = appState();
    if (!s) return null;
    const list = s.settings.clients.filter((c) => c.build === BUILD);
    return list.find((c) => c.id === s.settings.selected_client) || list[0] || null;
  };
  const fightOf = (k) => (cat ? cat.fights.find((f) => f.key === k) : null);
  const classOf = (w) => (cat ? cat.classes.find((c) => c.word === w) : null);
  const exact = () => (cat && cat.exact) || {};
  const isOn = () => page.classList.contains("on");
  function save() { store.set(KEY, JSON.stringify(pick)); }
  function say(text, kind) { line = [text || "", kind || ""]; const s = byId("br-status"); if (s) { s.textContent = line[0]; s.className = "status br-status" + (line[1] ? " " + line[1] : ""); } }
  function nav(id) { if (window.LeylineNav) window.LeylineNav.show(id); }

  // the pick stays valid as the catalog says: heroic only where the raid has one, light only where there is one
  function fix() {
    if (!cat) return;
    const f = fightOf(pick.fight) || cat.fights[0];
    pick.fight = f.key;
    if (!f.heroic) pick.diff = "normal";
    if (pick.engine === "light" && !f.light) pick.engine = "exact";
    const c = classOf(pick.cls) || cat.classes[0];
    pick.cls = c.word;
    if (!c.specs.some((s) => s[0] === pick.spec)) pick.spec = c.specs[0][0];
    save();
  }

  // ---------------------------------------------------------------- the parts of the page
  function head() {
    const h = mk("div", "br-head");
    const t = mk("div", "br-title");
    t.append(mk("h2", "", NAME), mk("p", "br-tag", TITLE));
    h.append(t);
    return h;
  }

  function engines() {
    const g = mk("div", "br-engines");
    g.setAttribute("role", "radiogroup");
    g.setAttribute("aria-label", "Engine");
    const f = fightOf(pick.fight);
    const one = (which, label, em, text, off) => {
      const on = pick.engine === which;
      const c = btn("", "br-engine" + (on ? " on" : "") + (off ? " off" : ""), () => { if (!off) { pick.engine = which; save(); paint(); } });
      c.id = "br-engine-" + which;
      c.setAttribute("role", "radio");
      c.setAttribute("aria-checked", String(on));
      if (off) c.setAttribute("aria-disabled", "true");
      const h = mk("span", "br-engine-h");
      h.append(mk("b", "", label), mk("em", "", em));
      c.append(h, mk("span", "br-engine-l", text));
      g.append(c);
    };
    one("exact", "Exact", "The real fight",
      "The game server's own script for this fight, on this PC. Set up once: only the raid maps are read from your game folder.", false);
    one("light", "Light version", "Not exact",
      f && f.light ? "Our own quick recreation at the same spot, in Your Den's small server: starts in seconds; the mechanics are ours, not the game's."
        : "This fight has no light version yet.", !(f && f.light) || !(cat && cat.light));
    return g;
  }

  function fights() {
    const g = mk("div", "br-fights");
    g.setAttribute("role", "radiogroup");
    g.setAttribute("aria-label", "Fight");
    for (const f of cat.fights) {
      const on = f.key === pick.fight;
      const b = btn("", "br-fight" + (on ? " on" : ""), () => { pick.fight = f.key; fix(); paint(); });
      b.dataset.fight = f.key;
      b.setAttribute("role", "radio");
      b.setAttribute("aria-checked", String(on));
      const tags = mk("span", "br-tags");
      tags.append(mk("span", "br-chip", f.heroic ? "Normal · Heroic" : "Normal"));
      if (f.light) tags.append(mk("span", "br-chip lite", "Light too"));
      const best = board && board.bests.find((r) => r.fight === f.key && r.engine === pick.engine);
      if (best) tags.append(mk("span", "br-chip best", "Best " + best.clock));
      if (featured && f.key === featured) tags.append(mk("span", "br-chip week", "This week"));
      b.append(mk("b", "", f.title), mk("span", "br-raid", f.raid), mk("span", "br-blurb", f.blurb), tags);
      g.append(b);
    }
    return g;
  }

  function options() {
    const box = mk("div", "br-opts");
    const f = fightOf(pick.fight);
    // difficulty
    const d = mk("div", "br-diff");
    d.setAttribute("role", "radiogroup");
    d.setAttribute("aria-label", "Difficulty");
    for (const [w, label] of [["normal", "Normal"], ["heroic", "Heroic"]]) {
      const off = w === "heroic" && !(f && f.heroic);
      const b = btn(label, "br-seg" + (pick.diff === w ? " on" : ""), () => { if (!off) { pick.diff = w; save(); paint(); } });
      b.dataset.diff = w;
      b.disabled = off;
      b.setAttribute("role", "radio");
      b.setAttribute("aria-checked", String(pick.diff === w));
      if (off) b.title = "This raid has no heroic";
      d.append(b);
    }
    // the hero: class, then its specs
    const h = mk("div", "br-hero");
    const sel = mk("select", "br-class");
    sel.id = "br-class";
    sel.setAttribute("aria-label", "Your hero's class");
    for (const c of cat.classes) {
      const o = mk("option", "", c.label);
      o.value = c.word;
      o.selected = c.word === pick.cls;
      sel.append(o);
    }
    sel.addEventListener("change", () => { pick.cls = sel.value; fix(); paint(); });
    const specs = mk("div", "br-specs");
    specs.setAttribute("role", "radiogroup");
    specs.setAttribute("aria-label", "Spec");
    const c = classOf(pick.cls);
    for (const [w, label, role] of c ? c.specs : []) {
      const b = btn(label, "br-seg" + (pick.spec === w ? " on" : ""), () => { pick.spec = w; save(); paint(); });
      b.dataset.spec = w;
      b.setAttribute("role", "radio");
      b.setAttribute("aria-checked", String(pick.spec === w));
      b.title = role === "tank" ? "Tank" : role === "heal" ? "Healer" : "Damage";
      specs.append(b);
    }
    h.append(mk("span", "br-lbl", "Your hero"), sel, specs);
    const note = mk("p", "br-note", "Level 80, geared, spells on your bars, both specs ready (switch free between pulls). Bot companions take the other nine seats.");
    box.append(mk("span", "br-lbl", "Difficulty"), d, h, note);
    return box;
  }

  // the exact engine's own state: not made for this PC, set up once, starting, running, the other realm running
  function exactCard() {
    const e = exact();
    const c = mk("div", "br-exact");
    c.id = "br-exact";
    if (!e.supported) {
      c.append(mk("p", "br-line", "The exact engine is made for Windows PCs for now. The light version runs here."));
      return c;
    }
    if (job && job.running) {
      const p = mk("div", "br-prog");
      p.id = "br-prog";
      p.append(mk("b", "", `Step ${job.n} of ${job.of}: ${job.step}`));
      if (job.pct !== null && job.pct !== undefined) {
        const bar = mk("div", "br-bar");
        const fill = mk("span", "");
        fill.style.width = Math.max(2, Math.min(100, job.pct)) + "%";
        bar.append(fill);
        p.append(bar);
      }
      if (job.detail) p.append(mk("span", "br-detail", job.detail));
      c.append(p);
      return c;
    }
    if (e.other_running) {
      const p = mk("p", "br-line", (e.other_running === "single" ? "Single player" : "Another realm") + " is running on this PC and uses the same ports.");
      const b = btn("Stop it and start Boss Rush", "btn small", stopOtherAndStart);
      b.id = "br-stop-other";
      c.append(p, b);
      return c;
    }
    if (!e.set_up) {
      // TODO(owner): his words for the set-up
      const p = mk("p", "br-line", "One-time set-up on this PC: the engine (shared with Single player), the raid maps from your game folder, a small database. It can take a while the first time; after that it starts in a minute or two.");
      const b = btn("Set up Boss Rush", "btn primary", setup);
      b.id = "br-setup";
      b.disabled = busy || !folder();
      c.append(p, b);
      if (!folder()) c.append(mk("p", "br-line warn", "Add your game folder first (the play bar's game folders): Boss Rush plays with your own game."));
      return c;
    }
    const row = mk("div", "br-run");
    const dot = mk("span", "br-dot " + (e.running ? "ok" : "off"));
    row.append(dot, mk("span", "", e.running ? "Boss Rush's engine is running" : "Boss Rush's engine is stopped"));
    if (e.running) {
      const s = btn("Stop", "btn small", stop);
      s.id = "br-stop";
      row.append(s);
    } else {
      const s = btn("Start", "btn small", startExact);
      s.id = "br-start";
      s.disabled = busy;
      row.append(s);
    }
    c.append(row);
    return c;
  }

  function fightBar() {
    const bar = mk("div", "br-go");
    const f = fightOf(pick.fight);
    const ready = pick.engine === "light" ? !!(cat && cat.light) : !!(exact().set_up && exact().running);
    const b = btn(pick.engine === "light" ? "Fight (light version)" : "Fight", "btn primary br-fight-btn", fight);
    b.id = "br-go";
    b.disabled = busy || !ready || !folder();
    const what = mk("span", "br-what", f ? `${f.title} · ${pick.diff === "heroic" ? "Heroic" : "Normal"} · ${labelOf()}` : "");
    bar.append(b, what);
    if (pick.engine === "exact" && exact().set_up && !exact().running && !(job && job.running)) bar.append(mk("span", "br-line", "Press Start above first."));
    return bar;
  }

  function labelOf() {
    const c = classOf(pick.cls);
    const s = c && c.specs.find((x) => x[0] === pick.spec);
    return c ? `${s ? s[1] + " " : ""}${c.label}` : "";
  }

  // the last run: the board's newest after this Fight
  function result() {
    if (!board || !board.recent.length || !lastSeen) return null;
    const r = board.recent[0];
    if (lastSeen && r.at <= lastSeen.at && r.ms === lastSeen.ms) return null;
    const c = mk("div", "br-result " + (r.cleared ? "clear" : "wipe"));
    c.id = "br-result";
    c.append(mk("b", "br-big", r.cleared ? "CLEARED" : "WIPED"), mk("span", "", `${r.title} · ${r.difficulty === "heroic" ? "Heroic" : "Normal"} · ${r.clock}`));
    if (r.new_best) c.append(mk("span", "br-chip best", "New best"));
    const again = btn("Try again", "btn small primary", fight);
    again.id = "br-again";
    c.append(again);
    return c;
  }

  function boardPart() {
    const box = mk("div", "br-board");
    box.id = "br-board";
    box.append(mk("h3", "", "Your best times"));
    const list = board ? board.bests.filter((r) => r.engine === pick.engine) : [];
    if (!list.length) box.append(mk("p", "br-line", "No clears yet. Your fastest kill of each fight shows here (kept on this PC)."));
    else {
      const t = mk("table", "br-table");
      const B = window.LeylineBoards;
      for (const r of list) {
        const tr = mk("tr", "");
        tr.append(mk("td", "", r.title), mk("td", "", r.difficulty === "heroic" ? "Heroic" : "Normal"), mk("td", "br-clock", r.clock), mk("td", "", r.class === "-" ? "" : r.class));
        // Share: onto the website's local-run board, only when pressed (a run on this PC is never on a realm's board)
        const td = mk("td", "");
        if (B && r.cleared) {
          const sh = btn("Share", "btn small br-share-run", async () => {
            const name = byId("br-share-name");
            sh.disabled = true;
            try { shareMsg = [await B.share(r, name ? name.value : ""), "ok"]; }
            catch (e) { shareMsg = [oops(e, "That run couldn't be shared."), "bad"]; }
            sh.disabled = false;
            paint();
          });
          sh.dataset.fight = r.fight;
          sh.title = "Put this run on the website's local-run board (marked: played on your PC, not checked by a server)";
          td.append(sh);
        }
        tr.append(td);
        t.append(tr);
      }
      box.append(t);
      if (B) {
        const row = mk("div", "br-share-row");
        const lab = mk("label", "br-line", "Name on the boards ");
        const inp = mk("input", "br-code");
        inp.id = "br-share-name";
        inp.maxLength = 12;
        inp.placeholder = "2 to 12 letters";
        inp.value = B.name() || "";
        inp.addEventListener("input", () => { try { localStorage.setItem("leyline.boards.name", JSON.stringify(inp.value)); } catch (_) { /* private window */ } });
        lab.append(inp);
        row.append(lab);
        box.append(row);
        if (shareMsg[0]) {
          const m = mk("p", "br-line " + shareMsg[1], shareMsg[0]);
          m.id = "br-share-msg";
          box.append(m);
        }
      }
    }
    return box;
  }

  function sharePart() {
    const box = mk("div", "br-share");
    box.append(mk("h3", "", "Share this fight"));
    const code = mk("input", "br-code");
    code.id = "br-code";
    code.readOnly = true;
    code.value = last && last.code ? last.code : "";
    code.placeholder = "Press Fight: the fight's code shows here";
    const copy = btn("Copy", "btn small", () => { if (code.value && navigator.clipboard) navigator.clipboard.writeText(code.value).then(() => say("Copied. A friend pastes it here to play the same fight.", "ok"), () => {}); });
    copy.id = "br-copy";
    const paste = mk("input", "br-code");
    paste.id = "br-paste";
    paste.placeholder = "A friend's code";
    const use = btn("Use code", "btn small", async () => {
      try {
        const p = await call("bossrush_code", { code: paste.value });
        pick = { fight: p.fight, diff: p.diff, cls: p.class, spec: p.spec, engine: p.engine };
        fix();
        say("Their fight is picked. Press Fight.", "ok");
        paint();
      } catch (e) { say(oops(e, "That code didn't work."), "bad"); }
    });
    use.id = "br-use";
    const a = mk("div", "br-share-row");
    a.append(code, copy);
    const b = mk("div", "br-share-row");
    b.append(paste, use);
    box.append(a, b);
    return box;
  }

  function loginPart() {
    if (!last || last.auto || !last.account) return null;
    const p = mk("p", "br-line br-login");
    p.id = "br-login";
    p.append(mk("span", "", "Log in with "), mk("b", "", last.account), mk("span", "", " and the password "), mk("b", "", last.password), mk("span", "", ". Your hero is on the character list."));
    return p;
  }

  document.addEventListener("leyline:boards", (ev) => {
    const d = ev.detail || {};
    const f = typeof d.featured_local === "string" ? d.featured_local : null;
    if (f !== featured) { featured = f; if (cat && page.classList.contains("on")) paint(); }
  });

  function paint() {
    const nodes = [head()];
    if (!cat) {
      nodes.push(mk("p", "br-line", "Getting the fights…"));
    } else {
      fix();
      const top = mk("div", "br-top");
      top.append(engines(), pick.engine === "exact" ? exactCard() : mk("p", "br-line", "The light version uses Your Den's small server: nothing to set up."));
      nodes.push(top, fights(), options(), fightBar());
      const l = loginPart();
      if (l) nodes.push(l);
      const r = result();
      if (r) nodes.push(r);
      const low = mk("div", "br-low");
      low.append(boardPart(), sharePart());
      nodes.push(low);
    }
    const s = mk("p", "status br-status" + (line[1] ? " " + line[1] : ""), line[0]);
    s.id = "br-status";
    s.setAttribute("role", "status");
    nodes.push(s);
    root.replaceChildren(...nodes);
  }

  // ---------------------------------------------------------------- doing things
  async function readCatalog() {
    try { cat = await call("bossrush_catalog"); }
    catch (_) { cat = null; say("Boss Rush's list didn't load. Open this page again in a moment.", "bad"); }
  }
  async function readBoard() {
    try { board = await call("bossrush_board"); } catch (_) { /* keep the last */ }
  }

  function follow() {
    clearTimeout(jobTimer);
    jobTimer = setTimeout(async () => {
      try { job = await call("engine_progress"); } catch (_) { job = null; }
      if (job && job.kind !== "bossrush") job = null;
      if (job && job.running) { paint(); follow(); return; }
      if (job && job.problem) say(oops(job.problem), "bad");
      else if (job && job.code === 0) say("Boss Rush is ready. Pick a fight and press Fight.", "ok");
      job = null;
      await readCatalog();
      paint();
    }, 1000);
  }

  async function setup() {
    const f = folder();
    if (!f || busy) return;
    busy = true;
    try { say(await call("bossrush_setup", { client: f.id }), ""); job = { running: true, n: 1, of: 7, step: "Starting…", kind: "bossrush" }; follow(); }
    catch (e) { say(oops(e), "bad"); }
    busy = false;
    paint();
  }
  async function startExact() {
    if (busy) return;
    busy = true;
    try { say(await call("bossrush_start"), ""); job = { running: true, n: 1, of: 1, step: "Starting the realm", kind: "bossrush" }; follow(); }
    catch (e) { say(oops(e), "bad"); }
    busy = false;
    paint();
  }
  async function stop() {
    busy = true;
    paint();
    try { say(await call("bossrush_stop"), "ok"); } catch (e) { say(oops(e), "bad"); }
    busy = false;
    await readCatalog();
    paint();
  }
  async function stopOtherAndStart() {
    busy = true;
    paint();
    try { await call("engine_stop"); say(await call("bossrush_start"), ""); job = { running: true, n: 1, of: 1, step: "Starting the realm", kind: "bossrush" }; follow(); }
    catch (e) { say(oops(e), "bad"); }
    busy = false;
    paint();
  }

  async function fight() {
    const f = folder();
    if (!f || busy) return;
    busy = true;
    lastSeen = board && board.recent[0] ? { at: board.recent[0].at, ms: board.recent[0].ms } : { at: 0, ms: -1 };
    paint();
    say(pick.engine === "light" ? "Getting the light version ready…" : "Getting your hero ready…", "");
    try {
      last = await call("bossrush_play", { client: f.id, fight: pick.fight, diff: pick.diff, class: pick.cls, spec: pick.spec, engine: pick.engine });
      if (last.engine === "light") {
        const d = await call("den_play_lite", { client: f.id });
        say((d && d.message) || "Starting your game…", "ok");
      } else {
        say(`${last.hero ? last.hero + (last.made ? " is made" : " is ready") + ". " : ""}${last.message || "Starting your game…"}`, "ok");
      }
      watchBoard();
    } catch (e) { say(oops(e), "bad"); }
    busy = false;
    paint();
  }

  // while the page is open after a Fight: the board read now and then, so the result card shows when the run ends
  function watchBoard() {
    clearInterval(boardTimer);
    boardTimer = setInterval(async () => {
      if (!isOn()) { clearInterval(boardTimer); boardTimer = null; return; }
      await readBoard();
      paint();
    }, 10000);
  }

  async function load() {
    paint();
    await Promise.all([readCatalog(), readBoard()]);
    if (!jobTimer) {
      try { const p = await call("engine_progress"); if (p && p.running && p.kind === "bossrush") { job = p; follow(); } } catch (_) { /* none */ }
    }
    paint();
  }

  // Home's row (index.html #hm-bossrush)
  const homeGo = byId("hm-bossrush-go");
  if (homeGo) homeGo.addEventListener("click", () => nav("bossrush"));
  document.addEventListener("leyline:page", (e) => { if (e.detail && e.detail.page === "bossrush") load(); });
  window.LeylineBossRush = { load, paint, fight, NAME, TITLE, pick: () => Object.assign({}, pick) };
  if (isOn()) load(); else paint();
})();
