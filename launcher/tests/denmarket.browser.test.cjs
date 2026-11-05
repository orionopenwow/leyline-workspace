// Mods & Addons › Dens (ui/denmarket.js, src-tauri/src/denmarket.rs, docs/DEN-MARKETPLACE.md) in Chromium with a fake
// Tauri core: the tab, cards (picture, maker, size, props, tags, likes, opened), search and filters, a build's detail
// (pictures, credit, the remix line, licence, Made with Leyline), Open in my Den, Add as a stamp, Like, Report; Share
// (from the tab and from Your Den › Saved Dens › Share) with the game's own pictures (the showcase) and the player's,
// Your shared Dens (state, Update, Unpublish); a leyline://den/<id> link; signed out and "not open yet"; no brand words.
//
//   node --test launcher/tests/denmarket.browser.test.cjs     (DENMARKET_SHOTS=<dir> for screenshots)
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const INDEX = 'file://' + path.join(__dirname, '..', 'ui', 'index.html');
const SHOTS = process.env.DENMARKET_SHOTS || '';
const BRANDS = /world of warcraft|\bwow\b|warcraft|lich king|blizzard/i;

const FAKE = (o) => {
  const calls = (window.__calls = []);
  try { localStorage.clear(); localStorage.setItem('leyline.page', 'play'); } catch (_) { /* fine */ }
  const card = (id, title, extra) => Object.assign({ id, kind: 'den', title, author: 'Bea', pid: 'beabeabea2', credit: '', tags: ['garden'], props: 42,
    bytes: 9000, likes: 3, downloads: 10, featured: false, stampable: false, picture: `/wrath/dens/pic/${id}/0.jpg`, pictures: 1,
    created: '2026-11-04T10:00:00Z', updated: '2026-11-04T10:00:00Z', based_on: false }, extra || {});
  const ALL = [card('gardengard', "Bea's Garden", { featured: true, credit: 'Bea and the Sunday crew', tags: ['garden', 'cozy'] }),
    card('campcampca', 'Little camp', { kind: 'stamp', author: 'Cal', pid: 'cacacacaa3', props: 18, stampable: true, tags: ['camp'], based_on: true }),
    card('innsinnsin', 'Corner inn', { tags: ['inn', 'cozy'], picture: '' })];
  const TAGS = [{ id: 'camp', name: 'Camp' }, { id: 'inn', name: 'Inn' }, { id: 'garden', name: 'Garden' }, { id: 'cozy', name: 'Cozy' }, { id: 'spooky', name: 'Spooky' }];
  const S = (window.__S = { liked: false, likes: 2, mine: o.mine || [], shotsAsked: 0 });
  window.__TAURI__ = { core: { invoke: async (cmd, args) => {
    calls.push([cmd, args]);
    switch (cmd) {
      case 'get_state': return { version: 'test', settings: { clients: [{ id: 'c1', name: 'My Game', path: '/games/wrath', build: '3.3.5a', realmlist: '' }],
        selected_client: 'c1', selected_realm: 'play.openvanilla.app', wine: { enabled: false, command: 'wine', prefix: '' } },
        official_realm: 'play.openvanilla.app', browser_url: 'https://play.openvanilla.app/wrath', windows: true, data_dir: '/tmp' };
      case 'realms_list': return { realms: [], network: { realms: 0, players: 0, bots: 0 }, from_cache: false, saved_at: 1, error: null };
      case 'realm_status': return { state: 'online', ping_ms: 10, players: 0 };
      case 'mods_catalog': return { categories: [], projects: [], from_cache: false, saved_at: 1, error: null };
      case 'mods_ranked': return { ids: [], metrics: {}, from_cache: false, error: null };
      case 'mods_installed': return { client: 'c1', build: '3.3.5a', mods: [] };
      case 'mods_updates': return [];
      case 'themes_list': return { active: 'dark', themes: [] };
      case 'deep_link_take': return o.link || null;
      case 'launcher_update_state': return { mode: 'auto', checking: false, downloading: false, ready: null, info: null };
      case 'host_info': return { dir: null, is_kit: false, problems: [], seeds: [], realm: null };
      case 'modes_list': return { modes: [], from_cache: false, builtin: false, saved_at: null, error: null };
      case 'p2p_flags': return { effective: { p2p_hosting: false, limits: {} }, known: [], local: { overrides: {} }, site: {} };
      case 'social_status': return true;
      case 'den_state': return { lite: { available: true, can_get: false, running: true, address: '127.0.0.1', account: 'DEN', den_dir: '/d' }, kit: false,
        docker_den: false, other_realm: null, account: '', saves: 'lite', packs: [],
        dens: [{ name: 'autosave', bytes: 100, props: 3, saved: '2026-11-04T09:00:00Z' }, { name: 'garden', bytes: 9000, props: 42, saved: '2026-11-04T08:00:00Z' }] };
      case 'forge_state': return { installed: false, running: false, exe: '', sha256: '', last: null, source: '', licence: '' };
      case 'den_models': return [];
      case 'den_net_state': return { running: false, connected: false, relay: '', realm_id: '', address: '', auth_port: 0, world_port: 0, streams: 0, last_error: '' };
      case 'docker_info': return { state: 'ready', can_start: false, help: '' };
      case 'hostpack_info': return { available: { size: 1 }, installed: null };
      case 'dens_browse': {
        if (o.closed) throw 'not_open';
        const q = args.query || {};
        let items = ALL.filter((b) => (!q.kind || b.kind === q.kind) && (!q.tag || b.tags.includes(q.tag))
          && (!q.q || (b.title + ' ' + b.author).toLowerCase().includes(q.q.toLowerCase())));
        return { ok: true, featured: q.q || q.tag ? [] : ALL.filter((b) => b.featured), items, tags: TAGS.slice(0, 4), all_tags: TAGS,
          licences: [{ id: 'cc-by', name: 'CC BY 4.0', line: 'Share and remix it; credit me.' }, { id: 'look', name: 'Look only', line: "Open it; don't share remixes." }],
          reasons: [{ id: 'stolen', name: 'Not theirs / stolen work' }, { id: 'spam', name: 'Spam' }], page: 1, pages: 1, total: items.length };
      }
      case 'dens_detail': {
        const b = ALL.find((x) => x.id === args.id);
        if (!b) throw 'No such build.';
        return Object.assign({}, b, { description: 'Rows of flowers.\n\nA bench to sit on.', pictures: [`/wrath/dens/pic/${b.id}/0.jpg`, `/wrath/dens/pic/${b.id}/1.png`, 'https://evil.example/x.png'],
          licence: { id: 'cc-by', name: 'CC BY 4.0', line: 'Share and remix it; credit me.' }, likes: S.likes, liked: S.liked, mine: false, remix_ok: true, remixes: b.id === 'gardengard' ? 1 : 0,
          lineage: b.id === 'campcampca' ? [{ id: 'gardengard', title: "Bea's Garden", by: 'Bea', live: true }, { id: 'oldoldoldo', title: 'First camp', by: 'Dax', live: false }] : [],
          sha256: 'a'.repeat(64), made_with: 'Made with Leyline', page: `https://leylinelauncher.com/wrath/dens/${b.id}` });
      }
      case 'dens_open': return { name: 'bea-s-garden', title: "Bea's Garden" };
      case 'dens_stamp': return { name: 'little-camp', title: 'Little camp' };
      case 'dens_post': {
        if (o.signedOut) throw 'signed_out';
        if (args.action === 'like') { S.liked = args.args.on; S.likes += args.args.on ? 1 : -1; return { ok: true, likes: S.likes, liked: S.liked }; }
        if (args.action === 'report') return { ok: true, message: 'Thanks' };
        if (args.action === 'unpublish') { S.mine = S.mine.filter((b) => b.id !== args.args.id); return { ok: true, message: 'Taken down. Copies people already opened stay theirs.' }; }
        throw 'Unknown action.';
      }
      case 'dens_mine': if (o.signedOut) throw 'signed_out'; return { ok: true, builds: S.mine };
      case 'dens_showcase': return { id: 'sc1', since: 1000 };
      case 'dens_shots': S.shotsAsked += 1; return S.shotsAsked < 2 ? { done: false, answer: null, shots: [] }
        : { done: true, answer: { id: 'sc1', ok: true, shots: 4 }, shots: [1, 2, 3, 4].map((n) => ({ path: `/games/wrath/Screenshots/WoWScrnShot_${n}.jpg`, name: `shot${n}.jpg`, bytes: 1000, png: false, data: '/9j/4AAQ' })) };
      case 'dens_share': {
        if (o.signedOut) throw 'signed_out';
        const id = 'newnewnewn';
        S.mine = [{ id, kind: args.args.kind || 'den', title: args.args.title, state: 'pending', reason: '', update_waiting: false, props: 42, likes: 0, downloads: 0, tags: args.args.tags,
          description: args.args.description, licence: args.args.licence, credit: args.args.credit }].concat(S.mine);
        return { ok: true, id, state: 'pending', message: 'Shared. A Leyline moderator looks at it soon; it shows for everyone once they approve it.' };
      }
      case 'dens_page_open': return null;
      case 'open_link': return null;
      default: throw 'unknown ' + cmd;
    }
  } } };
};

