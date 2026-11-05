// stages.js — when each step of a visit happened on the WrathUnleashed page, and whether the first
// 30 s in the world stuttered. Ported from Open Vanilla's 1.12 page layer (realm/www/stages.js);
// installed first by wrath-page.js, read by window.owdiag (wu-diag.js).
//
// The 3.3.5a client's progress stages (window.__wenilla_progress(stage), benilla-app webprogress.rs
// and startup_pace.rs; an older client build sends only the first two):
//   'startup'       the first update's Startup pile is done
//   'ready'         the first glue screen is up (login or the character list), once per page load
//   'enter'         the world session started: the loading screen is up
//   'world'         the first world frame the loading screen no longer covers
//   'reconnecting', 'reconnect:<s>', 'focus:<adt>'   (not marks: passed on as events only)
//
// Marks (ms since navigation; performance marks "ow:stage:<name>"):
//   engine-start   'startup'
//   engine         'ready'
//   login          the login screen (bridge state)
//   charlist       the character list (bridge state 'charselect')
//   world-enter    'enter', or the bridge state turning 'inworld' (older client)
//   world-ready    'world'
//   first-frame    the first animation frame after world-ready (after world-enter, older client)
//   first-move     the first snapshot after world-ready with the player moving (bridge)
//
// Every stage is also sent as a window 'ow:progress' event, with the 1.12 names touch.js and the
// page layer listen for added: 'world-loading' with 'enter', and 'world-ready' with 'world'. Nothing is shown on screen: the marks, the boot overlay's texts and
// the frame-gap summary go to the console and window.owStages.

const HITCH_MS = 100;
const WATCH_MS = 30000;
const TAG = '[WrathUnleashed]';

/** The marks and 1.12-style events a client stage stands for. */
export function stageMap(stage) {
  switch (stage) {
    case 'startup': return { mark: 'engine-start', events: [] };
    case 'ready': return { mark: 'engine', events: [] };
    case 'enter': return { mark: 'world-enter', events: ['world-loading'] };
    case 'world': return { mark: 'world-ready', events: ['world-ready'] };
    default: return { mark: null, events: [] };
  }
}

// Warm by destination, phase 0 (review 4 Nov, stream.md "Plan: warm by destination"): character select reports
// the ADT under the selected character ('focus:<adt>'), and boot.js warms the streamed tier that holds it. A
// brand-new character of most races stands on its race's own start tile, which no tier holds, so it entered cold
// and was then moved (ov_slices.lua, mini.json starter notes) to a cold Northshire or Valley of Trials. These seven
// tiles stand for the starter tier the realm moves them to (its home ADT, mini.json "home"; test_stream_review.py
// keeps the two in step). Only on a streamed slice: own files and community-realm hops warm nothing of ours.
const NORTHSHIRE = ['Azeroth', 32, 48];
const VALLEY = ['Kalimdor', 39, 33];
export const DEST_TILES = {
  'azeroth_31_43': NORTHSHIRE,      // dwarf, gnome: Coldridge Valley
  'kalimdor_30_12': NORTHSHIRE,     // night elf: Shadowglen
  'expansion01_58_39': NORTHSHIRE,  // draenei: Ammen Vale
  'azeroth_28_28': VALLEY,          // undead: Deathknell
  'kalimdor_32_37': VALLEY,         // tauren: Camp Narache
  'expansion01_43_12': VALLEY,      // blood elf: Sunstrider Isle
};
// (human Azeroth_32_48 and orc / troll Kalimdor_39_33 are the homes themselves; Death Knights start in the
// streamed Ebon Hold.)

// Warm by destination for an instant boss fight (stream/bossfights.py, docs/BOSS-FIGHTS.md): a session that came with
// ?go=bf:<boss>:... is taken from character select straight to the boss, so character select's focus becomes the
// fight's own tile and boot.js warms the tier that holds it (frozen-throne: mini.json's icc-toc tier, which has the
// whole of map 631) while the player is still picking a character. The tile is the platform's centre (631: 503.63,
// -2124.66), as benilla-wdt world_to_tile (ahead.js tileOf) gives it; the map folder as Map.dbc spells it.
// Only 'focus:' (character select): 'port:' is the realm's real teleport and is left alone.
export const BOSS_TILES = {
  'frozen-throne': ['IcecrownCitadel', 35, 31],
};

