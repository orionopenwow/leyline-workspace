// Leyline: Boss Rush, start anywhere (owner 12 Nov: "Boss Rush should allow you to start in front of any boss or any
// raid wing or the entire raid itself with companions or with friends in a one-click UI that's convenient"). A part
// of its own on the Boss Rush page (#bs, above bossrush.js's #br), so the two never paint over each other:
//
//   * where: a raid (tabs), then the whole raid, a wing, or one boss (the list kit's rows, grouped by wing; search);
//   * how: Normal / Heroic (heroic only where the raid has one), 10 or 25 players;
//   * who: With companions (the empty seats filled at once: tank / healer / damage icons with their counts) or With
//     friends (Invite a friend: the same invite link as a world's, the friend's hero picked here; companions fill the
//     seats left unless that's turned off), and your hero (class, then spec);
//   * one big Start (bossrush_start_at: the hero made or picked, the game signed in, the raid lands at the start);
//   * then the raid as the realm sees it (bossrush_state, read every few seconds while the page is open): where it
//     stands, who is in, the bosses down, the clock; waiting for friends with Start now (bossrush_go_now); after a
//     wipe Retry from this boss (bossrush_retry: back in front of the first boss still standing), after a clear Again.
//
// Only the exact engine plays it (the real raid): bossrush.js's card below sets Boss Rush up and starts it. Nothing is
// asked of the core until the page is opened. Text only (textContent); words are ours (docs/BOSS-RUSH.md).
"use strict";
(() => {
  const call = window.__TAURI__ && window.__TAURI__.core
    ? window.__TAURI__.core.invoke
    : async () => { throw "This is a preview page: open it in the Leyline app."; };
  const oops = (e, fb) => (window.LeylineFriendly ? window.LeylineFriendly(e, fb) : (fb || "Something went wrong. Try again."));
  const mk = (tag, cls, text) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined && text !== null) e.textContent = text;
    return e;
  };
  const btn = (label, cls, on) => { const b = mk("button", cls || "btn small", label); b.type = "button"; if (on) b.addEventListener("click", on); return b; };
  const byId = (id) => document.getElementById(id);
  const page = byId("page-bossrush");
  const root = byId("bs");
  if (!page || !root) return;

  const BUILD = "3.3.5a";
  const KEY = "leyline.bossstart.pick";
  const store = { get: (k) => { try { return localStorage.getItem(k); } catch (_) { return null; } },
    set: (k, v) => { try { localStorage.setItem(k, v); } catch (_) { /* private window */ } } };
  // TODO(owner): the section's line, his words
  const LINE = "In front of any boss, at the start of a wing, or the whole raid from its gate: with companions or with friends.";

  // The seats companions fill (host/bossrush.rs companions_for, the module's DefaultSeats / DefaultSeats25 and
  // BotSeats): each player takes the first seat of their role; a role with no seat left takes the last damage seat.
  function companionsFor(size, roles, fill) {
    if (!fill) return { tank: 0, heal: 0, dps: 0 };
    const seats = size === 25 ? [...Array(3).fill("tank"), ...Array(6).fill("heal"), ...Array(16).fill("dps")]
      : [...Array(2).fill("tank"), ...Array(3).fill("heal"), ...Array(5).fill("dps")];
    for (const r of roles) {
      if (!seats.length) break;
      let i = seats.indexOf(r);
      if (i < 0) i = seats.lastIndexOf("dps") >= 0 ? seats.lastIndexOf("dps") : seats.length - 1;
      seats.splice(i, 1);
    }
    seats.length = Math.min(seats.length, Math.max(0, size - roles.length));
    const n = (w) => seats.filter((s) => s === w).length;
    return { tank: n("tank"), heal: n("heal"), dps: n("dps") };
  }

  // role marks (drawn, no game art): a shield, a cross, a blade
  const ICONS = {
    tank: '<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path d="M12 2.5l8 3v6.2c0 5-3.4 8.6-8 9.8-4.6-1.2-8-4.8-8-9.8V5.5z"/></svg>',
    heal: '<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path d="M9.2 3h5.6v6.2H21v5.6h-6.2V21H9.2v-6.2H3V9.2h6.2z"/></svg>',
    dps: '<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path d="M19.5 2.5l2 2-9.7 9.7 1.8 1.8-1.4 1.4-1.8-1.8-3.6 3.6.3 2.3-1.4 1.4-2.2-2.9-2.9-2.2 1.4-1.4 2.3.3 3.6-3.6-1.8-1.8 1.4-1.4 1.8 1.8z"/></svg>',
  };
  const ROLE_WORD = { tank: ["tank", "tanks"], heal: ["healer", "healers"], dps: ["damage dealer", "damage dealers"] };
  function roleBadge(role, n, cls) {
    const s = mk("span", "bs-role " + role + (cls ? " " + cls : ""));
    s.insertAdjacentHTML("afterbegin", ICONS[role]);
    s.append(mk("b", "", String(n)));
    s.title = `${n} ${ROLE_WORD[role][n === 1 ? 0 : 1]}`;
    s.setAttribute("aria-label", s.title);
    return s;
  }

  let cat = null;        // bossrush_catalog (raids, fights, classes, the exact engine's state)
  let state = null;      // bossrush_state
  let invite = null;     // bossrush_invite's link
  let busy = false;
  let list = null;       // the list kit's list of starts
  let listRaid = null;
  let timer = null;
  let line = ["", ""];
  let pick = (() => {
    try { const p = JSON.parse(store.get(KEY) || "null"); if (p && p.start) return p; } catch (_) { /* default */ }
    return { raid: "icc", start: "frozen-throne", diff: "normal", size: 10, party: "companions", fill: true, cls: "mage", spec: "fire", fcls: "priest", fspec: "holy" };
  })();
  function save() { store.set(KEY, JSON.stringify(pick)); }
  function say(text, kind) { line = [text || "", kind || ""]; const s = byId("bs-status"); if (s) { s.textContent = line[0]; s.className = "status bs-status" + (line[1] ? " " + line[1] : ""); } }

  const appState = () => (window.LeylineApp && window.LeylineApp.state ? window.LeylineApp.state() : null);
  const folder = () => {
    const s = appState();
    if (!s) return null;
    const l = s.settings.clients.filter((c) => c.build === BUILD);
    return l.find((c) => c.id === s.settings.selected_client) || l[0] || null;
  };
  const raids = () => (cat && Array.isArray(cat.raids) ? cat.raids : []);
  const raidOf = (k) => raids().find((r) => r.key === k) || null;
  const fightOf = (k) => (cat ? cat.fights.find((f) => f.key === k) : null);
  const classOf = (w) => (cat ? cat.classes.find((c) => c.word === w) : null);
  const roleOf = (cls, spec) => { const c = classOf(cls); const s = c && c.specs.find((x) => x[0] === spec); return s ? s[2] : "dps"; };
  const isOn = () => page.classList.contains("on");
  const exact = () => (cat && cat.exact) || {};

  // every start of every raid, as the list's rows: the whole raid, each wing (of more than one boss), each boss
  function startsOf(r) {
    const out = [];
    if (r.whole) out.push({ key: r.whole, raid: r.key, scope: "raid", title: "The whole raid", sub: `${r.title} from its gate: every boss, ${r.fights.length === 1 ? "one fight" : "in order"}`, group: "The whole raid", n: r.fights.length });
    for (const w of r.wings) {
      if (w.key) out.push({ key: w.key, raid: r.key, scope: "wing", title: w.title, sub: `This wing: ${w.fights.length} bosses, from the first`, group: w.title, n: w.fights.length });
      for (const f of w.fights) {
        const fi = fightOf(f);
        if (fi && !out.some((x) => x.key === f)) out.push({ key: f, raid: r.key, scope: "boss", title: fi.title, sub: fi.blurb, group: w.title, n: 1 });
      }
    }
    for (const f of r.fights) {
      const fi = fightOf(f);
      if (fi && !out.some((x) => x.key === f)) out.push({ key: f, raid: r.key, scope: "boss", title: fi.title, sub: fi.blurb, group: r.wings.length ? "More fights" : "Bosses", n: 1 });
    }
    return out;
  }
  const allStarts = () => raids().flatMap(startsOf);
  const startOf = (k) => allStarts().find((s) => s.key === k) || null;

  // the pick stays valid: a start of the picked raid, heroic only where the raid has one, a class and its spec
  function fix() {
    if (!raids().length) return;
    const r = raidOf(pick.raid) || raids()[0];
    pick.raid = r.key;
    const s = startOf(pick.start);
    if (!s || s.raid !== r.key) pick.start = (startsOf(r).find((x) => x.scope === "boss") || startsOf(r)[0]).key;
    if (!r.heroic) pick.diff = "normal";
    if (pick.size !== 25) pick.size = 10;
    if (pick.party !== "friends") pick.party = "companions";
    for (const [ck, sk] of [["cls", "spec"], ["fcls", "fspec"]]) {
      const c = classOf(pick[ck]) || cat.classes[0];
      pick[ck] = c.word;
      if (!c.specs.some((x) => x[0] === pick[sk])) pick[sk] = c.specs[0][0];
    }
  }

  // ---------------------------------------------------------------- drawing
  function head() {
    const h = mk("div", "bs-head");
    const t = mk("div", "bs-title");
    t.append(mk("h2", "", "Start anywhere"), mk("p", "bs-tag", LINE));
    h.append(t);
    return h;
  }

  function raidTabs() {
    const g = mk("div", "bs-raids");
    g.setAttribute("role", "tablist");
    g.setAttribute("aria-label", "Raids");
    for (const r of raids()) {
      const on = r.key === pick.raid;
      const b = btn(r.title, "bs-raid" + (on ? " on" : ""), () => { pick.raid = r.key; pick.start = null; fix(); save(); paint(); });
      b.id = "bs-raid-" + r.key;
      b.setAttribute("role", "tab");
      b.setAttribute("aria-selected", String(on));
      g.append(b);
    }
    return g;
  }

  function scopeChip(s) {
    const c = mk("span", "bs-scope " + s.scope, s.scope === "raid" ? "Whole raid" : s.scope === "wing" ? `Wing · ${s.n} bosses` : "Boss");
    return c;
  }

  // the starts of the picked raid, in the list kit (search, groups by wing, keyboard)
  function startList() {
    const box = mk("div", "bs-list");
    box.id = "bs-list";
    const r = raidOf(pick.raid);
    if (!r) return box;
    const rows = startsOf(r);
    const order = ["The whole raid", ...r.wings.map((w) => w.title), "More fights", "Bosses"];
    list = window.LeylineList ? window.LeylineList.create(box, {
      id: "bossstart-" + r.key,
      noun: ["start", "starts"],
      key: (row) => row.key,
      label: "Where to start",
      search: { fields: (row) => [row.title, row.sub, row.group], placeholder: "Find a boss or a wing", label: "Find a boss or a wing" },
      group: { of: (row) => row.group, order, hideSingle: true },
      role: "listbox",
      render: (row, ctx) => {
        const el = mk("div", "bs-row" + (row.key === pick.start ? " on" : ""));
        el.id = "bs-start-" + row.key;
        const t = mk("div", "bs-row-t");
        const b = mk("b", "");
        b.append(ctx.hl(row.title));
        t.append(b, scopeChip(row));
        el.append(t, mk("span", "bs-row-s", row.sub));
        el.addEventListener("click", () => choose(row.key));
        return el;
      },
      onSelect: (row) => { if (row) choose(row.key, true); },
      primary: (row) => { choose(row.key); start(); },
      empty: (info) => (info.reason === "none" ? "This raid has no starts yet." : undefined),
    }) : null;
    if (list) {
      list.update(rows);
      list.select(pick.start);
      listRaid = r.key;
    } else {
      for (const row of rows) { const b = btn(row.title, "bs-row" + (row.key === pick.start ? " on" : ""), () => choose(row.key)); b.id = "bs-start-" + row.key; box.append(b); }
    }
    return box;
  }

  function choose(key, quiet) {
    if (pick.start === key) return;
    pick.start = key;
    save();
    if (quiet) {   // keyboard / focus: mark the row without a repaint (the list keeps its focus)
      for (const n of root.querySelectorAll(".bs-row")) n.classList.toggle("on", n.id === "bs-start-" + key);
      const w = byId("bs-what");
      if (w) w.replaceWith(what());
      return;
    }
    paint();
  }

  function seg(label, on, cls, run, off) {
    const b = btn(label, "bs-seg" + (on ? " on" : "") + (cls ? " " + cls : ""), () => { if (!off) run(); });
    b.setAttribute("aria-pressed", String(on));
    if (off) b.disabled = true;
    return b;
  }

  function heroPicker(prefix, ck, sk, label) {
    const h = mk("div", "bs-hero");
    const sel = mk("select", "bs-class");
    sel.id = prefix + "-class";
    sel.setAttribute("aria-label", label + ": class");
    for (const c of cat.classes) { const o = mk("option", "", c.label); o.value = c.word; o.selected = c.word === pick[ck]; sel.append(o); }
    sel.addEventListener("change", () => { pick[ck] = sel.value; pick[sk] = classOf(sel.value).specs[0][0]; save(); paint(); });
    const specs = mk("div", "bs-specs");
    for (const [w, lab, role] of classOf(pick[ck]).specs) {
      const b = seg(lab, pick[sk] === w, "", () => { pick[sk] = w; save(); paint(); });
      b.id = `${prefix}-spec-${w}`;
      b.prepend(roleIcon(role));
      b.title = role === "tank" ? "Tank" : role === "heal" ? "Healer" : "Damage";
      specs.append(b);
    }
    h.append(sel, specs);
    return h;
  }
  function roleIcon(role) { const s = mk("span", "bs-ri " + role); s.insertAdjacentHTML("afterbegin", ICONS[role]); return s; }

  function options() {
    const r = raidOf(pick.raid);
    const box = mk("div", "bs-opts");
    // difficulty and size
    const d = mk("div", "bs-segs");
    d.setAttribute("role", "group");
    d.setAttribute("aria-label", "Difficulty");
    const n = seg("Normal", pick.diff === "normal", "", () => { pick.diff = "normal"; save(); paint(); });
    n.id = "bs-diff-normal";
    const h = seg("Heroic", pick.diff === "heroic", "", () => { pick.diff = "heroic"; save(); paint(); }, !r.heroic);
    h.id = "bs-diff-heroic";
    if (!r.heroic) h.title = `${r.title} has no heroic.`;
    d.append(n, h);
    const z = mk("div", "bs-segs");
    z.setAttribute("role", "group");
    z.setAttribute("aria-label", "Players");
    for (const s of [10, 25]) { const b = seg(`${s} players`, pick.size === s, "", () => { pick.size = s; save(); paint(); }); b.id = "bs-size-" + s; z.append(b); }
    // who
    const who = mk("div", "bs-segs");
    who.setAttribute("role", "group");
    who.setAttribute("aria-label", "Who comes");
    const wc = seg("With companions", pick.party === "companions", "big", () => { pick.party = "companions"; save(); paint(); });
    wc.id = "bs-party-companions";
    const wf = seg("With friends", pick.party === "friends", "big", () => { pick.party = "friends"; save(); paint(); });
    wf.id = "bs-party-friends";
    who.append(wc, wf);
    box.append(mk("span", "bs-lbl", "Difficulty"), d, mk("span", "bs-lbl", "Raid size"), z, mk("span", "bs-lbl", "Who comes"), who);
    box.append(mk("span", "bs-lbl", "Your hero"), heroPicker("bs", "cls", "spec", "Your hero"));
    // the companions the raid brings (roles)
    const friends = pick.party === "friends";
    const roles = [roleOf(pick.cls, pick.spec)].concat(friends ? [roleOf(pick.fcls, pick.fspec)] : []);
    const fill = !friends || pick.fill;
    const c = companionsFor(pick.size, roles, fill);
    const team = mk("div", "bs-team");
    team.id = "bs-team";
    const you = mk("span", "bs-you");
    you.append(roleIcon(roles[0]), mk("span", "", "You"));
    team.append(you);
    if (friends) { const fr = mk("span", "bs-you friend"); fr.append(roleIcon(roles[1]), mk("span", "", "Your friend")); team.append(fr); }
    if (fill) {
      const comp = mk("span", "bs-comp");
      comp.append(mk("span", "bs-comp-l", friends ? "Companions in the seats left:" : "Companions:"), roleBadge("tank", c.tank), roleBadge("heal", c.heal), roleBadge("dps", c.dps));
      team.append(comp);
    } else team.append(mk("span", "bs-comp-l", "Players only: the raid starts with whoever is there."));
    box.append(mk("span", "bs-lbl", "The raid"), team);
    if (friends) box.append(mk("span", "bs-lbl", "Friends"), friendsPart());
    return box;
  }

  function friendsPart() {
    const f = mk("div", "bs-friends");
    f.id = "bs-friends";
    const fill = mk("label", "bs-check");
    const cb = mk("input");
    cb.type = "checkbox";
    cb.id = "bs-fill";
    cb.checked = !!pick.fill;
    cb.addEventListener("change", () => { pick.fill = cb.checked; save(); paint(); });
    fill.append(cb, mk("span", "", "Companions fill the seats your friends don't take"));
    const fh = mk("div", "bs-fhero");
    fh.append(mk("span", "bs-lbl", "Your friend plays"), heroPicker("bs-f", "fcls", "fspec", "Your friend's hero"));
    const row = mk("div", "bs-invite-row");
    const inv = btn("Invite a friend", "btn small primary", inviteFriend);
    inv.id = "bs-invite";
    row.append(inv);
    if (invite && invite.link) {
      const code = mk("input", "bs-link");
      code.id = "bs-link";
      code.readOnly = true;
      code.value = invite.link;
      code.setAttribute("aria-label", "Invite link");
      const copy = btn("Copy", "btn small", () => { if (navigator.clipboard) navigator.clipboard.writeText(code.value).then(() => say("Copied. Send it to your friend.", "ok"), () => {}); });
      copy.id = "bs-copy";
      row.append(code, copy);
    }
    const how = mk("p", "bs-note", "Your friend pastes the link in their Leyline (Realms › Got an invite link?), presses Join, and lands beside you as their own hero. Press Start, wait for them, then Start now. Your Friends list can send the link too.");
    f.append(fill, fh, row, how);
    return f;
  }

  function what() {
    const s = startOf(pick.start);
    const r = raidOf(pick.raid);
    const w = mk("span", "bs-what");
    w.id = "bs-what";
    if (s && r) w.textContent = `${s.scope === "boss" ? s.title : s.scope === "wing" ? s.title + " (wing)" : r.title + " (whole raid)"} · ${pick.diff === "heroic" ? "Heroic" : "Normal"} · ${pick.size} players · ${pick.party === "friends" ? "with friends" : "with companions"}`;
    return w;
  }

  function goBar() {
    const bar = mk("div", "bs-go");
    const e = exact();
    const ready = !!(e.set_up && e.running);
    const b = btn("Start", "btn primary bs-start", start);
    b.id = "bs-start";
    b.disabled = busy || !ready || !folder();
    bar.append(b, what());
    if (!e.supported && cat) bar.append(mk("span", "bs-line warn", "Start anywhere plays the real raid: Boss Rush's exact engine, made for Windows PCs for now."));
    else if (!ready) bar.append(mk("span", "bs-line", "Set up and start Boss Rush below first (one time)."));
    else if (!folder()) bar.append(mk("span", "bs-line warn", "Add your game folder first (the play bar's game folders)."));
    return bar;
  }

  const PHASE = { forming: "Your raid is forming", entering: "Entering the raid", preparing: "Getting the raid ready", ready: "Ready", fighting: "Fighting", wiped: "Wiped", cleared: "Cleared" };
  function stateCard() {
    if (!state || state.kind === "offline" || state.kind === "nothing" || state.kind === "no" || state.kind === "ok") return null;
    const c = mk("div", "bs-state " + (state.kind === "run" ? state.phase : "lobby"));
    c.id = "bs-state";
    if (state.kind === "lobby") {
      const s = startOf(state.start);
      c.append(mk("b", "bs-big", "Waiting for friends"), mk("span", "", `${s ? s.title : state.start} · ${state.waiting} of ${state.size} here`));
      const go = btn("Start now", "btn primary", goNow);
      go.id = "bs-go-now";
      const leave = btn("Leave", "btn small", leaveRun);
      leave.id = "bs-leave";
      c.append(go, leave, mk("span", "bs-note", pick.fill ? "Companions take the seats left." : "Players only."));
      return c;
    }
    const at = state.step_title || (startOf(state.step) || {}).title || state.step;
    c.append(mk("b", "bs-big", PHASE[state.phase] || state.phase));
    const facts = mk("span", "bs-facts");
    facts.textContent = [at ? "at " + at : "", `${state.inside} of ${state.members} in the raid${state.bots ? ` (${state.bots} companions)` : ""}`,
      state.of > 1 ? `${state.done} of ${state.of} bosses down` : "", state.phase === "fighting" || state.ms ? state.clock : ""].filter(Boolean).join(" · ");
    c.append(facts);
    if (state.phase === "wiped") {
      const again = btn("Retry from this boss", "btn primary", retry);
      again.id = "bs-retry";
      c.append(again);
    } else if (state.phase === "cleared") {
      const again = btn("Again", "btn primary", retry);
      again.id = "bs-retry";
      c.append(again);
    }
    if (state.phase !== "fighting") {
      const leave = btn("Leave", "btn small", leaveRun);
      leave.id = "bs-leave";
      c.append(leave);
    }
    return c;
  }

  function paint() {
    const old = list;   // the list kit's list of the last paint: let go once the new one is drawn
    list = null;
    const nodes = [head()];
    if (!cat) nodes.push(mk("p", "bs-line", "Getting the raids…"));
    else if (!raids().length) nodes.push(mk("p", "bs-line", "This launcher's Boss Rush has no raid list yet: update the launcher."));
    else {
      fix();
      const body = mk("div", "bs-body");
      const left = mk("div", "bs-where");
      left.append(raidTabs(), startList());
      const right = mk("div", "bs-how");
      right.append(options());
      body.append(left, right);
      nodes.push(body, goBar());
      const st = stateCard();
      if (st) nodes.push(st);
    }
    const s = mk("p", "status bs-status" + (line[1] ? " " + line[1] : ""), line[0]);
    s.id = "bs-status";
    s.setAttribute("role", "status");
    nodes.push(s);
    root.replaceChildren(...nodes);
    if (old) old.destroy();
    // the picked start in view inside its list (not the page)
    const on = root.querySelector(".bs-list .bs-row.on");
    const box = on && on.closest(".lk-body");
    if (box && box.scrollHeight > box.clientHeight) box.scrollTop = Math.max(0, on.offsetTop - box.offsetTop - box.clientHeight / 3);
  }

  // ---------------------------------------------------------------- doing things
  async function readCatalog() {
    try { cat = await call("bossrush_catalog"); } catch (_) { cat = null; }
  }
  async function readState() {
    if (!(exact().running)) { state = null; return; }
    try { state = await call("bossrush_state"); } catch (_) { state = null; }
  }
  function watch() {
    clearInterval(timer);
    timer = setInterval(async () => {
      if (!isOn()) { clearInterval(timer); timer = null; return; }
      const before = JSON.stringify(state);
      await readState();
      if (JSON.stringify(state) !== before && !root.contains(document.activeElement)) paint();
      else if (JSON.stringify(state) !== before) { const old = byId("bs-state"); const n = stateCard(); if (old && n) old.replaceWith(n); else if (old) old.remove(); else if (n) byId("bs-status").before(n); }
    }, 4000);
  }

  async function start() {
    const f = folder();
    if (!f || busy || !cat) return;
    busy = true;
    paint();
    say("Getting your raid ready…", "");
    try {
      const r = await call("bossrush_start_at", { client: f.id, start: pick.start, diff: pick.diff, size: pick.size, party: pick.party,
        companions: pick.party === "companions" ? true : !!pick.fill, class: pick.cls, spec: pick.spec });
      const login = !r.auto && r.account ? ` Log in with ${r.account} and the password ${r.password}.` : "";
      say(`${r.hero ? r.hero + (r.made ? " is made" : " is ready") + ". " : ""}${r.message || "Starting your game…"}${login}`, "ok");
      watch();
    } catch (e) { say(oops(e), "bad"); }
    busy = false;
    paint();
  }
  async function act(cmd, args, okText) {
    if (busy) return;
    busy = true;
    try {
      const r = await call(cmd, args || {});
      if (r && r.kind === "no") say(r.why ? r.why[0].toUpperCase() + r.why.slice(1) + "." : "Not now.", "bad");
      else say(okText, "ok");
      await readState();
    } catch (e) { say(oops(e), "bad"); }
    busy = false;
    paint();
  }
  const retry = () => act("bossrush_retry", {}, "Back in front of the boss: pull when you're ready.");
  const goNow = () => act("bossrush_go_now", { companions: !!pick.fill }, "Starting with who is here.");
  const leaveRun = () => act("bossrush_leave", {}, "You left the raid: your hero waits at the start.");
  async function inviteFriend() {
    if (busy) return;
    busy = true;
    try {
      invite = await call("bossrush_invite", { start: pick.start, diff: pick.diff, size: pick.size, companions: !!pick.fill, friendClass: pick.fcls, friendSpec: pick.fspec });
      if (invite && invite.link && navigator.clipboard) navigator.clipboard.writeText(invite.link).catch(() => {});
      say("Invite link copied: send it to your friend.", "ok");
    } catch (e) { say(oops(e), "bad"); }
    busy = false;
    paint();
  }

  async function load() {
    paint();
    await readCatalog();
    await readState();
    paint();
    if (state && state.kind !== "offline") watch();
  }

  document.addEventListener("leyline:page", (e) => { if (e.detail && e.detail.page === "bossrush") load(); });
  window.LeylineBossStart = { load, paint, companionsFor, pick: () => Object.assign({}, pick), state: () => state };
  if (isOn()) load(); else paint();
})();
