// The list kit (ui/listkit.js, docs/LAUNCHER-LISTS.md) alone in Chromium (tests/fixtures/listkit.html): rows from data,
// the search (every word, accents folded, the match marked, "/" and Escape, the debounce), chips, tabs, the sort menu
// and column headers, favourites kept in localStorage (and storage that throws), group headers, the keys, update()
// keeping focus, selection and scroll, the empty, no-match and loading states, and a few hundred rows in good time.
//
//   NODE_PATH=$(npm root -g) node --test launcher/tests/listkit.browser.test.cjs
'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { chromium } = require('playwright');

const PAGE = 'file://' + path.join(__dirname, 'fixtures', 'listkit.html');
let browser;
test.before(async () => { browser = await chromium.launch(); });
test.after(async () => { await browser.close(); });

// a small realm-like list, made in the page: window.L is the list, window.ROWS its data, window.hits the primary calls
const MAKE = (extra) => {
  window.ROWS = [
    { id: 'a', name: 'Ashen Vale', kind: 'PvP', online: true, players: 40, tags: ['pvp'] },
    { id: 'b', name: 'Brightwater', kind: 'PvE', online: true, players: 5, tags: ['rp'] },
    { id: 'c', name: 'Cinder Keep', kind: 'PvE', online: false, players: 0, tags: [] },
    { id: 'd', name: 'Dûn Morrow', kind: 'RP', online: true, players: 12, tags: ['rp', 'pvp'] },
  ];
  window.hits = [];
  const opts = Object.assign({
    id: 'test',
    noun: ['realm', 'realms'],
    key: (r) => r.id,
    render: (r, ctx) => {
      const row = document.createElement('div');
      row.className = 'row';
      const n = document.createElement('b');
      n.className = 'name';
      n.append(ctx.hl(r.name));
      const go = document.createElement('button');
      go.type = 'button';
      go.className = 'go';
      go.textContent = 'Play';
      go.addEventListener('click', () => window.hits.push('click:' + r.id));
      row.append(n, go, ctx.star());
      return row;
    },
    search: { fields: (r) => [r.name, r.kind, ...r.tags], placeholder: 'Find a realm', debounce: 0 },
    primary: (r) => window.hits.push(r.id),
    favourites: true,
  }, extra || {});
  window.L = window.LeylineList.create(document.getElementById('a'), opts);
  window.L.update(window.ROWS);
};
const names = (page) => page.$$eval('#a .lk-body > [data-lk-key] .name', (n) => n.map((x) => x.textContent));

async function open(extra, init) {
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  if (init) await page.addInitScript(init);
  await page.goto(PAGE);
  // functions in the options can't cross into the page: build them there from source
  await page.evaluate(`(${MAKE.toString()})(${extra || 'undefined'})`);
  return { page, errors };
}

test('rows, the count, the search: every word, accents folded, the match marked; / and Escape', async () => {
  const { page, errors } = await open();
  assert.deepEqual(await names(page), ['Ashen Vale', 'Brightwater', 'Cinder Keep', 'Dûn Morrow']);
  assert.equal(await page.textContent('#a .lk-count'), '4 realms');
  assert.equal(await page.getAttribute('#a .lk-search input', 'placeholder'), 'Find a realm');
  await page.fill('#a .lk-search input', 'water');
  assert.deepEqual(await names(page), ['Brightwater']);
  assert.equal(await page.textContent('#a .lk-body mark.lk-hl'), 'water', 'the match is marked');
  assert.equal(await page.textContent('#a .lk-count'), '1 of 4 realms');
  await page.fill('#a .lk-search input', 'dun');
  assert.deepEqual(await names(page), ['Dûn Morrow'], 'accents folded');
  assert.equal(await page.textContent('#a .lk-body mark.lk-hl'), 'Dûn', 'the original letters are marked');
  await page.fill('#a .lk-search input', 'pvp rp');
  assert.deepEqual(await names(page), ['Dûn Morrow'], 'every word, in any field');
  // Escape clears; / from the page goes to the search, but never from another box
  await page.focus('#a .lk-search input');
  await page.keyboard.press('Escape');
  assert.equal(await page.inputValue('#a .lk-search input'), '');
  assert.equal((await names(page)).length, 4);
  await page.click('body', { position: { x: 5, y: 5 } });
  await page.keyboard.press('/');
  assert.ok(await page.evaluate(() => document.activeElement.closest('.lk-search') !== null), '/ focuses the search');
  assert.equal(await page.inputValue('#a .lk-search input'), '', 'and types nothing');
  await page.focus('#outside');
  await page.keyboard.type('a/b');
  assert.equal(await page.inputValue('#outside'), 'a/b', 'a / typed elsewhere stays there');
  // a no-match: the words, and Clear search
  await page.fill('#a .lk-search input', 'nothing like it');
  assert.match(await page.textContent('#a .lk-empty'), /No realms match “nothing like it”\./);
  assert.equal(await page.getAttribute('#a .lk-empty', 'data-lk-reason'), 'nomatch');
  await page.click('#a .lk-empty button:has-text("Clear search")');
  assert.equal(await page.inputValue('#a .lk-search input'), '');
  assert.equal((await names(page)).length, 4);
  assert.deepEqual(errors, []);
  await page.close();
});

