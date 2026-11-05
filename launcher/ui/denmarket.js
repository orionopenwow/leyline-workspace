// Mods & Addons › Dens (docs/DEN-MARKETPLACE.md): the Dens and stamps players shared, and your own to share. The tab is
// called "Dens" (not "Worlds"): it is what the side menu's Your Den makes, and "worlds" reads like realms.
//
// Browse: cards (picture, name, maker, size, props, tags, likes, opened), search, Dens | Stamps, a tag and a sort.
// A card opens its detail: the pictures, the maker's credit and the remix line ("based on X by Y"), the licence,
// "Made with Leyline"; Open in my Den (dens_open: downloaded, checked like Add from a link, into Saved Dens, opened
// when you're in your Den), Add as a stamp (dens_stamp: Den Lite's shared stamps; each piece keeps the maker's
// credit), Like, Report. Share (from here or Your Den › Saved Dens › Share): which saved Den, a title, words, tags,
// a credit line and a licence; pictures the game takes by itself (dens_showcase: Den Lite poses the camera, the Den bar
// presses Screenshot) or the player's own; then it waits for a Leyline moderator. Your shared Dens: their state,
// Update, Unpublish.
//
// The core (src-tauri/src/denmarket.rs) checks every parameter and never takes a URL or a path from here (screenshot
// paths it gave are checked again). Loaded after app.js and den.js; app.js calls load() for the tab and focus(id) for a
// leyline://den/<id> link; den.js calls share(name).
"use strict";
(() => {
  const invoke = window.__TAURI__ && window.__TAURI__.core
    ? window.__TAURI__.core.invoke
    : async () => { throw "This is a preview page: open it in the Leyline app."; };
  const oops = (e, fb) => (window.LeylineFriendly ? window.LeylineFriendly(e, fb) : (fb || "Something went wrong. Try again."));
  const mk = (tag, cls, text) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  };
  const btn = (label, cls, on) => { const b = mk("button", cls || "btn small", label); b.type = "button"; if (on) b.addEventListener("click", on); return b; };
  const root = document.getElementById("dm");
  if (!root) return;
  // Pictures come from the website itself (the launcher's CSP allows its images); anything else is not shown.
  const SITE = "https://leylinelauncher.com";
  const picUrl = (p) => (typeof p === "string" && /^\/wrath\/dens\/pic\/[a-km-z2-9]{10}\/\d\.(jpg|png)$/.test(p) ? SITE + p : "");
  const KIND = { den: "Den", stamp: "Stamp" };
  const SORTS = [["new", "New"], ["likes", "Most liked"], ["downloads", "Most opened"]];
  const ID_RE = /^[a-km-z2-9]{10}$/;
  const PIC_MAX = 900 * 1024;

  const state = { q: "", kind: "", tag: "", sort: "new", page: 1 };
  let data = null;        // dens_browse
  let detail = null;      // dens_detail's build
  let view = "browse";    // browse | detail | share | mine
  let busy = false;
  let line = ["", ""];    // the status line: [text, kind]
  let shareFor = null;    // the share form: { name, id (an update), shots: [], picked: [], showcase }
  let mine = null;        // dens_mine's builds
  let dens = [];          // Saved Dens (den_state)
  let timer = null;

  const say = (text, kind) => { line = [text || "", kind || ""]; const s = root.querySelector(".dm-status"); if (s) { s.textContent = line[0]; s.dataset.kind = line[1]; } };
  const words = (e, fb) => {
    if (e === "signed_out") return "Log in first: Log in is at the top of the launcher.";
    if (e === "not_open") return "Shared Dens open with the grand release. Keep building: you can share yours then.";
    return oops(e, fb);
  };
  const kb = (n) => `${Math.max(1, Math.round((n || 0) / 1024))} KB`;
  const plural = (n, w) => `${n} ${w}${n === 1 ? "" : "s"}`;

  // ---------------------------------------------------------------- loading
  async function load() {
    if (view === "browse") await browse();
    else paint();
  }
  async function browse() {
    busy = true; paint();
    try {
      const q = { sort: state.sort, page: state.page };
      if (state.q) q.q = state.q;
      if (state.kind) q.kind = state.kind;
      if (state.tag) q.tag = state.tag;
      data = await invoke("dens_browse", { query: q });
      if (line[1] === "bad") say("");
    } catch (e) { data = data || null; say(words(e, "Couldn't load the shared Dens."), e === "not_open" ? "warn" : "bad"); }
    busy = false;
    paint();
  }
  async function open(id) {
    if (!ID_RE.test(id || "")) return;
    view = "detail"; detail = null; busy = true; paint();
    try { detail = await invoke("dens_detail", { id }); say(""); }
    catch (e) { say(words(e, "That build isn't shared any more."), "bad"); view = "browse"; }
    busy = false;
    paint();
  }

  // ---------------------------------------------------------------- painting
  function paint() {
    root.textContent = "";
    root.append(head());
    const st = mk("p", "status dm-status", line[0]);
    st.setAttribute("role", "status");
    st.dataset.kind = line[1];
    root.append(st);
    if (view === "detail") root.append(detailView());
    else if (view === "share") root.append(shareView());
    else if (view === "mine") root.append(mineView());
    else root.append(browseView());
  }

  function head() {
    const bar = mk("div", "toolbar dm-bar");
    if (view !== "browse") {
      bar.append(btn("‹ All shared Dens", "btn small dm-back", () => { view = "browse"; stopShots(); browse(); }));
    } else {
      const q = mk("input", "");
      q.type = "search"; q.id = "dm-q"; q.placeholder = "Search Dens and stamps"; q.maxLength = 80; q.value = state.q;
      q.setAttribute("aria-label", "Search Dens and stamps"); q.autocomplete = "off"; q.spellcheck = false;
      let t = null;
      q.addEventListener("input", () => { clearTimeout(t); t = setTimeout(() => { state.q = q.value.trim(); state.page = 1; browse(); }, 350); });
      bar.append(q);
    }
    bar.append(mk("span", "toolbar-fill"));
    bar.append(btn("Your shared Dens", "btn small" + (view === "mine" ? " on" : ""), () => showMine()),
      btn("Share a Den", "btn small primary", () => share("")));
    return bar;
  }

  function chips() {
    const box = mk("div", "chips dm-chips");
    box.setAttribute("role", "toolbar"); box.setAttribute("aria-label", "Show");
    for (const [k, label] of [["", "All"], ["den", "Dens"], ["stamp", "Stamps"]]) {
      const b = btn(label, "chip-btn" + (state.kind === k ? " on" : ""), () => { state.kind = k; state.page = 1; browse(); });
      b.setAttribute("aria-pressed", state.kind === k ? "true" : "false");
      b.dataset.kind = k;
      box.append(b);
    }
    const tag = mk("select", "dm-tag-pick");
    tag.setAttribute("aria-label", "Tag");
    tag.append(new Option("Any tag", ""));
    for (const t of (data && data.tags) || []) tag.append(new Option(t.name, t.id, false, t.id === state.tag));
    tag.value = state.tag;
    tag.addEventListener("change", () => { state.tag = tag.value; state.page = 1; browse(); });
    const sort = mk("select", "dm-sort");
    sort.setAttribute("aria-label", "Sort");
    for (const [k, l] of SORTS) sort.append(new Option(l, k, false, k === state.sort));
    sort.addEventListener("change", () => { state.sort = sort.value; state.page = 1; browse(); });
    box.append(tag, sort);
    return box;
  }

  function card(b) {
    const c = mk("button", "dm-card");
    c.type = "button";
    c.dataset.id = b.id;
    const pic = mk("div", "dm-pic");
    const u = picUrl(b.picture);
    if (u) { const img = mk("img", ""); img.src = u; img.alt = ""; img.loading = "lazy"; img.addEventListener("error", () => img.remove()); pic.append(img); }
    pic.append(mk("span", "dm-pic-kind", KIND[b.kind] || "Den"));
    const t = mk("div", "dm-t");
    t.append(mk("b", "dm-title", b.title), mk("span", "dm-by", `by ${b.author}`),
      mk("span", "dm-nums", [`${b.props} props`, kb(b.bytes), plural(b.likes, "like"), `${b.downloads} opened`].join(" · ")));
    const tags = mk("div", "dm-tags");
    const names = Object.fromEntries(((data && data.all_tags) || []).map((x) => [x.id, x.name]));
    for (const tg of (b.tags || []).slice(0, 3)) tags.append(mk("span", "dm-tag", names[tg] || tg));
    if (b.featured) tags.prepend(mk("span", "dm-tag dm-feat-tag", "Featured"));
    if (b.based_on) tags.append(mk("span", "dm-tag", "Remix"));
    t.append(tags);
    c.append(pic, t);
    c.setAttribute("aria-label", `${b.title} by ${b.author}`);
    c.addEventListener("click", () => open(b.id));
    return c;
  }

  function browseView() {
    const box = mk("div", "dm-browse");
    box.append(chips());
    if (busy && !data) { box.append(mk("p", "hint", "Loading the shared Dens…")); return box; }
    if (!data) { box.append(mk("p", "hint", "Places players built in their Dens show here: open one in yours, or place it as a stamp.")); return box; }
    if ((data.featured || []).length) {
      const f = mk("section", "dm-featured");
      f.append(mk("h3", "", "Featured"));
      const g = mk("div", "dm-grid");
      g.id = "dm-featured";
      for (const b of data.featured) g.append(card(b));
      f.append(g);
      box.append(f);
    }
    const all = mk("section", "dm-all");
    all.append(mk("h3", "", state.q || state.tag ? `${plural(data.total || 0, "build")} found` : "All"));
    const g = mk("div", "dm-grid");
    g.id = "dm-grid";
    for (const b of data.items || []) g.append(card(b));
    all.append(g);
    if (!(data.items || []).length) all.append(mk("p", "hint", state.q || state.tag || state.kind ? "Nothing matches that." : "Nothing shared yet. Build something in Your Den and press Share a Den."));
    if (data.pages > 1) {
      const p = mk("div", "row dm-pages");
      if (data.page > 1) p.append(btn("‹ Newer", "btn small", () => { state.page = data.page - 1; browse(); }));
      p.append(mk("span", "hint", `Page ${data.page} of ${data.pages}`));
      if (data.page < data.pages) p.append(btn("More ›", "btn small", () => { state.page = data.page + 1; browse(); }));
      all.append(p);
    }
    box.append(all);
    const web = btn("Shared Dens on the website", "btn small", () => invoke("dens_page_open", { id: null }).catch(() => {}));
    box.append(mk("p", "hint dm-made", "Made with Leyline. Every build is checked by a Leyline moderator before it shows here. A Den file is only where each piece stands: the pieces come from your own game."), web);
    return box;
  }

  // ---------------------------------------------------------------- one build
  function detailView() {
    const box = mk("article", "dm-detail");
    if (!detail) { box.append(mk("p", "hint", busy ? "Loading…" : "")); return box; }
    const b = detail;
    const pics = (b.pictures || []).map(picUrl).filter(Boolean);
    const gal = mk("div", "dm-gallery");
    const big = mk("div", "dm-big");
    if (pics.length) {
      const img = mk("img", ""); img.src = pics[0]; img.alt = `${b.title}, picture 1`; big.append(img);
      const thumbs = mk("div", "dm-thumbs");
      pics.forEach((u, i) => {
        const t = btn("", "dm-thumb" + (i === 0 ? " on" : ""), () => {
          img.src = u; img.alt = `${b.title}, picture ${i + 1}`;
          thumbs.querySelectorAll(".dm-thumb").forEach((x, j) => x.classList.toggle("on", j === i));
        });
        const ti = mk("img", ""); ti.src = u; ti.alt = `Picture ${i + 1}`; t.append(ti);
        thumbs.append(t);
      });
      gal.append(big, thumbs);
    } else {
      big.append(mk("span", "hint", "No pictures yet."));
      gal.append(big);
    }
    const info = mk("div", "dm-info");
    info.append(mk("span", "dm-kind", (KIND[b.kind] || "Den") + (b.featured ? " · featured" : "")), mk("h3", "dm-name", b.title));
    const by = mk("p", "dm-credit");
    by.append(document.createTextNode("by "), mk("b", "", b.author));
    if (b.credit) by.append(document.createTextNode(` · ${b.credit}`));
    info.append(by);
    if ((b.lineage || []).length) {
      const l = mk("p", "dm-lineage");
      l.append(document.createTextNode("Based on "));
      b.lineage.forEach((x, i) => {
        if (i) l.append(document.createTextNode(", based on "));
        const a = x.live ? btn(x.title, "linkish", () => open(x.id)) : mk("span", "", x.title);
        l.append(a, document.createTextNode(` by ${x.by}`));
      });
      info.append(l);
    }
    for (const p of String(b.description || "").split(/\n\n+/).filter(Boolean)) info.append(mk("p", "dm-desc", p));
    const dl = mk("dl", "dm-facts");
    const fact = (k, v) => { dl.append(mk("dt", "", k), mk("dd", "", v)); };
    fact("Size", `${b.props} props · ${kb(b.bytes)}`);
    fact("Likes", String(b.likes));
    fact("Opened", `${b.downloads} times`);
    if (b.licence) fact("Licence", `${b.licence.name}: ${b.licence.line}`);
    if (b.remixes) fact("Remixes", String(b.remixes));
    info.append(dl);
    const acts = mk("div", "row dm-acts");
    const o = btn("Open in my Den", "btn small primary", async () => {
      o.disabled = true;
      try {
        const r = await invoke("dens_open", { id: b.id });
        say(`'${b.title}' is in your Saved Dens as '${r.name}': it opens when you're in your Den (a copy of what was there is kept as 'backup').`, "ok");
        paint();
        const go = btn("Go to Your Den", "btn small", () => { if (window.LeylineNav) window.LeylineNav.show("den"); if (window.LeylineDen && window.LeylineDen.go) window.LeylineDen.go(); });
        go.id = "dm-go-den";
        root.querySelector(".dm-acts").append(go);
        return;
      } catch (e) { say(words(e, "That Den couldn't be opened."), "bad"); }
      o.disabled = false;
    });
    o.id = "dm-open";
    acts.append(o);
    if (b.stampable) {
      const s = btn("Add as a stamp", "btn small", async () => {
        s.disabled = true;
        try {
          const r = await invoke("dens_stamp", { id: b.id });
          say(`'${b.title}' is one of your stamps: in your Den type .den stamp ${r.name}, or ask your companion to build ${r.name}. Every piece keeps ${b.author}'s credit.`, "ok");
        } catch (e) { say(words(e, "That stamp couldn't be added."), "bad"); }
        s.disabled = false;
      });
      s.id = "dm-stamp";
      acts.append(s);
    }
    if (!b.mine) {
      const lk = btn(b.liked ? "Liked ✓" : "Like", "btn small", async () => {
        lk.disabled = true;
        try { const r = await invoke("dens_post", { action: "like", args: { id: b.id, on: !b.liked } }); b.liked = r.liked; b.likes = r.likes; paint(); return; }
        catch (e) { say(words(e), "bad"); }
        lk.disabled = false;
      });
      lk.id = "dm-like";
      lk.setAttribute("aria-pressed", b.liked ? "true" : "false");
      acts.append(lk, reportBox(b));
    }
    if (b.remix_ok === false) info.append(mk("p", "hint", "Look only: open it and build in your own copy; its maker asked that remixes not be shared."));
    acts.append(btn("On the website", "btn small", () => invoke("dens_page_open", { id: b.id }).catch(() => {})));
    info.append(acts, mk("p", "hint dm-made", "Made with Leyline. The file is only where each piece stands: the pieces come from your own game."));
    box.append(gal, info);
    return box;
  }

  function reportBox(b) {
    const d = mk("details", "dm-report");
    d.append(mk("summary", "", "Report"));
    const sel = mk("select", "");
    sel.setAttribute("aria-label", "Why");
    for (const r of (data && data.reasons) || [{ id: "other", name: "Something else" }]) sel.append(new Option(r.name, r.id));
    const note = mk("input", "");
    note.maxLength = 300; note.placeholder = "A note (optional)"; note.setAttribute("aria-label", "A note");
    const send = btn("Send", "btn small", async () => {
      send.disabled = true;
      try { await invoke("dens_post", { action: "report", args: { id: b.id, reason: sel.value, note: note.value.trim() } }); say("Thanks: a moderator will look at it.", "ok"); d.open = false; }
      catch (e) { say(words(e), "bad"); }
      send.disabled = false;
    });
    send.id = "dm-report-send";
    d.append(sel, note, send);
    return d;
  }

  // ---------------------------------------------------------------- share and update
  async function share(name, update) {
    view = "share";
    stopShots();
    shareFor = Object.assign({ name: name || "", id: "", shots: [], picked: [], showcase: null, kind: "den", title: "", description: "",
      credit: "", licence: "cc-by", tags: [] }, update || {});
    if (window.LeylineNav && (!window.LeylineNav.current || window.LeylineNav.current().tab !== "dens")) window.LeylineNav.show("mods", true, "dens");
    try { const s = await invoke("den_state"); dens = (s && s.dens) || []; } catch (_) { dens = []; }
    if (!data) { try { data = await invoke("dens_browse", { query: { sort: "new" } }); } catch (_) { /* the lists below fall back */ } }
    if (!shareFor.name && !shareFor.id) shareFor.name = (dens.find((d) => d.name !== "backup" && d.name !== "autosave") || dens[0] || {}).name || "";
    paint();
  }

  function shareView() {
    const f = shareFor;
    const box = mk("form", "dm-share");
    box.addEventListener("submit", (e) => e.preventDefault());
    box.append(mk("h3", "", f.id ? `Update '${f.title}'` : "Share a Den"));
    const row = (label, input, hint) => { const l = mk("label", "dm-field"); l.append(mk("span", "", label), input); if (hint) l.append(mk("small", "hint", hint)); box.append(l); return input; };
    const den = mk("select", "");
    den.id = "dm-share-den";
    if (f.id) den.append(new Option("Keep the shared file", ""));
    for (const d of dens) den.append(new Option(`${d.name} (${plural(d.props, "prop")})`, d.name, false, d.name === f.name));
    if (!dens.length && !f.id) den.append(new Option("No saved Dens yet: in your Den, type .den save and a name", ""));
    den.value = f.name;
    den.addEventListener("change", () => { f.name = den.value; });
    row(f.id ? "New version from" : "Which Den", den);
    if (!f.id) {
      const kind = mk("select", "");
      kind.id = "dm-share-kind";
      kind.append(new Option("A whole Den", "den"), new Option("A stamp (up to 60 pieces within 40 yards)", "stamp"));
      kind.value = f.kind;
      kind.addEventListener("change", () => { f.kind = kind.value; });
      row("Share it as", kind);
    }
    const title = mk("input", "");
    title.id = "dm-share-title"; title.maxLength = 48; title.value = f.title; title.required = true;
    title.addEventListener("input", () => { f.title = title.value; });
    row("Title", title, "3 to 48 letters.");
    const desc = mk("textarea", "");
    desc.id = "dm-share-desc"; desc.maxLength = 600; desc.rows = 3; desc.value = f.description;
    desc.addEventListener("input", () => { f.description = desc.value; });
    row("What it is", desc);
    const credit = mk("input", "");
    credit.id = "dm-share-credit"; credit.maxLength = 80; credit.value = f.credit; credit.placeholder = "e.g. Built with my guild";
    credit.addEventListener("input", () => { f.credit = credit.value; });
    row("Credit line", credit, "Shown under your name. Your name is always on it.");
    const lic = mk("select", "");
    lic.id = "dm-share-licence";
    for (const l of (data && data.licences) || [{ id: "cc-by", name: "CC BY 4.0", line: "Share and remix it; credit me." }]) lic.append(new Option(`${l.name}: ${l.line}`, l.id));
    lic.value = f.licence;
    lic.addEventListener("change", () => { f.licence = lic.value; });
    row("Licence", lic);
    const tags = mk("div", "chips dm-share-tags");
    tags.setAttribute("aria-label", "Tags");
    for (const t of (data && data.all_tags) || []) {
      const on = f.tags.includes(t.id);
      const b = btn(t.name, "chip-btn" + (on ? " on" : ""), () => {
        if (f.tags.includes(t.id)) f.tags = f.tags.filter((x) => x !== t.id);
        else if (f.tags.length < 5) f.tags = f.tags.concat([t.id]);
        else { say("Up to 5 tags.", "warn"); return; }
        b.classList.toggle("on", f.tags.includes(t.id));
        b.setAttribute("aria-pressed", f.tags.includes(t.id) ? "true" : "false");
      });
      b.setAttribute("aria-pressed", on ? "true" : "false");
      b.dataset.tag = t.id;
      tags.append(b);
    }
    const tl = mk("div", "dm-field");
    tl.append(mk("span", "", "Tags (up to 5)"), tags);
    box.append(tl);
    box.append(picturesBox());
    const go = btn(f.id ? "Send the update" : "Share", "btn small primary", () => send(go));
    go.id = "dm-share-go";
    const acts = mk("div", "row dm-share-acts");
    acts.append(go, btn("Cancel", "btn small", () => { view = "browse"; stopShots(); browse(); }));
    box.append(acts, mk("p", "hint", "Every share is checked by a Leyline moderator before others see it (an update too: the version that's out stays up meanwhile). The file holds only where each piece stands."));
    return box;
  }

  function picturesBox() {
    const f = shareFor;
    const d = mk("div", "dm-field dm-pics");
    d.append(mk("span", "", "Pictures (up to 4)"));
    const row = mk("div", "row");
    const take = btn(f.showcase && !f.showcase.done ? "Taking pictures…" : "Take pictures in the game", "btn small", () => showcase(take));
    take.id = "dm-shoot";
    take.disabled = !!(f.showcase && !f.showcase.done);
    const file = mk("input", "");
    file.type = "file"; file.accept = "image/jpeg,image/png"; file.multiple = true; file.hidden = true;
    file.addEventListener("change", async () => {
      for (const x of Array.from(file.files || [])) {
        if (f.shots.length + f.picked.length >= 4) { say("Up to 4 pictures.", "warn"); break; }
        if (x.size > PIC_MAX || !/^image\/(jpeg|png)$/.test(x.type)) { say(`${x.name}: pictures are JPEG or PNG, 900 KB at most.`, "warn"); continue; }
        f.picked.push(await b64(x));
      }
      file.value = "";
      paint();
    });
    row.append(take, btn("Add your own", "btn small", () => file.click()), file);
    d.append(row, mk("small", "hint", "Take pictures: be in your Den with it open; the camera goes over your build and the game takes four pictures, one from each side. Nothing is sent until you press Share."));
    const strip = mk("div", "dm-strip");
    strip.id = "dm-strip";
    f.shots.forEach((s, i) => strip.append(thumb(s.png ? "png" : "jpeg", s.data, () => { f.shots.splice(i, 1); paint(); })));
    f.picked.forEach((p, i) => strip.append(thumb(p.startsWith("iVBOR") ? "png" : "jpeg", p, () => { f.picked.splice(i, 1); paint(); })));
    d.append(strip);
    return d;
  }
  function thumb(type, b64data, drop) {
    const t = mk("div", "dm-shot");
    const img = mk("img", "");
    img.src = `data:image/${type};base64,${b64data}`;
    img.alt = "A picture to share";
    t.append(img, btn("Remove", "btn small dm-shot-x", drop));
    return t;
  }
  const b64 = (file) => new Promise((ok, bad) => {
    const r = new FileReader();
    r.onload = () => ok(String(r.result).replace(/^data:[^,]*,/, ""));
    r.onerror = () => bad("That picture couldn't be read.");
    r.readAsDataURL(file);
  });

  async function showcase(b) {
    const f = shareFor;
    b.disabled = true;
    try {
      f.showcase = await invoke("dens_showcase", { name: f.name || null });
      f.showcase.done = false;
      f.showcase.tries = 0;
      say("In your game: the camera goes over your build and takes four pictures. Keep the game window in front for a few seconds.", "ok");
      paint();
      timer = setInterval(pollShots, 1500);
    } catch (e) { say(words(e, "Your Den couldn't be asked: is it running?"), "bad"); b.disabled = false; }
  }
  async function pollShots() {
    const f = shareFor;
    if (!f || !f.showcase) return stopShots();
    f.showcase.tries += 1;
    let r = null;
    try { r = await invoke("dens_shots", { id: f.showcase.id, since: f.showcase.since }); } catch (_) { r = null; }
    if (r && Array.isArray(r.shots)) {
      const have = new Set(f.shots.map((s) => s.path));
      for (const s of r.shots) if (!have.has(s.path) && f.shots.length + f.picked.length < 4) f.shots.push(s);
    }
    const done = r && r.done;
    if (done || f.showcase.tries > 20) {
      stopShots();
      f.showcase.done = true;
      const ok = r && r.answer && r.answer.ok;
      say(f.shots.length ? `${plural(f.shots.length, "picture")} from your game. Remove any you don't want.`
        : ok === false ? "You need to be in your Den for the pictures: go in, then press Take pictures again." : "No pictures came: is the game open in your Den?", f.shots.length ? "ok" : "warn");
    }
    if (view === "share") paint();
  }
  function stopShots() { if (timer) { clearInterval(timer); timer = null; } }

  async function send(b) {
    const f = shareFor;
    if ((f.title || "").trim().length < 3) { say("Give it a title (3 to 48 letters).", "warn"); return; }
    if (!f.id && !f.name) { say("Pick a saved Den to share (in your Den, type .den save and a name).", "warn"); return; }
    b.disabled = true;
    const args = { title: f.title.trim(), description: (f.description || "").trim(), credit: (f.credit || "").trim(), licence: f.licence, tags: f.tags };
    if (f.id) args.id = f.id; else args.kind = f.kind;
    if (f.based_on) args.based_on = f.based_on;
    const pics = f.shots.length || f.picked.length;
    try {
      const r = await invoke("dens_share", { name: f.name || null, args, shots: pics ? f.shots.map((s) => s.path) : null, pictures: pics ? f.picked : null });
      say(r.message || "Shared. A Leyline moderator looks at it soon.", "ok");
      shareFor = null;
      await showMine(true);
      return;
    } catch (e) { say(words(e, "That couldn't be shared."), "bad"); }
    b.disabled = false;
  }

  // ---------------------------------------------------------------- your shared Dens
  const STATE_WORDS = { pending: "Waiting for a moderator", live: "Live", rejected: "Not approved", removed: "Taken down" };
  async function showMine(keepLine) {
    view = "mine"; mine = null; busy = true;
    if (!keepLine) say("");
    paint();
    try { const r = await invoke("dens_mine"); mine = (r && r.builds) || []; }
    catch (e) { mine = []; say(words(e, "Couldn't load your shared Dens."), "bad"); }
    busy = false;
    paint();
  }
  function mineView() {
    const box = mk("div", "dm-mine");
    box.append(mk("h3", "", "Your shared Dens"));
    if (busy && !mine) { box.append(mk("p", "hint", "Loading…")); return box; }
    if (!(mine || []).length) { box.append(mk("p", "hint", "Nothing shared yet. Share a Den puts one of your saved Dens up for a moderator.")); return box; }
    const ul = mk("ul", "dm-mine-list");
    ul.id = "dm-mine";
    for (const b of mine) {
      const li = mk("li", "");
      li.dataset.id = b.id;
      const t = mk("div", "");
      t.append(mk("b", "", b.title), mk("span", "dm-state", STATE_WORDS[b.state] || b.state));
      if (b.update_waiting) t.append(mk("span", "dm-state", "Update waiting"));
      t.append(mk("small", "hint", [KIND[b.kind], `${b.props} props`, plural(b.likes, "like"), `${b.downloads} opened`].join(" · ")));
      if (b.state === "rejected" && b.reason) t.append(mk("small", "hint dm-why", `Why: ${b.reason}`));
      const acts = mk("div", "row");
      acts.append(btn("Update", "btn small", () => share("", { id: b.id, title: b.title, description: b.description || "", credit: b.credit || "",
        licence: b.licence || "cc-by", tags: b.tags || [], kind: b.kind })));
      if (b.state === "live") acts.append(btn("View", "btn small", () => open(b.id)));
      const un = btn("Unpublish", "btn small", async () => {
        if (un.dataset.sure !== "1") { un.dataset.sure = "1"; un.textContent = "Sure? Unpublish"; return; }
        un.disabled = true;
        try { const r = await invoke("dens_post", { action: "unpublish", args: { id: b.id } }); say(r.message || "Taken down.", "ok"); await showMine(true); return; }
        catch (e) { say(words(e), "bad"); }
        un.disabled = false;
      });
      un.className = "btn small dm-unpublish";
      acts.append(un);
      li.append(t, acts);
      ul.append(li);
    }
    box.append(ul);
    return box;
  }

  // ---------------------------------------------------------------- entries
  document.addEventListener("leyline:page", (e) => { if (e.detail && e.detail.page !== "mods") stopShots(); });
  window.LeylineDenMarket = {
    load,
    share: (name) => share(name || ""),
    // leyline://den/<id> (deeplink.rs): that build's detail, nothing downloaded until Open in my Den
    focus: (id) => { if (window.LeylineNav) window.LeylineNav.show("mods", true, "dens"); return open(id); },
    state: () => ({ view, state: Object.assign({}, state), detail, shareFor, mine }),
  };
  paint();
  // a leyline://den link that came before this file loaded (app.js handleDeepLink)
  if (typeof window.LeylinePendingDen === "string") { const id = window.LeylinePendingDen; delete window.LeylinePendingDen; window.LeylineDenMarket.focus(id); }
})();
