// Leyline feature videos: shared bits for the recordings (tools/video/record.cjs). A fake Tauri core for launcher/ui (the
// same shape as tools/art/launcher-shots.cjs and the launcher/tests fakes), a soft cursor drawn in the page with a ripple
// on each click, eased mouse moves, and a CDP screencast recorder that keeps every frame with its time (assemble.py
// turns those into a steady 30 fps clip).
'use strict';
const fs = require('node:fs');
const path = require('node:path');

const ROOT = path.resolve(__dirname, '..', '..');
const UI = path.join(ROOT, 'launcher', 'ui');
const INDEX = 'file://' + path.join(UI, 'index.html');
// slow motion: the page's animations run SLOW times slower and every wait and move here is SLOW times longer; the
// screencast then has SLOW times more frames per moment, and assemble.py plays it back at speed
const SLOW = Number(process.env.SLOW || 4);
const BRANDS = /world of warcraft|\bwow\b|warcraft|lich king|blizzard|wotlk/i;

function modes() {
  const cat = JSON.parse(fs.readFileSync(path.join(ROOT, 'stream', 'modepacks.json'), 'utf8'));
  return cat.modes.map((m) => ({ id: m.id, name: m.name, line: m.line, version: m.version, author: m.author, based_on: m.based_on,
    status: m.status, profile: m.profile, scripts: m.scripts, env: m.env, mods: m.mods, den: m.den, tags: m.tags, jam: null, credit: m.credit || null,
    settings: m.settings, picture: m.picture_data || null, own: false }));
}

