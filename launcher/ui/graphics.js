// Mods & Addons › Discover › Graphics (owner 31 Oct 22:58: "add graphics tab for gfx tweaks"): the game client's own
// graphics settings (src-tauri/src/gfx.rs: WTF/Config.wtf), a preset in one click or each setting by hand, Save, and
// "Back to how it was". The tab itself is themes.js's (Mods | Addons | Themes | Graphics); this file fills
// #graphics-discover when it is shown. Graphics mods (HD textures, shaders) stay under Mods.
(() => {
  "use strict";
  const invoke = window.__TAURI__ && window.__TAURI__.core ? window.__TAURI__.core.invoke : null;
  const el = (tag, cls, text) => {
    const n = document.createElement(tag);
    if (cls) n.className = cls;
    if (text !== undefined && text !== null) n.textContent = text;
    return n;
  };
  let client = "";      // the client's id
  let st = null;        // gfx_get's answer
  let draft = {};       // changed values, not saved yet
  let note = ["", ""];

  function clients() {
    const s = document.getElementById("play-client");
    return s ? Array.from(s.options).filter((o) => o.value).map((o) => ({ id: o.value, name: o.textContent })) : [];
  }
  function friendly(e) {
    return window.LeylineFriendly && window.LeylineFriendly.say ? window.LeylineFriendly.say(e) : String(e);
  }

  async function load(box) {
    const list = clients();
    if (!client || !list.some((c) => c.id === client)) {
      const sel = document.getElementById("play-client");
      client = (sel && sel.value) || (list[0] && list[0].id) || "";
    }
    st = null; draft = {};
    if (client && invoke) {
      try { st = await invoke("gfx_get", { client }); } catch (e) { note = [friendly(e), "bad"]; }
    }
    render(box);
  }

  function valueOf(t) { return t.key in draft ? draft[t.key] : (st.values[t.key] ?? t.default); }

  function control(t) {
    const row = el("div", "gfx-row");
    const lab = el("label", "gfx-lab");
    const id = "gfx-" + t.key;
    lab.htmlFor = id;
    lab.append(el("b", "", t.label), el("span", "gfx-help", t.help));
    row.append(lab);
    const v = String(valueOf(t));
    let input;
    if (t.kind === "toggle") {
      input = el("input", "gfx-toggle");
      input.type = "checkbox";
      input.checked = v === "1";
      input.addEventListener("change", () => { draft[t.key] = input.checked ? "1" : "0"; dirty(); });
      row.append(input);
    } else if (t.kind === "choice") {
      input = el("select", "gfx-select");
      for (const [val, word] of t.choices) {
        const o = el("option", "", word);
        o.value = val;
        if (val === v) o.selected = true;
        input.append(o);
      }
      input.addEventListener("change", () => { draft[t.key] = input.value; dirty(); });
      row.append(input);
    } else {
      const wrap = el("div", "gfx-slide");
      input = el("input", "gfx-range");
      input.type = "range";
      input.min = t.min; input.max = t.max; input.step = t.step; input.value = v;
      const out = el("output", "gfx-val", v);
      input.addEventListener("input", () => { out.textContent = input.value; draft[t.key] = input.value; dirty(); });
      wrap.append(input, out);
      row.append(wrap);
    }
    input.id = id;
    return row;
  }

  function dirty() {
    const n = Object.keys(draft).length;
    const save = document.getElementById("gfx-save");
    if (save) { save.disabled = n === 0; save.textContent = n ? `Save ${n} change${n === 1 ? "" : "s"}` : "Saved"; }
  }

  function render(box) {
    box.textContent = "";
    box.append(el("p", "gfx-lede", "Your game's own graphics settings, without opening the game. A preset in one click, or each setting by hand. Changes take effect the next time the game starts."));
    const list = clients();
    if (!list.length) {
      box.append(el("p", "gfx-empty", "Add your game first: drop your game folder on the realm box below, then come back here."));
      return;
    }
    const head = el("div", "gfx-head");
    const pick = el("select", "gfx-client");
    pick.id = "gfx-client";
    pick.setAttribute("aria-label", "Game client");
    for (const c of list) { const o = el("option", "", c.name); o.value = c.id; if (c.id === client) o.selected = true; pick.append(o); }
    pick.addEventListener("change", () => { client = pick.value; load(box); });
    head.append(el("span", "gfx-for", "Settings for"), pick);
    box.append(head);
    if (!st) { box.append(el("p", "gfx-empty", note[0] || "Reading the game's settings…")); return; }
    if (!st.tweaks.length) {
      box.append(el("p", "gfx-empty", "This game version keeps its graphics in the game's own Video options (Esc › Video)."));
      return;
    }
    const pre = el("div", "gfx-presets");
    pre.append(el("span", "gfx-step", "1"), el("b", "", "Pick a preset"));
    const btns = el("div", "gfx-pbtns");
    for (const [id, name] of st.presets) {
      const b = el("button", "gfx-preset", name);
      b.type = "button";
      b.dataset.preset = id;
      b.addEventListener("click", async () => {
        try { st = await invoke("gfx_preset", { client, preset: id }); draft = {}; note = [`${name} saved. It takes effect the next time the game starts.`, "ok"]; }
        catch (e) { note = [friendly(e), "bad"]; }
        render(box);
      });
      btns.append(b);
    }
    pre.append(btns);
    box.append(pre);
    const each = el("div", "gfx-each");
    each.append(el("span", "gfx-step", "2"), el("b", "", "Or fine-tune each setting"));
    const grid = el("div", "gfx-grid");
    for (const t of st.tweaks) grid.append(control(t));
    each.append(grid);
    box.append(each);
    const foot = el("div", "gfx-foot");
    const save = el("button", "gfx-save", "Saved");
    save.type = "button"; save.id = "gfx-save"; save.disabled = true;
    save.addEventListener("click", async () => {
      try { st = await invoke("gfx_set", { client, values: draft }); draft = {}; note = ["Saved. It takes effect the next time the game starts.", "ok"]; }
      catch (e) { note = [friendly(e), "bad"]; }
      render(box);
    });
    foot.append(save);
    if (st.has_backup) {
      const back = el("button", "gfx-back", "Back to how it was");
      back.type = "button"; back.id = "gfx-back";
      back.title = "Puts back the game's settings from before Leyline first changed them";
      back.addEventListener("click", async () => {
        try { st = await invoke("gfx_restore", { client }); draft = {}; note = ["Back to how it was before Leyline changed it.", "ok"]; }
        catch (e) { note = [friendly(e), "bad"]; }
        render(box);
      });
      foot.append(back);
    }
    const msg = el("p", "gfx-note " + (note[1] || ""), note[0]);
    msg.id = "gfx-note"; msg.setAttribute("role", "status");
    foot.append(msg);
    box.append(foot);
    box.append(el("p", "gfx-more", "Graphics mods (HD textures, shaders) are on the Mods tab."));
  }

  window.LeylineGraphics = { show: (box) => { note = ["", ""]; return load(box); } };
})();
