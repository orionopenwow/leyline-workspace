// Leyline: Play with friends (owner 5 Nov: "highly develop the boarding to solo and joint private realms user should
// be able to fully host on their own machine and invite a friend easy 1 step and guide and troubleshoot").
//
// One window, opened from every Invite a friend (Home, Single player, the GM command center: solo.js inviteButton) and
// the Host page's banner; Your Den's Share card shows the same checklist under its own Invite a friend:
//   1. Your world: Your Den (Den Lite, built in: no Docker, starts in seconds; the default for friends) or a full
//      realm (the Host pack in Docker: the whole game; needs Docker Desktop and a one-time setup). Play solo opens it
//      for just you.
//   2. Invite a friend: ONE button. Your Den: app.rs den_invite (gets Den Lite on first use, starts it, puts it on the
//      Leyline relay privately, makes one friend's login) ; a full realm: its invite link (host_invite).
//   3. Ready for friends?: before the link is shown, app.rs host_ready (host/ready.rs) checks the path a friend's game
//      takes, line by line, green or red, each red line with one plain fix (a button where there is one): the world
//      answering on its login port, the relay tunnel, the relay seeing it online, a test login through the relay,
//      the connection's speed, the PC's sleep timer, the firewall. Copy diagnostics: the same as text, no secrets.
//   4. The link: copied once everything is green; Share…; the leyline:// form for a friend who has Leyline open.
//   5. A short guide and a troubleshooting table (docs/leyline/host-for-friends.md has every step).
// The friend's side (explainJoin, afterJoin): solo.js offerJoin and Realms' Join show what's happening (reaching the
// host, the host answering, logging in) and the one fix when it's stuck.
// Loaded after den.js and solo.js; everything it uses from them is looked up when a button is pressed.
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
  const btn = (label, cls, on, id) => { const b = mk("button", cls || "btn small", label); b.type = "button"; if (id) b.id = id; if (on) b.addEventListener("click", on); return b; };
  const nav = (id) => { if (window.LeylineNav) window.LeylineNav.show(id); };

  // ---------------------------------------------------------------- never a secret in what is copied for a helper
  function redact(t) {
    return String(t || "")
      .replace(/(\/join\/[a-z2-7]{16})#\S*/gi, "$1#…")
      .replace(/~~\S*/g, "~~…")
      .replace(/(password)\s*[:=]?\s*\S+/gi, "$1 …")
      .replace(/(leyline:\/\/join\?link=)\S+/gi, "$1…");
  }

  // ---------------------------------------------------------------- the friend's side
  // A Join error in plain words: which step stopped (reach: the relay or the host's world; answer: the host's world
  // isn't answering; invite: the link; folder: their game folder) and the one fix.
  // TODO(owner): the words
  const JOIN_FIXES = [
    [/realm offline|unknown realm|isn't known|not online|isn't running/i, "reach", "Your friend's world isn't online right now.",
      "Ask your friend to open Leyline (their world goes online with it) and keep their PC awake. Then press Join again."],
    [/not valid|made a new one|invite link is not|no longer works|expired/i, "invite", "This invite link no longer works.",
      "Links work for 24 hours, and stop when the host presses Stop sharing. Ask your friend for a new link."],
    [/open your invite link on this computer|private realm/i, "invite", "This world is private and this PC isn't let in yet.",
      "Open the invite link on this PC (press Open in Leyline on its page), then press Join."],
    [/not answering|without an answer|timed out: the world|isn't answering/i, "answer", "Your friend's world isn't answering.",
      "Tell your friend: open Leyline and press Invite a friend once, it checks everything and says what to fix. Then try again."],
    [/game folder|Wrath game folder|pick that one/i, "folder", "This world needs your own game folder, the same version as your friend's.",
      "Add it under Game folders, then press Join again."],
    [/full|resting|traffic for today/i, "reach", "The relay or your friend's world is full right now.", "Try again in a few minutes."],
    [/too many|too fast|slow down|wrong passwords/i, "reach", "Too many tries from your address.", "Wait a few minutes, then press Join again."],
    [/relay|dns|resolve|connect|network|offline|internet|timed out/i, "reach", "Your PC couldn't reach the Leyline relay.",
      "Check your internet. A school, work or VPN network can block it: try another network."],
  ];
  function explainJoin(e) {
    const text = typeof e === "string" ? e : (e && e.message) || String(e || "");
    for (const [re, step, line, fix] of JOIN_FIXES) if (re.test(text)) return { step, line, fix, text };
    return { step: "reach", line: "Joining didn't work.", fix: "Press Join again. Still stuck: ask your friend to send a new link.", text };
  }

  // The steps the friend sees, in a list: reach -> answer -> login. `steps(el)` makes it; `.set(id, state, line)`.
  function steps() {
    const ol = mk("ol", "wf-steps");
    const ITEMS = [["reach", "Reaching your friend's world"], ["answer", "Their world answers"], ["login", "Logging in"]];
    for (const [id, label] of ITEMS) {
      const li = mk("li", "wf-step wait");
      li.dataset.step = id;
      li.append(mk("b", "", label), mk("span", "wf-step-line", ""));
      ol.append(li);
    }
    ol.set = (id, st, line) => {
      const li = ol.querySelector(`[data-step="${id}"]`);
      if (!li) return;
      li.className = "wf-step " + st;
      li.querySelector(".wf-step-line").textContent = line || "";
    };
    return ol;
  }
  // After Join worked: does the host's world answer at that address? (app.rs realm_answers: the game's first login
  // message, the same one the host's check sends). The steps list (or a line) says so.
  async function afterJoin(j, list, sayTo) {
    if (list) list.set("reach", "ok", j && j.way === "lan" ? "On your shared network." : "Through the Leyline relay.");
    if (list) list.set("answer", "wait", "Asking…");
    let ms = null, why = "";
    try { ms = await call("realm_answers", { address: j.address }); } catch (e) { why = typeof e === "string" ? e : ""; }
    if (typeof ms === "number") {
      if (list) list.set("answer", "ok", `Yes (${ms} ms).`);
      return true;
    }
    // an older core (no such command) or an address it doesn't test: not asked
    if (!why || /not found|unknown|only tests/i.test(why)) { if (list) list.set("answer", "skip", ""); return null; }
    const x = explainJoin("not answering");
    if (list) { list.set("answer", "bad", x.line + " " + x.fix); list.set("login", "skip", ""); }
    if (sayTo) sayTo(`${x.line} ${x.fix}`, "warn");
    return false;
  }

  // ---------------------------------------------------------------- the checklist (host_ready)
  const ICON = { ok: "✓", warn: "!", bad: "✕", skip: "·", wait: "…" };
  const FIX_LABEL = { start: "Start again", retest: "Check again", power: "Open sleep settings", firewall: "Open firewall settings" };
  // TODO(owner): the words
  const HEAD = { ok: "Ready for friends: everything a friend's game needs is green.", warn: "Ready, with a note or two below.",
    bad: "Not ready yet: fix the red line, then press Check again." };

  function checklist(opts) {
    const o = opts || {};
    const box = mk("div", "wf-ready");
    box.id = o.id || "wf-ready";
    const head = mk("p", "wf-ready-head", "Checking the way your friend comes in…");
    head.setAttribute("role", "status");
    const ul = mk("ul", "wf-checks");
    const acts = mk("div", "wf-row");
    let last = null;
    const recheck = btn("Check again", "btn small", () => run(), box.id + "-again");
    const diag = btn("Copy diagnostics", "btn small", async () => {
      const t = redact(last && last.diagnostics ? last.diagnostics : textOf(last));
      try { await navigator.clipboard.writeText(t); note("Copied: paste it in a message to whoever helps you. It has no invite link and no passwords.", "ok"); }
      catch (_) { note(t, ""); }
    }, box.id + "-diag");
    diag.title = "The checks as text, for a helper (Discord, a friend). No invite link, no logins, no passwords.";
    const line = mk("p", "status wf-ready-note");
    line.setAttribute("role", "status");
    const note = (t, k) => { line.textContent = t || ""; line.className = "status wf-ready-note" + (k ? " " + k : ""); };
    acts.append(recheck, diag);
    box.append(head, ul, acts, line);
    function textOf(r) {
      if (!r) return "";
      return (r.checks || []).map((c) => `[${c.state}] ${c.id}: ${c.line}`).join("\n");
    }
    function paint(r) {
      ul.replaceChildren();
      for (const c of r.checks || []) {
        const li = mk("li", "wf-check " + c.state);
        li.id = (o.prefix || "wf-c-") + c.id;
        const ic = mk("span", "wf-ic", ICON[c.state] || "·");
        ic.setAttribute("aria-hidden", "true");
        const t = mk("div", "wf-check-t");
        t.append(mk("b", "", c.label), mk("span", "wf-check-line", c.line));
        if (c.fix && c.state !== "ok") {
          const f = mk("p", "wf-fix", c.fix);
          if (c.action && FIX_LABEL[c.action]) f.append(" ", btn(FIX_LABEL[c.action], "btn small wf-fix-go", () => fix(c.action), li.id + "-fix"));
          t.append(f);
        }
        li.append(ic, t);
        li.setAttribute("aria-label", `${c.label}: ${c.state === "ok" ? "OK" : c.state === "bad" ? "needs a fix" : c.state === "warn" ? "note" : "not checked"}`);
        ul.append(li);
      }
      const worst = (r.checks || []).some((c) => c.state === "bad") ? "bad" : (r.checks || []).some((c) => c.state === "warn") ? "warn" : "ok";
      head.textContent = HEAD[worst];
      box.dataset.state = worst;
    }
    async function fix(action) {
      if (action === "retest") return run();
      if (action === "start") { if (o.onStart) return o.onStart(); return run(); }
      try { note(await call("host_ready_fix", { what: action }), "ok"); } catch (e) { note(oops(e), "warn"); }
    }
    async function run() {
      recheck.disabled = true;
      box.dataset.state = "wait";
      head.textContent = "Checking the way your friend comes in… (up to half a minute)";
      if (!ul.children.length) {
        for (const [id, label] of [["server", "Your world is running"], ["tunnel", "Connected to the Leyline relay"], ["relay", "The relay sees your world"], ["path", "A friend can reach you"]]) {
          const li = mk("li", "wf-check wait");
          li.id = (o.prefix || "wf-c-") + id;
          li.append(mk("span", "wf-ic", ICON.wait), mk("b", "", label));
          ul.append(li);
        }
      }
      let r = null;
      try { r = await call("host_ready", { kind: o.kind || "lite" }); } catch (e) { r = null; note(oops(e, "The checks didn't run. Try again."), "bad"); }
      recheck.disabled = false;
      if (!r || !Array.isArray(r.checks)) { if (!r) { box.hidden = !!o.quietWithout; } head.textContent = "The checks didn't run."; box.dataset.state = "none"; last = null; if (o.onDone) o.onDone(null); return null; }
      last = r;
      paint(r);
      if (o.onDone) o.onDone(r);
      return r;
    }
    box.run = run;
    box.last = () => last;
    if (o.auto !== false) run();
    return box;
  }

  // ---------------------------------------------------------------- the window
  let root = null, st = null, kitNow = null, kind = "lite", invited = null, busy = false;
  const store = { get: (k) => { try { return localStorage.getItem(k); } catch (_) { return null; } }, set: (k, v) => { try { localStorage.setItem(k, v); } catch (_) { /* fine */ } } };
  const KIND_KEY = "leyline.friendsKind";

  async function look() {
    const [d, k] = await Promise.all([call("den_state").catch(() => null), call("host_info").catch(() => null)]);
    st = d; kitNow = k;
  }
  const liteCan = () => !!(st && st.lite && (st.lite.available || st.lite.can_get || st.lite.running));
  const realmHere = () => !!(kitNow && kitNow.realm && kitNow.realm.profile !== "den");

  function say(t, k) {
    const s = root && root.querySelector("#wf-status");
    if (!s) return;
    s.textContent = t || "";
    s.className = "status wf-status" + (k ? " " + k : "");
  }

  function pickTile(id, title, line, needs, on, can) {
    const b = mk("button", "wf-pick" + (kind === id ? " on" : "") + (can ? "" : " off"));
    b.type = "button";
    b.id = "wf-pick-" + id;
    b.setAttribute("aria-pressed", String(kind === id));
    b.append(mk("b", "", title), mk("span", "wf-pick-line", line), mk("span", "wf-pick-needs", needs));
    b.addEventListener("click", on);
    return b;
  }

  function paint() {
    if (!root) return;
    const body = root.querySelector(".wf-body");
    body.replaceChildren();
    // 1. your world
    const s1 = mk("section", "wf-sec");
    s1.id = "wf-step-world";
    s1.append(mk("h3", "", "1. Your world"));
    const picks = mk("div", "wf-picks");
    // TODO(owner): the words
    picks.append(
      pickTile("lite", "Your Den (recommended)", "Build, explore and hang out together. Nothing to set up: no Docker, it starts in seconds.",
        liteCan() ? "Needs: nothing more. Friends need Leyline, their own game folder and a Hero Key." : "Not made for this kind of computer yet: use a full realm.",
        () => { kind = "lite"; store.set(KIND_KEY, kind); invited = null; paint(); }, liteCan()),
      pickTile("docker", "Full realm", "The whole game together: quests, dungeons, bots, your own rules.",
        realmHere() ? `Set up here: ${kitNow.realm.name || "your realm"}.` : "Needs: Docker Desktop (free), about 4 GB of free memory, and a one-time setup (5 to 20 minutes).",
        () => { if (!realmHere()) { close(); nav("host"); if (window.LeylineHostFlow && window.LeylineHostFlow.show) window.LeylineHostFlow.show(); return; } kind = "docker"; store.set(KIND_KEY, kind); invited = null; paint(); }, true));
    const solo = btn("Play solo", "btn small", () => {
      close();
      if (kind === "docker") { nav("host"); return; }
      if (window.LeylineDen) { nav("den"); window.LeylineDen.go(); } else nav("den");
    }, "wf-solo");
    solo.title = "Just you, on this PC: nothing to set up";
    s1.append(picks, mk("div", "wf-row", ""));
    s1.lastChild.append(solo, mk("span", "wf-hint", "Just you first? Play solo. Friends can join any time after."));
    // 2. invite
    const s2 = mk("section", "wf-sec");
    s2.id = "wf-step-invite";
    s2.append(mk("h3", "", "2. Invite a friend"));
    const go = btn(busy ? "Working…" : invited ? "Invite another friend" : "Invite a friend", "btn primary wf-go", () => invite(), "wf-invite");
    go.disabled = busy || (kind === "lite" && !liteCan());
    s2.append(go, mk("p", "wf-hint", kind === "lite"
      ? "One link for one friend, with their own login inside. Private: only people with your link get in. No router settings, no ports."
      : "Your realm's invite link. Private realms let in only people with the link. No router settings, no ports."));
    const status = mk("p", "status wf-status");
    status.id = "wf-status";
    status.setAttribute("role", "status");
    s2.append(status);
    body.append(s1, s2);
    // 3. ready + the link
    if (invited) {
      const s3 = mk("section", "wf-sec");
      s3.id = "wf-step-ready";
      s3.append(mk("h3", "", "3. Ready for friends?"));
      s3.append(invited.list);
      const lb = mk("div", "wf-linkbox");
      lb.id = "wf-link-box";
      lb.hidden = !invited.done;
      const li = mk("input", "");
      li.id = "wf-link"; li.readOnly = true; li.value = invited.link; li.setAttribute("aria-label", "The invite link");
      const row = mk("div", "wf-row");
      row.append(li, btn("Copy link", "btn small", () => copy(invited.link, "Invite link copied."), "wf-copy"));
      if (navigator.share) row.append(btn("Share…", "btn small", () => navigator.share({ title: "Join my world in Leyline", url: invited.link }).catch(() => { /* closed */ }), "wf-share"));
      lb.append(row);
      // TODO(owner): the words
      lb.append(mk("p", "wf-hint", `Your friend opens it and presses Open in Leyline${kind === "lite" ? "" : " (or plays in their browser)"}.${invited.hours ? ` It works for ${invited.hours} hours (Keep on Your Den's Friends list makes it last).` : ""}`));
      const ll = btn("Copy the Leyline link", "link-s", () => copy("leyline://join?link=" + encodeURIComponent(invited.link), "Leyline link copied: it opens straight in your friend's Leyline."), "wf-copy-ll");
      ll.title = "For a friend who has Leyline open: leyline://join?link=…";
      lb.append(ll);
      s3.append(lb);
      body.append(s3);
    }
    body.append(guide());
  }

  async function copy(t, ok) {
    try { await navigator.clipboard.writeText(t); say(ok, "ok"); } catch (_) { say(t, ""); }
  }

  async function invite() {
    if (busy) return;
    busy = true;
    invited = null;
    paint();
    say(kind === "lite" ? "Starting your Den and putting it on the Leyline network… (a few seconds; the first time it also gets Your Den's server)"
      : "Getting your realm's invite link…");
    let link = null, hours = null;
    try {
      if (kind === "lite") {
        const r = await call("den_invite", { name: null });
        link = r.link; hours = 24;
      } else {
        try { link = await call("host_invite"); } catch (_) { link = null; }
        if (!link) link = await call("host_new_invite");
      }
    } catch (e) {
      busy = false; paint();
      say(oops(e, "Your invite link couldn't be made. Try again."), "bad");
      return null;
    }
    invited = { link, hours, done: false, list: null };
    invited.list = checklist({ kind, onStart: () => invite(), onDone: (r) => done(r) });
    busy = false;
    paint();
    say("Checking the way your friend comes in…");
    return invited;
  }
  async function done(r) {
    if (!invited) return;
    invited.done = true;
    const lb = root && root.querySelector("#wf-link-box");
    if (lb) lb.hidden = false;
    if (!r) { say("The checks didn't run (an older Leyline?). Your link is below: send it, and if your friend gets stuck, the guide below has the fixes.", "warn"); return; }
    if (r.ok) {
      let copied = true;
      try { await navigator.clipboard.writeText(invited.link); } catch (_) { copied = false; }
      say(copied ? "Ready: your invite link is copied. Send it to one friend." : "Ready: send the link below to one friend.", "ok");
      if (window.LeylineGuide && window.LeylineGuide.moment) window.LeylineGuide.moment("hosted");
    } else {
      say("Not ready yet: fix the red line above. Your link is below, but your friend can't get in until it's green.", "warn");
    }
  }

  // the short guide and the troubleshooting table (the long one: docs/leyline/host-for-friends.md)
  // TODO(owner): the words
  const TROUBLE = [
    ["Your friend's game loops between Connecting and Connected", "Your world isn't answering (still starting, stopped, or the PC slept).", "Press Invite a friend (or Check again): fix what's red. Keep Leyline open and the PC awake."],
    ["It's laggy", "Distance or a busy home connection: every move goes your PC → relay → friend.", "Use a wired connection, pause big uploads (backups, streaming), or let the friend with the faster internet host."],
    ["The link says it isn't valid", "Links work for 24 hours, and stop when you press Stop sharing.", "Press Invite a friend again and send the new link (Keep on the Friends list makes a link last)."],
    ["Your friend can't make an account", "Your Den needs none: the link has a login made for them. A full realm makes accounts on its join page.", "Your Den: they press Open in Leyline. A full realm: they make it on the link's page, step 2."],
    ["\"Private realm: open your invite link on this computer\"", "Their PC hasn't opened the link yet (or their internet address changed).", "They open the link again on the PC they play on."],
    ["Your world isn't online", "Leyline is closed on your PC, or it slept or lost its internet.", "Open Leyline. Sharing comes back by itself after a restart."],
  ];
  function guide() {
    const d = mk("details", "wf-guide");
    d.id = "wf-guide";
    d.append(mk("summary", "", "How it works, and what to do when something's wrong"));
    const ul = mk("ul", "wf-how");
    for (const t of [
      "Your world runs on this PC. Your PC keeps one connection out to the Leyline relay, and the relay passes your friend's game through it: nothing to open on your router, and your address is never shown.",
      "Your friend needs Leyline, their own game folder (the same version as yours) and a Hero Key. Game files never travel.",
      "Your world is there while Leyline is open and your PC is awake. For a world that's always up, a full realm on an always-on computer is the way (Host page › More).",
      "Each friend uses some of your upload. If it lags with more friends, a big upload on your connection is the usual cause.",
    ]) ul.append(mk("li", "", t));
    d.append(ul);
    const tb = mk("table", "wf-trouble");
    tb.id = "wf-trouble";
    const hr = mk("tr", "");
    for (const h of ["What you see", "Why", "What to do"]) hr.append(mk("th", "", h));
    tb.append(hr);
    for (const row of TROUBLE) { const tr = mk("tr", ""); for (const c of row) tr.append(mk("td", "", c)); tb.append(tr); }
    d.append(tb);
    const more = btn("The full guide ›", "link-s", () => call("open_link", { name: "hostfriends" }).catch(() => call("open_link", { name: "docs" }).catch(() => { /* no browser */ })), "wf-guide-open");
    d.append(more);
    return d;
  }

  function build() {
    root = mk("div", "wf");
    root.id = "wf";
    root.setAttribute("role", "dialog");
    root.setAttribute("aria-modal", "true");
    root.setAttribute("aria-labelledby", "wf-title");
    const card = mk("div", "wf-card");
    const top = mk("div", "wf-top");
    const h = mk("h2", "", "Play with friends");
    h.id = "wf-title";
    top.append(h, btn("Close", "btn small wf-close", () => close(), "wf-close"));
    // TODO(owner): the words
    card.append(top, mk("p", "wf-lead", "Host on this PC and invite a friend in one step. Leyline checks everything a friend's game needs before you send the link."),
      mk("div", "wf-body"));
    root.append(card);
    root.addEventListener("keydown", (e) => { if (e.key === "Escape") close(); });
    root.addEventListener("click", (e) => { if (e.target === root) close(); });
    document.body.append(root);
  }
  async function open(opts) {
    const o = opts || {};
    if (!root || !root.isConnected) build();
    root.hidden = false;
    await look();
    kind = o.kind || (realmHere() && !liteCan() ? "docker" : store.get(KIND_KEY) === "docker" && realmHere() ? "docker" : "lite");
    if (!liteCan() && realmHere()) kind = "docker";
    paint();
    const go = root.querySelector("#wf-invite");
    if (go) go.focus();
    if (o.go && go && !go.disabled) await invite();
  }
  function close() { if (root) root.hidden = true; }

  // the Host page's banner: Play with friends, no setup (above the Host page's own flow)
  function banner() {
    const page = byId("page-host");
    if (!page || byId("wf-banner")) return;
    const b = mk("div", "wf-banner");
    b.id = "wf-banner";
    const t = mk("div", "wf-banner-t");
    // TODO(owner): the words
    t.append(mk("b", "", "Play with friends, nothing to set up"), mk("span", "wf-hint", "Your Den on this PC, one link for a friend, and a check that it works before you send it."));
    b.append(t, btn("Invite a friend", "btn small primary", () => open({ go: true }), "wf-banner-go"));
    const head = page.querySelector("h1, h2");
    if (head && head.parentNode === page) head.after(b); else page.prepend(b);
  }
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", banner); else banner();

  window.LeylineHostFriends = { open, close, invite: () => invite(), checklist, explainJoin, steps, afterJoin, redact, TROUBLE };
})();
