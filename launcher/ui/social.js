// Leyline Launcher: your profile (the title bar's chip, top right) and the sign-in. Under it in the code: Home's old right
// panel (the Discord and the forum's latest threads), hidden for now (owner 2026-10-30).
//
// The profile is the Leyline sign-in (the Rust core's social commands, src-tauri/src/social.rs, talking to the portal's
// /wrath/api/social: stream/social.py and stream/profiles.py). Signing in happens on the website, in your browser: you
// pick a way (Hero Key, Google or Discord: only the ones the website offers right now), the browser opens that way's
// sign-in with a code, and Leyline continues by itself once the code is approved. The launcher keeps only a revocable
// token. The first time, Manage account (account.js) opens for the profile step (name, picture, bio). Signed in: your
// picture and name; the chip opens your account panel (you, your friends, quick links, Manage account, Sign out). The
// same sign-in rates mods and signs in to community realms with your Hero Key.
//
// The sign-in's state (flow) is shared with Manage account, which shows the same chooser and waiting state; both listen
// for "leyline:signin" and "leyline:me". Loaded before app.js. Everything from the server is shown with textContent or
// as an image address from it: no markup from anyone's name.
(function () {
  "use strict";
  const tauri = window.__TAURI__ && window.__TAURI__.core ? window.__TAURI__.core.invoke : null;
  const invoke = tauri || (async (cmd) => {
    if (cmd === "social_status") return false;
    throw "This is a preview page: open it in the Leyline app.";
  });
  const $ = (id) => document.getElementById(id);
  const el = (tag, cls, text) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  };
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const openLink = (name) => invoke("open_link", { name }).catch((e) => say(String(e), "bad"));
  const emit = (name, detail) => document.dispatchEvent(new CustomEvent(name, { detail }));

  let me = null, extra = {}, signedIn = false, code = "", ways = null;
  // the sign-in in progress: stage idle | choose | waiting | busy | error; via: the way picked; note: what went wrong;
  // form: the in-launcher form open in the chooser ("" | "key": paste your Hero Key | "password": name#tag and password)
  const flow = { stage: "idle", via: "", note: "", form: "" };
  // Get your Hero Key (owner 31 Oct 19:15): a new account's key, made just now, to show once in Manage account
  // ({key, name}); never kept anywhere but this page's memory, and dropped once the player leaves Manage account
  let fresh = null;
  let runId = 0;

  function say(text, kind) {
    const s = $("hf-status");
    if (!s) return;
    s.textContent = text || "";
    s.className = "status hf-status" + (kind ? " " + kind : "");
  }

  // ---------------------------------------------------------------- pictures
  // the picture: a portrait (art/avatars/, avatars.js) on a slate medallion, never a letter. A website address: its preset
  // shows from the local copy; a player's own address (…/avatar/u/<pid>.svg, only the site (https://leylinelauncher.com, or its old name), the one
  // place the launcher loads images from) loads from the site, and while it loads or when it can't, the portrait picked
  // from the name shows
  const PIC_RE = /^https:\/\/(leylinelauncher\.com|play\.openvanilla\.app)\/wrath\/avatar\/[a-z]\/[a-z0-9]{2,12}\.svg(\?v=\d{1,9})?$/;   // either name
  function avatar(name, cls, url) {
    const A = window.LeylineAvatars;
    const av = el("span", "sx-av" + (cls ? " " + cls : ""));
    const img = el("img"); img.alt = ""; img.decoding = "async"; img.draggable = false;
    const local = A ? A.src(A.pick(name)) : "";
    const preset = A && url ? A.fromUrl(url) : "";
    if (preset) img.src = A.src(preset);
    else if (url && PIC_RE.test(url)) {
      img.referrerPolicy = "no-referrer";
      if (local) { img.onerror = () => { img.onerror = null; img.src = local; }; }
      img.src = url;
    } else if (local) img.src = local;
    if (img.src) av.append(img);
    return av;
  }
  const picOf = (m) => (m && m.profile && m.profile.avatar_url) || "";

  // ---------------------------------------------------------------- the ways and the chooser
  const WAYS = [["herokey", "Hero Key", "Sign in with your Hero Key"], ["google", "Google", "Continue with Google"], ["discord", "Discord", "Continue with Discord"]];
  const WAY_ICON = {
    herokey: '<path fill="currentColor" d="M7 14a3 3 0 1 1 0-6 3 3 0 0 1 0 6Zm5.65-4A6 6 0 1 0 12.65 14H15v3h3v-3h2v-4h-7.35Z"/>',
    google: '<path fill="#EA4335" d="M12 4.75c1.77 0 3.36.61 4.6 1.8l3.43-3.42C17.95 1.19 15.24 0 12 0 7.31 0 3.26 2.69 1.28 6.61l3.99 3.1C6.22 6.86 8.87 4.75 12 4.75z"/><path fill="#4285F4" d="M23.49 12.27c0-.79-.07-1.54-.19-2.27H12v4.51h6.47c-.29 1.48-1.13 2.74-2.39 3.59l3.86 3c2.26-2.09 3.55-5.18 3.55-8.83z"/><path fill="#FBBC05" d="M5.26 14.29A7.2 7.2 0 0 1 4.89 12c0-.8.14-1.57.38-2.29l-3.99-3.1A11.95 11.95 0 0 0 0 12c0 1.94.46 3.77 1.28 5.39l3.98-3.1z"/><path fill="#34A853" d="M12 24c3.24 0 5.97-1.06 7.95-2.9l-3.86-3c-1.08.72-2.46 1.15-4.09 1.15-3.13 0-5.78-2.11-6.73-4.96l-3.98 3.1C3.25 21.3 7.3 24 12 24z"/>',
    discord: '<path fill="currentColor" d="M20.32 4.37A19.8 19.8 0 0 0 15.43 2.8a13.7 13.7 0 0 0-.63 1.29 18.4 18.4 0 0 0-5.6 0 13 13 0 0 0-.64-1.29 19.7 19.7 0 0 0-4.89 1.52C.56 9.04-.28 13.6.14 18.1a19.9 19.9 0 0 0 6.02 3.04c.49-.66.92-1.36 1.29-2.1a12.9 12.9 0 0 1-2.03-.97c.17-.13.34-.26.5-.39a14.2 14.2 0 0 0 12.16 0c.17.14.33.27.5.39-.65.38-1.33.71-2.04.97.37.74.8 1.44 1.29 2.1a19.8 19.8 0 0 0 6.03-3.04c.5-5.22-.84-9.74-3.54-13.73ZM8.02 15.33c-1.18 0-2.16-1.08-2.16-2.42 0-1.33.96-2.42 2.16-2.42 1.21 0 2.18 1.1 2.16 2.42 0 1.34-.96 2.42-2.16 2.42Zm7.97 0c-1.19 0-2.16-1.08-2.16-2.42 0-1.33.95-2.42 2.16-2.42 1.2 0 2.18 1.1 2.16 2.42 0 1.34-.95 2.42-2.16 2.42Z"/>',
  };
  function svg(inner, cls) {
    const s = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    s.setAttribute("viewBox", "0 0 24 24"); s.setAttribute("aria-hidden", "true"); if (cls) s.classList.add(cls);
    s.innerHTML = inner;   // our own constant icons only
    return s;
  }
  async function loadWays() {
    try { ways = await invoke("social_ways"); }
    catch (_) { if (!ways) ways = { herokey: true, google: true, discord: false }; }
    emit("leyline:signin", { ...flow, code });
    return ways;
  }
  const wayOn = (k) => !ways || ways[k] === true || k === "herokey";

  // The chooser and the waiting state, built into box (the title bar's pop-over, or Manage account's card).
  // Owner 31 Oct 19:15: Get your Hero Key first (a new account made right here, its key shown in Manage account), then
  // Sign in with your Hero Key (paste it here: no browser), Google or Discord (the website, in your browser), and a
  // name-and-password sign-in for players who set one. stream/herokeys.py answers the in-launcher ones.
  function buildFlow(box, big) {
    box.textContent = "";
    if (flow.stage === "busy") {
      const w = el("div", "sf-wait");
      const sp = el("span", "ly-spin"); sp.setAttribute("aria-hidden", "true");
      const t = el("div", "sf-wait-t"); t.append(el("b", "", flow.via === "new" ? "Making your account…" : "Signing in…"));
      w.append(sp, t); box.append(w);
      return;
    }
    if (flow.stage === "waiting") {
      const w = el("div", "sf-wait");
      const sp = el("span", "ly-spin"); sp.setAttribute("aria-hidden", "true");
      const t = el("div", "sf-wait-t");
      t.append(el("b", "", "Waiting for your browser…"),
        el("span", "", flow.via === "guest"
          ? "On the website, pick More ways to sign in, then Play as guest, and approve this code. Leyline continues by itself."
          : "Finish signing in there" + (flow.via ? " with " + (WAYS.find((x) => x[0] === flow.via) || ["", "the website"])[1] : "") + ", then approve this code. Leyline continues by itself."));
      w.append(sp, t);
      const c = el("p", "sx-code", code); c.setAttribute("aria-label", "Sign-in code " + code.split("").join(" "));
      const row = el("div", "sf-acts");
      const again = el("button", "btn small", "Open the page again"); again.type = "button";
      again.onclick = () => invoke("social_signin_reopen").catch((e) => fail(String(e)));
      const cancel = el("button", "link-s", "Cancel"); cancel.type = "button"; cancel.onclick = cancelSignIn;
      row.append(again, cancel);
      box.append(w, c, row);
      return;
    }
    if (flow.stage === "error") {
      const p = el("p", "sf-err", flow.note); p.setAttribute("role", "alert");
      box.append(p);
    }
    if (!big) box.append(el("p", "sf-h", "Sign in to Leyline"));
    // new here: an account and its Hero Key, right here
    const nw = el("button", "sf-way sf-new"); nw.type = "button"; nw.dataset.via = "new";
    nw.append(svg(WAY_ICON.herokey), el("span", "", "Get your Hero Key"));
    nw.onclick = newAccount;
    box.append(nw, el("p", "sf-fine sf-new-t", "New here? This makes your free account with a name made for you. Change the name any time."));
    const or = el("p", "sf-or"); or.append(el("span", "", "or sign in"));
    box.append(or);
    const list = el("div", "sf-ways");
    for (const [k, , label] of WAYS) {
      if (!wayOn(k)) continue;
      const b = el("button", "sf-way sf-" + k); b.type = "button"; b.dataset.via = k;
      b.append(svg(WAY_ICON[k]), el("span", "", label));
      if (k === "herokey") {
        b.setAttribute("aria-expanded", String(flow.form === "key")); b.setAttribute("aria-controls", "sf-keyform");
        b.onclick = () => { flow.form = flow.form === "key" ? "" : "key"; flow.note = ""; rebuild(box, big, "#sf-key"); };
        list.append(b);
        if (flow.form === "key") list.append(keyForm(box, big));
        continue;
      }
      b.onclick = () => signIn(k);
      list.append(b);
    }
    box.append(list);
    const pw = el("button", "link-s sf-pwlink", "Sign in with your name and password"); pw.type = "button";
    pw.setAttribute("aria-expanded", String(flow.form === "password"));
    pw.onclick = () => { flow.form = flow.form === "password" ? "" : "password"; flow.note = ""; rebuild(box, big, "#sf-pwname"); };
    box.append(pw);
    if (flow.form === "password") box.append(passwordForm(box, big));
    box.append(el("p", "sf-fine", "Google" + (wayOn("discord") ? " and Discord open" : " opens") + " the website in your browser. "
      + "One account works on Wrath Unleashed and every realm that accepts Hero Key."));
  }

  function rebuild(box, big, focus) {
    buildFlow(box, big);
    const f = focus && box.querySelector(focus); if (f) f.focus();
  }

  // Sign in with your Hero Key, pasted here (the core sends it once to the website and keeps only the sign-in token)
  function keyForm(box, big) {
    const f = el("form", "sf-form"); f.id = "sf-keyform"; f.noValidate = true;
    const lab = el("label", "sf-label", "Your Hero Key"); lab.htmlFor = "sf-key";
    const i = el("input"); i.id = "sf-key"; i.type = "password"; i.autocomplete = "off"; i.spellcheck = false; i.placeholder = "ovk1.";
    i.maxLength = 400;
    const go = el("button", "btn small primary", "Sign in"); go.type = "submit";
    const row = el("div", "sf-form-row"); row.append(i, go);
    f.append(lab, row, el("p", "sf-fine", "Paste the whole line from your key file. It starts with ovk1."));
    f.onsubmit = (e) => {
      e.preventDefault();
      const v = i.value.trim();
      if (!/ovk1\./.test(v)) { flow.note = "That doesn't look like a Hero Key. It starts with ovk1. Paste the whole line."; flow.stage = "error"; rebuild(box, big, "#sf-key"); return; }
      quickSignIn("key", () => invoke("social_key_signin", { key: v }));
    };
    return f;
  }

  function passwordForm(box, big) {
    const f = el("form", "sf-form"); f.id = "sf-pwform"; f.noValidate = true;
    const l1 = el("label", "sf-label", "Your name and #tag"); l1.htmlFor = "sf-pwname";
    const n = el("input"); n.id = "sf-pwname"; n.type = "text"; n.autocomplete = "username"; n.spellcheck = false; n.placeholder = "Alice#1234"; n.maxLength = 40;
    const l2 = el("label", "sf-label", "Password"); l2.htmlFor = "sf-pw";
    const p = el("input"); p.id = "sf-pw"; p.type = "password"; p.autocomplete = "current-password"; p.maxLength = 128;
    const go = el("button", "btn small primary", "Sign in"); go.type = "submit";
    const row = el("div", "sf-form-row"); row.append(p, go);
    f.append(l1, n, l2, row, el("p", "sf-fine", "Set a password in Manage account once you're signed in."));
    f.onsubmit = (e) => {
      e.preventDefault();
      if (!/^[^#\s]{2,24}#\d{4}$/.test(n.value.trim()) || !p.value) { flow.note = "Type your name with its #tag, like Alice#1234, and your password."; flow.stage = "error"; rebuild(box, big, "#sf-pwname"); return; }
      quickSignIn("password", () => invoke("social_password_signin", { name: n.value.trim(), password: p.value }));
    };
    return f;
  }

  // The in-launcher sign-ins (a key, a password, a new account): no browser, no code to approve.
  async function quickSignIn(via, call) {
    const id = ++runId;
    flow.via = via; flow.stage = "busy"; flow.note = "";
    renderCode();
    try {
      const out = await call();
      if (id !== runId) return;
      flow.stage = "idle"; flow.form = ""; signedIn = true;
      if (via === "new" && out && out.key) {
        fresh = { key: String(out.key), name: out.me ? `${out.me.name}#${out.me.tag}` : "" };
      }
      renderCode();
      await loadMe();
      if (via === "new" || (me && me.profile && me.profile.setup === false)) {
        if (window.LeylineNav) window.LeylineNav.show("account");
        emit("leyline:welcome", {});
      } else say("Signed in.", "ok");
    } catch (e) {
      if (id !== runId) return;
      flow.stage = "error"; flow.note = String(e); renderCode();
    }
  }
  function newAccount() { quickSignIn("new", () => invoke("social_new_account")); }

  // ---------------------------------------------------------------- the title bar's chip and its account panel
  // Owner 31 Oct 18:36: "make this popout menu from profile icon more consistent and in line with the module at top,
  // open a panel downward from it and open friends list, other stuff and sign out manage account etc". The whole chip
  // is one button; it opens #hf-panel under it, joined to it (account.css): you (picture, name#tag, how you show,
  // change name or picture), your friends (online first, requests, add a friend; the core's social_call "state",
  // "request", "accept" and "decline", asked each time the panel opens: no live poll), quick links, and the account
  // (Manage account, Hero Key, Sign out last). Esc or a click outside closes it; Tab stays inside; focus goes back to
  // the chip. Signed out, the same panel look holds the chooser (#hf-code) with Play as guest under it.
  let panel = null, friends = null, friendsErr = "", adding = false, addNote = ["", ""], panelRun = 0;
  const isGuest = () => !!(me && me.profile && me.profile.name_generated);
  const ICONS = {
    profile: '<circle cx="12" cy="8" r="4"/><path d="M4 20c1.5-3.5 4.5-5 8-5s6.5 1.5 8 5"/>',
    den: '<path d="M3 11l9-7 9 7"/><path d="M5 10v10h14V10"/><path d="M10 20v-5h4v5"/>',
    solo: '<path d="M14.5 4H20v5.5L9 20.5 3.5 15z"/><path d="M5 13l6 6M3 21l2.5-2.5"/>',
    settings: '<circle cx="12" cy="12" r="3"/><path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M4.9 19.1L7 17M17 7l2.1-2.1"/>',
    forum: '<path d="M4 4h11a1 1 0 0 1 1 1v7a1 1 0 0 1-1 1H9l-4 3.5V13H4a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1z"/><path d="M16 8h4a1 1 0 0 1 1 1v7a1 1 0 0 1-1 1h-1v3.5L15 17h-4a1 1 0 0 1-1-1v-1"/>',
    help: '<circle cx="12" cy="12" r="9"/><path d="M9.5 9a2.5 2.5 0 1 1 3.5 2.3c-.7.3-1 .9-1 1.7M12 17h.01"/>',
    manage: '<rect x="4" y="5" width="16" height="14" rx="2"/><circle cx="9.5" cy="11" r="2"/><path d="M6.5 16c.6-1.4 1.7-2 3-2s2.4.6 3 2M14.5 10h3M14.5 13h3"/>',
    key: '<circle cx="8" cy="12" r="4"/><path d="M12 12h9M18 12v3M15 12v2"/>',
    out: '<path d="M10 5H6a1 1 0 0 0-1 1v12a1 1 0 0 0 1 1h4"/><path d="M14 8l4 4-4 4M18 12H9"/>',
    plus: '<path d="M12 5v14M5 12h14"/>',
  };
  const icon = (k) => svg(ICONS[k], "ap-ic");
  const STATUS_LINE = { game: (f) => `Playing ${f.char || "a character"}${f.realm ? " on " + f.realm : ""}`, launcher: () => "In the launcher", offline: () => "Offline" };
  const RANK = { game: 0, launcher: 1, offline: 2 };

  function focusables(box) {
    return [...box.querySelectorAll("button, input, [tabindex]:not([tabindex='-1'])")]
      .filter((x) => !x.disabled && !x.closest("[hidden]") && x.getClientRects().length);
  }
  // Tab stays inside box; arrow keys move between its menu items
  function trap(box, e) {
    if (e.key === "Tab") {
      const f = focusables(box);
      if (!f.length) return;
      const i = f.indexOf(document.activeElement);
      if (e.shiftKey && i <= 0) { e.preventDefault(); f[f.length - 1].focus(); }
      else if (!e.shiftKey && i === f.length - 1) { e.preventDefault(); f[0].focus(); }
    } else if ((e.key === "ArrowDown" || e.key === "ArrowUp") && document.activeElement && document.activeElement.getAttribute("role") === "menuitem") {
      const items = [...box.querySelectorAll("[role='menuitem']")];
      const i = items.indexOf(document.activeElement);
      e.preventDefault();
      items[(i + (e.key === "ArrowDown" ? 1 : items.length - 1)) % items.length].focus();
    }
  }

  function closePanel(back) {
    if (!panel) return;
    panel.remove(); panel = null; panelRun++;
    const tb = document.querySelector(".tb-me"); if (tb) tb.classList.remove("open");
    const b = $("hf-me-btn");
    if (b) { b.setAttribute("aria-expanded", "false"); if (back) b.focus(); }
  }
  function openPanel(focus) {
    const tb = document.querySelector(".tb-me"), btn = $("hf-me-btn");
    if (!tb || !btn || !me) return;
    closePanel(false);
    panel = buildPanel();
    tb.append(panel); tb.classList.add("open");
    btn.setAttribute("aria-expanded", "true");
    if (focus) { const f = panel.querySelector(".ap-edit"); if (f) f.focus(); }
    loadFriends();
  }
  const go = (fn) => () => { closePanel(false); fn(); };
  const nav = (page) => () => { if (window.LeylineNav) window.LeylineNav.show(page); };
  // no Den or Solo leveling here (owner 1 Nov: one way in): Your Den is in the side menu and the play bar

  function menuItem(ic, label, line, fn, cls) {
    const b = el("button", "ap-item" + (cls ? " " + cls : "")); b.type = "button"; b.setAttribute("role", "menuitem");
    const t = el("span", "ap-item-t"); t.append(el("b", "", label)); if (line) t.append(el("small", "", line));
    b.append(icon(ic), t);
    b.onclick = go(fn);
    return b;
  }

  function buildPanel() {
    const p = el("div", "acct-panel"); p.id = "hf-panel";
    p.setAttribute("role", "dialog"); p.setAttribute("aria-label", "Your account");
    p.addEventListener("keydown", (e) => {
      if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); closePanel(true); return; }
      trap(p, e);
    });
    const guest = isGuest();

    // you
    const head = el("div", "ap-head");
    const who = el("div", "ap-who");
    const nm = el("p", "ap-name"); nm.append(el("b", "", me.name), el("span", "ap-tag", me.tag ? "#" + me.tag : ""));
    const st = el("p", "ap-st" + (me.offline ? " off" : "")); st.id = "ap-st";
    st.append(el("i", "ap-dot"), el("span", "", me.offline ? "Appearing offline" : "Online"));
    who.append(nm, st);
    if (guest) who.append(el("p", "ap-guest", "Guest account"));
    head.append(avatar(me.name, "ap-av", picOf(me)), who);
    const edit = el("button", "btn small ap-edit" + (guest ? " primary" : ""), guest ? "Pick your own name and picture" : "Change name or picture");
    edit.type = "button"; edit.onclick = go(manage);
    head.append(edit);
    p.append(head);

    // the scrolling middle: friends, then the quick links
    const mid = el("div", "ap-mid");
    const fr = el("section", "ap-sec ap-friends"); fr.setAttribute("aria-labelledby", "ap-fh");
    const fh = el("div", "ap-sec-h");
    const h = el("h3", "", "Friends"); h.id = "ap-fh";
    const count = el("span", "ap-count"); count.id = "ap-fcount";
    const add = el("button", "ap-add"); add.type = "button"; add.id = "ap-add";
    add.append(icon("plus"), el("span", "", "Add a friend"));
    add.setAttribute("aria-expanded", String(adding)); add.setAttribute("aria-controls", "ap-addform");
    add.onclick = () => { adding = !adding; addNote = ["", ""]; renderFriends(); if (adding) { const i = $("ap-addin"); if (i) i.focus(); } };
    fh.append(h, count, add);
    const form = el("form", "ap-addform"); form.id = "ap-addform"; form.hidden = !adding; form.noValidate = true;
    const inp = el("input"); inp.id = "ap-addin"; inp.type = "text"; inp.maxLength = 40; inp.autocomplete = "off"; inp.spellcheck = false;
    inp.placeholder = "Name#1234"; inp.setAttribute("aria-label", "Their name and tag, like Alice#1234");
    const send = el("button", "btn small primary", "Send"); send.type = "submit";
    const note = el("p", "ap-note"); note.id = "ap-addnote"; note.setAttribute("role", "status");
    form.append(inp, send, note);
    form.onsubmit = (e) => { e.preventDefault(); sendRequest(inp.value); };
    const list = el("ul", "ap-flist"); list.id = "ap-flist"; list.setAttribute("aria-label", "Friends");
    fr.append(fh, form, list);
    mid.append(fr);

    const ql = el("section", "ap-sec ap-links"); ql.setAttribute("aria-label", "Quick links");
    const qm = el("div", "ap-grid"); qm.setAttribute("role", "menu"); qm.setAttribute("aria-label", "Quick links");
    qm.append(menuItem("profile", "My profile", "On the website", () => openLink("profile")),
      menuItem("forum", "Forum", "Ask and share", nav("forum")),
      menuItem("settings", "Settings", "This launcher", nav("settings")),
      menuItem("help", "Help", "Guides and fixes", nav("help")));
    ql.append(qm);
    mid.append(ql);
    p.append(mid);

    // the account, Sign out last
    const ac = el("div", "ap-acct"); ac.setAttribute("role", "menu"); ac.setAttribute("aria-label", "Account");
    ac.append(menuItem("manage", "Manage account", "Name, picture, bio and ways to sign in", manage),
      menuItem("key", "Hero Key", guest ? "Save it to keep this guest account" : "See it, save it, make a new one", manageKey, guest ? "warn" : ""));
    const sep = el("div", "ap-sep"); sep.setAttribute("role", "separator");
    ac.append(sep, menuItem("out", "Sign out", "", signOut, "ap-out"));
    p.append(ac);
    renderFriendsInto(p);
    return p;
  }

  function renderFriends() { if (panel) renderFriendsInto(panel); }
  function renderFriendsInto(p) {
    const list = p.querySelector("#ap-flist"), count = p.querySelector("#ap-fcount"), form = p.querySelector("#ap-addform");
    const add = p.querySelector("#ap-add"), note = p.querySelector("#ap-addnote");
    if (!list) return;
    form.hidden = !adding; add.setAttribute("aria-expanded", String(adding));
    note.textContent = addNote[0]; note.className = "ap-note" + (addNote[1] ? " " + addNote[1] : "");
    list.textContent = "";
    if (!friends) {
      count.textContent = "";
      if (friendsErr) {
        const li = el("li", "ap-empty"); li.append(el("span", "", friendsErr));
        const again = el("button", "link-s", "Try again"); again.type = "button"; again.onclick = loadFriends;
        li.append(" ", again); list.append(li);
      } else list.append(el("li", "ap-empty", "Loading your friends…"));
      return;
    }
    const all = friends.friends.slice().sort((a, b) => (RANK[a.status] ?? 3) - (RANK[b.status] ?? 3) || String(a.name).localeCompare(String(b.name)));
    const on = all.filter((f) => f.status !== "offline").length;
    count.textContent = all.length ? `${on} of ${all.length} online` : "";
    for (const r of friends.incoming || []) {
      const li = el("li", "ap-f ap-req");
      li.append(avatar(r.name, "ap-fav"));
      const t = el("div", "ap-f-t"); t.append(el("b", "", `${r.name}#${r.tag}`), el("span", "", "Wants to be friends"));
      const yes = el("button", "btn small primary", "Accept"); yes.type = "button"; yes.onclick = () => act("accept", { pid: r.pid });
      const no = el("button", "btn small", "Decline"); no.type = "button"; no.onclick = () => act("decline", { pid: r.pid });
      const acts = el("div", "ap-f-acts"); acts.append(yes, no);
      li.append(t, acts); list.append(li);
    }
    for (const f of all) {
      const li = el("li", "ap-f st-" + f.status);
      const av = avatar(f.name, "ap-fav"); av.append(el("i", "ap-fdot"));
      const t = el("div", "ap-f-t"); t.append(el("b", "", f.name), el("span", "", (STATUS_LINE[f.status] || STATUS_LINE.offline)(f)));
      li.title = `${f.name}#${f.tag}`;
      li.append(av, t); list.append(li);
    }
    for (const o of friends.outgoing || []) {
      const li = el("li", "ap-f ap-wait");
      li.append(avatar(o.name, "ap-fav"));
      const t = el("div", "ap-f-t"); t.append(el("b", "", o.name), el("span", "", "Asked: waiting for them"));
      li.append(t); list.append(li);
    }
    if (!list.children.length) {
      const mine = me && me.tag ? `${me.name}#${me.tag}` : "";
      list.append(el("li", "ap-empty", "No friends yet. Add someone with their name and tag, like Alice#1234" + (mine ? `, or give them yours: ${mine}.` : ".")));
    }
  }

  async function loadFriends() {
    const id = panelRun;
    friendsErr = "";
    if (!friends) renderFriends();
    try {
      const s = await invoke("social_call", { action: "state", args: {} });
      if (id !== panelRun) return;
      friends = { friends: (s && s.friends) || [], incoming: (s && s.incoming) || [], outgoing: (s && s.outgoing) || [] };
    } catch (e) {
      if (id !== panelRun) return;
      if (String(e) === "signed_out") { closePanel(false); loadMe(); return; }
      friendsErr = "Couldn't load your friends right now.";
    }
    renderFriends();
  }

  async function act(action, args) {
    try { await invoke("social_call", { action, args }); }
    catch (e) { addNote = [String(e), "bad"]; }
    await loadFriends();
  }
  async function sendRequest(text) {
    const to = String(text || "").trim();
    if (!to) { addNote = ["Type their name and tag, like Alice#1234.", "bad"]; renderFriends(); return; }
    addNote = ["Sending…", ""]; renderFriends();
    try {
      await invoke("social_call", { action: "request", args: { to } });
      addNote = [`Asked ${to}. They show here once they say yes.`, "ok"];
      const i = $("ap-addin"); if (i) i.value = "";
    } catch (e) { addNote = [String(e) === "signed_out" ? "You were signed out. Sign in again." : String(e), "bad"]; }
    await loadFriends();
  }

  document.addEventListener("pointerdown", (e) => {
    if (panel && !e.target.closest(".tb-me")) closePanel(false);
    if (flow.stage === "choose" && !e.target.closest(".tb-me")) { flow.stage = "idle"; renderCode(); }
  });
  document.addEventListener("keydown", (e) => { if (e.key === "Escape" && flow.stage === "choose") { flow.stage = "idle"; renderCode(); const b = $("hf-signin"); if (b) b.focus(); } });
  document.addEventListener("leyline:page", () => closePanel(false));

  function renderMe() {
    const box = $("hf-me");
    if (!box) return;
    const wasOpen = !!panel;
    closePanel(false);
    if (!signedIn || !me) { friends = null; adding = false; }
    box.textContent = "";
    if (!signedIn || !me) {
      const t = el("div", "hf-me-t");
      t.append(el("b", "hf-me-name", "Leyline"), el("span", "hf-me-st", "Signed out"));
      const b = el("button", "hf-signin"); b.append(el("span", "", "Log In")); b.type = "button"; b.id = "hf-signin";   // the PLAY stone (owner 31 Oct 21:59)
      b.title = "Sign in on the website, in your browser";   // neutral: the chooser lists only the ways the website offers
      b.setAttribute("aria-haspopup", "dialog");
      b.onclick = () => {
        if (flow.stage === "waiting") { renderCode(); return; }
        flow.stage = flow.stage === "choose" ? "idle" : "choose"; flow.note = "";
        renderCode();
        if (flow.stage === "choose") { loadWays(); const f = document.querySelector("#hf-code .sf-way"); if (f) f.focus(); }
      };
      box.append(avatar("?", "hf-me-av hf-anon"), t, b);
      renderCode();
      return;
    }
    // the whole chip is the button: picture, name and #tag, the caret
    const btn = el("button", "hf-me-btn");
    btn.type = "button"; btn.id = "hf-me-btn";
    btn.setAttribute("aria-haspopup", "dialog"); btn.setAttribute("aria-expanded", "false"); btn.setAttribute("aria-controls", "hf-panel");
    const who = el("span", "hf-me-who");
    who.append(el("b", "hf-me-name", me.name), el("span", "hf-me-st", me.tag ? "#" + me.tag : "Signed in"));
    btn.append(avatar(me.name, "hf-me-av", picOf(me)), who, svg('<path d="M6 9l6 6 6-6"/>', "hf-caret"));
    btn.title = me.tag ? `${me.name}#${me.tag}` : me.name;
    btn.setAttribute("aria-label", `Your account: ${btn.title}`);
    btn.onclick = () => { if (panel) closePanel(true); else openPanel(true); };
    box.append(btn);
    renderCode();
    if (wasOpen) openPanel(false);
  }
  function manage() { if (window.LeylineNav) window.LeylineNav.show("account"); }
  // the panel's Hero Key: Manage account, at its Hero Key card
  function manageKey() { manage(); emit("leyline:focus-key", {}); }

  // the pop-over under the chip: the chooser, then the code while the browser is open
  function renderCode() {
    const box = $("hf-code");
    if (box) {
      const open = !signedIn && (flow.stage === "choose" || flow.stage === "waiting" || flow.stage === "error" || flow.stage === "busy")
        && !(window.LeylineNav && window.LeylineNav.current().page === "account");
      box.hidden = !open;
      if (open) buildFlow(box, false); else box.textContent = "";
      box.classList.add("acct-panel", "ap-signin");
      if (!box.dataset.trap) { box.dataset.trap = "1"; box.addEventListener("keydown", (e) => trap(box, e)); }
      const sb = $("hf-signin"); if (sb) sb.setAttribute("aria-expanded", String(open));
      const tb = document.querySelector(".tb-me"); if (tb) tb.classList.toggle("open-in", open);
      box.setAttribute("role", flow.stage === "waiting" ? "status" : "dialog");
      box.setAttribute("aria-label", "Sign in");
    }
    emit("leyline:signin", { ...flow, code });
  }

  async function loadMe() {
    try {
      const r = await invoke("social_call", { action: "me", args: {} });
      me = r && r.me ? r.me : null;
      extra = r ? { avatars: r.avatars || [], ways: r.ways || null, manage_url: r.manage_url || "" } : {};
      if (extra.ways) ways = Object.assign({}, ways || {}, extra.ways);
      signedIn = !!me;
    } catch (e) {
      if (String(e) === "signed_out") { signedIn = false; me = null; say("You were signed out. Sign in again.", "warn"); }
      else if (signedIn && !me) say("Couldn't reach the website to load your name. It will try again.", "warn");
    }
    renderMe();
    emit("leyline:me", { me, signedIn });
    return me;
  }

  function fail(note) { flow.stage = "error"; flow.note = note; code = ""; renderCode(); }

  async function signIn(via) {
    const id = ++runId;
    flow.via = typeof via === "string" ? via : ""; flow.note = "";
    try {
      const s = await invoke("social_signin_start", { via: flow.via && flow.via !== "guest" ? flow.via : null });
      if (id !== runId) return;
      code = s.user_code || "";
      flow.stage = "waiting";
      renderCode();
      say("");
      for (;;) {
        await sleep(4000);
        if (id !== runId || flow.stage !== "waiting") return;
        let p;
        try { p = await invoke("social_signin_poll"); }
        catch (e) { if (id !== runId) return; continue; }   // a dropped connection: keep waiting
        if (id !== runId) return;
        if (p.state === "pending") continue;
        code = "";
        if (p.state === "signed_in") {
          flow.stage = "idle"; signedIn = true;
          renderCode();
          await loadMe();
          if (me && me.profile && me.profile.setup === false) {   // the first time: the profile step, in Manage account
            if (window.LeylineNav) window.LeylineNav.show("account");
            emit("leyline:welcome", {});
          } else say("Signed in.", "ok");
        } else if (p.state === "denied") fail("Not signed in: the code was refused on the website.");
        else if (p.state === "signed_out") { flow.stage = "idle"; renderCode(); return; }
        else fail("The code expired. Pick a way to try again.");
        return;
      }
    } catch (e) { if (id === runId) fail(String(e)); }
  }

  async function cancelSignIn() {
    runId++;
    await invoke("social_signin_cancel").catch(() => {});
    code = ""; flow.stage = "idle"; flow.note = "";
    renderCode(); say("");
  }

  async function signOut() {
    signedIn = false; me = null; fresh = null;
    await invoke("social_signout").catch(() => {});
    renderMe(); say("Signed out. The sign-in was revoked on the website too.", "ok");
    emit("leyline:me", { me: null, signedIn: false });
  }

  // after a save in Manage account: the chip shows the new name and picture
  function setMe(m) { if (m) { me = Object.assign({}, me || {}, m); signedIn = true; renderMe(); emit("leyline:me", { me, signedIn }); } }

  // ---------------------------------------------------------------- under it: the Discord and the forum
  function renderBody() {
    const box = $("hf-body");
    if (!box) return;
    box.textContent = "";
    const dc = el("section", "hf-dc");
    dc.setAttribute("aria-labelledby", "hf-dc-h");
    const ic = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    ic.setAttribute("viewBox", "0 0 24 24"); ic.setAttribute("aria-hidden", "true"); ic.classList.add("hf-dc-ic");
    ic.innerHTML = '<path d="M4 6.5C4 5.1 5.1 4 6.5 4h11C18.9 4 20 5.1 20 6.5v8c0 1.4-1.1 2.5-2.5 2.5H11l-4.5 3.5V17C5.1 17 4 15.9 4 14.5z"/><circle cx="9" cy="10.5" r="1.2"/><circle cx="15" cy="10.5" r="1.2"/>';
    const h = el("h3", "", "Join the Discord"); h.id = "hf-dc-h";
    // TODO(owner): the Discord line in the launcher
    const p = el("p", "", "Find a group, voice chat, show what you made and hear about new realms first.");
    const b = el("button", "btn small primary hf-dc-go", "Join the Discord"); b.type = "button";
    b.onclick = () => openLink("discord");
    dc.append(ic, h, p, b);
    const fm = el("section", "hf-fm");
    fm.setAttribute("aria-labelledby", "hf-fm-h");
    const head = el("div", "hf-fm-head");
    const fh = el("h3", "", "On the forum"); fh.id = "hf-fm-h";
    const all = el("button", "link-s", "Open ›"); all.type = "button"; all.title = "Open the Forum page";
    all.onclick = () => window.LeylineNav && window.LeylineNav.show("forum");
    head.append(fh, all);
    const list = el("ul", "hf-fm-list"); list.id = "hf-fm-list";
    fm.append(head, list);
    box.append(dc, fm, Object.assign(el("p", "status hf-status"), { id: "hf-status" }));
    loadLatest();
  }

  async function loadLatest() {
    const list = $("hf-fm-list");
    if (!list) return;
    let threads = [];
    try {
      const r = await invoke("forum_call", { action: "latest", args: { n: 3 } });
      threads = (r && Array.isArray(r.threads)) ? r.threads.slice(0, 3) : [];
    } catch (_) {
      list.replaceChildren(el("li", "hint", "The forum isn't reachable right now."));
      return;
    }
    if (!threads.length) { list.replaceChildren(el("li", "hint", "No threads yet. Start the first one on the Forum page.")); return; }
    list.replaceChildren(...threads.map((t) => {
      const li = el("li");
      const b = el("button", "hf-fm-row"); b.type = "button";
      b.append(el("b", "", String(t.title || "")), el("span", "", `${t.author || "Someone"} · ${t.replies || 0} repl${t.replies === 1 ? "y" : "ies"}`));
      b.onclick = () => { if (window.LeylineForum) window.LeylineForum.openThread(t.id); };
      li.append(b);
      return li;
    }));
  }

  // ---------------------------------------------------------------- start
  renderBody();
  (async () => {
    let yes = false;
    try { yes = await invoke("social_status"); } catch (_) { yes = false; }
    signedIn = !!yes;
    renderMe();
    if (yes) loadMe(); else loadWays();
  })();
  // back on Home: the latest threads again
  document.addEventListener("leyline:page", (ev) => { if ((ev.detail || {}).page === "play") loadLatest(); });

  document.addEventListener("leyline:page", (ev) => { if ((ev.detail || {}).page !== "account" || flow.stage !== "idle") renderCode(); });

  window.LeylineSocial = { refresh: loadMe, me: () => me, extra: () => extra, signedIn: () => signedIn, signIn, signOut, cancelSignIn,
    loadLatest, flow: () => ({ ...flow, code }), ways: () => ways, loadWays, buildFlow, avatar, setMe, openLink,
    newAccount, freshKey: () => fresh, dropFreshKey: () => { fresh = null; } };
})();