/** The boss of a boss-fight go ({ kind: 'bf', id: 'frozen-throne:normal:bots' }, or the string 'bf:...'), else null. */
export function bossOf(go) {
  let kind = null, id = null;
  if (typeof go === 'string') { const i = go.indexOf(':'); kind = go.slice(0, i); id = go.slice(i + 1); }
  else if (go && typeof go === 'object') { kind = go.kind; id = go.id; }
  if (kind !== 'bf' || typeof id !== 'string') return null;
  const m = /^([a-z0-9-]{2,24}):(?:normal|heroic):(?:bots|group|group-only)$/.exec(id);
  return m && Object.hasOwn(BOSS_TILES, m[1]) ? m[1] : null;
}

/** The page's data-saving rules (ahead.js): Save-Data on, or a 2g/3g connection, warms nothing extra. */
export function dataSaving(c) {
  return !!(c && (c.saveData || ['slow-2g', '2g', '3g'].includes(c.effectiveType)));
}

/** A streamed session that warms our slice: env.slice set, not a hop to another realm, not own files only. */
export function streamedSlice(env) {
  return !!(env && env.slice && !env.hop && env.mode !== 'own');
}

/**
 * The stage boot.js should see: 'focus:<adt>' / 'port:<adt>' on an unstreamed racial start tile becomes the same
 * stage on the destination tier's home ADT, written the same way (separator and folder kept). Anything else, or a
 * session that isn't streamed, comes back unchanged.
 */
export function destFocus(stage, env, go = null, conn = null) {
  const s = String(stage);
  const m = /^(focus|port):(.*)$/.exec(s);
  if (!m || !streamedSlice(env)) return s;
  const adt = m[2];
  const t = /(^|[\\/])(?:([A-Za-z0-9]+)([\\/]))?([A-Za-z0-9]+)_(\d+)_(\d+)(\.adt)?$/i.exec(adt);
  if (!t) return s;
  const [, lead, folder, sep, map, x, y, ext] = t;
  const boss = m[1] === 'focus' ? bossOf(go) : null;
  const dest = boss && !dataSaving(conn) ? BOSS_TILES[boss] : DEST_TILES[`${map}_${Number(x)}_${Number(y)}`.toLowerCase()];
  if (!dest) return s;
  const [dm, dx, dy] = dest;
  const head = adt.slice(0, t.index) + lead;
  return `${m[1]}:${head}${folder ? dm + sep : ''}${dm}_${dx}_${dy}${ext || ''}`;
}

