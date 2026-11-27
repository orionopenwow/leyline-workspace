// Leyline: Settings › Network (p2p/link.rs, docs/leyline/joining.md). How this PC reaches friends from anywhere by
// their key, with no relay of ours: the main switch, "Help friends connect" (pass sealed bytes between two of your
// friends, capped per day), letting the router open a port, community relays, and Be a node's relay for other
// players. Counts only: never a key or an address. Text only (textContent). An older core without the commands
// hides the section.
"use strict";
(() => {
  const call = window.__TAURI__ && window.__TAURI__.core
    ? window.__TAURI__.core.invoke
    : async () => { throw "This is a preview page: open it in the Leyline app."; };
  const oops = (e, fb) => (window.LeylineFriendly ? window.LeylineFriendly(e, fb) : (typeof e === "string" ? e : (fb || "Something went wrong. Try again.")));
  const missing = (e) => /not found|unknown (command )?/i.test(String(e || ""));
  const mk = (tag, cls, text) => { const e = document.createElement(tag); if (cls) e.className = cls; if (text !== undefined) e.textContent = text; return e; };
  const page = document.getElementById("page-settings");
  if (!page) return;

  const box = mk("section", "set-network");
  box.id = "set-network";
  box.setAttribute("aria-label", "Network");
  const line = mk("p", "status");
  line.setAttribute("role", "status");
  let view = null;

  function toggle(key, label, hint) {
    const l = mk("label", "row");
    const c = document.createElement("input");
    c.type = "checkbox";
    c.checked = !!view.settings[key];
    c.id = "net-" + key;
    c.addEventListener("change", () => save({ [key]: c.checked }));
    l.append(c, mk("span", "", label));
    const wrap = mk("div", "net-opt");
    wrap.append(l);
    if (hint) wrap.append(mk("p", "lede", hint));
    return wrap;
  }

  async function save(change) {
    try {
      view = await call("link_set", { settings: Object.assign({}, view.settings, change) });
      line.textContent = "Saved.";
      line.className = "status ok";
    } catch (e) {
      if (missing(e)) { box.hidden = true; return; }
      line.textContent = oops(e, "That wasn't saved. Try again.");
      line.className = "status bad";
    }
    paint();
  }

  function paint() {
    if (!view) return;
    const s = view.settings;
    const nodes = [mk("h3", "", "Network"),
      mk("p", "lede", "Your launcher reaches friends' launchers by their key, from anywhere: on the same network, straight across, by hole punching, or through a friend you both know. Everything is sealed with your two keys; nothing of Leyline's sits in between.")];
    nodes.push(toggle("on", "Reach friends from anywhere", null));
    if (s.on) {
      nodes.push(toggle("help_friends", "Help friends connect", "When two of your friends can't reach each other, your launcher passes their sealed bytes along while you're online. It can't read them."));
      if (s.help_friends) {
        const r = mk("label", "row");
        const n = document.createElement("input");
        n.type = "number"; n.min = "16"; n.max = "1000000"; n.step = "64"; n.value = String(s.help_day_mb); n.id = "net-help-mb";
        n.addEventListener("change", () => save({ help_day_mb: Math.max(16, parseInt(n.value, 10) || 16) }));
        r.append(mk("span", "", "Most data passed for friends per day (MB): "), n);
        nodes.push(r);
      }
      nodes.push(toggle("router_port", "Let my router open a port for Leyline", "Uses your router's automatic port opening (UPnP, NAT-PMP or PCP). Friends then reach you directly more often. Takes effect next time Leyline starts."));
      nodes.push(toggle("community", "Use community relays when nothing else works", "Relays run by players who chose to carry others. They pass sealed bytes only."));
      nodes.push(toggle("community_relay", "Carry sealed connections for other players", "Only while Be a node is on, and within its caps (data per day, connections at once)."));
      const st = [];
      st.push(view.running ? "On." : "Not running (open the Friends list, or share a world).");
      if (view.helped_today) st.push(`Passed for friends today: ${Math.round(view.helped_today / 1048576)} MB.`);
      if (view.refused) st.push(`Refused by your caps: ${view.refused}.`);
      if (view.held) st.push(`Messages kept for friends who are offline: ${view.held}.`);
      if (view.community_relay_running) st.push("Carrying for other players now.");
      for (const [who, way] of view.ways || []) st.push(`${who}: ${way}.`);
      nodes.push(mk("p", "lede", st.join(" ")));
    }
    nodes.push(line);
    box.replaceChildren(...nodes);
  }

  async function load() {
    try {
      view = await call("link_state");
    } catch (e) {
      if (missing(e)) { box.hidden = true; return; }
      line.textContent = oops(e, "Network settings couldn't be read.");
    }
    paint();
  }

  const key = document.getElementById("set-peerkey");
  if (key && key.parentNode) key.parentNode.insertBefore(box, key);
  else page.append(box);
  load();
})();
