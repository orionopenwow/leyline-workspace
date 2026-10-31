// The in-game realm browser's page side, in node: wrath-page.js's leyline-browse block (the listing contract read
// defensively, the merge with the network listing, the filters, the Lua answers the game reads, the preview's verdict)
// and the fast hop (the Hero Key login redeemed before the reload, the same-session switch, the fallbacks).
// The Lua answers are run through lua5.1 to prove they are valid Lua and carry what the window reads.
//
//   node --test tests/browse.page.test.cjs          (from the stream folder; lua5.1 on PATH)
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { execFileSync } = require('node:child_process');
const { pathToFileURL } = require('node:url');

const WWW = path.join(__dirname, '..', 'www-files');
const A = 'aaaaaaaaaaaaaaaa', B = 'bbbbbbbbbbbbbbbb', C = 'cccccccccccccccc', D = 'dddddddddddddddd', H = 'hhhhhhhhhhhhhhhh';
const load = async () => { globalThis.window = { __wenilla_env: {} }; return import(pathToFileURL(path.join(WWW, 'wrath-page.js')).href); };

const rec = (id, name, o = {}) => Object.assign({ schema: 1, name, official: false, state: 'live', tagline: '',
  profile: { build: 12340, version: '3.3.5a', ruleset: 'main' }, rules: [], mods: [],
  community: { realmId: id, lastHeartbeat: { online: true, players: 0, bots: 0 }, uptime: { h24: null, d7: null } } }, o);

// the network listing: staff order, a hidden realm (D) left out, featured flags
const NETWORK = { realms: [
  rec(H, 'Wrath Unleashed', { community: { realmId: H, sample: true, group: 'sample', lastHeartbeat: { players: 4, bots: 10 } } }),
  rec(A, 'Frost Haven', { community: { realmId: A, featured: true, lastHeartbeat: { players: 3, bots: 0 }, uptime: { d7: 91 } } }),
  rec(B, 'Old Days of WoW 1.12.1 fun', { profile: { build: 5875, version: '1.12.1' } }),
  rec(C, 'Quiet Meadow', { state: 'offline' }),
] };
// the relay listing: the contract's fields on A; B and C carry nothing new (older relay); D is hidden by staff
const RELAY = { realms: [
  rec(A, 'Frost Haven', { build: '3.3.5a', online: 7, who: ['Mira', 'Tolan', 'not ok!', 'x', 'Wow'], rules: 'Be kind. No |cffff0000spam|r. World of Warcraft rules 3.3.5a apply.',
    mods: [{ id: 'mode:minigolf', name: 'Minigolf', required: true, client: false }, { id: 'up:skada', name: 'Skada', required: false, client: true },
           { id: 'mod:bad id!' }, 'mod:dalaran-dice'], herokey: true, uptime_pct: 99.4 }),
  rec(B, 'Old Days', { profile: { build: 5875 } }),
  rec(C, 'Quiet Meadow', { state: 'offline' }),
  rec(D, 'Hidden', { online: 50 }),
] };

function lua(chunk, read) {
  return execFileSync('lua5.1', ['-e', chunk + '\n' + read], { encoding: 'utf8' }).trim();
}

test('the listing contract, read defensively: missing = unknown, host text cleaned, the version only a chip', async () => {
  const m = await load();
  const list = m.mergeListings(NETWORK, RELAY);
  assert.deepEqual(list.map((r) => r.id), [H, A, B, C], 'the network order; the staff-hidden realm is not there');
  const a = list[1];
  assert.equal(a.online, 7);
  assert.deepEqual(a.who, ['Mira', 'Tolan'], 'names only, and never a brand word');
  assert.equal(a.rules, 'Be kind. No spam. rules apply.');
  assert.equal(a.herokey, true);
  assert.equal(a.up, 99);
  assert.equal(a.build, 12340);
  assert.deepEqual(a.mods, [
    { id: 'mode:minigolf', name: 'Minigolf', required: true, client: false },
    { id: 'up:skada', name: 'Skada', required: false, client: true },
    { id: 'mod:dalaran-dice', name: 'Dalaran dice', required: null, client: null }]);
  const b = list[2];
  assert.equal(b.name, 'Old Days of fun', 'no brand word, no version number in the name');
  assert.equal(b.online, 0, 'the heartbeat count when the realm sends no "online"');
  assert.equal(b.who, null, 'no "who" = the realm did not opt in');
  assert.equal(b.herokey, null, 'unknown');
  assert.equal(b.build, 5875);
  assert.equal(b.browser, false);
  assert.equal(b.own, true);
  assert.equal(list[0].hangout, true);
  // the relay listing alone when the network one is down
  assert.deepEqual(m.mergeListings(null, RELAY).map((r) => r.id), [A, B, C, D]);
  // junk records are skipped
  assert.deepEqual(m.mergeListings({ realms: [null, 5, { community: { realmId: '../x' } }, rec(A, 'x', { official: true })] }, null), []);
});

