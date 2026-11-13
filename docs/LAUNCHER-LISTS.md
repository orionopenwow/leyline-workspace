# Launcher lists: the list kit

Every list in the Leyline launcher uses one kit, so it looks and works the same everywhere (owner, 12 Nov: "list
features with one consistent style everywhere"):

- search;
- filter chips;
- tabs;
- sort;
- favourites;
- group headers;
- keyboard;
- empty, no-match and loading states;
- the count.

| File | What |
|---|---|
| `launcher/ui/listkit.js` | `window.LeylineList`. Loaded before every page script (index.html) |
| `launcher/ui/listkit.css` | The look. Only the theme's `--t-*` tokens, so Dark, Light, every theme pack and the stone pages fit |
| `launcher/tests/listkit.browser.test.cjs` | The kit alone, on `tests/fixtures/listkit.html` |

**Uses it now:**
- the Realms page (`app.js` `rlList`);
- the play bar's realm picker, which reads the Realms page's favourites.

**Next:**
- the Friends list (`friends.js`, `worldfriends.js`);
- the Mods lists (`addons.js`, `gamemods.js`, `looks.js`, the Mods page).

## Making a list

```js
const list = LeylineList.create(document.getElementById("my-list"), {
  id: "friends",                      // favourites, the tab and the sort are kept under leyline.list.friends.*
  noun: ["friend", "friends"],        // the count: "12 friends", "3 of 12 friends"
  key: (f) => f.peer_id,              // stable across loads: favourites, focus and selection follow it
  render: (f, ctx) => {               // your own row. Build it with textContent, never innerHTML
    const row = document.createElement("div");
    const name = document.createElement("b");
    name.append(ctx.hl(f.name));      // the search match, marked
    row.append(name, ctx.star());     // the favourite star, if favourites are on
    return row;
  },
  search: { fields: (f) => [f.name, f.status, f.playing], placeholder: "Find a friend" },
  tabs: { items: [
    { id: "all", label: "All", suggest: false },
    { id: "on", label: "Online", filter: (f) => f.online },
    { id: "fav", label: "Favourites", favourites: true },
  ] },
  group: { of: (f) => (f.online ? "Online" : "Offline"), order: ["Online", "Offline"] },
  favourites: true,
  primary: (f, rowEl) => invite(f),   // Enter on a row, or a double-click
});
list.update(friends);                 // and again whenever the data changes
```

`update(rows)` repaints in one go. It keeps:
- the scroll;
- the focused row, and the button inside it that had focus;
- the selection.

So a list can repaint on a timer (Friends polls every 5 s) without the player losing their place.

## Options

| Option | What it does |
|---|---|
| `id` | The list's name. Its storage keys are `leyline.list.<id>.fav`, `.tab` and `.sort` |
| `key(row)` | A stable key per row. Default `row.id` |
| `render(row, ctx)` | Returns one row element. The kit adds the role, `data-lk-key`, `aria-selected` and the roving `tabindex` |
| `columns` | Instead of `render`: `[{ key, label, text(row), value(row), render(row, ctx), sortable, desc, width, align: "end" \| "center", className }]`. Header buttons sort: first click goes up (down if `desc`), the second click reverses, the third turns the sort off. The star column comes first |
| `search` | `{ fields: (row) => [...] \| ["name", "tags"], placeholder, label, input, debounce }`. Every word must match, in any field; case and accents are ignored. `input` adopts the page's own box (the Realms page keeps `#rl-q`). `debounce: "auto"` (the default) searches at once up to 150 rows and waits 120 ms above that; a number sets the wait |
| `tabs` | `{ items: [{ id, label, long, filter(row, list), favourites, suggest, empty, count }], value, el, idPrefix, panel, storageKey, buttonClass, label, remember, onChange(tab, was) }`. `el` adopts the page's tab strip; `idPrefix` names the buttons (`rl-tab-` gives `#rl-tab-official`); `panel` gets `aria-labelledby`. `long` is the name in "Show 2 in Community realms". `suggest: false` keeps a tab (All, Favourites) out of that offer. With a search or filters on, each tab shows how many it holds |
| `filters` | Chips. A toggle: `{ id, label, test(row) }`. A choice: `{ id, label, options: [{ value, label }] \| (rows) => [...], multi, all, test(row, value) }`. Single choice has an "All" chip; multi matches any of the picked |
| `match`, `filtersOn`, `onReset` | The page's own filters (the Realms page's FILTERS popover): an extra test, how many are on (for the words and the reset), and how to clear them |
| `sort` | `{ options: [{ id, label, compare(a, b), desc }], none: "Suggested", value, el, label, storageKey }`. A plain `<select>`. `none` is the list's own order |
| `rank(row)` | A number kept before any sort, lowest first. The Realms page uses it to keep offline realms last |
| `group` | `{ of(row), order: [...], label(name, n), hideSingle }`. A header with its count. One group alone gets no header (`hideSingle`, on by default) |
| `favourites` | `true` or `{ id, label }`. A star per row, kept in localStorage (no storage: kept for the visit). `id` shares the store with another list |
| `primary(row, rowEl)` | Enter on a row, a double-click, or Enter in the search when one row is left |
| `empty(info)` | Your words when nothing shows: a string, `{ text, actions: [{ label, run, primary }] }`, `null` for the kit's own words, or `false` for nothing. See the next table for `info` |
| `bar` | Where the search, chips, sort and count go: an element, or `false`. Default: the top of the list |
| `count` | An element for the count, or `false` |
| `role` | `"grid"` (default: rows that hold their own buttons) or `"listbox"` (plain rows) |
| `classes` | `{ row, empty, group, body }`: extra class names, so a page's old selectors keep working (the Realms page's `.rl-empty`) |
| `label` | The list's `aria-label` |
| `nameOf(row)` | The name in the star's label ("Add Busy Den to favourites"). Default: `row.name` |
| `onChange(view, list)` | After every paint: `{ tab, query, sort, shown, total, inTab, counts, filtersOn }`. Use it for lines outside the list (the Realms page's network line and FILTERS count) |
| `onSelect(row, rowEl)` | When the selection moves |

### What `empty(info)` gets

`info` is `{ reason, query, filtersOn, tab, total, inTab, suggest, counts }`.

| `reason` | When |
|---|---|
| `loading` | `setLoading(true)` and no rows yet |
| `none` | No rows at all |
| `tab` | The open tab holds nothing, with no search or filters on |
| `nomatch` | The search or filters hide everything |

`suggest` is `{ tab, n, long }` when another tab has matches.

The kit's own words:
- "No friends match “bo”." with **Clear search**;
- "… match these filters." with **Clear filters**;
- **Reset** for both together;
- **Show N in <tab>** when another tab has matches.

## The list's API

| Call | What |
|---|---|
| `update(rows)` | New data, same place |
| `setLoading(on, text)` | The loading line while there are no rows yet. With rows, the list is only marked busy |
| `setTab(id, focus)` / `tab()` | Change or read the open tab |
| `setQuery(q, focus)` / `query()` | Change or read the search |
| `setSort(id, dir)` / `sort()` | `dir` is `1` or `-1`; `id` null is the list's own order |
| `setFilter(id, value)` / `filters()` | `value` true for a toggle, a value, or a `Set` for multi |
| `resetFilters()` / `reset()` | The chips and your own filters (`onReset`); `reset()` clears the search too |
| `isFav(row \| key)` / `toggleFav(row \| key, on)` | Every list with the same favourites id repaints |
| `select(key)` / `selected()` / `focusRow(key)` | The selection, and moving the focus to a row |
| `visible()` / `rows()` | What is shown, in order / all the data |
| `refresh()` | Repaint (after something your render reads has changed) |
| `destroy()` | Stops listening |

**Also on `LeylineList`:**
- `highlight(text, query)`: a fragment;
- `mark(root, query)`: marks matches in place;
- `favourites(id)`: the store alone (`has`, `toggle`, `list`, `clear`);
- `norm(text)`: the search's folding.

The play bar reads `favourites("realms")` to list the starred realms.

## Keys

| Key | Does |
|---|---|
| `/` | Goes to the search of the list on screen (never while typing in a box) |
| Escape in the search | Clears it |
| Down in the search | Goes to the first row |
| Up / Down | Move between rows. The list is one tab stop (roving focus) |
| Home / End | The first or last row |
| Page Up / Page Down | Ten rows |
| Enter on a row | The primary action |
| Space on a row | Selects it |
| Tab | Goes into the row's own buttons |

## Rules for every list

- **Text only.** Rows are built with `textContent` (`ctx.hl` is safe); nothing from a server is parsed as HTML.
- **Words.** Same as the rest of the launcher: no brand words, no version numbers in sentences (the gold chip shows them).
- **Owner copy.** Copy in the owner's voice is `TODO(owner)`.
- **Old selectors.** Keep the page's ids and classes that tests use: adopt the page's own search box and tab strip (`search.input`, `tabs.el`, `tabs.idPrefix`) and add classes with `classes`.

## Moving a list onto the kit

### Friends (`friends.js` `drawFriends`)

- Rows are `s.friends`; key `peer_id`.
- Tabs: All, Online, Favourites.
- Groups: Online / Offline.
- `primary`: Invite to my world.
- The request and near lists can be two more lists with `bar: false`.
- `worldfriends.js` `paintFriend`: rows are `F.list`, key `realm_id`, `primary` is Join again.

### Mods (`addons.js`, `gamemods.js`, `looks.js`, the Mods page)

- Chips for the category (`shelf.js` GROUPS), "Installed" and "Fits my game folder".
- Sort: Popular, Newest, Name.
- `primary`: Install / Open.
- Keep `#cat-q` with `search.input`.
- `simple.js`'s smart chips become `filters`.

## The Realms page

`app.js` `rlList` follows the in-game Realm Selection window (`stream/fui-web/Interface/AddOns/WrathUnleashedUI/Leyline.lua`).

**Tabs:** All, Official, Community, Single player & Den, Friends' worlds, Favourites.
- The ids are `#rl-tab-<id>`.
- The tab is remembered in `leyline.realmsTab`; Official is the default.
- **Single player & Den** has the play bar's own rows (Your Den, Single player, and the original world on the Built-in engine), plus the realm this PC hosts.
- **Friends' worlds** has the realms your Leyline friends are on (`friends_state`, when they share what they play) and the friends' worlds this PC joined (`friend_worlds`, with Join again).
- A core without these commands shows none.

**Search:** name, line, type, rules, region, language, population and friends.

**Sort:** Suggested (the network order), Name, Population, Players · bots, Type. Offline realms stay last.

**Groups:** Online, On this PC, Friends' worlds, Offline.

**Population:** Low, Medium, High or Full, as in the game's own list.
- Against the realm's maximum when it has one: a quarter, 60 % and full.
- Otherwise 10 and 50 players (people, not bots).
- The bots count sits under the players.

**The star** sits at the end of each row's buttons. Starred realms also show under Favourites in the play bar's realm list.

**The FILTERS popover** (build, rules, region, language, has players, my game folders) is the page's own, through `match` / `filtersOn` / `onReset`. A filter is about realms, so this PC's own worlds step aside while one is on.

## The Mods page

Every list on Mods & Addons is the kit's. They share one favourites store, `favourites: { id: "mods" }`
(`leyline.list.mods.fav`), so a star on Discover is the same star on Installed, Looks and Game modes.

| List | Where | Tabs | Chips | Sort |
|---|---|---|---|---|
| Discover's full list | `app.js` `catListMake`, on `#cat-grid` | Mods / Addons / Looks / Themes / Graphics stay `themes.js`'s; the rows are the open tab's | the categories (`simple.js`, `market.js`) as the page's own filters (`match` / `filtersOn` / `onReset`), then the kit's: Works with this game folder (on by default, `leyline.catFit`), Installed, Updates, Favourites, Game (only with more than one game in the list) | `#cat-sort`: the hub's orders ask the core (`mods_ranked`); Name is the kit's own A to Z |
| Installed | `app.js` `instListMake`, on `#mods-list` | Mods / Addons (`#inst-tab-mods`, `#inst-tab-addons`, with `data-shelf` and the `#inst-n-*` badges; remembered in `leyline.instShelf`) | Updates, Turned off, Favourites | As installed, Name, Updates first |
| Looks | `looks.js`, on `#looks-list` (the cards stay `#looks-grid .lk-card`) | the kinds (`#looks-tab-all`, `-graphics-pack`, `-ui-look`, `-preset`, each with `data-type`) | Works with this game folder, In use, Favourites | Suggested, Name |
| Game modes | `gamemods.js`, on `#gm-list` | none: groups Play now, Game mode mods, Coming and hosted | Installed, Favourites | Suggested, Name |

- **Search:** `#cat-q` is Discover's box (`search.input`): name, author, description, tags, category and the addon's words.
- **Marks:** `decorateCard` (app.js) puts Installed or Update ready on every card, in every view.
- **Enter:** Install or Update when the card has it, else its page (a card with nothing to install never gets an Install).
- **After an install** the player stays on the same tab: the card is lit (`.just`) and keeps the focus. Installed opens on the
  tab of what went in last.
- **Pending stays pending:** the lists add no Install of their own. Install, Update and Use are the cards' own buttons,
  drawn from what the catalog and the core say, exactly as before the kit.
- Tests: `launcher/tests/modlists.browser.test.cjs` (screenshots with `SHOTS=<dir>`: `lists-mods-*.png`).
