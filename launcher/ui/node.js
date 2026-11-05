// Leyline: "Be a node", the card at the top of the Host page (src-tauri/src/p2p/node.rs, docs/BE-A-NODE.md). The owner,
// 5 Nov: "create the ability to allow someone to be a node on the network without playing the game but have it on in
// the client launcher already as a setting, intuitively in the host section".
//
// One big On/Off switch, one line in plain words, the live stats, Pause/Resume, the caps behind "Limits" (upload, peers,
// data per day, only while idle and plugged in, keep running when the window closes, start with Windows), and
// "Advanced: full relay" (the Docker community relay on the Network tab, with "Can other players reach this PC?").
// Settings gets a short part that opens it. Never shows an address: the network check keeps only its kind.
// Not offered by the website: the switch is shown greyed with "Not open yet".
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
  const head = page && page.querySelector(".hc-panel > .panel-head");
  if (!page || !head) return;

  const card = mk("section", "node-card");
  card.id = "node-card";
  card.setAttribute("aria-labelledby", "node-title");
  card.hidden = true;
  head.after(card);

  // TODO(owner): the card's line in your words. Draft: "Your PC helps the Leyline network while you're not playing: it
  // shares Leyline's own files with other players. Never game files."
  const PITCH = "While you're not playing, this PC helps the Leyline network: it shares Leyline's own files with other players. Never game files.";

  const STATE = {
    off: ["off", "Off."],
    not_offered: ["off", "Stopped for now by the Leyline team. It starts again by itself when they turn it back on."],
    stopped: ["off", "Switched off for this PC by whoever runs it."],
    paused: ["rest", "Paused."],
    resting_in_use: ["rest", "Resting while you use this PC."],
    resting_battery: ["rest", "Resting while this PC runs on battery."],
    day_cap: ["rest", "Today's limit is used up: back tomorrow."],
    full: ["rest", "Lots of players at once: resting a minute or two."],
    running: ["on", "Working."],
  };
  const NET = {
    open: "Yes: nothing is in the way. The full relay can work here.",
    endpoint_independent: "Probably, with a port opened on your router. The full relay needs that.",
    symmetric: "No: your router changes ports. The simple node works fine; the full relay won't.",
    unknown: "Could not tell. Try again later.",
    blocked: "No answer: your network blocks the check. The simple node works fine.",
  };

  let view = null, busy = false, timer = null;
  const status = mk("p", "node-status");
  status.setAttribute("role", "status");
  const say = (t, bad) => { status.textContent = t || ""; status.classList.toggle("bad", !!bad); };

  const mb = (bytes) => { const m = (bytes || 0) / 1048576; return m >= 1024 ? `${(m / 1024).toFixed(1)} GB` : m >= 10 ? `${Math.round(m)} MB` : `${m.toFixed(1)} MB`; };
  const dur = (s) => { s = s || 0; const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60); return h >= 48 ? `${Math.floor(h / 24)} days` : h ? `${h} h ${m} min` : `${m} min`; };
  const stat = (label, value, id) => { const d = mk("div", "node-stat"); if (id) d.id = id; d.append(mk("b", "", value), mk("span", "", label)); return d; };

  function stateLine(v) {
    const [kind, words] = STATE[v.state] || STATE.off;
    if (v.state === "running") {
      if (!v.engine) return ["rest", "On, but this version of Leyline can't upload yet."];
      if (!v.has_files) return ["rest", "On. Nothing to share yet: waiting for Leyline's signed file list."];
      const f = v.stats.files === 1 ? "1 file" : `${v.stats.files} files`;
      return [kind, `Working: sharing ${f} of Leyline's own${v.stats.peers_now ? `, ${v.stats.peers_now} connected now` : ""}.`];
    }
    return [kind, words];
  }

  async function save(next, b) {
    if (busy) return;
    busy = true;
    if (b) b.disabled = true;
    try {
      const [line, v] = await call("node_set", { settings: next });
      view = v;
      say(line);
      paint();
    } catch (e) { say(oops(e, "That did not save. Try again."), true); if (b) b.disabled = false; }
    finally { busy = false; }
  }

  function limits(s) {
    const d = mk("details", "node-more");
    d.id = "node-limits";
    d.append(mk("summary", "", "Limits"));
    const num = (id, v, min, max) => { const i = mk("input"); Object.assign(i, { type: "number", id, value: v, min, max }); return i; };
    const field = (label, input, hint) => { const l = mk("label", "node-field"); l.append(mk("span", "", label), input); if (hint) l.append(mk("small", "", hint)); return l; };
    const up = num("node-kbps", s.upload_kbps, 16, 100000);
    const peers = num("node-peers", s.max_peers, 1, 200);
    const day = num("node-day", s.day_mb, 50, 1000000);
    const grid = mk("div", "node-grid");
    grid.append(field("Upload, KB/s", up, "Everything this PC sends, together."), field("Connections at most", peers),
      field("MB per day", day, "Then it rests until tomorrow."));
    const check = (id, on, text, disabled) => {
      const l = mk("label", "node-check");
      const c = mk("input"); Object.assign(c, { type: "checkbox", id, checked: !!on, disabled: !!disabled });
      l.append(c, document.createTextNode(" " + text));
      return [l, c];
    };
    const [idleL, idle] = check("node-idle", s.only_idle, "Only while this PC is idle and plugged in");
    const [keepL, keep] = check("node-keep", s.keep_running, "Keep running when I close the window (Leyline stays in the taskbar)");
    const [bootL, boot] = check("node-boot", s.start_with_windows, view.autostart ? "Start with Windows (Leyline opens minimized)" : "Start with Windows (Windows only)", !view.autostart);
    const row = mk("div", "row");
    row.append(btn("Save limits", "btn small", (ev) => save(Object.assign({}, s, {
      upload_kbps: Number(up.value) || 256, max_peers: Number(peers.value) || 20, day_mb: Number(day.value) || 2048,
      only_idle: idle.checked, keep_running: keep.checked, start_with_windows: boot.checked,
    }), ev.currentTarget)));
    d.append(grid, idleL, keepL, bootL, row);
    return d;
  }

  function advanced() {
    const d = mk("details", "node-more");
    d.id = "node-advanced";
    d.append(mk("summary", "", "Advanced: full relay"));
    d.append(mk("p", "node-fine", "The simple node shares files. Carrying other players' game connections needs more: Docker, a public name for this PC and ports open on your router. That is Help carry the network, on the Network tab."));
    const c = view.carry || {};
    if (c.on) d.append(mk("p", "node-fine", c.running ? `The full relay is on: carrying ${c.players === 1 ? "1 player" : `${c.players} players`} now.` : "The full relay is on but not running: check Docker."));
    const row = mk("div", "row");
    row.append(btn("Open Help carry the network", "btn small ghost", () => {
      const t = byId("hc-tab-network");
      if (t) { t.click(); const box = byId("hc-net-carry"); if (box && box.scrollIntoView) setTimeout(() => box.scrollIntoView({ block: "start" }), 50); }
    }));
    const netLine = mk("p", "node-fine node-net");
    netLine.id = "node-net";
    if (view.stats.network) netLine.textContent = "Can other players reach this PC? " + (NET[view.stats.network] || "Could not tell.");
    row.append(btn("Can other players reach this PC?", "btn small ghost", async (ev) => {
      const b = ev.currentTarget; b.disabled = true;
      netLine.textContent = "Asking two public servers how this PC looks from outside…";
      try { const k = await call("node_check_network"); view.stats.network = k; netLine.textContent = "Can other players reach this PC? " + (NET[k] || "Could not tell."); }
      catch (e) { netLine.textContent = oops(e, "The check did not run."); }
      finally { b.disabled = false; }
    }));
    d.append(row, netLine);
    return d;
  }

  function paint() {
    if (!view) { card.hidden = true; return; }
    card.hidden = false;
    const openMore = [...card.querySelectorAll("details[open]")].map((x) => x.id);
    const s = view.settings;
    const offered = view.offered || s.on;
    card.replaceChildren();
    card.classList.toggle("on", s.on);
    const top = mk("div", "node-top");
    const t = mk("div", "node-t");
    const h = mk("h3", "node-h", "Be a node");
    h.id = "node-title";
    t.append(h, mk("p", "node-pitch", PITCH));
    const sw = mk("button", "node-switch");
    Object.assign(sw, { type: "button", id: "node-switch" });
    sw.setAttribute("role", "switch");
    sw.setAttribute("aria-checked", String(!!s.on));
    sw.setAttribute("aria-labelledby", "node-title");
    sw.append(mk("span", "node-knob"), mk("span", "node-sw-t", s.on ? "On" : "Off"));
    sw.disabled = !offered;
    sw.addEventListener("click", () => save(Object.assign({}, s, { on: !s.on, paused: false }), sw));
    top.append(t, sw);
    card.append(top);
    if (!offered) {
      card.append(mk("p", "node-line muted", "Not open yet: it opens for everyone at once."), status);
      return;
    }
    if (s.on) {
      const [kind, words] = stateLine(view);
      const line = mk("p", "node-line node-" + kind, words);
      line.id = "node-state";
      line.setAttribute("aria-live", "polite");
      card.append(line);
      const st = view.stats;
      const stats = mk("div", "node-stats");
      stats.append(stat("shared today", mb(st.today_bytes), "node-today"), stat("shared in all", mb(st.shared_bytes), "node-total"),
        stat("time as a node", dur(st.uptime_secs), "node-uptime"));
      if (view.carry && view.carry.on) stats.append(stat("players carried now", String(view.carry.players || 0), "node-players"));
      card.append(stats);
      const row = mk("div", "row node-row");
      const p = btn(s.paused ? "Resume" : "Pause", "btn small", (ev) => save(Object.assign({}, s, { paused: !s.paused }), ev.currentTarget));
      p.id = "node-pause";
      row.append(p);
      if (s.keep_running) row.append(btn("Quit Leyline", "btn small ghost", () => call("node_quit").catch(() => {})));
      card.append(row);
    }
    card.append(limits(s), advanced(), status);
    for (const id of openMore) { const x = byId(id); if (x) x.open = true; }
  }

  async function load() {
    try { view = await call("node_state"); } catch (e) { view = null; }
    paint();
  }
  function poll() {
    if (timer) return;
    timer = setInterval(() => {
      const shown = page.classList.contains("on") && !document.hidden;
      if (shown && view && view.settings.on && !busy && !card.querySelector("details[open] input:focus")) load();
    }, 10000);
  }
  document.addEventListener("leyline:page", (e) => { if (e.detail && e.detail.page === "host") { load(); poll(); } });
  if (page.classList.contains("on")) { load(); poll(); }

  // Settings: a short part that opens the card (before Advanced: peer-to-peer, which host.js adds)
  const setPanel = byId("page-settings") && byId("page-settings").querySelector(".panel");
  if (setPanel) {
    const part = mk("section", "set-node");
    part.id = "set-node";
    part.append(mk("h3", "", "Network"),
      mk("p", "lede", "Be a node: let this PC help the Leyline network while you're not playing. The switch is at the top of the Host page."));
    const row = mk("div", "row");
    row.append(btn("Open Be a node", "btn", () => focusCard()));
    part.append(row);
    setPanel.insertBefore(part, byId("p2p-adv") || byId("set-about"));
  }

  function focusCard() {
    if (window.LeylineNav) window.LeylineNav.show("host");
    else { const n = document.querySelector('.nav[data-page="host"]'); if (n) n.click(); }
    setTimeout(() => {
      if (card.scrollIntoView) card.scrollIntoView({ block: "start" });
      const sw = byId("node-switch");
      if (sw) sw.focus({ preventScroll: true });
    }, 60);
  }
  window.LeylineNode = { focus: focusCard, reload: load };
})();
