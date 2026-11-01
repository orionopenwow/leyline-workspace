// Leyline Launcher: List your mod. Any signed-in player pastes their repo link; the website (stream/listings.py) pins
// it to one exact download, checks it with the same rules as the launcher, and says exactly what one click installs.
// Safe mods go live at once as "Community · not reviewed"; native code, client patches, textures and big packs wait for
// a moderator. Through the Rust core's listing commands (src-tauri/src/listing.rs) with the launcher's own sign-in.
//
// Also: the tag on a listed mod's card (decorate) and its Report button (reportButton), which app.js calls.
// Loaded before app.js. Everything from the server is shown with textContent.
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
  const oops = (e, fb) => (window.LeylineFriendly ? window.LeylineFriendly(e, fb) : fb);
  const S = () => window.LeylineSocial;
  const signedIn = () => !!(S() && S().signedIn && S().signedIn());
  const REASONS = [["broken", "It doesn't work"], ["malware", "Harmful or a virus"], ["stolen", "Not theirs / stolen work"],
    ["gamedata", "Ships game files"], ["cheat", "A cheat or exploit"], ["rude", "Rude or hateful"], ["spam", "Spam"],
    ["other", "Something else"]];
  // Addon or mod (docs/WORDS.md): Addon first, then Mod with its categories in the Mods tab's order (shelf.js GROUPS);
  // the website's forms group them the same way (stream/hub.py kind_options).
  const CATS = [["addons", "Addons and UI"], ["graphics", "Shaders and graphics"], ["sandbox", "Sandbox and game modes"],
    ["realm-scripts", "Realm scripts"], ["audio", "Sounds and music"], ["tools", "Tools"], ["models", "Models and props"],
    ["maps", "Custom maps"]];
  const ADDON_LABEL = "Addon: an in-game interface helper", MOD_LABEL = "Mod: changes the game itself";
  function modOrder() {
    const G = window.LeylineShelf ? window.LeylineShelf.GROUPS : [];
    const order = G.flatMap((g) => g[2]);
    return CATS.filter(([id]) => id !== "addons").sort((a, b) => (order.indexOf(a[0]) + 99) % 99 - (order.indexOf(b[0]) + 99) % 99);
  }
  const DEFAULT_CAT = { addon: "addons", package: "addons", patch: "graphics", native: "tools", "realm-script": "realm-scripts", "game-mode": "sandbox", den: "sandbox" };
  const failed = (e, fb) => {
    const m = String(e);
    if (m === "signed_out") { if (S() && S().refresh) S().refresh(); return "You were signed out. Sign in again from Manage account."; }
    return oops(e, m || fb);
  };

  // its few styles, in the theme's tokens (themes/themes.css), so style.css stays as it is
  const CSS = `.lm-dlg{width:min(560px,92vw)}.lm-dlg input,.lm-dlg select{width:100%;box-sizing:border-box;margin:0 0 10px}
.lm-dlg .lm-plan{margin:4px 0 10px 18px;padding:0;font:13px/1.45 var(--t-font);color:var(--t-text-2);list-style:disc}.lm-dlg .lm-plan li{margin:3px 0}
.lm-flag,.lm-bad,.lm-ok{border-left:3px solid var(--t-warn);padding:6px 10px;margin:6px 0;font-size:13px;color:var(--t-text-2);background:var(--t-surface-2)}
.lm-bad{border-left-color:var(--t-bad)}.lm-ok{border-left-color:var(--t-ok)}
.lm-mine{list-style:none;padding:0;margin:0 0 10px}.lm-mine li{padding:8px 0;border-top:var(--t-border-w,1px) solid var(--t-border)}
.lm-mine li .btn{margin-left:8px}.lm-tag.ok{color:var(--t-ok)}`;
  const style = el("style", "", CSS);
  style.id = "lm-style";
  document.head.append(style);

  // ------------------------------------------------------------------ the dialog
  let dlg = null;
  function dialog() {
    if (dlg) return dlg;
    dlg = el("dialog", "dlg lm-dlg");
    dlg.id = "lm-dlg";
    dlg.setAttribute("aria-labelledby", "lm-title");
    document.body.append(dlg);
    dlg.addEventListener("click", (ev) => { if (ev.target === dlg) dlg.close(); });
    return dlg;
  }
  function head(title) {
    const d = dialog();
    d.replaceChildren();
    const h = el("h3", "", title); h.id = "lm-title";
    d.append(h);
    return d;
  }
  function closeRow(extra) {
    const row = el("div", "row");
    (extra || []).forEach((b) => row.append(b));
    const c = el("button", "btn", "Close"); c.type = "button"; c.onclick = () => dlg.close();
    row.append(c);
    return row;
  }
  function status() { const s = el("p", "status lm-status"); s.setAttribute("role", "status"); return s; }

  let listKind = "mod";   // "addon" when it was opened from an Addons tab ("List your addon"); "den" from Your Den (den.js)
  function open(kind) {
    listKind = kind === "addon" || kind === "den" ? kind : "mod";
    const den = listKind === "den";
    const d = head(listKind === "addon" ? "List your addon" : den ? "Share your Den on the Workshop" : "List your mod");
    if (!signedIn()) {
      d.append(el("p", "", "Sign in to list a mod: any player can. Paste your repo link and Leyline shows exactly what one click will install."));
      const acc = el("button", "btn primary", "Sign in"); acc.type = "button";
      acc.onclick = () => { dlg.close(); if (window.LeylineNav) window.LeylineNav.show("account"); };
      const web = el("button", "btn", "Open on the website"); web.type = "button";
      web.onclick = () => invoke("listing_open").catch(() => {});
      d.append(closeRow([acc, web]));
      d.showModal();
      return;
    }
    d.append(el("p", "", den
      ? "Paste the link to the GitHub release that has your Den file (.den.json) attached. We check the file once, like a Den opens it; anyone can then add it from the Workshop."
      : "Paste a link to your mod on GitHub, GitLab or Codeberg: the repo, a branch, a release, or one release file (.zip or .MPQ). We check it once; players install it from your repo."));
    const l = el("label", "ac-label", den ? "Your Den's release link" : "Your mod's link"); l.htmlFor = "lm-url";
    const u = el("input"); u.id = "lm-url"; u.type = "url"; u.maxLength = 400; u.spellcheck = false;
    u.placeholder = den ? "https://github.com/you/your-den/releases/tag/v1" : "https://github.com/you/your-mod";
    const go = el("button", "btn primary", "Check it"); go.type = "button";
    const st = status();
    const out = el("div", "lm-out");
    go.onclick = async () => {
      const url = u.value.trim();
      if (!/^https:\/\//.test(url)) { st.textContent = "Paste a link that starts with https://"; return; }
      go.disabled = true; st.textContent = "Checking… (Leyline reads the download once)"; out.replaceChildren();
      try {
        const r = await invoke("listing_post", { action: "check", args: { url } });
        st.textContent = "";
        preview(out, r.check, url);
      } catch (e) { st.textContent = failed(e, "The check didn't work. Try again."); }
      go.disabled = false;
    };
    u.addEventListener("keydown", (ev) => { if (ev.key === "Enter") go.click(); });
    const mineBtn = el("button", "btn", "Your listings"); mineBtn.type = "button"; mineBtn.onclick = () => mine();
    d.append(l, u, closeRow([go, mineBtn]), st, out);
    d.showModal();
    u.focus();
  }

  function preview(out, c, url) {
    out.replaceChildren();
    const title = el("p", "");
    title.append(el("b", "", c.name || c.repo || ""), document.createTextNode(` · ${c.label || ""}${c.version ? " · v" + c.version : ""}${c.licence ? " · " + c.licence : ""}`));
    out.append(title);
    (c.problems || []).forEach((p) => out.append(el("p", "lm-bad", p)));
    if (!c.ok) { out.append(el("p", "small", "Fix this in your repo, then check again.")); return; }
    out.append(el("p", "", "What one click does:"));
    const ul = el("ul", "lm-plan");
    (c.plan || []).forEach((x) => ul.append(el("li", "", x)));
    out.append(ul);
    (c.flags || []).forEach((f) => out.append(el("p", "lm-flag", f)));
    (c.notes || []).forEach((n) => out.append(el("p", "small", n)));
    const cl = el("label", "ac-label", "Addon or mod"); cl.htmlFor = "lm-cat";
    const cat = el("select"); cat.id = "lm-cat";
    const want = listKind === "addon" ? "addons" : DEFAULT_CAT[c.format] || "addons";
    const group = (label, list) => {
      const g = el("optgroup"); g.label = label;
      list.forEach(([id, name]) => { const o = el("option", "", name); o.value = id; if (id === want) o.selected = true; g.append(o); });
      cat.append(g);
    };
    group(ADDON_LABEL, CATS.filter(([id]) => id === "addons"));
    group(MOD_LABEL, modOrder());
    const dl = el("label", "ac-label", "One-line description (10 to 140 characters)"); dl.htmlFor = "lm-desc";
    const desc = el("input"); desc.id = "lm-desc"; desc.maxLength = 140;
    const st = status();
    const list = el("button", "btn primary", (c.flags || []).length ? "List it (a moderator looks first)" : "List it"); list.type = "button";
    list.onclick = async () => {
      if (desc.value.trim().length < 10) { st.textContent = "Write what it does in at least 10 characters."; return; }
      list.disabled = true; st.textContent = "Listing…";
      try {
        const r = await invoke("listing_post", { action: "submit", args: { t: c.token, category: cat.value, description: desc.value.trim() } });
        out.replaceChildren(el("p", r.state === "live" ? "lm-ok" : "lm-flag", r.state === "live"
          ? "Listed. It shows as Community · not reviewed, and players can install it in one click."
          : "Thanks: a moderator looks at it first (" + (r.why || []).join("; ") + "). It goes live as soon as they say yes."));
        if (typeof window.LeylineRefreshCatalog === "function") window.LeylineRefreshCatalog();
      } catch (e) { st.textContent = failed(e, "Listing didn't work. Try again."); list.disabled = false; }
    };
    out.append(cl, cat, dl, desc, list, st);
  }

  async function mine() {
    const d = head("Your listings");
    const st = status();
    const ul = el("ul", "lm-mine");
    d.append(ul, st, closeRow([]));
    if (!d.open) d.showModal();
    st.textContent = "Loading…";
    try {
      const r = await invoke("listing_mine");
      st.textContent = r.listings && r.listings.length ? "" : "You haven't listed a mod yet.";
      (r.listings || []).forEach((m) => {
        const li = el("li");
        li.append(el("b", "", m.name), document.createTextNode(` · ${m.state}${m.update ? " · an update waits for a moderator" : ""}`));
        if (m.why && m.why.length) li.append(el("p", "small", m.why.join("; ")));
        const rm = el("button", "btn small", "Remove"); rm.type = "button";
        rm.onclick = async () => {
          if (!window.confirm(`Remove ${m.name} from the list? Players who installed it keep it.`)) return;
          try { await invoke("listing_post", { action: "remove", args: { id: m.id } }); li.remove(); }
          catch (e) { st.textContent = failed(e, "Couldn't remove it. Try again."); }
        };
        li.append(rm);
        ul.append(li);
      });
    } catch (e) { st.textContent = failed(e, "Couldn't load your listings. Try again."); }
  }

  // ------------------------------------------------------------------ the card bits (app.js projectCard)
  function decorate(p, thumb, meta) {
    const reviewed = p.state === "reviewed";
    const t = el("span", "src lm-tag" + (reviewed ? " ok" : ""), reviewed ? "Reviewed" : "Community · not reviewed");
    t.title = reviewed ? "A moderator looked at it. It installs from its author's repo."
      : "Listed by a player and checked automatically; no person has looked yet. It installs from its author's repo.";
    meta.append(t);
    if (p.native) { const n = el("span", "dc-tag warn", "Native code"); n.title = "Leyline asks you first: native code runs with full access to your PC."; thumb.append(n); }
  }

  function reportButton(p) {
    const b = el("button", "btn small quiet", "Report"); b.type = "button";
    b.title = `Report ${p.name} to the moderators`;
    b.addEventListener("click", (ev) => { ev.stopPropagation(); report(p); });
    return b;
  }

  function report(p) {
    const d = head(`Report ${p.name}`);
    if (!signedIn()) {
      d.append(el("p", "", "Sign in to report a mod."), closeRow([]));
      d.showModal();
      return;
    }
    const rl = el("label", "ac-label", "What's wrong"); rl.htmlFor = "lm-reason";
    const sel = el("select"); sel.id = "lm-reason";
    REASONS.forEach(([id, name]) => { const o = el("option", "", name); o.value = id; sel.append(o); });
    const nl = el("label", "ac-label", "Anything to add (optional)"); nl.htmlFor = "lm-note";
    const note = el("input"); note.id = "lm-note"; note.maxLength = 300;
    const st = status();
    const send = el("button", "btn primary", "Send report"); send.type = "button";
    send.onclick = async () => {
      send.disabled = true;
      try {
        const r = await invoke("listing_post", { action: "report", args: { id: p.id, reason: sel.value, note: note.value.trim() } });
        st.textContent = r.message || "Thanks: the moderators will take a look.";
      } catch (e) { st.textContent = failed(e, "Couldn't send the report. Try again."); send.disabled = false; }
    };
    d.append(rl, sel, nl, note, closeRow([send]), st);
    d.showModal();
  }

  // ------------------------------------------------------------------ the button on Mods & Addons › Installed
  function addButton() {
    if ($("mods-list-yours")) return;
    const find = document.querySelector('#pane-installed [data-page-go="discover"]');
    if (!find) return;
    const addon = document.getElementById("pane-installed")?.dataset.shelf === "addons";
    const b = el("button", "btn small", addon ? "List your addon" : "List your mod"); b.type = "button"; b.id = "mods-list-yours";
    b.title = "List your mod for one-click install: paste your repo link";
    b.addEventListener("click", () => open(document.getElementById("pane-installed")?.dataset.shelf === "addons" ? "addon" : "mod"));
    find.after(b);
  }
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", addButton); else addButton();

  window.LeylineListMod = { open, mine, decorate, reportButton, report };
})();
