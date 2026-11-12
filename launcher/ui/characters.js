// Leyline: Characters (docs/SINGLE-PLAYER-ONE-CLICK.md), after the way a well-loved action game of 2000 kept them: a
// character of your own on this PC, and an online one on a shared realm, side by side, each one click from play.
//
// Two tabs:
//   * On this PC: each local world (Single player, the original world, Boss Rush: app.rs local_characters) and its
//     characters (read from the world that runs, the others as they were when they last ran). Play starts the right
//     world (set up once, started when stopped; another world running is said, with "Stop it and play") and enters
//     that character (world_play's pick: the game's own character select, signed in by itself). An empty world offers
//     Quick start (the Single player page).
//   * Online: the official realm. Its characters live on the realm; the ones this PC's game folders played there are
//     listed by name (online_characters: the game's own settings folders, nothing else is read). Play joins the realm
//     as its card on the Realms page does (sign-in first).
// Loaded after app.js and vanilla.js (window.LeylineWorlds).
"use strict";
(() => {
  const call = window.__TAURI__ && window.__TAURI__.core
    ? window.__TAURI__.core.invoke
    : async () => { throw "This is a preview page: open it in the Leyline app."; };
  const oops = (e, fb) => (window.LeylineFriendly ? window.LeylineFriendly(e, fb) : (typeof e === "string" ? e : fb || "Something went wrong. Try again."));
  const mk = (tag, cls, text) => { const e = document.createElement(tag); if (cls) e.className = cls; if (text !== undefined) e.textContent = text; return e; };
  const btn = (label, cls, on, id) => { const b = mk("button", cls || "btn small", label); b.type = "button"; if (id) b.id = id; if (on) b.addEventListener("click", on); return b; };
  const byId = (id) => document.getElementById(id);
  const nav = (id) => { if (window.LeylineNav) window.LeylineNav.show(id); };
  const page = byId("page-characters");
  const root = byId("ch");
  if (!page || !root) return;
  const store = { get: (k) => { try { return localStorage.getItem(k); } catch (_) { return null; } },
    set: (k, v) => { try { localStorage.setItem(k, v); } catch (_) { /* private window */ } } };

  // TODO(owner): the page's words
  const LOCAL_LABEL = "On this PC: just you (and friends you invite)";
  const ONLINE_LABEL = "Online: a shared realm";
  let tab = store.get("leyline.chars.tab") === "online" ? "online" : "local";
  let worlds = null;     // local_characters
  let online = null;     // online_characters
  let busy = null;       // "<kind>:<name>" being started
  let ask = null;        // { kind, name, other }
  let job = null;
  let line = ["", ""];

  const W = () => window.LeylineWorlds || null;
  function say(text, kind) { line = [text || "", kind || ""]; const s = byId("ch-status"); if (s) { s.textContent = line[0]; s.className = "status ch-status" + (line[1] ? " " + line[1] : ""); } }
  const played = (secs) => { const h = Math.floor((secs || 0) / 3600); const m = Math.floor(((secs || 0) % 3600) / 60); return h ? `${h} h ${m} min played` : m ? `${m} min played` : "new"; };

  async function read() {
    try { worlds = await call("local_characters"); } catch (_) { worlds = worlds || []; }
    try { online = await call("online_characters"); } catch (_) { online = online || { realm: "", chars: [] }; }
    paint();
  }

  async function play(kind, name, stopOther) {
    const w = W();
    if (!w || busy) return;
    ask = null;
    busy = `${kind}:${name || ""}`;
    paint();
    try {
      const r = await w.run(kind, { pick: name || null, stopOther, say: (t, k) => say(t, k), job: (p) => { job = p; paint(); } });
      const [t, k] = w.played(r, w.WORLDS[kind].title);
      say(name && r && r.auto !== false ? `${r.hero ? "Logged in with your Hero Key. " : ""}Taking ${name} into ${w.WORLDS[kind].title}…` : t, k);
    } catch (e) {
      if (e && e.other) { ask = { kind, name, other: e.other }; say(""); }
      else if (e && e.docker) say("The Built-in engine is made for Windows PCs for now: on this PC, Single player runs in Docker (its page).", "warn");
      else if (e && e.folder) say("Add your game folder first (Game folders): your worlds read their maps from it once.", "warn");
      else say(oops(e, "That world didn't start. Try again."), "bad");
    }
    job = null;
    busy = null;
    read();
  }

  function playOnline(b) {
    const A = window.LeylineApp;
    const r = A && A.realms ? (A.realms() || []).find((x) => x.official) : null;
    if (r && A.playRealm) { A.playRealm(r, b); return; }
    nav("realms");
  }

  function charCard(w, c) {
    const card = mk("li", "ch-card " + (c.side || ""));
    card.dataset.name = c.name;
    const t = mk("div", "ch-card-t");
    t.append(mk("b", "", c.name), mk("span", "ch-fine", `Level ${c.level} ${c.race_label} ${c.class_label}`.replace(/\s+/g, " ").trim()), mk("span", "ch-fine", played(c.played)));
    const key = `${w.kind}:${c.name}`;
    const go = btn(busy === key ? "Starting…" : "Play", "btn small primary", () => play(w.kind, c.name, false));
    go.disabled = !!busy || !w.supported;
    go.setAttribute("aria-label", `Play ${c.name}`);
    card.append(t, go);
    return card;
  }

  function worldBlock(w) {
    const sec = mk("section", "ch-world");
    sec.id = `ch-world-${w.kind}`;
    sec.dataset.kind = w.kind;
    const head = mk("div", "ch-world-h");
    const state = !w.set_up ? ["", "Not set up"] : w.running ? ["ok", "Running"] : ["warn", "Stopped"];
    const chip = mk("span", "sp-dock " + state[0], state[1]);
    head.append(mk("h3", "", w.title), chip);
    sec.append(head);
    if (!w.set_up) {
      sec.append(mk("p", "ch-fine", w.supported ? "Not on this PC yet." : "The Built-in engine is made for Windows PCs for now."));
      const r = mk("div", "sp-row");
      r.append(btn(w.kind === "bossrush" ? "Open Boss Rush" : "Set it up", "btn small", () => nav(w.kind === "bossrush" ? "bossrush" : "single")));
      sec.append(r);
      return sec;
    }
    if (!w.chars.length) {
      sec.append(mk("p", "ch-fine", w.kind === "bossrush" ? "No heroes yet: pick a fight on the Boss Rush page." : "No characters yet."));
      const r = mk("div", "sp-row");
      if (w.kind === "bossrush") r.append(btn("Pick a fight", "btn small", () => nav("bossrush")));
      else {
        r.append(btn("Play", "btn small primary", () => play(w.kind, null, false)), btn("Quick start", "btn small", () => nav("single")));
      }
      sec.append(r);
      return sec;
    }
    const ul = mk("ul", "ch-list");
    for (const c of w.chars) ul.append(charCard(w, c));
    sec.append(ul);
    if (!w.live && w.at) sec.append(mk("p", "ch-fine", "As they were when this world last ran."));
    return sec;
  }

  function localPane() {
    const box = mk("div", "ch-pane");
    box.id = "ch-pane-local";
    box.append(mk("p", "ch-lede", LOCAL_LABEL));
    box.append(mk("p", "ch-fine", "Each world keeps its own characters on this PC. One world runs at a time; your characters are saved when it stops."));
    if (ask) {
      const o = mk("div", "sv-other");
      o.id = "ch-other";
      const t = W() ? W().WORLDS[ask.kind].title : ask.kind;
      o.append(mk("p", "sp-line", `${ask.other.title} is running on this PC, and one world runs at a time. Stop it (its characters are saved) and play ${ask.name || t}?`));
      const r = mk("div", "sp-row");
      r.append(btn(`Stop ${ask.other.title} and play`, "btn small primary", () => play(ask.kind, ask.name, true), "ch-other-go"), btn("Not now", "btn small", () => { ask = null; paint(); }));
      o.append(r);
      box.append(o);
    }
    if (job) {
      const p = mk("p", "ch-fine ch-job", `${job.step || "Getting ready"} · step ${Math.max(1, job.n || 1)} of ${job.of || 7}${typeof job.pct === "number" ? ` · ${job.pct}%` : ""}`);
      p.id = "ch-job";
      box.append(p);
    }
    if (!worlds) { box.append(mk("p", "ch-fine", "Reading your worlds…")); return box; }
    for (const w of worlds) box.append(worldBlock(w));
    return box;
  }

  function onlinePane() {
    const box = mk("div", "ch-pane");
    box.id = "ch-pane-online";
    box.append(mk("p", "ch-lede", ONLINE_LABEL));
    const sec = mk("section", "ch-world");
    sec.id = "ch-world-online";
    const head = mk("div", "ch-world-h");
    const realm = (online && online.realm) || "The official realm";
    head.append(mk("h3", "", realm), mk("span", "sp-dock ok", "Online"));
    sec.append(head, mk("p", "ch-fine", "Your characters there live on the realm, shared with everyone who plays it. You pick one at character select after you log in."));
    const chars = (online && online.chars) || [];
    if (chars.length) {
      const ul = mk("ul", "ch-list");
      for (const c of chars) {
        const li = mk("li", "ch-card");
        li.dataset.name = c.name;
        const t = mk("div", "ch-card-t");
        t.append(mk("b", "", c.name), mk("span", "ch-fine", "Played from this PC"));
        const go = btn("Play", "btn small primary", (ev) => playOnline(ev.currentTarget));
        go.setAttribute("aria-label", `Play ${c.name} on ${realm}`);
        li.append(t, go);
        ul.append(li);
      }
      sec.append(ul);
    } else sec.append(mk("p", "ch-fine", "None played from this PC yet."));
    const r = mk("div", "sp-row");
    r.append(btn(`Play on ${realm}`, "btn small primary", (ev) => playOnline(ev.currentTarget), "ch-online-play"));
    sec.append(r);
    box.append(sec);
    return box;
  }

  function paint() {
    const head = mk("div", "ch-head");
    head.append(mk("h2", "", "Characters"));
    const tabs = mk("div", "page-tabs");
    tabs.setAttribute("role", "tablist");
    tabs.setAttribute("aria-label", "Characters");
    for (const [id, label] of [["local", "On this PC"], ["online", "Online"]]) {
      const t = btn(label, "page-tab", () => { tab = id; store.set("leyline.chars.tab", id); paint(); }, `ch-tab-${id}`);
      t.setAttribute("role", "tab");
      t.setAttribute("aria-selected", String(tab === id));
      tabs.append(t);
    }
    head.append(tabs);
    const s = mk("p", "status ch-status" + (line[1] ? " " + line[1] : ""), line[0]);
    s.id = "ch-status";
    s.setAttribute("role", "status");
    root.replaceChildren(head, tab === "local" ? localPane() : onlinePane(), s);
  }

  document.addEventListener("leyline:page", (e) => { if (e.detail && e.detail.page === "characters") read(); });
  window.LeylineCharacters = { read, play, tab: () => tab };
  paint();
  if (page.classList.contains("on")) read();
})();
