// Leyline: the Passport (docs/leyline/passport.md; the core is app.rs passport_* over passport.rs). Your launcher's
// own key, grown up: a recovery phrase, your devices, guardians, and logging in to friends' worlds with no server.
//
// window.LeylinePassport.mount(el) draws the section into `el` (any element with data-leyline-passport gets it on
// load, like friends.js):
//   * first launch, "Make your Passport": a name, a picture if you like, Make my Passport, the 12 words, "I wrote them
//     down", Done (three clicks);
//   * Back up: show my phrase, save a backup file, guardians (2 to 5 friends: add, remove, save), my old Hero Key;
//   * Devices: the list, add a device by a 6-digit code (or a file when the PCs can't reach each other), remove one;
//   * Recover: with the phrase, from my other PC, or with 2 guardians; and helping a friend who named you a guardian;
//   * a warning when someone asks your guardians for your Passport, with Stop it.
// One main button per screen, plain words, the "i" bubble (info.js) explains. Text only (textContent); keys are never
// on the page, only the phrase on its own screen.
"use strict";
(() => {
  const call = window.__TAURI__ && window.__TAURI__.core
    ? window.__TAURI__.core.invoke
    : async () => { throw "This is a preview page: open it in the Leyline app."; };
  const oops = (e, fb) => (window.LeylineFriendly ? window.LeylineFriendly(e, fb) : (typeof e === "string" ? e : (fb || "Something went wrong. Try again.")));
  const missing = (e) => /not found|unknown (command )?/i.test(String(e || ""));
  const mk = (tag, cls, text) => { const e = document.createElement(tag); if (cls) e.className = cls; if (text !== undefined) e.textContent = text; return e; };
  const btn = (label, cls, on, id) => { const b = mk("button", cls || "btn", label); b.type = "button"; if (id) b.id = id; if (on) b.addEventListener("click", on); return b; };
  const day = (ts) => (ts ? new Date(ts * 1000).toLocaleDateString() : "");

  const INFO = { page: "single", title: "Your Passport", link: "hostfriends", more: "Playing with friends", lines: [
    "Your Passport is a key on your PC. Your friends and the worlds you play on know you by it. No website or server of ours is needed.",
    "Write down your recovery phrase: with it, any PC becomes you again. Guardians are 2 to 5 friends; any 2 of them can help you back in.",
    "Each world you join gets its own key made from your Passport, so two hosts can't follow you from world to world.",
  ] };

  function mount(el) {
    if (!el || el.dataset.passportMounted) return el && el.__passport;
    el.dataset.passportMounted = "1";
    el.classList.add("lp");
    if (!el.id) el.id = "lp";
    if (window.LeylineInfo && window.LeylineInfo.SECTIONS && !window.LeylineInfo.SECTIONS[el.id]) window.LeylineInfo.SECTIONS[el.id] = INFO;

    let st = null;          // passport_state
    let view = "home";      // home | words | backup | devices | recover | help
    let words = null;       // the phrase on screen (setup or Back up)
    let note = ["", ""];    // [text, kind]
    let busy = false;
    let pairTimer = null;
    let sub = {};           // per-screen bits (a code to type, an answer to show, ...)

    const say = (t, k) => { note = [t || "", k || ""]; };
    const go = (v, s) => { view = v; sub = s || {}; say(""); paint(); };

    async function run(fn, fb) {
      if (busy) return;
      busy = true;
      paint();
      try { await fn(); } catch (e) { if (missing(e)) { el.hidden = true; busy = false; return; } say(oops(e, fb), "bad"); }
      busy = false;
      paint();
    }
    async function load() {
      try { st = await call("passport_state"); } catch (e) { if (missing(e)) { el.hidden = true; return; } say(oops(e, "Your Passport didn't load. Try again."), "bad"); }
      paint();
    }

    function head() {
      const h = mk("div", "lp-head");
      const t = mk("h3", "lp-title", "Your Passport");
      const i = btn("i", "lp-i", () => { if (window.LeylineInfo) window.LeylineInfo.open(); });
      i.setAttribute("aria-label", "What is the Passport?");
      i.title = "What is the Passport?";
      h.append(t, i);
      return h;
    }
    function status() {
      const p = mk("p", "status lp-status" + (note[1] ? " " + note[1] : ""), note[0]);
      p.setAttribute("role", "status");
      return p;
    }
    function back() { return btn("Back", "btn small lp-back", () => go("home")); }
    function wordGrid(ws) {
      const ol = mk("ol", "lp-words");
      ol.setAttribute("aria-label", "Your recovery phrase");
      for (const w of ws) ol.append(mk("li", "", w));
      return ol;
    }

    // ---- first launch
    function setupScreen() {
      const box = mk("div", "lp-card lp-setup");
      box.append(mk("h4", "", "Make your Passport"),
        mk("p", "lp-lede", "Your Passport is how your friends and their worlds know you. It lives on this PC. Leyline keeps nothing."));
      const name = mk("input", "lp-name");
      name.id = "lp-name";
      name.placeholder = "Your name (friends see it)";
      name.maxLength = 32;
      name.value = (st && st.name) || "";
      const lab = mk("label", "lp-field");
      lab.append(mk("span", "", "Name"), name);
      box.append(lab);
      let picture = (st && st.picture) || "";
      if (window.LeylineAvatars) {
        const row = mk("div", "lp-pics");
        row.setAttribute("role", "radiogroup");
        row.setAttribute("aria-label", "A picture (you can skip this)");
        for (const [id, label] of window.LeylineAvatars.list().slice(0, 8)) {
          const b = btn("", "lp-pic" + (picture === id ? " on" : ""), () => { picture = picture === id ? "" : id; for (const x of row.children) x.classList.toggle("on", x.dataset.id === picture); });
          b.dataset.id = id;
          b.title = label;
          b.setAttribute("aria-label", label);
          const img = mk("img");
          img.alt = "";
          try { img.src = window.LeylineAvatars.src(id); } catch (_) { /* no art: the button still works */ }
          b.append(img);
          row.append(b);
        }
        box.append(mk("p", "lp-sub", "A picture, if you like:"), row);
      }
      const r = mk("div", "row");
      r.append(btn("Make my Passport", "btn primary", () => run(async () => {
        words = await call("passport_setup", { name: name.value.trim(), picture });
        view = "words";
      }, "Your Passport wasn't made. Try again."), "lp-make"),
      btn("I already have one", "btn lp-link", () => go("recover"), "lp-have"));
      box.append(r);
      return box;
    }

    function wordsScreen(setup) {
      const box = mk("div", "lp-card lp-phrase");
      box.append(mk("h4", "", "Your recovery phrase"),
        mk("p", "lp-lede", `Write these ${words.length} words on paper, in this order. With them, any PC becomes you again. Anyone who has them can be you, so never type them into a website or send them to anyone.`),
        wordGrid(words));
      if (setup) {
        const ok = mk("input");
        ok.type = "checkbox";
        ok.id = "lp-saved";
        const lab = mk("label", "lp-check");
        lab.append(ok, mk("span", "", "I wrote these words down"));
        const done = btn("Done", "btn primary", () => run(async () => {
          await call("passport_finish");
          words = null;
          await load();
          view = "home";
          say("Your Passport is ready.", "ok");
        }, "That didn't finish. Try again."), "lp-done");
        done.disabled = true;
        ok.addEventListener("change", () => { done.disabled = !ok.checked; });
        const r = mk("div", "row");
        r.append(done);
        box.append(lab, r);
      } else {
        const r = mk("div", "row");
        r.append(btn("Hide the words", "btn primary", () => { words = null; paint(); }, "lp-hide"));
        box.append(r);
      }
      return box;
    }

    // ---- home: who you are, and the three ways in
    function homeScreen() {
      const nodes = [];
      for (const a of (st.alerts || [])) {
        const c = mk("div", "lp-card lp-alert");
        c.setAttribute("role", "alert");
        c.append(mk("p", "", "Someone is asking your guardians for your Passport on another PC. If that isn't you, stop it now."));
        const r = mk("div", "row");
        r.append(btn("Stop it", "btn primary", () => run(async () => { say(await call("passport_veto", { id: a.id }), "ok"); await load(); }, "That didn't go through. Try again.")));
        c.append(r);
        nodes.push(c);
      }
      const me = mk("div", "lp-card lp-me");
      me.append(mk("p", "lp-who", `${st.name || "You"} · Hero ${st.short}`),
        mk("p", "lp-sub", st.phrase_waiting ? "Your recovery phrase isn't written down yet." : `${(st.devices || []).length} device${(st.devices || []).length === 1 ? "" : "s"}, ${(st.guardians || []).length ? (st.guardians.length + " guardians") : "no guardians yet"}.`));
      nodes.push(me);
      const r = mk("div", "row lp-nav");
      r.append(btn("Back up", "btn primary", () => go("backup"), "lp-go-backup"),
        btn("Devices", "btn", () => go("devices"), "lp-go-devices"),
        btn("Recover", "btn", () => go("recover"), "lp-go-recover"));
      if ((st.guarding || []).length) r.append(btn("Help a friend", "btn", () => go("help"), "lp-go-help"));
      nodes.push(r);
      return nodes;
    }

    // ---- Back up
    function backupScreen() {
      if (words) return [back(), wordsScreen(false)];
      const nodes = [back()];
      const c = mk("div", "lp-card");
      c.append(mk("h4", "", "Back up"),
        mk("p", "lp-lede", st.phrase_words === 12 ? "Your recovery phrase is 12 words." : "Your recovery phrase is 24 words (your key was made before the Passport; the words bring back the same key)."));
      const r = mk("div", "row");
      r.append(btn("Show my phrase", "btn primary", () => run(async () => { words = await call("passport_phrase"); }, "The words didn't load. Try again."), "lp-show"),
        btn("Save a backup file…", "btn", () => run(async () => { const m = await call("peer_key_export"); if (m) say(m, "ok"); }, "The file wasn't saved. Try again."), "lp-file"));
      c.append(r);
      nodes.push(c);
      nodes.push(guardiansCard());
      const h = mk("div", "lp-card");
      h.append(mk("h4", "", "Your old Hero Key"));
      if (st.herokey) {
        h.append(mk("p", "lp-sub", `Brought in (${st.herokey.id}). You can delete the old file.`));
      } else {
        h.append(mk("p", "lp-sub", "Had a Hero Key file from the old website? Bring it in once, so anything tied to it comes along. Leyline keeps only a fingerprint of it, never the key. No file? You don't need one."));
        const rr = mk("div", "row");
        rr.append(btn("Bring in my Hero Key file…", "btn", () => run(async () => { const m = await call("passport_herokey_import"); if (m) { say(m, "ok"); await load(); } }, "That file wasn't brought in."), "lp-herokey"));
        h.append(rr);
      }
      nodes.push(h);
      return nodes;
    }

    function guardiansCard() {
      const c = mk("div", "lp-card lp-guardians");
      c.append(mk("h4", "", "Guardians"),
        mk("p", "lp-sub", "Pick 2 to 5 friends. If you lose every PC and your words, any 2 of them can help you back in. No one of them can do it alone, and you get 3 days to stop it."));
      if (!sub.pick) sub.pick = (st.guardians || []).map((g) => g.peer_id);
      const friends = st.friends || [];
      const ul = mk("ul", "lp-list");
      for (const pid of sub.pick) {
        const f = friends.find((x) => x.peer_id === pid) || (st.guardians || []).find((x) => x.peer_id === pid) || { name: `Hero ${pid.slice(0, 4).toUpperCase()}` };
        const li = mk("li", "lp-item");
        li.append(mk("span", "lp-name", f.name), btn("Remove", "btn small", () => { sub.pick = sub.pick.filter((x) => x !== pid); paint(); }));
        ul.append(li);
      }
      if (!sub.pick.length) ul.append(mk("li", "lp-sub", "No guardians yet."));
      c.append(ul);
      const left = friends.filter((f) => !sub.pick.includes(f.peer_id));
      if (sub.pick.length < 5 && left.length) {
        const sel = mk("select", "lp-add");
        sel.id = "lp-add-guardian";
        sel.setAttribute("aria-label", "Add a guardian");
        sel.append(new Option("Add a friend…", ""));
        for (const f of left) sel.append(new Option(f.name, f.peer_id));
        sel.addEventListener("change", () => { if (sel.value) { sub.pick.push(sel.value); paint(); } });
        c.append(sel);
      } else if (!friends.length) {
        c.append(mk("p", "lp-sub", "Add friends first (your friends list is just below)."));
      }
      const r = mk("div", "row");
      const save = btn("Save guardians", "btn", () => run(async () => { say(await call("passport_guardians", { peerIds: sub.pick }), "ok"); await load(); sub.pick = null; }, "Your guardians weren't saved. Try again."), "lp-save-guardians");
      save.disabled = sub.pick.length < 2 || sub.pick.length > 5;
      r.append(save);
      c.append(r);
      return c;
    }

    // ---- Devices
    function devicesScreen() {
      const nodes = [back()];
      const c = mk("div", "lp-card");
      c.append(mk("h4", "", "Your devices"));
      const ul = mk("ul", "lp-list");
      for (const d of (st.devices || [])) {
        const li = mk("li", "lp-item lp-device");
        li.append(mk("span", "lp-name", d.name + (d.this ? " (this PC)" : "")), mk("span", "lp-sub", `added ${day(d.created)}`));
        if (!d.this) {
          if (sub.asking === d.id) {
            li.append(btn("Remove it", "btn small danger", () => run(async () => { say(await call("passport_device_revoke", { id: d.id }), "ok"); sub.asking = null; await load(); }, "It wasn't removed. Try again.")),
              btn("Keep", "btn small", () => { sub.asking = null; paint(); }));
          } else {
            li.append(btn("Remove", "btn small", () => { sub.asking = d.id; paint(); }));
          }
        }
        ul.append(li);
      }
      c.append(ul);
      nodes.push(c);
      const a = mk("div", "lp-card lp-pair");
      a.append(mk("h4", "", "Add a device"));
      if (sub.code !== undefined) {
        a.append(mk("p", "lp-lede", `Found ${sub.name}. Type the 6-digit code it shows.`));
        const inp = mk("input", "lp-code-in");
        inp.id = "lp-code";
        inp.inputMode = "numeric";
        inp.maxLength = 7;
        inp.placeholder = "123456";
        const r = mk("div", "row");
        r.append(inp, btn("Send my Passport", "btn primary", () => run(async () => { say(await call("passport_pair_confirm", { code: inp.value }), "ok"); delete sub.code; await load(); }, "It didn't go over. Start again on both PCs."), "lp-send"));
        a.append(r);
      } else if (sub.six) {
        a.append(mk("p", "lp-lede", "Saved. Take the file to your other PC, and type these 6 words there. They're shown only now."), wordGrid(sub.six.split(" ")));
        a.append(btn("Done", "btn primary", () => { delete sub.six; paint(); }));
      } else {
        a.append(mk("p", "lp-lede", "On the new PC, open Passport, pick Recover, then From my other PC. Then press Find it here. Both PCs need to be on the same network."));
        const addr = mk("input", "lp-addr");
        addr.id = "lp-addr";
        addr.placeholder = "Its address, if it isn't found (optional)";
        const r = mk("div", "row");
        r.append(btn("Find it", "btn primary", () => run(async () => { const n = await call("passport_pair_find", { addr: addr.value.trim() || null }); sub.name = n; sub.code = ""; }, "It wasn't found."), "lp-find"), addr);
        a.append(r, btn("No network between them? Use a file", "btn lp-link", () => run(async () => { const six = await call("passport_pair_file_export"); if (six) sub.six = six; }, "The file wasn't saved."), "lp-pair-file"));
      }
      nodes.push(a);
      return nodes;
    }

    // ---- Recover
    function stopPoll() { if (pairTimer) { clearInterval(pairTimer); pairTimer = null; } }
    function pollPair() {
      stopPoll();
      pairTimer = setInterval(async () => {
        let s;
        try { s = await call("passport_pair_status"); } catch (_) { return; }
        if (s.state === "show" && sub.show !== s.code) { sub.show = s.code; paint(); }
        if (s.state === "done" || s.state === "failed" || s.state === "off") { stopPoll(); sub.waiting = false; say(s.message || "", s.state === "done" ? "ok" : "bad"); paint(); }
      }, 1000);
    }

    function recoverScreen() {
      const nodes = [back()];
      if (st.recovering || sub.mode === "guardians") return nodes.concat(guardiansRecover());
      const c = mk("div", "lp-card");
      c.append(mk("h4", "", "With your recovery phrase"));
      const ta = mk("textarea", "lp-phrase-in");
      ta.id = "lp-phrase";
      ta.rows = 3;
      ta.placeholder = "Your 12 or 24 words";
      ta.setAttribute("autocomplete", "off");
      ta.spellcheck = false;
      const r = mk("div", "row");
      r.append(btn("Bring back my Passport", "btn primary", () => run(async () => { say(await call("passport_restore", { phrase: ta.value }), "ok"); ta.value = ""; }, "That phrase didn't work."), "lp-restore"));
      c.append(ta, r);
      nodes.push(c);
      const p = mk("div", "lp-card");
      p.append(mk("h4", "", "From your other PC"));
      if (sub.waiting) {
        p.append(mk("p", "lp-lede", sub.show ? "Type this code on your other PC:" : "Waiting for your other PC. There: Passport, Devices, Find it."));
        if (sub.show) p.append(mk("p", "lp-bigcode", sub.show));
        p.append(btn("Stop", "btn", () => { stopPoll(); sub.waiting = false; call("passport_pair_cancel").catch(() => {}); paint(); }));
      } else if (sub.file) {
        const six = mk("input", "lp-six");
        six.id = "lp-six";
        six.placeholder = "The 6 words from your other PC";
        const rr = mk("div", "row");
        rr.append(six, btn("Pick the file…", "btn primary", () => run(async () => { const m = await call("passport_pair_file_import", { words: six.value }); if (m) say(m, "ok"); }, "That file didn't open."), "lp-file-in"));
        p.append(rr);
      } else {
        p.append(mk("p", "lp-sub", "This PC becomes your Passport too. Your other PC shows nothing secret on screen."));
        const rr = mk("div", "row");
        rr.append(btn("Add this PC", "btn", () => run(async () => { await call("passport_pair_wait"); sub.waiting = true; sub.show = ""; pollPair(); }, "That didn't start."), "lp-wait"),
          btn("Use a file instead", "btn lp-link", () => { sub.file = true; paint(); }));
        p.append(rr);
      }
      nodes.push(p);
      const g = mk("div", "lp-card");
      g.append(mk("h4", "", "With 2 guardians"), mk("p", "lp-sub", "Lost every PC and your words? Two of the friends you picked as guardians can help."));
      g.append(btn("Ask 2 guardians", "btn", () => run(async () => { sub.mode = "guardians"; sub.code = await call("passport_recover_start"); await load(); }, "That didn't start."), "lp-ask"));
      nodes.push(g);
      return nodes;
    }

    function guardiansRecover() {
      const c = mk("div", "lp-card lp-recover");
      const code = (st.recovering && st.recovering.code) || sub.code || "";
      const got = (st.recovering && st.recovering.approvals) || 0;
      c.append(mk("h4", "", "Ask 2 guardians"),
        mk("p", "lp-lede", "Call or meet two of your guardians. Give each this code. In their Passport they pick Help a friend and send you back an answer."),
        mk("p", "lp-code", code),
        mk("p", "lp-sub", `${got} of 2 answers so far.`));
      const ta = mk("textarea", "lp-answer");
      ta.id = "lp-answer";
      ta.rows = 3;
      ta.placeholder = "Paste a guardian's answer (it starts with LG1-)";
      const r = mk("div", "row");
      r.append(btn("Add the answer", "btn primary", () => run(async () => {
        const res = await call("passport_recover_add", { text: ta.value });
        if (res && res.done) {
          words = await call("passport_phrase");
          view = "words";
          say(`You're back. ${res.told} friend${res.told === 1 ? "" : "s"} heard now; friends see the change after 3 days. Write down your new words.`, "ok");
        }
        await load();
      }, "That answer didn't work."), "lp-add-answer"),
      btn("Stop", "btn", () => { call("passport_recover_cancel").catch(() => {}); sub = {}; load(); }));
      c.append(ta, r);
      return [c];
    }

    // ---- Help a friend (I am their guardian)
    function helpScreen() {
      const c = mk("div", "lp-card lp-help");
      c.append(mk("h4", "", "Help a friend get back in"),
        mk("p", "lp-lede", "Only do this when you've spoken to them and you're sure it's them. Paste the code their new PC shows."));
      const sel = mk("select");
      sel.id = "lp-help-who";
      for (const g of (st.guarding || [])) sel.append(new Option(g.name, g.peer_id));
      const ta = mk("input", "lp-help-code");
      ta.id = "lp-help-code";
      ta.placeholder = "Their code (it starts with LR1-)";
      c.append(sel, ta);
      if (sub.answer) {
        c.append(mk("p", "lp-sub", "Send them this answer:"), mk("p", "lp-code", sub.answer));
        c.append(btn("Copy", "btn primary", () => { try { navigator.clipboard.writeText(sub.answer); say("Copied.", "ok"); } catch (_) { /* select it by hand */ } paint(); }));
      } else {
        c.append(btn("Make my answer", "btn primary", () => run(async () => { sub.answer = await call("passport_help_friend", { code: ta.value, peerId: sel.value }); }, "That didn't work."), "lp-help-go"));
      }
      return [back(), c];
    }

    function paint() {
      const nodes = [head()];
      if (!st) nodes.push(mk("p", "lp-sub", "Loading your Passport…"));
      else if (view === "words" && words) nodes.push(wordsScreen(true));
      else if (!st.setup_done && view === "home") nodes.push(setupScreen());
      else if (view === "backup") nodes.push(...backupScreen());
      else if (view === "devices") nodes.push(...devicesScreen());
      else if (view === "recover") nodes.push(...recoverScreen());
      else if (view === "help") nodes.push(...helpScreen());
      else nodes.push(...homeScreen());
      nodes.push(status());
      el.replaceChildren(...nodes);
      el.classList.toggle("lp-busy", busy);
      for (const b of el.querySelectorAll("button")) if (busy) b.disabled = true;
    }

    paint();
    load();
    el.__passport = { reload: load };
    return el.__passport;
  }

  window.LeylinePassport = { mount };
  const auto = () => document.querySelectorAll("[data-leyline-passport]").forEach((e) => mount(e));
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", auto);
  else auto();
})();
