// Leyline: the host console's Network tab (src-tauri/src/host/network.rs; docs/leyline/join-network.md and
// be-a-node.md). How this realm shows on the Leyline network (listed, Hero Key sign-in with its self-test, who's online
// and the opt-in to share names, the mods players need, house rules, players who came in through the network), and
// "Help carry the network" for this PC. Adds its own tab beside the Host page's (hostconsole.js keeps the others).
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
  const tablist = page && page.querySelector(".page-tabs");
  const after = byId("hc-pane-guide");
  if (!page || !tablist || !after) return;

  // ---------------------------------------------------------------- the tab
  const tabBtn = mk("button", "page-tab", "Network");
  Object.assign(tabBtn, { type: "button", id: "hc-tab-network", tabIndex: -1 });
  tabBtn.setAttribute("role", "tab");
  tabBtn.dataset.net = "1";
  tabBtn.setAttribute("aria-controls", "hc-pane-network");
  tabBtn.setAttribute("aria-selected", "false");
  const guideTab = byId("hc-tab-guide");
  if (guideTab && guideTab.nextSibling) tablist.insertBefore(tabBtn, guideTab.nextSibling); else tablist.append(tabBtn);
  const pane = mk("div", "hc-pane hc-net");
  pane.id = "hc-pane-network";
  pane.setAttribute("role", "tabpanel");
  pane.setAttribute("aria-labelledby", "hc-tab-network");
  after.after(pane);
  let shown = false;
  function show(on) {
    shown = on;
    tabBtn.setAttribute("aria-selected", String(on));
    tabBtn.tabIndex = on ? 0 : -1;
    if (on) {
      for (const b of tablist.querySelectorAll(".page-tab")) if (b !== tabBtn) { b.setAttribute("aria-selected", "false"); b.tabIndex = -1; }
      for (const p of page.querySelectorAll(".hc-pane")) p.classList.toggle("on", p === pane);
      load();
    }
  }
  tabBtn.addEventListener("click", () => show(true));
  tabBtn.addEventListener("keydown", (e) => {
    if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
    e.preventDefault();
    const all = [...tablist.querySelectorAll(".page-tab")];
    const t = all[(all.indexOf(tabBtn) + (e.key === "ArrowRight" ? 1 : -1) + all.length) % all.length];
    t.click(); t.focus();
  });
  tablist.addEventListener("click", (e) => { const t = e.target.closest(".page-tab"); if (t && t !== tabBtn) show(false); });

  // ---------------------------------------------------------------- pieces
  const box = (title, ...kids) => { const b = mk("section", "hc-box hc-net-box"); b.append(mk("p", "hc-bh", title), ...kids); return b; };
  const line = (text, cls) => mk("p", "hc-net-line" + (cls ? " " + cls : ""), text);
  const pill = (ok, text) => { const s = mk("span", "hc-net-pill " + (ok ? "ok" : "off"), text); return s; };
  function seg(options, current, on) {
    const g = mk("div", "hc-seg");
    g.setAttribute("role", "group");
    for (const [v, label] of options) {
      const b = mk("button", "hc-seg-b", label);
      b.type = "button";
      b.setAttribute("aria-pressed", String(v === current));
      b.addEventListener("click", () => { if (v !== current) on(v, b); });
      g.append(b);
    }
    return g;
  }
  const status = mk("p", "hc-net-status");
  status.setAttribute("role", "status");
  const say = (t, bad) => { status.textContent = t || ""; status.classList.toggle("bad", !!bad); };
  async function act(what, arg, b) {
    if (b) b.disabled = true;
    try { say(await call("host_network_set", { what, arg: arg === undefined ? null : arg })); await load(true); }
    catch (e) { say(oops(e, "That did not save. Try again."), true); }
    finally { if (b) b.disabled = false; }
  }

  // ---------------------------------------------------------------- the realm on the network
  let net = null, carry = null, rules = [], tick = null;
  async function load(keepStatus) {
    try { net = await call("host_network"); } catch (e) { net = null; if (!keepStatus) say(oops(e, "Could not read your realm."), true); }
    try { carry = await call("host_carry"); } catch (e) { carry = null; }
    try { rules = (net && net.set_up) ? await call("host_rules") : []; } catch (e) { rules = []; }
    paint();
    if (!tick) tick = setInterval(() => { if (shown && !document.hidden) refreshCarry(); }, 10000);
  }
  async function refreshCarry() {
    try { carry = await call("host_carry"); const l = byId("hc-net-carrying"); if (l) l.textContent = carryingLine(); } catch (e) { /* the next tick */ }
  }

  function realmBoxes(n) {
    const out = [];
    const head = mk("div", "hc-net-pills");
    head.append(pill(n.listed, n.listed ? "Listed" : (n.public ? "Listing soon" : "Not listed")),
      pill(n.herokey_live, n.herokey_live ? "Hero Key sign-in on" : "Hero Key sign-in off"),
      pill(n.connected, n.connected ? "Connected" : "Not connected"));
    out.push(head);
    out.push(box("On the realm list",
      line(n.public ? "Players see your realm on the Leyline realm list and can hop to it." : "Only people with your invite link can find your realm."),
      seg([["on", "Listed"], ["off", "Not listed"]], n.public ? "on" : "off", (v, b) => act(v === "on" ? "listed-on" : "listed-off", undefined, b))));
    if (!n.official) {
      const hk = box("Hero Key sign-in",
        line("Players signed in on the Leyline website get in with no new account: each gets one here, named HK and 14 letters."),
        seg([["on", "On"], ["off", "Off"]], n.herokey_on ? "on" : "off", (v, b) => act(v === "on" ? "herokey-on" : "herokey-off", undefined, b)));
      const facts = mk("ul", "hc-net-facts");
      const fact = (ok, yes, no) => { const li = mk("li", ok ? "ok" : "bad", ok ? yes : no); facts.append(li); };
      fact(n.herokey_key, "The website's key is pinned.", "The website's key is not pinned yet.");
      fact(n.herokey_db, "Leyline can make accounts on your server.", "Leyline has no login database for your server: run Join the network.");
      fact(n.herokey_live, "The relay offers Hero Key sign-in for your realm.", "The relay does not offer it for your realm yet.");
      hk.append(facts);
      const row = mk("div", "row");
      const test = btn("Test Hero Key sign-in", "btn small", async () => {
        test.disabled = true; results.replaceChildren(line("Testing every step (nothing is kept)...", "muted"));
        try {
          const r = await call("host_herokey_test");
          results.replaceChildren(...r.steps.map((s) => mk("li", s.ok ? "ok" : "bad", s.text)));
          say(r.ok ? "Every step passed: Hero Key players can get in." : "Some steps did not pass: each line says what to do.", !r.ok);
        } catch (e) { results.replaceChildren(); say(oops(e, "The test did not run."), true); }
        finally { test.disabled = false; }
      });
      test.id = "hc-net-hktest";
      row.append(test);
      if (!n.herokey_key) row.append(btn("Get the website's key", "btn small ghost", (ev) => act("herokey-pin", undefined, ev.currentTarget)));
      const results = mk("ul", "hc-net-facts hc-net-results");
      results.id = "hc-net-hkresults";
      hk.append(row, results);
      out.push(hk);
      const who = box("Who's online",
        line(n.players == null ? "The count shows once your realm is on the list." : `${n.players} playing now.`));
      const lab = mk("label", "hc-net-check");
      const cb = mk("input");
      cb.type = "checkbox"; cb.checked = !!n.share_names; cb.id = "hc-net-names";
      cb.addEventListener("change", () => act(cb.checked ? "names-on" : "names-off"));
      lab.append(cb, document.createTextNode(" Show their character names on the realm list (never GMs or bots, at most 50)"));
      who.append(lab);
      if (n.share_names && n.who.length) who.append(line(n.who.join(", "), "hc-net-names"));
      out.push(who);
    }
    out.push(modsBox(n));
    out.push(rulesBox());
    out.push(box("Players from the network",
      line(`${n.arrivals_hour} logins came in through the Leyline network in the last hour, ${n.arrivals_day} today.`),
      line("No names or addresses: just the count.", "muted")));
    return out;
  }

  function modsBox(n) {
    const b = box("Mods players need");
    b.append(line("The mods your realm runs or asks for. Players' launchers offer the ones in the Workshop when they join: names only, never files."));
    const list = mk("div", "hc-net-mods");
    let mods = (n.mods || []).map((m) => Object.assign({}, m));
    function draw() {
      list.replaceChildren();
      mods.forEach((m, i) => {
        const r = mk("div", "hc-net-mod");
        const id = mk("code", "", m.id);
        const name = mk("input");
        Object.assign(name, { type: "text", value: m.name || "", maxLength: 40, placeholder: "Name players see" });
        name.setAttribute("aria-label", `Name of ${m.id}`);
        name.addEventListener("input", () => { m.name = name.value; });
        const req = seg([["req", "Required"], ["opt", "Optional"]], m.required ? "req" : "opt", (v) => { m.required = v === "req"; draw(); });
        const cl = mk("label", "hc-net-check");
        const c = mk("input"); c.type = "checkbox"; c.checked = m.client !== false;
        c.addEventListener("change", () => { m.client = c.checked; });
        cl.append(c, document.createTextNode(" Players install it"));
        r.append(id, name, req, cl, btn("Remove", "btn small ghost", () => { mods.splice(i, 1); draw(); }));
        list.append(r);
      });
      if (!mods.length) list.append(line("No mods listed.", "muted"));
    }
    draw();
    const add = mk("input");
    Object.assign(add, { type: "text", placeholder: "Workshop id, like up:skada", maxLength: 70, id: "hc-net-modid" });
    add.setAttribute("aria-label", "Mod id to add");
    const row = mk("div", "row");
    row.append(add, btn("Add", "btn small ghost", () => {
      const v = add.value.trim();
      if (!/^(mod:[a-z][a-z0-9-]{2,31}|up:[a-z0-9][a-z0-9._-]{0,63}|mode:[a-z][a-z0-9-]{1,31})$/.test(v)) { say("That is not a mod id: copy it from the Workshop, like up:skada.", true); return; }
      if (!mods.some((m) => m.id === v)) mods.push({ id: v, name: "", required: false, client: !v.startsWith("mode:") });
      add.value = ""; draw();
    }), btn("Save mods", "btn small", (ev) => act("mods", JSON.stringify(mods), ev.currentTarget)));
    b.append(list, row);
    return b;
  }

  function rulesBox() {
    const ta = mk("textarea", "hc-net-rules");
    ta.rows = 5; ta.value = rules.join("\n"); ta.id = "hc-net-rules";
    ta.setAttribute("aria-label", "House rules, one per line");
    const b = box("House rules", line("One per line, up to 10. Shown on the realm list and in game before players hop in."), ta);
    const row = mk("div", "row");
    row.append(btn("Save rules", "btn small", async (ev) => {
      const lines = ta.value.split("\n").map((l) => l.trim()).filter(Boolean);
      if (lines.length > 10 || lines.some((l) => l.length > 120)) { say("Up to 10 rules of 120 characters each.", true); return; }
      ev.currentTarget.disabled = true;
      try { say(await call("host_rules_set", { lines })); } catch (e) { say(oops(e, "The rules did not save."), true); }
      finally { ev.currentTarget.disabled = false; }
    }));
    b.append(row);
    return b;
  }

  // ---------------------------------------------------------------- Help carry the network
  function carryingLine() {
    if (!carry || !carry.settings.on) return "Off.";
    const n = carry.node || {};
    if (!n.running) return "Starting, or not running: check Docker.";
    const why = { owner: "Resting while you use this PC or it runs on battery.", banned: "Stopped by the Leyline team.", kill: "Paused for everyone by the Leyline team." }[n.paused];
    const p = n.players === 1 ? "1 player" : `${n.players} players`;
    return why ? `${why} Carrying ${p} who were already here.` : `Carrying ${p} now, ${Math.round(n.kbps || 0)} KB/s.`;
  }
  function carryBox() {
    const s = carry ? carry.settings : { on: false, public_host: "", region: "", kbps: 512, max_players: 20, only_idle: true };
    const b = box("Help carry the network",
      line("Passes other players' game connections through this PC, so the network holds more people. It never stores or shares game files."));
    b.id = "hc-net-carry";
    if (carry && !carry.offered && !s.on) {
      b.append(line("Not open yet: it opens for everyone at once.", "muted"));
      return b;
    }
    const field = (label, input) => { const l = mk("label", "hc-net-field"); l.append(mk("span", "", label), input); return l; };
    const host = mk("input"); Object.assign(host, { type: "text", value: s.public_host, placeholder: "node.example.org", id: "hc-net-host" });
    const kbps = mk("input"); Object.assign(kbps, { type: "number", min: 64, max: 100000, value: s.kbps, id: "hc-net-kbps" });
    const max = mk("input"); Object.assign(max, { type: "number", min: 1, max: 500, value: s.max_players, id: "hc-net-max" });
    const region = mk("select"); region.id = "hc-net-region";
    for (const [v, t] of [["", "Pick later"], ["na-east", "North America east"], ["na-west", "North America west"], ["sa", "South America"], ["eu", "Europe"], ["africa", "Africa"], ["me", "Middle East"], ["asia", "Asia"], ["oce", "Oceania"]]) {
      const o = mk("option", "", t); o.value = v; o.selected = v === (s.region || ""); region.append(o);
    }
    const idle = mk("input"); idle.type = "checkbox"; idle.checked = s.only_idle !== false; idle.id = "hc-net-idle";
    const idleL = mk("label", "hc-net-check"); idleL.append(idle, document.createTextNode(" Only while this PC is idle and plugged in"));
    const grid = mk("div", "hc-net-grid");
    grid.append(field("This PC's public name", host), field("Region", region), field("Upload, KB/s", kbps), field("Players at most", max));
    const live = line(carryingLine(), "hc-net-carrying");
    live.id = "hc-net-carrying";
    live.setAttribute("aria-live", "polite");
    const go = btn(s.on ? "Turn off" : "Start helping", s.on ? "btn small danger" : "btn small", async () => {
      go.disabled = true;
      const want = { on: !s.on, public_host: host.value.trim(), region: region.value, kbps: Number(kbps.value) || 512, max_players: Number(max.value) || 20, only_idle: idle.checked };
      try { say(await call("host_carry_set", { settings: want })); await load(true); }
      catch (e) { say(oops(e, "That did not work. Try again."), true); go.disabled = false; }
    });
    go.id = "hc-net-carry-go";
    const save = s.on ? btn("Save caps", "btn small ghost", async (ev) => {
      ev.currentTarget.disabled = true;
      try { say(await call("host_carry_set", { settings: { on: true, public_host: host.value.trim(), region: region.value, kbps: Number(kbps.value) || 512, max_players: Number(max.value) || 20, only_idle: idle.checked } })); await load(true); }
      catch (e) { say(oops(e, "That did not save."), true); }
    }) : null;
    const row = mk("div", "row"); row.append(go); if (save) row.append(save);
    b.append(live, grid, idleL, row,
      line("Needs a public name for this PC and ports 80, 443 and 41000 to 41051 open to the internet. Players' chat passes through in the clear; passwords never do.", "muted"));
    return b;
  }

  function paint() {
    pane.replaceChildren();
    const top = mk("div", "hc-net-top");
    top.append(mk("h3", "hc-net-h", "Your realm on the Leyline network"));
    pane.append(top, status);
    if (net && net.set_up) pane.append(...realmBoxes(net));
    else pane.append(box("Your realm", line("Set up your realm (the Start tab) or link a server you already run first. You can still help carry the network below.")));
    pane.append(carryBox());
  }
  window.addEventListener("leyline:kit", () => { if (shown) load(true); });
})();
