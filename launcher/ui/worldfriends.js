// Leyline: friends who keep a character on a single player world, both sides (docs/SINGLE-PLAYER-ONE-CLICK.md,
// "Playing with friends"; the core is host/worldfriends.rs through app.rs world_friends, world_friend, friend_worlds,
// join_friend_again, friend_world_forget). On the Single player page's one Friends area (index.html #spf):
//   * #wf, the host's side: who keeps a login and characters on your world (Single player or the original world),
//     each with their characters, whether they can come back without a new invite, Trust / Untrust, Remove (their
//     characters stay; invite them again and they get them back) and Remove and delete characters (asks first, in
//     plain words, on the row itself);
//   * #fw, the friend's side: the friends' worlds this PC joined, with Join again (no new link: this PC's own key
//     proves it's you) and Forget.
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

  function paintHost() {
    const root = byId("wf");
    if (!root) return;
    root.hidden = H.gone;
    if (H.gone) { root.replaceChildren(); return; }
    const head = mk("div", "wf-head");
    const sel = mk("select", "oc-sel wf-kind");
    sel.id = "wf-kind";
    sel.setAttribute("aria-label", "Which world");
    for (const [k, t] of WORLDS) { const o = mk("option", "", t); o.value = k; o.selected = k === H.kind; sel.append(o); }
    sel.addEventListener("change", () => { H.kind = sel.value; H.list = null; H.ask = ""; H.line = ["", ""]; paintHost(); readHost(); });
    head.append(mk("h3", "oc-set-h", "Friends with a character on your world"), sel);
    const nodes = [head];
    if (!H.list) nodes.push(mk("p", "oc-fine", "Reading your world's friends…"));
    else if (!H.list.length) {
      nodes.push(mk("p", "oc-fine wf-empty", "No friend has a character here yet. A friend who joins with your invite keeps their login and characters on your world for next time."));
    } else {
      const ul = mk("ul", "wf-list");
      ul.id = "wf-list";
      for (const f of H.list) {
        const li = mk("li", "wf-friend");
        li.dataset.user = f.user;
        const top = mk("div", "wf-top");
        const n = mk("span", "wf-name", who(f));
        const [ct, ck] = stateChip(f);
        const chip = mk("span", "sp-dock wf-state " + ck, ct);
        top.append(n, chip);
        if (f.on_list) top.append(mk("span", "wf-sub", f.online ? "On your friends list, online" : "On your friends list"));
        const sub = mk("p", "wf-chars", charsText(f));
        const acts = mk("div", "wf-acts");
        const off = H.busy === f.user;
        const trust = btn(f.trusted ? "Stop trusting" : "Trust", "btn small wf-trust", () => act(f, f.trusted ? "untrust" : "trust"));
        trust.title = f.trusted ? "They'll need a new invite after you stop sharing your world." : "They can come back any time your world is shared, with no new invite.";
        const rm = btn("Remove", "btn small wf-remove", () => { H.ask = "remove:" + f.user; paintHost(); });
        rm.title = "Their login stops working. Their characters stay: invite them again and they get them back.";
        const del = btn("Remove and delete characters…", "btn small wf-delete", () => { H.ask = "delete:" + f.user; paintHost(); });
        for (const b of [trust, rm, del]) b.disabled = off;
        if (!H.running) { rm.disabled = true; del.disabled = true; }
        acts.append(trust, rm, del);
        li.append(top, sub, acts);
        if (H.ask === "remove:" + f.user || H.ask === "delete:" + f.user) {
          const deleting = H.ask.startsWith("delete:");
          const nChars = (f.chars || []).length;
          const box = mk("div", "wf-confirm" + (deleting ? " bad" : ""));
          box.setAttribute("role", "alertdialog");
          box.append(mk("p", "", deleting
            ? `Delete ${who(f)}'s login and ${nChars ? (nChars === 1 ? "their character" : `all ${nChars} of their characters`) : "any characters they have"} on your world for good? This can't be undone.`
            : `Remove ${who(f)} from your world? Their login stops working, and their characters stay here: invite them again and they get them back.`));
          const yes = btn(deleting ? "Delete for good" : "Remove", deleting ? "btn small danger wf-yes" : "btn small primary wf-yes", () => act(f, deleting ? "delete" : "remove"));
          const no = btn(deleting ? "Keep them" : "Cancel", "btn small wf-no", () => { H.ask = ""; paintHost(); });
          const r = mk("div", "wf-acts");
          r.append(yes, no);
          box.append(r);
          li.append(box);
          setTimeout(() => no.focus(), 0);
        }
        ul.append(li);
      }
      nodes.push(ul);
      if (!H.running) nodes.push(mk("p", "oc-fine", `Start ${titleOf(H.kind)} to remove a friend: your world makes the change.`));
    }
    const st = mk("p", "status wf-line" + (H.line[1] ? " " + H.line[1] : ""), H.line[0]);
    st.id = "wf-line";
    st.setAttribute("role", "status");
    nodes.push(st);
    root.replaceChildren(...nodes);
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

  function paintFriend() {
    const root = byId("fw");
    if (!root) return;
    root.hidden = F.gone || !F.list || !F.list.length;
    if (root.hidden) { root.replaceChildren(); return; }
    const nodes = [mk("h3", "oc-set-h", "Your friends' worlds"),
      mk("p", "oc-fine", "Worlds you joined keep your character. Join again needs no new link while your friend shares their world (or trusts you).")];
    const ul = mk("ul", "wf-list fw-list");
    ul.id = "fw-list";
    for (const w of F.list) {
      const li = mk("li", "wf-friend fw-world");
      li.dataset.realm = w.realm_id;
      const top = mk("div", "wf-top");
      top.append(mk("span", "wf-name", w.name || "Your friend's world"), mk("span", "wf-sub", w.at ? `joined ${ago(w.at)}` : ""));
      const acts = mk("div", "wf-acts");
      const go = btn(F.busy === w.realm_id ? "Joining…" : "Join again", "btn small primary fw-join", () => joinAgain(w));
      const fg = btn("Forget", "btn small fw-forget", () => forget(w));
      fg.title = "Take it off this list. Your character stays on your friend's world.";
      go.disabled = fg.disabled = !!F.busy;
      acts.append(go, fg);
      li.append(top, acts);
      ul.append(li);
    }
    nodes.push(ul);
    const st = mk("p", "status wf-line" + (F.line[1] ? " " + F.line[1] : ""), F.line[0]);
    st.id = "fw-line";
    st.setAttribute("role", "status");
    nodes.push(st);
    root.replaceChildren(...nodes);
  }

  function refresh() { return Promise.all([readHost(), readFriend()]); }
  document.addEventListener("leyline:page", (e) => { if (e.detail && e.detail.page === "single") { H.kind = null; refresh(); } });
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", () => { paintHost(); }); else paintHost();

  window.LeylineWorldFriends = { refresh, host: () => Object.assign({}, H), friend: () => Object.assign({}, F) };
})();
