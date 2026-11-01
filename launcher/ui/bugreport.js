// Leyline Launcher: Help › Report a bug. One small dialog that sends the player's words to the website's bug list
// (stream/bugreport.py) through the core (bugs.rs bug_report), as the launcher's signed-in player. Where is filled in
// from the page the player was on before Help and the realm in the play bar; the launcher's recent log (the warnings
// and errors this window wrote, kept here in memory, never on disk) goes only when the box is ticked, and the core
// cleans it (user names in paths, anything token-like) before it leaves. Signed out: Log in, or the website's form.
// TODO(owner): the words in this file, his voice.
(function () {
  "use strict";
  const $ = (id) => document.getElementById(id);
  const invoke = (cmd, args) => window.__TAURI__.core.invoke(cmd, args);
  const oops = (e, fb) => (window.LeylineFriendly ? window.LeylineFriendly(e, fb) : fb);
  const MIN = 10, MAX = 1500, LOG_MAX = 40;
  const NAMES = { play: "Home", den: "Your Den", realms: "Realms", mods: "Mods & Addons", host: "Host", forum: "Forum",
    news: "News", settings: "Settings", agents: "Agents", account: "Manage account", findfiles: "Find your files",
    clients: "Game clients", help: "Help" };
  const KIND = { den: "den", host: "host", mods: "mods" };
  const RATE = "Thanks! That's a lot of reports in a short while. Please wait a little before the next one.";

  // ---------------------------------------------------------------- the launcher's recent log (memory only)
  const log = [];
  const stamp = () => new Date().toTimeString().slice(0, 8);
  function keep(level, args) {
    try {
      const line = Array.from(args).map((a) => (a instanceof Error ? a.message : typeof a === "string" ? a : (() => {
        try { return JSON.stringify(a); } catch (_) { return String(a); } })())).join(" ");
      log.push(`${stamp()} ${level} ${line}`.slice(0, 300));
      if (log.length > LOG_MAX) log.splice(0, log.length - LOG_MAX);
    } catch (_) { /* never break the console */ }
  }
  for (const level of ["error", "warn"]) {
    const orig = console[level] && console[level].bind(console);
    console[level] = function () { keep(level, arguments); if (orig) orig.apply(null, arguments); };
  }
  window.addEventListener("error", (e) => keep("error", [e.message || "script error"]));
  window.addEventListener("unhandledrejection", (e) => keep("error", ["unhandled: " + (e.reason && e.reason.message || e.reason)]));

  // ---------------------------------------------------------------- where the player was
  let lastPage = "play";
  document.addEventListener("leyline:page", (e) => {
    const p = e.detail && e.detail.page;
    if (p && p !== "help") lastPage = p;
  });
  function realmName() {
    const r = $("pb-realm");
    return r ? r.textContent.trim().replace(/\s+/g, " ") : "";
  }
  function whereLine(page) {
    const realm = realmName();
    return (`Launcher › ${NAMES[page] || page}` + (realm ? ` · realm: ${realm}` : "")).slice(0, 120);
  }

  // ---------------------------------------------------------------- the dialog
  const dlg = $("bug-dlg");
  if (!dlg) return;
  let busy = false;
  function view(which) {
    $("bug-out").hidden = which !== "out";
    $("bug-done").hidden = which !== "done";
    $("bug-form").hidden = which !== "form";
  }
  function say(t, kind) {
    const s = $("bug-status");
    s.textContent = t || "";
    s.className = "status" + (kind ? " " + kind : "");
  }
  function count() {
    const n = $("bug-text").value.trim().length;
    $("bug-count").textContent = n < MIN ? `At least ${MIN} characters (${n} so far)` : `${n} / ${MAX}`;
  }
  async function open(page) {
    const at = page || lastPage;
    let yes = false;
    try { yes = await invoke("social_status"); } catch (_) { yes = false; }
    say("");
    if (!yes) view("out");
    else {
      view("form");
      $("bug-where").value = KIND[at] || "launcher";
      $("bug-page").value = whereLine(at);
      $("bug-log").checked = false;
      count();
    }
    if (!dlg.open) { if (dlg.showModal) dlg.showModal(); else dlg.setAttribute("open", ""); }
    if (yes) $("bug-text").focus();
  }
  function close() { if (dlg.close) dlg.close(); else dlg.removeAttribute("open"); }

  async function send(ev) {
    ev.preventDefault();
    if (busy) return;
    const text = $("bug-text").value.trim();
    if (text.length < MIN) { say(`Tell us a little more: at least ${MIN} characters about what happened.`, "bad"); $("bug-text").focus(); return; }
    const args = { text: text.slice(0, MAX), where: $("bug-where").value, page: $("bug-page").value.trim().slice(0, 120) };
    if ($("bug-log").checked) args.log = log.slice(-LOG_MAX);
    busy = true;
    $("bug-send").disabled = true;
    say("Sending…");
    try {
      const r = await invoke("bug_report", { args });
      $("bug-text").value = "";
      $("bug-done-text").textContent = `Thanks, we have it${r && r.id ? ` (report ${r.id})` : ""}. We read every one.`;
      view("done");
    } catch (e) {
      const m = String(e);
      if (m === "signed_out") {
        if (window.LeylineSocial) window.LeylineSocial.refresh();
        view("out");
      } else if (m === "rate_limited") say(RATE, "warn");
      else say(oops(e, "That report didn't go. Try again shortly, or use the website's form."), "bad");
    } finally {
      busy = false;
      $("bug-send").disabled = false;
    }
  }

  $("bug-form").addEventListener("submit", send);
  $("bug-text").addEventListener("input", count);
  dlg.querySelectorAll("[data-bug-close]").forEach((b) => b.addEventListener("click", close));
  $("bug-login").addEventListener("click", () => { close(); if (window.LeylineNav) window.LeylineNav.show("account"); });
  const opener = $("bug-open");
  if (opener) opener.addEventListener("click", () => open());

  window.LeylineBugReport = { open, log: () => log.slice(), where: whereLine };
})();
