// Leyline Launcher: the Forum page. Reads the website's forum (stream/forum.py) through the Rust core's forum commands
// (src-tauri/src/forum.rs): the categories, the latest threads, a category's threads, a thread, title search.
// Signed in on the launcher (social.js), New thread, Reply and Report happen right here (forum_post: the core sends the
// launcher's own sign-in token, no second sign-in; the forum checks the same rules as its website forms). Signed out,
// New thread and Reply open the forum's own pages in your browser.
//
// Loaded before app.js; app.js's navigation calls LeylineForum.load() when the page opens. Everything from the server is
// shown with textContent: a post is plain text, and its links are buttons that open only links the forum sent
// (forum.rs page_for "link").
(function () {
  "use strict";
  const tauri = window.__TAURI__ && window.__TAURI__.core ? window.__TAURI__.core.invoke : null;
  const invoke = tauri || (async () => { throw "This is a preview page: open it in the Leyline app."; });
  const $ = (id) => document.getElementById(id);
  const el = (tag, cls, text) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  };
  // never a raw error to the player (friendly.js): the error goes to the console
  const DOWN = "The forum isn't reachable right now. Try again, or ask on the Discord.";
  const oops = (e, fb) => (window.LeylineFriendly ? window.LeylineFriendly(e, fb) : fb);
  const call = (action, args) => invoke("forum_call", { action, args: args || {} });
  const send = (action, args) => invoke("forum_post", { action, args: args || {} });
  const S = () => window.LeylineSocial;
  const signedIn = () => !!(S() && S().signedIn() && S().me());
  const REASONS = [["spam", "Spam or advertising"], ["harassment", "Harassment"], ["hate", "Hate"],
    ["unsafe", "A harmful or scam link"], ["off-topic", "Off topic"], ["other", "Something else"]];
  const TITLE_MAX = 100, BODY_MAX = 5000;
  let busy = false;
  let after = "";   // a line to show once the next view has loaded ("Posted.")
  // what went wrong with a post, in plain words (the forum's own refusal is already plain)
  function postFailed(e) {
    const m = String(e);
    if (m === "signed_out") { if (S()) S().refresh(); return "You were signed out. Sign in again from Manage account."; }
    return oops(e, m || DOWN);
  }
  function asWho() {
    const me = S() && S().me();
    const p = el("p", "hint fm-as");
    if (!me) return p;
    p.append(document.createTextNode(`Posting as ${me.name}${me.tag ? "#" + me.tag : ""} · `));
    const ch = el("button", "link-s", "Change your name"); ch.type = "button";
    ch.onclick = () => { if (window.LeylineNav) window.LeylineNav.show("account"); };
    p.append(ch);
    return p;
  }
  function textBox(id, label, max, rows, value) {
    const l = el("label", "ac-label", label); l.htmlFor = id;
    const t = el("textarea"); t.id = id; t.maxLength = max; t.rows = rows; t.value = value || "";
    return [l, t];
  }
  const open = (kind, arg) => invoke("forum_open", { kind, arg: arg === undefined ? null : String(arg) })
    .then(() => say("Opened in your browser.", "ok")).catch((e) => say(oops(e, "Couldn't open your browser. Try again."), "bad"));

  function say(text, kind) { const s = $("fm-status"); if (s) { s.textContent = text || ""; s.className = "status" + (kind ? " " + kind : ""); } }

  function ago(t) {
    const d = Math.max(0, Math.floor(Date.now() / 1000) - (t || 0));
    if (d < 60) return "just now";
    for (const [n, w] of [[86400 * 30, "month"], [86400 * 7, "week"], [86400, "day"], [3600, "hour"], [60, "minute"]]) {
      if (d >= n) { const k = Math.floor(d / n); return `${k} ${w}${k === 1 ? "" : "s"} ago`; }
    }
    return "just now";
  }
  const replies = (n) => `${n || 0} repl${n === 1 ? "y" : "ies"}`;

  let cats = [];
  let view = { kind: "home" };
  let seq = 0;

  function catName(id) { const c = cats.find((x) => x.id === id); return c ? c.name : "Forum"; }

  function loading(box) { box.replaceChildren(el("p", "hint fm-loading", "Loading…")); }
  function failed(box, e) {
    box.replaceChildren(el("p", "fm-empty", oops(e, DOWN)));
    const again = el("button", "btn small", "Try again"); again.type = "button"; again.onclick = () => render();
    box.append(again);
  }

  function threadRow(t, showCat) {
    const b = el("button", "fm-row"); b.type = "button";
    const left = el("span", "fm-row-t");
    const title = el("b");
    if (t.pinned) title.append(el("span", "fm-tag", "Pinned"));
    if (t.locked) title.append(el("span", "fm-tag lock", "Locked"));
    title.append(document.createTextNode(String(t.title || "")));
    left.append(title, el("small", "", `by ${t.author || "Someone"}${showCat ? " in " + catName(t.cat) : ""} · ${ago(t.created)}`));
    b.append(left, el("span", "fm-row-n", `${replies(t.replies)} · active ${ago(t.bumped)}`));
    b.onclick = () => go({ kind: "thread", id: t.id, cat: t.cat });
    return b;
  }
  function list(threads, showCat, empty) {
    if (!threads.length) return el("p", "fm-empty", empty);
    const ul = el("ul", "fm-list");
    for (const t of threads) { const li = el("li"); li.append(threadRow(t, showCat)); ul.append(li); }
    return ul;
  }
  function box(title, ...kids) {
    const s = el("section", "fm-box");
    if (title) s.append(el("h3", "", title));
    s.append(...kids);
    return s;
  }
  function crumb(...parts) {
    const n = el("nav", "fm-crumb"); n.setAttribute("aria-label", "Where you are");
    parts.forEach(([label, to], i) => {
      if (i) n.append(document.createTextNode(" / "));
      if (to) { const b = el("button", "link-s", label); b.type = "button"; b.onclick = () => go(to); n.append(b); }
      else n.append(el("span", "", label));
    });
    return n;
  }
  // a post: plain text, its links as buttons (forum.rs opens only links the forum sent)
  function postText(p) {
    const d = el("p", "fm-text");
    for (const part of p.parts || [{ t: p.text || "" }]) {
      if (part.u) {
        const a = el("button", "fm-link", part.u.length > 70 ? part.u.slice(0, 60) + "…" + part.u.slice(-8) : part.u);
        a.type = "button"; a.title = part.u;
        a.onclick = () => open("link", part.u);
        d.append(a);
      } else d.append(document.createTextNode(part.t || ""));
    }
    return d;
  }

  async function render() {
    const out = $("fm-view");
    if (!out) return;
    const mine = ++seq;
    const stale = () => mine !== seq;
    loading(out);
    try {
      if (!cats.length || view.kind === "home") {
        const r = await call("categories");
        cats = Array.isArray(r && r.categories) ? r.categories : [];
      }
      if (stale()) return;
      if (view.kind === "home") {
        const r = await call("latest", { n: 10 });
        if (stale()) return;
        const cl = el("ul", "fm-list fm-cats");
        for (const c of cats) {
          const b = el("button", "fm-row"); b.type = "button";
          const t = el("span", "fm-row-t");
          t.append(el("b", "", c.name), el("small", "", c.line || ""));
          if (c.last) t.append(el("small", "fm-last", "Latest: " + c.last.title));
          b.append(t, el("span", "fm-row-n", `${c.threads} thread${c.threads === 1 ? "" : "s"}`));
          b.onclick = () => go({ kind: "cat", cat: c.id, page: 1 });
          const li = el("li"); li.append(b); cl.append(li);
        }
        const two = el("div", "fm-two");
        two.append(box("Categories", cl), box("Latest threads", list(r.threads || [], true, "No threads yet. Start the first one.")));
        out.replaceChildren(two);
      } else if (view.kind === "cat") {
        const r = await call("threads", { cat: view.cat, page: view.page || 1 });
        if (stale()) return;
        const c = cats.find((x) => x.id === view.cat) || {};
        const head = el("div", "fm-head");
        const t = el("div"); t.append(crumb(["Forum", { kind: "home" }]), el("h3", "fm-title", c.name || r.name || "Forum"), el("p", "hint", c.line || ""));
        const nb = el("button", "btn small", c.staff_only ? "The team posts here" : "New thread here"); nb.type = "button";
        nb.disabled = !!c.staff_only;
        nb.onclick = () => newThread(view.cat);
        head.append(t, nb);
        const pager = el("div", "fm-pager");
        if ((view.page || 1) > 1) { const p = el("button", "link-s", "‹ Newer"); p.type = "button"; p.onclick = () => go({ ...view, page: view.page - 1 }); pager.append(p); }
        if (r.more) { const n = el("button", "link-s", "Older ›"); n.type = "button"; n.onclick = () => go({ ...view, page: (view.page || 1) + 1 }); pager.append(n); }
        out.replaceChildren(head, box("", list(r.threads || [], false, "No threads here yet. Start the first one.")), pager);
      } else if (view.kind === "thread") {
        const r = await call("thread", { id: view.id, page: view.page || 1 });
        if (stale()) return;
        const th = r.thread || {};
        const head = el("div", "fm-head");
        const t = el("div");
        const title = el("h3", "fm-title");
        if (th.pinned) title.append(el("span", "fm-tag", "Pinned"));
        if (th.locked) title.append(el("span", "fm-tag lock", "Locked"));
        title.append(document.createTextNode(String(th.title || "")));
        t.append(crumb(["Forum", { kind: "home" }], [catName(th.cat), { kind: "cat", cat: th.cat, page: 1 }]), title);
        const acts = el("div", "fm-acts");
        const rp = el("button", "btn small primary", th.locked ? "Locked" : "Reply"); rp.type = "button";
        rp.disabled = !!th.locked;
        rp.title = signedIn() ? "Write your reply below" : "Opens the reply page on the website, in your browser";
        rp.onclick = () => {
          const box = $("fm-reply-b");
          if (box) { box.focus(); box.scrollIntoView({ block: "center" }); return; }
          if (signedIn() && r.more) { go({ ...view, page: r.pages || view.page }); return; }
          open("reply", th.id);
        };
        const web = el("button", "btn small", "On the website"); web.type = "button";
        web.onclick = () => open("thread", th.id);
        acts.append(rp, web);
        head.append(t, acts);
        const posts = el("div", "fm-posts");
        const me = signedIn() ? S().me() : null;
        for (const p of r.posts || []) {
          const a = el("article", "fm-post");
          if (p.hidden) { a.append(el("p", "fm-gone", "This post was removed by a moderator.")); posts.append(a); continue; }
          const by = el("p", "fm-by");
          const av = S().avatar(String(p.author || ""), "fm-av");   // the portrait from the name (avatars.js), never a letter
          by.append(av, el("b", "", String(p.author || "Someone")));
          if (p.tag) by.append(el("small", "", "#" + p.tag));
          if (p.staff) by.append(el("span", "fm-team", "Team"));
          by.append(el("span", "fm-when", ago(p.at)));
          if (me && !(p.author === me.name && (p.tag || "") === (me.tag || ""))) {
            const rb = el("button", "link-s fm-rep", "Report"); rb.type = "button";
            rb.onclick = () => reportForm(a, p);
            by.append(rb);
          }
          a.append(by, postText(p));
          posts.append(a);
        }
        const pager = el("div", "fm-pager");
        if ((view.page || 1) > 1) { const p = el("button", "link-s", "‹ Earlier"); p.type = "button"; p.onclick = () => go({ ...view, page: view.page - 1 }); pager.append(p); }
        if (r.more) { const n = el("button", "link-s", "Later ›"); n.type = "button"; n.onclick = () => go({ ...view, page: (view.page || 1) + 1 }); pager.append(n); }
        out.replaceChildren(head, box("", posts), pager);
        if (!r.more && !th.locked && signedIn()) out.append(replyForm(th));
        else if (th.locked) out.append(el("p", "hint", "This thread is locked, so no new replies."));
      } else if (view.kind === "new") {
        out.replaceChildren(newForm());
      } else if (view.kind === "search") {
        const r = await call("search", { q: view.q });
        if (stale()) return;
        out.replaceChildren(crumb(["Forum", { kind: "home" }], [`Titles with “${view.q}”`]),
          box("", list(r.threads || [], true, "No thread titles match.")));
      }
      say(after, after ? "ok" : "");
      after = "";
    } catch (e) {
      if (!stale()) failed(out, e);
    }
  }

  function go(v) { view = v; render(); const m = document.querySelector("main"); if (m) m.scrollTop = 0; }

  // ---------------------------------------------------------------- posting (signed in on the launcher)
  function newThread(cat) {
    if (!signedIn()) { open("new", cat); return; }
    go({ kind: "new", cat: cat || "" });
  }

  function formStatus() { const st = el("p", "status"); st.setAttribute("role", "status"); return st; }
  function setStatus(st, text, kind) { st.textContent = text || ""; st.className = "status" + (kind ? " " + kind : ""); }

  function newForm() {
    const f = el("form", "fm-box fm-form ac-form"); f.id = "fm-new-form"; f.noValidate = true;
    const inner = el("div", "fm-form-in");
    inner.append(crumb(["Forum", { kind: "home" }], ["New thread"]), el("h3", "fm-title", "New thread"),
      el("p", "hint", "Search first: the answer may be in a thread already."), asWho());
    const cl = el("label", "ac-label", "Category"); cl.htmlFor = "fm-new-c";
    const sel = el("select"); sel.id = "fm-new-c";
    sel.append(Object.assign(el("option", "", "Pick one"), { value: "" }));
    for (const c of cats) if (!c.staff_only) sel.append(Object.assign(el("option", "", c.name), { value: c.id, selected: c.id === view.cat }));
    const tl = el("label", "ac-label", "Title"); tl.htmlFor = "fm-new-t";
    const title = el("input"); title.type = "text"; title.id = "fm-new-t"; title.maxLength = TITLE_MAX; title.autocomplete = "off";
    const [bl, body] = textBox("fm-new-b", "Post", BODY_MAX, 8);
    const acts = el("div", "fm-acts");
    const sub = el("button", "btn primary small", "Post thread"); sub.type = "submit"; sub.id = "fm-new-go";
    const cancel = el("button", "btn small", "Cancel"); cancel.type = "button"; cancel.onclick = () => go(view.cat ? { kind: "cat", cat: view.cat, page: 1 } : { kind: "home" });
    acts.append(sub, cancel);
    const st = formStatus(); st.id = "fm-new-st";
    inner.append(cl, sel, tl, title, bl, body, el("p", "hint", "Plain text. Links work; screenshots go in as links. Sharing a realm? Put its invite link here."), acts, st);
    f.append(inner);
    f.onsubmit = async (ev) => {
      ev.preventDefault();
      if (busy) return;
      if (!sel.value) { setStatus(st, "Pick a category.", "bad"); sel.focus(); return; }
      if (title.value.trim().length < 4) { setStatus(st, "Give the thread a title of at least 4 characters.", "bad"); title.focus(); return; }
      if (!body.value.trim()) { setStatus(st, "Write something first.", "bad"); body.focus(); return; }
      busy = true; sub.disabled = true; setStatus(st, "Posting…");
      try {
        const out = await send("new", { cat: sel.value, title: title.value, body: body.value });
        busy = false;
        const th = (out && out.thread) || {};
        after = "Posted.";
        go({ kind: "thread", id: th.id, cat: th.cat });
      } catch (e) {
        busy = false; sub.disabled = false; setStatus(st, postFailed(e), "bad");
      }
    };
    setTimeout(() => (view.cat ? title : sel).focus(), 0);
    return f;
  }

  function replyForm(th) {
    const f = el("form", "fm-box fm-form ac-form"); f.id = "fm-reply-form"; f.noValidate = true;
    const inner = el("div", "fm-form-in");
    const [bl, body] = textBox("fm-reply-b", "Your reply", BODY_MAX, 4);
    const acts = el("div", "fm-acts");
    const sub = el("button", "btn primary small", "Post reply"); sub.type = "submit"; sub.id = "fm-reply-go";
    acts.append(sub);
    const st = formStatus(); st.id = "fm-reply-st";
    inner.append(bl, body, asWho(), acts, st);
    f.append(inner);
    f.onsubmit = async (ev) => {
      ev.preventDefault();
      if (busy) return;
      if (!body.value.trim()) { setStatus(st, "Write something first.", "bad"); body.focus(); return; }
      busy = true; sub.disabled = true; setStatus(st, "Posting…");
      try {
        const out = await send("reply", { thread: th.id, body: body.value });
        busy = false;
        after = "Reply posted.";
        go({ kind: "thread", id: (out && out.thread) || th.id, cat: th.cat, page: (out && out.page) || 1 });
      } catch (e) {
        busy = false; sub.disabled = false; setStatus(st, postFailed(e), "bad");
      }
    };
    return f;
  }

  function reportForm(article, p) {
    const old = article.querySelector(".fm-report");
    if (old) { old.remove(); return; }
    const f = el("form", "fm-report ac-form"); f.noValidate = true;
    const rl = el("label", "ac-label", "Why are you reporting this post?"); rl.htmlFor = "fm-rr-" + p.id;
    const sel = el("select"); sel.id = "fm-rr-" + p.id;
    sel.append(Object.assign(el("option", "", "Pick a reason"), { value: "" }));
    for (const [k, v] of REASONS) sel.append(Object.assign(el("option", "", v), { value: k }));
    const nl = el("label", "ac-label", "Anything the moderators should know (optional)"); nl.htmlFor = "fm-rn-" + p.id;
    const note = el("input"); note.type = "text"; note.id = "fm-rn-" + p.id; note.maxLength = 300; note.autocomplete = "off";
    const acts = el("div", "fm-acts");
    const sub = el("button", "btn small", "Send the report"); sub.type = "submit";
    const cancel = el("button", "link-s", "Cancel"); cancel.type = "button"; cancel.onclick = () => f.remove();
    acts.append(sub, cancel);
    const st = formStatus();
    f.append(rl, sel, nl, note, el("p", "hint", "The moderators see the post and your reason, not your name in public."), acts, st);
    f.onsubmit = async (ev) => {
      ev.preventDefault();
      if (busy) return;
      if (!sel.value) { setStatus(st, "Pick a reason.", "bad"); sel.focus(); return; }
      busy = true; sub.disabled = true; setStatus(st, "Sending…");
      try {
        const out = await send("report", { post: p.id, reason: sel.value, note: note.value });
        busy = false;
        f.replaceChildren(el("p", "status ok", String((out && out.text) || "Thanks. The moderators will look at it.")));
      } catch (e) {
        busy = false; sub.disabled = false; setStatus(st, postFailed(e), "bad");
      }
    };
    article.append(f);
    sel.focus();
  }

  // the page's line under the forum, and the New thread button's tooltip, follow the sign-in
  function signLine() {
    const fine = document.querySelector("#page-forum .fm-fine");
    if (fine) fine.textContent = signedIn() ? "You post and reply right here, signed in on this launcher. Plain text and links only."
      : "Reading here; sign in on Home or Manage account to post from the launcher. Plain text and links only.";
    const nb = $("fm-new");
    if (nb) nb.title = signedIn() ? "Start a thread here" : "Opens the website in your browser";
  }
  document.addEventListener("leyline:me", () => {
    signLine();
    const onForum = window.LeylineNav && window.LeylineNav.current && window.LeylineNav.current().page === "forum";
    if (onForum && !busy && view.kind === "thread") render();
  });

  // the page's own controls (index.html #page-forum)
  const form = $("fm-search");
  if (form) form.addEventListener("submit", (ev) => {
    ev.preventDefault();
    const q = $("fm-q").value.trim();
    if (q.length >= 2) go({ kind: "search", q: q.slice(0, 60) }); else say("Type at least two letters.", "warn");
  });
  if ($("fm-new")) $("fm-new").addEventListener("click", () => newThread(view.kind === "cat" ? view.cat : undefined));
  if ($("fm-discord")) $("fm-discord").addEventListener("click", () => invoke("open_link", { name: "discord" }).catch((e) => say(oops(e, "Couldn't open your browser. Try again."), "bad")));

  window.LeylineForum = {
    load: () => { signLine(); render(); },
    home: () => go({ kind: "home" }),
    openThread: (id) => {
      view = { kind: "thread", id };
      if (window.LeylineNav) window.LeylineNav.show("forum", true);
      render();
    },
    view: () => view,
  };
})();
