// Leyline Launcher: the sign-in screen (owner 2 Nov: "create login flow for the launcher so people have to log in to use
// it etc with hero key sensed from browser login or pasted"). Signed out, it covers the whole window: Log in or Sign up
// first, then the launcher.
//
// Log in: Sign in with your browser (the website's code sign-in, social.js signIn: a browser already signed in on the
// website, or keeping the Hero Key, approves it once), Continue with Google (the same, through Google), or the Hero
// Key itself: dropped or chosen as its text file (the file is read here, or by the core for a drop: social_key_file)
// or pasted. Sign up: Create your Hero Key (social.js newAccount; Manage account then shows the key once to save).
// No Discord sign-in (owner 2 Nov 21:04).
//
// It never locks anyone out: a sign-in kept from before lets them straight in, offline too (social_status is local;
// the chip says Offline); with the website not answering, the screen says so plainly, with Try again and Continue
// without signing in. A core that can't tell (social_status failing: a preview without one) shows no gate.
// PLAY on the official realm while signed out opens this first and goes on with the play once signed in (need()).
//
// Loaded after social.js and account.js, before app.js. Everything from the server is shown with textContent.
(function () {
  "use strict";
  const $ = (id) => document.getElementById(id);
  const el = (tag, cls, text) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  };
  const S = () => window.LeylineSocial;
  const tauri = window.__TAURI__ && window.__TAURI__.core ? window.__TAURI__.core.invoke : null;
  const win = window.__TAURI__ && window.__TAURI__.window ? window.__TAURI__.window.getCurrentWindow() : null;
  const realmName = () => (window.LeylineApp && window.LeylineApp.HANGOUT ? window.LeylineApp.HANGOUT.name : "Wrath Unleashed");
  const FILE = "Leyline Hero Key.txt";

  // open: the screen shows; phase: checking (the core not asked yet) | in | welcome (signed in just now, a moment);
  // tab: login | signup; then: what goes on once signed in (a PLAY); why: the line saying why it opened for that;
  // here: a sign-in started on this screen (its welcome is shown); note: a line of ours (a file that isn't a key)
  let open = true, phase = "checking", tab = "login", then = null, why = "", here = false, note = "", pw = false, over = false, welcomeT = 0;

  function setOpen(on) {
    open = on;
    const g = $("gate");
    if (g) g.hidden = !on;
    document.body.classList.toggle("gated", on);
    const f = document.querySelector(".frame");
    if (f) { if (on) f.setAttribute("inert", ""); else f.removeAttribute("inert"); }
    document.dispatchEvent(new CustomEvent("leyline:gate", { detail: { open: on } }));
  }

  // ---------------------------------------------------------------- the Hero Key, from a file or a paste
  const findKey = (t) => {
    const j = String(t || "").replace(/[\s­​-‍⁠﻿]+/g, "");
    const m = j.match(/ovk1\.[A-Za-z0-9]{10}\.[A-Za-z0-9_-]{43}/i);
    return m ? "ovk1." + m[0].slice(5) : null;
  };
  function say(text) { note = text || ""; render(); }
  async function readFile(file) {
    if (!file) return;
    if (file.size > 16384) return say(`That isn't a Hero Key file. Pick the one Leyline saved (${FILE}), or paste the key.`);
    let text = "";
    try { text = await file.text(); } catch (_) { return say("Couldn't read that file. Pick it again, or paste the key."); }
    const k = findKey(text);
    if (!k) return say(`There's no Hero Key in that file. Pick the one Leyline saved (${FILE}), or paste the key.`);
    go(() => S().keySignIn(k));
  }
  // a drop in the app (Tauri gives the path, app.js setupDrop): the core reads it
  function dropPath(path) {
    over = false;
    if (!path || !tauri) return render();
    go(() => quick(() => tauri("social_key_file", { path })));
  }
  // the core's own sign-in for a dropped file, through social.js's state (busy, then signed in or the error)
  const quick = (call) => S().keyFileSignIn(call);
  function go(run) { here = true; note = ""; run(); }

  // ---------------------------------------------------------------- the screen
  function brand() {
    const b = el("div", "gate-brand");
    const m = el("div", "ly-spin slow"); m.setAttribute("aria-hidden", "true");
    const t = el("p", "gate-word"); t.append(el("b", "", "Leyline"), el("i", "", "Launcher"));
    b.append(m, t);
    return b;
  }
  function spinLine(text, sub) {
    const w = el("div", "gate-wait");
    const sp = el("span", "ly-spin"); sp.setAttribute("aria-hidden", "true");
    const t = el("div", "gate-wait-t"); t.append(el("b", "", text));
    if (sub) t.append(el("span", "", sub));
    w.append(sp, t);
    return w;
  }
  function way(cls, label, icon, run, id) {
    const b = el("button", "sf-way " + cls); b.type = "button"; if (id) b.id = id;
    if (icon) b.append(icon);
    b.append(el("span", "", label));
    b.onclick = run;
    return b;
  }
  function svg(inner) {
    const s = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    s.setAttribute("viewBox", "0 0 24 24"); s.setAttribute("aria-hidden", "true");
    s.innerHTML = inner;   // our own constant icons only
    return s;
  }
  const IC = {
    browser: '<rect x="3" y="4" width="18" height="16" rx="2" fill="none" stroke="currentColor" stroke-width="1.8"/><path d="M3 9h18" stroke="currentColor" stroke-width="1.8"/><circle cx="6.5" cy="6.5" r=".9" fill="currentColor"/><circle cx="9.5" cy="6.5" r=".9" fill="currentColor"/>',
    google: '<path fill="#EA4335" d="M12 4.75c1.77 0 3.36.61 4.6 1.8l3.43-3.42C17.95 1.19 15.24 0 12 0 7.31 0 3.26 2.69 1.28 6.61l3.99 3.1C6.22 6.86 8.87 4.75 12 4.75z"/><path fill="#4285F4" d="M23.49 12.27c0-.79-.07-1.54-.19-2.27H12v4.51h6.47c-.29 1.48-1.13 2.74-2.39 3.59l3.86 3c2.26-2.09 3.55-5.18 3.55-8.83z"/><path fill="#FBBC05" d="M5.26 14.29A7.2 7.2 0 0 1 4.89 12c0-.8.14-1.57.38-2.29l-3.99-3.1A11.95 11.95 0 0 0 0 12c0 1.94.46 3.77 1.28 5.39l3.98-3.1z"/><path fill="#34A853" d="M12 24c3.24 0 5.97-1.06 7.95-2.9l-3.86-3c-1.08.72-2.46 1.15-4.09 1.15-3.13 0-5.78-2.11-6.73-4.96l-3.98 3.1C3.25 21.3 7.3 24 12 24z"/>',
    file: '<path d="M7 3h7l5 5v13H7z" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"/><path d="M14 3v5h5M10 13h6M10 17h6" fill="none" stroke="currentColor" stroke-width="1.8"/>',
    key: '<path fill="currentColor" d="M7 14a3 3 0 1 1 0-6 3 3 0 0 1 0 6Zm5.65-4A6 6 0 1 0 12.65 14H15v3h3v-3h2v-4h-7.35Z"/>',
  };

  const down = () => {
    const s = S(), f = s.flow();
    return s.waysDown() || (f.stage === "error" && /reach|answer|connection|offline/i.test(f.note));
  };
  const googleOn = () => { const w = S().ways(); return !w || w.google === true; };

  function render() {
    const body = $("gate-body");
    if (!body || !open) return;
    const s = S();
    body.textContent = "";
    const card = el("section", "gate-card");
    card.append(brand());
    body.append(card);
    if (!s || phase === "checking") { card.append(spinLine("Checking your sign-in…")); return; }
    const f = s.flow();
    if (phase === "welcome") {
      const me = s.me() || {};
      const w = el("div", "gate-in"); w.id = "gate-in"; w.setAttribute("role", "status");
      w.append(s.avatar(me.name || "?", "gate-av", (me.profile && me.profile.avatar_url) || ""));
      const t = el("p", "gate-in-n"); t.append(el("b", "", me.name || "Signed in"), el("span", "", me.tag ? "#" + me.tag : ""));
      w.append(el("p", "gate-in-h", "Signed in"), t);
      const c = el("button", "btn primary", then ? "Continue to PLAY" : "Continue"); c.type = "button"; c.id = "gate-go";
      c.onclick = finish;
      w.append(c);
      card.append(w);
      setTimeout(() => { const g = $("gate-go"); if (g) g.focus(); }, 0);
      return;
    }
    if (f.stage === "busy") { card.append(spinLine(f.via === "new" ? "Making your account…" : "Logging in…")); return; }
    if (f.stage === "waiting") {
      card.append(spinLine("Waiting for your browser…", f.via === "google" ? "Continue with Google there, then approve this code. Leyline goes on by itself."
        : "Log in there (a browser already signed in only asks you to approve), then approve this code. Leyline goes on by itself."));
      const c = el("p", "sx-code gate-code", f.code); c.id = "gate-code"; c.setAttribute("aria-label", "Sign-in code " + String(f.code).split("").join(" "));
      const row = el("div", "gate-acts");
      const again = el("button", "btn small", "Open the page again"); again.type = "button";
      again.onclick = () => tauri && tauri("social_signin_reopen").catch(() => {});
      const cancel = el("button", "link-s", "Cancel"); cancel.type = "button"; cancel.id = "gate-cancel"; cancel.onclick = () => s.cancelSignIn();
      row.append(again, cancel);
      card.append(c, row);
      return;
    }

    // the two tabs
    const tabs = el("div", "gate-tabs"); tabs.setAttribute("role", "tablist"); tabs.setAttribute("aria-label", "Log in or sign up");
    for (const [k, label] of [["login", "Log in"], ["signup", "Sign up"]]) {
      const t = el("button", "gate-tab", label); t.type = "button"; t.id = "gate-tab-" + k; t.setAttribute("role", "tab");
      t.setAttribute("aria-selected", String(tab === k)); t.setAttribute("aria-controls", "gate-pane"); t.tabIndex = tab === k ? 0 : -1;
      t.onclick = () => { tab = k; note = ""; render(); const n = $("gate-tab-" + k); if (n) n.focus(); };
      t.onkeydown = (e) => { if (e.key === "ArrowRight" || e.key === "ArrowLeft") { e.preventDefault(); tab = tab === "login" ? "signup" : "login"; render(); $("gate-tab-" + tab).focus(); } };
      tabs.append(t);
    }
    card.append(tabs);
    if (why) { const w = el("p", "gate-why", why); w.id = "gate-why"; card.append(w); }
    const pane = el("div", "gate-pane"); pane.id = "gate-pane"; pane.setAttribute("role", "tabpanel"); pane.setAttribute("aria-labelledby", "gate-tab-" + tab);
    card.append(pane);
    const err = (f.stage === "error" && f.note) || note;
    if (err) { const p = el("p", "sf-err", err); p.id = "gate-err"; p.setAttribute("role", "alert"); pane.append(p); }
    if (down()) {
      const d = el("div", "gate-down"); d.id = "gate-down";
      d.append(el("p", "", "The Leyline website isn't answering, so signing in can't work right now."));
      const a = el("div", "gate-acts");
      const again = el("button", "btn small", "Try again"); again.type = "button"; again.id = "gate-retry";
      again.onclick = async () => { note = ""; await s.loadWays(); render(); };
      const sk = el("button", "link-s", "Continue without logging in"); sk.type = "button"; sk.id = "gate-skip";
      sk.onclick = skip;
      a.append(again, sk); d.append(a); pane.append(d);
    }
    if (tab === "login") loginPane(pane); else signupPane(pane);
  }

  function loginPane(pane) {
    const s = S();
    pane.append(el("p", "gate-lede", `One account for the launcher, the website and ${realmName()}.`));
    const ways = el("div", "gate-ways");
    const br = el("button", "btn primary gate-big", "Log in with your browser"); br.type = "button"; br.id = "gate-browser";
    br.onclick = () => go(() => s.signIn(""));
    ways.append(br, el("p", "sf-fine gate-under", "Signed in on the website already, or your Hero Key kept in your browser? Approve once and you're in."));
    if (googleOn()) ways.append(way("sf-google", "Continue with Google", svg(IC.google), () => go(() => s.signIn("google")), "gate-google"));
    pane.append(ways);
    const or = el("p", "sf-or"); or.append(el("span", "", "or use your Hero Key")); pane.append(or);

    // the key file: dropped here, or chosen
    const drop = el("div", "gate-drop" + (over ? " over" : "")); drop.id = "gate-drop";
    const input = el("input"); input.type = "file"; input.id = "gate-file"; input.accept = ".txt,text/plain"; input.hidden = true;
    input.onchange = () => { const fl = input.files && input.files[0]; input.value = ""; readFile(fl); };
    const choose = el("button", "btn small", "Choose the file"); choose.type = "button"; choose.id = "gate-choose";
    choose.onclick = () => input.click();
    const dt = el("div", "gate-drop-t"); dt.append(el("b", "", "Drop your Hero Key file here"), el("span", "", FILE));
    drop.append(svg(IC.file), dt, choose, input);
    drop.addEventListener("dragover", (e) => { e.preventDefault(); if (!over) { over = true; drop.classList.add("over"); } });
    drop.addEventListener("dragleave", () => { over = false; drop.classList.remove("over"); });
    drop.addEventListener("drop", (e) => { e.preventDefault(); over = false; drop.classList.remove("over");
      const fl = e.dataTransfer && e.dataTransfer.files && e.dataTransfer.files[0]; readFile(fl); });
    pane.append(drop);

    // or pasted
    const form = el("form", "sf-form gate-key"); form.id = "gate-keyform"; form.noValidate = true;
    const lab = el("label", "sf-label", "Or paste your Hero Key"); lab.htmlFor = "gate-key";
    const i = el("input"); i.id = "gate-key"; i.type = "password"; i.autocomplete = "off"; i.spellcheck = false; i.placeholder = "ovk1."; i.maxLength = 400;
    const b = el("button", "btn small primary", "Log in"); b.type = "submit";
    const row = el("div", "sf-form-row"); row.append(i, b);
    form.append(lab, row);
    form.onsubmit = (e) => {
      e.preventDefault();
      const k = findKey(i.value) || (/ovk1\./i.test(i.value) ? i.value : null);
      if (!k) { say("That doesn't look like a Hero Key. It starts with ovk1. Paste the whole line."); const n = $("gate-key"); if (n) n.focus(); return; }
      go(() => s.keySignIn(k));
    };
    pane.append(form);

    // a name and password, for players who set one in Manage account
    const pl = el("button", "link-s gate-pwlink", "Log in with your name and password"); pl.type = "button"; pl.id = "gate-pwlink";
    pl.setAttribute("aria-expanded", String(pw));
    pl.onclick = () => { pw = !pw; render(); const n = $(pw ? "gate-pwname" : "gate-pwlink"); if (n) n.focus(); };
    pane.append(pl);
    if (pw) {
      const pf = el("form", "sf-form"); pf.id = "gate-pwform"; pf.noValidate = true;
      const l1 = el("label", "sf-label", "Your name and #tag"); l1.htmlFor = "gate-pwname";
      const n = el("input"); n.id = "gate-pwname"; n.type = "text"; n.autocomplete = "username"; n.spellcheck = false; n.placeholder = "Alice#1234"; n.maxLength = 40;
      const l2 = el("label", "sf-label", "Password"); l2.htmlFor = "gate-pw";
      const p = el("input"); p.id = "gate-pw"; p.type = "password"; p.autocomplete = "current-password"; p.maxLength = 128;
      const go2 = el("button", "btn small primary", "Log in"); go2.type = "submit";
      const r2 = el("div", "sf-form-row"); r2.append(p, go2);
      pf.append(l1, n, l2, r2);
      pf.onsubmit = (e) => {
        e.preventDefault();
        const who = n.value.normalize("NFC").trim().replace(/\s*#\s*/, "#");
        if (!/^[^#\s]{2,24}#\d{4}$/.test(who) || !p.value) { say("Type your name with its #tag, like Alice#1234, and your password."); return; }
        go(() => s.passwordSignIn(who, p.value));
      };
      pane.append(pf);
    }
    const foot = el("p", "gate-foot"); foot.append("New to Leyline? ");
    const su = el("button", "link-s", "Sign up"); su.type = "button"; su.id = "gate-to-signup";
    su.onclick = () => { tab = "signup"; note = ""; render(); const n = $("gate-new"); if (n) n.focus(); };
    foot.append(su);
    pane.append(foot);
  }

  function signupPane(pane) {
    const s = S();
    pane.append(el("p", "gate-lede", `Your Hero Key is your Leyline account: it signs you in here, on the website and on ${realmName()}. `
        + "Leyline makes it now, free, with a name made for you that you can change."));
    const nw = el("button", "btn primary gate-big", "Create your Hero Key"); nw.type = "button"; nw.id = "gate-new";
    nw.onclick = () => go(() => s.newAccount());
    pane.append(nw, el("p", "sf-fine gate-under", `Next you see your key once: save it as ${FILE} and keep it like a password.`));
    if (googleOn()) {
      const or = el("p", "sf-or"); or.append(el("span", "", "or")); pane.append(or);
      pane.append(way("sf-google", "Sign up with Google", svg(IC.google), () => go(() => s.signIn("google")), "gate-google-up"));
    }
    const d = el("details", "gate-what"); d.id = "gate-what";
    d.append(el("summary", "", "What is a Hero Key?"),
      el("p", "", "A Hero Key is one line of text that starts with ovk1. It is your account: it signs you in to Leyline, "
        + `the website and ${realmName()} on any computer, and on community realms that accept Hero Key.`),
      el("p", "", "Keep its file somewhere safe, like a password: anyone with it can log in as you. Lost it? Log in another way "
        + "and make a new one in Manage account."));
    pane.append(d);
    const foot = el("p", "gate-foot"); foot.append("Already have a Hero Key? ");
    const li = el("button", "link-s", "Log in"); li.type = "button"; li.id = "gate-to-login";
    li.onclick = () => { tab = "login"; note = ""; render(); const n = $("gate-browser"); if (n) n.focus(); };
    foot.append(li);
    pane.append(foot);
  }

  // ---------------------------------------------------------------- in and out
  // Continue without signing in (only offered while the website isn't answering): the launcher, signed out; PLAY on
  // the official realm still opens this first
  function skip() { then = null; why = ""; phase = "in"; setOpen(false); }
  function finish() {
    clearTimeout(welcomeT);
    phase = "in"; here = false; why = "";
    const run = then; then = null;
    setOpen(false);
    if (run) setTimeout(run, 0);
  }
  function show(reason, next) {
    phase = "in"; tab = "login"; note = "";
    if (reason !== undefined) why = reason || "";
    if (next !== undefined) then = next;
    setOpen(true);
    if (!S().ways()) S().loadWays();
    render();
    setTimeout(() => { const b = $("gate-browser"); if (b) b.focus(); }, 0);
  }
  function signedInNow() {
    const s = S();
    if (!open) return;
    // a new account: Manage account shows the key to save; nothing else waits for it
    if (s.freshKey() || (window.LeylineAccount && window.LeylineAccount.welcome())) { then = null; finish(); return; }
    if (!here) { finish(); return; }
    phase = "welcome"; render();
    clearTimeout(welcomeT);
    welcomeT = setTimeout(finish, 1600);
  }

  document.addEventListener("leyline:status", (ev) => {
    const d = ev.detail || {};
    if (!d.known || d.signedIn) { phase = "in"; setOpen(false); return; }
    show();
  });
  document.addEventListener("leyline:me", (ev) => {
    const d = ev.detail || {};
    if (d.signedIn) { if (S().me()) signedInNow(); return; }
    if (S().known() && !open) show("");   // signed out (Sign out, or the website refused the sign-in): the screen again
  });
  document.addEventListener("leyline:signin", () => { if (open && phase !== "welcome" && phase !== "checking") render(); });

  // its own window buttons: the title bar's are under it
  const wb = (id, fn) => { const b = $(id); if (b) b.addEventListener("click", () => win && fn(win)); };
  wb("gate-min", (w) => w.minimize());
  wb("gate-max", (w) => w.toggleMaximize());
  wb("gate-close", (w) => w.close());
  document.addEventListener("keydown", (e) => { if (open && e.key === "Escape" && S() && S().flow().stage === "waiting") S().cancelSignIn(); });

  // PLAY on the official realm: signed in first (or no way to tell, or no screen): run now; else this screen, then it
  const ready = () => { const s = S(); return !s || !s.known() || s.signedIn(); };
  function need(reason, next) {
    if (ready()) { next(); return true; }
    show(reason, next);
    return false;
  }

  window.LeylineGate = { isOpen: () => open, ready, need, show, skip, dropPath, dragOver: (on) => { if (over !== on) { over = on; const d = $("gate-drop"); if (d) d.classList.toggle("over", on); } } };
  // social.js asked the core before this ran (its answer can come first): go by it
  const s0 = S();
  if (s0 && s0.checked()) {
    if (!s0.known() || s0.signedIn()) { phase = "in"; setOpen(false); }
    else show();
  } else { setOpen(true); render(); }
})();
