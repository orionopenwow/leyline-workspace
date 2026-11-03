// Leyline: the Host pack, Docker, realm controls (stop, logs, invite link) on the Host page, and the P2P /
// torrent switches under Settings > Advanced. Commands: src-tauri/src/p2p/cmds.rs. Loaded after app.js.
"use strict";
(() => {
  const call = window.__TAURI__ && window.__TAURI__.core
    ? window.__TAURI__.core.invoke
    : async () => { throw "This is a preview page: open it in the Leyline app."; };
  // never a raw error to the player (friendly.js): the error goes to the console, the player gets a plain line
  const oops = (e, fb) => (window.LeylineFriendly ? window.LeylineFriendly(e, fb) : (fb || "Something went wrong. Try again."));
  const byId = (id) => document.getElementById(id);
  const mk = (tag, cls, text) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  };
  const note = (node, text, kind) => { node.textContent = text || ""; node.className = "status" + (kind ? " " + kind : ""); };
  const btn = (label, cls, on) => { const b = mk("button", cls || "btn", label); b.type = "button"; b.addEventListener("click", on); return b; };
  const linkBtn = (label, name) => btn(label, "btn small", () => call("open_link", { name }).catch(() => {}));

  // ---------------------------------------------------------------- Host page: the pack and Docker
  const page = byId("page-host");
  // the Host page's "Host pack & settings" tab (hostconsole.js has the console and the guide)
  const panel = page && (byId("hc-pane-kit") || page.querySelector(".panel"));
  if (!panel) return;
  // owner 2026-10-31: "quick step guides 1 2 3 and buttons to click": step 1 (Get ready) is two check rows, the Host
  // pack and Docker, each a green tick when done or one obvious button; the version and folder sit under Details.
  const packCard = mk("section", "hs-checks");
  packCard.id = "hp-card";
  packCard.setAttribute("aria-label", "Host pack and Docker");
  const checkRow = (title) => {
    const r = mk("div", "hs-check");
    const n = mk("span", "hs-check-n", "");
    n.setAttribute("aria-hidden", "true");
    const t = mk("div", "hs-check-t");
    const line = mk("span", "", "Checking…");
    t.append(mk("b", "", title), line);
    const acts = mk("div", "hs-row-acts");
    r.append(n, t, acts);
    return { r, n, line, acts };
  };
  const packRow = checkRow("Host pack");
  const dockRow = checkRow("Docker");
  const packLine = packRow.line, packActs = packRow.acts, dockLine = dockRow.line, dockActs = dockRow.acts;
  const packStatus = mk("p", "status");
  const packMore = mk("details", "hs-details");
  const packInfo = mk("p", "hint", "");
  packMore.append(mk("summary", "", "Details"), packInfo);
  packCard.append(packRow.r, dockRow.r, packStatus);
  let packOk = false, dockOk = false;
  const setRow = (row, ok, n) => { row.r.classList.toggle("done", ok); row.n.textContent = ok ? "✓" : String(n); };
  function paintKitSteps() {
    const ready = packOk && dockOk;
    const li = (k) => byId("hs-k" + k);
    if (!li(1)) return;
    li(1).classList.toggle("hs-ok", ready);
    li(1).querySelector(".hs-num").textContent = ready ? "✓" : "1";
    li(1).classList.toggle("now", !ready);
    li(3).classList.toggle("now", ready);
  }
  const k1 = byId("hs-k1-body");
  if (k1) { k1.prepend(packCard); k1.append(packMore); }
  else { const steps = panel.querySelector("ol.steps"); panel.insertBefore(packCard, steps || panel.firstChild); }

  let pack = null;
  async function useKit(dir) {
    try { await call("set_host_kit", { path: dir }); } catch (e) { note(packStatus, oops(e), "bad"); return; }
    if (typeof loadHost === "function") loadHost();   // app.js: re-read the kit
  }
  async function loadPack() {
    try { pack = await call("hostpack_info"); } catch (e) { packLine.textContent = oops(e); return; }
    packActs.replaceChildren();
    const inst = pack.installed, av = pack.available;
    if (!inst) {
      packOk = false;
      packLine.textContent = av ? `One download${av.size ? `, ${Math.ceil(av.size / 1024)} KB` : ""}: your realm and its tools. No game files.`
        : (pack.error || "Not listed right now: try again in a minute.");
      packInfo.textContent = av ? `Not installed. Version ${av.version}.` : "Not installed.";
      if (av) packActs.append(btn("Get the Host pack", "btn primary", install));
    } else {
      packOk = !pack.update;
      packLine.textContent = pack.update && av ? "A new Host pack is out." : "Installed and ready.";
      packInfo.textContent = `Installed: version ${inst.version}` + (pack.update && av ? `. Version ${av.version} is out.` : ".") + ` Folder: ${pack.kit_dir}`;
      if (pack.update) packActs.append(btn(`Update to ${av.version}`, "btn primary", install));
      packActs.append(btn("Use the Host pack", "btn small", () => useKit(pack.kit_dir)));
    }
    setRow(packRow, packOk, 1);
    paintKitSteps();
  }
  async function install() {
    note(packStatus, "Downloading and checking the Host pack…");
    packActs.querySelectorAll("button").forEach((b) => { b.disabled = true; });
    try {
      const [inst, dir] = await call("hostpack_install");
      note(packStatus, `Host pack ${inst.version} installed and checked. Your realm's own files were kept.`, "ok");
      await useKit(dir);
    } catch (e) { note(packStatus, oops(e), "bad"); }
    loadPack();
  }
  async function loadDocker() {
    dockLine.textContent = "Checking Docker…";
    dockActs.replaceChildren();
    let d;
    try { d = await call("docker_info"); } catch (e) { dockLine.textContent = oops(e); return; }
    dockOk = d.state === "ready";
    dockLine.className = dockOk ? "ok" : "warn";
    dockLine.textContent = (dockOk ? `Docker ${d.version || ""} is ready.` : d.help);
    if (!dockOk) { const g = linkBtn("Get Docker", d.get_url.includes("engine") ? "dockerengine" : "docker"); g.className = "btn primary"; dockActs.append(g); }
    dockActs.append(btn("Check again", "btn small", loadDocker));
    setRow(dockRow, dockOk, 2);
    paintKitSteps();
  }

  // ---------------------------------------------------------------- Host page: realm controls
  const hostCard = byId("host-card");
  const ctl = mk("div", "row");
  const inviteLine = mk("div", "hint");
  const ctlStatus = mk("p", "status");
  const logs = mk("pre", "logs");
  logs.hidden = true;
  logs.style.cssText = "max-height:260px;overflow:auto;white-space:pre-wrap;font:11px ui-monospace,Consolas,monospace;background:#0b0c0f;padding:8px;border:1px solid #333;user-select:text";
  let invite = null;
  async function loadInvite() {
    try { invite = await call("host_invite"); } catch { invite = null; }
    inviteLine.textContent = invite ? `Invite link: ${invite}` : "The invite link appears after the first start.";
  }
  ctl.append(
    btn("Stop", "btn small", async () => {
      note(ctlStatus, "Stopping…");
      try { note(ctlStatus, await call("host_stop"), "ok"); } catch (e) { note(ctlStatus, oops(e), "bad"); }
    }),
    btn("Logs", "btn small", async () => {
      if (!logs.hidden) { logs.hidden = true; return; }
      note(ctlStatus, "Reading the logs…");
      try { logs.textContent = await call("host_logs", { lines: 200 }) || "(no log lines yet)"; logs.hidden = false; logs.scrollTop = logs.scrollHeight; note(ctlStatus, ""); }
      catch (e) { note(ctlStatus, oops(e), "bad"); }
    }),
    btn("Copy invite link", "btn small", async () => {
      await loadInvite();
      if (!invite) { note(ctlStatus, "No invite link yet: start your realm once.", "warn"); return; }
      try { await navigator.clipboard.writeText(invite); note(ctlStatus, "Copied. Friends open it once, then play from the launcher or the browser.", "ok"); }
      catch { note(ctlStatus, invite, "ok"); }
    }),
    btn("New invite link", "btn small", async () => {
      note(ctlStatus, "Making a new invite link…");
      try { invite = await call("host_new_invite"); inviteLine.textContent = `Invite link: ${invite}`; note(ctlStatus, "New link made. The old one no longer lets anyone new in.", "ok"); }
      catch (e) { note(ctlStatus, oops(e), "bad"); }
    }),
  );
  if (hostCard) hostCard.append(ctl, inviteLine, ctlStatus, logs);

  // ---------------------------------------------------------------- Host page: the realm list details
  // Tags, region, language and the player limit are the network's filters; they live in the kit's state/beacon.json
  // (host.rs ListingDetails) and reach the list with the next heartbeat. Direct connections stay off unless the
  // host allows them AND the network turns them on (the p2p_hosting flag); nothing here turns a flag on.
  const TAGS = [["blizzlike", "Original rules"], ["boosted", "Boosted"], ["custom", "Custom"], ["pvp", "PvP"], ["rp", "RP"], ["hangout", "Hangout"], ["bots", "Bots"]];
  const REGIONS = [["", "Not shown"], ["na-east", "North America east"], ["na-west", "North America west"], ["sa", "South America"],
    ["eu", "Europe"], ["africa", "Africa"], ["me", "Middle East"], ["asia", "Asia"], ["oce", "Oceania"]];
  const listCard = mk("section", "card");
  listCard.id = "hl-card";
  const lhead = mk("div", "card-head");
  lhead.append(mk("span", "", "ON THE REALM LIST"));
  const tagBox = mk("div", "row");
  const tagChecks = TAGS.map(([v, label]) => {
    const l = mk("label", "check inline"); const c = mk("input", ""); c.type = "checkbox"; c.value = v;
    l.append(c, document.createTextNode(" " + label)); tagBox.append(l); return c;
  });
  const region = mk("select", "");
  for (const [v, t] of REGIONS) { const o = mk("option", "", t); o.value = v; region.append(o); }
  const lang = mk("input", ""); lang.placeholder = "en, de, pt-BR"; lang.maxLength = 6; lang.size = 8; lang.spellcheck = false;
  const maxp = mk("input", ""); maxp.type = "number"; maxp.min = "0"; maxp.max = "10000"; maxp.placeholder = "not shown";
  const direct = mk("input", ""); direct.type = "checkbox";
  const fields = mk("div", "row");
  const lr = mk("label", "inline", "Region "); lr.append(region);
  const ll = mk("label", "inline", "Language "); ll.append(lang);
  const lm = mk("label", "inline", "Player limit "); lm.append(maxp);
  fields.append(lr, ll, lm);
  const dl = mk("label", "check inline"); dl.append(direct, document.createTextNode(" Allow direct connections (off by default; only used once the network turns direct connections on)"));
  const listStatus = mk("p", "status");
  listCard.append(lhead, mk("p", "lede", "How players find your realm: the rules it runs, where it is and its language. Bots are tagged by themselves when your realm runs them."),
    tagBox, fields, dl, mk("div", "row"), listStatus);
  listCard.lastChild.previousSibling.append(btn("Save", "btn small", saveListing));
  // Advanced (owner audit 2026-10-30): the realm list settings sit under a disclosure at the end of the tab, shown once
  // there is a realm to list (app.js "leyline:kit": a realm, or its realm file in the Host pack)
  const listAdv = mk("details", "hl-adv");
  listAdv.id = "hl-adv";
  listAdv.hidden = true;
  listAdv.append(mk("summary", "", "Advanced: how players find your realm"), listCard);
  panel.append(listAdv);
  document.addEventListener("leyline:kit", (e) => {
    const k = e.detail;
    listAdv.hidden = !(k && (k.realm || (k.seeds || []).some((x) => x.in_kit)));
  });

  async function loadListing() {
    let d;
    try { d = await call("host_listing"); } catch (e) { note(listStatus, oops(e), "warn"); return; }
    tagChecks.forEach((c) => { c.checked = d.tags.includes(c.value); });
    region.value = d.region || "";
    lang.value = d.language || "";
    maxp.value = d.max_players || "";
    direct.checked = !!d.direct;
  }
  async function saveListing() {
    const details = {
      tags: tagChecks.filter((c) => c.checked).map((c) => c.value),
      region: region.value || null,
      language: lang.value.trim() || null,
      max_players: +maxp.value > 0 ? Math.floor(+maxp.value) : null,
      direct: direct.checked,
    };
    note(listStatus, "Saving…");
    try { note(listStatus, await call("host_listing_set", { details }), "ok"); } catch (e) { note(listStatus, oops(e), "bad"); }
  }

  // ---------------------------------------------------------------- Host page: which game (host any client)
  // The realm profiles the kit can run for the clients Leyline found on this PC (host/profiles.rs). The pick is
  // saved in the kit folder; START MY REALM hands it to the kit's script with that client's folder, from which the
  // server data is made (read-only, nothing downloaded).
  const gameCard = mk("section", "card");
  gameCard.id = "hg-card";
  const ghead = mk("div", "card-head");
  ghead.append(mk("span", "", "WHICH GAME"));
  const gameSel = mk("select", "");
  const clientSel = mk("select", "");
  const gameHint = mk("p", "hint");
  const gameStatus = mk("p", "status");
  const gRow = mk("div", "row");
  const lg = mk("label", "inline", "Realm "); lg.append(gameSel);
  const lc = mk("label", "inline", "Your client "); lc.append(clientSel);
  gRow.append(lg, lc);
  if (byId("hs-k2-body")) { gameCard.className = "hs-game"; gRow.className = "hs-fields"; lg.className = lc.className = "hc-field"; gameCard.append(gRow, gameHint, gameStatus); }
  else gameCard.append(ghead, mk("p", "lede", "Host a realm for any client you have on this PC. Players need their own client of the same version."),
    gRow, gameHint, gameStatus);
  if (byId("hs-k2-body")) byId("hs-k2-body").prepend(gameCard);
  else panel.insertBefore(gameCard, packCard.nextSibling);
  let offers = [];
  function showClients(keep) {
    const o = offers.find((x) => x.id === gameSel.value);
    clientSel.replaceChildren();
    for (const c of (o ? o.clients : [])) { const op = mk("option", "", `${c.name} · ${c.path}`); op.value = c.id; clientSel.append(op); }
    lc.hidden = !o || !o.clients.length;
    if (keep) clientSel.value = keep;
    gameHint.textContent = o ? (o.note || `Runs ${o.core}. The realm's map data is made from your own game folder, which is only read, never changed.`) : "";
    gameHint.className = "hint" + (o && o.note ? " warn" : "");
  }
  async function saveGame() {
    const o = offers.find((x) => x.id === gameSel.value);
    if (o && o.note) { note(gameStatus, ""); return; }
    if (!o) {
      try { await call("host_choose", { profile: "", clientId: null }); note(gameStatus, "The Host pack keeps the realm it hosts, or asks on its first start.", "ok"); }
      catch (e) { note(gameStatus, oops(e), "bad"); }
      return;
    }
    try {
      await call("host_choose", { profile: o.id, clientId: clientSel.value || null });
      note(gameStatus, `Saved: Start my realm hosts a ${o.version} realm.`, "ok");
    } catch (e) { note(gameStatus, oops(e), "bad"); }
  }
  gameSel.addEventListener("change", () => { showClients(); saveGame(); });
  clientSel.addEventListener("change", saveGame);
  async function loadGames() {
    let r;
    try { r = await call("host_profiles"); } catch (e) { gameHint.textContent = oops(e); return; }
    if (!Array.isArray(r) || !Array.isArray(r[0])) return;
    const [list, choice] = r;
    offers = list.filter((o) => o && Array.isArray(o.clients));
    gameSel.replaceChildren();
    const first = mk("option", "", "As set up before (or the Host pack asks)"); first.value = ""; gameSel.append(first);
    for (const o of offers) {
      const op = mk("option", "", o.note ? `${o.label} (needs a ${o.version} client)` : o.label);
      op.value = o.id; op.disabled = !!o.note; gameSel.append(op);
    }
    let keep = null;
    if (choice && offers.some((o) => o.id === choice.profile && !o.note)) {
      gameSel.value = choice.profile;
      const o = offers.find((x) => x.id === choice.profile);
      const c = o.clients.find((x) => x.path === choice.client_dir);
      keep = c ? c.id : null;
    }
    showClients(keep);
  }

  function loadHostExtras() { loadPack(); loadDocker(); loadInvite(); loadListing(); loadGames(); }

  // ---------------------------------------------------------------- Settings > Advanced: P2P and torrent
  const setPanel = byId("page-settings") && byId("page-settings").querySelector(".panel");
  const adv = mk("section", "");
  adv.id = "p2p-adv";
  const advList = mk("div", "");
  const advStatus = mk("p", "status");
  const natLine = mk("p", "hint");
  adv.append(mk("h3", "", "Advanced: peer-to-peer"),
    mk("p", "lede", "Off unless you or the website turn them on. Peer-to-peer never shares game files or a game client. It only shares Leyline itself, checked mods, theme packs and the Host pack, and each file is checked before it is used."),
    advList, natLine, advStatus);
  if (setPanel) setPanel.insertBefore(adv, setPanel.querySelector("h3") || null);

  async function loadFlags() {
    let v;
    try { v = await call("p2p_flags"); } catch (e) { note(advStatus, oops(e), "bad"); return; }
    advList.replaceChildren();
    for (const [name, label] of v.known) {
      const row = mk("label", "inline");
      const sel = mk("select", "");
      const local = Object.prototype.hasOwnProperty.call(v.local.overrides, name) ? v.local.overrides[name] : null;
      for (const [val, text] of [["", `Follow the website (${v.site[name] ? "on" : "off"})`], ["on", "On on this PC"], ["off", "Off on this PC"]]) {
        const o = mk("option", "", text); o.value = val; sel.append(o);
      }
      sel.value = local === null ? "" : (local ? "on" : "off");
      sel.addEventListener("change", async () => {
        try { await call("p2p_set_flag", { name, value: sel.value === "" ? null : sel.value === "on" }); note(advStatus, "Saved.", "ok"); loadFlags(); }
        catch (e) { note(advStatus, oops(e), "bad"); }
      });
      row.append(mk("span", "", `${label}: ${v.effective[name] ? "ON" : "off"} `), sel);
      advList.append(row);
    }
    const lim = v.effective.limits;
    const up = mk("input", ""); up.type = "number"; up.min = "0"; up.max = "100000"; up.value = lim.seed_upload_kbps;
    const ratio = mk("input", ""); ratio.type = "number"; ratio.min = "0"; ratio.max = "1000"; ratio.value = lim.seed_ratio_pct;
    const limRow = mk("div", "row");
    const l1 = mk("label", "inline", "Upload cap KB/s (0 = none) "); l1.append(up);
    const l2 = mk("label", "inline", "Share back up to % of size (0 = never) "); l2.append(ratio);
    limRow.append(l1, l2, btn("Save limits", "btn small", async () => {
      try {
        await call("p2p_set_limits", { limits: { seed_upload_kbps: +up.value || 0, seed_ratio_pct: +ratio.value || 0, max_peers: lim.max_peers || 30 } });
        note(advStatus, "Limits saved.", "ok");
      } catch (e) { note(advStatus, oops(e), "bad"); }
    }));
    advList.append(limRow);
    natLine.replaceChildren();
    if (v.effective.p2p_hosting) {
      natLine.append(btn("Check my network", "btn small", async () => {
        note(advStatus, "Asking a STUN server for this PC's public address…");
        try {
          const r = await call("p2p_nat_check");
          const words = { open: "no router in the way", endpoint_independent: "direct connections can work", symmetric: "direct connections will not work: the Leyline network is used", unknown: "could not tell", blocked: "direct connections are blocked: the Leyline network is used" };
          note(advStatus, `Network: ${words[r.kind] || r.kind}.`, r.kind === "open" || r.kind === "endpoint_independent" ? "ok" : "warn");
        } catch (e) { note(advStatus, oops(e), "bad"); }
      }));
    }
    // "Help the network: relay for others" (docs/LEYLINE-MESH.md): only when the website offers it AND this PC is "On".
    if (v.site.relay_volunteer && v.local.overrides.relay_volunteer === true) {
      const hostIn = mk("input", ""); hostIn.placeholder = "this PC's public name, like relay.example.org"; hostIn.size = 34;
      const vRow = mk("div", "row");
      vRow.append(mk("span", "hint", "Relaying needs a public name for this PC and ports 80, 443 and 41000-41051 open. Players' game traffic passes through it (never passwords). "), hostIn,
        btn("Start relaying", "btn small", async () => {
          note(advStatus, "Starting the community relay…");
          try { note(advStatus, await call("host_volunteer_relay", { on: true, publicHost: hostIn.value.trim(), region: null }), "ok"); }
          catch (e) { note(advStatus, oops(e), "bad"); }
        }),
        btn("Stop", "btn small", async () => {
          try { note(advStatus, await call("host_volunteer_relay", { on: false, publicHost: null, region: null }), "ok"); }
          catch (e) { note(advStatus, oops(e), "bad"); }
        }));
      advList.append(vRow);
    }
    if (v.error) note(advStatus, v.from_cache ? "Using the saved copy of the website's switches (offline)." : "Could not read the website's switches: everything stays off unless you turn it on here.", "warn");
  }

  // ---------------------------------------------------------------- when the pages open
  document.querySelectorAll('[data-page="host"]').forEach((b) => b.addEventListener("click", loadHostExtras));
  document.querySelectorAll('[data-page="settings"]').forEach((b) => b.addEventListener("click", loadFlags));
})();
