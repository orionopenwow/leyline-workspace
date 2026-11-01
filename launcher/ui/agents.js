// Leyline Launcher: the Agents page and the approval card (owner, 31 Oct 2026: "Create a place for agents to hook into").
// The player's own AI assistant starts `Leyline mcp` (src-tauri/src/agents.rs). Read tools answer at once; ask tools
// put a request in the launcher, shown here as a card on every page (Approve / Deny; no answer in 2 minutes is Deny);
// the rest has no tool at all. Nothing answers until Allow agents is on.
//
// Commands: agents_state (polled every 2 s), agents_set_allow, agents_answer, agents_connect. Everything an agent
// wrote (its name, the card's lines) is shown with textContent, never as markup.
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
  const oops = (e, fb) => (window.LeylineFriendly ? window.LeylineFriendly(e, fb) : (typeof e === "string" ? e : fb));

  // what each tool means, in plain words (the page's tier lists; agents.rs TOOLS)
  const WORDS = {
    realm_status: "If a realm is up, and how many people play",
    realms_list: "The realm list",
    my_clients: "Your game folders and the mods in each",
    catalog_search: "Search the Workshop",
    host_status: "Your own realm: if it runs",
    host_logs: "Your realm's recent logs, with addresses, names and keys hidden",
    forum_my_threads: "Your own forum threads",
    mod_install: "Install or update a mod",
    mod_remove: "Remove a mod (it can be installed again)",
    realm_start: "Start your own realm",
    realm_stop: "Stop your own realm",
    forum_new_thread: "Post a forum thread as you",
    forum_reply: "Reply on the forum as you",
  };
  const READ = ["realm_status", "realms_list", "my_clients", "catalog_search", "host_status", "host_logs", "forum_my_threads"];
  const ASK = ["mod_install", "mod_remove", "realm_start", "realm_stop", "forum_new_thread", "forum_reply"];
  const NEVER = ["Sign in, see or change a password, a Hero Key or your account", "Anything with money",
    "Delete files, remove a game client, wipe a realm", "Make your realm public, open its sign-up, or make invites",
    "Kick, ban or unban players, or change who may join", "Change your realm list address, or your launcher's updates",
    "Use the website's staff tools"];
  const OUTCOME = {
    done: ["Done", "ok"], failed: ["Didn't work", "bad"], denied: ["You said no", ""], "timed out": ["No answer, not done", ""],
    "agents off": ["Blocked: agents were off", ""], "launcher closed": ["Leyline was closed", ""], busy: ["Another request was waiting", ""],
    refused: ["Refused: a bad request", "bad"], "still running": ["Still running", "warn"],
  };

  let state = null;
  let skew = 0;          // the core's clock minus this page's, in seconds
  let busy = "";         // the request id being answered
  let timer = null;

  function say(text, kind) { const s = $("ag-status"); if (s) { s.textContent = text || ""; s.className = "status" + (kind ? " " + kind : ""); } }
  const onPage = () => !!(window.LeylineNav && window.LeylineNav.current().page === "agents");
  const nowCore = () => Math.floor(Date.now() / 1000) + skew;
  const left = (p) => Math.max(0, (p.expires || 0) - nowCore());

  function tiers() {
    const fill = (id, items) => { const ul = $(id); if (!ul) return; ul.replaceChildren(...items.map((t) => el("li", "", t))); };
    fill("ag-t-read", READ.map((n) => WORDS[n]));
    fill("ag-t-ask", ASK.map((n) => WORDS[n]));
    fill("ag-t-never", (state && state.never) || NEVER);
  }

  function connRow(key, conn) {
    const dot = $(`ag-${key}-state`), add = $(`ag-${key}-add`), del = $(`ag-${key}-del`);
    if (!dot) return;
    const st = conn ? conn.state : "missing";
    const words = { connected: "Added", other: "Added for another copy", no: "Not added", missing: key === "dt" ? "App not found" : "Not added" };
    dot.textContent = words[st] || "Not added";
    dot.className = "ag-dot" + (st === "connected" ? " on" : st === "other" ? " warn" : "");
    add.hidden = st === "connected";
    add.textContent = st === "other" ? "Point it here" : "Add for me";
    del.hidden = !(st === "connected" || st === "other");
    if (conn && conn.path) { dot.title = conn.path; }
  }

  function requestView(p, compact) {
    const box = el("div", compact ? "ag-req" : "ag-req card");
    const head = el("div", "ag-card-head");
    head.append(el("span", "ag-card-from", `${p.from || "An agent"} asks`), el("span", "ag-card-left", `${left(p)} s left`));
    const ul = el("ul");
    for (const l of (p.card && p.card.lines) || []) ul.append(el("li", "", l));
    const row = el("div", "row");
    const yes = el("button", "btn primary small", "Approve"); yes.type = "button";
    const no = el("button", "btn small", "Deny"); no.type = "button";
    yes.onclick = () => answer(p.id, true);
    no.onclick = () => answer(p.id, false);
    yes.disabled = no.disabled = busy === p.id;
    row.append(yes, no);
    box.append(head, el("h4", "", (p.card && p.card.title) || p.tool), ul, row);
    return box;
  }

  function renderCard() {
    const card = $("ag-card");
    if (!card) return;
    const p = state && state.allow && state.pending && state.pending[0];
    const show = !!p && (!onPage() || busy === (p && p.id));
    if (!p && busy) return;   // keep the card while its answer runs
    card.hidden = !show;
    if (!p) return;
    $("ag-card-from").textContent = `${p.from || "An agent"} asks`;
    $("ag-card-left").textContent = busy === p.id ? "" : `${left(p)} s left`;
    $("ag-card-title").textContent = (p.card && p.card.title) || p.tool;
    $("ag-card-lines").replaceChildren(...((p.card && p.card.lines) || []).map((l) => el("li", "", l)));
    $("ag-card-yes").disabled = $("ag-card-no").disabled = busy === p.id;
    $("ag-card-yes").onclick = () => answer(p.id, true);
    $("ag-card-no").onclick = () => answer(p.id, false);
    if (busy !== p.id) $("ag-card-status").textContent = "";
  }

  function render() {
    tiers();
    const badge = $("nav-agents-count");
    const n = state && state.allow ? (state.pending || []).length : 0;
    if (badge) { badge.hidden = !n; badge.textContent = n ? String(n) : ""; }
    renderCard();
    if (!state) return;
    const allow = $("ag-allow");
    if (allow && document.activeElement !== allow) allow.checked = !!state.allow;
    const note = $("ag-allow-note");
    if (note) note.textContent = state.allow ? "On: agents can look, and ask you before they change anything." : "Off: agents get no answers at all.";
    $("ag-cc-cmd").textContent = state.command || "";
    $("ag-raw").textContent = state.exe ? `"${state.exe}" mcp` : "";
    $("ag-json").textContent = state.json || "";
    connRow("cc", state.claude_code);
    connRow("dt", state.desktop);
    const wait = $("ag-wait");
    const list = state.allow ? state.pending || [] : [];
    wait.replaceChildren(...(list.length ? list.map((p) => requestView(p)) : [el("p", "hint", state.allow ? "Nothing is waiting." : "Nothing can ask while agents are off.")]));
    const audit = $("ag-audit");
    const rows = (state.audit || []).slice(0, 60);
    if (!rows.length) { audit.replaceChildren(el("p", "hint", "Nothing yet.")); return; }
    const ul = el("ul", "ag-log");
    for (const a of rows) {
      const li = el("li");
      const t = new Date((a.at || 0) * 1000);
      const when = el("span", "ag-when", t.toLocaleString([], { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" }));
      const what = el("span", "ag-what", WORDS[a.tool] || a.tool);
      const [word, kind] = OUTCOME[a.outcome] || [a.outcome, ""];
      const res = el("span", "ag-res" + (kind ? " " + kind : ""), word);
      li.append(when, el("span", "ag-from", a.from || "An agent"), what, res);
      if (a.note) li.title = a.note;
      ul.append(li);
    }
    audit.replaceChildren(ul);
  }

  async function refresh() {
    if (!tauri) { render(); return; }
    try {
      state = await invoke("agents_state");
      skew = (state.now || 0) - Math.floor(Date.now() / 1000);
    } catch (e) {
      console.warn("agents_state", e);
    }
    render();
  }

  async function answer(id, yes) {
    if (busy) return;
    busy = id;
    render();
    const cs = $("ag-card-status");
    if (cs) { cs.textContent = yes ? "Working…" : ""; cs.className = "status"; }
    try {
      const text = await invoke("agents_answer", { id, approve: yes });
      say(text, yes ? "ok" : "");
      if (cs) { cs.textContent = text; cs.className = "status ok"; }
    } catch (e) {
      const m = oops(e, "That didn't work. Try again.");
      say(m, "bad");
      if (cs) { cs.textContent = m; cs.className = "status bad"; }
    }
    busy = "";
    await refresh();
    const card = $("ag-card");
    if (card && !(state && state.pending && state.pending.length)) setTimeout(() => { if (!busy && !(state && state.pending && state.pending.length)) card.hidden = true; }, 2500);
  }

  async function connect(target, on) {
    say(on ? "Adding…" : "Removing…");
    try { say(await invoke("agents_connect", { target, on }), "ok"); } catch (e) { say(oops(e, "That didn't work."), "bad"); }
    refresh();
  }

  function copy(id) {
    const text = ($(id) && $(id).textContent) || "";
    const done = () => say("Copied.", "ok");
    if (navigator.clipboard && navigator.clipboard.writeText) navigator.clipboard.writeText(text).then(done, () => say("Select it and copy it yourself.", "warn"));
    else say("Select it and copy it yourself.", "warn");
  }

  function wire() {
    const allow = $("ag-allow");
    if (allow) allow.addEventListener("change", async () => {
      try { await invoke("agents_set_allow", { on: allow.checked }); say(allow.checked ? "Agents are on." : "Agents are off.", "ok"); }
      catch (e) { allow.checked = !allow.checked; say(oops(e, "That didn't work."), "bad"); }
      refresh();
    });
    const bind = (id, f) => { const b = $(id); if (b) b.addEventListener("click", f); };
    bind("ag-cc-add", () => connect("claude_code", true));
    bind("ag-cc-del", () => connect("claude_code", false));
    bind("ag-dt-add", () => connect("desktop", true));
    bind("ag-dt-del", () => connect("desktop", false));
    document.querySelectorAll("#page-agents [data-copy]").forEach((b) => b.addEventListener("click", () => copy(b.dataset.copy)));
    document.addEventListener("leyline:page", (e) => { if (e.detail && e.detail.page === "agents") refresh(); else renderCard(); });
    // the countdown and new requests: every 2 seconds (a slow tick while nothing can ask)
    const tick = async () => { await refresh(); timer = setTimeout(tick, state && state.allow ? 2000 : 6000); };
    tick();
  }

  window.LeylineAgents = { refresh, state: () => state };
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", wire); else wire();
})();
