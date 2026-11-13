// Leyline: Friends on the Leyline network, with no server (owner 12 Nov: "working friends and persistent profiles on
// the Leyline network, even if it's local to local, recognized in the cache and signed by each other").
// docs/LEYLINE-FRIENDS.md; the core is app.rs friends_* (p2p/friends.rs, p2p/friendnet.rs, identity.rs).
//
// window.LeylineFriends.mount(el) draws the whole Friends section into `el` (the Single player page's "Play with a
// friend" area calls it; any element with data-leyline-friends gets it on load too):
//   * your friend code (copy it, good for a day) and a box to paste a friend's code;
//   * one list on the list kit (listkit.js, docs/LAUNCHER-LISTS.md): tabs All / Online / Requests / Near you /
//     Favourites, groups Requests, Online, Offline, Near you, a search, sort by name or last seen, a star per friend,
//     the kit's keys (Enter on a row: Join their invite, else Invite; on a request: Accept; near: Add):
//       - friends: what they play (if they share it), Invite to my world, Join their invite, Remove (asks on the row);
//       - requests to you (Accept / Decline) and yours (Cancel);
//       - people near you (on this network, showing their name) with Add;
//   * your profile: name, status, sharing what you play, how you're seen on this network, the relay mailbox.
// Friends start on load (friends_start): this PC listens on its own network and reads its mailbox.
"use strict";
(() => {
  const call = window.__TAURI__ && window.__TAURI__.core
    ? window.__TAURI__.core.invoke
    : async () => { throw "This is a preview page: open it in the Leyline app."; };
  const oops = (e, fb) => (window.LeylineFriendly ? window.LeylineFriendly(e, fb) : (typeof e === "string" ? e : (fb || "Something went wrong. Try again.")));
  const mk = (tag, cls, text) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  };
  const btn = (label, cls, on) => { const b = mk("button", cls || "btn small", label); b.type = "button"; if (on) b.addEventListener("click", on); return b; };
  const WORLDS = [["single", "Single player"], ["vanilla", "Original world"]];
  const WAYS = { lan: "on your network", direct: "straight to their PC", relay: "through the Leyline relay's mailbox", waiting: "when you're both online", here: "here" };

  function ago(ts) {
    if (!ts) return "not seen yet";
    const s = Math.max(0, Math.floor(Date.now() / 1000) - ts);
    if (s < 120) return "just now";
    if (s < 3600) return `${Math.floor(s / 60)} minutes ago`;
    if (s < 86400 * 2) return `${Math.floor(s / 3600)} hours ago`;
    return `${Math.floor(s / 86400)} days ago`;
  }
  const short = (pid) => String(pid || "").slice(0, 4).toUpperCase();
  const label = (f) => f.name || `Hero ${short(f.peer_id)}`;

  let started = null;
  let mounts = 0;
  function start() {
    if (!started) started = call("friends_start").catch(() => null);
    return started;
  }

  function mount(el) {
    if (!el || el.dataset.friendsMounted) return el && el.__friends;
    el.dataset.friendsMounted = "1";
    el.classList.add("lf");
    el.replaceChildren();
    const head = mk("div", "lf-head");
    head.append(mk("h3", "lf-title", "Your friends list"), mk("p", "lf-lede",
      "You and your friends sign your friendship yourselves, and it lives on your PCs. It works on the same network with no internet, and from anywhere through a relay that can't read your messages."));
    const status = mk("p", "status lf-status");
    status.setAttribute("role", "status");
    const say = (t, k) => { status.textContent = t || ""; status.className = "status lf-status" + (k ? " " + k : ""); };

    // add a friend
    const add = mk("div", "lf-card lf-add");
    add.append(mk("b", "", "Add a friend"));
    const codeRow = mk("div", "lf-row");
    const codeOut = mk("code", "lf-code");
    codeOut.hidden = true;
    const showCode = btn("Show my friend code", "btn small", async () => {
      showCode.disabled = true;
      try {
        const c = await call("friends_code");
        codeOut.textContent = c;
        codeOut.hidden = false;
        let copied = true;
        try { await navigator.clipboard.writeText(c); } catch (_) { copied = false; }
        say(copied ? "Your friend code is copied. Send it to your friend: it works for a day." : "Send this code to your friend: it works for a day.", "ok");
      } catch (e) { say(oops(e), "bad"); }
      showCode.disabled = false;
    });
    showCode.classList.add("lf-show-code");
    codeRow.append(showCode, codeOut);
    const pasteRow = mk("div", "lf-row");
    const paste = mk("input", "lf-paste");
    paste.placeholder = "Paste a friend's code (LF1-…)";
    paste.setAttribute("aria-label", "A friend's code");
    paste.autocomplete = "off";
    paste.spellcheck = false;
    const addBtn = btn("Add", "btn small primary", async () => {
      const code = paste.value.trim();
      if (!code) { say("Paste your friend's code first.", "warn"); return; }
      addBtn.disabled = true;
      try {
        const way = await call("friends_add", { code });
        paste.value = "";
        say(way === "waiting" ? "Request saved: it goes out when you're both online." : `Request sent ${WAYS[way] || ""}. They accept it on their Friends list.`, "ok");
        await refresh();
      } catch (e) { say(oops(e), "bad"); }
      addBtn.disabled = false;
    });
    addBtn.classList.add("lf-add-go");
    pasteRow.append(paste, addBtn);
    add.append(codeRow, pasteRow);

    // Your friends: ONE list on the list kit (listkit.js, docs/LAUNCHER-LISTS.md) that holds friends, requests and
    // people near you. Tabs All / Online / Requests / Near you / Favourites; groups Requests, Online, Offline, Near
    // you; search; sort by name or last seen; a star per friend; the keys of every list (Up / Down, Enter = Join or
    // Invite, "/" to search).
    const listBox = mk("div", "lf-card lf-list");
    const profBox = mk("details", "lf-card lf-profile");
    profBox.append(mk("summary", "", "Your profile"));
    const pf = {};
    const field = (k, lab, input) => { const l = mk("label", "lf-field"); l.append(mk("span", "", lab), input); pf[k] = input; return l; };
    const name = mk("input"); name.maxLength = 32;
    const st = mk("input"); st.maxLength = 80; st.placeholder = "Around tonight";
    const share = mk("input"); share.type = "checkbox";
    const lan = mk("select");
    for (const [v, t] of [["friends", "Friends only"], ["everyone", "Everyone (shows my name)"], ["off", "Nobody"]]) { const o = mk("option", "", t); o.value = v; lan.append(o); }
    const mail = mk("input"); mail.type = "checkbox";
    const save = btn("Save profile", "btn small", async () => {
      save.disabled = true;
      try {
        await call("friends_profile", { name: name.value, status: st.value, sharePlaying: share.checked, lan: lan.value, mailbox: mail.checked });
        say("Profile saved: your friends see it next time you're online.", "ok");
        await refresh();
      } catch (e) { say(oops(e), "bad"); }
      save.disabled = false;
    });
    save.classList.add("lf-save");
    profBox.append(field("name", "Name", name), field("status", "Status", st), field("share", "Tell friends what I'm playing", share),
      field("lan", "Seen on this network by", lan), field("mailbox", "Friends can reach me through the Leyline relay's mailbox", mail), save,
      mk("p", "lf-note", "Your key stays on this PC (peer.key). Lose it and friends need to add you again."));

    el.append(head, status, add, listBox, profBox);

    // ---- the rows: { k: "friend" | "in" | "out" | "near", ...what the core sent }
    const nameOf = (r) => (r.k === "out" ? `Hero ${short(r.peer_id)}` : r.name || `Hero ${short(r.peer_id)}`);
    const keyOf = (r) => (r.k === "friend" ? String(r.peer_id) : r.k + ":" + r.peer_id);   // favourites keep the plain peer id
    const lineOf = (f) => (f.online ? (f.playing ? `Playing ${f.playing}` : (f.near ? "Online, on your network" : "Online")) : `Last seen ${ago(f.last_seen)}`);
    const seenOf = (r) => (r.k !== "friend" ? -1 : r.online ? Number.MAX_SAFE_INTEGER : r.last_seen || 0);
    let ask = "";        // the friend whose Remove is being asked about, on the row itself
    let busy = "";       // the row a call is running for
    let failText = "";   // the core didn't answer the first time: said in the list
    const base = el.id || "lf" + ++mounts;
    const listTitle = mk("b", "", "Your friends");
    const kitEl = mk("div", "lf-kit");
    kitEl.id = base + "-list";
    const countEl = mk("span", "lk-count lf-count");
    countEl.setAttribute("aria-live", "polite");
    listBox.append(listTitle, kitEl);

    async function act(peerId, action, msg) {
      busy = peerId;
      list.refresh();
      try { await call("friends_answer", { peerId, action }); say(msg, "ok"); } catch (e) { say(oops(e), "bad"); }
      busy = "";
      await refresh(true);
    }
    async function join(f) {
      try {
        const link = await call("friends_join", { peerId: f.peer_id });
        if (window.LeylineSolo && window.LeylineSolo.offerJoin) window.LeylineSolo.offerJoin(link);
        else { try { await navigator.clipboard.writeText(link); } catch (_) { /* shown below */ } say("Their invite is copied: paste it under Realms › Got an invite link?", "ok"); }
      } catch (e) { say(oops(e), "bad"); }
    }
    async function invite(f, kind, b) {
      if (b) b.disabled = true;
      say(`Inviting ${label(f)}…`);
      try {
        const way = await call("friends_invite", { peerId: f.peer_id, kind });
        say(`Invite sent to ${label(f)} ${WAYS[way] || ""}. They press Join on your name, and their character stays on your world for next time.`, "ok");
      } catch (e) { say(oops(e), "bad"); }
      if (b) b.disabled = false;
    }
    async function askNear(n, b) {
      if (n.asked) return;
      if (b) b.disabled = true;
      try { await call("friends_ask_near", { peerId: n.peer_id }); say(`Asked ${n.name}.`, "ok"); } catch (e) { say(oops(e), "bad"); if (b) b.disabled = false; }
      await refresh(true);
    }
    const askRemove = (f) => { ask = f.peer_id; list.refresh(); setTimeout(() => { const n = listBox.querySelector(".lf-confirm .lf-remove-no"); if (n) n.focus(); }, 0); };
    const unask = (focusKey) => { ask = ""; list.refresh(); if (focusKey) list.focusRow(focusKey); };

    function friendRow(f, ctx) {
      const row = mk("div", "lf-friend" + (f.online ? " on" : "") + (f.invite ? " has-invite" : ""));
      row.dataset.peer = f.peer_id;
      const dot = mk("span", "lf-dot");
      dot.setAttribute("role", "img");
      dot.setAttribute("aria-label", f.online ? "online" : "offline");
      const who = mk("div", "lf-who");
      const nm = mk("span", "lf-name");
      nm.append(ctx.hl(label(f)));
      who.append(nm);
      const sub = mk("span", "lf-sub");
      const line = lineOf(f);
      if (f.online && f.playing) { const p = mk("span", "lf-playing"); p.append(ctx.hl(line)); sub.append(p); } else sub.append(line);
      if (f.status) { sub.append(" · "); sub.append(ctx.hl(f.status)); }
      who.append(sub);
      const acts = mk("div", "lf-acts");
      const off = busy === f.peer_id;
      if (ask === f.peer_id) {
        // Remove asks first, on the row: the safe choice has focus, Escape keeps them
        const box = mk("div", "lf-confirm");
        box.setAttribute("role", "alertdialog");
        box.setAttribute("aria-label", `Remove ${label(f)}?`);
        box.append(mk("p", "", `Remove ${label(f)} from your friends? They'll need your code to add you again.`));
        const yes = btn("Remove", "btn small danger lf-remove-yes", () => { ask = ""; act(f.peer_id, "remove", `${label(f)} is no longer on your list.`); });
        const no = btn("Keep", "btn small lf-remove-no", () => unask(f.peer_id));
        box.addEventListener("keydown", (e) => { if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); unask(f.peer_id); } });
        const r = mk("div", "lf-acts");
        r.append(yes, no);
        box.append(r);
        row.append(dot, who, box);
        return row;
      }
      if (f.invite) acts.append(btn("Join", "btn small primary lf-join", () => join(f)));
      const pick = mk("select", "lf-world");
      pick.setAttribute("aria-label", `Which world to invite ${label(f)} to`);
      for (const [v, t] of WORLDS) { const o = mk("option", "", t); o.value = v; pick.append(o); }
      pick.value = picked || runningKind;
      pick.addEventListener("change", () => { picked = pick.value; });
      const inv = btn("Invite", "btn small lf-invite", (ev) => invite(f, pick.value, ev.currentTarget));
      inv.title = `Invite ${label(f)} to your world`;
      const rm = btn("Remove", "btn small lf-remove", () => askRemove(f));
      rm.title = `Take ${label(f)} off your friends list (asks first)`;
      for (const b of [inv, rm]) b.disabled = off;
      acts.append(pick, inv, rm, ctx.star());
      row.append(dot, who, acts);
      return row;
    }
    function otherRow(r, ctx) {
      const cls = r.k === "in" ? "lf-row lf-req" : r.k === "out" ? "lf-row lf-out" : "lf-row lf-near-row";
      const row = mk("div", cls);
      row.dataset.peer = r.peer_id;
      const who = mk("div", "lf-who");
      const nm = mk("span", "lf-name");
      nm.append(ctx.hl(nameOf(r)));
      const subText = r.k === "in" ? (r.via === "near" ? "Wants to be friends · on your network" : "Wants to be friends · used your code")
        : r.k === "out" ? "Waiting for them to accept" : (r.asked ? "Asked: waiting for them" : "On your network");
      who.append(nm, mk("span", "lf-sub", subText));
      const acts = mk("div", "lf-acts");
      const off = busy === r.peer_id;
      if (r.k === "in") {
        acts.append(btn("Accept", "btn small primary lf-accept", () => act(r.peer_id, "accept", `You and ${label(r)} are friends.`)),
          btn("Decline", "btn small lf-decline", () => act(r.peer_id, "decline", "Declined.")));
      } else if (r.k === "out") {
        acts.append(btn("Cancel", "btn small lf-cancel", () => act(r.peer_id, "cancel", "Request cancelled.")));
      } else {
        const b = btn(r.asked ? "Asked" : "Add", "btn small lf-near-add", () => askNear(r, b));
        b.disabled = !!r.asked;
        acts.append(b);
      }
      if (off) for (const b of acts.querySelectorAll("button")) b.disabled = true;
      row.append(who, acts);
      return row;
    }

    const list = window.LeylineList.create(kitEl, {
      id: "friends",
      noun: ["friend", "friends"],
      key: keyOf,
      nameOf,
      label: "Your friends",
      render: (r, ctx) => (r.k === "friend" ? friendRow(r, ctx) : otherRow(r, ctx)),
      search: { fields: (r) => [nameOf(r), r.status, r.k === "friend" && r.online ? r.playing : "", r.k === "friend" && r.online ? "online" : ""], placeholder: "Find a friend", label: "Find a friend" },
      tabs: {
        idPrefix: base + "-tab-",
        label: "Show",
        remember: false,
        items: [
          { id: "all", label: "All", suggest: false },
          { id: "on", label: "Online", long: "Online", filter: (r) => r.k === "friend" && !!r.online, count: true,
            empty: "None of your friends is online right now. Invite one anyway: the invite waits for them." },
          { id: "req", label: "Requests", long: "Requests", filter: (r) => r.k === "in" || r.k === "out", count: true,
            empty: "No requests right now. When someone adds your code, it shows here." },
          { id: "near", label: "Near you", long: "Near you", filter: (r) => r.k === "near", count: true,
            empty: "Nobody near you right now. People on your network who show their name appear here." },
          { id: "fav", label: "Favourites", favourites: true, suggest: false,
            empty: "No favourites yet. Press the star by a friend to keep them here." },
        ],
      },
      group: { of: (r) => (r.k === "in" || r.k === "out" ? "Requests" : r.k === "near" ? "Near you" : r.online ? "Online" : "Offline"),
        order: ["Requests", "Online", "Offline", "Near you"] },
      sort: { none: "Online first", label: "Sort", options: [
        { id: "name", label: "Name", compare: (a, b) => window.LeylineList.norm(nameOf(a)).localeCompare(window.LeylineList.norm(nameOf(b))) },
        { id: "seen", label: "Last seen", desc: true, compare: (a, b) => seenOf(a) - seenOf(b) },
      ] },
      favourites: { label: "Favourites" },
      count: false,
      classes: { empty: "lf-empty" },
      primary: (r, rowEl) => {
        if (r.k === "friend") {
          if (ask === r.peer_id) return;
          if (r.invite) join(r);
          else invite(r, (rowEl && rowEl.querySelector(".lf-world") || {}).value || picked || runningKind, rowEl && rowEl.querySelector(".lf-invite"));
        } else if (r.k === "in") act(r.peer_id, "accept", `You and ${label(r)} are friends.`);
        else if (r.k === "near") askNear(r, rowEl && rowEl.querySelector(".lf-near-add"));
      },
      empty: (info) => {
        if (failText && (info.reason === "none" || info.reason === "loading")) return { text: failText };
        if (info.reason === "none") {
          // TODO(owner): the empty-list line in your own words
          return { text: "No friends yet — share your code, or add someone near you.",
            actions: [{ label: "Show my friend code", primary: true, run: () => { showCode.click(); showCode.focus(); } }] };
        }
        return null;
      },
      onChange: (view, l) => {
        // Requests and Near you show only when they hold someone (or are open)
        const all = l.rows();
        for (const [t, test] of [["req", (r) => r.k === "in" || r.k === "out"], ["near", (r) => r.k === "near"]]) {
          const b = kitEl.querySelector(`[data-lk-tab="${t}"]`);
          if (b) b.hidden = view.tab !== t && !all.some(test);
        }
        const nFriends = all.filter((r) => r.k === "friend").length;
        const nOn = all.filter((r) => r.k === "friend" && r.online).length;
        listTitle.textContent = nFriends ? `Your friends (${nOn} online)` : "Your friends";
        countEl.textContent = view.query || view.filtersOn ? `${view.shown} of ${view.inTab} shown` : nFriends ? `${nFriends} ${nFriends === 1 ? "friend" : "friends"}` : "";
        kitEl.classList.toggle("lf-none", !all.length);
      },
    });
    if (list.bar) list.bar.append(countEl);
    list.setLoading(true, "Reading your friends…");

    let profileLoaded = false;
    let loadedOnce = false;
    let lastSig = "", lastAt = 0;
    // Invite's world: the one running on this PC (else Single player), unless the player picked another
    let runningKind = "single";
    let picked = "";
    async function whichRuns() {
      const W = window.LeylineWorlds;
      if (!W || !W.status) return;
      for (const [k] of WORLDS) {
        try { const st = await W.status(k); if (st && st.running) { runningKind = k; return; } } catch (_) { /* not this one */ }
      }
    }
    async function refresh(force) {
      let s;
      try { s = await call("friends_state"); } catch (e) {
        if (loadedOnce) say(oops(e), "bad");
        else {
          const old = /not found|unknown (command )?/i.test(String(e || ""));
          failText = old ? "Your friends list needs a newer Leyline: update Leyline, then come back." : oops(e, "Your friends list didn't load. It tries again in a few seconds.");
          list.setLoading(false);
        }
        return;
      }
      if (!s || typeof s !== "object") return;
      s = { me: s.me || null, friends: s.friends || [], incoming: s.incoming || [], outgoing: s.outgoing || [], near: s.near || [], net: s.net || null };
      loadedOnce = true;
      failText = "";
      if (s.friends.length) await whichRuns();
      const rows = [
        ...s.friends.map((f) => Object.assign({}, f, { k: "friend" })),
        ...s.incoming.map((r) => Object.assign({}, r, { k: "in" })),
        ...s.outgoing.map((r) => Object.assign({}, r, { k: "out" })),
        ...s.near.map((n) => Object.assign({}, n, { k: "near" })),
      ];
      // repaint when something changed (or once a minute, for "last seen"); never under an open world menu
      const sig = JSON.stringify([s.friends, s.incoming, s.outgoing, s.near, runningKind]);
      const menuOpen = document.activeElement && document.activeElement.classList && document.activeElement.classList.contains("lf-world") && listBox.contains(document.activeElement);
      if (force === true || sig !== lastSig || Date.now() - lastAt > 60000 || !lastAt) {
        if (!menuOpen || force === true) {
          lastSig = sig; lastAt = Date.now();
          list.setLoading(false);
          list.update(rows);
        }
      }
      if (!profileLoaded && s.me && s.me.profile) {
        const p = s.me.profile;
        name.value = p.name || ""; st.value = p.status || ""; share.checked = !!p.share_playing; lan.value = p.lan || "friends"; mail.checked = !!p.mailbox;
        profileLoaded = true;
      }
      const n = (s.net && s.net.notices) || [];
      if (n.length && n[0] !== el.__lastNotice) { el.__lastNotice = n[0]; if (!status.textContent) say(n[0], "ok"); }
      if (s.net && s.net.last_error && !status.textContent) say(s.net.last_error, "warn");
      return s;
    }

    const api = { refresh: () => refresh(true), el, list };
    el.__friends = api;
    start().then(refresh);
    const timer = setInterval(() => { if (!document.body.contains(el)) { clearInterval(timer); return; } if (!document.hidden) refresh(); }, 5000);
    return api;
  }

  function auto() { document.querySelectorAll("[data-leyline-friends]").forEach((el) => mount(el)); }
  if (window.__TAURI__) start();
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", auto); else auto();
  window.LeylineFriends = { mount, start };
})();