test('the debounce: a set wait, and "auto" (at once for a short list)', async () => {
  const { page, errors } = await open("{ search: { fields: (r) => [r.name], debounce: 200 } }");
  await page.fill('#a .lk-search input', 'ash');
  assert.equal((await names(page)).length, 4, 'not yet');
  await page.waitForFunction(() => document.querySelectorAll('#a .lk-body > [data-lk-key]').length === 1);
  await page.evaluate(() => { window.L.destroy(); document.getElementById('a').replaceChildren(); });
  await page.evaluate(`(${MAKE.toString()})({ search: { fields: (r) => [r.name], debounce: 'auto' } })`);
  await page.fill('#a .lk-search input', 'ash');
  assert.equal((await names(page)).length, 1, 'four rows: at once');
  assert.deepEqual(errors, []);
  await page.close();
});

test('chips: a toggle, a single choice and a multi choice; no match; Clear filters', async () => {
  const { page, errors } = await open(`{ filters: [
    { id: 'on', label: 'Online', test: (r) => r.online },
    { id: 'kind', label: 'Type', options: [{ value: 'PvE', label: 'PvE' }, { value: 'PvP', label: 'PvP' }, { value: 'RP', label: 'RP' }], test: (r, v) => r.kind === v },
    { id: 'tag', label: 'Tags', multi: true, options: (rows) => [...new Set(rows.flatMap((r) => r.tags))].sort().map((t) => ({ value: t, label: t })), test: (r, v) => r.tags.includes(v) },
  ] }`);
  const chip = (f, v) => `#a .lk-chip[data-lk-filter="${f}"]` + (v === undefined ? '' : `[data-lk-value="${v}"]`);
  assert.deepEqual(await page.$$eval('#a .lk-chip', (c) => c.map((x) => x.textContent)), ['Online', 'All', 'PvE', 'PvP', 'RP', 'pvp', 'rp']);
  await page.click(chip('on'));
  assert.equal(await page.getAttribute(chip('on'), 'aria-pressed'), 'true');
  assert.deepEqual(await names(page), ['Ashen Vale', 'Brightwater', 'Dûn Morrow']);
  await page.click(chip('kind', 'PvE'));
  assert.deepEqual(await names(page), ['Brightwater']);
  assert.equal(await page.getAttribute(chip('kind', ''), 'aria-pressed'), 'false');
  await page.click(chip('kind', ''));
  await page.click(chip('tag', 'pvp'));
  await page.click(chip('tag', 'rp'));
  assert.deepEqual(await names(page), ['Ashen Vale', 'Brightwater', 'Dûn Morrow'], 'multi: any of them');
  await page.click(chip('tag', 'rp'));
  assert.deepEqual(await names(page), ['Ashen Vale', 'Dûn Morrow']);
  await page.click(chip('kind', 'PvE'));
  assert.match(await page.textContent('#a .lk-empty'), /No realms match these filters\./);
  await page.click('#a .lk-empty button:has-text("Clear filters")');
  assert.equal((await names(page)).length, 4);
  assert.equal(await page.$$eval('#a .lk-chip.on', (c) => c.length), 1, 'only the All chip of the single choice');
  // a search and filters together: Reset clears both
  await page.click(chip('on'));
  await page.fill('#a .lk-search input', 'cinder');
  await page.click('#a .lk-empty button:has-text("Reset")');
  assert.equal(await page.inputValue('#a .lk-search input'), '');
  assert.equal((await names(page)).length, 4);
  assert.deepEqual(errors, []);
  await page.close();
});

