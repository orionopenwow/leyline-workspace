// Leyline: Leaderboards for Boss Rush and Duels (docs/LEADERBOARDS.md), on the list kit (listkit.js,
// docs/LAUNCHER-LISTS.md). Every row comes from a board the realm that recorded it signed; the core checks every
// signature before a row reaches this page (boards_get, src-tauri/src/boards.rs), and each row says which realm
// signed it. Runs from this PC (Boss Rush, single player) are never on those boards: they show under Local runs, from
// the website's separate local-run board, marked "not checked by a server", and only when their player shared them.
//
// Where it shows: any element with data-board="bossrush" | "duels" (the first tab). Today: the Modes page
// (#md-boards, modes.js) and the Boss Rush page (#br-boards, index.html). LeylineBoards.mount(el) mounts one by hand.
// Nothing is asked of the core until a mount's page is open.
//
//   tabs (the kit's)   Boss Rush | Duels | Local runs
//   the choices        When (This week / All time), Fight, Difficulty (Boss Rush, Local runs), Board (Duels: duels /
//                      1v1 arena), Realm (with more than one)
//   the kit's bar      search by name or realm, the "Only you" chip, the count
//   you                "That's me" on a row keeps that name (on that realm) as yours: your rows are lit and marked You
//
// Rank: Boss Rush by time, each character's best; Duels by rating (all time), by points (duels this week) or by wins
// (1v1 arena this week). The website ranks the same way (stream/boards.py rank). Everything is text (textContent).
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
  const store = {
    get: (k, d) => { try { const v = JSON.parse(localStorage.getItem(k) || "null"); return v === null ? d : v; } catch (_) { return d; } },
    set: (k, v) => { try { localStorage.setItem(k, JSON.stringify(v)); } catch (_) { /* private window */ } },
  };

  // Boss Rush's catalog, our own titles (host/bossrush.rs FIGHTS; the boss's own name never shows)
  const FIGHTS = [
    ["frozen-throne", "The Frozen Throne"], ["bone-lord", "The Bone Lord"], ["cult-mistress", "The Cult Mistress"],
    ["twilight-destroyer", "The Twilight Destroyer"], ["obsidian-sanctum", "The Obsidian Sanctum"],
    ["obsidian-three", "The Obsidian Sanctum, three drakes"], ["brood-mother", "The Brood Mother's Lair"],
    ["stone-watcher", "The Stone Watcher"], ["storm-watcher", "The Storm Watcher"], ["flame-watcher", "The Flame Watcher"],
    ["frost-watcher", "The Frost Watcher"], ["spell-weaver", "The Spellweaver's Eye"],
  ];
  const TITLE = Object.fromEntries(FIGHTS);
  const CLASS = { warrior: "Warrior", paladin: "Paladin", hunter: "Hunter", rogue: "Rogue", priest: "Priest", deathknight: "Death Knight",
    shaman: "Shaman", mage: "Mage", warlock: "Warlock", druid: "Druid", unknown: "" };
  const TINT = { warrior: "#c79c6e", paladin: "#f58cba", hunter: "#abd473", rogue: "#fff569", priest: "#e8e8e8", deathknight: "#ff4d6a",
    shaman: "#2f8fff", mage: "#69ccf0", warlock: "#a29bff", druid: "#ff7d0a" };
  const ME_KEY = "leyline.boards.me";
  const NAME_KEY = "leyline.boards.name";
  const SEL_KEY = "leyline.boards.sel";
  const STALE_MS = 5 * 60 * 1000;

  let data = null;          // boards_get
  let loadedAt = 0;
  let loading = null;
  const views = new Set();

  const clock = (ms) => { const t = Math.floor(ms / 100); return `${Math.floor(t / 600)}:${String(Math.floor(t / 10) % 60).padStart(2, "0")}.${t % 10}`; };
  const ymd = (secs) => new Date(secs * 1000).toISOString().slice(0, 10);
  const meList = () => { const m = store.get(ME_KEY, []); return Array.isArray(m) ? m.filter((x) => typeof x === "string") : []; };
  const meKey = (r) => `${String(r.name).toLowerCase()}@${r.realm ? r.realm.id : "local"}`;
  const isMe = (r) => meList().includes(meKey(r));

  // ---------------------------------------------------------------- ranking (stream/boards.py rank)
  function rank(d, sel) {
    const week = d.week || 0;
    const wkey = "week:" + ymd(week);
    const rows = Array.isArray(d.rows) ? d.rows : [];
    const inRealm = (r) => !sel.realm || (r.realm && r.realm.id === sel.realm);
    // Boss Rush: each character's best (per realm) on this fight and difficulty
    const best = new Map();
    for (const r of rows) {
      if (r.mode !== "bossrush" || r.board !== sel.fight || r.diff !== sel.diff || !inRealm(r)) continue;
      if (sel.period === "week" && r.at < week) continue;
      const k = r.realm.id + "|" + r.name.toLowerCase();
      const b = best.get(k);
      if (!b || r.ms < b.ms || (r.ms === b.ms && r.at < b.at)) best.set(k, r);
    }
    const br = [...best.values()].sort((a, b) => a.ms - b.ms || a.at - b.at || a.name.localeCompare(b.name));
    // Duels
    const period = sel.period === "all" ? "all" : wkey;
    const du = rows.filter((r) => r.mode === "duels" && r.board === sel.duel && r.period === period && inRealm(r));
    du.sort(sel.period === "all"
      ? (a, b) => b.rating - a.rating || b.wins - a.wins || a.name.localeCompare(b.name)
      : sel.duel === "duel"
        ? (a, b) => b.points - a.points || b.wins - a.wins || a.name.localeCompare(b.name)
        : (a, b) => b.wins - a.wins || a.losses - b.losses || a.name.localeCompare(b.name));
    // Local runs: each name's best per engine
    const lb = new Map();
    for (const r of Array.isArray(d.local) ? d.local : []) {
      if (r.board !== sel.fight || r.diff !== sel.diff) continue;
      if (sel.period === "week" && r.at < week) continue;
      const k = r.name.toLowerCase() + "|" + r.engine;
      const b = lb.get(k);
      if (!b || r.ms < b.ms) lb.set(k, r);
    }
    const lo = [...lb.values()].sort((a, b) => a.ms - b.ms || a.at - b.at);
    const tag = (list, tab) => list.map((r, i) => Object.assign({}, r, { tab, rank: i + 1,
      key: `${tab}|${r.realm ? r.realm.id : "local"}|${r.name.toLowerCase()}|${r.engine || r.kind || ""}` }));
    return [...tag(br, "bossrush"), ...tag(du, "duels"), ...tag(lo, "local")];
  }

  // ---------------------------------------------------------------- loading
  async function load(force) {
    if (loading) return loading;
    if (!force && data && Date.now() - loadedAt < STALE_MS) return data;
    for (const v of views) v.busy(true);
    loading = (async () => {
      try {
        data = await call("boards_get");
        loadedAt = Date.now();
      } catch (e) {
        if (!data) data = { rows: [], local: [], realms: [], error: oops(e, "The boards didn't load. Try again in a moment.") };
      } finally {
        loading = null;
      }
      for (const v of views) v.paint();
      document.dispatchEvent(new CustomEvent("leyline:boards", { detail: data }));
      return data;
    })();
    return loading;
  }

  // ---------------------------------------------------------------- one view
  function mount(el, o) {
    if (!el || el.dataset.lbMounted) return null;
    if (!window.LeylineList) return null;
    el.dataset.lbMounted = "1";
    el.classList.add("lb");
    const first = (o && o.board) || el.dataset.board || "bossrush";
    const id = el.id || ("lb-" + Math.random().toString(36).slice(2, 8));
    if (!el.id) el.id = id;
    const saved = store.get(SEL_KEY, {});
    const sel = {
      period: saved.period === "all" ? "all" : "week",
      fight: TITLE[saved.fight] ? saved.fight : "",
      diff: saved.diff === "heroic" ? "heroic" : "normal",
      duel: saved.duel === "arena-1v1" ? "arena-1v1" : "duel",
      realm: "",
    };

    const head = mk("div", "lb-head");
    const h = mk("h3", "", "Leaderboards");
    const feat = mk("p", "lb-feat");
    feat.id = id + "-feat";
    const refresh = mk("button", "btn small", "Refresh");
    refresh.type = "button";
    refresh.id = id + "-refresh";
    refresh.addEventListener("click", () => load(true));
    head.append(h, feat, refresh);
    const tabsEl = mk("div", "lb-tabs");
    const controls = mk("div", "lb-controls");
    const bar = mk("div", "lb-bar");
    const listEl = mk("div", "lb-list");
    listEl.id = id + "-list";
    const note = mk("p", "lb-note");
    note.id = id + "-note";
    el.replaceChildren(head, tabsEl, controls, bar, listEl, note);

    function select(name, label, options, value, on) {
      const l = mk("label", "lb-sel");
      l.append(mk("span", "", label));
      const s = mk("select", "");
      s.name = name;
      s.id = id + "-" + name;
      for (const [v, t] of options) {
        const op = mk("option", "", t);
        op.value = v;
        if (v === value) op.selected = true;
        s.append(op);
      }
      s.addEventListener("change", () => { on(s.value); keep(); update(); paintControls(); });
      l.append(s);
      return l;
    }
    function keep() { store.set(SEL_KEY, { period: sel.period, fight: sel.fight, diff: sel.diff, duel: sel.duel }); }

    function paintControls() {
      const tab = list.tab();
      const nodes = [select("period", "When", [["week", "This week"], ["all", "All time"]], sel.period, (v) => { sel.period = v; })];
      if (tab !== "duels") {
        nodes.push(select("fight", "Fight", FIGHTS, sel.fight, (v) => { sel.fight = v; }));
        nodes.push(select("diff", "Difficulty", [["normal", "Normal"], ["heroic", "Heroic"]], sel.diff, (v) => { sel.diff = v; }));
      } else {
        nodes.push(select("duel", "Board", [["duel", "Duels"], ["arena-1v1", "1v1 arena"]], sel.duel, (v) => { sel.duel = v; }));
      }
      const realms = data && Array.isArray(data.realms) ? data.realms : [];
      if (tab !== "local" && realms.length > 1) {
        nodes.push(select("realm", "Realm", [["", "Every realm"], ...realms.map((r) => [r.id, r.name])], sel.realm, (v) => { sel.realm = v; }));
      }
      controls.replaceChildren(...nodes);
    }

    function mark(r) {
      const m = mk("span", "lb-mark" + (r.tab === "local" ? " local" : ""));
      const ok = mk("span", "lb-ok", r.tab === "local" ? "?" : "✓");
      ok.setAttribute("aria-hidden", "true");
      if (r.tab === "local") {
        m.append(ok, mk("span", "", "Local run"), mk("small", "", "not checked by a server"));
        m.title = "Played on the player's own PC and shared by them. No server saw it.";
      } else {
        m.append(ok, mk("span", "lb-realm", r.realm.name), mk("small", "", r.realm.kind === "official" ? "Official" : "Community"));
        m.title = `Signed by ${r.realm.name} (${r.realm.id}); the signature was checked on this PC`;
      }
      return m;
    }

    let listRef = null;   // the list, once made (its empty words can be asked for while it is being made)
    const list = window.LeylineList.create(listEl, {
      id: "boards-" + id,
      noun: ["result", "results"],
      key: (r) => r.key,
      bar,
      role: "grid",
      label: "Leaderboard",
      tabs: { el: tabsEl, idPrefix: id + "-tab-", value: first, remember: false, label: "Boards",
        items: [
          { id: "bossrush", label: "Boss Rush", long: "Boss Rush", filter: (r) => r.tab === "bossrush" },
          { id: "duels", label: "Duels", long: "Duels", filter: (r) => r.tab === "duels" },
          { id: "local", label: "Local runs", long: "Local runs", filter: (r) => r.tab === "local" },
        ],
        onChange: () => { paintControls(); paintNote(); } },
      search: { fields: (r) => [r.name, r.realm ? r.realm.name : "Local run", CLASS[r.class] || ""], placeholder: "Find a name or a realm" },
      filters: [{ id: "me", label: "Only you", test: (r) => isMe(r) }],
      empty: (info) => {
        if (info.reason === "loading") return { text: "Getting the boards…" };
        if (info.reason === "nomatch" || info.reason === "tab" && info.suggest) return null;
        if (data && data.error) return { text: data.error, actions: [{ label: "Try again", run: () => load(true), primary: true }] };
        const tab = listRef ? listRef.tab() : first;
        if (tab === "local") return { text: "No local runs on this board yet. Share a run from Boss Rush's best times." };
        return { text: sel.period === "week" ? "No results this week yet. Be the first." : "No results on this board yet." };
      },
      render: (r, ctx) => {
        const row = mk("div", "lb-row" + (r.rank <= 3 ? " p" + r.rank : "") + (isMe(r) ? " lb-me" : ""));
        row.dataset.tab = r.tab;
        const rk = mk("span", "lb-rank", String(r.rank));
        const who = mk("span", "lb-who");
        const dot = mk("span", "lb-dot");
        dot.style.background = TINT[r.class] || "var(--t-muted)";
        dot.setAttribute("aria-hidden", "true");
        const name = mk("b", "lb-name");
        name.append(ctx.hl(r.name));
        who.append(dot, name);
        if (CLASS[r.class]) who.append(mk("small", "lb-class", CLASS[r.class]));
        if (isMe(r)) who.append(mk("span", "lb-you", "You"));
        let big, sub;
        if (r.tab === "duels") {
          big = sel.period === "all" ? String(r.rating) : String(sel.duel === "duel" ? r.points : r.wins);
          sub = `${r.wins}-${r.losses}`;
        } else if (r.tab === "bossrush") {
          big = clock(r.ms);
          const bits = [r.bots ? `${r.size} · ${r.bots} companions` : `${r.size} players`];
          if (typeof r.deaths === "number") bits.push(r.deaths === 1 ? "1 death" : `${r.deaths} deaths`);
          sub = bits.join(" · ");
        } else {
          big = clock(r.ms);
          sub = r.engine === "exact" ? "Exact" : "Light version";
        }
        const val = mk("span", "lb-val", big);
        const subEl = mk("span", "lb-sub", sub);
        const date = mk("span", "lb-date", ymd(r.at));
        const me = mk("button", "lb-mebtn", isMe(r) ? "Not me" : "That's me");
        me.type = "button";
        me.setAttribute("aria-pressed", String(isMe(r)));
        me.title = isMe(r) ? "Stop marking this name as you" : `Mark ${r.name} as you: your rows are lit on every board`;
        me.addEventListener("click", () => {
          const m = new Set(meList());
          if (m.has(meKey(r))) m.delete(meKey(r)); else m.add(meKey(r));
          store.set(ME_KEY, [...m].slice(0, 40));
          for (const v of views) v.refresh();
        });
        row.append(rk, who, val, subEl, mark(r), date, me);
        return row;
      },
    });

    listRef = list;

    function paintNote() {
      const tab = list.tab();
      const bits = [];
      if (tab === "local") bits.push("Local runs were played on players' own PCs and shared by them. No server saw them, so they're kept apart from the signed boards.");
      else bits.push("Every row is signed by the realm that recorded it; this PC checked each signature. Weeks start Monday 00:00 UTC. Bragging rights only.");
      if (data && data.from_cache) bits.push("Offline: the last boards Leyline saw.");
      if (data && data.skipped) bits.push(`${data.skipped} board${data.skipped === 1 ? "" : "s"} didn't check and ${data.skipped === 1 ? "isn't" : "aren't"} shown.`);
      note.textContent = bits.join(" ");
    }

    function update() {
      if (!data) { list.setLoading(true, "Getting the boards…"); return; }
      if (!sel.fight) sel.fight = TITLE[data.featured] ? data.featured : "frozen-throne";
      list.update(rank(data, sel));
      list.setLoading(false);
    }

    const view = {
      el, list, sel,
      busy: (on) => { if (!data) list.setLoading(on, "Getting the boards…"); },
      paint: () => {
        const f = data && TITLE[data.featured];
        feat.textContent = f ? `This week's featured fight: ${f}` : "";
        paintControls();
        update();
        paintNote();
      },
      refresh: () => { list.refresh(); },
      rank: () => rank(data || {}, sel),
    };
    views.add(view);
    paintControls();
    update();
    paintNote();
    // the page around it: ask the core when it opens (and again when it is older than five minutes)
    const pageEl = el.closest(".page");
    const visible = () => !pageEl || pageEl.classList.contains("on");
    if (pageEl) {
      new MutationObserver(() => { if (visible()) load(false); }).observe(pageEl, { attributes: true, attributeFilter: ["class"] });
    }
    if (visible()) load(false);
    else if (data) view.paint();
    return view;
  }

  // ---------------------------------------------------------------- sharing a run from this PC
  // The Boss Rush page's best times: Share puts one on the website's local-run board (never a realm's signed board).
  async function share(run, name) {
    const n = String(name || "").trim();
    if (!/^[A-Za-z]{2,12}$/.test(n)) throw "Pick a name of 2 to 12 letters for the board.";
    store.set(NAME_KEY, n);
    try {
      const msg = await call("boards_share_local", { fight: run.fight, diff: run.difficulty || run.diff, engine: run.engine, name: n,
        class: run.class === "-" ? "" : (run.class || ""), spec: run.spec === "-" ? "" : (run.spec || ""), ms: run.ms, at: run.at });
      load(true);
      return msg;
    } catch (e) {
      if (String(e) === "signed_out") throw "Log in to share a run (the Log in button at the top).";
      throw oops(e, "That run couldn't be shared. Try again in a moment.");
    }
  }

  function scan() { document.querySelectorAll("#md-boards, #br-boards, [data-board][data-lb-mount]").forEach((el) => mount(el)); } // not the Modes cards' own .md-board stat boxes
  window.LeylineBoards = { mount, load, share, scan, rank, clock, name: () => store.get(NAME_KEY, ""), data: () => data, views: () => [...views] };
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", scan); else scan();
})();
