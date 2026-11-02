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
// The site and relay that go with this game page (docs/DOMAIN-MOVE.md; the names: tools/domain/addresses.json): the
// game on play.leylinelauncher.com talks to leylinelauncher.com, on its old name (wrath.openvanilla.app, and anywhere
// else) to play.openvanilla.app. Same backends either way; the sign-in stays on the name it was made on.
const NEW_GAME_HOST = 'play.leylinelauncher.com';
const onNewName = () => { try { return location.hostname === NEW_GAME_HOST; } catch (_) { return false; } };
const TIP_MS = 7000;

// One short tip at a time under the bar, on every loading screen (owner 31 Oct 23:51: "all loading screens the
// same … our spinning leyline logo and loadbar, maybe relevant tips float"). Plain words, no brand names.
export const TIPS = [
  'Type /realms to see every realm on the network and hop between them.',
  'Your Hero Key signs you in on any realm in the network. No password to remember.',
  'Build in your Den: drop a stamp like the Inn, then move, turn and undo each piece.',
  'Type /shot to hide the interface for a clean screenshot.',
  'What loads once stays in your browser, so the next visit is faster.',
  'Ideas, questions or a realm to share? The forum is open at ' + (onNewName() ? 'leylinelauncher.com' : 'play.openvanilla.app') + '.',
  'Host your own realm from the Leyline launcher in three steps.',
];

/** Put a rotating tip line in `box` (once); it stops when the box leaves the page. */
export function addTips(box, start = Math.floor(Math.random() * TIPS.length)) {
  if (!box || box.querySelector(':scope > .ly-tip')) return null;
  const tip = document.createElement('div');
  tip.className = 'ly-tip';
  let i = start % TIPS.length;
  tip.textContent = TIPS[i];
  box.append(tip);
  const t = setInterval(() => {
    if (!tip.isConnected) { clearInterval(t); return; }
    tip.classList.add('out');
    setTimeout(() => { i = (i + 1) % TIPS.length; tip.textContent = TIPS[i]; tip.classList.remove('out'); }, 400);
  }, TIP_MS);
  return tip;
}

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
  addTips(cover.querySelector('.box'));
  // wu-loading.css draws the line from data-line (its ::after); the text node keeps it for readers.
  const setLine = (text) => {
    // unchanged: touch nothing (a write is a DOM mutation, and the page-wide observer below calls back in here)
    if ((text || null) === (stage.dataset.line || null) && stage.textContent === (text || FIRST_LINE)) return;
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
  // -- the client's own cover (#wenilla-cover: grey words, a yellow bar, drawn over everything by newer clients):
  // while it is up, ours is up instead with its words and its progress, and it is hidden under ours (wu-loading.css,
  // body.wu-client-cover). Without this module the client's cover shows as before.
  let ccWatch = null, ccEl = null, viaClient = false;
  function clientCoverUp(cc) {
    if (!cc || !cc.isConnected) return false;
    const s = cc.style;
    return s.display !== 'none' && s.visibility !== 'hidden' && s.opacity !== '0';
  }
  function mirrorClient() {
    const cc = document.getElementById('wenilla-cover');
    if (cc !== ccEl) {
      if (ccWatch) ccWatch.disconnect();
      ccEl = cc; ccWatch = null;
      if (cc) {
        try {
          ccWatch = new MutationObserver(mirrorClient);
          ccWatch.observe(cc, { attributes: true, attributeFilter: ['style', 'class'], subtree: true, childList: true, characterData: true });
        } catch { /* no observer: the page-wide one still notices it come and go */ }
      }
    }
    if (clientCoverUp(cc)) {
      if (!viaClient) { viaClient = true; document.body.classList.add('wu-client-cover'); }
      show('client cover');
      const label = cc.querySelector('.label');
      // the client's own notice over its cover ("Still loading the building under you…": a bare black box, top 18%):
      // its words become the line, the box stays under ours (wu-loading.css .wu-under)
      const note = [...document.body.children].find((d) => d.tagName === 'DIV' && !d.id && d.style.zIndex === '2147482000');
      if (note && !note.classList.contains('wu-under')) note.classList.add('wu-under');
      const words = (note && note.isConnected && note.style.display !== 'none' && note.textContent.trim())
        || (label && label.textContent.trim());
      if (words) setLine(words);
      const fi = cc.querySelector('i');
      const w = fi && coverWidth(parseFloat(fi.style.width));
      if (w) { measured = true; bar.classList.remove('pulse'); fill.style.width = w; }
    } else if (viaClient) {
      viaClient = false;
      document.body.classList.remove('wu-client-cover');
      hide('client cover gone');
    }
  }
  st.mirror = mirrorClient;
  // the boot overlay gets the same tips
  const bootTips = () => { const b = document.querySelector('#wenilla-boot .box'); if (b) addTips(b); };

  try {
    const mo = new MutationObserver(() => { readCorner(); mirrorClient(); bootTips(); busy(); });
    mo.observe(document.body, { subtree: true, childList: true, characterData: true });
  } catch { /* no observer: the numbers just aren't kept */ }
  mirrorClient(); bootTips();
  busy();
  return st;
}