test('tabs: the filter, the keys, remembered; a no-match offers the tab that has it', async () => {
  const { page, errors } = await open(`{ tabs: { items: [
    { id: 'all', label: 'All', suggest: false },
    { id: 'on', label: 'Online', long: 'Online realms', filter: (r) => r.online },
    { id: 'off', label: 'Offline', filter: (r) => !r.online, empty: 'Every realm is up.' },
    { id: 'fav', label: 'Favourites', favourites: true },
  ], value: 'on' } }`);
  assert.equal(await page.getAttribute('#a [role="tablist"]', 'role'), 'tablist');
  assert.equal(await page.getAttribute('#lk-test-tab-on', 'aria-selected'), 'true', 'the default');
  assert.deepEqual(await names(page), ['Ashen Vale', 'Brightwater', 'Dûn Morrow']);
  assert.equal(await page.textContent('#a .lk-count'), '3 realms', 'the count of the open tab');
  await page.focus('#lk-test-tab-on');
  await page.keyboard.press('ArrowRight');
  assert.equal(await page.getAttribute('#lk-test-tab-off', 'aria-selected'), 'true');
  assert.equal(await page.evaluate(() => document.activeElement.id), 'lk-test-tab-off');
  assert.equal(await page.getAttribute('#lk-test-tab-on', 'tabindex'), '-1');
  assert.deepEqual(await names(page), ['Cinder Keep']);
  await page.keyboard.press('End');
  assert.equal(await page.getAttribute('#lk-test-tab-fav', 'aria-selected'), 'true');
  assert.match(await page.textContent('#a .lk-empty'), /No favourites yet/);
  await page.keyboard.press('ArrowRight');
  assert.equal(await page.getAttribute('#lk-test-tab-all', 'aria-selected'), 'true', 'wraps');
  await page.click('#lk-test-tab-off');
  assert.equal(await page.evaluate(() => localStorage.getItem('leyline.list.test.tab')), 'off');
  // a search that finds nothing here but something on another tab: a counted tab and a button to it
  await page.fill('#a .lk-search input', 'ashen');
  assert.equal(await page.textContent('#lk-test-tab-on .lk-tab-n'), '1');
  assert.equal(await page.textContent('#lk-test-tab-off .lk-tab-n'), '0');
  await page.click('#a .lk-empty button:has-text("Show 1 in Online realms")');
  assert.equal(await page.getAttribute('#lk-test-tab-on', 'aria-selected'), 'true');
  assert.deepEqual(await names(page), ['Ashen Vale']);
  await page.fill('#a .lk-search input', '');
  assert.equal(await page.isHidden('#lk-test-tab-on .lk-tab-n'), true, 'no counts without a search');
  // remembered on the next start
  await page.click('#lk-test-tab-off');
  await page.evaluate(() => { window.L.destroy(); document.getElementById('a').replaceChildren(); });
  await page.evaluate(`(${MAKE.toString()})({ tabs: { items: [{ id: 'on', label: 'Online', filter: (r) => r.online }, { id: 'off', label: 'Offline', filter: (r) => !r.online }], value: 'on' } })`);
  assert.equal(await page.getAttribute('#lk-test-tab-off', 'aria-selected'), 'true');
  // an empty tab: its own words
  await page.evaluate(() => window.L.update(window.ROWS.filter((r) => r.online)));
  assert.match(await page.textContent('#a .lk-empty'), /Nothing here yet/);
  assert.deepEqual(errors, []);
  await page.close();
});