// ---------------------------------------------------------------- the fake core (runs in the page)
const FAKE = (o) => {
  window.LEYLINE_HERO_MS = 1e9;   // the Home gallery stays on its first slide
  const calls = (window.__calls = []);
  window.__unknown = [];
  try {
    localStorage.clear();
    localStorage.setItem('leyline.page', o.page || 'play');
    localStorage.setItem('leyline.mode', 'online');
    localStorage.setItem('leyline.joined', JSON.stringify(['relay.leylinelauncher.com:40002']));
    for (const [k, v] of Object.entries(o.ls || {})) localStorage.setItem(k, v);
  } catch (_) { /* fine */ }
  const clients = [
    { id: 'c335a-1', name: 'Northrend', path: 'C:\\Games\\Northrend', build: '3.3.5a', realmlist: '' },
    { id: 'c1121-1', name: 'Classic', path: 'C:\\Games\\Classic', build: '1.12.1', realmlist: '' },
    { id: 'c243-1', name: 'Outland', path: 'C:\\Games\\Outland', build: '2.4.3', realmlist: '' },
    { id: 'c434-1', name: 'Shattered World', path: 'C:\\Games\\Shattered', build: '4.3.4', realmlist: '' },
    { id: 'c548-1', name: 'Misty Isles', path: 'C:\\Games\\Misty Isles', build: '5.4.8', realmlist: '' },
  ];
  const RID = 'emberemberember2';
  const mode = (id) => { const m = o.modes.find((x) => x.id === id); return m ? { id: m.id, name: m.name, author: m.author, forkable: true, recipe: m } : null; };
  const baseRealms = [
    { slug: 'official', name: 'Wrath Unleashed', official: true, state: 'live', build: '3.3.5a', version: '3.3.5a', core: 'azerothcore', tags: [], players: 42, bots: 120, address: 'leylinelauncher.com', mods: [] },
    { slug: 'c-aaaa', realm_id: 'aaaaaaaaaaaaaaaa', name: 'Frost Haven', state: 'live', build: '3.3.5a', version: '3.3.5a', core: 'azerothcore', tags: ['pve'], players: 17, bots: 30, address: 'relay.leylinelauncher.com:40002', mods: [], tagline: 'A friendly levelling realm', mode: mode('levelling') },
    { slug: 'c-bbbb', realm_id: 'bbbbbbbbbbbbbbbb', name: 'Duel Town', state: 'live', build: '3.3.5a', version: '3.3.5a', core: 'azerothcore', tags: ['pvp'], players: 7, bots: 0, address: 'relay.leylinelauncher.com:40003', mods: [], mode: mode('duel-ladder') },
    { slug: 'c-dddd', realm_id: 'dddddddddddddddd', name: 'Putt Party', state: 'live', build: '3.3.5a', version: '3.3.5a', core: 'azerothcore', tags: ['fun'], players: 5, bots: 0, address: 'relay.leylinelauncher.com:40004', mods: [], mode: mode('minigolf-night') },
  ];
  let hosted = null;   // { realm_id, name, visibility, relay, profile } once Host finishes
  const realms = () => {
    const l = JSON.parse(JSON.stringify(baseRealms));
    if (hosted) l.splice(1, 0, { slug: 'c-' + RID, realm_id: RID, name: hosted.name, state: 'live', build: '3.3.5a', version: '3.3.5a', core: 'azerothcore',
      tags: ['pve'], players: 1, bots: 0, address: 'relay.leylinelauncher.com:40077', mods: [], mode: mode('levelling'), tagline: 'New on the network' });
    return l;
  };
  const S = { forgeOpen: false, forgeLooks: 0, forgeLast: null, jobAt: undefined, answers: null };
  const dens = [{ name: 'house', bytes: 300, props: 42, saved: '2026-11-01T01:00:00Z' }, { name: 'garden', bytes: 300, props: 18, saved: '2026-10-31T19:00:00Z' }];
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return { version: '0.9', settings: { clients, selected_client: 'c335a-1', selected_realm: 'leylinelauncher.com',
        wine: { enabled: false, command: 'wine', prefix: '' } }, official_realm: 'leylinelauncher.com', browser_url: 'https://leylinelauncher.com/wrath', windows: true, data_dir: 'C:\\Users\\me\\AppData\\Leyline' };
      case 'realms_list': { const r = realms(); return { realms: r, network: { realms: r.length - 1, players: r.reduce((a, x) => a + x.players, 0) - 42, bots: 150 }, from_cache: false, saved_at: 1, error: null }; }
      case 'realm_status': return { state: 'online', ping_ms: 24, players: 42 };
      case 'mods_catalog': return Object.assign({ from_cache: false, saved_at: 1, error: null }, o.catalog || { categories: [], projects: [] });
      case 'mods_ranked': return { ids: [], metrics: {}, from_cache: false, error: null };
      case 'mods_installed': return { client: 'c335a-1', build: '3.3.5a', mods: S.mods || [] };
      case 'mod_install': {
        const p = (o.catalog.projects || []).find((x) => x.id === args.project) || { id: args.project, name: 'Mod', version: '1' };
        S.mods = (S.mods || []).concat([{ key: p.id, name: p.name, version: p.version, source: p.source, update: null, in_catalog: true, enabled: true, files: [], addons: p.addons || [], patches: [] }]);
        await new Promise((r) => setTimeout(r, 700));
        return { key: p.id, name: p.name, version: p.version, warnings: [], kept: [] };
      }
      case 'mods_updates': return [];
      case 'themes_list': return { active: 'dark', themes: [] };
      case 'deep_link_take': return null;
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
      case 'play': await new Promise((r) => setTimeout(r, 400)); return 'Started Northrend on ' + ((args && args.realm) || 'Wrath Unleashed') + '.';
      case 'open_link': case 'open_url': case 'save_text_file': return null;
      // hosting (the Host page's templates)
      case 'host_info': return { dir: '/kit', is_kit: true, has_windows_script: true, has_unix_script: true, problems: [], seeds: [],
        realm: hosted || (o.dockerDen ? { realm_id: 'denden2denden2de', name: 'My Den', visibility: 'private', relay: 'relay.leylinelauncher.com', profile: 'den' } : null) };
      case 'host_modes': return JSON.parse(JSON.stringify(o.modes));
      case 'host_mode_use': if (!args.id) return null; { const m = o.modes.find((x) => x.id === args.id); return { profile: m.profile, name: m.name, den: null }; }
      case 'host_profiles': return [[{ id: 'main', label: 'x', version: '3.3.5a', core: 'azerothcore', in_kit: true, clients: [{ id: 'c335a-1', name: 'Northrend', path: '/g' }], note: null }], null];
      case 'host_choose': return { profile: args.profile, client_dir: '' };
      case 'host_start_quiet': S.jobAt = 0; S.answers = args.answers; return 'Setting up your realm.';
      case 'host_progress': {
        if (S.jobAt === undefined) return { idle: true, kind: '', running: false, code: null, step: '', n: 0, of: 0, secs: 0, problem: null };
        const n = Math.min(++S.jobAt, 3);
        const done = n >= 3;
        if (done && !hosted) hosted = { realm_id: RID, name: (S.answers && S.answers.name) || 'Ember Vale', visibility: 'public', relay: 'relay.leylinelauncher.com', profile: 'main' };
        return { idle: false, kind: 'host', running: !done, code: done ? 0 : null, step: done ? 'Done' : ['Getting the helper', 'Starting the servers'][n - 1], n, of: 3, secs: n * 30, problem: null };
      }
      case 'host_console': return { set_up: !!hosted, linked: false, name: hosted ? hosted.name : '', realm_id: hosted ? RID : '', profile: 'main', core: 'azerothcore',
        visibility: 'public', signup: 'open', herokey: false, claim_sent: false, browser: true, max_players: 0, relay: 'relay.leylinelauncher.com', relay_connected: true,
        relay_error: null, docker: true, services: o.dockerDen ? [{ role: 'world', state: 'ok' }] : [], uptime: '1 min', gm: true, gm_note: null, invite: 'https://relay.leylinelauncher.com/join/' + RID + '#a1b2c3' };
      case 'host_status': return { realm_id: args.realmId, online: true, name: hosted ? hosted.name : '', visibility: 'public', players: 1, bots: 0, listed: true };
      case 'host_invite': return 'https://relay.leylinelauncher.com/join/' + (o.denInvite ? 'denden2denden2de' : RID) + '#k7f3q9';
      case 'host_gm': return 'gm=1\nup=60\nplayers=0\nbots=0\nrestart=0\nallow=off\nallowed=0\nvis=public\nmotd=-';
      case 'host_scan': return null;
      case 'host_listing': return { tags: [], region: null, language: null, max_players: null, direct: false };
      case 'host_network': return null;
      case 'hostpack_info': return { installed: { version: '7' }, available: { version: '7' }, update: false, kit_dir: '/kit' };
      case 'docker_info': return { state: 'ready', version: '27', help: '', get_url: '' };
      case 'modes_list': return { modes: [], from_cache: false, builtin: false, saved_at: null, error: null };
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      // signed in (o.signed): your name, a portrait, friends with theirs
      case 'social_status': return !!o.signed;
      case 'social_ways': return { herokey: true, google: true, discord: true };
      case 'forum_call': return { threads: [], categories: [] };
      case 'social_call': {
        S.prof = S.prof || { bio: 'Builds small houses by the sea.', avatar: 'ranger', avatar_url: 'https://leylinelauncher.com/wrath/avatar/p/ranger.svg', setup: true };
        const me = () => ({ pid: 'kkkkkkkkkk', name: 'Kestrel', tag: '1290', main: '', show_main: false, offline: false, profile: Object.assign({}, S.prof) });
        if (args.action === 'me') return { me: me(), avatars: [], ways: { herokey: true, google: true, discord: true }, manage_url: 'https://leylinelauncher.com/wrath/profile' };
        if (args.action === 'profile') {
          const a = args.args;
          if ('avatar' in a) { S.prof.avatar = a.avatar; S.prof.avatar_url = 'https://leylinelauncher.com/wrath/avatar/p/' + a.avatar + '.svg'; }
          if ('bio' in a) S.prof.bio = a.bio;
          return { ok: true, me: me() };
        }
        if (args.action === 'state') return { v: 1, me: { pid: 'kkkkkkkkkk', name: 'Kestrel', tag: '1290' }, incoming: [], outgoing: [], blocked: [], friends: [
          { pid: 'bbbbbbbbbb', name: 'Bramble', tag: '4655', main: '', status: 'game', realm: 'Frost Haven', char: 'Thistle', unread: 0, profile: { avatar_url: 'https://leylinelauncher.com/wrath/avatar/p/druid.svg' } },
          { pid: 'cccccccccc', name: 'Wren', tag: '5521', main: '', status: 'launcher', realm: '', char: '', unread: 0, profile: { avatar_url: 'https://leylinelauncher.com/wrath/avatar/p/owl.svg' } },
          { pid: 'dddddddddd', name: 'Moss', tag: '3301', main: '', status: 'game', realm: 'Wrath Unleashed', char: 'Fennick', unread: 0, profile: { avatar_url: 'https://leylinelauncher.com/wrath/avatar/p/fox.svg' } },
          { pid: 'eeeeeeeeee', name: 'Juniper', tag: '7712', main: '', status: 'offline', realm: '', char: '', unread: 0, profile: { avatar_url: 'https://leylinelauncher.com/wrath/avatar/p/witch.svg' } },
        ] };
        return {};
      }
      case 'agents_state': return { allow: true, pending: [], audit: [], connections: { claude_code: 'none', desktop: 'none' }, command: 'leyline mcp', tools: {} };
      // Your Den
      case 'den_state': return { lite: { available: true, running: !!S.denUp, address: '127.0.0.1', account: 'DEN', password: 'den', den_dir: '/den' },
        kit: true, docker_den: !!o.dockerDen, other_realm: null, account: o.dockerDen ? 'KESTREL' : '', saves: o.dockerDen ? 'kit' : 'lite', dens, packs: [] };
      case 'den_play_lite': await new Promise((r) => setTimeout(r, 600)); S.denUp = true; return 'Started Northrend on 127.0.0.1.';
      case 'den_prefill_account': return 'KESTREL';
      case 'join_invite': return { address: '127.0.0.1:3724', name: 'My Den' };
      case 'den_open': return args.name;
      case 'den_text': return '{"format":"leyline-den","version":1,"props":[]}';
      case 'forge_state':
        if (S.forgeOpen && ++S.forgeLooks > 1) { S.forgeOpen = false; S.forgeLast = { seq: 1, ok: true, changed: true, chunks: 1, message: 'Terrain updated: 1 part of your Den changed. Start your game again to see it.' }; }
        return { installed: true, running: !!S.forgeOpen, exe: '', sha256: '', last: S.forgeLast, source: 'https://github.com/orionopenwow/leyline-workspace/tree/main/forge',
          licence: 'Forge is free software under the GNU GPL-3.0, built on Noggit Red by the Noggit contributors.' };
      case 'forge_open': S.forgeOpen = true; S.forgeLooks = 0; return null;
      default: window.__unknown.push(cmd); throw 'unknown ' + cmd;
    }
  } } };
};

