// stages.js warm by destination (phase 0, review 4 Nov): a new character's unstreamed racial start tile is told to
// boot.js as its destination tier's home ADT, on a streamed slice only. Run: node stream/tests/stages-dest.test.cjs
const fs = require('fs'), os = require('os'), path = require('path');
const src = path.join(__dirname, '..', 'www-files', 'stages.js');
const mini = JSON.parse(fs.readFileSync(path.join(__dirname, '..', 'slices', 'mini.json'), 'utf8'));
const tmp = path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'stages-')), 'stages.mjs');
fs.writeFileSync(tmp, fs.readFileSync(src, 'utf8'));
let fails = 0;
const ok = (c, m, x) => { console.log((c ? 'PASS ' : 'FAIL ') + m + (c || x === undefined ? '' : ' ' + JSON.stringify(x))); if (!c) fails++; };
(async () => {
  const { destFocus, DEST_TILES, streamedSlice, installStages, BOSS_TILES, bossOf, dataSaving } = await import('file://' + tmp);
  const env = { slice: 'mini', slice_tier: '12' };
  const W = 'World\\Maps\\';
  const rows = [
    ['Azeroth', '31_43', 'Azeroth', '32_48', 'dwarf, gnome'], ['Kalimdor', '30_12', 'Azeroth', '32_48', 'night elf'],
    ['Expansion01', '58_39', 'Azeroth', '32_48', 'draenei'], ['Azeroth', '28_28', 'Kalimdor', '39_33', 'undead'],
    ['Kalimdor', '32_37', 'Kalimdor', '39_33', 'tauren'], ['Expansion01', '43_12', 'Kalimdor', '39_33', 'blood elf'],
  ];
  for (const [m, t, dm, dt, who] of rows) {
    ok(destFocus(`focus:${W}${m}\\${m}_${t}.adt`, env) === `focus:${W}${dm}\\${dm}_${dt}.adt`, `${who}: focus goes to ${dm}_${dt}`,
       destFocus(`focus:${W}${m}\\${m}_${t}.adt`, env));
  }
  ok(destFocus('focus:world/maps/azeroth/azeroth_31_43.adt', env) === 'focus:world/maps/Azeroth/Azeroth_32_48.adt', 'any case, / separators');
  ok(destFocus('port:World\\Maps\\Kalimdor\\Kalimdor_32_37.adt', env) === 'port:World\\Maps\\Kalimdor\\Kalimdor_39_33.adt', 'port: too');
  ok(destFocus('focus:Azeroth_28_28', env) === 'focus:Kalimdor_39_33', 'a bare tile name');
  ok(destFocus(`focus:${W}Azeroth\\Azeroth_32_48.adt`, env) === `focus:${W}Azeroth\\Azeroth_32_48.adt`, 'a streamed tile (human) unchanged');
  ok(destFocus(`focus:${W}Kalimdor\\Kalimdor_39_33.adt`, env) === `focus:${W}Kalimdor\\Kalimdor_39_33.adt`, 'orc / troll home unchanged');
  ok(destFocus(`focus:${W}Azeroth\\Azeroth_31_43.adt`, {}) === `focus:${W}Azeroth\\Azeroth_31_43.adt`, 'no env.slice: unchanged');
  ok(destFocus(`focus:${W}Azeroth\\Azeroth_31_43.adt`, { mode: 'own' }) === `focus:${W}Azeroth\\Azeroth_31_43.adt`, 'own files: unchanged');
  ok(destFocus(`focus:${W}Azeroth\\Azeroth_31_43.adt`, { slice: 'mini', hop: 'r1' }) === `focus:${W}Azeroth\\Azeroth_31_43.adt`, 'a hop: unchanged');
  ok(destFocus('ready', env) === 'ready' && destFocus('reconnect:5', env) === 'reconnect:5', 'other stages unchanged');
  ok(!streamedSlice(null) && streamedSlice(env), 'streamedSlice');
  // the destinations are the starter tiers' homes in mini.json (ADT x from world y, y from world x)
  const homes = {};
  for (const t of mini.tiers) if (t.kind === 'starter' && t.home) homes[t.key] = `${Math.floor(32 - t.home.y / (1600 / 3))}_${Math.floor(32 - t.home.x / (1600 / 3))}`;
  const dests = new Set(Object.values(DEST_TILES).map((d) => `${d[1]}_${d[2]}`));
  ok(dests.has(homes['northshire']) && dests.has(homes['valley-of-trials']) && dests.size === 2, 'destinations are mini.json starter homes', homes);
  // a boss fight (?go=bf:...): character select's focus becomes the fight's tile (the icc-toc tier), whoever the character is
  const BF = { kind: 'bf', id: 'frozen-throne:heroic:group' };
  const ICC = `focus:${W}IcecrownCitadel\\IcecrownCitadel_35_31.adt`;
  ok(destFocus(`focus:${W}Northrend\\Northrend_30_21.adt`, env, BF) === ICC, 'boss fight: Dalaran focus -> the platform tile',
     destFocus(`focus:${W}Northrend\\Northrend_30_21.adt`, env, BF));
  ok(destFocus(`focus:${W}Azeroth\\Azeroth_31_43.adt`, env, 'bf:frozen-throne:normal:bots') === ICC, 'boss fight wins over a racial start (string go)');
  ok(destFocus('focus:world/maps/northrend/northrend_30_21.adt', env, BF) === 'focus:world/maps/IcecrownCitadel/IcecrownCitadel_35_31.adt', 'any separators');
  ok(destFocus(`port:${W}Northrend\\Northrend_30_21.adt`, env, BF) === `port:${W}Northrend\\Northrend_30_21.adt`, 'port: is the real teleport: unchanged');
  ok(destFocus(`focus:${W}Northrend\\Northrend_30_21.adt`, {}, BF) === `focus:${W}Northrend\\Northrend_30_21.adt`, 'not streamed: unchanged');
  ok(destFocus(`focus:${W}Northrend\\Northrend_30_21.adt`, { slice: 'mini', mode: 'own' }, BF) === `focus:${W}Northrend\\Northrend_30_21.adt`, 'own files: unchanged');
  for (const c of [{ saveData: true }, { effectiveType: '3g' }, { effectiveType: '2g' }, { effectiveType: 'slow-2g' }]) {
    ok(destFocus(`focus:${W}Northrend\\Northrend_30_21.adt`, env, BF, c) === `focus:${W}Northrend\\Northrend_30_21.adt`,
       'data saving: nothing extra warmed ' + JSON.stringify(c));
  }
  ok(destFocus(`focus:${W}Azeroth\\Azeroth_31_43.adt`, env, BF, { saveData: true }) === `focus:${W}Azeroth\\Azeroth_32_48.adt`,
     'data saving: a new character still goes to its starter tier');
  ok(destFocus(`focus:${W}Northrend\\Northrend_30_21.adt`, env, BF, { type: 'cellular', effectiveType: '4g' }) === ICC, 'a phone on 4g: warmed');
  for (const g of [{ kind: 'b', id: 'icc-frozen-throne' }, 'bf:frozen-throne:mythic:bots', 'bf:nowhere:normal:bots', { kind: 'bf', id: 5 }, null, 'arena:1v1']) {
    ok(bossOf(g) === null && destFocus(`focus:${W}Northrend\\Northrend_30_21.adt`, env, g) === `focus:${W}Northrend\\Northrend_30_21.adt`,
       'not a boss fight: unchanged ' + JSON.stringify(g));
  }
  ok(!dataSaving(null) && !dataSaving({ effectiveType: '4g' }) && dataSaving({ saveData: true }), 'dataSaving');
  // the tile is the platform's centre (631: 503.63, -2124.66) through world_to_tile, as test_bossfights.py checks too
  const T = 1600 / 3;
  ok(BOSS_TILES['frozen-throne'][1] === Math.floor(32 - -2124.66 / T) && BOSS_TILES['frozen-throne'][2] === Math.floor(32 - 503.63 / T),
     'the platform tile', BOSS_TILES['frozen-throne']);
  // the icc-toc tier holds the whole of map 631
  const icc = mini.tiers.find((t) => t.key === 'icc-toc');
  ok(icc && icc.maps.some((m) => m.id === 631 && !m.box && !m.zone), 'mini.json icc-toc streams all of 631');
  // installed: boot.js's hook is told the destination, the page's own events keep the stage as sent
  const told = [], events = [];
  global.window = { __wenilla_env: env, dispatchEvent: (e) => events.push(e.detail) };
  global.CustomEvent = class { constructor(t, o) { this.detail = o.detail; } };
  global.performance = { now: () => 1, mark() {} };
  global.requestAnimationFrame = () => 0;
  global.document = { getElementById: () => null, documentElement: {} };
  global.MutationObserver = class { observe() {} disconnect() {} };
  const info = console.info; console.info = () => {};
  installStages(null);
  window.__wenilla_progress = (s) => told.push(s);
  window.__wenilla_progress(`focus:${W}Kalimdor\\Kalimdor_30_12.adt`);
  window.__wenilla_progress('ready');
  console.info = info;
  ok(told[0] === `focus:${W}Azeroth\\Azeroth_32_48.adt` && told[1] === 'ready', 'boot.js is told the destination', told);
  ok(events.includes(`focus:${W}Kalimdor\\Kalimdor_30_12.adt`), 'the page event keeps the client\'s stage', events);
  window.wuGo = { kind: 'bf', id: 'frozen-throne:normal:bots' };
  console.info = () => {};
  window.__wenilla_progress(`focus:${W}Northrend\\Northrend_30_21.adt`);
  console.info = info;
  ok(told[2] === ICC, 'installed: a boss fight session tells boot.js the platform tile', told);
  process.exit(fails ? 1 : 0);
})();
