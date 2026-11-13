// Leyline: friends who keep a character on a single player world, both sides (docs/SINGLE-PLAYER-ONE-CLICK.md,
// "Playing with friends"; the core is host/worldfriends.rs through app.rs world_friends, world_friend, friend_worlds,
// join_friend_again, friend_world_forget). On the Single player page's one Friends area (index.html #spf):
//   * #wf, the host's side: who keeps a login and characters on your world (Single player or the original world),
//     each with their characters, whether they can come back without a new invite, Trust / Untrust, Remove (their
//     characters stay; invite them again and they get them back) and Remove and delete characters (asks first, in
//     plain words, on the row itself);
//   * #fw, the friend's side: the friends' worlds this PC joined, with Join again (no new link: this PC's own key
//     proves it's you) and Forget.
// Both lists are on the list kit (listkit.js, docs/LAUNCHER-LISTS.md): a search, #wf's status filter (Invited / Can
// come back / Needs a new invite), sort, the kit's keys (Enter on a world: Join again) and its empty / loading states.
// A core without these commands (an older Leyline) hides both. Text only (textContent).
"use strict";
(() => {
  const call = window.__TAURI__ && window.__TAURI__.core
    ? window.__TAURI__.core.invoke
    : async () => { throw "This is a preview page: open it in the Leyline app."; };
  const oops = (e, fb) => (window.LeylineFriendly ? window.LeylineFriendly(e, fb) : (typeof e === "string" ? e : (fb || "Something went wrong. Try again.")));
  const missing = (e) => /not found|unknown (command )?/i.test(String(e || ""));
  const mk = (tag, cls, text) => { const e = document.createElement(tag); if (cls) e.className = cls; if (text !== undefined) e.textContent = text; return e; };
  const btn = (label, cls, on, id) => { const b = mk("button", cls || "btn small", label); b.type = "button"; if (id) b.id = id; if (on) b.addEventListener("click", on); return b; };
  const byId = (id) => document.getElementById(id);
  const WORLDS = [["single", "Single player"], ["vanilla", "Original world"]];
  const titleOf = (k) => (WORLDS.find((w) => w[0] === k) || [k, k])[1];
  const short = (pid) => String(pid || "").slice(0, 4).toUpperCase();

  function ago(ts) {
    if (!ts) return "";
    const s = Math.max(0, Math.floor(Date.now() / 1000) - ts);
    if (s < 120) return "just now";
    if (s < 3600) return `${Math.floor(s / 60)} minutes ago`;
    if (s < 86400 * 2) return `${Math.floor(s / 3600)} hours ago`;
    return `${Math.floor(s / 86400)} days ago`;
  }
  const W = () => window.LeylineWorlds;
  async function running(kind) {
    try { const st = W() ? await W().status(kind) : await call("world_status", { kind }); return !!(st && st.running); } catch (_) { return false; }
  }

  // ---------------------------------------------------------------- the host: friends on your world
  const H = { kind: null, list: null, running: false, line: ["", ""], busy: "", ask: "", gone: false, loading: false };
  const who = (f) => f.name || (f.legacy && !f.peer ? `An earlier friend (${f.user})` : `Hero ${short(f.peer || f.user.slice(1))}`);

  async function readHost() {
    const root = byId("wf");
    if (!root || H.loading) return;
    H.loading = true;
    if (!H.kind) {
      H.kind = "single";
      for (const [k] of WORLDS) if (await running(k)) { H.kind = k; break; }
    }
    H.running = await running(H.kind);
    try {
      const v = await call("world_friends", { kind: H.kind });
      H.list = Array.isArray(v) ? v : [];
      H.gone = false;
    } catch (e) {
      if (missing(e)) H.gone = true;
      else { H.list = H.list || []; H.line = [oops(e, "Your world's friends didn't load. Try again."), "bad"]; }
    }
    H.loading = false;
    paintHost();
  }

  async function act(f, action) {
    const name = who(f);
    H.busy = f.user;
    H.ask = "";
    H.line = [action === "delete" ? `Deleting ${name}'s characters…` : "Saving…", ""];
    paintHost();
    try {
      const msg = await call("world_friend", { kind: H.kind, user: f.user, action });
      H.line = [String(msg || "Done."), "ok"];
    } catch (e) { H.line = [oops(e, "That didn't work. Try again."), "bad"]; }
    H.busy = "";
    await readHost();
  }

  function charsText(f) {
    const c = Array.isArray(f.chars) ? f.chars : [];
    if (c.length) return "Characters: " + c.map((x) => `${x.name} (level ${x.level}${x.online ? ", in the world now" : ""})`).join(", ");
    return H.running ? "No characters yet." : "Their characters show while your world runs.";
  }
  function stateChip(f) {
    if (f.trusted) return ["Can come back any time", "ok"];
    if (f.enabled) return ["Invited", "ok"];
    return ["Needs a new invite", "warn"];
  }

  // The host's list, on the list kit (listkit.js, docs/LAUNCHER-LISTS.md): search (names and characters), a status
  // filter (Invited / Can come back / Needs a new invite), sort by name or last seen, the kit's keys. Built once; each
  // read repaints it in place (the scroll, the focus and an open question on a row stay).
  const STATES = [["invited", "Invited"], ["back", "Can come back"], ["new", "Needs a new invite"]];
  const stateOf = (f) => (f.trusted ? "back" : f.enabled ? "invited" : "new");
  const HK = { list: null, sel: null, note: null, line: null };
  function hostRow(f, ctx) {
    const li = mk("div", "wf-friend");
    li.dataset.user = f.user;
    const top = mk("div", "wf-top");
    const n = mk("span", "wf-name");
    n.append(ctx.hl(who(f)));
    const [ct, ck] = stateChip(f);
    const chip = mk("span", "sp-dock wf-state " + ck, ct);
    top.append(n, chip);
    if (f.on_list) top.append(mk("span", "wf-sub", f.online ? "On your friends list, online" : "On your friends list"));
    else if (f.seen) top.append(mk("span", "wf-sub", `Last here ${ago(f.seen)}`));
    const sub = mk("p", "wf-chars");
    sub.append(ctx.hl(charsText(f)));
    const acts = mk("div", "wf-acts");
    const off = H.busy === f.user;
    const trust = btn(f.trusted ? "Stop trusting" : "Trust", "btn small wf-trust", () => act(f, f.trusted ? "untrust" : "trust"));
    trust.title = f.trusted ? "They'll need a new invite after you stop sharing your world." : "They can come back any time your world is shared, with no new invite.";
    const rm = btn("Remove", "btn small wf-remove", () => { H.ask = "remove:" + f.user; paintHost(); });
    rm.title = "Their login stops working. Their characters stay: invite them again and they get them back.";
    const del = btn("Remove and delete characters…", "btn small wf-delete", () => { H.ask = "delete:" + f.user; paintHost(); });
    for (const x of [trust, rm, del]) x.disabled = off;
    if (!H.running) { rm.disabled = true; del.disabled = true; }
    acts.append(trust, rm, del);
    // a friend who came back with a new Leyline key (a reinstall, a new PC) is a new row: their old row's characters
    // go to it in one step (app.rs world_friend "give:", backed up first)
    const others = (H.list || []).filter((x) => x.user !== f.user);
    if ((f.chars || []).length && others.length) {
      const give = btn("Give their characters to…", "btn small wf-give", () => { H.ask = "give:" + f.user; paintHost(); });
      give.title = "For a friend who came back with a new Leyline key (a reinstall or a new PC): their characters move to that friend's login.";
      give.disabled = off || !H.running;
      acts.append(give);
    }
    li.append(top, sub, acts);
    if (H.ask === "give:" + f.user) {
      const box = mk("div", "wf-confirm");
      box.setAttribute("role", "alertdialog");
      const pick = mk("select", "oc-sel wf-give-to");
      pick.setAttribute("aria-label", "Give them to");
      for (const x of others) { const o = mk("option", "", who(x)); o.value = x.user; pick.append(o); }
      box.append(mk("p", "", `Give ${who(f)}'s characters to another friend's login on your world? Use it when a friend came back with a new Leyline key. A back-up is taken first.`), pick);
      const yes = btn("Give them", "btn small primary wf-yes", () => act(f, "give:" + pick.value));
      const no = btn("Cancel", "btn small wf-no", () => { H.ask = ""; paintHost(); });
      const r = mk("div", "wf-acts");
      r.append(yes, no);
      box.append(r);
      li.append(box);
    }
    if (H.ask === "remove:" + f.user || H.ask === "delete:" + f.user) {
      const deleting = H.ask.startsWith("delete:");
      const nChars = (f.chars || []).length;
      const box = mk("div", "wf-confirm" + (deleting ? " bad" : ""));
      box.setAttribute("role", "alertdialog");
      box.append(mk("p", "", deleting
        ? `Delete ${who(f)}'s login and ${nChars ? (nChars === 1 ? "their character" : `all ${nChars} of their characters`) : "any characters they have"} on your world for good? This can't be undone.`
        : `Remove ${who(f)} from your world? Their login stops working, and their characters stay here: invite them again and they get them back.`));
      const yes = btn(deleting ? "Delete for good" : "Remove", deleting ? "btn small danger wf-yes" : "btn small primary wf-yes", () => act(f, deleting ? "delete" : "remove"));
      const no = btn(deleting ? "Keep them" : "Cancel", "btn small wf-no", () => { H.ask = ""; paintHost(); if (HK.list) HK.list.focusRow(f.user); });
      box.addEventListener("keydown", (e) => { if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); no.click(); } });
      const r = mk("div", "wf-acts");
      r.append(yes, no);
      box.append(r);
      li.append(box);
      setTimeout(() => { if (no.isConnected) no.focus(); }, 0);
    }
    return li;
  }
  function buildHost(root) {
    const head = mk("div", "wf-head");
    const sel = mk("select", "oc-sel wf-kind");
    sel.id = "wf-kind";
    sel.setAttribute("aria-label", "Which world");
    for (const [k, t] of WORLDS) { const o = mk("option", "", t); o.value = k; sel.append(o); }
    sel.addEventListener("change", () => { H.kind = sel.value; H.list = null; H.ask = ""; H.line = ["", ""]; paintHost(); readHost(); });
    head.append(mk("h3", "oc-set-h", "Friends with a character on your world"), sel);
    const box = mk("div", "wf-kit");
    box.id = "wf-list";
    const note = mk("p", "oc-fine wf-note");
    const st = mk("p", "status wf-line");
    st.id = "wf-line";
    st.setAttribute("role", "status");
    root.replaceChildren(head, box, note, st);
    HK.sel = sel; HK.note = note; HK.line = st;
    HK.list = window.LeylineList.create(box, {
      id: "worldfriends",
      noun: ["friend", "friends"],
      key: (f) => f.user,
      nameOf: who,
      label: "Friends with a character on your world",
      render: hostRow,
      search: { fields: (f) => [who(f), f.user, ...(f.chars || []).map((c) => c.name)], placeholder: "Find a friend or character", label: "Find a friend or character" },
      filters: [{ id: "state", label: "Status", options: (rows) => STATES.map(([v, t]) => ({ value: v, label: `${t} · ${rows.filter((f) => stateOf(f) === v).length}` })),
        test: (f, v) => stateOf(f) === v }],
      filtersLabel: "Status",
      sort: { none: "Suggested", options: [
        { id: "name", label: "Name", compare: (a, b) => window.LeylineList.norm(who(a)).localeCompare(window.LeylineList.norm(who(b))) },
        { id: "seen", label: "Last here", desc: true, compare: (a, b) => (a.online ? 1e12 : a.seen || 0) - (b.online ? 1e12 : b.seen || 0) },
      ] },
      classes: { empty: "wf-empty" },
      empty: (info) => (info.reason === "none"
        ? "No friend has a character here yet. A friend who joins with your invite keeps their login and characters on your world for next time."
        : info.reason === "loading" ? "Reading your world's friends…" : null),
      onChange: (view, l) => { box.classList.toggle("wf-none", !l.rows().length); },
    });
  }
  function paintHost() {
    const root = byId("wf");
    if (!root) return;
    root.hidden = H.gone;
    if (H.gone) { root.replaceChildren(); HK.list && HK.list.destroy(); HK.list = null; return; }
    if (!HK.list || !root.contains(HK.list.el)) buildHost(root);
    HK.sel.value = H.kind || "single";
    HK.note.textContent = H.list && H.list.length && !H.running ? `Start ${titleOf(H.kind)} to remove a friend: your world makes the change.` : "";
    HK.line.textContent = H.line[0];
    HK.line.className = "status wf-line" + (H.line[1] ? " " + H.line[1] : "");
    HK.list.setLoading(!H.list);
    HK.list.update(H.list || []);
  }

  // ---------------------------------------------------------------- the friend: friends' worlds you joined
  const F = { list: null, line: ["", ""], busy: "", gone: false };

  async function readFriend() {
    if (!byId("fw")) return;
    try {
      const v = await call("friend_worlds");
      F.list = Array.isArray(v) ? v : [];
      F.gone = false;
    } catch (e) {
      if (missing(e)) F.gone = true; else { F.list = F.list || []; F.line = [oops(e), "bad"]; }
    }
    paintFriend();
  }

  async function joinAgain(w) {
    F.busy = w.realm_id;
    F.line = ["Joining…", ""];
    paintFriend();
    try {
      const s = await call("get_state");
      const clients = (s && s.settings && s.settings.clients) || [];
      const c = clients.find((x) => x.id === s.settings.selected_client && x.build === "3.3.5a") || clients.find((x) => x.build === "3.3.5a");
      if (!c) F.line = ["Your friend's world plays with your own game folder, the same version as theirs: add it under Settings › Game folders, then press Join again.", "warn"];
      else {
        const j = await call("join_friend_again", { client: c.id, realmId: w.realm_id });
        const g = j && j.guest;
        F.line = g && !g.auto
          ? [`Joining your friend's world. At the login screen: account ${g.account}, password ${g.password}.`, "ok"]
          : ["Joining your friend's world: your game signs you in by itself, with your character there.", "ok"];
      }
    } catch (e) {
      F.line = [oops(e, "Your friend's world didn't answer. Ask them to start it and share it (or trust you)."), "bad"];
    }
    F.busy = "";
    paintFriend();
  }

  async function forget(w) {
    F.busy = w.realm_id;
    try { await call("friend_world_forget", { realmId: w.realm_id }); F.line = ["Forgotten on this PC. Your character stays on your friend's world.", "ok"]; }
    catch (e) { F.line = [oops(e), "bad"]; }
    F.busy = "";
    await readFriend();
  }

  // Your friends' worlds, on the list kit: search, sort by name or when you joined, Enter = Join again.
  const FK = { list: null, line: null };
  function worldRow(w, ctx) {
    const li = mk("div", "wf-friend fw-world");
    li.dataset.realm = w.realm_id;
    const top = mk("div", "wf-top");
    const n = mk("span", "wf-name");
    n.append(ctx.hl(w.name || "Your friend's world"));
    top.append(n, mk("span", "wf-sub", w.at ? `joined ${ago(w.at)}` : ""));
    const acts = mk("div", "wf-acts");
    const go = btn(F.busy === w.realm_id ? "Joining…" : "Join again", "btn small primary fw-join", () => joinAgain(w));
    const fg = btn("Forget", "btn small fw-forget", () => forget(w));
    fg.title = "Take it off this list. Your character stays on your friend's world.";
    go.disabled = fg.disabled = !!F.busy;
    acts.append(go, fg);
    li.append(top, acts);
    return li;
  }
  function buildFriend(root) {
    const box = mk("div", "wf-kit");
    box.id = "fw-list";
    const st = mk("p", "status wf-line");
    st.id = "fw-line";
    st.setAttribute("role", "status");
    root.replaceChildren(mk("h3", "oc-set-h", "Your friends' worlds"),
      mk("p", "oc-fine", "Worlds you joined keep your character. Join again needs no new link while your friend shares their world (or trusts you)."), box, st);
    FK.line = st;
    FK.list = window.LeylineList.create(box, {
      id: "friendworlds",
      noun: ["world", "worlds"],
      key: (w) => w.realm_id,
      nameOf: (w) => w.name || "Your friend's world",
      label: "Your friends' worlds",
      render: worldRow,
      search: { fields: (w) => [w.name || "Your friend's world"], placeholder: "Find a world", label: "Find a friend's world" },
      sort: { none: "Suggested", options: [
        { id: "name", label: "Name", compare: (a, b) => window.LeylineList.norm(a.name || "Your friend's world").localeCompare(window.LeylineList.norm(b.name || "Your friend's world")) },
        { id: "joined", label: "Joined", desc: true, compare: (a, b) => (a.at || 0) - (b.at || 0) },
      ] },
      primary: (w) => { if (!F.busy) joinAgain(w); },
    });
  }
  function paintFriend() {
    const root = byId("fw");
    if (!root) return;
    root.hidden = F.gone || !F.list || !F.list.length;
    if (root.hidden) { root.replaceChildren(); if (FK.list) FK.list.destroy(); FK.list = null; return; }
    if (!FK.list || !root.contains(FK.list.el)) buildFriend(root);
    FK.line.textContent = F.line[0];
    FK.line.className = "status wf-line" + (F.line[1] ? " " + F.line[1] : "");
    FK.list.update(F.list);
  }

  function refresh() { return Promise.all([readHost(), readFriend()]); }
  document.addEventListener("leyline:page", (e) => { if (e.detail && e.detail.page === "single") { H.kind = null; refresh(); } });
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", () => { paintHost(); }); else paintHost();

  window.LeylineWorldFriends = { refresh, host: () => Object.assign({}, H), friend: () => Object.assign({}, F) };
})();