// ---------------------------------------------------------------- the soft cursor (runs in the page)
const CURSOR = (o) => {
  const scale = o.scale || 1;
  const add = () => {
    if (document.getElementById('__cur')) return;
    const st = document.createElement('style');
    st.textContent = `
#__cur{position:fixed;left:0;top:0;width:0;height:0;z-index:2147483647;pointer-events:none;transition:opacity .25s}
#__cur .halo{position:absolute;left:${-22 * scale}px;top:${-22 * scale}px;width:${44 * scale}px;height:${44 * scale}px;border-radius:50%;
  background:radial-gradient(circle,rgba(255,236,190,.38) 0%,rgba(255,236,190,.16) 45%,rgba(255,236,190,0) 70%);transition:transform .18s ease}
#__cur.down .halo{transform:scale(.8)}
#__cur svg{position:absolute;left:${-3 * scale}px;top:${-2 * scale}px;width:${24 * scale}px;height:${30 * scale}px;filter:drop-shadow(0 ${2 * scale}px ${4 * scale}px rgba(0,0,0,.55))}
.__rip{position:fixed;z-index:2147483646;pointer-events:none;border-radius:50%;border:${2.5 * scale}px solid rgba(255,232,170,.95);
  width:${16 * scale}px;height:${16 * scale}px;margin:${-8 * scale}px 0 0 ${-8 * scale}px;box-shadow:0 0 ${14 * scale}px rgba(255,210,120,.7);
  animation:__rip .65s cubic-bezier(.2,.7,.3,1) forwards}
@keyframes __rip{from{transform:scale(.6);opacity:1}to{transform:scale(4.2);opacity:0}}`;
    document.documentElement.append(st);
    const c = document.createElement('div');
    c.id = '__cur';
    c.innerHTML = '<div class="halo"></div><svg viewBox="0 0 24 30"><path d="M3 2 L3 24 L8.6 18.6 L12.4 27.2 L16 25.6 L12.3 17.2 L20 17.2 Z" fill="#fff" stroke="#1b1b22" stroke-width="1.6" stroke-linejoin="round"/></svg>';
    c.style.transform = `translate(${o.x || -100}px,${o.y || -100}px)`;
    document.documentElement.append(c);
    addEventListener('mousemove', (e) => { c.style.transform = `translate(${e.clientX}px,${e.clientY}px)`; }, true);
    addEventListener('mousedown', (e) => {
      c.classList.add('down');
      const r = document.createElement('div'); r.className = '__rip'; r.style.left = e.clientX + 'px'; r.style.top = e.clientY + 'px';
      document.documentElement.append(r); setTimeout(() => r.remove(), 800);
    }, true);
    addEventListener('mouseup', () => c.classList.remove('down'), true);
    window.__cursorHide = (h) => { c.style.opacity = h ? '0' : '1'; };
  };
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', add); else add();
};