test('filters, search and pages; the Lua answer is valid and small', async () => {
  const m = await load();
  const list = m.mergeListings(NETWORK, RELAY);
  assert.deepEqual(m.parseAsk('list|2|b,p,v12340,zz|Frost'), { kind: 'list', page: 2, filter: { browser: true, players: true, build: 12340 }, q: 'frost' });
  assert.deepEqual(m.parseAsk('preview|' + A), { kind: 'preview', id: A });
  assert.equal(m.parseAsk('preview|../x'), null);
  assert.equal(m.parseAsk('hop|' + A), null);
  assert.deepEqual(m.filterRealms(list, { browser: true }).map((r) => r.id), [H, A, C]);
  assert.deepEqual(m.filterRealms(list, { own: true }).map((r) => r.id), [B]);
  assert.deepEqual(m.filterRealms(list, { players: true }).map((r) => r.id), [H, A]);
  assert.deepEqual(m.filterRealms(list, { build: 5875 }).map((r) => r.id), [B]);
  assert.deepEqual(m.filterRealms(list, {}, 'minigolf').map((r) => r.id), [A], 'search finds mod names too');
  const chunk = m.listLua(list, m.parseAsk('list|1||'), { sso: true });
  const out = lua(chunk, 'local n = WUUI_LEYLINE_NET; local r = n.rows[2]; print(n.total, n.pages, #n.rows, n.sso, r.id, r.name, r.online, r.chip, tostring(r.hk), r.browser, n.network.players, n.rows[3].chip, n.rows[3].own)');
  assert.equal(out, `4\t1\t4\ttrue\t${A}\tFrost Haven\t7\t3.3.5a\ttrue\ttrue\t11\t1.12.1\ttrue`);
  // the callbacks run when set, and a failing one can't break the other
  const cb = lua('WUUI_LEYLINE_ONNET = function() error("x") end; WUUI_LEYLINE_ONNET_GLUE = function(k) GOT = k end\n' + chunk, 'print(GOT)');
  assert.equal(cb, 'list');
  // many realms: 20 a page
  const many = Array.from({ length: 45 }, (_, i) => m.realmFrom(rec('a'.repeat(14) + 'b' + 'abcdefghijklmnopqrstuvwxyz234567'[i % 32], 'R' + i), null));
  const p3 = lua(m.listLua(many, { kind: 'list', page: 3, filter: {}, q: '' }), 'print(WUUI_LEYLINE_NET.page, WUUI_LEYLINE_NET.pages, #WUUI_LEYLINE_NET.rows)');
  assert.equal(p3, '3\t3\t5');
  assert.ok(m.listLua(many, { kind: 'list', page: 1, filter: {}, q: '' }).length < 8000, 'a page stays small');
  assert.equal(lua(m.listLua([], null, { err: 'Realm list unavailable' }), 'print(WUUI_LEYLINE_NET.err, #WUUI_LEYLINE_NET.rows)'), 'Realm list unavailable\t0');
  // strings that could break out of a Lua string are escaped
  const evil = m.realmFrom(rec(A, 'a"b\\c]]..os.exit()--é'), null);
  assert.equal(lua(m.listLua([evil], { kind: 'list', page: 1, filter: {}, q: '' }), 'print(WUUI_LEYLINE_NET.rows[1].name)'), 'a"b\\c]]..os.exit()--é');
});

