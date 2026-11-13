// Leyline: Settings › Your Leyline network key (docs/LEYLINE-FRIENDS.md; identity.rs). The key (peer.key) is what
// makes this PC "you" to your friends and on their worlds: a reinstall or a new PC without it comes back as someone
// new, and the characters kept on friends' worlds stay with the old key. Two buttons: Save my key (a file where the
// player picks, with the warning in it) and Bring back my key (a saved file picked; the key this PC had is kept
// aside). The key itself is never shown on the page and never logged. Text only (textContent). An older core
// without the commands hides the section.
"use strict";
(() => {
  const call = window.__TAURI__ && window.__TAURI__.core
    ? window.__TAURI__.core.invoke
    : async () => { throw "This is a preview page: open it in the Leyline app."; };
  const oops = (e, fb) => (window.LeylineFriendly ? window.LeylineFriendly(e, fb) : (typeof e === "string" ? e : (fb || "Something went wrong. Try again.")));
  const missing = (e) => /not found|unknown (command )?/i.test(String(e || ""));
  const mk = (tag, cls, text) => { const e = document.createElement(tag); if (cls) e.className = cls; if (text !== undefined) e.textContent = text; return e; };
  const btn = (label, cls, on, id) => { const b = mk("button", cls || "btn", label); b.type = "button"; if (id) b.id = id; if (on) b.addEventListener("click", on); return b; };
  const page = document.getElementById("page-settings");
  if (!page) return;
  const about = document.getElementById("set-about");

  const box = mk("section", "set-peerkey");
  box.id = "set-peerkey";
  box.setAttribute("aria-label", "Your Leyline network key");
  const line = mk("p", "status");
  line.id = "peerkey-status";
  line.setAttribute("role", "status");
  const say = (t, k) => { line.textContent = t || ""; line.className = "status" + (k ? " " + k : ""); };
  let asking = false;
  async function run(cmd, fb) {
    for (const b of box.querySelectorAll("button")) b.disabled = true;
    try {
      const m = await call(cmd);
      if (m) say(String(m), "ok");
      else say("");
    } catch (e) {
      if (missing(e)) { box.hidden = true; return; }
      say(oops(e, fb), "bad");
    }
    for (const b of box.querySelectorAll("button")) b.disabled = false;
  }
  function paint() {
    const warn = mk("p", "lede peerkey-warn", "Keep the saved file private: whoever has it is you to your friends. Never send it to anyone, Leyline included.");
    const nodes = [mk("h3", "", "Your Leyline network key"),
      mk("p", "lede", "This key is what makes this PC you to your friends and on their worlds. Save it once: after a reinstall or on a new PC, bring it back and your friends' worlds know you, with your characters there."),
      warn];
    const r = mk("div", "row");
    r.append(btn("Save my key…", "btn", () => run("peer_key_export", "The key wasn't saved. Try again."), "peerkey-save"),
      btn("Bring back my key…", "btn", () => { asking = true; paint(); }, "peerkey-load"));
    nodes.push(r);
    if (asking) {
      const c = mk("div", "peerkey-ask");
      c.setAttribute("role", "alertdialog");
      c.append(mk("p", "lede", "Bring back a saved key? This PC becomes that one to your friends (the key it has now is kept aside in Leyline's folder). Restart Leyline after."));
      const rr = mk("div", "row");
      rr.append(btn("Pick the saved file…", "btn primary", () => { asking = false; paint(); run("peer_key_import", "That key wasn't brought back. Nothing changed."); }, "peerkey-load-go"),
        btn("Cancel", "btn", () => { asking = false; paint(); }, "peerkey-load-no"));
      c.append(rr);
      nodes.push(c);
    }
    nodes.push(line);
    box.replaceChildren(...nodes);
  }
  paint();
  const host = about ? about.parentNode : page;
  if (about) host.insertBefore(box, about); else host.append(box);
  window.LeylinePeerKey = { paint };
})();