export function installStages(wenilla) {
  if (window.owStages) return window.owStages;
  const marks = {};
  const hitches = [];
  const boot = [];
  const st = (window.owStages = { marks, hitches, boot, frames: 0, worstFrameMs: 0, summary });
  const fire = (detail) => { try { window.dispatchEvent(new CustomEvent('ow:progress', { detail })); } catch { /* old browser */ } };
  const mark = (name, at = performance.now()) => {
    if (name in marks) return;
    marks[name] = Math.round(at);
    try { performance.mark('ow:stage:' + name, { startTime: at }); } catch { /* old browser */ }
    console.info(`${TAG} ${name} at ${marks[name]} ms`);
    if (name === 'world-ready' || (name === 'world-enter' && !watchStarted)) startWatch(name);
  };

  // The engine's progress hook: boot.js assigns window.__wenilla_progress; keep its function and
  // see every stage first, whichever of the two installs first.
  let inner = window.__wenilla_progress;
  let inWorldLoad = false;     // between 'world-loading' and 'world-ready' (each world entry)
  const hook = (stage) => {
    const s = String(stage);
    const m = stageMap(s);
    if (m.mark) mark(m.mark);
    fire(s);
    for (const ev of m.events) {
      if (ev === 'world-loading') { if (inWorldLoad) continue; inWorldLoad = true; }
      if (ev === 'world-ready') inWorldLoad = false;
      fire(ev);
    }
    if (s === 'world') afterFrame(() => mark('first-frame'));
    if (typeof inner === 'function') {
      // boot.js warms what it is told: a new character's start tile becomes its destination (destFocus above);
      // the page's own marks and events keep the stage as the client sent it
      const told = s.startsWith('focus:') || s.startsWith('port:')
        ? destFocus(s, window.__wenilla_env, window.wuGo, typeof navigator !== 'undefined' ? navigator.connection : null) : stage;
      if (told !== stage && told !== s) console.info(`${TAG} warm by destination: ${s} -> ${told}`);
      return inner(told === s ? stage : told);
    }
    return undefined;
  };
  try {
    Object.defineProperty(window, '__wenilla_progress', {
      configurable: true,
      get: () => hook,
      set: (f) => { if (f !== hook) inner = f; },
    });
  } catch { /* a non-configurable global: stages fall back to the bridge */ }

  // The bridge: screen states and the first movement.
  let readyPos = null;
  wenilla?.on?.('state', (s) => {
    const name = s?.state;
    if (name === 'login') mark('login');
    if (name === 'charselect') mark('charlist');
    // an older client has no 'enter': the bridge's state stands in for the mark (not for the
    // 'world-loading' event: with no 'world' after it, the touch controls would stay hidden)
    if (name === 'inworld' && !('world-enter' in marks)) { mark('world-enter'); afterFrame(() => markLate('first-frame')); }
  });
  wenilla?.on?.('frame', (snap) => {
    if (!('world-ready' in marks) || 'first-move' in marks) return;
    const p = snap?.self?.pos;
    if (!p) return;
    readyPos ??= p;
    if (snap.self.moving || Math.hypot(p[0] - readyPos[0], p[1] - readyPos[1]) > 0.5) mark('first-move');
  });
  function markLate(name) { if (!('world-ready' in marks)) mark(name); }

  // The boot overlay's lines (download percent, data counts) are hidden on screen (wrath-page.js);
  // their phases are kept here instead, one entry per phase, with the last text seen.
  watchBootOverlay(boot);

  // The frame-gap watch.
  let watchStarted = false;
  function startWatch(from) {
    watchStarted = true;
    const t0 = performance.now();
    let last = t0;
    const tick = (t) => {
      const gap = t - last;
      last = t;
      st.frames += 1;
      if (gap > st.worstFrameMs) st.worstFrameMs = Math.round(gap);
      if (gap > HITCH_MS) hitches.push({ at: Math.round(t - t0), ms: Math.round(gap), from });
      if (t - t0 < WATCH_MS) requestAnimationFrame(tick);
      else console.info(`${TAG} first ${WATCH_MS / 1000}s after ${from}: ${st.frames} frames, ${hitches.length} hitches over ${HITCH_MS} ms (worst ${st.worstFrameMs} ms)`);
    };
    requestAnimationFrame(tick);
  }

  function summary() {
    const d = (a, b) => (marks[a] != null && marks[b] != null ? marks[b] - marks[a] : null);
    return {
      marks: { ...marks },
      spans: {
        boot: marks.engine ?? null,
        startup: d('engine-start', 'engine'),
        toCharlist: marks.charlist ?? null,
        worldLoad: d('world-enter', 'world-ready'),
        toFirstFrame: d('world-enter', 'first-frame'),
        toFirstMove: d('world-ready', 'first-move'),
      },
      boot: boot.slice(),
      hitches: hitches.length,
      worstFrameMs: st.worstFrameMs,
    };
  }
  return st;
}

/** Follow #wenilla-boot's text lines (boot.js) into `out`: [{ phase, text, at }]. */
function watchBootOverlay(out) {
  const phaseOf = (t) => t.split(/\s+[—-]\s+|\s+\d/)[0].trim();
  const seen = new Map();
  const read = () => {
    const el = document.getElementById('wenilla-boot');
    if (!el) return;
    for (const sel of ['.stage', '.data', '.err']) {
      const t = el.querySelector(sel)?.textContent?.trim();
      if (!t) continue;
      const phase = sel + ' ' + phaseOf(t);
      const row = seen.get(phase);
      if (row) { row.text = t; continue; }
      const r = { phase, text: t, at: Math.round(performance.now()) };
      seen.set(phase, r);
      out.push(r);
      console.info(`${TAG} boot: ${t}`);
    }
  };
  try {
    const mo = new MutationObserver(read);
    mo.observe(document.documentElement, { subtree: true, childList: true, characterData: true });
    setTimeout(() => mo.disconnect(), 180000);
  } catch { /* no observer */ }
}

function afterFrame(fn) {
  requestAnimationFrame(() => requestAnimationFrame(() => fn()));
}