test('the preview: who is there only when the realm opted in, the mods, the verdict, the Hero Key state', async () => {
  const m = await load();
  const list = m.mergeListings(NETWORK, RELAY);
  const a = list[1], b = list[2], c = list[3];
  const out = lua(m.previewLua(a, {}, { hk: 'ready' }), `local p = WUUI_LEYLINE_PREVIEW
    print(p.name, p.online, #p.who, p.who[1], #p.mods, p.mods[1].name, tostring(p.mods[1].req), tostring(p.mods[2].client), tostring(p.mods[3].req), p.chip, p.up, p.hk, p.verdict, p.rules)`);
  assert.equal(out, 'Frost Haven\t7\t2\tMira\t3\tMinigolf\ttrue\ttrue\tnil\t3.3.5a\t99\tready\tok\tBe kind. No spam. rules apply.');
  assert.equal(lua(m.previewLua(b, {}), 'local p = WUUI_LEYLINE_PREVIEW; print(tostring(p.who), p.verdict, p.chip, p.hk)'), 'nil\tlauncher\t1.12.1\tunknown');
  assert.match(m.verdict(b).why, /different game version/);
  assert.doesNotMatch(m.verdict(b).why, /\d\.\d/, 'no version number in the words');
  assert.equal(m.verdict(c).verdict, 'offline');
  assert.equal(m.verdict(Object.assign({}, a, { own: true })).verdict, 'own');
  assert.equal(m.verdict(Object.assign({}, a, { own: true }), { mode: 'own' }).verdict, 'ok');
  assert.equal(m.verdict(Object.assign({}, a, { warden: true })).verdict, 'launcher');
  assert.equal(lua(m.previewLua(null), 'print(WUUI_LEYLINE_PREVIEW.missing)'), 'true');
  for (const s of [m.verdict(b).why, m.verdict(Object.assign({}, a, { own: true })).why, m.verdict(c).why]) {
    assert.doesNotMatch(s, /world of warcraft|warcraft|wow|blizzard|lich king/i);
  }
});

function fakeFetch(routes) {
  const calls = [];
  const f = async (url, init = {}) => {
    calls.push({ url: String(url), init });
    for (const [re, fn] of routes) if (re.test(String(url))) return fn(url, init);
    return { ok: false, status: 404, json: async () => ({}) };
  };
  f.calls = calls;
  return f;
}
const json = (doc, status = 200) => async () => ({ ok: status < 400, status, json: async () => doc });
function storage(init = {}) {
  const mm = new Map(Object.entries(init));
  return { getItem: (k) => (mm.has(k) ? mm.get(k) : null), setItem: (k, v) => mm.set(k, String(v)), removeItem: (k) => mm.delete(k), m: mm };
}
const TICKET = { ok: true, realm: A, assertion: { aud: A }, sig: 'ab'.repeat(64), redeem: `https://relay.example/api/realms/${A}/herokey`,
  login: { user: 'HKABCDEFGHIJKLMN', pass: 'Q7W8E9R2T3Y4U5I6' }, ttl: 120 };

async function netWith(m, fetchFn, flags = { herokey_sso: true }) {
  const net = m.makeNet({ env: { home: 'https://play.example', hop_ws: `wss://relay.example/ws/${B}` }, fetchFn });
  await net.load();
  net.flags = flags;
  return net;
}
const listingRoutes = () => [
  [/flags\.json$/, json({ flags: { herokey_sso: true } })],
  [/play\.example\/wrath\/realms\.json$/, json(NETWORK)],
  [/relay\.example\/api\/listing\/realms\.json$/, json(RELAY)],
];

test('the network: flags, both listings (relay from the hop session), cached; the mesh when its flag is on', async () => {
  const m = await load();
  const f = fakeFetch(listingRoutes());
  const net = m.makeNet({ env: { home: 'https://play.example', hop_ws: `wss://relay.example/ws/${B}` }, fetchFn: f });
  await net.load();
  assert.equal(net.list.length, 4);
  assert.equal(net.flags.herokey_sso, true);
  const n = f.calls.length;
  await net.load();
  assert.equal(f.calls.length, n, 'cached');
  assert.ok(f.calls.every((c) => c.init.credentials === 'omit'), 'no cookies to the listings');
  const g = fakeFetch([[/flags\.json$/, json({ flags: { mesh_realms: true } })], [/realms\.json$/, json(null, 500)],
    [/api\/listing\/realms\.json$/, json(RELAY)]]);
  const net2 = m.makeNet({ env: { home: 'https://play.example' }, fetchFn: g });
  await net2.load();
  assert.ok(g.calls.some((c) => c.url === 'https://relay.openvanilla.app/api/mesh/realms.json'), 'the mesh first');
  const down = m.makeNet({ env: {}, fetchFn: fakeFetch([]) });
  await down.load();
  assert.equal(down.err, 'Realm list unavailable');
});

