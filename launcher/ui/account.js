// Leyline Launcher: Manage account (owner 2026-10-30: "Allow manage account within launcher, bio profile pic etc basic
// stuff, smooth out the login process"). A page of its own (#page-account), opened from the title bar's profile chip.
//
// Signed out: the sign-in in steps (1 Sign in, 2 Your profile, 3 Play), the same chooser as the website's /wrath/signin
// (Hero Key, Google, Discord: only the ways the website offers) and the waiting state while the browser is open; the
// sign-in itself is social.js's (LeylineSocial). The first time after signing in: step 2, the profile, with Skip.
// Signed in: your picture, name#tag and bio; edit the display name, the picture (your initial or one of the website's
// preset pictures) and the bio (a guest account too: the same rules, and a line when its name is still the one made for
// it, me.profile.name_generated); the ways to sign in (each opens the website: the Hero Key page, link Google or Discord,
// linked launchers); Sign out. Saving goes through the core's social_call "profile" (src-tauri/src/social.rs ACTIONS),
// which only sends name, avatar, bio and setup. Everything from the server is shown with textContent.
//
// Loaded after social.js, before app.js.
(function () {
  "use strict";
  const $ = (id) => document.getElementById(id);
  const el = (tag, cls, text) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  };
  const tauri = window.__TAURI__ && window.__TAURI__.core ? window.__TAURI__.core.invoke : null;
  const invoke = tauri || (async () => { throw "This is a preview page: open it in the Leyline app."; });
  const S = () => window.LeylineSocial;
  const BIO_MAX = 300;
  const NAME_RE = /^[A-Za-z0-9_.\-À-ɏ]{2,24}$/;
  const PRESET_RE = /^https:\/\/play\.openvanilla\.app\/wrath\/avatar\/p\/([a-z]{2,12})\.svg$/;

  let welcome = false, back = "play", draft = null, busy = false;
  // Your Hero Key (owner 31 Oct 19:15; stream/herokeys.py through the core's social_call): what the key is up to
  // (herokey_status), a key shown just now (a new account's from social.js, or a new one), the confirm before a new key
  let keyState = null, keyShown = null, keyAsk = false, keyBusy = false, keyNote = ["", ""], wantKeyFocus = false;
  const FILE = "Leyline Hero Key.txt";

  function steps(on) {
    const ol = el("ol", "ac-steps"); ol.setAttribute("aria-label", "Steps");
    ["Sign in", "Your profile", "Play"].forEach((n, i) => {
      const li = el("li", i + 1 === on ? "on" : i + 1 < on ? "done" : "");
      li.dataset.n = String(i + 1);
      if (i + 1 === on) li.setAttribute("aria-current", "step");
      li.append(el("span", "", n));
      ol.append(li);
    });
    return ol;
  }

  function status(text, kind) {
    const s = $("ac-status");
    if (!s) return;
    s.textContent = text || "";
    s.className = "status" + (kind ? " " + kind : "");
  }

  // ---------------------------------------------------------------- signed out: the sign-in
  function renderSignedOut(view) {
    const card = el("section", "card ac-card ac-signin");
    card.append(steps(1), el("h2", "", "Sign in"),
      el("p", "ac-lede", "One account for the launcher, the website and the forum: your name and picture, mod ratings, and your Hero Key on community realms."));
    const box = el("div", "ac-flow"); box.id = "ac-flow";
    S().buildFlow(box, true);
    card.append(box);
    view.append(card);
  }

  // ---------------------------------------------------------------- signed in: the profile
  function pictureGrid(me, extra, chosen) {
    const fs = el("fieldset", "ac-pics");
    fs.append(el("legend", "ac-label", "Picture"));
    const opts = [["", "Your initial", ""]];
    for (const a of (extra.avatars || [])) {
      const m = PRESET_RE.exec(String(a.url || ""));
      if (m && m[1] === a.id) opts.push([a.id, String(a.name || a.id), a.url]);
    }
    for (const [id, name, url] of opts) {
      const lab = el("label", "ac-pic"); lab.title = name;
      const r = el("input"); r.type = "radio"; r.name = "ac-avatar"; r.value = id; r.checked = id === chosen;
      r.setAttribute("aria-label", name);
      r.onchange = () => { draft.avatar = id; preview(me); };
      const av = id ? S().avatar(name, "ac-pic-av", url) : S().avatar(draft.name || me.name, "ac-pic-av");
      lab.append(r, av);
      fs.append(lab);
    }
    if (opts.length === 1) fs.append(el("p", "hint", "The pictures load from the website. They'll show when it's reachable."));
    return fs;
  }

  function preview(me) {
    const box = $("ac-me-av");
    if (!box) return;
    const extra = S().extra();
    const p = (extra.avatars || []).find((a) => a.id === draft.avatar);
    const url = draft.avatar ? (p ? p.url : "") : (draft.avatar === (me.profile && me.profile.avatar) ? (me.profile && me.profile.avatar_url) : "");
    box.replaceChildren(S().avatar(draft.name || me.name, "ac-big", draft.avatar === "" && me.profile && me.profile.avatar === "" ? me.profile.avatar_url : url));
  }

  function renderProfile(view, me) {
    const extra = S().extra();
    const prof = me.profile || { bio: "", avatar: "", setup: true };
    if (!draft) draft = { name: me.name, avatar: prof.avatar || "", bio: prof.bio || "" };
    const ways = S().ways() || {};

    if (!welcome) {
      const head = el("section", "card ac-card ac-head");
      const av = el("div", "ac-me-av"); av.id = "ac-me-av";
      const t = el("div", "ac-me-t");
      const n = el("p", "ac-me-n"); n.append(el("b", "", me.name), el("span", "", me.tag ? "#" + me.tag : ""));
      t.append(n, el("p", "hint", "Signed in on this launcher"));
      if (prof.bio) t.append(el("p", "ac-me-bio", prof.bio));
      head.append(av, t);
      view.append(head);
    }

    // the form
    // the first time, the form is step 2 itself: the steps and the welcome on top of it, all in one card
    const form = el("form", "card ac-card ac-form" + (welcome ? " ac-welcome" : "")); form.id = "ac-form"; form.noValidate = true;
    // TODO(owner): the welcome words
    if (welcome) form.append(steps(2), el("h2", "", `Welcome, ${me.name}`), el("p", "ac-lede", "Pick how others see you on the forum and in the launcher. You can change it any time."));
    else form.append(el("h3", "", "Edit your profile"));
    const nl = el("label", "ac-label", "Display name"); nl.htmlFor = "ac-name";
    const name = el("input"); name.id = "ac-name"; name.type = "text"; name.maxLength = 24; name.value = draft.name; name.autocomplete = "off"; name.spellcheck = false;
    name.setAttribute("aria-describedby", "ac-name-h");
    const nh = el("p", "hint", (prof.name_generated ? "Your name was made for you (a guest's or the default): pick your own. " : "")
      + `2 to 24 letters, digits, dots, dashes or underscores. Others see it with your tag${me.tag ? " (#" + me.tag + ")" : ""}.`); nh.id = "ac-name-h";
    name.oninput = () => { draft.name = name.value; };
    const bl = el("label", "ac-label", "Bio (optional)"); bl.htmlFor = "ac-bio";
    const bio = el("textarea"); bio.id = "ac-bio"; bio.maxLength = BIO_MAX; bio.rows = 3; bio.value = draft.bio;
    bio.placeholder = "A line about you, your realm or what you play.";
    const count = el("span", "ac-count", `${draft.bio.length}/${BIO_MAX}`); count.setAttribute("aria-live", "polite");
    bio.oninput = () => { draft.bio = bio.value; count.textContent = `${bio.value.length}/${BIO_MAX}`; };
    const acts = el("div", "ac-acts");
    const save = el("button", "btn primary", welcome ? "Save and continue" : "Save"); save.type = "submit"; save.id = "ac-save";
    acts.append(save);
    if (welcome) {
      const skip = el("button", "btn", "Skip for now"); skip.type = "button"; skip.id = "ac-skip";
      skip.onclick = () => submit(true);
      acts.append(skip);
    }
    const st = el("p", "status"); st.id = "ac-status"; st.setAttribute("role", "status");
    form.append(nl, name, nh, pictureGrid(me, extra, draft.avatar), bl, bio, count, acts, st);
    form.onsubmit = (e) => { e.preventDefault(); submit(false); };
    view.append(form);

    if (welcome) {
      if (keyShown || S().freshKey()) view.prepend(keyCard(me));   // a new account: its key first, then the profile
      preview(me); return;
    }

    view.append(keyCard(me));

    // the ways to sign in
    const wc = el("section", "card ac-card ac-ways");
    wc.append(el("h3", "", "Ways to sign in"),
      el("p", "hint", "Each opens the website in your browser. Linking another way lets it sign in to this same account."));
    const ul = el("ul", "ac-list");
    const row = (title, line, label, link) => {
      const li = el("li");
      const t = el("div"); t.append(el("b", "", title)); if (line) t.append(el("span", "", line));
      const b = el("button", "btn small", label); b.type = "button"; b.dataset.link = link;
      b.onclick = () => S().openLink(link);
      li.append(t, b); ul.append(li);
    };
    if (ways.google) row("Google", "", "Link Google", "linkgoogle");
    if (ways.discord) row("Discord", "", "Link Discord", "linkdiscord");
    row("Leyline launchers", "This one and any others signed in as you. Revoke one there.", "Linked launchers", "linkedlaunchers");
    wc.append(ul);
    view.append(wc);

    const so = el("section", "card ac-card ac-out");
    const sot = el("div"); sot.append(el("h3", "", "Sign out"), el("p", "hint", "Signs this launcher out and revokes its sign-in on the website."));
    const sob = el("button", "btn", "Sign out"); sob.type = "button"; sob.id = "ac-signout";
    sob.onclick = () => S().signOut();
    const web = el("button", "link-s", "Your profile on the website ›"); web.type = "button"; web.onclick = () => S().openLink("profile");
    so.append(sot, el("div", "ac-acts", ""), web);
    so.querySelector(".ac-acts").append(sob);
    view.append(so);
    preview(me);
  }

  async function submit(skip) {
    if (busy) return;
    const me = S().me();
    if (!me) return;
    const prof = me.profile || {};
    const args = { setup: true };
    if (!skip) {
      const n = (draft.name || "").trim();
      if (!NAME_RE.test(n)) { status("Names are 2 to 24 letters, digits, dots, dashes or underscores.", "bad"); $("ac-name").focus(); return; }
      if (n !== me.name) args.name = n;
      if (draft.avatar !== (prof.avatar || "")) args.avatar = draft.avatar;
      if (draft.bio !== (prof.bio || "")) args.bio = draft.bio;
    }
    busy = true;
    const btn = $("ac-save"); if (btn) btn.disabled = true;
    status("Saving…");
    try {
      const out = await invoke("social_call", { action: "profile", args });
      busy = false;
      S().setMe(out && out.me ? out.me : null);
      draft = null;
      if (welcome) {
        welcome = false;
        if (window.LeylineNav) window.LeylineNav.show(back && back !== "account" ? back : "play");
        return;
      }
      render();
      status("Saved.", "ok");
    } catch (e) {
      busy = false;
      if (btn) btn.disabled = false;
      const msg = String(e);
      status(msg === "signed_out" ? "You were signed out. Sign in again." : msg, "bad");
      if (msg === "signed_out") S().refresh();
    }
  }

  // ---------------------------------------------------------------- Your Hero Key
  function keyText(name, key) {
    const day = new Date().toLocaleDateString("en-GB", { day: "numeric", month: "long", year: "numeric" });
    return `Leyline Hero Key\r\n\r\nName: ${name}\r\nKey: ${key}\r\nMade: ${day}\r\n\r\n`
      + "This key signs you in to your Leyline account on Wrath Unleashed and on every realm that accepts Hero Key. "
      + "Keep it private, like a password.\r\n";
  }
  function keyStatus(text, kind) {
    keyNote = [text || "", kind || ""];
    const s = $("ac-key-status");
    if (s) { s.textContent = keyNote[0]; s.className = "status" + (keyNote[1] ? " " + keyNote[1] : ""); }
  }
  async function keySaved() {
    try { await invoke("social_call", { action: "herokey_saved", args: {} }); } catch (_) { /* the key is saved on their side anyway */ }
    if (keyState) keyState.saved = true;
  }

  // the key, hidden until Show; Save as text file (the native save dialog) and Copy
  function reveal(key, name) {
    const wrap = el("div", "ac-reveal");
    const box = el("div", "ac-keybox");
    const code = el("code", "ac-keytext dots"); code.id = "ac-keytext"; code.setAttribute("aria-label", "Your Hero Key");
    const show = el("button", "btn small", "Show"); show.type = "button"; show.id = "ac-key-show";
    show.setAttribute("aria-pressed", "false"); show.setAttribute("aria-controls", "ac-keytext");
    const draw = (on) => { code.textContent = on ? key : key.slice(0, 5) + "\u2022".repeat(24); code.classList.toggle("dots", !on);
      show.textContent = on ? "Hide" : "Show"; show.setAttribute("aria-pressed", String(on)); };
    draw(false);
    show.onclick = () => draw(code.classList.contains("dots"));
    box.append(code, show);
    const acts = el("div", "ac-acts");
    const save = el("button", "btn primary", "Save as text file"); save.type = "button"; save.id = "ac-key-save";
    save.onclick = async () => {
      try {
        const ok = await invoke("save_text_file", { name: FILE, text: keyText(name, key) });
        if (ok) { keyStatus(`Saved ${FILE}. Keep it somewhere safe.`, "ok"); keySaved(); }
      } catch (e) { keyStatus(String(e), "bad"); }
    };
    const copy = el("button", "btn", "Copy"); copy.type = "button"; copy.id = "ac-key-copy";
    copy.onclick = async () => {
      try { await navigator.clipboard.writeText(key); keyStatus("Copied. Keep it somewhere safe.", "ok"); keySaved(); }
      catch (_) { draw(true); keyStatus("Select the key and copy it.", "warn"); }
    };
    acts.append(save, copy);
    wrap.append(el("p", "hint", "Save it now: it is shown only this once. Anyone with it can sign in as you."), box, acts);
    return wrap;
  }

  function keyCard(me) {
    const c = el("section", "card ac-card ac-key"); c.id = "ac-key";
    c.append(el("h3", "", "Your Hero Key"));
    const name = me.tag ? `${me.name}#${me.tag}` : me.name;
    const fresh = keyShown || (S().freshKey() ? S().freshKey().key : "") || (keyState && keyState.key) || "";
    if (fresh) c.append(reveal(fresh, name));
    else if (!keyState) {
      c.append(el("p", "hint", "Checking your key…"));
      loadKey();
    } else if (keyState.web_only) {
      c.append(el("p", "hint", "This launcher signed in through your browser, so your key is looked after on the website."));
      const b = el("button", "btn", "Open Your Hero Key"); b.type = "button"; b.dataset.link = "herokey";
      b.onclick = () => S().openLink("herokey");
      const a = el("div", "ac-acts"); a.append(b); c.append(a);
    } else {
      c.append(el("p", "hint", keyState.has_key ? (keyState.saved ? "Your key is saved. It signs you in on any device, here and on the website."
        : "Your key was made but not saved, so it can't be shown again. Make a new one and save it.") : "You don't have a key yet."));
      const meta = el("p", "ac-key-meta");
      if (keyState.made) meta.append(el("span", "", "Made "), el("b", "", keyState.made));
      if (keyState.last_used) meta.append(el("span", "", " · Last used "), el("b", "", keyState.last_used));
      if (meta.childNodes.length) c.append(meta);
    }
    if (keyState && !keyState.web_only || fresh) {
      const a = el("div", "ac-acts ac-key-new");
      if (keyAsk) {
        a.append(el("p", "ac-ask", "Make a new key? Your old key stops working at once."));
        const yes = el("button", "btn small primary", "Make a new key"); yes.type = "button"; yes.id = "ac-key-yes";
        yes.disabled = keyBusy; yes.onclick = newKey;
        const no = el("button", "btn small", "Cancel"); no.type = "button"; no.onclick = () => { keyAsk = false; render(); };
        a.append(yes, no);
      } else {
        const nk = el("button", "btn small", "Make a new key"); nk.type = "button"; nk.id = "ac-key-new";
        nk.onclick = () => { keyAsk = true; render(); const y = $("ac-key-yes"); if (y) y.focus(); };
        a.append(nk);
      }
      c.append(a);
    }
    const st = el("p", "status" + (keyNote[1] ? " " + keyNote[1] : ""), keyNote[0]); st.id = "ac-key-status"; st.setAttribute("role", "status");
    c.append(st);
    if (!welcome) c.append(passwordBox(me));
    return c;
  }

  function passwordBox(me) {
    const has = !!(keyState && keyState.has_password);
    const d = el("details", "ac-pass"); d.id = "ac-pass";
    d.append(el("summary", "", has ? "Password: set" : "Password (optional)"));
    const f = el("form", "ac-pass-f"); f.noValidate = true;
    f.append(el("p", "hint", has ? "Change it, or remove it. Your Hero Key keeps working either way."
      : `Set one to sign in here with your name and #tag${me.tag ? ` (${me.name}#${me.tag})` : ""}. Your Hero Key keeps working.`));
    const l1 = el("label", "ac-label", has ? "New password" : "Password"); l1.htmlFor = "ac-pw1";
    const p1 = el("input"); p1.id = "ac-pw1"; p1.type = "password"; p1.autocomplete = "new-password"; p1.maxLength = 128;
    const l2 = el("label", "ac-label", "Type it again"); l2.htmlFor = "ac-pw2";
    const p2 = el("input"); p2.id = "ac-pw2"; p2.type = "password"; p2.autocomplete = "new-password"; p2.maxLength = 128;
    const acts = el("div", "ac-acts");
    const save = el("button", "btn small primary", "Save password"); save.type = "submit";
    acts.append(save);
    const st = el("p", "status"); st.id = "ac-pw-status"; st.setAttribute("role", "status");
    const note = (t, k) => { st.textContent = t; st.className = "status" + (k ? " " + k : ""); };
    if (has) {
      const rm = el("button", "btn small", "Remove password"); rm.type = "button"; rm.id = "ac-pw-remove";
      rm.onclick = async () => {
        try { await invoke("social_call", { action: "password", args: { remove: true } }); keyState.has_password = false; render(); }
        catch (e) { note(String(e), "bad"); }
      };
      acts.append(rm);
    }
    f.onsubmit = async (e) => {
      e.preventDefault();
      if (p1.value.length < 10) { note("Passwords are at least 10 characters.", "bad"); p1.focus(); return; }
      if (p1.value !== p2.value) { note("The two passwords are different.", "bad"); p2.focus(); return; }
      note("Saving…");
      try {
        await invoke("social_call", { action: "password", args: { password: p1.value } });
        p1.value = p2.value = "";
        if (keyState) keyState.has_password = true;
        note("Password saved. Sign in with your name, its #tag and this password.", "ok");
      } catch (err) { note(String(err) === "signed_out" ? "You were signed out. Sign in again." : String(err), "bad"); }
    };
    f.append(l1, p1, l2, p2, acts, st);
    d.append(f);
    return d;
  }

  async function loadKey() {
    if (keyBusy) return;
    keyBusy = true;
    try { keyState = await invoke("social_call", { action: "herokey_status", args: {} }) || {}; }
    catch (e) { keyState = { web_only: true }; if (String(e) === "signed_out") { keyBusy = false; S().refresh(); return; } }
    keyBusy = false;
    if (onPage()) render();
  }

  async function newKey() {
    if (keyBusy) return;
    keyBusy = true; keyStatus("Making your new key…");
    try {
      const out = await invoke("social_call", { action: "herokey_new", args: {} });
      keyShown = String(out && out.key || "");
      keyAsk = false; keyNote = ["Your new key is below. Your old key no longer works.", "ok"];
      if (keyState) keyState.saved = false;
    } catch (e) {
      keyAsk = false;
      const msg = String(e);
      keyNote = [msg.includes("website") ? msg : (msg === "signed_out" ? "You were signed out. Sign in again." : msg), "bad"];
      if (msg.includes("website")) keyState = Object.assign({}, keyState || {}, { web_only: true });
    }
    keyBusy = false;
    render();
  }

  function render() {
    const view = $("ac-view");
    if (!view) return;
    const s = S();
    if (!s) return;
    const me = s.me();
    view.textContent = "";
    view.classList.toggle("ac-two", !!(s.signedIn() && me && !welcome));
    if (!s.signedIn() || !me) { draft = null; renderSignedOut(view); return; }
    renderProfile(view, me);
    if (wantKeyFocus) { wantKeyFocus = false; const k = $("ac-key"); if (k) { k.scrollIntoView({ block: "nearest" }); const b = k.querySelector("button"); if (b) b.focus(); } }
  }

  const onPage = () => window.LeylineNav && window.LeylineNav.current().page === "account";
  document.addEventListener("leyline:page", (ev) => {
    const page = (ev.detail || {}).page;
    if (page !== "account") {
      back = page; welcome = false; draft = null;
      // leaving Manage account: a key shown just now is gone from memory (Make a new key if it was never saved)
      keyShown = null; keyAsk = false; keyNote = ["", ""]; keyState = null; if (S()) S().dropFreshKey();
      return;
    }
    if (!S().signedIn()) S().loadWays();
    render();
    if (S().signedIn() && !S().me()) S().refresh();
  });
  document.addEventListener("leyline:me", (ev) => {
    if (!(ev.detail || {}).signedIn) { keyState = null; keyShown = null; }
    if (onPage() && !busy) { draft = null; render(); }
  });
  document.addEventListener("leyline:focus-key", () => { wantKeyFocus = true; if (onPage()) render(); });
  document.addEventListener("leyline:welcome", () => { welcome = true; draft = null; if (onPage()) render(); });
  document.addEventListener("leyline:signin", () => {
    if (!onPage() || S().signedIn()) return;
    const box = $("ac-flow");
    if (box) S().buildFlow(box, true); else render();
  });

  window.LeylineAccount = { render, welcome: () => welcome };
})();