// ---------------------------------------------------------------- eased moves and clicks
// The real mouse moves (so hovers show), and every position and click goes in the page's log with its wall time: the
// cursor and its ripples are drawn afterwards (assemble.py), smooth at 30 fps whatever the screencast's own rate.
const pos = new WeakMap();
const logs = new WeakMap();
const now = () => Date.now() / 1000;
const ease = (t) => (t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2);
const log = (page) => { if (!logs.has(page)) logs.set(page, { moves: [], clicks: [] }); return logs.get(page); };
async function moveTo(page, x, y, ms = 700) {
  const from = pos.get(page) || { x: x - 200, y: y + 120 };
  const mx = (from.x + x) / 2 + (y - from.y) * 0.12, my = (from.y + y) / 2 - (x - from.x) * 0.12;   // a slight arc, like a hand
  const at = (t) => { const e = ease(t); return [(1 - e) * (1 - e) * from.x + 2 * (1 - e) * e * mx + e * e * x, (1 - e) * (1 - e) * from.y + 2 * (1 - e) * e * my + e * e * y]; };
  const t0 = now(), dur = ms * SLOW / 1000;
  // the drawn path, exact: 60 points along the planned time
  for (let i = 0; i <= 60; i++) { const [px, py] = at(i / 60); log(page).moves.push([t0 + dur * i / 60, px, py]); }
  for (;;) {   // the real mouse follows as fast as the page lets it
    const t = Math.min(1, (now() - t0) / dur);
    const [px, py] = at(t);
    await page.mouse.move(px, py);
    if (t >= 1) break;
    await page.waitForTimeout(12);
  }
  const left = t0 + dur - now();
  if (left > 0) await page.waitForTimeout(left * 1000);
  pos.set(page, { x, y });
}
async function center(page, sel) {
  const loc = page.locator(sel).first();
  await loc.waitFor({ state: 'visible', timeout: 20000 });
  await loc.scrollIntoViewIfNeeded();
  const b = await loc.boundingBox();
  return { x: b.x + b.width / 2, y: b.y + b.height / 2 };
}
async function hover(page, sel, ms = 700) { const c = await center(page, sel); await moveTo(page, c.x, c.y, ms); return c; }
async function click(page, sel, { ms = 700, before = 220, after = 350 } = {}) {
  const c = await hover(page, sel, ms);
  await page.waitForTimeout(before * SLOW);
  log(page).clicks.push([now(), c.x, c.y]);
  await page.mouse.down(); await page.waitForTimeout(90 * SLOW); await page.mouse.up();
  await page.waitForTimeout(after * SLOW);
  return c;
}
async function typeSlow(page, text, delay = 70) { for (const ch of text) { await page.keyboard.type(ch); await page.waitForTimeout(delay * SLOW); } }
const wait = (page, ms) => page.waitForTimeout(ms * SLOW);
async function setCursor(page, x, y) { await page.mouse.move(x, y); pos.set(page, { x, y }); log(page).moves.push([now(), x, y]); }