let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

async function open(opts) {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 }, colorScheme: 'dark' });
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  await page.addInitScript(FAKE, opts || {});
  await page.goto(INDEX);
  await page.waitForFunction(() => window.LeylineDenMarket && window.LeylineNav);
  return { page, errors };
}
const shot = async (page, name) => { if (SHOTS) await page.screenshot({ path: path.join(SHOTS, name + '.png'), fullPage: false }); };
const calls = (page, name) => page.evaluate((n) => window.__calls.filter(([c]) => c === n).map(([, a]) => a), name);
async function tab(page) {
  await page.evaluate(() => window.LeylineNav.show('mods', false, 'dens'));
  await page.waitForSelector('#dm-grid .dm-card');
}

test('the Dens tab: featured, cards with what a build is, search and filters', async () => {
  const { page, errors } = await open();
  await page.evaluate(() => window.LeylineNav.show('mods'));
  assert.equal(await page.textContent('#tab-mods-dens'), 'Dens', 'a tab in Mods & Addons');
  await page.click('#tab-mods-dens');
  await page.waitForSelector('#dm-grid .dm-card');
  await shot(page, 'dens-tab');
  assert.deepEqual(await page.$$eval('#dm-featured .dm-card', (c) => c.map((x) => x.dataset.id)), ['gardengard'], 'featured on top');
  assert.deepEqual(await page.$$eval('#dm-grid .dm-card', (c) => c.map((x) => x.dataset.id)), ['gardengard', 'campcampca', 'innsinnsin']);
  const c = '#dm-grid .dm-card[data-id="gardengard"]';
  assert.equal(await page.getAttribute(`${c} img`, 'src'), 'https://leylinelauncher.com/wrath/dens/pic/gardengard/0.jpg', 'its picture from the website');
  assert.equal(await page.textContent(`${c} .dm-by`), 'by Bea');
  assert.equal(await page.textContent(`${c} .dm-nums`), '42 props · 9 KB · 3 likes · 10 opened');
  assert.deepEqual(await page.$$eval(`${c} .dm-tag`, (t) => t.map((x) => x.textContent)), ['Featured', 'Garden', 'Cozy']);
  assert.equal(await page.textContent('#dm-grid .dm-card[data-id="campcampca"] .dm-pic-kind'), 'Stamp');
  assert.equal(await page.$('#dm-grid .dm-card[data-id="innsinnsin"] img'), null, 'no picture: the plain card');
  await page.click('#dm .dm-chips .chip-btn[data-kind="stamp"]');
  await page.waitForFunction(() => document.querySelectorAll('#dm-grid .dm-card').length === 1);
  assert.equal((await calls(page, 'dens_browse')).pop().query.kind, 'stamp');
  await page.click('#dm .dm-chips .chip-btn[data-kind=""]');
  await page.waitForFunction(() => document.querySelectorAll('#dm-grid .dm-card').length === 3);
  await page.selectOption('#dm .dm-tag-pick', 'inn');
  await page.waitForFunction(() => document.querySelectorAll('#dm-grid .dm-card').length === 1);
  await page.selectOption('#dm .dm-tag-pick', '');
  await page.selectOption('#dm .dm-sort', 'likes');
  await page.waitForFunction(() => document.querySelectorAll('#dm-grid .dm-card').length === 3);
  await page.fill('#dm-q', 'camp');
  await page.waitForFunction(() => document.querySelectorAll('#dm-grid .dm-card').length === 1 && !document.querySelector('#dm-featured'));
  const last = (await calls(page, 'dens_browse')).pop().query;
  assert.deepEqual([last.q, last.sort], ['camp', 'likes']);
  assert.match(await page.textContent('#dm .dm-all h3'), /1 build found/);
  assert.doesNotMatch(await page.textContent('#pane-dens'), BRANDS);
  assert.deepEqual(errors, []);
  await page.close();
});

