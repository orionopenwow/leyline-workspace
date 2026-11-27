// Leyline: Characters (docs/SINGLE-PLAYER-ONE-CLICK.md), after the way a well-loved action game of 2000 kept them: a
// character of your own on this PC, and an online one on a shared realm, side by side, each one click from play.
//
// Two tabs:
//   * On this PC: each local world (Single player, the original world, Boss Rush: app.rs local_characters) and its
//     characters (read from the world that runs, the others as they were when they last ran). Play starts the right
//     world (set up once, started when stopped; another world running is said, with "Stop it and play") and enters
//     that character (world_play's pick: the game's own character select, signed in by itself). An empty world offers
//     Quick start (the Single player page).
//     Each character also shows its last backup (a signed save, host/charsave.rs) and which friends hold a sealed
//     copy; each world has Back up now; Bring to another world picks a world, previews what comes, what goes to the
//     vault and what stays, then brings it (chars_saves, chars_backup_now, chars_bring_preview, chars_bring).
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
  let saves = null;      // chars_saves
  let bring = null;      // { id, name, from, to, plan, busy }

  const W = () => window.LeylineWorlds || null;
  function say(text, kind) { line = [text || "", kind || ""]; const s = byId("ch-status"); if (s) { s.textContent = line[0]; s.className = "status ch-status" + (line[1] ? " " + line[1] : ""); } }
  const played = (secs) => { const h = Math.floor((secs || 0) / 3600); const m = Math.floor(((secs || 0) % 3600) / 60); return h ? `${h} h ${m} min played` : m ? `${m} min played` : "new"; };

  async function read() {
    try { worlds = await call("local_characters"); } catch (_) { worlds = worlds || []; }
    try { online = await call("online_characters"); } catch (_) { online = online || { realm: "", chars: [] }; }
    try { saves = await call("chars_saves"); } catch (_) { saves = saves || { chars: [] }; }
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

  const ago = (t) => {
    const s = Math.max(0, Math.floor(Date.now() / 1000) - (t || 0));
    if (s < 90) return "just now";
    if (s < 5400) return `${Math.round(s / 60)} min ago`;
    if (s < 172800) return `${Math.round(s / 3600)} h ago`;
    return `${Math.round(s / 86400)} days ago`;
  };
  const saveOf = (kind, name) => ((saves && saves.chars) || []).find((x) => x.world === kind && x.name === name) || null;
  // the worlds a character can be brought into (Boss Rush's heroes are its own)
  const TAKES = [["single", "Single player"], ["vanilla", "Original world"]];
  function backupLine(sv) {
    if (!sv) return "Not backed up yet.";
    const who = sv.holders && sv.holders.length ? ` · copies with ${sv.holders.join(", ")}` : "";
    return `Backed up ${ago(sv.made)}${who}`;
  }

  async function backUp(kind) {
    say("Backing up…");
    try { say(await call("chars_backup_now", { kind }), "ok"); } catch (e) { say(oops(e, "That didn't work. Try again."), "bad"); }
    read();
  }

  function openBring(sv, from) {
    const to = TAKES.find(([k]) => k !== from && (worlds || []).some((w) => w.kind === k && w.set_up));
    bring = { id: sv.id, name: sv.name, from, to: to ? to[0] : "", plan: null, busy: false };
    say("");
    paint();
  }

  async function previewBring() {
    if (!bring || !bring.to) return;
    bring.busy = true; bring.plan = null; paint();
    try { bring.plan = await call("chars_bring_preview", { id: bring.id, to: bring.to }); }
    catch (e) { say(oops(e, "That world couldn't be read. Try again."), "bad"); }
    if (bring) bring.busy = false;
    paint();
  }

  async function doBring() {
    if (!bring || !bring.plan || !bring.plan.ok) return;
    bring.busy = true; paint();
    try { say(await call("chars_bring", { id: bring.id, to: bring.to }), "ok"); bring = null; }
    catch (e) { say(oops(e, "Nothing was brought. Try again."), "bad"); if (bring) bring.busy = false; }
    read();
  }

  function bringPanel() {
    const b = mk("div", "sv-other ch-bring");
    b.id = "ch-bring";
    b.append(mk("p", "sp-line", `Bring ${bring.name} to another world`));
    const choices = TAKES.filter(([k]) => k !== bring.from && (worlds || []).some((w) => w.kind === k && w.set_up));
    if (!choices.length) {
      b.append(mk("p", "ch-fine", "Set up another world first (Single player or the original world)."));
      const r = mk("div", "sp-row");
      r.append(btn("Close", "btn small", () => { bring = null; paint(); }, "ch-bring-close"));
      b.append(r);
      return b;
    }
    const sel = mk("select", "ch-bring-to");
    sel.id = "ch-bring-to";
    sel.setAttribute("aria-label", "Bring to");
    for (const [k, t] of choices) { const o = mk("option", "", t); o.value = k; if (k === bring.to) o.selected = true; sel.append(o); }
    sel.addEventListener("change", () => { bring.to = sel.value; bring.plan = null; paint(); });
    b.append(sel);
    const p = bring.plan;
    if (p && !p.ok) b.append(mk("p", "ch-fine bad", p.why));
    if (p && p.ok) {
      const list = (title, items, id) => {
        const box = mk("div", "ch-bring-list");
        box.id = id;
        box.append(mk("b", "", title));
        const ul = mk("ul", "");
        for (const x of items) ul.append(mk("li", "", x));
        if (!items.length) ul.append(mk("li", "", "Nothing."));
        box.append(ul);
        return box;
      };
      b.append(list("What comes", p.comes, "ch-bring-comes"));
      b.append(list("Goes to your vault (kept for a world that has it)", p.vault.map((v) => `${v.id.replace(/^[^:]+:item:/, "Item ")}${v.n > 1 ? ` ×${v.n}` : ""}: ${v.why}`), "ch-bring-vault"));
      b.append(list("Stays behind", p.stays, "ch-bring-stays"));
    }
    const r = mk("div", "sp-row");
    if (p && p.ok) r.append(btn(bring.busy ? "Bringing…" : "Bring", "btn small primary", doBring, "ch-bring-go"));
    else r.append(btn(bring.busy ? "Looking…" : "See what comes", "btn small primary", previewBring, "ch-bring-look"));
    r.append(btn("Not now", "btn small", () => { bring = null; paint(); }, "ch-bring-close"));
    for (const x of r.querySelectorAll("button")) if (bring.busy && x.id !== "ch-bring-close") x.disabled = true;
    b.append(r);
    b.append(mk("p", "ch-fine", "Gold never travels, and items can't be sent between worlds. The world is backed up first, and both worlds must be stopped."));
    return b;
  }

  function charCard(w, c) {
    const card = mk("li", "ch-card " + (c.side || ""));
    card.dataset.name = c.name;
    const t = mk("div", "ch-card-t");
    t.append(mk("b", "", c.name), mk("span", "ch-fine", `Level ${c.level} ${c.race_label} ${c.class_label}`.replace(/\s+/g, " ").trim()), mk("span", "ch-fine", played(c.played)));
    const sv = saveOf(w.kind, c.name);
    const bl = mk("span", "ch-fine ch-backup", backupLine(sv));
    t.append(bl);
    if (sv && w.kind !== "bossrush") {
      const mv = btn("Bring to another world", "linkish ch-bring-open", () => openBring(sv, w.kind));
      mv.setAttribute("aria-label", `Bring ${c.name} to another world`);
      t.append(mv);
    }
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
    const br = mk("div", "sp-row");
    br.append(btn("Back up now", "btn small", () => backUp(w.kind), `ch-backup-${w.kind}`));
    sec.append(br);
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
    if (bring) box.append(bringPanel());
    if (!worlds) { box.append(mk("p", "ch-fine", "Reading your worlds…")); return box; }
    for (const w of worlds) box.append(worldBlock(w));
    box.append(backupsBlock());
    return box;
  }

  // characters brought back from friends' sealed copies (not in a world on this PC yet), and asking for them
  function backupsBlock() {
    const sec = mk("section", "ch-world");
    sec.id = "ch-world-backups";
    const head = mk("div", "ch-world-h");
    head.append(mk("h3", "", "Your backups"));
    sec.append(head);
    const loose = ((saves && saves.chars) || []).filter((x) => !x.world);
    if (loose.length) {
      const ul = mk("ul", "ch-list");
      for (const x of loose) {
        const li = mk("li", "ch-card");
        li.dataset.name = x.name;
        const t = mk("div", "ch-card-t");
        t.append(mk("b", "", x.name), mk("span", "ch-fine", `Saved ${ago(x.made)}, from a friend's copy`));
        const go = btn("Bring to a world", "btn small primary", () => openBring(x, ""));
        go.setAttribute("aria-label", `Bring ${x.name} to a world`);
        li.append(t, go);
        ul.append(li);
      }
      sec.append(ul);
    } else sec.append(mk("p", "ch-fine", "Each character is saved and signed when its world stops, and every 30 minutes while it runs. A sealed copy goes to friends you play with: they can't open it."));
    const r = mk("div", "sp-row");
    r.append(btn("Get my characters back from friends", "btn small", async () => {
      try { say(await call("chars_ask_back"), "ok"); } catch (e) { say(oops(e, "That didn't work. Try again."), "bad"); }
      setTimeout(read, 4000);
    }, "ch-ask-back"));
    sec.append(r);
    return sec;
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
    // lite (lite.js): the official realm is gone, so only this PC's worlds
    const lite = !!(window.LeylineLite && window.LeylineLite.lite);
    if (lite) tab = "local";
    for (const [id, label] of (lite ? [["local", "On this PC"]] : [["local", "On this PC"], ["online", "Online"]])) {
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
  window.LeylineCharacters = { read, play, backUp, tab: () => tab };
  paint();
  if (page.classList.contains("on")) read();
})();
