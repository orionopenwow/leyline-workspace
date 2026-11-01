// Leyline Launcher: paste a realm to add it, and the Live now numbers (owner 31 Oct, "share links, paste to add, live now").
//
// What can be pasted (the Realms page's "Paste a realm link or address" box, or the play bar's realm box, whose drop and
// paste handler in app.js, barImport, asks here first):
//   https://play.openvanilla.app/wrath/r/<short>   a realm's share link (stream/links.py; /r/<short> too). short = the
//                                                  first 8 letters of its realm id, or the whole id, or "official"
//   leyline://realm/<id>                           the website's Open in Leyline (deeplink.rs takes the same)
//   https://<relay>/join/<id>                      a realm's join link (an invite's #secret is ignored here)
//   host:port                                      a realm address (a relay address or the host's own)
// Recognised: the realm goes to Your realms (the play bar's pick, saved like a click there), with the client it needs
// (app.js pickRealm: leyline.clientFor), and a Play now button starts it the way PLAY does. Never starts by itself.
// Anything else: one plain line saying what can be pasted. A folder, a zip or a download link in the play bar still goes
// on to the game-folder import (take() returns false for those).
//
// Live now (Home): the busiest player realms right now, by players online then uptime; New this week: realms first seen
// in the last 7 days (the listing's is_new) with a player online in the last day, which this PC notes itself
// (localStorage leyline.lastPlayers: realm id -> when it last saw players there). Everything is set with textContent.
(function () {
  "use strict";
  const App = window.LeylineApp;
  if (!App) return;
  const $ = (id) => document.getElementById(id);
  const el = (tag, cls, text) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  };

  const UNKNOWN = "That isn't a realm link. Paste a realm's share link, a leyline:// link, a join link or an address like realm.example.org:3724.";
  const ID = /^[a-z2-7]{16}$/;
  const SHORT = /^[a-z2-7]{6,16}$/;
  const HOST = /^(?=.{1,253}$)(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z0-9-]{1,63}$|^(?:\d{1,3}\.){3}\d{1,3}$/i;

  // { kind: "official" } | { kind: "id", id, via } | { kind: "short", short } | { kind: "address", address } | null
  function parse(text) {
    let t = String(text || "").trim();
    if (!t || t.length > 300 || /\s/.test(t)) return null;
    let m = /^leyline:\/\/realm\/([a-z0-9]+)\/?$/i.exec(t);
    if (m) {
      const id = m[1].toLowerCase();
      return id === "official" ? { kind: "official" } : ID.test(id) ? { kind: "id", id, via: "app" } : null;
    }
    let url = null;
    try { url = new URL(/^[a-z]+:\/\//i.test(t) ? t : "https://" + t); } catch (_) { url = null; }
    if (url && /^https?:$/.test(url.protocol) && !url.username && !url.password) {
      const host = url.hostname.toLowerCase();
      const path = url.pathname.replace(/\/+$/, "");
      m = /^\/(?:wrath\/)?r\/([a-z0-9-]{1,32})$/.exec(path);
      if (m && /(^|\.)openvanilla\.app$/.test(host)) {
        const s = m[1];
        if (["official", "wrath", "wrathunleashed"].includes(s)) return { kind: "official" };
        return ID.test(s) ? { kind: "id", id: s, via: "link" } : SHORT.test(s) ? { kind: "short", short: s } : null;
      }
      m = /^\/join\/([a-z2-7]{16})$/.exec(path);
      if (m && HOST.test(host)) return { kind: "id", id: m[1], via: "join" };
    }
    // host:port, nothing else around it
    m = /^([a-z0-9.-]+):(\d{1,5})$/i.exec(t);
    if (m && HOST.test(m[1]) && +m[2] >= 1 && +m[2] <= 65535) return { kind: "address", address: m[1].toLowerCase() + ":" + (+m[2]) };
    return null;
  }
  // a game folder, a zip, a download link, a magnet or a torrent: the play bar's import takes those
  const importable = (t) => /[\\/]/.test(t) || /^magnet:/i.test(t) || /\.(zip|torrent)$/i.test(t) || /^[a-z]:/i.test(t);

  const realms = () => App.realms() || [];
  function resolve(p) {
    const list = realms();
    if (p.kind === "official") return list.find((r) => r.official) || { official: true, name: App.HANGOUT.name };
    if (p.kind === "id") return list.find((r) => r.realm_id === p.id) || null;
    if (p.kind === "short") {
      const hits = list.filter((r) => r.realm_id && r.realm_id.startsWith(p.short));
      return hits.length === 1 ? hits[0] : null;
    }
    return list.find((r) => !r.official && r.address === p.address) || { official: false, address: p.address, name: p.address, typed: true };
  }

  // where the answer goes: the play bar's box ("bar") or the Realms page box ("page")
  let pending = null;   // the realm Play now starts
  function tell(where, kind, head, line, realm) {
    pending = realm || null;
    if (where === "bar") {
      const B = window.LeylineBarImport;
      if (!B || !B.view) return;
      B.view(kind === "bad" ? "error" : "done", head, line);
      const retry = $("pbd-retry");
      if (retry && kind === "bad") retry.hidden = true;   // nothing to try again: the text was the problem
      if ($("pbd-play")) $("pbd-play").hidden = !realm;
      return;
    }
    const st = $("rl-paste-status");
    if (st) { st.textContent = line ? `${head} ${line}` : head; st.className = "status " + (kind || ""); }
    if ($("rl-paste-play")) $("rl-paste-play").hidden = !realm;
  }

  // the text in, the realm added (or the plain line); true when this took the text
  function take(text, where) {
    const t = String(text || "").trim();
    const p = parse(t);
    if (!p) {
      if (where === "bar" && importable(t)) return false;
      tell(where, "bad", UNKNOWN, "");
      return true;
    }
    add(p, where);
    return true;
  }
  async function add(p, where) {
    let r = resolve(p);
    if (!r && !realms().filter((x) => !x.official).length) { try { await App.loadRealms(true); } catch (_) { /* below */ } r = resolve(p); }
    if (!r) {
      tell(where, "warn", "That realm isn't on the realm list right now.", "It may be offline or private. Try again in a minute.");
      return;
    }
    if (r.official || r.address) App.pickRealm(r);   // Your realms, with the client it needs (leyline.clientFor)
    const name = r.official ? App.HANGOUT.name : r.name;
    const need = r.official ? "3.3.5a" : r.build;
    const st = App.state();
    const have = !need || (st && st.settings.clients.some((c) => c.build === need));
    tell(where, have ? "ok" : "warn", `Added ${name} to Your realms.`, have ? "Press Play now to start." : "It needs the game files for this realm: add that game folder first.", r);
  }
  async function playNow(btn) {
    const r = pending;
    if (!r) return;
    if (window.LeylineBarImport && window.LeylineBarImport.view) window.LeylineBarImport.view("idle");
    if (r.official || r.address) {
      App.pickRealm(r);
      const f = $("play-form");
      if (f) (f.requestSubmit ? f.requestSubmit() : f.dispatchEvent(new Event("submit", { cancelable: true })));
    } else {
      App.playRealm(r, btn);   // listed, no address yet: the Realms page's PLAY asks its relay
    }
  }

  // ---- the boxes
  const form = $("rl-paste");
  if (form) {
    form.addEventListener("submit", (e) => { e.preventDefault(); take($("rl-paste-in").value, "page"); });
    $("rl-paste-in").addEventListener("paste", (e) => {
      const t = ((e.clipboardData && e.clipboardData.getData("text")) || "").trim();
      if (!t) return;
      e.preventDefault();
      $("rl-paste-in").value = t;
      take(t, "page");
    });
    $("rl-paste-play").addEventListener("click", (e) => playNow(e.currentTarget));
  }
  if ($("pbd-play")) $("pbd-play").addEventListener("click", (e) => { e.stopPropagation(); playNow(e.currentTarget); });

  // ---- Live now and New this week (home.js draws them)
  const SEEN_KEY = "leyline.lastPlayers";
  const DAY = 86400000;
  function seenMap() {
    try { const v = JSON.parse(localStorage.getItem(SEEN_KEY) || "{}"); return v && typeof v === "object" ? v : {}; } catch (_) { return {}; }
  }
  function notePlayers(now) {
    now = now || Date.now();
    const v = seenMap();
    for (const r of realms()) if (r.realm_id && r.state === "live" && Number(r.players) > 0) v[r.realm_id] = now;
    for (const k of Object.keys(v)) if (!(now - v[k] <= 8 * DAY)) delete v[k];
    try { localStorage.setItem(SEEN_KEY, JSON.stringify(v)); } catch (_) { /* fine: only players now count then */ }
    return v;
  }
  const playerRealm = (r) => !r.official && !r.sample && r.group !== "official" && !App.hiddenBuild(r.build);
  const up = (r) => (r.uptime_7d != null ? r.uptime_7d : r.uptime_24h != null ? r.uptime_24h : -1);
  function liveNow(n) {
    return realms().filter((r) => playerRealm(r) && r.state === "live")
      .sort((a, b) => (Number(b.players) || 0) - (Number(a.players) || 0) || up(b) - up(a) || String(a.name).localeCompare(String(b.name)))
      .slice(0, n || 4);
  }
  function newThisWeek(n, now) {
    now = now || Date.now();
    const seen = notePlayers(now);
    return realms().filter((r) => playerRealm(r) && r.is_new
      && ((r.state === "live" && Number(r.players) > 0) || (r.realm_id && now - (seen[r.realm_id] || 0) <= DAY))).slice(0, n || 4);
  }
  // the mode badge: the realm's ruleset in words (stream/wrath_portal.py RULESET_LABELS)
  const MODES = { main: "Level 1 or boost to 80", brawl: "Brawl: everyone 80", progression: "Progression", casual: "Casual",
    dalaran: "Dalaran hangout", vanillaplus: "Vanilla+", custom: "Custom rules" };
  const modeLabel = (r) => MODES[r && r.ruleset] || MODES.custom;
  // Play from a strip: select it and press PLAY (the play bar's own flow)
  function play(r, btn) {
    pending = r;
    playNow(btn);
  }

  window.LeylineRealmLink = { parse, take, liveNow, newThisWeek, modeLabel, play, UNKNOWN };
})();
