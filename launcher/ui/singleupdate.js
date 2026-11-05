// Leyline: the bots, always up to date (owner 4 Nov, Single player Phase 4; docs/BOTS-UPDATES.md). A small part of the
// Single player page's Manage card (single.js manageCard appends LeylineSingleUpdate.node()):
//   * "New bots are ready" with Update the bots, when the core says this PC's realm runs older bots than the promoted
//     ones (bots_status, host/bots.rs). Pressing it: a newer Host pack first (its realm scripts go with the bots,
//     hostpack_install), then the realm images (bots_update). Never automatic.
//   * then the choice: Restart now (Stop, then Start: everyone is saved) or Later (the new bots join at the next start).
//     Never restarted by itself.
//   * "Bots updated" with the date, once, after the bots this realm runs changed (dismissed with Got it). A first
//     set-up shows none: it only marks what it has.
// The pins come from the owner merging the weekly canary's pull request (bots-canary.yml), never from here.
// Loaded after single.js.
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
  const SEEN_KEY = "leyline.bots.seen";       // the playerbots pin last acknowledged on this PC
  const AGAIN_MS = 10 * 60 * 1000;            // looked at again at most this often (or when asked)
  const store = { get: (k) => { try { return localStorage.getItem(k); } catch (_) { return null; } },
    set: (k, v) => { try { localStorage.setItem(k, v); } catch (_) { /* private window */ } } };

  let st = null;          // bots_status
  let at = 0;             // when it was read
  let reading = null;
  let busy = false;
  let after = false;      // the new bots are downloaded: Restart now / Later
  let say = () => {};

  const repaint = () => { if (window.LeylineSingle && window.LeylineSingle.paint) window.LeylineSingle.paint(); };
  // "10 Nov" from 2026-11-10 (the date as a chip, never in a sentence with a version)
  const day = (d) => {
    const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(d || "");
    if (!m) return "";
    const names = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    return `${Number(m[3])} ${names[Number(m[2]) - 1] || ""}`.trim();
  };

  async function refresh(force) {
    if (reading) return reading;
    if (!force && at && Date.now() - at < AGAIN_MS) return st;
    reading = (async () => {
      const was = JSON.stringify(st);
      at = Date.now();   // before the answer: a page painted meanwhile doesn't ask again
      try { st = await call("bots_status"); } catch (_) { st = null; }   // no kit, no Docker: nothing shown
      // a first look marks what runs here: "Bots updated" is only for a change after that
      const pb = st && st.engine && st.engine.playerbots;
      if (pb && !store.get(SEEN_KEY)) store.set(SEEN_KEY, pb);
      reading = null;
      if (JSON.stringify(st) !== was) repaint();
      return st;
    })();
    return reading;
  }

  async function update(b) {
    if (busy) return;
    busy = true;
    b.disabled = true;
    try {
      if (st && st.pack_first) {
        say("Getting the new Host pack…");
        const r = await call("hostpack_install");
        await call("set_host_kit", { path: r[1] });
      }
      say("Getting the new bots… This can take a few minutes.");
      say(await call("bots_update"), "ok");
      after = true;
    } catch (e) { say(oops(e, "The bots didn't update. Try again."), "bad"); }
    busy = false;
    await refresh(true);
    repaint();
  }

  async function restartNow(b) {
    b.disabled = true;
    after = false;
    say("Restarting Single player… Everyone is saved.");
    try {
      await call("host_stop");
      await call("host_start_quiet", { answers: { name: null, visibility: "private", gm: null } });
      if (window.LeylineSingle && window.LeylineSingle.load) await window.LeylineSingle.load();   // follows the start
    } catch (e) { say(oops(e, "Single player didn't restart. Press Start."), "bad"); repaint(); }
  }

  function node(sayFn) {
    if (sayFn) say = sayFn;
    const box = mk("div", "sp-bots");
    box.id = "sp-bots";
    if (!reading && (!at || Date.now() - at >= AGAIN_MS)) refresh(false);
    if (!st) return box;
    if (after) {
      const p = mk("p", "sp-line", "The new bots are ready. Restart Single player now to bring them in? Everyone is saved.");
      const row = mk("div", "sp-row");
      const now = btn("Restart now", "btn small primary", (ev) => restartNow(ev.currentTarget)); now.id = "sp-bots-restart";
      const later = btn("Later", "btn small", () => { after = false; say("The new bots join at the next start.", "ok"); repaint(); }); later.id = "sp-bots-later";
      row.append(now, later);
      box.append(p, row);
      return box;
    }
    if (st.update) {
      const row = mk("div", "sp-row");
      const chip = mk("span", "sp-dock ok", "New bots");
      const d = day(st.latest && st.latest.date);
      const line = mk("span", "sp-line", "Newer bots are ready: tested, then switched on by Leyline.");
      const go = btn("Update the bots", "btn small primary", (ev) => update(ev.currentTarget)); go.id = "sp-bots-update";
      go.disabled = busy;
      go.title = "Downloads the new bots. Restarting is up to you.";
      row.append(chip, line);
      if (d) row.append(mk("span", "rl-ver sm", d));
      row.append(go);
      box.append(row);
      return box;
    }
    const pb = st.engine && st.engine.playerbots;
    const seen = store.get(SEEN_KEY);
    if (pb && seen && seen !== pb) {
      const row = mk("div", "sp-row");
      row.id = "sp-bots-news";
      row.append(mk("span", "sp-dock ok", "Bots updated"));
      const d = day(st.engine.date);
      if (d) row.append(mk("span", "rl-ver sm", d));
      const n = st.engine.changes;
      row.append(mk("span", "sp-line", n ? `${n} changes to how your bots play, tested before they reached you.` : "Your bots got better, tested before they reached you."));
      const ok = btn("Got it", "btn small", () => { store.set(SEEN_KEY, pb); repaint(); }); ok.id = "sp-bots-seen";
      row.append(ok);
      box.append(row);
      return box;
    }
    if (st.note) box.append(mk("p", "sp-line", st.note));
    return box;
  }

  window.LeylineSingleUpdate = { node, refresh, day };
})();