test('fast hop with Hero Key: prefetched at the preview, redeemed before the reload, the same session, no join page', async () => {
  const m = await load();
  const f = fakeFetch([...listingRoutes(), [/__gate\?herokey=/, json(TICKET)], [/\/herokey$/, json({ ok: true })],
    [/__gate\?hop=/, json({ ok: true, land: `/?go=r:${A}` })]]);
  const net = await netWith(m, f);
  const t = await m.prefetchTicket(A, net, { fetchFn: f });
  assert.equal(t.user, 'HKABCDEFGHIJKLMN');
  assert.equal(f.calls.filter((c) => /herokey/.test(c.url)).length, 1, 'only the website was asked at the preview: the realm sees nothing');
  const session = storage(), steps = [];
  let landed = null;
  const went = await m.fastHop(A, { env: {}, net, fetchFn: f, session, store: storage(), replace: (u) => { landed = u; },
    cover: () => ({ step: (s, p) => steps.push([s, p]), done() {} }) });
  assert.equal(went, true);
  assert.equal(landed, `/?go=r:${A}`);
  const redeem = f.calls.find((c) => c.url === TICKET.redeem);
  assert.equal(redeem.init.credentials, 'omit');
  assert.ok(!redeem.init.body.includes(TICKET.login.pass), 'the password never goes to the relay');
  const hop = f.calls.find((c) => /__gate\?hop=/.test(c.url));
  assert.equal(JSON.parse(hop.init.body).sso, 'pre');
  assert.equal(hop.init.credentials, 'same-origin');
  assert.equal(f.calls.filter((c) => /__gate\?herokey=/.test(c.url)).length, 1, 'the prefetched ticket was used');
  const pre = JSON.parse(session.getItem('wu.leyline.pre.' + A));
  assert.equal(pre.user, 'HKABCDEFGHIJKLMN');
  assert.equal(pre.ttl, 120);
  assert.ok(session.getItem('wu.leyline.hop.t0'));
  assert.deepEqual(steps.map((s) => s[0]), ['Signing in with your Hero Key', 'Switching realm', 'Loading']);
});