test('sort: the menu, column headers (up, down, off), and rank before any sort', async () => {
  const { page, errors } = await open(`{ render: null,
    columns: [{ key: 'name', label: 'Realm Name', width: 'minmax(0, 2fr)' }, { key: 'kind', label: 'Type' },
      { key: 'players', label: 'Players', value: (r) => r.players, desc: true, align: 'end' }],
    sort: { none: 'Suggested', options: [{ id: 'z', label: 'Z to A', compare: (a, b) => b.name.localeCompare(a.name) }] },
    rank: (r) => (r.online ? 0 : 1) }`);
  const cells = () => page.$$eval('#a .lk-body > [data-lk-key]', (rs) => rs.map((r) => r.children[1].textContent));
  assert.deepEqual(await cells(), ['Ashen Vale', 'Brightwater', 'Dûn Morrow', 'Cinder Keep'], 'offline last (rank)');
  assert.equal(await page.getAttribute('#a .lk-head', 'role'), 'row');
  assert.deepEqual(await page.$$eval('#a .lk-sort-s option', (o) => o.map((x) => x.textContent)), ['Suggested', 'Z to A', 'Realm Name', 'Type', 'Players']);
  await page.selectOption('#a .lk-sort-s', 'z');
  assert.deepEqual(await cells(), ['Dûn Morrow', 'Brightwater', 'Ashen Vale', 'Cinder Keep']);
  // the Players header: down first (it says so), then up, then off
  await page.click('#a .lk-hb[data-lk-sort="players"]');
  assert.deepEqual(await cells(), ['Ashen Vale', 'Dûn Morrow', 'Brightwater', 'Cinder Keep']);
  assert.equal(await page.getAttribute('#a .lk-hc:has(.lk-hb[data-lk-sort="players"])', 'aria-sort'), 'descending');
  assert.equal(await page.inputValue('#a .lk-sort-s'), 'players', 'the menu follows');
  await page.click('#a .lk-hb[data-lk-sort="players"]');
  assert.deepEqual(await cells(), ['Brightwater', 'Dûn Morrow', 'Ashen Vale', 'Cinder Keep']);
  await page.click('#a .lk-hb[data-lk-sort="players"]');
  assert.equal(await page.inputValue('#a .lk-sort-s'), '', 'off: Suggested');
  assert.deepEqual(await cells(), ['Ashen Vale', 'Brightwater', 'Dûn Morrow', 'Cinder Keep']);
  await page.click('#a .lk-hb[data-lk-sort="name"]');
  assert.equal(await page.evaluate(() => JSON.parse(localStorage.getItem('leyline.list.test.sort')).id), 'name', 'remembered');
  // the star column comes first in a column list
  assert.equal(await page.locator('#a .lk-body > [data-lk-key] >> nth=0').locator('.lk-c-fav .lk-star').count(), 1);
  assert.deepEqual(errors, []);
  await page.close();
});

test('favourites: the star, kept in localStorage, shared by lists of the same id; storage that throws still works', async () => {
  const { page, errors } = await open(`{ tabs: { items: [{ id: 'all', label: 'All' }, { id: 'fav', label: 'Favourites', favourites: true }], remember: false } }`);
  const star = (k) => `#a [data-lk-key="${k}"] .lk-star`;
  assert.equal(await page.getAttribute(star('b'), 'aria-label'), 'Add Brightwater to favourites');
  await page.click(star('b'));
  assert.equal(await page.getAttribute(star('b'), 'aria-pressed'), 'true');
  assert.ok(await page.locator(star('b')).evaluate((e) => e.classList.contains('on')));
  assert.deepEqual(await page.evaluate(() => JSON.parse(localStorage.getItem('leyline.list.test.fav'))), ['b']);
  // a second list of the same id follows at once
  await page.evaluate(() => {
    window.L2 = window.LeylineList.create(document.getElementById('b'), { id: 'test', key: (r) => r.id, favourites: true,
      render: (r, ctx) => { const d = document.createElement('div'); d.textContent = r.name; d.append(ctx.star()); return d; } });
    window.L2.update(window.ROWS);
  });
  assert.equal(await page.getAttribute('#b [data-lk-key="b"] .lk-star', 'aria-pressed'), 'true');
  await page.click('#b [data-lk-key="d"] .lk-star');
  await page.click('#lk-test-tab-fav');
  assert.deepEqual(await names(page), ['Brightwater', 'Dûn Morrow']);
  assert.equal(await page.evaluate(() => window.LeylineList.favourites('test').has('d')), true, 'the store alone');
  await page.click(star('b'));
  assert.deepEqual(await names(page), ['Dûn Morrow'], 'unstarred: gone from Favourites');
  // kept across a reload
  await page.reload();
  await page.evaluate(`(${MAKE.toString()})()`);
  assert.equal(await page.getAttribute(star('d'), 'aria-pressed'), 'true');
  assert.deepEqual(errors, []);
  await page.close();
  // localStorage that throws: stars still work for the visit
  const p2 = await open(undefined, () => { Object.defineProperty(window, 'localStorage', { get() { throw new Error('blocked'); } }); });
  await p2.page.click(star('a'));
  assert.equal(await p2.page.getAttribute(star('a'), 'aria-pressed'), 'true');
  assert.deepEqual(p2.errors, []);
  await p2.page.close();
});