test('a build: pictures, credit, remix line, licence; Open in my Den, Add as a stamp, Like, Report', async () => {
  const { page, errors } = await open();
  await tab(page);
  await page.click('#dm-grid .dm-card[data-id="campcampca"]');
  await page.waitForSelector('#dm .dm-detail .dm-name');
  await shot(page, 'dens-detail');
  assert.equal(await page.textContent('#dm .dm-name'), 'Little camp');
  assert.deepEqual(await page.$$eval('#dm .dm-thumb img', (i) => i.map((x) => x.getAttribute('src'))),
    ['https://leylinelauncher.com/wrath/dens/pic/campcampca/0.jpg', 'https://leylinelauncher.com/wrath/dens/pic/campcampca/1.png'], 'only the website\'s own pictures');
  await page.click('#dm .dm-thumb:nth-child(2)');
  assert.equal(await page.getAttribute('#dm .dm-big img', 'src'), 'https://leylinelauncher.com/wrath/dens/pic/campcampca/1.png');
  assert.equal(await page.textContent('#dm .dm-credit'), 'by Cal');
  assert.equal(await page.textContent('#dm .dm-lineage'), "Based on Bea's Garden by Bea, based on First camp by Dax");
  assert.match(await page.textContent('#dm .dm-facts'), /CC BY 4\.0: Share and remix it; credit me\./);
  assert.match(await page.textContent('#dm .dm-detail'), /Made with Leyline/);
  await page.click('#dm-stamp');
  await page.waitForFunction(() => /\.den stamp little-camp/.test(document.querySelector('#dm .dm-status').textContent));
  assert.match(await page.textContent('#dm .dm-status'), /keeps Cal's credit/);
  await page.click('#dm-open');
  await page.waitForSelector('#dm-go-den');
  assert.match(await page.textContent('#dm .dm-status'), /in your Saved Dens as 'bea-s-garden'/);
  assert.deepEqual((await calls(page, 'dens_open'))[0], { id: 'campcampca' });
  await page.click('#dm-like');
  await page.waitForFunction(() => document.querySelector('#dm-like').textContent === 'Liked ✓');
  assert.deepEqual((await calls(page, 'dens_post'))[0], { action: 'like', args: { id: 'campcampca', on: true } });
  await page.click('#dm .dm-report summary');
  await page.selectOption('#dm .dm-report select', 'spam');
  await page.fill('#dm .dm-report input', 'Seen it before');
  await page.click('#dm-report-send');
  await page.waitForFunction(() => /a moderator will look/.test(document.querySelector('#dm .dm-status').textContent));
  assert.deepEqual((await calls(page, 'dens_post')).pop(), { action: 'report', args: { id: 'campcampca', reason: 'spam', note: 'Seen it before' } });
  // the remix line's original opens its own detail
  await page.click('#dm .dm-lineage .linkish');
  await page.waitForFunction(() => document.querySelector('#dm .dm-name') && document.querySelector('#dm .dm-name').textContent === "Bea's Garden");
  assert.equal(await page.$('#dm-stamp'), null, 'too big for a stamp: no Add as a stamp');
  assert.match(await page.textContent('#dm .dm-credit'), /Bea and the Sunday crew/);
  assert.doesNotMatch(await page.textContent('#pane-dens'), BRANDS);
  assert.deepEqual(errors, []);
  await page.close();
});

test('share a Den with the game\'s own pictures, then manage it in Your shared Dens', async () => {
  const { page, errors } = await open();
  await tab(page);
  await page.click('#dm .dm-bar .btn.primary');
  await page.waitForSelector('#dm-share-title');
  assert.equal(await page.inputValue('#dm-share-den'), 'garden', 'the first saved Den (not the autosave)');
  await page.selectOption('#dm-share-kind', 'den');
  await page.fill('#dm-share-title', "Bea's Garden");
  await page.fill('#dm-share-desc', 'Rows of flowers.');
  await page.fill('#dm-share-credit', 'Bea and the Sunday crew');
  await page.click('#dm .dm-share-tags .chip-btn[data-tag="garden"]');
  await page.click('#dm .dm-share-tags .chip-btn[data-tag="cozy"]');
  await page.click('#dm-shoot');
  await page.waitForFunction(() => document.querySelectorAll('#dm-strip .dm-shot').length === 4, null, { timeout: 8000 });
  await shot(page, 'dens-share');
  assert.deepEqual((await calls(page, 'dens_showcase'))[0], { name: 'garden' });
  assert.match(await page.textContent('#dm .dm-status'), /4 pictures from your game/);
  await page.click('#dm-strip .dm-shot:nth-child(4) .dm-shot-x');
  assert.equal(await page.$$eval('#dm-strip .dm-shot', (s) => s.length), 3, 'one removed');
  await page.click('#dm-share-go');
  await page.waitForSelector('#dm-mine li');
  const sent = (await calls(page, 'dens_share'))[0];
  assert.equal(sent.name, 'garden');
  assert.deepEqual(sent.args, { title: "Bea's Garden", description: 'Rows of flowers.', credit: 'Bea and the Sunday crew', licence: 'cc-by', tags: ['garden', 'cozy'], kind: 'den' });
  assert.deepEqual(sent.shots, ['/games/wrath/Screenshots/WoWScrnShot_1.jpg', '/games/wrath/Screenshots/WoWScrnShot_2.jpg', '/games/wrath/Screenshots/WoWScrnShot_3.jpg']);
  assert.match(await page.textContent('#dm .dm-status'), /moderator/);
  assert.match(await page.textContent('#dm-mine li'), /Waiting for a moderator/);
  // update: the form with what it has, an id, the shared file kept unless another Den is picked
  await page.click('#dm-mine li .btn:text-is("Update")');
  await page.waitForSelector('#dm-share-title');
  assert.equal(await page.inputValue('#dm-share-title'), "Bea's Garden");
  assert.equal(await page.inputValue('#dm-share-den'), '', 'Keep the shared file');
  await page.fill('#dm-share-desc', 'Now with a pond.');
  await page.click('#dm-share-go');
  await page.waitForSelector('#dm-mine li');
  const up = (await calls(page, 'dens_share'))[1];
  assert.deepEqual([up.name, up.args.id, up.args.description, up.shots], [null, 'newnewnewn', 'Now with a pond.', null]);
  // unpublish asks once more
  await page.click('#dm-mine li .dm-unpublish');
  assert.equal(await page.textContent('#dm-mine li .dm-unpublish'), 'Sure? Unpublish');
  await page.click('#dm-mine li .dm-unpublish');
  await page.waitForFunction(() => /Taken down/.test(document.querySelector('#dm .dm-status').textContent));
  assert.doesNotMatch(await page.textContent('#pane-dens'), BRANDS);
  assert.deepEqual(errors, []);
  await page.close();
});

test('Your Den › Saved Dens › Share opens the share form for that Den', async () => {
  const { page, errors } = await open();
  await page.evaluate(() => window.LeylineNav.show('den'));
  await page.waitForSelector('#yd-dens li');
  await page.click('#yd-dens li:nth-child(2) .btn:text-is("Share")');
  await page.click('#yd-dens .yd-share-dm');
  await page.waitForSelector('#dm-share-title');
  assert.equal(await page.inputValue('#dm-share-den'), 'garden');
  assert.equal(await page.evaluate(() => window.LeylineNav.current().tab), 'dens');
  assert.deepEqual(errors, []);
  await page.close();
});

test('a leyline://den link opens that build; signed out and not open yet say so', async () => {
  let { page, errors } = await open({ link: { action: 'den', id: 'gardengard' } });
  await page.waitForSelector('#dm .dm-detail .dm-name');
  assert.equal(await page.textContent('#dm .dm-name'), "Bea's Garden");
  assert.equal((await calls(page, 'dens_open')).length, 0, 'nothing downloaded before the click');
  assert.deepEqual(errors, []);
  await page.close();
  ({ page, errors } = await open({ signedOut: true }));
  await tab(page);
  await page.click('#dm-grid .dm-card[data-id="gardengard"]');
  await page.waitForSelector('#dm-like');
  await page.click('#dm-like');
  await page.waitForFunction(() => /Log in first/.test(document.querySelector('#dm .dm-status').textContent));
  await page.close();
  ({ page, errors } = await open({ closed: true }));
  await page.evaluate(() => window.LeylineNav.show('mods', false, 'dens'));
  await page.waitForFunction(() => /grand release/.test(document.querySelector('#dm .dm-status').textContent));
  assert.doesNotMatch(await page.textContent('#pane-dens'), BRANDS);
  assert.deepEqual(errors, []);
  await page.close();
});
