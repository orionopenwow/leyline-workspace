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
  const { destFocus, DEST_TILES, streamedSlice, installStages } = await import('file://' + tmp);
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
  process.exit(fails ? 1 : 0);
})();