test('fast hop fallbacks: realm refuses Hero Key, server refuses the switch, declined here, another version, own files', async () => {
  const m = await load();
  const cover = () => ({ step() {}, done() {} });
  // the realm refuses the assertion: still a fast hop, without "pre" (the env path then tries, then the join page)
  let f = fakeFetch([...listingRoutes(), [/__gate\?herokey=/, json(TICKET)], [/\/herokey$/, json({ code: 'off' }, 403)],
    [/__gate\?hop=/, json({ ok: true, land: `/?go=r:${A}` })]]);
  let net = await netWith(m, f), session = storage(), landed = null;
  assert.equal(await m.fastHop(A, { env: {}, net, fetchFn: f, session, store: storage(), replace: (u) => { landed = u; }, cover }), true);
  assert.equal(JSON.parse(f.calls.find((c) => /__gate\?hop=/.test(c.url)).init.body).sso, '');
  assert.equal(session.getItem('wu.leyline.pre.' + A), null);
  // the server can't switch (an old portal, a session gone): false = the caller takes the old path
  f = fakeFetch([...listingRoutes(), [/__gate\?hop=/, json({ ok: false }, 404)]]);
  net = await netWith(m, f, {});
  assert.equal(await m.fastHop(A, { env: {}, net, fetchFn: f, session: storage(), store: storage(), replace: () => assert.fail('no'), cover }), false);
  // a junk landing is refused
  f = fakeFetch([...listingRoutes(), [/__gate\?hop=/, json({ ok: true, land: 'https://evil.example/' })]]);
  net = await netWith(m, f, {});
  assert.equal(await m.fastHop(A, { env: {}, net, fetchFn: f, session: storage(), store: storage(), replace: () => assert.fail('no'), cover }), false);
  // declined in this browser ("Use another account"): no prefetch, no redeem
  f = fakeFetch([...listingRoutes(), [/__gate\?hop=/, json({ ok: true, land: `/?go=r:${A}` })]]);
  net = await netWith(m, f);
  await m.fastHop(A, { env: {}, net, fetchFn: f, session: storage(), store: storage({ ['wu.leyline.sso.off.' + A]: '1' }), replace() {}, cover });
  assert.ok(!f.calls.some((c) => /herokey/.test(c.url)));
  // another game version: no hop at all (the launcher bar instead)
  f = fakeFetch(listingRoutes());
  net = await netWith(m, f);
  assert.equal(await m.fastHop(B, { env: {}, net, fetchFn: f, session: storage(), store: storage(), replace: () => assert.fail('no'), assign: () => assert.fail('no'), cover }), true);
  assert.ok(!f.calls.some((c) => /__gate/.test(c.url)));
  // offline: the old path (which lands on home), never a switch
  assert.equal(await m.fastHop(C, { env: {}, net, fetchFn: f, session: storage(), store: storage(), cover }), false);
  // a realm needing own files from a streamed session: the website's own-files switch
  const own = m.mergeListings({ realms: [rec(A, 'Frost Haven', { own_files: true })] }, null);
  net.list = own;
  let assigned = null;
  assert.equal(await m.fastHop(A, { env: { home: 'https://play.example' }, net, fetchFn: f, session: storage(), store: storage(), assign: (u) => { assigned = u; }, cover }), true);
  assert.equal(assigned, `https://play.example/wrath/play?mode=own&go=r:${A}`);
  // back home, from a hop session: the same session
  f = fakeFetch([[/__gate\?hop=wrath$/, json({ ok: true, land: '/?go=h:dalaran' })]]);
  landed = null;
  assert.equal(await m.fastHop('wrath', { env: { hop: A }, net: null, fetchFn: f, session: storage(), store: storage(), replace: (u) => { landed = u; }, cover }), true);
  assert.equal(landed, '/?go=h:dalaran');
  assert.equal(await m.fastHop('wrath', { env: {}, net: null, fetchFn: f, cover }), false, 'already home');
  assert.equal(await m.fastHop(A, { env: { demo: '1' }, net, fetchFn: f, cover }), false, 'never from the guest pass');
});

test('watchBrowse: the ask is taken through the bridge, the answer goes back in; watchHops tries the fast hop first', async () => {
  const m = await load();
  const sent = [];
  let pending = 'list|1|p|';
  const bridge = { ready: true, state: {}, lua: async (code) => {
    sent.push(code);
    if (code === m.BROWSE_TAKE) { const a = pending; pending = null; return [a]; }
    return [];
  } };
  const f = fakeFetch([...listingRoutes(), [/__gate\?herokey=/, json(TICKET)]]);
  const net = m.makeNet({ env: { home: 'https://play.example', hop_ws: `wss://relay.example/ws/${B}` }, fetchFn: f });
  const h = m.watchBrowse(bridge, { env: {}, net, pollMs: 0, fetchFn: f });
  assert.equal(await h.tick(), 'list');
  assert.match(sent[1], /^WUUI_LEYLINE_NET = /);
  assert.equal(lua(sent[1], 'print(WUUI_LEYLINE_NET.total)'), '2');
  pending = 'preview|' + A;
  assert.equal(await h.tick(), 'preview');
  assert.equal(lua(sent[3], 'print(WUUI_LEYLINE_PREVIEW.hk)'), 'ready', 'the Hero Key sign-in is prepared while the preview is open');
  assert.equal(await h.tick(), null, 'nothing asked');
  // watchHops: fast first; when it says no, the old way
  const hops = [];
  let wantFast = false;
  const hb = { ready: true, state: {}, lua: async () => ['abcdefghijklmnop'] };
  const w = m.watchHops(hb, { env: {}, go: (u) => hops.push(u), pollMs: 0, fast: async (t) => { hops.push('fast:' + t); return wantFast; } });
  await w.take();
  await new Promise((r) => setTimeout(r, 5));
  assert.deepEqual(hops, ['fast:abcdefghijklmnop', 'https://play.openvanilla.app/wrath?go=r:abcdefghijklmnop']);
});
