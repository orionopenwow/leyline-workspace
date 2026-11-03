// Leyline Launcher: the sign-in dialog (owner 2 Nov 23:14: "I want the launcher to download with no immediate login screen,
// but encouragement to log in top right, and with access to hero key in a simplified way, which is a generated username
// and password that auto-detects from the browser, as well as Google").
//
// The launcher opens straight away, signed in or not. Signed out, the title bar's Log In (social.js renderMe) invites;
// it opens this dialog over the launcher, and so does PLAY on the official realm (need(): signed in first, then the
// play goes on). Not now, Esc or a click beside the card closes it.
//
// One pane (owner 2 Nov 23:31: "Sign In / Create Guest Account / Hero Key, Google Sign In, Instant Launcher Recognition"):
// Sign in with your browser (the website's code sign-in, social.js signIn: a browser already signed in on the website
// only approves: the launcher recognises it), Continue with Google, your name#tag and password (the fields a password
// manager fills); Create a guest account: the account and a password for it at once (social.js newLogin), signed in and
// on to play at once, nothing to read first. Its login (name, password, Hero Key and the text file) waits in Manage
// account › Your login until saved (leyline.guestLogin; account.js), with a reminder. The Hero Key file or a pasted key
// is folded under "Log in with your Hero Key".
//
// A Hero Key file in Downloads (the website's Create a Hero Key saves one, owner 2 Nov 23:18) signs in by itself: when
// the launcher starts signed out and while this dialog is open (social_key_found, every few seconds). A file from before
// the player's last Log out is never used by itself again (leyline.signedOutAt, social.js signOut): this dialog offers
// it as one button instead, "Log in as Name#1234" (3 Nov: logging back in on the same PC was 3 clicks and a file picker).
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
  const realmName = () => (window.LeylineApp && window.LeylineApp.HANGOUT ? window.LeylineApp.HANGOUT.name : "Wrath Unleashed");
  const FILE = "Leyline Hero Key.txt";
  const FIND_EVERY = 3000;

  // open: the dialog shows; then: what goes on once signed in (a PLAY); why: the line saying why it
  // opened for that; here: a sign-in started in this dialog (its welcome is shown); note: a line of ours; keyWays: the
  // Hero Key file and paste shown
  let open = false, phase = "in", then = null, why = "", here = false, note = "", keyWays = false, over = false,
    welcomeT = 0, findT = 0, finding = false, lastFocus = null, known = null;

  function setOpen(on) {
    if (open === on) return;
    open = on;
    const g = $("gate");
    if (g) g.hidden = !on;
    document.body.classList.toggle("gated", on);
    const f = document.querySelector(".frame");
    if (f) { if (on) f.setAttribute("inert", ""); else f.removeAttribute("inert"); }
    if (on) { lastFocus = document.activeElement; findSoon(); }
    else {
      clearTimeout(findT);
      if (lastFocus && lastFocus.focus && document.contains(lastFocus)) { try { lastFocus.focus(); } catch (_) { /* fine */ } }
      lastFocus = null;
    }
    document.dispatchEvent(new CustomEvent("leyline:gate", { detail: { open: on } }));
  }

  // ---------------------------------------------------------------- a Hero Key file in Downloads
  const outAt = () => { try { return Number(localStorage.getItem("leyline.signedOutAt")) || 0; } catch (_) { return 0; } };
  const triedAt = () => { try { return Number(localStorage.getItem("leyline.keyFileTried")) || 0; } catch (_) { return 0; } };
  // signed out, a key file newer than the last Sign out and than the last one tried: sign in with it
  async function findKeyFile() {
    if (!tauri || finding) return false;
    const s = S();
    if (!s || s.signedIn() || (s.flow().stage === "busy")) return false;
    finding = true;
    try {
      const f = await tauri("social_key_found");
      // a key file from before the last Log out isn't used by itself, but it is offered: "Log in as …" (one click)
      const was = known ? known.path + known.mtime_ms : "";
      known = f && f.path && f.mtime_ms <= outAt() ? { path: f.path, name: f.name || "", mtime_ms: f.mtime_ms } : null;
      if ((known ? known.path + known.mtime_ms : "") !== was && open && phase === "in") render();
      if (!f || !f.path || !(f.mtime_ms > outAt()) || !(f.mtime_ms > triedAt())) return false;
      try { localStorage.setItem("leyline.keyFileTried", String(f.mtime_ms)); } catch (_) { /* fine */ }
      here = open;
      note = "";
      const who = f.name ? ` as ${f.name}` : "";
      await S().keyFileSignIn(() => tauri("social_key_file", { path: f.path }));
      if (S().signedIn()) toast(`Logged in${who} with the Hero Key you downloaded.`);
      return true;
    } catch (_) {
      return false;
    } finally {
      finding = false;
    }
  }
  function findSoon() {
    clearTimeout(findT);
    if (!open || !tauri) return;
    findT = setTimeout(async () => { await findKeyFile(); findSoon(); }, FIND_EVERY);
  }
  function toast(text, onClick) {
    let t = $("gate-toast");
    if (!t) { t = el("button", "gate-toast"); t.type = "button"; t.id = "gate-toast"; t.setAttribute("role", "status"); document.body.append(t); }
    t.textContent = text;
    t.hidden = false;
    t.onclick = () => { t.hidden = true; if (onClick) onClick(); };
    clearTimeout(t._h);
    t._h = setTimeout(() => { t.hidden = true; }, onClick ? 12000 : 6000);
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
    go(() => S().keyFileSignIn(() => tauri("social_key_file", { path })));
  }
  function go(run) { here = true; note = ""; run(); }

  // ---------------------------------------------------------------- a password made for a new login
  // 4 groups of 4 from letters and digits that can't be mistaken for each other: easy to read out, ~94 bits
  function makePassword() {
    const A = "abcdefghjkmnpqrstuvwxyz23456789";
    const r = new Uint32Array(16);
    (window.crypto || window.msCrypto).getRandomValues(r);
    let out = "";
    for (let i = 0; i < 16; i++) { if (i && i % 4 === 0) out += "-"; out += A[r[i] % A.length]; }
    return out;
  }
  // Create a guest account: made, signed in, a password set; the login kept for Manage account › Your login
  const GUEST = "leyline.guestLogin";
  async function createGuest() {
    here = false; note = "";
    phase = "making"; render();
    const out = await S().newLogin();
    if (!out || !out.key) { phase = "in"; render(); return; }   // social.js put the error in its flow
    const name = out.me ? `${out.me.name}${out.me.tag ? "#" + out.me.tag : ""}` : "";
    let pass = makePassword();
    try { await tauri("social_call", { action: "password", args: { password: pass } }); } catch (_) { pass = ""; }
    try { localStorage.setItem(GUEST, JSON.stringify({ name, pass, key: String(out.key), at: Date.now() })); } catch (_) { /* fine */ }
    phase = "in";
    finish();
    toast(`You're in as ${name || "a guest"}. Save your login any time: Manage account › Your login.`, () => { if (window.LeylineNav) window.LeylineNav.show("account"); });
    document.dispatchEvent(new CustomEvent("leyline:guest", { detail: { name } }));
  }

  // ---------------------------------------------------------------- the dialog
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
    google: '<path fill="#EA4335" d="M12 4.75c1.77 0 3.36.61 4.6 1.8l3.43-3.42C17.95 1.19 15.24 0 12 0 7.31 0 3.26 2.69 1.28 6.61l3.99 3.1C6.22 6.86 8.87 4.75 12 4.75z"/><path fill="#4285F4" d="M23.49 12.27c0-.79-.07-1.54-.19-2.27H12v4.51h6.47c-.29 1.48-1.13 2.74-2.39 3.59l3.86 3c2.26-2.09 3.55-5.18 3.55-8.83z"/><path fill="#FBBC05" d="M5.26 14.29A7.2 7.2 0 0 1 4.89 12c0-.8.14-1.57.38-2.29l-3.99-3.1A11.95 11.95 0 0 0 0 12c0 1.94.46 3.77 1.28 5.39l3.98-3.1z"/><path fill="#34A853" d="M12 24c3.24 0 5.97-1.06 7.95-2.9l-3.86-3c-1.08.72-2.46 1.15-4.09 1.15-3.13 0-5.78-2.11-6.73-4.96l-3.98 3.1C3.25 21.3 7.3 24 12 24z"/>',
    file: '<path d="M7 3h7l5 5v13H7z" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"/><path d="M14 3v5h5M10 13h6M10 17h6" fill="none" stroke="currentColor" stroke-width="1.8"/>',
  };

  const down = () => {
    const s = S(), f = s.flow();
    return s.waysDown() || (f.stage === "error" && /reach|answer|connection|offline/i.test(f.note));
  };
  const googleOn = () => { const w = S().ways(); return !w || w.google === true; };

  function closeBtn() {
    const x = el("button", "gate-x", "Not now"); x.type = "button"; x.id = "gate-close-dlg";
    x.setAttribute("aria-label", "Not now: close");
    x.onclick = close;
    return x;
  }

  function render() {
    const body = $("gate-body");
    if (!body || !open) return;
    const s = S();
    body.textContent = "";
    const card = el("section", "gate-card");
    card.append(closeBtn(), brand());
    body.append(card);
    if (!s) { card.append(spinLine("Getting ready…")); return; }
    const f = s.flow();
    if (phase === "making") { card.append(spinLine("Making your guest account…")); return; }
    if (phase === "welcome") {
      const me = s.me() || {};
      const w = el("div", "gate-in"); w.id = "gate-in"; w.setAttribute("role", "status");
      w.append(s.avatar(me.name || "?", "gate-av", (me.profile && me.profile.avatar_url) || ""));
      const t = el("p", "gate-in-n"); t.append(el("b", "", me.name || "Logged in"), el("span", "", me.tag ? "#" + me.tag : ""));
      w.append(el("p", "gate-in-h", "Logged in"), t);
      const c = el("button", "btn primary", then ? "Continue to PLAY" : "Continue"); c.type = "button"; c.id = "gate-go";
      c.onclick = finish;
      w.append(c);
      card.append(w);
      setTimeout(() => { const g = $("gate-go"); if (g) g.focus(); }, 0);
      return;
    }
    if (f.stage === "busy") { card.append(spinLine(f.via === "login" || f.via === "new" ? "Making your guest account…" : "Logging in…")); return; }
    if (f.stage === "waiting") {
      card.append(spinLine("Waiting for your browser…", f.via === "google" ? "Continue with Google there, then approve this code. Leyline goes on by itself."
        : "Log in there (a browser already logged in only asks you to approve), then approve this code. Leyline goes on by itself."));
      const c = el("p", "sx-code gate-code", f.code); c.id = "gate-code"; c.setAttribute("aria-label", "Sign-in code " + String(f.code).split("").join(" "));
      const row = el("div", "gate-acts");
      const again = el("button", "btn small", "Open the page again"); again.type = "button";
      again.onclick = () => tauri && tauri("social_signin_reopen").catch(() => {});
      const cancel = el("button", "link-s", "Cancel"); cancel.type = "button"; cancel.id = "gate-cancel"; cancel.onclick = () => s.cancelSignIn();
      row.append(again, cancel);
      card.append(c, row);
      return;
    }

    card.append(el("h2", "gate-h", "Log in to Leyline"));
    if (why) { const w = el("p", "gate-why", why); w.id = "gate-why"; card.append(w); }
    const pane = el("div", "gate-pane"); pane.id = "gate-pane";
    card.append(pane);
    const err = (f.stage === "error" && f.note) || note;
    if (err) { const p = el("p", "sf-err", err); p.id = "gate-err"; p.setAttribute("role", "alert"); pane.append(p); }
    if (down()) {
      const d = el("div", "gate-down"); d.id = "gate-down";
      d.append(el("p", "", "The Leyline website isn't answering, so logging in can't work right now. Everything else in the launcher does."));
      const a = el("div", "gate-acts");
      const again = el("button", "btn small", "Try again"); again.type = "button"; again.id = "gate-retry";
      again.onclick = async () => { note = ""; await s.loadWays(); render(); };
      const sk = el("button", "link-s", "Continue without logging in"); sk.type = "button"; sk.id = "gate-skip";
      sk.onclick = skip;
      a.append(again, sk); d.append(a); pane.append(d);
    }
    loginPane(pane);
  }

  function loginPane(pane) {
    const s = S();
    pane.append(el("p", "gate-lede", `One account for the launcher, the website and ${realmName()}.`));
    const ways = el("div", "gate-ways");
    // back after a Log out on this PC: the Hero Key file in Downloads, one click (never by itself: the player logged out)
    if (known) {
      const again = el("button", "btn primary gate-big gate-again", known.name ? `Log in as ${known.name}` : "Log in with your Hero Key file");
      again.type = "button"; again.id = "gate-again";
      const path = known.path;
      again.onclick = () => go(() => s.keyFileSignIn(() => tauri("social_key_file", { path })));
      ways.append(again, el("p", "sf-fine gate-under", `With ${FILE} from your Downloads.`));
    }
    const br = el("button", known ? "btn gate-big" : "btn primary gate-big", "Log in with your browser"); br.type = "button"; br.id = "gate-browser";
    br.onclick = () => go(() => s.signIn(""));
    ways.append(br, el("p", "sf-fine gate-under", "Instant launcher recognition: logged in on leylinelauncher.com? Approve once and you're in."));
    if (googleOn()) ways.append(way("sf-google", "Continue with Google", svg(IC.google), () => go(() => s.signIn("google")), "gate-google"));
    pane.append(ways);
    const or = el("p", "sf-or"); or.append(el("span", "", "or your name and password")); pane.append(or);

    // the name#tag and password (a password manager fills these)
    const pf = el("form", "sf-form gate-pwform"); pf.id = "gate-pwform"; pf.noValidate = true; pf.autocomplete = "on";
    const l1 = el("label", "sf-label", "Name and #tag"); l1.htmlFor = "gate-pwname";
    const n = el("input"); n.id = "gate-pwname"; n.name = "username"; n.type = "text"; n.autocomplete = "username"; n.spellcheck = false; n.placeholder = "Brightfeather42#1234"; n.maxLength = 40;
    const l2 = el("label", "sf-label", "Password"); l2.htmlFor = "gate-pw";
    const p = el("input"); p.id = "gate-pw"; p.name = "password"; p.type = "password"; p.autocomplete = "current-password"; p.maxLength = 128;
    const go2 = el("button", "btn small primary", "Log in"); go2.type = "submit"; go2.id = "gate-pwgo";
    const r2 = el("div", "sf-form-row"); r2.append(p, go2);
    pf.append(l1, n, l2, r2);
    pf.onsubmit = (e) => {
      e.preventDefault();
      const who = n.value.normalize("NFC").trim().replace(/\s*#\s*/, "#");
      if (!/^[^#\s]{2,24}#\d{4}$/.test(who) || !p.value) { say("Type your name with its #tag, like Brightfeather42#1234, and your password."); return; }
      go(() => s.passwordSignIn(who, p.value));
    };
    pane.append(pf);

    // the Hero Key file or the key itself, folded away
    const kl = el("button", "link-s gate-keylink", keyWays ? "Hide the Hero Key options" : "Log in with your Hero Key"); kl.type = "button"; kl.id = "gate-keylink";
    kl.setAttribute("aria-expanded", String(keyWays));
    kl.onclick = () => { keyWays = !keyWays; render(); const x = $(keyWays ? "gate-choose" : "gate-keylink"); if (x) x.focus(); };
    pane.append(kl);
    if (keyWays) {
      const drop = el("div", "gate-drop" + (over ? " over" : "")); drop.id = "gate-drop";
      const input = el("input"); input.type = "file"; input.id = "gate-file"; input.accept = ".txt,text/plain"; input.hidden = true;
      input.onchange = () => { const fl = input.files && input.files[0]; input.value = ""; readFile(fl); };
      const choose = el("button", "btn small", "Choose the file"); choose.type = "button"; choose.id = "gate-choose";
      choose.onclick = () => input.click();
      const dt = el("div", "gate-drop-t"); dt.append(el("b", "", "Drop your Hero Key file here"), el("span", "", `${FILE}, from Downloads it is found by itself`));
      drop.append(svg(IC.file), dt, choose, input);
      drop.addEventListener("dragover", (e) => { e.preventDefault(); if (!over) { over = true; drop.classList.add("over"); } });
      drop.addEventListener("dragleave", () => { over = false; drop.classList.remove("over"); });
      drop.addEventListener("drop", (e) => { e.preventDefault(); over = false; drop.classList.remove("over");
        const fl = e.dataTransfer && e.dataTransfer.files && e.dataTransfer.files[0]; readFile(fl); });
      pane.append(drop);
      const form = el("form", "sf-form gate-key"); form.id = "gate-keyform"; form.noValidate = true;
      const lab = el("label", "sf-label", "Or paste your Hero Key"); lab.htmlFor = "gate-key";
      const i = el("input"); i.id = "gate-key"; i.type = "password"; i.autocomplete = "off"; i.spellcheck = false; i.placeholder = "ovk1."; i.maxLength = 400;
      const b = el("button", "btn small primary", "Log in"); b.type = "submit";
      const row = el("div", "sf-form-row"); row.append(i, b);
      form.append(lab, row);
      form.onsubmit = (e) => {
        e.preventDefault();
        const k = findKey(i.value) || (/ovk1\./i.test(i.value) ? i.value : null);
        if (!k) { say("That doesn't look like a Hero Key. It starts with ovk1. Paste the whole line."); const x = $("gate-key"); if (x) x.focus(); return; }
        go(() => s.keySignIn(k));
      };
      pane.append(form);
    }
    // new here: a guest account, at once
    const gs = el("div", "gate-guest"); gs.id = "gate-guest";
    gs.append(el("p", "gate-guest-h", "New to Leyline?"));
    const nw = el("button", "btn gate-big gate-guest-go", "Create a guest account"); nw.type = "button"; nw.id = "gate-new";
    nw.onclick = createGuest;
    gs.append(nw, el("p", "sf-fine gate-under", "Free and instant: play now. Your name, password and Hero Key wait in Manage account, to save when you like."));
    pane.append(gs);
  }

  // ---------------------------------------------------------------- in and out
  function skip() { then = null; why = ""; phase = "in"; setOpen(false); }
  // Not now, Esc, a click beside the card: closed; a PLAY that waited for it doesn't start
  function close() {
    then = null; why = ""; phase = "in"; here = false;
    const s = S();
    if (s && s.flow().stage === "waiting") { /* the browser is still open for it: the title bar shows the code (social.js) */ }
    setOpen(false);
  }
  function finish() {
    clearTimeout(welcomeT);
    phase = "in"; here = false; why = "";
    const run = then; then = null;
    setOpen(false);
    if (run) setTimeout(run, 0);
  }
  function show(reason, next) {
    if (S() && S().signedIn() && S().me()) { if (next) next(); return; }
    phase = "in"; note = ""; keyWays = false;
    if (reason !== undefined) why = reason || "";
    if (next !== undefined) then = next;
    setOpen(true);
    if (S() && !S().ways()) S().loadWays();
    render();
    findKeyFile();   // at once, not in a few seconds: the "Log in as …" of a key file kept from before
    setTimeout(() => { const b = $("gate-again") || $("gate-browser"); if (b) b.focus(); }, 0);
  }
  function signedInNow() {
    const s = S();
    if (!open || phase === "making") return;
    if (!here) { finish(); return; }
    phase = "welcome"; render();
    clearTimeout(welcomeT);
    welcomeT = setTimeout(finish, 1600);
  }

  document.addEventListener("leyline:status", (ev) => {
    const d = ev.detail || {};
    if (d.known && !d.signedIn) findKeyFile();   // signed out at start: a key file just downloaded signs in by itself
  });
  document.addEventListener("leyline:me", (ev) => {
    const d = ev.detail || {};
    if (d.signedIn) { if (S().me()) signedInNow(); }
  });
  document.addEventListener("leyline:signin", () => { if (open && phase !== "welcome" && phase !== "making") render(); });

  // Esc closes (while the browser waits for its code, Esc cancels that first); a click beside the card closes
  document.addEventListener("keydown", (e) => {
    if (!open || e.key !== "Escape") return;
    const s = S();
    if (s && s.flow().stage === "waiting") s.cancelSignIn(); else close();
  });
  const g0 = $("gate");
  if (g0) g0.addEventListener("pointerdown", (e) => { if (e.target === g0 || e.target.id === "gate-body") close(); });
  // its own window buttons: the title bar's are under it
  const win = window.__TAURI__ && window.__TAURI__.window ? window.__TAURI__.window.getCurrentWindow() : null;
  const wb = (id, fn) => { const b = $(id); if (b) b.addEventListener("click", () => win && fn(win)); };
  wb("gate-min", (w) => w.minimize());
  wb("gate-max", (w) => w.toggleMaximize());
  wb("gate-close", (w) => w.close());

  // PLAY on the official realm: signed in first (or no way to tell): run now; else this dialog, then it
  const ready = () => { const s = S(); return !s || !s.known() || s.signedIn(); };
  function need(reason, next) {
    if (ready()) { next(); return true; }
    show(reason, next);
    return false;
  }

  window.LeylineGate = { isOpen: () => open, makePassword, ready, need, show, close, skip, dropPath, findKeyFile,
    dragOver: (on) => { if (over !== on) { over = on; const d = $("gate-drop"); if (d) d.classList.toggle("over", on); } } };
  // never left over the launcher: closed at the start, whatever the page said
  open = true; setOpen(false);
  const s0 = S();
  if (s0 && s0.checked() && s0.known() && !s0.signedIn()) findKeyFile();
})();
