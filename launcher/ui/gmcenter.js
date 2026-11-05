// Leyline: the GM command center (owner 3 Nov: "GM command center available for full command sandbox control").
// One page for full control of YOUR OWN worlds on this PC, in groups: Move (teleport presets, speed, fly), Spawn
// (creatures and objects by search, Clear), World (time of day, weather), Character (level, items by search, revive,
// heal, kill your target) and Players & realm (announce, kick, ban, restart).
//
// Where it acts:
//   * your realm on this PC (the Host pack, AzerothCore): host_gm (host/console.rs) adds one row to the realm's own
//     queue and ov_hostctl.lua runs it as the console: the existing controls (announce, kick, ban, restart...) and
//     "sandbox" (console.rs sandbox_data: fixed verbs, checked numbers, preset places; nothing typed runs as a GM
//     command). A linked server or another core keeps its own console (it says why).
//   * Your Den on Den Lite: its controls are in the game (the Den panel, /den); this says so, and Invite a friend works.
//   * the official realm: never from here. Staff use Leyline HQ (the website checks you are staff).
// Kick, ban, restart, kill and clear ask first. No page menu item: Home's solo row, the Host page and
// window.LeylineGM.open() open it.
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
  const main = document.querySelector("main");
  if (!main) return;

  // the teleport presets: the same words as console.rs SANDBOX_TELE (ov_hostctl.lua maps them to the realm's places)
  const PLACES = [["dalaran", "Dalaran"], ["stormwind", "Stormwind"], ["ironforge", "Ironforge"], ["darnassus", "Darnassus"],
    ["exodar", "The Exodar"], ["orgrimmar", "Orgrimmar"], ["undercity", "Undercity"], ["thunderbluff", "Thunder Bluff"],
    ["silvermoon", "Silvermoon"], ["shattrath", "Shattrath"], ["gmisland", "GM Island"]];
  const WEATHER = [["clear", "Clear"], ["rain", "Rain"], ["snow", "Snow"], ["storm", "Storm"]];
  const NAME_OK = /^[A-Za-z]{2,12}$/;

  const page = mk("section", "page");
  page.id = "page-gm";
  page.innerHTML = `
    <div class="panel wide gm">
      <div class="panel-head"><h2>GM command center</h2></div>
      <p class="lede">Full control of your own worlds on this PC: move, spawn, change the world, help yourself. Only your own worlds.</p>
      <div class="gm-worlds" id="gm-worlds" role="group" aria-label="Which world"></div>
      <p class="status" id="gm-status" role="status" aria-live="polite"></p>
      <div class="gm-body" id="gm-body"></div>
    </div>`;
  const hostPage = document.getElementById("page-host");
  if (hostPage) hostPage.after(page); else main.appendChild(page);
  const $ = (id) => document.getElementById(id);

  function say(text, kind) {
    const s = $("gm-status");
    s.textContent = text || "";
    s.className = "status" + (kind ? " " + kind : "");
  }

  // ---------------------------------------------------------------- the worlds
  let con = null;      // host_console
  let den = null;      // den_state
  let world = null;    // "realm" | "den" | "official"
  async function load() {
    say("Looking at your worlds…");
    [con, den] = await Promise.all([call("host_console").catch(() => null), call("den_state").catch(() => null)]);
    say("");
    const has = worlds();
    if (!world || !has.some((w) => w.id === world)) world = (has.find((w) => w.ok) || has[0]).id;
    paint();
  }
  const realmOk = () => !!(con && con.set_up && !con.linked && con.core === "azerothcore");
  function worlds() {
    const out = [];
    if (con && con.set_up) out.push({ id: "realm", name: con.name || "Your realm", sub: "On this PC (Host pack)", ok: realmOk() });
    if (den && den.lite && (den.lite.available || den.lite.can_get || den.lite.running)) out.push({ id: "den", name: "Your Den", sub: "On this PC (Den Lite)", ok: true });
    out.push({ id: "official", name: "Official realm", sub: "Staff only, on Leyline HQ", ok: false });
    return out;
  }

  function paint() {
    const ws = $("gm-worlds");
    ws.replaceChildren();
    for (const w of worlds()) {
      const b = btn("", "gm-world" + (w.id === world ? " on" : ""), () => { world = w.id; paint(); });
      b.dataset.world = w.id;
      b.setAttribute("aria-pressed", String(w.id === world));
      b.append(mk("b", "", w.name), mk("span", "", w.sub));
      ws.append(b);
    }
    const body = $("gm-body");
    body.replaceChildren();
    if (world === "official") return paintOfficial(body);
    if (world === "den") return paintDen(body);
    if (world === "realm") return paintRealm(body);
    paintNone(body);
  }

  function paintNone(body) {
    const p = mk("div", "gm-empty");
    p.append(mk("p", "", "No world of yours on this PC yet. Open your Den once, and it shows here."),
      btn("Open my Den", "btn primary solo-go", () => { if (window.LeylineNav) window.LeylineNav.show("play"); const g = $("hm-den-go"); if (g) g.click(); }));
    body.append(p);
  }

  function paintOfficial(body) {
    const p = mk("div", "gm-empty gm-official");
    p.append(mk("p", "", "The official realm is never run from here. Its staff tools are on Leyline HQ, which checks you are staff."),
      btn("Open Leyline HQ ›", "btn small", async () => { try { await call("open_link", { name: "hq" }); } catch (e) { say(oops(e), "bad"); } }));
    body.append(p);
  }

  function paintDen(body) {
    const p = mk("div", "gm-empty gm-den");
    p.append(mk("p", "", "Your Den on Den Lite keeps its controls in the game: open the Den panel (type /den) for speed, fly, weather, building and the rest. The full sandbox below is for your realm on this PC (Host pack)."));
    const acts = mk("div", "gm-acts");
    acts.append(btn("Open my Den", "btn small solo-go", () => { if (window.LeylineDen) { if (window.LeylineNav) window.LeylineNav.show("den"); window.LeylineDen.go(); } }));
    if (window.LeylineSolo) acts.append(window.LeylineSolo.inviteButton((t, k) => say(t, k)));
    p.append(acts);
    body.append(p);
  }

  // ---------------------------------------------------------------- your realm: the groups
  let who = "";   // the character to act on ("" = the GM online)
  function group(title, hint) {
    const g = mk("section", "gm-group");
    g.append(mk("h3", "", title));
    if (hint) g.append(mk("p", "gm-hint", hint));
    return g;
  }
  function row(...kids) { const r = mk("div", "gm-row"); r.append(...kids); return r; }
  function select(id, opts) {
    const s = mk("select", "gm-sel");
    s.id = id;
    for (const [v, l] of opts) { const o = mk("option", "", l); o.value = v; s.append(o); }
    return s;
  }
  function input(id, ph, max) {
    const i = mk("input", "gm-in");
    i.id = id; i.type = "text"; i.placeholder = ph || ""; i.maxLength = max || 30; i.spellcheck = false; i.autocomplete = "off";
    return i;
  }

  // one control: the realm's answer said; `ask` asks first
  async function run(action, data, b, ask) {
    if (ask && !window.confirm(ask)) return null;
    if (b) b.disabled = true;
    say("Sending…");
    try {
      const r = await call("host_gm", { action, data });
      say(r || "Done.", "ok");
      return r;
    } catch (e) { say(oops(e, "Your realm didn't answer. Is it running?"), "bad"); return null; }
    finally { if (b) b.disabled = false; }
  }
  const sandbox = (verb, arg, b, ask) => run("sandbox", `${verb}\t${arg == null ? "" : arg}\t${who}`, b, ask);

  function paintRealm(body) {
    if (!realmOk()) {
      const p = mk("div", "gm-empty");
      p.append(mk("p", "", con && con.linked ? "Your linked server keeps its own GM tools: use its console."
        : "GM controls come with AzerothCore realms. This realm runs another core: use its own console."));
      body.append(p);
      return;
    }
    const top = mk("div", "gm-top");
    const whoIn = input("gm-who", "You (the GM online)", 12);
    whoIn.value = who;
    whoIn.addEventListener("change", () => {
      const v = whoIn.value.trim();
      if (v && !NAME_OK.test(v)) { say("A character name: letters only.", "warn"); whoIn.value = who; return; }
      who = v;
    });
    const wl = mk("label", "gm-who");
    wl.append(mk("span", "", "Acts on"), whoIn);
    top.append(wl);
    if (window.LeylineSolo) top.append(window.LeylineSolo.inviteButton((t, k) => say(t, k)));
    body.append(top);
    if (!con.gm) body.append(mk("p", "status warn gm-off", con.gm_note || "Start your realm first: the controls work while its world server runs."));

    const grid = mk("div", "gm-grid");
    // Move
    const mv = group("Move", "Teleport, run faster, fly.");
    const place = select("gm-tele", PLACES);
    mv.append(row(place, btn("Teleport", "btn small", (ev) => sandbox("tele", place.value, ev.currentTarget))));
    const sp = mk("div", "gm-seg");
    for (const n of [1, 2, 3, 5]) sp.append(btn(`${n}x`, "btn small gm-speed", (ev) => sandbox("speed", n, ev.currentTarget)));
    mv.append(row(mk("span", "gm-l", "Speed"), sp));
    mv.append(row(mk("span", "gm-l", "Fly"), btn("On", "btn small", (ev) => sandbox("fly", "on", ev.currentTarget)),
      btn("Off", "btn small", (ev) => sandbox("fly", "off", ev.currentTarget))));
    // Spawn
    const sw = group("Spawn", "Search, then place it beside you. Spawns go after half an hour, or with Clear.");
    const kind = select("gm-spawn-kind", [["creature", "Creature"], ["object", "Object"]]);
    const q = input("gm-spawn-q", "Name, e.g. Wolf", 30);
    const found = mk("ul", "gm-found");
    found.id = "gm-spawn-found";
    const find = async (b) => {
      found.replaceChildren();
      const r = await sandbox("find-" + kind.value, q.value.trim(), b);
      if (r) { listFound(found, r, kind.value === "creature" ? "spawn-creature" : "spawn-object", "Spawn"); say(""); }
    };
    q.addEventListener("keydown", (e) => { if (e.key === "Enter") find(null); });
    sw.append(row(kind, q, btn("Search", "btn small", (ev) => find(ev.currentTarget))), found,
      row(btn("Clear my spawns", "btn small gm-danger", (ev) => sandbox("clear", "", ev.currentTarget, "Take back everything you spawned here?"))));
    // World
    const wd = group("World", "Time of day (for everyone online until they log in again) and the weather where you stand.");
    const hour = select("gm-hour", Array.from({ length: 24 }, (_, h) => [String(h), `${String(h).padStart(2, "0")}:00`]));
    hour.value = "12";
    wd.append(row(mk("span", "gm-l", "Time"), hour, btn("Set", "btn small", (ev) => sandbox("time", hour.value, ev.currentTarget))));
    const wx = mk("div", "gm-seg");
    for (const [v, l] of WEATHER) wx.append(btn(l, "btn small gm-wx", (ev) => sandbox("weather", v, ev.currentTarget)));
    wd.append(row(mk("span", "gm-l", "Weather"), wx));
    // Character
    const ch = group("Character", "Level, gear and getting back up.");
    const lvl = mk("input", "gm-in gm-num");
    lvl.id = "gm-level"; lvl.type = "number"; lvl.min = "1"; lvl.max = "80"; lvl.value = "80";
    ch.append(row(mk("span", "gm-l", "Level"), lvl, btn("Set", "btn small", (ev) => {
      const n = Number(lvl.value);
      if (!Number.isInteger(n) || n < 1 || n > 80) { say("A level from 1 to 80.", "warn"); return; }
      sandbox("level", n, ev.currentTarget);
    })));
    const iq = input("gm-item-q", "Item name", 30);
    const items = mk("ul", "gm-found");
    items.id = "gm-item-found";
    const findItem = async (b) => { items.replaceChildren(); const r = await sandbox("find-item", iq.value.trim(), b); if (r) { listFound(items, r, "item", "Give"); say(""); } };
    iq.addEventListener("keydown", (e) => { if (e.key === "Enter") findItem(null); });
    ch.append(row(iq, btn("Search", "btn small", (ev) => findItem(ev.currentTarget))), items);
    ch.append(row(btn("Revive", "btn small", (ev) => sandbox("revive", "", ev.currentTarget)),
      btn("Heal", "btn small", (ev) => sandbox("heal", "", ev.currentTarget)),
      btn("Kill my target", "btn small gm-danger", (ev) => sandbox("kill", "", ev.currentTarget, "Kill the creature you have targeted in the game?"))));
    // Players & realm
    const pr = group("Players & realm", "Everyone on your realm: your friends too.");
    const ann = input("gm-announce", "A message for everyone online", 200);
    pr.append(row(ann, btn("Announce", "btn small", (ev) => { if (!ann.value.trim()) { say("Write the message first.", "warn"); return; } run("announce", ann.value.trim(), ev.currentTarget).then((r) => { if (r) ann.value = ""; }); })));
    const kn = input("gm-kick", "Character name", 12);
    pr.append(row(kn, btn("Kick", "btn small gm-danger", (ev) => {
      const n = kn.value.trim();
      if (!NAME_OK.test(n)) { say("A character name: letters only.", "warn"); return; }
      run("kick", n, ev.currentTarget, `Kick ${n} off your realm?`);
    })));
    const bn = input("gm-ban", "Character name", 12);
    const bh = mk("input", "gm-in gm-num");
    bh.id = "gm-ban-hours"; bh.type = "number"; bh.min = "0"; bh.max = "8760"; bh.value = "24"; bh.title = "Hours (0 = for good)";
    const br = input("gm-ban-why", "Why", 160);
    pr.append(row(bn, bh, br, btn("Ban", "btn small gm-danger", (ev) => {
      const n = bn.value.trim(), h = Number(bh.value), why = br.value.trim();
      if (!NAME_OK.test(n)) { say("A character name: letters only.", "warn"); return; }
      if (!Number.isInteger(h) || h < 0 || h > 8760) { say("Hours: 1 to 8760, or 0 for a permanent ban.", "warn"); return; }
      if (why.length < 3) { say("Say why (3 letters or more).", "warn"); return; }
      run("ban", `${n}\t${h}\t${why}`, ev.currentTarget, `Ban ${n}'s whole account ${h === 0 ? "for good" : `for ${h} hours`}?`);
    })));
    pr.append(row(btn("Restart the server", "btn small gm-danger", (ev) => run("restart", "", ev.currentTarget, "Restart your realm? Everyone online gets five minutes' warning.")),
      btn("Cancel the restart", "btn small", (ev) => run("cancel", "", ev.currentTarget))));
    grid.append(mv, sw, wd, ch, pr);
    if (!con.gm) grid.querySelectorAll("button, input, select").forEach((x) => { x.disabled = true; });
    body.append(grid);
  }

  // the realm's search answer ("<entry>\t<name>" lines) as rows with one button each
  function listFound(box, text, verb, label) {
    box.replaceChildren();
    for (const line of String(text).split("\n")) {
      const [entry, ...rest] = line.split("\t");
      if (!/^\d{1,7}$/.test(entry || "")) continue;
      const li = mk("li", "gm-hit");
      li.append(mk("span", "", rest.join(" ") || `#${entry}`), mk("code", "", entry), btn(label, "btn small", (ev) => sandbox(verb, entry, ev.currentTarget)));
      box.append(li);
    }
    if (!box.children.length) box.append(mk("li", "hint", "Nothing found."));
  }

  // ---------------------------------------------------------------- open
  function open(w) {
    if (w) world = w;
    if (window.LeylineNav) window.LeylineNav.show("gm"); else { document.querySelectorAll(".page").forEach((p) => p.classList.toggle("on", p === page)); }
    load();
  }
  new MutationObserver(() => { if (page.classList.contains("on") && !con && !den) load(); }).observe(page, { attributes: true, attributeFilter: ["class"] });
  // the Host page's way in: one button by its tabs
  const head = hostPage && hostPage.querySelector(".panel-head");
  if (head && !document.getElementById("gm-open-host")) {
    const b = btn("GM command center", "btn small gm-open", () => open("realm"));
    b.id = "gm-open-host";
    b.title = "Teleport, spawn, time, weather, fly and more on your realm on this PC";
    head.append(b);
  }
  window.LeylineGM = { open, load, worlds: () => worlds(), PLACES };
})();
