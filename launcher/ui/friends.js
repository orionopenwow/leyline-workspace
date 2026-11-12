// Leyline: Friends on the Leyline network, with no server (owner 12 Nov: "working friends and persistent profiles on
// the Leyline network, even if it's local to local, recognized in the cache and signed by each other").
// docs/LEYLINE-FRIENDS.md; the core is app.rs friends_* (p2p/friends.rs, p2p/friendnet.rs, identity.rs).
//
// window.LeylineFriends.mount(el) draws the whole Friends section into `el` (the Single player page's "Play with a
// friend" area calls it; any element with data-leyline-friends gets it on load too):
//   * your friend code (copy it, good for a day) and a box to paste a friend's code;
//   * people near you (on this network, showing their name) with Add;
//   * requests to you (Accept / Decline) and yours (Cancel);
//   * friends, online first: what they play (if they share it), Invite to my world, Join their invite, Remove;
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

    const nearBox = mk("div", "lf-card lf-near");
    const reqBox = mk("div", "lf-card lf-requests");
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

    el.append(head, status, add, reqBox, nearBox, listBox, profBox);
    // until the first answer: nothing empty on screen
    reqBox.hidden = true;
    nearBox.hidden = true;
    listBox.append(mk("b", "", "Your friends"), mk("p", "lf-empty", "Reading your friends…"));

    async function act(peerId, action, msg) {
      try { await call("friends_answer", { peerId, action }); say(msg, "ok"); } catch (e) { say(oops(e), "bad"); }
      await refresh();
    }

    function drawRequests(s) {
      reqBox.replaceChildren();
      reqBox.hidden = !(s.incoming.length || s.outgoing.length);
      if (reqBox.hidden) return;
      reqBox.append(mk("b", "", "Requests"));
      for (const r of s.incoming) {
        const row = mk("div", "lf-row lf-req");
        row.dataset.peer = r.peer_id;
        row.append(mk("span", "lf-name", label(r)), mk("span", "lf-sub", r.via === "near" ? "on your network" : "used your code"),
          btn("Accept", "btn small primary lf-accept", () => act(r.peer_id, "accept", `You and ${label(r)} are friends.`)),
          btn("Decline", "btn small lf-decline", () => act(r.peer_id, "decline", "Declined.")));
        reqBox.append(row);
      }
      for (const r of s.outgoing) {
        const row = mk("div", "lf-row lf-out");
        row.dataset.peer = r.peer_id;
        row.append(mk("span", "lf-name", `Hero ${short(r.peer_id)}`), mk("span", "lf-sub", "waiting for them to accept"),
          btn("Cancel", "btn small lf-cancel", () => act(r.peer_id, "cancel", "Request cancelled.")));
        reqBox.append(row);
      }
    }

    function drawNear(s) {
      nearBox.replaceChildren();
      nearBox.hidden = !s.near.length;
      if (nearBox.hidden) return;
      nearBox.append(mk("b", "", "People near you"));
      for (const n of s.near) {
        const row = mk("div", "lf-row lf-near-row");
        row.dataset.peer = n.peer_id;
        const b = btn(n.asked ? "Asked" : "Add", "btn small lf-near-add", async () => {
          b.disabled = true;
          try { await call("friends_ask_near", { peerId: n.peer_id }); say(`Asked ${n.name}.`, "ok"); } catch (e) { say(oops(e), "bad"); b.disabled = false; }
          await refresh();
        });
        b.disabled = !!n.asked;
        row.append(mk("span", "lf-name", n.name), b);
        nearBox.append(row);
      }
    }

    function drawFriends(s) {
      listBox.replaceChildren();
      listBox.append(mk("b", "", s.friends.length ? `Your friends (${s.friends.filter((f) => f.online).length} online)` : "Your friends"));
      if (!s.friends.length) { listBox.append(mk("p", "lf-empty", "No friends yet: share your code, or add someone near you.")); return; }
      for (const f of s.friends) {
        const row = mk("div", "lf-friend" + (f.online ? " on" : ""));
        row.dataset.peer = f.peer_id;
        const dot = mk("span", "lf-dot");
        dot.setAttribute("aria-label", f.online ? "online" : "offline");
        const who = mk("div", "lf-who");
        who.append(mk("span", "lf-name", label(f)));
        const line = f.online ? (f.playing ? `Playing ${f.playing}` : (f.near ? "Online, on your network" : "Online")) : `Last seen ${ago(f.last_seen)}`;
        who.append(mk("span", "lf-sub", f.status ? `${line} · ${f.status}` : line));
        const acts = mk("div", "lf-acts");
        if (f.invite) {
          acts.append(btn("Join", "btn small primary lf-join", async () => {
            try {
              const link = await call("friends_join", { peerId: f.peer_id });
              if (window.LeylineSolo && window.LeylineSolo.offerJoin) window.LeylineSolo.offerJoin(link);
              else { try { await navigator.clipboard.writeText(link); } catch (_) { /* shown below */ } say("Their invite is copied: paste it under Realms › Got an invite link?", "ok"); }
            } catch (e) { say(oops(e), "bad"); }
          }));
        }
        const pick = mk("select", "lf-world");
        pick.setAttribute("aria-label", "Which world");
        for (const [v, t] of WORLDS) { const o = mk("option", "", t); o.value = v; o.selected = v === runningKind; pick.append(o); }
        pick.addEventListener("change", () => { picked = pick.value; });
        if (picked) pick.value = picked;
        acts.append(pick, btn("Invite", "btn small lf-invite", async (ev) => {
          const b = ev.currentTarget;
          b.disabled = true;
          say(`Inviting ${label(f)}…`);
          try {
            const way = await call("friends_invite", { peerId: f.peer_id, kind: pick.value });
            say(`Invite sent to ${label(f)} ${WAYS[way] || ""}. They press Join on your name, and their character stays on your world for next time.`, "ok");
          } catch (e) { say(oops(e), "bad"); }
          b.disabled = false;
        }), btn("Remove", "btn small lf-remove", async () => {
          if (!window.confirm(`Remove ${label(f)} from your friends? They'll need your code to add you again.`)) return;
          await act(f.peer_id, "remove", `${label(f)} is no longer on your list.`);
        }));
        row.append(dot, who, acts);
        listBox.append(row);
      }
    }

    let profileLoaded = false;
    let loadedOnce = false;
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
    async function refresh() {
      let s;
      try { s = await call("friends_state"); } catch (e) {
        if (loadedOnce) say(oops(e), "bad");
        else {
          const old = /not found|unknown (command )?/i.test(String(e || ""));
          listBox.replaceChildren(mk("b", "", "Your friends"), mk("p", "lf-empty", old ? "Your friends list needs a newer Leyline: update Leyline, then come back." : oops(e, "Your friends list didn't load. It tries again in a few seconds.")));
        }
        return;
      }
      if (!s || typeof s !== "object") return;
      s = { me: s.me || null, friends: s.friends || [], incoming: s.incoming || [], outgoing: s.outgoing || [], near: s.near || [], net: s.net || null };
      loadedOnce = true;
      if (s.friends.length) await whichRuns();
      drawRequests(s);
      drawNear(s);
      drawFriends(s);
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

    const api = { refresh, el };
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