// ---------------------------------------------------------------- the recorder (CDP screencast: every frame with its time)
async function record(page, dir) {
  fs.rmSync(dir, { recursive: true, force: true });
  fs.mkdirSync(dir, { recursive: true });
  const cdp = await page.context().newCDPSession(page);
  const frames = [];
  let n = 0;
  cdp.on('Page.screencastFrame', async (f) => {
    const name = String(n++).padStart(5, '0') + '.jpg';
    fs.writeFileSync(path.join(dir, name), Buffer.from(f.data, 'base64'));
    frames.push({ file: name, t: f.metadata.timestamp });
    try { await cdp.send('Page.screencastFrameAck', { sessionId: f.sessionId }); } catch (_) { /* closing */ }
  });
  const vp = page.viewportSize();
  const dpr = await page.evaluate(() => devicePixelRatio);
  await cdp.send('Animation.enable');
  await cdp.send('Animation.setPlaybackRate', { playbackRate: 1 / SLOW });
  await cdp.send('Page.startScreencast', { format: 'jpeg', quality: 92, maxWidth: Math.round(vp.width * dpr), maxHeight: Math.round(vp.height * dpr), everyNthFrame: 1 });
  const t0 = Date.now() / 1000;
  const marks = [];
  return {
    mark: (label) => marks.push({ label, t: Date.now() / 1000 }),
    stop: async () => {
      const t1 = Date.now() / 1000;
      await cdp.send('Page.stopScreencast');
      await new Promise((r) => setTimeout(r, 300));
      const lg = log(page);
      fs.writeFileSync(path.join(dir, 'frames.json'), JSON.stringify({ t0, t1, frames, marks, scale: dpr, slow: SLOW, moves: lg.moves.filter((m) => m[0] >= t0 - 5), clicks: lg.clicks }, null, 0));
      return frames.length;
    },
  };
}

// every visible word on the page, checked for the names we never show
async function brandCheck(page, label) {
  const txt = await page.evaluate(() => document.body.innerText + ' ' + [...document.querySelectorAll('[title],[aria-label],img[alt]')].map((e) => e.title || e.getAttribute('aria-label') || e.alt).join(' '));
  const m = txt.match(BRANDS);
  if (m) throw new Error(`${label}: "${m[0]}" is on screen`);
}

module.exports = { SLOW, wait, ROOT, UI, INDEX, FAKE, CURSOR, modes, moveTo, hover, click, typeSlow, setCursor, record, brandCheck, BRANDS };