test('group headers with counts; one group alone has no header', async () => {
  const { page, errors } = await open(`{ group: { of: (r) => (r.online ? 'Online' : 'Offline'), order: ['Online', 'Offline'] } }`);
  assert.deepEqual(await page.$$eval('#a .lk-body > *', (n) => n.map((x) => (x.classList.contains('lk-group') ? '# ' + x.querySelector('.lk-group-l').textContent + ' ' + x.querySelector('.lk-group-n').textContent : x.querySelector('.name').textContent))),
    ['# Online 3', 'Ashen Vale', 'Brightwater', 'Dûn Morrow', '# Offline 1', 'Cinder Keep']);
  await page.fill('#a .lk-search input', 'vale');
  assert.equal(await page.locator('#a .lk-group').count(), 0, 'one group: no header');
  assert.deepEqual(errors, []);
  await page.close();
});

test('keys: listbox, roving focus, arrows, Home / End, Enter is the primary action, Space selects', async () => {
  const { page, errors } = await open(`{ role: 'listbox', label: 'Realms' }`);
  assert.equal(await page.getAttribute('#a .lk-body', 'role'), 'listbox');
  assert.equal(await page.getAttribute('#a .lk-body', 'aria-label'), 'Realms');
  const roles = await page.$$eval('#a .lk-body > [data-lk-key]', (r) => r.map((x) => [x.getAttribute('role'), x.tabIndex]));
  assert.deepEqual(roles, [['option', 0], ['option', -1], ['option', -1], ['option', -1]], 'one tab stop');
  await page.focus('#a .lk-search input');
  await page.keyboard.press('ArrowDown');
  const focused = () => page.evaluate(() => document.activeElement.dataset.lkKey);
  assert.equal(await focused(), 'a');
  await page.keyboard.press('ArrowDown');
  assert.equal(await focused(), 'b');
  assert.equal(await page.getAttribute('#a [data-lk-key="b"]', 'aria-selected'), 'true');
  assert.equal(await page.getAttribute('#a [data-lk-key="a"]', 'aria-selected'), 'false');
  assert.equal(await page.getAttribute('#a [data-lk-key="b"]', 'tabindex'), '0');
  await page.keyboard.press('End');
  assert.equal(await focused(), 'd');
  await page.keyboard.press('ArrowDown');
  assert.equal(await focused(), 'd', 'stays at the end');
  await page.keyboard.press('Home');
  assert.equal(await focused(), 'a');
  await page.keyboard.press('Enter');
  assert.deepEqual(await page.evaluate(() => window.hits), ['a'], 'Enter: the primary action');
  // Enter on a button in the row is that button's, not the row's
  await page.keyboard.press('Tab');
  assert.equal(await page.evaluate(() => document.activeElement.className), 'go');
  await page.keyboard.press('Enter');
  assert.deepEqual(await page.evaluate(() => window.hits), ['a', 'click:a']);
  // a double-click on the row is the primary action too
  await page.dblclick('#a [data-lk-key="c"] .name');
  assert.deepEqual(await page.evaluate(() => window.hits), ['a', 'click:a', 'c']);
  // Enter in the search with one row left
  await page.fill('#a .lk-search input', 'bright');
  await page.keyboard.press('Enter');
  assert.deepEqual(await page.evaluate(() => window.hits), ['a', 'click:a', 'c', 'b']);
  assert.deepEqual(errors, []);
  await page.close();
});

test('update(rows) keeps the focus (the button in the row too), the selection and the scroll', async () => {
  const { page, errors } = await open();
  await page.evaluate(() => {
    const many = Array.from({ length: 60 }, (_, i) => ({ id: 'r' + i, name: 'Realm ' + i, kind: 'PvE', online: true, players: i, tags: [] }));
    document.getElementById('a').style.cssText = 'max-width:760px';
    window.L.body.style.cssText = 'max-height:300px; overflow:auto';
    window.L.update(many);
  });
  await page.locator('#a [data-lk-key="r30"] .go').focus();
  await page.evaluate(() => { window.L.body.scrollTop = 900; });
  const before = await page.evaluate(() => window.L.body.scrollTop);
  await page.evaluate(() => window.L.update(window.L.rows().map((r) => Object.assign({}, r, { players: r.players + 1 }))));
  assert.equal(await page.evaluate(() => document.activeElement.closest('[data-lk-key]').dataset.lkKey), 'r30');
  assert.equal(await page.evaluate(() => document.activeElement.className), 'go', 'the same button');
  assert.equal(await page.getAttribute('#a [data-lk-key="r30"]', 'aria-selected'), 'true');
  assert.equal(await page.evaluate(() => window.L.body.scrollTop), before, 'the scroll stays');
  assert.equal(await page.evaluate(() => window.L.selected().id), 'r30');
  // the focused row is gone: focus goes to the list, not to the page
  await page.evaluate(() => window.L.update(window.L.rows().filter((r) => r.id !== 'r30')));
  assert.ok(await page.evaluate(() => window.L.body.contains(document.activeElement)));
  assert.deepEqual(errors, []);
  await page.close();
});

