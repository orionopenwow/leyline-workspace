// wu-loading.js — the world-loading cover on the WrathUnleashed page, and the loading numbers kept
// off screen. The look is wu-loading.css (the 1.12 Open Vanilla boot screen's), which also restyles
// boot.js's pre-login overlay; this module adds the same screen over the client's own world-loading
// screens and keeps what the overlays used to print.
//
// When the cover shows, from the client's progress stages (stages.js passes each on as a window
// 'ow:progress' event):
//   'enter'         the world session started (Enter World): up       (startup_pace.rs)
//   'world'         the first world frame the loading screen no longer covers: down
//   'loading:on'    any loading screen rising (teleports, instances, the hearthstone): up
//   'loading:off'   that loading screen cleared: down
// 'enter'/'world' are in the client's uncommitted startup_pace.rs; 'loading:on'/'off' are not sent
// by any client yet (BRIDGE-GAPS.md section 5). A client without them shows its own loading screen,
// as today. A cover never stays up more than three minutes.
//
// The bar: the client's own world-load fraction (boot.js's 'wenilla-cover' event, `cover:<pct>`)
// once it reports one, else the page's world prefetch count (boot.js's corner line "preparing
// world data — N/M", hidden by wu-loading.css), else a gliding segment. The line names what the
// load waits on in plain words (boot.js's 'wenilla-loading' event from the client's `load:<what>`:
// "Loading the ground…", "Loading buildings…"), never a number. The numbers go to the console once
// per phase and to window.wuLoading.lines (owdiag reports them).

const SAFETY_MS = 180000;
const FIRST_LINE = 'Entering the world…';

/** The cover's bar width from the client's percent: never empty, never full while it is up. */
export function coverWidth(percent) {
  const p = Number(percent);
  if (!Number.isFinite(p)) return null;
  return Math.max(8, Math.min(97, Math.round(p))) + '%';
}
const TAG = '[WrathUnleashed]';

/** "preparing world data — 1058/2603" -> { phase: 'preparing world data', done: 1058, total: 2603 } */
export function parseCount(text) {
  const m = /^\s*(.*?)\s*[—–-]+\s*(\d+)\s*\/\s*(\d+)\s*$/.exec(String(text || ''));
  if (!m) return null;
  return { phase: m[1], done: Number(m[2]), total: Number(m[3]) };
}

export function installLoading(opts = {}) {
  if (window.wuLoading) return window.wuLoading;
  const st = (window.wuLoading = { shown: 0, on: false, lines: [], last: null });
  const art = new URLSearchParams(location.search).get('loadart') || opts.art || '';
  if (art === 'orion') document.documentElement.classList.add('wu-art-orion');

  // -- the cover --
  const cover = document.createElement('div');
  cover.id = 'wu-loading';
  cover.setAttribute('role', 'status');
  cover.setAttribute('aria-live', 'polite');
  cover.innerHTML = `<div class="box"><div class="stage">${FIRST_LINE}</div><div class="bar pulse"><i></i></div></div>`;
  const bar = cover.querySelector('.bar'), fill = cover.querySelector('.bar > i');
  const stage = cover.querySelector('.stage');
  // wu-loading.css draws the line from data-line (its ::after); the text node keeps it for readers.
  const setLine = (text) => {
    stage.textContent = text || FIRST_LINE;
    if (text) stage.dataset.line = text; else delete stage.dataset.line;
  };
  // The client's own fraction, once it has sent one for this cover: it wins over the prefetch count.
  let measured = false;
  let safety = 0;
  const busy = () => document.body.classList.toggle('wu-loading', st.on || !!document.getElementById('wenilla-boot'));
  function show(why) {
    if (!cover.isConnected) document.body.append(cover);
    if (!st.on) { st.shown += 1; console.info(`${TAG} loading cover up (${why})`); }
    if (!st.on) { measured = false; setLine(null); }
    st.on = true;
    if (!measured) { bar.classList.add('pulse'); fill.style.width = ''; }
    cover.classList.add('on');
    clearTimeout(safety);
    safety = setTimeout(() => hide('safety'), SAFETY_MS);
    busy();
  }
  function hide(why) {
    if (!st.on) return;
    st.on = false;
    clearTimeout(safety);
    cover.classList.remove('on');
    console.info(`${TAG} loading cover down (${why})`);
    busy();
  }
  addEventListener('ow:progress', (e) => {
    const s = String(e.detail || '');
    if (s === 'enter' || s === 'loading:on') show(s);
    else if (s === 'world' || s === 'loading:off') hide(s);
  });
  st.show = show;
  st.hide = hide;
  // What the load waits on (boot.js 'wenilla-loading'), and the client's real fraction
  // (boot.js 'wenilla-cover', `cover:<pct>`).
  addEventListener('wenilla-loading', (e) => {
    const text = e.detail && e.detail.text;
    st.what = (e.detail && e.detail.what) || null;
    if (text) setLine(text);
  });
  addEventListener('wenilla-cover', (e) => {
    const d = e.detail || {};
    if (!d.active || !st.on) return;
    const w = coverWidth(d.percent);
    if (!w) return;
    measured = true;
    bar.classList.remove('pulse');
    fill.style.width = w;
  });

  // -- the numbers: off screen, into the console and window.wuLoading --
  const seen = new Map();
  function readCorner() {
    const el = document.getElementById('wenilla-corner');
    const c = parseCount(el?.textContent);
    if (!c) return;
    st.last = c;
    if (!seen.has(c.phase)) {
      seen.set(c.phase, true);
      st.lines.push({ ...c, at: Math.round(performance.now()) });
      console.info(`${TAG} ${c.phase}: ${c.done}/${c.total}`);
    } else {
      const row = st.lines.findLast?.((r) => r.phase === c.phase);
      if (row) { row.done = c.done; row.total = c.total; }
    }
    // measured progress for the cover's bar while it is up
    if (st.on && !measured && c.total > 0) {
      bar.classList.remove('pulse');
      fill.style.width = Math.max(8, Math.min(100, Math.round((100 * c.done) / c.total))) + '%';
    }
  }
  try {
    const mo = new MutationObserver(() => { readCorner(); busy(); });
    mo.observe(document.body, { subtree: true, childList: true, characterData: true });
  } catch { /* no observer: the numbers just aren't kept */ }
  busy();
  return st;
}
