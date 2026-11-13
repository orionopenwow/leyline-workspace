// Leyline Launcher: the list kit. ONE way to show a list anywhere in the launcher (owner 12 Nov: "list features with
// one consistent style everywhere"): search, filter chips, tabs, sort, favourites, group headers, keyboard, empty /
// no-match / loading states and a count. The Realms page and the play bar's realm picker use it first; the Friends
// list (friends.js, worldfriends.js) and the Mods lists (addons.js, gamemods.js, looks.js, the Mods page) move onto it
// next. The full guide is docs/LAUNCHER-LISTS.md; the short one:
//
//   const list = LeylineList.create(el, {
//     id: "realms",                          // the list's name: favourites and the remembered tab / sort live under it
//     noun: ["realm", "realms"],             // the count: "12 realms", "3 of 12 realms" (or a function (shown, total))
//     key: (row) => row.id,                  // a stable key per row (favourites, focus and selection follow it)
//     render: (row, ctx) => Element,         // one row, your own markup (ctx below) ...
//     columns: [{ key, label, text(row), value(row), sortable, desc, width, align, className }],   // ... or columns
//     search: { fields: (row) => [strings] | ["name", "tags"], placeholder, label, input: <existing input>,
//               debounce: "auto" | ms },     // "auto": at once up to 150 rows, 120 ms above
//     tabs: { items: [{ id, label, long, filter(row, list), favourites, suggest, empty }], value, el: <container>,
//             idPrefix, panel: <tabpanel element>, storageKey, buttonClass },
//     filters: [{ id, label, test(row) }                                     // a toggle chip
//              | { id, label, options: [{ value, label }] | (rows) => [...], multi, test(row, value) }],   // a chip group
//     match: (row) => bool, filtersOn: () => n, onReset: () => {},           // the page's own filters, if it has some
//     sort: { options: [{ id, label, compare(a, b) }], value, storageKey, el: <container>, label },
//     rank: (row) => number,                 // kept before any sort (e.g. offline last whatever the sort)
//     group: { of: (row) => "Online", order: ["Online", "Offline"], label: (name, n) => text, hideSingle: true },
//     favourites: true | { label: "Favourites" },  // a star per row, kept in localStorage leyline.list.<id>.fav
//     primary: (row, rowEl) => {},           // Enter on a row, double-click (Play, Open, Install...)
//     empty: (info) => "text" | { text, actions: [{ label, run, primary }] } | null,   // null: the kit's own words
//     bar: <element> | false,                // where search / chips / sort / count go (default: the top of el)
//     role: "listbox" | "grid",              // listbox for plain rows; grid when rows hold their own buttons (default)
//     classes: { row, empty, group },        // extra class names (a page's old selectors keep working)
//     onChange: (view) => {},                // after each paint: { tab, query, sort, shown, total, inTab, counts }
//     onSelect: (row, rowEl) => {}, label: "Realms" (aria-label), nameOf: (row) => name (the star's label),
//     count: <element> | false,              // where the count goes (default: the bar)
//   });
//   list.update(rows)        new data; keeps the scroll, the focused row (and the button in it) and the selection
//   list.setLoading(on, text)  a loading line while there are no rows yet; a quiet busy mark when there are
//   list.setTab(id, focus) / tab()   list.setQuery(q) / query()   list.setSort(id, dir) / sort()
//   list.setFilter(id, value) / filters()   list.reset()  (search, chips and the page's own filters via onReset)
//   list.isFav(row|key) / toggleFav(row|key, on)   list.select(key) / selected()   list.focusRow(key)
//   list.visible()  the rows shown, in order    list.refresh()  repaint    list.destroy()
//
//   ctx (render's second argument): { query, hl(text) -> a fragment with the match marked, star() -> the favourite
//   button, fav, selected, index, list }
//
// Also: LeylineList.highlight(text, query), LeylineList.mark(root, query) (marks matches in place),
// LeylineList.favourites(id) (the store alone: has, toggle, list; the play bar reads the Realms page's this way),
// LeylineList.norm(text) (the search's folding: lower case, no accents).
//
// Keys: "/" goes to the search of the list on screen; Escape in the search clears it; Up / Down / Home / End /
// Page Up / Page Down move between rows (one tab stop: roving focus); Enter on a row is its primary action.
// Text only (textContent): nothing from a row is ever parsed as HTML. Styles: listkit.css, in the theme's tokens.
"use strict";
(() => {
  const mk = (tag, cls, text) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined && text !== null) e.textContent = text;
    return e;
  };
  const store = {
    get(k, fb) {
      try { const v = localStorage.getItem(k); return v === null ? fb : JSON.parse(v); } catch (_) { return fb; }
    },
    set(k, v) {
      try { localStorage.setItem(k, JSON.stringify(v)); } catch (_) { /* storage may be off: the list still works */ }
    },
    raw(k) { try { return localStorage.getItem(k); } catch (_) { return null; } },
    rawSet(k, v) { try { localStorage.setItem(k, v); } catch (_) { /* fine */ } },
  };

  // ---------------------------------------------------------------- search helpers
  const norm = (s) => String(s == null ? "" : s).normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
  // a fragment with every match of query in text wrapped in <mark class="lk-hl"> (case and accents ignored)
  function highlight(text, query) {
    const f = document.createDocumentFragment();
    text = String(text == null ? "" : text);
    const q = norm(query).trim();
    if (!q) { f.append(text); return f; }
    // norm() can change the length of a character (NFD); map back through the original one by one
    const chars = [...text];
    const folded = chars.map((c) => norm(c));
    const flat = folded.join("");
    let from = 0, pos = flat.indexOf(q);
    const starts = [];
    let acc = 0;
    for (const c of folded) { starts.push(acc); acc += c.length; }
    const idxAt = (n) => { let i = 0; while (i < starts.length && starts[i] < n) i++; return i; };
    while (pos !== -1 && q.length) {
      const a = idxAt(pos), b = idxAt(pos + q.length);
      if (a > from) f.append(chars.slice(from, a).join(""));
      f.append(mk("mark", "lk-hl", chars.slice(a, b).join("")));
      from = b;
      pos = flat.indexOf(q, pos + q.length);
    }
    if (from < chars.length) f.append(chars.slice(from).join(""));
    return f;
  }
  // marks the matches in place, in every text node under root (already marked text is left alone)
  function mark(root, query) {
    if (!root || !norm(query).trim()) return root;
    const walk = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
      acceptNode: (n) => (n.parentElement && n.parentElement.closest("mark.lk-hl") ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT),
    });
    const nodes = [];
    while (walk.nextNode()) nodes.push(walk.currentNode);
    const q = norm(query).trim();
    for (const n of nodes) if (norm(n.nodeValue).includes(q)) n.replaceWith(highlight(n.nodeValue, query));
    return root;
  }

  // ---------------------------------------------------------------- favourites: one store per list id, shared
  const favCache = {};
  function favourites(id) {
    const k = "leyline.list." + id + ".fav";
    if (favCache[k]) return favCache[k];
    let set = new Set((store.get(k, []) || []).filter((x) => typeof x === "string"));
    const api = {
      key: k,
      has: (key) => set.has(String(key)),
      list: () => [...set],
      toggle(key, on) {
        key = String(key);
        const want = on === undefined ? !set.has(key) : !!on;
        if (want === set.has(key)) return want;
        if (want) set.add(key); else set.delete(key);
        store.set(k, [...set]);
        document.dispatchEvent(new CustomEvent("leyline:list-fav", { detail: { list: id, key, on: want } }));
        return want;
      },
      clear() { set = new Set(); store.set(k, []); document.dispatchEvent(new CustomEvent("leyline:list-fav", { detail: { list: id, key: null, on: false } })); },
      reload() { set = new Set((store.get(k, []) || []).filter((x) => typeof x === "string")); },
    };
    favCache[k] = api;
    return api;
  }

  // ---------------------------------------------------------------- "/" goes to the search of the list on screen
  const live = new Set();
  const shown = (e) => !!(e && e.isConnected && e.getClientRects().length);
  const editable = (t) => t && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName));
  document.addEventListener("keydown", (e) => {
    if (e.key !== "/" || e.ctrlKey || e.metaKey || e.altKey || editable(e.target)) return;
    for (const l of live) {
      const input = l._searchInput();
      if (input && shown(l.el) && shown(input)) { e.preventDefault(); input.focus(); input.select(); return; }
    }
  });

  const ICON_STAR = '<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path d="M12 3.2l2.7 5.6 6.1.8-4.5 4.2 1.1 6.1L12 17l-5.4 2.9 1.1-6.1-4.5-4.2 6.1-.8z"/></svg>';
  const ICON_SEARCH = '<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false"><circle cx="11" cy="11" r="6.5"/><path d="M16 16l4.5 4.5"/></svg>';

  let uid = 0;
  function create(el, opts) {
    if (!el) throw new Error("LeylineList.create: no element");
    opts = Object.assign({}, opts || {});
    const id = String(opts.id || "list" + ++uid);
    const nid = id.replace(/[^a-z0-9_-]/gi, "-");
    const keyOf = typeof opts.key === "function" ? opts.key : (r) => r.id;
    const role = opts.role === "listbox" ? "listbox" : "grid";
    const classes = opts.classes || {};
    const cols = Array.isArray(opts.columns) ? opts.columns : null;
    const favOpt = opts.favourites ? (opts.favourites === true ? {} : opts.favourites) : null;
    const fav = favOpt ? favourites(favOpt.id || id) : null;
    const noun = opts.noun || ["item", "items"];

    // ---- state
    let rows = [];
    let shownRows = [];
    let loading = false, loadingText = "";
    let query = "";
    let selectedKey = null;
    let destroyed = false;
    const tabsOpt = opts.tabs && Array.isArray(opts.tabs.items) && opts.tabs.items.length ? opts.tabs : null;
    const tabStoreKey = tabsOpt ? (tabsOpt.storageKey || "leyline.list." + id + ".tab") : null;
    let tab = null;
    if (tabsOpt) {
      const saved = tabsOpt.remember === false ? null : store.raw(tabStoreKey);
      const ids = tabsOpt.items.map((t) => t.id);
      tab = ids.includes(saved) ? saved : ids.includes(tabsOpt.value) ? tabsOpt.value : ids[0];
    }
    const filterDefs = Array.isArray(opts.filters) ? opts.filters : [];
    const filters = {};   // id -> true (toggle) | value (single) | Set (multi)
    // sort: options from sort.options, plus the sortable columns
    const sortOpts = [];
    if (opts.sort && Array.isArray(opts.sort.options)) sortOpts.push(...opts.sort.options);
    if (cols) for (const c of cols) if (c.sortable !== false && c.key && !sortOpts.some((o) => o.id === c.key)) {
      const v = c.value || c.text || ((r) => r[c.key]);
      sortOpts.push({ id: c.key, label: c.label || c.key, column: true, desc: !!c.desc,
        compare: (a, b) => { const x = v(a), y = v(b); return typeof x === "number" && typeof y === "number" ? x - y : norm(x).localeCompare(norm(y)); } });
    }
    const sortStoreKey = opts.sort && opts.sort.storageKey !== false ? (opts.sort && opts.sort.storageKey) || "leyline.list." + id + ".sort" : null;
    let sort = { id: (opts.sort && opts.sort.value) || null, dir: 1 };
    if (sortStoreKey && sortOpts.length) {
      const s = store.get(sortStoreKey, null);
      if (s && typeof s === "object" && (s.id === null || sortOpts.some((o) => o.id === s.id))) sort = { id: s.id, dir: s.dir === -1 ? -1 : 1 };
    }

    // ---- DOM
    el.classList.add("lk");
    el.dataset.lkList = id;
    let bar = null;
    if (opts.bar !== false) {
      bar = opts.bar && opts.bar.nodeType === 1 ? opts.bar : mk("div");
      bar.classList.add("lk-bar");
      if (!(opts.bar && opts.bar.nodeType === 1)) el.append(bar);
    }
    // the search box: the page's own, or one of ours
    let input = null;
    const searchOpt = opts.search ? (opts.search === true ? {} : opts.search) : null;
    if (searchOpt) {
      if (searchOpt.input) input = searchOpt.input;
      else if (bar) {
        const wrap = mk("label", "lk-search");
        wrap.insertAdjacentHTML("afterbegin", ICON_SEARCH);
        input = mk("input");
        input.type = "search";
        input.placeholder = searchOpt.placeholder || "Search";
        input.setAttribute("aria-label", searchOpt.label || searchOpt.placeholder || "Search");
        input.autocomplete = "off";
        input.spellcheck = false;
        wrap.append(input);
        const kbd = mk("kbd", "lk-kbd", "/");
        kbd.title = "Press / to search";
        kbd.setAttribute("aria-hidden", "true");
        wrap.append(kbd);
        bar.append(wrap);
      }
      if (input) {
        input.setAttribute("aria-controls", (el.id || (el.id = "lk-" + nid)));
        query = input.value || "";
      }
    }
    // the tabs
    const tabBox = tabsOpt ? (tabsOpt.el || (bar ? bar.appendChild(mk("div")) : null)) : null;
    const tabBtns = {};
    if (tabBox) {
      tabBox.classList.add("lk-tabs");
      tabBox.setAttribute("role", "tablist");
      if (!tabBox.getAttribute("aria-label")) tabBox.setAttribute("aria-label", tabsOpt.label || "Lists");
      tabBox.replaceChildren();
      for (const t of tabsOpt.items) {
        const b = mk("button", (tabsOpt.buttonClass || "lk-tab") + " lk-tab-b");
        b.type = "button";
        b.setAttribute("role", "tab");
        b.id = (tabsOpt.idPrefix || "lk-" + nid + "-tab-") + t.id;
        b.dataset.lkTab = t.id;
        if (tabsOpt.panel && tabsOpt.panel.id) b.setAttribute("aria-controls", tabsOpt.panel.id);
        else b.setAttribute("aria-controls", el.id || (el.id = "lk-" + nid));
        b.append(mk("span", "lk-tab-l", t.label));
        const n = mk("span", "lk-tab-n");
        n.hidden = true;
        b.append(n);
        if (t.long && t.long !== t.label) b.title = t.long;
        b.addEventListener("click", () => setTab(t.id));
        b.addEventListener("keydown", (e) => {
          const ids = tabsOpt.items.map((x) => x.id).filter((x) => x === t.id || shown(tabBtns[x]));   // a tab a page hides is skipped
          const i = ids.indexOf(t.id);
          const j = e.key === "ArrowRight" ? i + 1 : e.key === "ArrowLeft" ? i - 1 : e.key === "Home" ? 0 : e.key === "End" ? ids.length - 1 : null;
          if (j === null) return;
          e.preventDefault();
          setTab(ids[(j + ids.length) % ids.length], true);
        });
        tabBtns[t.id] = b;
        tabBox.append(b);
      }
    }
    // the chips
    let chipBox = null;
    const chipBtns = {};
    if (filterDefs.length && bar) {
      chipBox = mk("div", "lk-chips");
      chipBox.setAttribute("role", "group");
      chipBox.setAttribute("aria-label", opts.filtersLabel || "Filters");
      bar.append(chipBox);
    }
    // the sort menu (a plain select: themed, keyboard-ready, and it says what it does)
    let sortSel = null;
    if (sortOpts.length && opts.sort !== false && !(opts.sort && opts.sort.menu === false)) {
      const where = (opts.sort && opts.sort.el) || bar;
      if (where) {
        const lab = mk("label", "lk-sort");
        lab.append(mk("span", "lk-sort-l", (opts.sort && opts.sort.label) || "Sort"));
        sortSel = mk("select", "lk-sort-s");
        sortSel.setAttribute("aria-label", (opts.sort && opts.sort.label) || "Sort");
        const none = opts.sort && opts.sort.none;   // the list's own order ("Suggested"), when there is one
        if (none) { const o = mk("option", "", none); o.value = ""; sortSel.append(o); }
        for (const o of sortOpts) {
          const op = mk("option", "", o.label);
          op.value = o.id;
          sortSel.append(op);
        }
        sortSel.addEventListener("change", () => {
          const o = sortOpts.find((x) => x.id === sortSel.value);
          setSort(o ? o.id : null, o && o.desc ? -1 : 1);
        });
        lab.append(sortSel);
        where.append(lab);
      }
    }
    // the count
    let countEl = null;
    if (opts.count !== false && bar) {
      countEl = opts.count && opts.count.nodeType === 1 ? opts.count : mk("span", "lk-count");
      countEl.setAttribute("aria-live", "polite");
      if (!(opts.count && opts.count.nodeType === 1)) bar.append(countEl);
    }
    // the header row of a column list
    let head = null;
    if (cols) {
      head = mk("div", "lk-head");
      head.setAttribute("role", role === "grid" ? "row" : "presentation");
      el.append(head);
    }
    // the rows
    const body = mk("div", "lk-body" + (classes.body ? " " + classes.body : ""));
    body.setAttribute("role", role);
    body.id = "lk-" + nid + "-body";
    if (opts.label) body.setAttribute("aria-label", opts.label);
    else if (tabsOpt && tabsOpt.panel) body.setAttribute("aria-label", "List");
    el.append(body);
    if (cols) el.style.setProperty("--lk-cols", (favOpt ? "34px " : "") + cols.map((c) => c.width || "minmax(0, 1fr)").join(" "));

    // ---- helpers
    const tabDef = (t) => (tabsOpt ? tabsOpt.items.find((x) => x.id === (t || tab)) : null);
    const isFav = (r) => !!fav && fav.has(typeof r === "object" && r !== null ? keyOf(r) : r);
    const api = {};
    const inTab = (r, t) => {
      const d = tabDef(t);
      if (!d) return true;
      if (d.favourites) return isFav(r);
      return typeof d.filter === "function" ? !!d.filter(r, api) : true;
    };
    const fieldsOf = (r) => {
      const f = searchOpt && searchOpt.fields;
      if (typeof f === "function") return f(r) || [];
      if (Array.isArray(f)) return f.map((k) => r[k]).flat();
      return [r.name, r.label, r.title];
    };
    const searched = (r) => {
      const q = norm(query).trim();
      if (!q) return true;
      const hay = norm(fieldsOf(r).filter((x) => x !== null && x !== undefined && x !== "").join("\n"));
      return q.split(/\s+/).every((w) => hay.includes(w));   // every word, in any order
    };
    const chipCount = () => Object.keys(filters).filter((k) => { const v = filters[k]; return v instanceof Set ? v.size : v !== undefined && v !== null && v !== "" && v !== false; }).length;
    const filtersOn = () => chipCount() + (typeof opts.filtersOn === "function" ? opts.filtersOn() || 0 : 0);
    const filtered = (r) => {
      for (const d of filterDefs) {
        const v = filters[d.id];
        if (v === undefined || v === null || v === "" || v === false || (v instanceof Set && !v.size)) continue;
        if (!d.options) { if (!d.test(r)) return false; continue; }
        if (v instanceof Set) { if (![...v].some((x) => d.test(r, x))) return false; }
        else if (!d.test(r, v)) return false;
      }
      return typeof opts.match === "function" ? !!opts.match(r) : true;
    };
    const countText = (n, total) => {
      if (typeof noun === "function") return noun(n, total);
      const w = (k) => (k === 1 ? noun[0] : noun[1]);
      return n === total ? `${n} ${w(n)}` : `${n} of ${total} ${w(total)}`;
    };
    const say = (n) => (typeof noun === "function" ? null : n === 1 ? noun[0] : noun[1]);

    // ---- chips
    function paintChips() {
      if (!chipBox) return;
      chipBox.replaceChildren();
      for (const d of filterDefs) {
        if (!d.options) {
          const b = chip(d.label, !!filters[d.id], () => setFilter(d.id, !filters[d.id]));
          b.dataset.lkFilter = d.id;
          chipBtns[d.id] = b;
          chipBox.append(b);
          continue;
        }
        const g = mk("span", "lk-chip-g");
        g.setAttribute("role", "group");
        if (d.label) { g.setAttribute("aria-label", d.label); g.append(mk("span", "lk-chip-gl", d.label)); }
        const options = typeof d.options === "function" ? d.options(rows) || [] : d.options;
        const cur = filters[d.id];
        if (!d.multi) {
          const all = chip(d.all || "All", cur === undefined || cur === null || cur === "", () => setFilter(d.id, null));
          all.dataset.lkFilter = d.id; all.dataset.lkValue = "";
          g.append(all);
        }
        for (const o of options) {
          const on = d.multi ? !!(cur instanceof Set && cur.has(o.value)) : cur === o.value;
          const b = chip(o.label, on, () => {
            if (d.multi) {
              const s = new Set(cur instanceof Set ? cur : []);
              if (s.has(o.value)) s.delete(o.value); else s.add(o.value);
              setFilter(d.id, s);
            } else setFilter(d.id, on ? null : o.value);
          });
          b.dataset.lkFilter = d.id;
          b.dataset.lkValue = String(o.value);
          g.append(b);
        }
        chipBox.append(g);
      }
      chipBox.hidden = !chipBox.children.length;
    }
    function chip(label, on, fn) {
      const b = mk("button", "lk-chip" + (on ? " on" : ""), label);
      b.type = "button";
      b.setAttribute("aria-pressed", String(!!on));
      b.addEventListener("click", fn);
      return b;
    }

    // ---- the header row
    function paintHead() {
      if (!head) return;
      head.replaceChildren();
      if (favOpt) { const s = mk("span", "lk-hc lk-hc-fav"); s.setAttribute("role", role === "grid" ? "columnheader" : "presentation"); s.setAttribute("aria-label", favOpt.label || "Favourite"); head.append(s); }
      for (const c of cols) {
        const cell = mk("span", "lk-hc" + (c.align ? " lk-" + c.align : ""));
        cell.setAttribute("role", role === "grid" ? "columnheader" : "presentation");
        const sortable = c.sortable !== false && c.key;
        if (sortable) {
          const on = sort.id === c.key;
          cell.setAttribute("aria-sort", on ? (sort.dir === -1 ? "descending" : "ascending") : "none");
          const b = mk("button", "lk-hb" + (on ? " on" : ""), c.label || "");
          b.type = "button";
          b.dataset.lkSort = c.key;
          if (on) b.append(mk("span", "lk-arrow" + (sort.dir === -1 ? " down" : ""), sort.dir === -1 ? "▼" : "▲"));
          // first click: its own way (desc for numbers when the column says so); again: the other way; a third: off
          b.addEventListener("click", () => {
            const first = c.desc ? -1 : 1;
            if (sort.id !== c.key) setSort(c.key, first);
            else if (sort.dir === first) setSort(c.key, -first);
            else setSort(null, 1);
          });
          cell.append(b);
        } else cell.textContent = c.label || "";
        head.append(cell);
      }
    }

    // ---- painting
    function order(list) {
      const o = sortOpts.find((x) => x.id === sort.id);
      const idx = new Map(list.map((r, i) => [r, i]));
      const rank = typeof opts.rank === "function" ? opts.rank : null;
      return list.slice().sort((a, b) => {
        if (rank) { const d = (rank(a) || 0) - (rank(b) || 0); if (d) return d; }
        if (o) { const d = o.compare(a, b) * sort.dir; if (d) return d; }
        return idx.get(a) - idx.get(b);
      });
    }
    function starBtn(r) {
      const b = mk("button", "lk-star");
      b.type = "button";
      const on = isFav(r);
      const name = opts.nameOf ? opts.nameOf(r) : (r.name || r.label || r.title || "");
      b.classList.toggle("on", on);
      b.setAttribute("aria-pressed", String(on));
      b.setAttribute("aria-label", on ? `Remove ${name} from favourites` : `Add ${name} to favourites`);
      b.title = on ? "In your favourites: click to take it out" : "Add to your favourites";
      b.insertAdjacentHTML("afterbegin", ICON_STAR);
      b.addEventListener("click", (e) => { e.stopPropagation(); toggleFav(r); });
      return b;
    }
    function rowEl(r, i) {
      const key = String(keyOf(r));
      const ctx = { query, hl: (t) => highlight(t, query), star: () => starBtn(r), fav: isFav(r), selected: key === selectedKey, index: i, list: api };
      let node;
      if (cols) {
        node = mk("div", "lk-row lk-crow");
        if (favOpt) { const c = mk("span", "lk-c lk-c-fav"); c.append(starBtn(r)); node.append(c); }
        for (const c of cols) {
          const cell = mk("span", "lk-c" + (c.align ? " lk-" + c.align : "") + (c.className ? " " + c.className : ""));
          if (typeof c.render === "function") { const v = c.render(r, ctx); if (v != null) cell.append(v); }
          else {
            const t = c.text ? c.text(r) : r[c.key];
            if (c.search !== false && query) cell.append(highlight(t == null ? "" : String(t), query));
            else cell.textContent = t == null ? "" : String(t);
          }
          node.append(cell);
        }
      } else {
        node = opts.render ? opts.render(r, ctx) : mk("div", "", String(r.name || r.label || key));
        if (!node) return null;
        node.classList.add("lk-row");
      }
      if (classes.row) for (const c of String(classes.row).split(/\s+/).filter(Boolean)) node.classList.add(c);
      node.dataset.lkKey = key;
      node.setAttribute("role", role === "grid" ? "row" : "option");
      node.setAttribute("aria-selected", String(key === selectedKey));
      if (role === "grid") for (const ch of node.children) if (!ch.getAttribute("role")) ch.setAttribute("role", "gridcell");
      node.tabIndex = -1;
      return node;
    }
    function groupEl(name, n) {
      const g = mk("div", "lk-group" + (classes.group ? " " + classes.group : ""));
      g.setAttribute("role", "presentation");
      const lab = opts.group && typeof opts.group.label === "function" ? opts.group.label(name, n) : name;
      g.append(mk("span", "lk-group-l", lab));
      g.append(mk("span", "lk-group-n", String(n)));
      g.dataset.lkGroup = name;
      return g;
    }
    // the words when nothing shows (info: why), and the buttons that get the player out of it
    function emptyEl(info) {
      let spec = typeof opts.empty === "function" ? opts.empty(info, api) : null;
      if (spec === undefined) spec = null;
      if (spec === false) return null;
      if (spec === null) spec = defaultEmpty(info);
      if (typeof spec === "string") spec = { text: spec };
      const box = mk("div", "lk-empty" + (classes.empty ? " " + classes.empty : ""));
      box.setAttribute("role", "status");
      box.dataset.lkReason = info.reason;
      box.classList.add("lk-empty-" + info.reason);   // its mark is drawn by listkit.css, so the words stay the words
      if (info.reason === "loading") { const sp = mk("span", "ly-spin"); sp.setAttribute("aria-hidden", "true"); box.append(sp); }
      box.append(mk("span", "lk-empty-t", spec.text || ""));
      for (const a of spec.actions || []) {
        const b = mk("button", "btn small" + (a.primary ? " primary" : ""), a.label);
        b.type = "button";
        b.addEventListener("click", () => a.run());
        box.append(b);
      }
      return box;
    }
    function defaultEmpty(info) {
      const many = say(2) || "items";
      if (info.reason === "loading") return { text: loadingText || `Loading the ${many}…` };
      if (info.reason === "none") return { text: (tabDef() && tabDef().empty) || `No ${many} yet.` };
      if (info.reason === "tab") {
        const d = tabDef();
        if (d && d.favourites) return { text: d.empty || "No favourites yet. Press the star by one to keep it here." };
        return { text: (d && d.empty) || `Nothing here yet.` };
      }
      // nomatch
      const acts = [];
      if (info.suggest) acts.push({ label: `Show ${info.suggest.n} in ${info.suggest.long}`, run: () => setTab(info.suggest.tab, true) });
      if (info.query && info.filtersOn) acts.push({ label: "Reset", run: () => reset() });
      else if (info.query) acts.push({ label: "Clear search", run: () => setQuery("", true) });
      else if (info.filtersOn) acts.push({ label: "Clear filters", run: () => resetFilters() });
      const text = info.query ? `No ${many} match “${info.query}”.` : `No ${many} match these filters.`;
      return { text: text + (info.query && !info.filtersOn ? " Try fewer words." : ""), actions: acts };
    }

    function paint() {
      if (destroyed) return;
      // what is focused now, to give it back after the paint (the row, or the n-th control in it)
      const act = document.activeElement;
      let fKey = null, fIdx = -1;
      if (act && body.contains(act)) {
        const row = act.closest("[data-lk-key]");
        if (row) {
          fKey = row.dataset.lkKey;
          fIdx = act === row ? -1 : [...row.querySelectorAll("button, a[href], input, select, [tabindex]")].indexOf(act);
        }
      }
      const scroller = body.scrollHeight > body.clientHeight ? body : null;
      const top = scroller ? scroller.scrollTop : 0;

      const base = rows.filter(filtered);
      const matching = base.filter(searched);
      const inTabRows = rows.filter((r) => inTab(r));
      shownRows = order(matching.filter((r) => inTab(r)));
      const counts = {};
      if (tabsOpt) for (const t of tabsOpt.items) counts[t.id] = matching.filter((r) => inTab(r, t.id)).length;

      // tabs: selected, one tab stop, the count of what they hold (with a search or filters on)
      if (tabsOpt) {
        for (const t of tabsOpt.items) {
          const b = tabBtns[t.id];
          if (!b) continue;
          const on = t.id === tab;
          b.setAttribute("aria-selected", String(on));
          b.tabIndex = on ? 0 : -1;
          b.classList.toggle("on", on);
          const n = b.querySelector(".lk-tab-n");
          const narrowed = !!(norm(query).trim() || filtersOn());
          n.hidden = !(t.count === true || (narrowed && t.count !== false));
          n.textContent = n.hidden ? "" : String(counts[t.id]);
        }
        if (tabsOpt.panel) tabsOpt.panel.setAttribute("aria-labelledby", tabBtns[tab] ? tabBtns[tab].id : "");
      }
      if (sortSel) sortSel.value = sort.id || (sortSel.querySelector('option[value=""]') ? "" : sortOpts[0].id);
      paintHead();

      const frag = document.createDocumentFragment();
      const grouping = opts.group && typeof opts.group.of === "function" ? opts.group : null;
      if (shownRows.length) {
        if (grouping) {
          const by = new Map();
          for (const r of shownRows) { const g = grouping.of(r) || ""; if (!by.has(g)) by.set(g, []); by.get(g).push(r); }
          const names = [...by.keys()].sort((a, b) => {
            const ord = grouping.order || [];
            const ia = ord.indexOf(a), ib = ord.indexOf(b);
            return (ia < 0 ? 999 : ia) - (ib < 0 ? 999 : ib);
          });
          const heads = !(grouping.hideSingle !== false && names.length < 2);
          shownRows = [];
          let i = 0;
          for (const g of names) {
            const list = by.get(g);
            if (heads && g) frag.append(groupEl(g, list.length));
            for (const r of list) { const n = rowEl(r, i++); if (n) frag.append(n); shownRows.push(r); }
          }
        } else shownRows.forEach((r, i) => { const n = rowEl(r, i); if (n) frag.append(n); });
      } else {
        const narrowed = norm(query).trim() || filtersOn();
        let reason = loading && !rows.length ? "loading" : !rows.length ? "none" : !inTabRows.length && !narrowed ? "tab" : "nomatch";
        if (reason === "nomatch" && !narrowed) reason = "tab";
        let suggest = null;
        if (reason === "nomatch" && tabsOpt) {
          for (const t of tabsOpt.items) {
            if (t.id === tab || t.suggest === false || t.favourites || !counts[t.id]) continue;
            suggest = { tab: t.id, n: counts[t.id], label: t.label, long: t.long || t.label };
            break;
          }
        }
        const info = { reason, query: query.trim(), filtersOn: filtersOn(), tab, total: rows.length, inTab: inTabRows.length, suggest, counts };
        const e = emptyEl(info);
        if (e) frag.append(e);
      }
      body.replaceChildren(frag);
      el.classList.toggle("lk-busy", loading && rows.length > 0);
      el.classList.toggle("lk-is-empty", !shownRows.length);
      if (countEl) {
        const total = tabsOpt ? inTabRows.length : rows.length;
        countEl.textContent = rows.length || !loading ? countText(shownRows.length, total) : "";
      }
      // the roving tab stop: the selected row, else the first
      const all = [...body.querySelectorAll(":scope > [data-lk-key]")];
      const stop = all.find((n) => n.dataset.lkKey === selectedKey) || all[0];
      if (stop) stop.tabIndex = 0;
      if (fKey !== null) {
        const row = all.find((n) => n.dataset.lkKey === fKey);
        if (row) {
          const ctl = fIdx >= 0 ? row.querySelectorAll("button, a[href], input, select, [tabindex]")[fIdx] : null;
          for (const n of all) n.tabIndex = n === row ? 0 : -1;
          (ctl || row).focus({ preventScroll: true });
        } else if (stop) stop.focus({ preventScroll: true });
      }
      if (scroller) body.scrollTop = top;
      if (typeof opts.onChange === "function") opts.onChange({ tab, query: query.trim(), sort: Object.assign({}, sort), shown: shownRows.length, total: rows.length, inTab: inTabRows.length, counts, filtersOn: filtersOn() }, api);
    }

    // ---- keys and pointer on the rows
    const rowsNow = () => [...body.querySelectorAll(":scope > [data-lk-key]")];
    function moveTo(node) {
      if (!node) return;
      for (const n of rowsNow()) n.tabIndex = n === node ? 0 : -1;
      node.focus();
      if (node.scrollIntoView) node.scrollIntoView({ block: "nearest" });
    }
    const rowOf = (key) => shownRows.find((r) => String(keyOf(r)) === key);
    body.addEventListener("keydown", (e) => {
      const row = e.target.closest && e.target.closest("[data-lk-key]");
      if (!row || row.parentElement !== body) return;
      const onRow = e.target === row;
      const list = rowsNow();
      const i = list.indexOf(row);
      let j = null;
      if (e.key === "ArrowDown") j = Math.min(list.length - 1, i + 1);
      else if (e.key === "ArrowUp") j = Math.max(0, i - 1);
      else if (e.key === "Home" && onRow) j = 0;
      else if (e.key === "End" && onRow) j = list.length - 1;
      else if (e.key === "PageDown") j = Math.min(list.length - 1, i + 10);
      else if (e.key === "PageUp") j = Math.max(0, i - 10);
      else if (e.key === "Enter" && onRow) {
        e.preventDefault();
        const r = rowOf(row.dataset.lkKey);
        if (r && typeof opts.primary === "function") opts.primary(r, row);
        return;
      } else if (e.key === " " && onRow) {   // Space: select (a listbox's own key)
        e.preventDefault();
        select(row.dataset.lkKey);
        return;
      }
      if (j === null) return;
      if (/^(INPUT|SELECT|TEXTAREA)$/.test(e.target.tagName)) return;
      e.preventDefault();
      moveTo(list[j]);
    });
    body.addEventListener("focusin", (e) => {
      const row = e.target.closest && e.target.closest("[data-lk-key]");
      if (!row || row.parentElement !== body) return;
      for (const n of rowsNow()) n.tabIndex = n === row ? 0 : -1;
      if (row.dataset.lkKey !== selectedKey) {
        selectedKey = row.dataset.lkKey;
        for (const n of rowsNow()) n.setAttribute("aria-selected", String(n === row));
        if (typeof opts.onSelect === "function") opts.onSelect(rowOf(selectedKey), row);
      }
    });
    body.addEventListener("dblclick", (e) => {
      if (e.target.closest("button, a, input, select, label")) return;
      const row = e.target.closest && e.target.closest("[data-lk-key]");
      if (!row || typeof opts.primary !== "function") return;
      const r = rowOf(row.dataset.lkKey);
      if (r) opts.primary(r, row);
    });
    body.addEventListener("click", (e) => {
      if (e.target.closest("button, a, input, select, label")) return;
      const row = e.target.closest && e.target.closest("[data-lk-key]");
      if (row && row.parentElement === body && document.activeElement !== row && !row.contains(document.activeElement)) row.focus({ preventScroll: true });
    });

    // ---- the search box
    let timer = null;
    function onInput() {
      const v = input.value;
      const wait = searchOpt.debounce === undefined || searchOpt.debounce === "auto" ? (rows.length > 150 ? 120 : 0) : +searchOpt.debounce || 0;
      clearTimeout(timer);
      if (!wait) { query = v; paint(); return; }
      timer = setTimeout(() => { query = v; paint(); }, wait);
    }
    function onSearchKey(e) {
      if (e.key === "Escape" && input.value) { e.preventDefault(); e.stopPropagation(); setQuery("", true); }
      else if (e.key === "ArrowDown") { const f = rowsNow()[0]; if (f) { e.preventDefault(); moveTo(f); } }
      else if (e.key === "Enter" && shownRows.length === 1 && typeof opts.primary === "function") {
        e.preventDefault();
        const n = rowsNow()[0];
        opts.primary(shownRows[0], n);
      }
    }
    if (input) { input.addEventListener("input", onInput); input.addEventListener("keydown", onSearchKey); }

    // ---- favourites from elsewhere (another list of the same id, the play bar)
    const onFav = (e) => { if (fav && e.detail && e.detail.list === (favOpt.id || id)) paint(); };
    if (fav) document.addEventListener("leyline:list-fav", onFav);

    // ---- the API
    function update(next) { rows = Array.isArray(next) ? next.slice() : []; paintChips(); paint(); return api; }
    function setLoading(on, text) { loading = !!on; loadingText = text || ""; paint(); return api; }
    function setTab(t, focus) {
      if (!tabsOpt || !tabsOpt.items.some((x) => x.id === t)) return api;
      const was = tab;
      tab = t;
      if (tabsOpt.remember !== false) store.rawSet(tabStoreKey, t);
      paint();
      if (focus && tabBtns[t]) tabBtns[t].focus();
      if (was !== t && typeof tabsOpt.onChange === "function") tabsOpt.onChange(t, was);
      return api;
    }
    function setQuery(q, focus) {
      clearTimeout(timer);
      query = String(q || "");
      if (input) input.value = query;
      paint();
      if (focus && input) input.focus();
      return api;
    }
    function setSort(sid, dir) {
      sort = { id: sid && sortOpts.some((o) => o.id === sid) ? sid : null, dir: dir === -1 ? -1 : 1 };
      if (sortStoreKey) store.set(sortStoreKey, sort);
      paint();
      return api;
    }
    function setFilter(fid, v) {
      if (v === null || v === undefined || v === false || v === "" || (v instanceof Set && !v.size)) delete filters[fid];
      else filters[fid] = v;
      paintChips();
      paint();
      return api;
    }
    function resetFilters() {
      for (const k of Object.keys(filters)) delete filters[k];
      if (typeof opts.onReset === "function") opts.onReset();
      paintChips();
      paint();
      return api;
    }
    function reset() { clearTimeout(timer); query = ""; if (input) input.value = ""; return resetFilters(); }
    function toggleFav(r, on) {
      if (!fav) return false;
      const key = typeof r === "object" && r !== null ? keyOf(r) : r;
      const v = fav.toggle(String(key), on);   // the event repaints every list of this id, this one too
      return v;
    }
    function select(key) {
      selectedKey = key === null || key === undefined ? null : String(key);
      for (const n of rowsNow()) { const on = n.dataset.lkKey === selectedKey; n.setAttribute("aria-selected", String(on)); }
      return api;
    }
    function focusRow(key) {
      const n = rowsNow().find((x) => x.dataset.lkKey === String(key));
      if (n) { select(key); moveTo(n); }
      return !!n;
    }
    function destroy() {
      destroyed = true;
      live.delete(api);
      if (fav) document.removeEventListener("leyline:list-fav", onFav);
      if (input) { input.removeEventListener("input", onInput); input.removeEventListener("keydown", onSearchKey); }
      clearTimeout(timer);
    }
    Object.assign(api, {
      el, body, bar, id, update, setLoading, setTab, tab: () => tab, setQuery, query: () => query, setSort, sort: () => Object.assign({}, sort),
      setFilter, filters: () => Object.assign({}, filters), resetFilters, reset, isFav, toggleFav, select, selected: () => (selectedKey === null ? null : rowOf(selectedKey) || null),
      focusRow, visible: () => shownRows.slice(), rows: () => rows.slice(), refresh: () => { paint(); return api; }, destroy,
      inTab: (r, t) => inTab(r, t), favourites: fav, _searchInput: () => input,
    });
    live.add(api);
    paintChips();
    paint();
    return api;
  }

  window.LeylineList = { create, highlight, mark, favourites, norm, version: 1 };
})();