test('empty, loading and the page\'s own empty words', async () => {
  const { page, errors } = await open(`{ empty: (info) => (info.reason === 'none' ? { text: 'No realms yet: host one.', actions: [{ label: 'Host', run: () => window.hits.push('host') }] } : null) }`);
  await page.evaluate(() => { window.L.update([]); window.L.setLoading(true, 'Loading the realm list…'); });
  assert.equal(await page.getAttribute('#a .lk-empty', 'data-lk-reason'), 'loading');
  assert.match(await page.textContent('#a .lk-empty'), /Loading the realm list…/);
  assert.equal(await page.locator('#a .lk-empty .ly-spin').count(), 1);
  assert.equal(await page.textContent('#a .lk-count'), '', 'no count while loading');
  await page.evaluate(() => window.L.setLoading(false));
  assert.equal(await page.textContent('#a .lk-empty'), 'No realms yet: host one.Host');
  await page.click('#a .lk-empty button');
  assert.deepEqual(await page.evaluate(() => window.hits), ['host']);
  // loading with rows: the rows stay, the list is marked busy
  await page.evaluate(() => { window.L.update(window.ROWS); window.L.setLoading(true); });
  assert.equal((await names(page)).length, 4);
  assert.ok(await page.locator('#a').evaluate((e) => e.classList.contains('lk-busy')));
  assert.deepEqual(errors, []);
  await page.close();
});

test('a few hundred rows: painted and searched in good time; text only', async () => {
  const { page, errors } = await open();
  const ms = await page.evaluate(() => {
    const many = Array.from({ length: 600 }, (_, i) => ({ id: 'r' + i, name: (i % 7 ? 'Realm ' : '<img src=x onerror=alert(1)> ') + i, kind: i % 2 ? 'PvE' : 'PvP', online: i % 5 !== 0, players: i, tags: ['t' + (i % 9)] }));
    const t0 = performance.now();
    window.L.update(many);
    const t1 = performance.now();
    window.L.setQuery('realm 55');
    const t2 = performance.now();
    return [t1 - t0, t2 - t1];
  });
  assert.ok(ms[0] < 400, `600 rows painted in ${Math.round(ms[0])} ms`);
  assert.ok(ms[1] < 400, `searched in ${Math.round(ms[1])} ms`);
  assert.equal(await page.locator('#a img').count(), 0, 'a name is never HTML');
  assert.deepEqual(await names(page), Array.from({ length: 600 }, (_, i) => i).filter((i) => i % 7 && String(i).includes('55')).map((i) => 'Realm ' + i));
  // above 150 rows "auto" waits a little before searching
  await page.evaluate(() => { window.L.destroy(); document.getElementById('a').replaceChildren(); });
  await page.evaluate(`(${MAKE.toString()})({ search: { fields: (r) => [r.name], debounce: 'auto' } })`);
  await page.evaluate(() => window.L.update(Array.from({ length: 300 }, (_, i) => ({ id: 'r' + i, name: 'Realm ' + i, tags: [] }))));
  await page.fill('#a .lk-search input', 'realm 299');
  assert.equal((await names(page)).length, 300, 'not at once');
  await page.waitForFunction(() => document.querySelectorAll('#a .lk-body > [data-lk-key]').length === 1);
  assert.deepEqual(errors, []);
  await page.close();
});

test('LeylineList.highlight and mark', async () => {
  const { page, errors } = await open();
  const got = await page.evaluate(() => {
    const d = document.createElement('div');
    d.append(window.LeylineList.highlight('Café Crème', 'cafe'));
    const m = document.createElement('p');
    m.innerHTML = '<b>Frost</b> Haven and frost';
    window.LeylineList.mark(m, 'frost');
    return [d.innerHTML, m.querySelectorAll('mark.lk-hl').length, m.textContent, window.LeylineList.norm('Dûn')];
  });
  assert.deepEqual(got, ['<mark class="lk-hl">Café</mark> Crème', 2, 'Frost Haven and frost', 'dun']);
  assert.deepEqual(errors, []);
  await page.close();
});
