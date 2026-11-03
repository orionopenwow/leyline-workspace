// The sign-in cover's page (cover.html; src-tauri/src/cover.rs). It asks the core what to show a few times a second
// (cover_state: { id, title, steps, step, shown }) and draws the heading and the steps: those before the current one
// done, the current one lit. "Show the game" or Esc hides the cover at once (cover_hide, for this start only). The
// core places and hides the window itself; this page only draws.
(function () {
  'use strict';
  const POLL_MS = 250;
  const $ = (id) => document.getElementById(id);
  const invoke = (cmd, args) => window.__TAURI__.core.invoke(cmd, args);
  let id = 0;
  let drawn = '';

  function render(v) {
    if (!v || typeof v !== 'object') return;
    id = Number(v.id) || 0;
    if (typeof v.title === 'string' && v.title) $('cv-title').textContent = v.title;
    const steps = Array.isArray(v.steps) ? v.steps.map(String) : [];
    const step = Math.max(0, Math.min(Number(v.step) || 0, Math.max(steps.length - 1, 0)));
    const key = JSON.stringify([steps, step]);
    if (key === drawn) return;
    drawn = key;
    const list = $('cv-steps');
    list.replaceChildren(...steps.map((name, i) => {
      const li = document.createElement('li');
      li.className = 'cv-step ' + (i < step ? 'done' : i === step ? 'now' : 'next');
      li.textContent = name;
      if (i === step) li.setAttribute('aria-current', 'step');
      return li;
    }));
    // the line fills to the middle of the current step
    const pct = steps.length ? Math.round(((step + 0.5) / steps.length) * 100) : 12;
    $('cv-fill').style.width = pct + '%';
  }

  async function poll() {
    try { render(await invoke('cover_state')); } catch (_) { /* the core is busy: the next look */ }
    setTimeout(poll, POLL_MS);
  }

  function showGame() {
    Promise.resolve().then(() => invoke('cover_hide', { id })).catch(() => {});
  }

  $('cv-show').addEventListener('click', showGame);
  document.addEventListener('keydown', (e) => { if (e.key === 'Escape') { e.preventDefault(); showGame(); } });
  window.LeylineCover = { render };
  poll();
})();
