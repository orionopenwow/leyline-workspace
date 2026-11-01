// Mods & Addons › Discover › Looks (owner 1 Nov 14:38: "make the graphical upgrades and changes a main part ... featuring
// different looks and 1 click install on them for most clients"): graphics packs, UI looks and graphics presets from the
// mods list's `source: "look"` entries (src-tauri/src/looks.rs, stream/looks.json). One click where the core says it can
// (a preset, a pinned pack, a UI look whose addons are reviewed); the author's page otherwise. "Back to the original look"
// takes every look out of that game and puts its settings back. The tab is themes.js's; themes.js hands the looks over
// (take) so they never show in the Mods or Addons lists. The website's "Install with Leyline" lands here (focus).
(() => {
  "use strict";
  const invoke = window.__TAURI__ && window.__TAURI__.core ? window.__TAURI__.core.invoke : null;
  const el = (tag, cls, text) => {
    const n = document.createElement(tag);
    if (cls) n.className = cls;
    if (text !== undefined && text !== null) n.textContent = text;
    return n;
  };
  const TYPES = { "graphics-pack": "Graphics pack", "ui-look": "UI look", preset: "Graphics preset" };
  const FILTERS = [["", "All looks"], ["graphics-pack", "Graphics packs"], ["ui-look", "UI looks"], ["preset", "Presets"]];
  let looks = [];        // the catalog's looks, in their order
  let client = "";       // the client's id
  let st = null;         // looks_state's answer for that client
  let type = "";
  let focusId = "";      // a look a leyline:// link asked for
  let busy = "";
  let note = ["", ""];
  let box = null;

  function friendly(e) {
    return window.LeylineFriendly && window.LeylineFriendly.say ? window.LeylineFriendly.say(e) : String(e);
  }
  function clients() {
    const s = document.getElementById("play-client");
    return s ? Array.from(s.options).filter((o) => o.value).map((o) => ({ id: o.value, name: o.textContent })) : [];
  }
  function size(n) {
    if (n === null || n === undefined) return "Size on the author's page";
    if (n === 0) return "Nothing to download";
    if (n < 1048576) return `${Math.max(1, Math.round(n / 1024))} KB`;
    if (n < 1073741824) return `${Math.round(n / 1048576)} MB`;
    return `${(n / 1073741824).toFixed(1)} GB`;
  }
  // Each look's own drawing, as on the website (stream/looks.py MOTIFS and motif(); the CSS is looks-art.css, a copy)
  const MOTIFS = {
    sky: ["m-sun", "m-far", "m-near"], pack: ["m-moon", "m-shore", "m-water"],
    ui: ["m-panel", "m-unit a", "m-unit b", "m-map", "m-bar"],
  };
  const PRESET_MOTIFS = ["ultra", "high", "balanced", "classic", "low"];
  function motif(p) {
    const lk = p.look || {};
    if (lk.type === "preset") return PRESET_MOTIFS.includes(lk.preset) ? lk.preset : "balanced";
    return lk.type === "graphics-pack" ? "pack" : "ui";
  }
  // Our own pictures, bundled (art/looks/, a copy of stream/www-files/art/looks/; looks.browser.test.cjs checks they match).
  // The catalog names them by their website address; only these files are ever shown.
  const PICS = ["look-balanced.webp", "look-classic.webp", "look-forever-ui-character.webp", "look-forever-ui-charselect.webp",
    "look-forever-ui-hud.webp", "look-forever-ui-professions.webp", "look-max.webp"];
  function pics(p) {
    const pv = (p.look && Array.isArray(p.look.previews)) ? p.look.previews : [];
    return pv.map((x) => Object.assign({}, x, { file: String(x.src || "").replace(/^\/wrath\/art\/looks\//, "") }))
      .filter((x) => PICS.includes(x.file)).map((x) => Object.assign(x, { url: "art/looks/" + x.file }));
  }
  function img(x, cls) {
    const i = el("img", cls || "");
    i.src = x.url; i.alt = x.alt || ""; i.width = x.w || 0; i.height = x.h || 0; i.loading = "lazy"; i.decoding = "async";
    if (x.w) i.style.maxWidth = x.w + "px";   // never wider than the picture itself
    return i;
  }
  // a larger view of one picture: Previous, Next, Close (Esc too)
  function viewer(p, list, at) {
    const old = document.getElementById("looks-view");
    if (old) old.remove();
    const v = el("div", "lk-view");
    v.id = "looks-view";
    v.setAttribute("role", "dialog");
    v.setAttribute("aria-modal", "true");
    v.setAttribute("aria-label", `${p.name}: picture ${at + 1} of ${list.length}`);
    const x = list[at];
    const fig = el("figure", "");
    fig.append(img(x), el("figcaption", "", `${x.alt} · ${x.credit} · ${at + 1} of ${list.length}`));
    const nav = el("div", "lk-view-nav");
    const go = (label, to) => { const b = el("button", "btn", label); b.type = "button"; b.addEventListener("click", to); nav.append(b); return b; };
    const close = () => { v.remove(); document.removeEventListener("keydown", key); };
    function key(ev) { if (ev.key === "Escape") close(); }
    if (list.length > 1) go("‹ Previous", () => { close(); viewer(p, list, (at + list.length - 1) % list.length); });
    const c = go("Close", close);
    if (list.length > 1) go("Next ›", () => { close(); viewer(p, list, (at + 1) % list.length); });
    v.addEventListener("click", (ev) => { if (ev.target === v) close(); });
    document.addEventListener("keydown", key);
    v.append(fig, nav);
    document.body.append(v);
    c.focus();
  }
  function art(p) {
    const m = motif(p);
    const a = el("span", "lk-art m-" + m);
    for (const c of MOTIFS[m] || MOTIFS.sky) a.append(el("i", c));
    return a;
  }
  const order = (p) => (p.look && Number.isFinite(p.look.order) ? p.look.order : 0);

  function take(projects) {
    looks = (projects || []).filter((p) => p && p.source === "look" && p.look).sort((a, b) => order(a) - order(b) || a.name.localeCompare(b.name));
    if (box && !box.hidden) render();
  }

  async function load() {
    const list = clients();
    if (!client || !list.some((c) => c.id === client)) {
      const sel = document.getElementById("play-client");
      client = (sel && sel.value) || (list[0] && list[0].id) || "";
    }
    st = null;
    if (client && invoke) {
      try { st = await invoke("looks_state", { client }); } catch (e) { note = [friendly(e), "bad"]; }
    }
    render();
  }

  function howOf(p) { return (st && st.how && st.how[p.id]) || (p.look.how === "launcher" ? "" : "link"); }
  const isOn = (p) => !!(st && Array.isArray(st.on) && st.on.includes(p.id));

  async function act(p, what) {
    if (!client) { note = ["Add your game first: drop your game folder on the realm box below.", "warn"]; render(); return; }
    busy = p.id;
    render();
    try {
      if (what === "apply") {
        const d = await invoke("looks_apply", { client, look: p.id });
        note = [d.said || `${p.name} is in.`, d.warnings && d.warnings.length ? "warn" : "ok"];
      } else {
        await invoke("looks_remove", { client, look: p.id });
        note = [`${p.name} is out.`, "ok"];
      }
    } catch (e) { note = [friendly(e), "bad"]; }
    busy = "";
    await load();
  }

  async function reset() {
    if (!client) return;
    busy = "reset";
    render();
    try { const r = await invoke("looks_reset", { client }); note = [r.said, "ok"]; }
    catch (e) { note = [friendly(e), "bad"]; }
    busy = "";
    await load();
  }

  async function openPage(p) {
    const url = p.page || p.repo || p.author_url;
    if (!url) return;
    try { await invoke("open_url", { url }); note = [`Opened ${p.name}'s page in your browser.`, "ok"]; }
    catch (e) { note = [friendly(e), "bad"]; }
    render();
  }

  function card(p, build) {
    const lk = p.look;
    const c = el("article", "lk-card");
    c.dataset.id = p.id;
    c.dataset.type = lk.type;
    if (p.id === focusId) c.classList.add("linked");
    const list = pics(p);
    const ph = el("div", "lk-ph " + lk.type + (list.length ? " pic" : ""));
    if (list.length) {
      ph.append(img(list[0]), el("span", "lk-ph-cap", list[0].credit || ""));
    } else {
      ph.setAttribute("aria-hidden", "true");
      ph.append(art(p));
    }
    c.append(ph);
    if (list.length > 1) {   // a strip of the look's pictures, each opening a larger view
      const strip = el("div", "lk-thumbs");
      strip.setAttribute("aria-label", `More pictures of ${p.name}`);
      list.forEach((x, i) => {
        const b = el("button", "lk-thumb");
        b.type = "button";
        b.setAttribute("aria-label", `${x.alt} (larger)`);
        b.append(img(x));
        b.addEventListener("click", () => viewer(p, list, i));
        strip.append(b);
      });
      c.append(strip);
    }
    const b = el("div", "lk-b");
    b.append(el("p", "lk-type", TYPES[lk.type] || "Look"), el("h3", "lk-name", p.name));
    const chips = el("ul", "lk-chips");
    chips.setAttribute("aria-label", "Works with");
    for (const v of p.builds || []) chips.append(el("li", v === build ? "on" : "", v));
    b.append(chips, el("p", "lk-desc", p.description));
    if (Array.isArray(lk.changes) && lk.changes.length) {
      const ch = el("p", "lk-ch");
      ch.append(el("b", "", "Changes: "), document.createTextNode(lk.changes.join(", ")));
      b.append(ch);
    }
    b.append(el("p", "lk-meta", size(p.size)), el("p", "lk-credit", p.credits || `${p.name} by ${p.author}`));
    if (lk.note) b.append(el("p", "lk-note", lk.note));
    const acts = el("div", "lk-acts");
    const how = howOf(p);
    const fits = !build || !(p.builds || []).length || p.builds.includes(build);
    const on = isOn(p);
    if (how !== "link" && how !== "") {
      const btn = el("button", "btn primary lk-go", how === "preset" ? (on ? "In use" : "Use") : on ? "Remove" : "Install");
      btn.type = "button";
      btn.disabled = !fits || !!busy || (how === "preset" && on);
      if (busy === p.id) btn.textContent = how === "preset" ? "Saving…" : on ? "Removing…" : "Installing…";
      btn.addEventListener("click", () => act(p, on && how !== "preset" ? "remove" : "apply"));
      acts.append(btn);
      if (!fits) acts.append(el("span", "lk-nofit", "Not for this game version"));
    }
    if (p.author !== "Leyline" && (p.page || p.repo)) {
      const a = el("button", how === "link" ? "btn lk-page" : "lk-link lk-page", "Author's page");
      a.type = "button";
      a.addEventListener("click", () => openPage(p));
      acts.append(a);
    }
    b.append(acts);
    c.append(b);
    return c;
  }

  function render() {
    if (!box) return;
    box.textContent = "";
    box.append(el("p", "lk-lede", "Make your game look new: graphics packs, new interfaces and graphics presets, with credit to the people who made them. One click where Leyline can check it; the author's page for the rest."));
    const list = clients();
    const head = el("div", "lk-head");
    if (list.length) {
      const pick = el("select", "gfx-client");
      pick.id = "looks-client";
      pick.setAttribute("aria-label", "Game client");
      for (const c of list) { const o = el("option", "", c.name); o.value = c.id; if (c.id === client) o.selected = true; pick.append(o); }
      pick.addEventListener("change", () => { client = pick.value; note = ["", ""]; load(); });
      head.append(el("span", "gfx-for", "Looks for"), pick);
    }
    const back = el("button", "btn lk-reset", busy === "reset" ? "Putting it back…" : "Back to the original look");
    back.type = "button";
    back.id = "looks-reset";
    back.title = "Takes out every look Leyline put in this game and puts its settings back as they were";
    back.disabled = !st || !st.changed || !!busy;
    back.addEventListener("click", reset);
    head.append(back);
    box.append(head);
    const fl = el("div", "ly-tabs lk-filter");
    fl.setAttribute("role", "tablist");
    fl.setAttribute("aria-label", "Kind of look");
    for (const [k, label] of FILTERS) {
      const b = el("button", "ly-tab", label);
      b.type = "button";
      b.dataset.type = k;
      b.setAttribute("role", "tab");
      b.setAttribute("aria-selected", String(k === type));
      b.addEventListener("click", () => { type = k; render(); });
      fl.append(b);
    }
    box.append(fl);
    const msg = el("p", "lk-note-line " + (note[1] || ""), note[0]);
    msg.id = "looks-note";
    msg.setAttribute("role", "status");
    box.append(msg);
    if (!list.length) box.append(el("p", "gfx-empty", "Add your game first to install a look: drop your game folder on the realm box below. You can still browse."));
    const build = st ? st.build : "";
    const shown = looks.filter((p) => !type || p.look.type === type);
    if (!looks.length) { box.append(el("p", "gfx-empty", "Loading the looks…")); return; }
    if (!shown.length) { box.append(el("p", "gfx-empty", "No looks of that kind yet.")); return; }
    const grid = el("div", "lk-grid");
    grid.id = "looks-grid";
    for (const p of shown) grid.append(card(p, build));
    box.append(grid);
    box.append(el("p", "gfx-more", "Packs from other sites are made and hosted by their authors. Your game's settings one by one are on the Graphics tab."));
    if (focusId) {
      const f = grid.querySelector(`.lk-card[data-id="${CSS.escape(focusId)}"]`);
      if (f) { f.scrollIntoView({ block: "center" }); const b = f.querySelector("button:not([disabled])"); if (b) b.focus(); }
      focusId = "";
    }
  }

  // a leyline://install/look:<slug> link from the website: this tab, that card, nothing installed until the click
  function focus(id) {
    const p = looks.find((x) => x.id === id);
    if (!p) return false;
    focusId = id;
    type = "";
    note = [`From the website: ${p.name}. ${p.look.type === "preset" ? "Press Use" : "Press Install"} to put it in your game.`, "ok"];
    if (window.LeylineThemes) window.LeylineThemes.showTab("looks");
    return true;
  }

  window.LeylineLooks = {
    take, focus,
    show: (b) => { box = b; return load(); },
    list: () => looks.slice(),
  };
})();
