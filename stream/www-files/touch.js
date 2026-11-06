// touch.js — touch controls for phones and tablets on the WrathUnleashed (3.3.5a) browser page.
//
// Ported from Open Vanilla's 1.12 page layer (realm/www/touch.js, build '2026-10-24 clicks when it
// shows pushed'); the behaviour is the same, the game side is 3.3.5a's (see "3.3.5a" below).
// Loaded by wrath-page.js; it only switches on for a touch screen (a coarse pointer and touch
// points), or with ?touch=1 (?touch=0 turns it off, remembered as "ow:touch").
// Everything goes through the page bridge (window.wenilla) by way of touch-driver.js, so the game
// sees exactly what keys and a mouse would do; without a bridge in the client the driver sends the
// 3.3.5a default keys instead (movement, jump, target, action keys 1-=, the tray's windows):
//
//   * Floating stick, bottom-left (24 px in, 120 px range): holds MOVEFORWARD / MOVEBACKWARD / STRAFELEFT / STRAFERIGHT; a double-tap toggles auto-run.
//   * Right-thumb cluster, pivoting on the bottom-right corner: a 72 px Attack/Interact key showing
//     what a tap does with the target in the game's own art (the hover sword for an enemy, loot
//     bags, speech bubble, anvil hammer, trainer's book, flight boot, the quest "!" and "?" from the
//     Quests addon's data); the target reticle above it (hold: clear target); an arc of four 48 px
//     ability keys with the real icons, cooldowns, range and mana; empty slots as dark sockets.
//     Swipe a key sideways, or tap the dots, for the next four actions. Hold a key for its tooltip,
//     with Move and Remove; anything dragged out of the Spellbook or bags lands on the next key tapped.
//   * Jump pill under Attack: JUMP.
//   * One tray button in the top-right corner slides Bags, Quests, Chat and Menu (a big-icon sheet)
//     out along the top edge and folds them away again, so nothing sits next to the cluster during a fight.
//   * Close (top centre) whenever a window is open; windows over the stick take the touch.
//   * Stances / forms and the pet bar as a short row along the bottom edge (hold a pet action:
//     autocast). Auto loot (setting). Whispers and party chat light the tray and pop up.
//   * Typing: a text bar appears when one of the game's text boxes has focus (mail, auction search,
//     names...); a keyboard button on the character screens relays keys to the glue screens.
//   * Menu > Controls: Classic (stick strafes, drag turns you), Simple (stick turns, drag only
//     looks, the camera follows), Keyboard & mouse (touch off, a corner button to come back), with
//     a phone-vs-PC table; Tap test, Copy debug info, Edit layout, Layout, Left-handed, Vibration,
//     Soft lock and Bigger interface. Interface size and auto loot sit on the Menu sheet.
//   * The rest of the screen: tap = click (target, buttons, windows), double-tap = right-click
//     (talk, loot, open), long-press = hover (tooltips), drag = turn the camera (or drag a window),
//     two-finger pinch = zoom.
//   * In the world the game's own frames are rearranged for a small screen (bigger interface;
//     player and target frames side by side along the top-left edge, the quest arrow top-centre,
//     minimap top-right, tutorial alerts above the bottom edge, action bars and chat hidden:
//     the cluster and the chat sheet replace them); all restored when touch is turned off.
//   * Phones: every game screen asks to be turned sideways in portrait (the glue screens are laid
//     out for landscape), and sideways but not full screen one tap goes full screen and locks
//     landscape (browsers only allow it from a tap; iPhone Safari can't, so it suggests Add to
//     Home Screen, whose manifest opens full screen in landscape). Login, realm, character list
//     and creation all take taps; a tap during a resize or rotation is dropped, not misplaced.
//   * A gamepad connecting hands over to the bridge's gamepad mapper and hides the touch controls; the first time on a
//     device a "Controller connected" card shows its buttons' jobs (the mapper's list, or PAD_MAP).
//
// Added 5 Nov (build 2026-11-05b; docs/TOUCH-CONTROLS.md has every control and a phone checklist):
//   * Tap reliability: a release sent within one game update of its press is sent again after one more settle;
//     the button magnet (a glue tap within 10 px of a scanned button where the game hovers nothing goes to its
//     centre; character creation only up or down, never sideways); the last 10 taps with timings in
//     owTouch.taps; Menu > Controls > Copy debug info (one line to paste to us); the Tap test (five rings, hits,
//     misses, the factor and where the game's cursor went), also on the character screens by a corner button
//     and by ?taptest=1. ?tapframe=<ms> forces the game-update length.
//   * Targeting: the reticle is the nearest-enemy button; swipe the target frame left / right for the next / last
//     enemy; soft lock (a tap on open ground, no unit under it, keeps the target).
//   * Casting: a cast bar over the ring (UnitCastingInfo / UnitChannelInfo); a ground-targeted spell
//     (SpellIsTargeting) waits for the next tap in the world, with an X to cancel.
//   * Layouts: Phone and Tablet presets (Auto by screen size); Edit layout drags the stick, the ring, Jump and the
//     tray and resizes the ring, saved per character (ow:touch-layout:<realm>/<name>), shared as an L1 code;
//     everything clamped on screen. Left-handed mirrors the stick and the ring. Vibration on / off. Bigger
//     interface is a slider (the game's interface size).
//
// Mobile pass, 5 Nov night (build 2026-11-05c):
//   * Fixes: typing a name with a phone keyboard that composes words (Android's): each letter went to the game again
//     with every new letter ("Bob" arrived as "BBoBob"); the keyboard button and the chat box now focus on the lift (a
//     finger's press is no user gesture to Android, so its keyboard never came up); a quick pinch no longer ends in
//     a click from the last finger; going to another app (or a call) lets go of the stick, Jump and a camera drag (the
//     character ran on), and the screen lock comes back after it; a lost lift no longer jams the stick or turns the
//     next taps into a pinch; no page zoom (iOS pinch, double-tap, a name box under 16 px); the chat sheet sits above
//     the phone's keyboard (visualViewport); the notch: the corner buttons and the game's player frame, minimap and
//     tracker are kept clear of it.
//   * New: right-click in windows and the world with a two-finger tap (use or sell a bag item, talk, loot); Touch and
//     hold: Tooltip or Right-click (Menu > Controls); Button size and Opacity sliders; a hardware keyboard hides the
//     touch controls until the next touch (setting); stronger, device-aware vibration (iPhone: the switch haptic);
//     timers paused while the page is hidden; a typed-text echo while the phone keyboard covers the name box; more
//     in Copy debug info (notch, keyboard, haptics, settings).
//
// The art is drawn here as vector graphics (crisp at any screen density), in the look of the
// Open Vanilla mobile mockups: dark stone faces and bronze-gold rims.
//
// 3.3.5a, compared with the 1.12 file:
//   * keyboard focus: GetCurrentKeyBoardFocus() (an engine function in 3.3.5a; 1.12 hooked every
//     EditBox); scripts are called with self (h(f)), not through the `this` global;
//   * the chat box is ChatFrame1EditBox (one per chat frame), not ChatFrameEditBox;
//   * action slots: GetActionBarPage() and GetBonusBarOffset(), and the vehicle and possess bars
//     (offset 5, slots 121-132) whenever UnitHasVehicleUI("player"); a key casts with
//     UseAction(slot) through the bridge, so it never depends on which bar frame is showing
//     (3.3.5a's ActionButtonDown picks the button by VehicleMenuBar/BonusActionBarFrame:IsShown(),
//     and those bars are hidden here); without the bridge it presses the key (bars stay visible);
//   * a Leave button in the bottom row while in a vehicle (CanExitVehicle / VehicleExit) or
//     possessing (PossessButton2);
//   * open windows: GetUIPanel("left" | "center" | "doublewide" | "fullscreen");
//     something on the cursor: GetCursorInfo();
//   * Attack/Interact's quest marks from the bridge snapshot (target.questStatus) instead of the
//     1.12 Quests addon's data; services from the target's tooltip, as before;
//   * the stock frames moved for a small screen are 3.3.5a's (WatchFrame is the quest tracker;
//     the vehicle, possess and totem bars and the chat dock are hidden with the action bars), and
//     the layout is put back after UIParent_ManageFramePositions moves them;
//   * auto loot is the autoLootDefault setting (3.3.5a has it), not a loot-window loop;
//   * the Menu sheet adds Achievements, Dungeon Finder and PvP (3.3.5a binding commands).
// Never load-bearing: any failure leaves the game as it was. window.owTouch for owdiag.

// Shown on the ?touchdebug=1 line, so a phone can tell at a glance which build it is running.
import { gamePointerEvent } from './input.js';
import { makeDriver } from './touch-driver.js';

export const TOUCH_BUILD = 'wrath 2026-11-05c (mobile pass: phone keyboard, two-finger right-click, hold mode, size and opacity, keyboard hides, background let-go, no page zoom, notch; tap re-send, magnet, tap test, debug copy; targeting, cast bar, ground spells; layouts; controller card)';
// the driver installTouch made (gameTicks below asks it for game updates)
let DRIVER = null;

const LS = (k, v) => {
  try {
    if (v === undefined) return localStorage.getItem(k);
    if (v === null) localStorage.removeItem(k); else localStorage.setItem(k, v);
  } catch { /* storage blocked */ }
  return null;
};

// -- When it is on ------------------------------------------------------------------------------

export function wanted() {
  const q = new URLSearchParams(location.search).get('touch');
  if (q === '1' || q === 'on') LS('ow:touch', 'on');
  else if (q === '0' || q === 'off') LS('ow:touch', 'off');
  const pref = LS('ow:touch');
  if (pref === 'on') return true;
  if (pref === 'off') return false;
  return matchMedia('(pointer: coarse)').matches && (navigator.maxTouchPoints || 0) > 0;
}

/** An iPhone or iPad (an iPad asks for desktop pages and calls itself a Mac with touch points). */
export function iOSDevice() {
  return /iPhone|iPad|iPod/.test(navigator.userAgent) || (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1);
}

/** The ghost box's text change as game keys: Backspaces for what went, then what came. A phone keyboard that composes
 *  words (Android's) rewrites the word in the box at each letter, and IME events can't be cancelled, so the box is
 *  diffed instead of read event by event. ZWSP (the box's padding, so Backspace always has something to delete) is
 *  never sent. Returns { back, text }. */
export function ghostDiff(prev, next) {
  let i = 0;
  while (i < prev.length && i < next.length && prev[i] === next[i]) i++;
  return { back: prev.length - i, text: next.slice(i).replace(/​/g, '') };
}

// -- Art (SVG) ----------------------------------------------------------------------------------

const GOLD = `<linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#f6dc93"/><stop offset=".45" stop-color="#c79a3c"/><stop offset=".75" stop-color="#8a6122"/><stop offset="1" stop-color="#e2bd6a"/></linearGradient>`;
const svgUrl = (s) => `url("data:image/svg+xml;charset=utf-8,${encodeURIComponent(s)}")`;

export const ART = {
  stickBase: svgUrl(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 120"><defs>${GOLD}<radialGradient id="f" cx=".5" cy=".42" r=".6"><stop offset="0" stop-color="#2a2733" stop-opacity=".62"/><stop offset="1" stop-color="#07060b" stop-opacity=".78"/></radialGradient></defs><circle cx="60" cy="60" r="57" fill="url(#f)" stroke="url(#g)" stroke-width="3"/><circle cx="60" cy="60" r="52" fill="none" stroke="#000" stroke-opacity=".45" stroke-width="2"/><circle cx="60" cy="60" r="36" fill="none" stroke="#e8c879" stroke-opacity=".22" stroke-width="1.5"/><g fill="#f0d79a" fill-opacity=".8"><path d="M60 9l8 10h-16z"/><path d="M60 111l8-10h-16z"/><path d="M9 60l10-8v16z"/><path d="M111 60l-10-8v16z"/></g><g fill="#f0d79a" fill-opacity=".38"><path d="M24 24l10 2-8 8z"/><path d="M96 24l-2 10-8-8z"/><path d="M24 96l2-10 8 8z"/><path d="M96 96l-10-2 8-8z"/></g></svg>`),
  knob: svgUrl(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 52 52"><defs>${GOLD}<radialGradient id="k" cx=".4" cy=".32" r=".75"><stop offset="0" stop-color="#7d7884"/><stop offset=".55" stop-color="#3a3641"/><stop offset="1" stop-color="#141218"/></radialGradient></defs><circle cx="26" cy="26" r="24" fill="url(#k)" stroke="url(#g)" stroke-width="2.6"/><ellipse cx="21" cy="17" rx="10" ry="5" fill="#fff" fill-opacity=".12"/></svg>`),
  ring: svgUrl(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 48"><defs>${GOLD}</defs><circle cx="24" cy="24" r="21.6" fill="none" stroke="#000" stroke-opacity=".55" stroke-width="2"/><circle cx="24" cy="24" r="22" fill="none" stroke="url(#g)" stroke-width="3.2"/><circle cx="24" cy="24" r="19.6" fill="none" stroke="#000" stroke-opacity=".6" stroke-width="1.4"/></svg>`),
  emptyFace: svgUrl(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 48"><defs><radialGradient id="e" cx=".45" cy=".35" r=".7"><stop offset="0" stop-color="#5d5a62"/><stop offset="1" stop-color="#26242b"/></radialGradient></defs><circle cx="24" cy="24" r="21" fill="url(#e)"/></svg>`),
  attack: svgUrl(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 96 96"><defs>${GOLD}<radialGradient id="a" cx=".5" cy=".42" r=".62"><stop offset="0" stop-color="#4a3a31"/><stop offset=".7" stop-color="#1f1612"/><stop offset="1" stop-color="#0d0908"/></radialGradient><radialGradient id="em" cx=".5" cy="1" r=".6"><stop offset="0" stop-color="#ff7a24" stop-opacity=".28"/><stop offset="1" stop-color="#ff7a24" stop-opacity="0"/></radialGradient><filter id="b" x="-20%" y="-20%" width="140%" height="140%"><feGaussianBlur stdDeviation="2.6"/></filter></defs><circle cx="48" cy="48" r="38" fill="url(#a)"/><circle cx="48" cy="48" r="38" fill="url(#em)"/><circle cx="48" cy="48" r="38" fill="none" stroke="url(#g)" stroke-width="4"/><circle cx="48" cy="48" r="34.5" fill="none" stroke="#000" stroke-opacity=".55" stroke-width="1.5"/><g fill="url(#g)"><path d="M48 3l4 8h-8z"/><path d="M48 93l4-8h-8z"/><path d="M3 48l8-4v8z"/><path d="M93 48l-8-4v8z"/></g></svg>`),
  pill: svgUrl(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 34" preserveAspectRatio="none"><defs>${GOLD}<linearGradient id="p" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#4a4850"/><stop offset=".5" stop-color="#2b2a31"/><stop offset="1" stop-color="#17161b"/></linearGradient></defs><rect x="1.5" y="1.5" width="97" height="31" rx="15.5" fill="url(#p)" stroke="url(#g)" stroke-width="2.6"/><rect x="5" y="4" width="90" height="10" rx="5" fill="#fff" fill-opacity=".07"/></svg>`),
  roundBtn: svgUrl(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 36 36"><defs>${GOLD}<radialGradient id="r" cx=".45" cy=".35" r=".7"><stop offset="0" stop-color="#3f3c46"/><stop offset="1" stop-color="#141218"/></radialGradient></defs><circle cx="18" cy="18" r="16.2" fill="url(#r)" stroke="url(#g)" stroke-width="2.4"/></svg>`),
  sheet: svgUrl(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="#0d0b12"/></svg>`),
};

const ICON = {
  crosshair: `<svg viewBox="0 0 24 24" fill="none" stroke="#f0d79a" stroke-width="2"><circle cx="12" cy="12" r="7"/><circle cx="12" cy="12" r="2.2" fill="#f0d79a"/><path d="M12 1v5M12 18v5M1 12h5M18 12h5"/></svg>`,
  jump: `<svg viewBox="0 0 24 24" fill="none" stroke="#f0d79a" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 18c4-1 7-5 9-11"/><path d="M9 6h6v6"/></svg>`,
  bag: `<svg viewBox="0 0 24 24" fill="#f0d79a"><path d="M8 7a4 4 0 0 1 8 0h2.2l1.3 13.5a1.4 1.4 0 0 1-1.4 1.5H5.9a1.4 1.4 0 0 1-1.4-1.5L5.8 7zm2 0h4a2 2 0 0 0-4 0z"/></svg>`,
  quest: `<svg viewBox="0 0 24 24" fill="#f0d79a"><path d="M10.2 3h3.6l-.7 12h-2.2zM12 17.4a2 2 0 1 1 0 4 2 2 0 0 1 0-4z"/></svg>`,
  chat: `<svg viewBox="0 0 24 24" fill="#f0d79a"><path d="M4 4h16a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2h-8l-5 4v-4H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2z"/></svg>`,
  menu: `<svg viewBox="0 0 24 24" fill="#f0d79a"><rect x="4" y="5" width="16" height="2.6" rx="1.3"/><rect x="4" y="10.7" width="16" height="2.6" rx="1.3"/><rect x="4" y="16.4" width="16" height="2.6" rx="1.3"/></svg>`,
  keyboard: `<svg viewBox="0 0 24 24" fill="none" stroke="#f0d79a" stroke-width="1.8" stroke-linecap="round"><rect x="2.5" y="6" width="19" height="12" rx="2"/><path d="M6 10h.01M9.5 10h.01M13 10h.01M16.5 10h.01M6 14h.01M18 14h.01M9 14h6"/></svg>`,
  phone: `<svg viewBox="0 0 24 24" fill="none" stroke="#f0d79a" stroke-width="1.8" stroke-linecap="round"><rect x="6.5" y="2.5" width="11" height="19" rx="2.2"/><path d="M11 18.5h2"/></svg>`,
  tray: `<svg viewBox="0 0 24 24" fill="#f0d79a"><rect x="4" y="4" width="7" height="7" rx="1.6"/><rect x="13" y="4" width="7" height="7" rx="1.6"/><rect x="4" y="13" width="7" height="7" rx="1.6"/><rect x="13" y="13" width="7" height="7" rx="1.6"/></svg>`,
  close: `<svg viewBox="0 0 24 24" fill="none" stroke="#f0d79a" stroke-width="2.6" stroke-linecap="round"><path d="M6 6l12 12M18 6L6 18"/></svg>`,
  full: `<svg viewBox="0 0 24 24" fill="none" stroke="#f0d79a" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5"/></svg>`,
  gear: `<svg viewBox="0 0 24 24" fill="none" stroke="#f0d79a" stroke-width="2" stroke-linecap="round"><path d="M4 7h10M18 7h2M4 17h2M10 17h10"/><circle cx="16" cy="7" r="2.2"/><circle cx="8" cy="17" r="2.2"/></svg>`,
  rotate: `<svg viewBox="0 0 64 64" fill="none" stroke="#f0d79a" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><rect x="20" y="8" width="24" height="40" rx="4"/><path d="M50 30a20 20 0 0 1-18 22M14 34a20 20 0 0 1 18-22"/><path d="M28 56l4-4-4-4M36 8l-4 4 4 4"/></svg>`,
};

// Attack / Interact's face: what a tap would do with the current target. Gold line art like the
// rim; a quest turn-in is the gold "?", a quest to pick up the gold "!", one still in progress grey.
const ARTGOLD = '#f3d98f';
const glyph = (d, w = 1.9, extra = '') => `<svg viewBox="0 0 24 24" fill="none" stroke="${ARTGOLD}" stroke-width="${w}" stroke-linecap="round" stroke-linejoin="round">${d}${extra}</svg>`;
const mark = (ch, color) => `<svg viewBox="0 0 24 24"><text x="12" y="19.5" text-anchor="middle" font-family="'OW Friz', Georgia, serif" font-size="21" font-weight="700" fill="${color}" stroke="#2a1a05" stroke-width="1.1" paint-order="stroke">${ch}</text></svg>`;
const ROLE_ICON = {
  fight: glyph('<path d="M14.5 17.5L3 6V3h3l11.5 11.5"/><path d="M13 19l6-6M16 16l4 4M19 21l2-2"/><path d="M14.5 6.5L18 3h3v3l-3.5 3.5"/><path d="M5 14l4 4M7 17l-3 3M3 19l2 2"/>'),
  loot: glyph('<path d="M9 3h6l-1.6 3h-2.8z"/><path d="M10.4 6C6 8 4 12 4 15.4 4 19 7 21 12 21s8-2 8-5.6C20 12 18 8 13.6 6"/><path d="M14 11.5h-2.6a1.3 1.3 0 0 0 0 2.6h1.2a1.3 1.3 0 0 1 0 2.6H10M12 10v1.5M12 16.7v1.5"/>'),
  talk: glyph('<path d="M4 5h16a1.5 1.5 0 0 1 1.5 1.5v9A1.5 1.5 0 0 1 20 17h-8l-5 4v-4H4a1.5 1.5 0 0 1-1.5-1.5v-9A1.5 1.5 0 0 1 4 5z"/><path d="M8 11h.01M12 11h.01M16 11h.01" stroke-width="2.6"/>'),
  player: glyph('<path d="M11 17l2 2a1 1 0 1 0 3-3"/><path d="M14 14l2.5 2.5a1 1 0 1 0 3-3l-3.9-3.9a3 3 0 0 0-4.2 0l-.9.9a1 1 0 1 1-3-3l2.8-2.8a5.8 5.8 0 0 1 7 .9L21 7.5"/><path d="M21 3l1 11h-2"/><path d="M3 3L2 14l6.5 6.5a1 1 0 1 0 3-3"/><path d="M3 4h8"/>', 1.6),
  quest: mark('!', '#ffd100'),
  turnin: mark('?', '#ffd100'),
  inprogress: mark('?', '#b9b9b9'),
  repair: glyph('<path d="M4 8h12l4-2v4h-3a3 3 0 0 1-3 3h-1v3h3v3H8v-3h3v-3H9a5 5 0 0 1-5-5z"/>'),
  shop: glyph('<path d="M5 8h14l-1 12.5a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 6 20.5z"/><path d="M9 8V6.5a3 3 0 0 1 6 0V8"/>'),
  train: glyph('<path d="M4 5.5A2.5 2.5 0 0 1 6.5 3H20v15H6.5A2.5 2.5 0 0 0 4 20.5z"/><path d="M4 20.5A2.5 2.5 0 0 0 6.5 23H20v-5"/><path d="M9 8h7M9 11.5h5"/>'),
  flight: glyph('<path d="M21 3C13 3.5 6.5 8 3 17c3-1.6 6-2 8.5-1.3C9.8 17 9 18.6 9 21c5.5-2.6 10-8.4 12-18z"/><path d="M14 9l-6 6"/>'),
  inn: glyph('<path d="M3 20V8M3 15h18v5M21 15v-3a3 3 0 0 0-3-3h-7v6"/><circle cx="7" cy="11.5" r="2"/>'),
  bank: glyph('<path d="M3 9l9-5 9 5"/><path d="M5 10v8M9.5 10v8M14.5 10v8M19 10v8M3 20.5h18"/>'),
  stable: glyph('<circle cx="11" cy="4.5" r="1.8"/><circle cx="18" cy="8" r="1.8"/><circle cx="20" cy="15" r="1.8"/><path d="M8.5 13.5c1.6-2.2 4.4-2.4 6-.3 1.4 1.9 2.9 3 2.9 5a2.6 2.6 0 0 1-3.3 2.6c-1.3-.4-2.4-.4-3.7 0A2.6 2.6 0 0 1 7 18c0-1.6 0-2.5 1.5-4.5z"/><circle cx="4" cy="8.5" r="1.8"/>'),
  battle: glyph('<path d="M5 21V4M5 4h11l-2 3.5 2 3.5H5"/><path d="M19 21V9"/>'),
  guild: glyph('<path d="M6 3h12v14l-6 4-6-4z"/><path d="M9 8h6M12 8v7"/>'),
  spirit: glyph('<path d="M12 4.5a2 2 0 1 0 .01 0"/><path d="M12 8v6M7 21l5-7 5 7M3 9c3 2 6 2 9 0 3 2 6 2 9 0"/>'),
};
// ...shown as the game's own art wherever it has one: the cursors you see when hovering a unit on
// PC (the sword over an enemy, the loot bags, the speech bubble, the anvil hammer, the trainer's
// book, the flight boot) and the gossip window's icons (the quest "!" and "?", bank, inn, tabard,
// battlemaster, spirit healer). The line art above stays as the fallback while one loads.
const ROLE_TEX = {
  fight: 'Interface\\Cursor\\Attack', loot: 'Interface\\Cursor\\LootAll', talk: 'Interface\\Cursor\\Speak',
  player: 'Interface\\Cursor\\Inspect', shop: 'Interface\\Cursor\\Buy', repair: 'Interface\\Cursor\\Repair',
  train: 'Interface\\Cursor\\Trainer', flight: 'Interface\\Cursor\\Taxi', stable: 'Interface\\Cursor\\Interact',
  quest: 'Interface\\GossipFrame\\AvailableQuestIcon', turnin: 'Interface\\GossipFrame\\ActiveQuestIcon',
  inprogress: 'Interface\\GossipFrame\\ActiveQuestIcon', inn: 'Interface\\GossipFrame\\BinderGossipIcon',
  bank: 'Interface\\GossipFrame\\BankerGossipIcon', guild: 'Interface\\GossipFrame\\TabardGossipIcon',
  battle: 'Interface\\GossipFrame\\BattleMasterGossipIcon', spirit: 'Interface\\GossipFrame\\HealerGossipIcon',
};
const ROLE_SAY = {
  loot: 'Double-tap the body to loot it', talk: 'Double-tap them to talk', player: 'Double-tap them to trade, invite or whisper',
  quest: 'Double-tap them for the quest', turnin: 'Double-tap them to turn your quest in', inprogress: 'Your quest for them is not done yet',
  repair: 'Double-tap them to shop and repair', shop: 'Double-tap them to shop', train: 'Double-tap them to train',
  flight: 'Double-tap them to fly', inn: 'Double-tap them to make this inn your home', bank: 'Double-tap them to open your bank',
  stable: 'Double-tap them for your pets', battle: 'Double-tap them to join a battleground', guild: 'Double-tap them for guild business',
  spirit: 'Double-tap them to come back to life here',
};

// The Menu sheet: one big button per window. `cmd` = a binding fired through the bridge; `lua` = a chunk run
// through the bridge (an addon's window), with `chat` (its slash command) when there is no bridge.
const MENU = [
  { label: 'Character', cmd: 'TOGGLECHARACTER0', icon: 'Interface\\Icons\\INV_Chest_Chain_05' },
  { label: 'Spellbook', cmd: 'TOGGLESPELLBOOK', icon: 'Interface\\Icons\\INV_Misc_Book_09' },
  { label: 'Talents', cmd: 'TOGGLETALENTS', icon: 'Interface\\Icons\\Ability_Marksmanship' },
  { label: 'Quest log', cmd: 'TOGGLEQUESTLOG', icon: 'Interface\\Icons\\INV_Misc_Note_01' },
  { label: 'World map', cmd: 'TOGGLEWORLDMAP', icon: 'Interface\\Icons\\INV_Misc_Map_01' },
  { label: 'Social', cmd: 'TOGGLESOCIAL', icon: 'Interface\\Icons\\Spell_Holy_PrayerOfSpirit' },
  { label: 'Bags', cmd: 'OPENALLBAGS', icon: 'Interface\\Icons\\INV_Misc_Bag_08' },
  { label: 'Achievements', cmd: 'TOGGLEACHIEVEMENT', icon: 'Interface\\Icons\\Achievement_Quests_Completed_08' },
  { label: 'Dungeon Finder', cmd: 'TOGGLELFGPARENT', icon: 'Interface\\Icons\\INV_Misc_GroupLooking' },
  { label: 'PvP', cmd: 'TOGGLECHARACTER4', icon: 'Interface\\Icons\\INV_BannerPVP_02' },
  // Boss fights (WrathUnleashedUI BossFights.lua): its window, scaled there so every control is 44 px or more
  { label: 'Boss fights', lua: 'local B = WrathUnleashedUI and WrathUnleashedUI.BossFights if B then B.Show(true) return "ok" end return "no"',
    chat: '/bf', icon: 'Interface\\Icons\\Spell_Frost_ChainsOfIce' },
  { label: 'Game menu', cmd: 'TOGGLEGAMEMENU', icon: 'Interface\\Icons\\INV_Misc_QuestionMark' },
];

// -- Styles -------------------------------------------------------------------------------------

function style() {
  if (document.getElementById('ow-touch-style')) return;
  const s = document.createElement('style');
  s.id = 'ow-touch-style';
  s.textContent = `
    @font-face { font-family: 'OW Friz'; src: url('/data/Fonts%5CFRIZQT__.TTF') format('truetype'); font-display: swap; }
    #ow-touch { position: fixed; inset: 0; z-index: 22; pointer-events: none; display: none;
      font-family: 'OW Cinzel', Georgia, serif; -webkit-user-select: none; user-select: none;
      -webkit-font-smoothing: antialiased; -moz-osx-font-smoothing: grayscale; text-rendering: geometricPrecision; font-kerning: normal;
      -webkit-touch-callout: none; -webkit-tap-highlight-color: transparent; }
    #ow-touch.on { display: block; }
    #ow-touch .zone, #ow-touch .ctl { pointer-events: auto; touch-action: none; }
    #ow-touch .world { position: absolute; inset: 0; }
    #ow-touch.mouse .world { pointer-events: none; }
    #ow-touch .stick-zone { position: absolute; left: 0; bottom: 0; width: 42%; height: 62%; }
    #ow-touch .stick { position: absolute; width: 120px; height: 120px; margin: -60px 0 0 -60px;
      background: ${ART.stickBase} center / contain no-repeat; opacity: calc(.55 * var(--ow-alpha, 1)); transition: opacity .15s;
      pointer-events: none; }
    #ow-touch .stick.live { opacity: calc(.5 + .45 * var(--ow-alpha, 1)); transition: none; }
    #ow-touch.windows .stick:not(.live) { opacity: .28; }
    #ow-touch .knob { position: absolute; left: 50%; top: 50%; width: 52px; height: 52px; margin: -26px 0 0 -26px;
      background: ${ART.knob} center / contain no-repeat; }
    #ow-touch .pill { position: absolute; display: flex; align-items: center; justify-content: center; gap: 6px;
      background: ${ART.pill} center / 100% 100% no-repeat; color: #f0d79a; font-weight: 600; filter: drop-shadow(0 3px 3px rgba(0, 0, 0, .65));
      text-shadow: 0 1px 2px #000; letter-spacing: .04em; }
    #ow-touch .jump { width: 64px; height: 30px; margin: -15px 0 0 -32px; }
    #ow-touch .jump svg { width: 20px; height: 20px; }
    #ow-touch .rbtn { width: 36px; height: 36px; display: flex; align-items: center; justify-content: center;
      background: ${ART.roundBtn} center / contain no-repeat; filter: drop-shadow(0 3px 3px rgba(0, 0, 0, .65)); }
    #ow-touch .rbtn svg { width: 18px; height: 18px; }
    #ow-touch .target { position: absolute; width: 46px; height: 46px; margin: -23px 0 0 -23px; }
    #ow-touch .target svg { width: 24px; height: 24px; }
    #ow-touch .attack { position: absolute; width: 96px; height: 96px; margin: -48px 0 0 -48px;
      background: ${ART.attack} center / contain no-repeat; display: flex; align-items: center; justify-content: center;
      color: #f6e3a8; font-size: 13px; font-weight: 600; line-height: 1.12; letter-spacing: .03em; text-align: center;
      text-shadow: 0 1px 1px rgba(0, 0, 0, .95), 0 0 2px rgba(0, 0, 0, .6); padding-top: 1px; }
    #ow-touch .attack { filter: drop-shadow(0 4px 4px rgba(0, 0, 0, .6)); }
    #ow-touch .attack .ico { width: 42px; height: 42px; display: flex; filter: drop-shadow(0 1.5px 1px rgba(0, 0, 0, .9)); }
    #ow-touch .attack .ico svg, #ow-touch .attack .ico img { width: 100%; height: 100%; }
    #ow-touch .attack .ico.cursor { width: 46px; height: 46px; margin: 2px 0 0 4px; }   /* cursor art sits up-left in its square */
    #ow-touch .attack .ico.gossip { width: 34px; height: 34px; }
    #ow-touch .attack .ico.dim img { filter: grayscale(1) brightness(.8); }
    /* most cursors carry the little pointer arrow in their top-left corner: trimmed off here */
    #ow-touch .attack .ico.cursor.tip img { clip-path: polygon(34% 0, 100% 0, 100% 100%, 0 100%, 0 34%); }
    /* a hostile target: the label turns ember-red instead of a glow */
    #ow-touch .attack.hostile .ico svg { stroke: #ffb48a; }
    #ow-touch .attack.hostile .ico img { filter: drop-shadow(0 0 3px rgba(255, 90, 40, .7)); }
    #ow-touch .key { position: absolute; width: 48px; height: 48px; margin: -24px 0 0 -24px; border-radius: 50%;
      filter: drop-shadow(0 3px 3px rgba(0, 0, 0, .7)) drop-shadow(0 0 1px rgba(0, 0, 0, .9)); }
    #ow-touch .key .face { position: absolute; inset: 3px; border-radius: 50%; background: ${ART.emptyFace} center / cover; }
    /* depth: a soft top highlight and a shaded lower rim over the icon */
    #ow-touch .key .face::after { content: ''; position: absolute; inset: 0; border-radius: 50%; pointer-events: none;
      background: radial-gradient(ellipse 70% 45% at 38% 22%, rgba(255, 255, 255, .28), rgba(255, 255, 255, 0) 70%);
      box-shadow: inset 0 -4px 7px rgba(0, 0, 0, .55), inset 0 2px 2px rgba(255, 255, 255, .12); }
    /* an empty slot: the stock action bar's dark, recessed socket */
    #ow-touch .key.empty .face { background: radial-gradient(circle at 50% 42%, rgba(66, 62, 70, .62), rgba(14, 13, 17, .86) 74%);
      box-shadow: inset 0 4px 8px rgba(0, 0, 0, .95), inset 0 -1px 2px rgba(255, 255, 255, .1); }
    #ow-touch .key.empty .face::after { background: none; box-shadow: inset 0 0 0 1px rgba(120, 112, 100, .35); }
    #ow-touch .key.empty .ring { opacity: .8; }
    #ow-touch .key .face.icon { background-size: 118%; }
    #ow-touch .key .ring { position: absolute; inset: 0; background: ${ART.ring} center / contain no-repeat; }
    #ow-touch .key .cd { position: absolute; inset: 3px; border-radius: 50%; display: none; align-items: center; justify-content: center;
      color: #fff; font: 700 14px system-ui, sans-serif; text-shadow: 0 1px 2px #000; }
    #ow-touch .key.cooling .cd { display: flex; }
    #ow-touch .key.unusable .face { filter: grayscale(1) brightness(.55); }
    #ow-touch .key.nomana .face { filter: brightness(.6) sepia(1) hue-rotate(185deg) saturate(3); }
    #ow-touch .key.far .face { filter: brightness(.75) sepia(1) hue-rotate(-40deg) saturate(4); }
    #ow-touch .key .count { position: absolute; right: 4px; bottom: 2px; font: 700 11px system-ui, sans-serif; color: #fff; text-shadow: 0 1px 2px #000; }
    #ow-touch .pressed { transform: scale(.92); filter: brightness(1.25); }
    #ow-touch .dots { position: absolute; display: flex; gap: 6px; padding: 7px 9px; transform: translate(-50%, -50%); border-radius: 13px; background: rgba(12, 10, 8, .55); box-shadow: inset 0 0 0 1px rgba(240, 215, 154, .35); }
    #ow-touch .dots i { width: 7px; height: 7px; border-radius: 50%; background: rgba(240, 215, 154, .3); }
    #ow-touch .dots i.on { background: #f0d79a; }
    #ow-touch .tray { position: absolute; display: flex; flex-direction: row-reverse; align-items: center; gap: 6px; }
    #ow-touch .tray .item { display: none; }
    #ow-touch .tray.open .item { display: flex; animation: ow-slide .16s ease-out both; }
    @keyframes ow-slide { from { opacity: 0; transform: translateX(10px); } }
    #ow-touch .sheet { position: absolute; left: 50%; transform: translateX(-50%); bottom: calc(max(10px, env(safe-area-inset-bottom)) + var(--ow-kb, 0px));
      width: min(560px, 92vw); max-height: calc(var(--ow-vvh, 100vh) * .82); display: none; flex-direction: column; pointer-events: auto;
      background: rgba(13, 11, 18, .94); border: 1px solid #8a6a2a; box-shadow: inset 0 0 0 1px #3d2c10, 0 10px 40px #000;
      border-radius: 10px; padding: 12px 14px; color: #e8dcc0; font: 14px system-ui, sans-serif; }
    #ow-touch .sheet.open { display: flex; }
    #ow-touch .sheet h3 { margin: 0 0 8px; font: 600 15px 'OW Cinzel', Georgia, serif; color: #f0c058; letter-spacing: .06em; display: flex; justify-content: space-between; align-items: center; }
    #ow-touch .sheet .x { width: 32px; height: 32px; border: 0; background: none; color: #e8dcc0; font-size: 22px; }
    #ow-touch .log { overflow-y: auto; flex: 1 1 auto; min-height: 60px; max-height: 42vh; font-size: 13px; line-height: 1.35; padding-right: 4px; }
    #ow-touch .log div { margin: 1px 0; overflow-wrap: anywhere; }
    #ow-touch .send { display: flex; gap: 8px; margin-top: 8px; }
    #ow-touch .send input { flex: 1; min-width: 0; height: 40px; border-radius: 8px; border: 1px solid #6b5326; background: #07060a; color: #f3ead2; padding: 0 10px; font-size: 16px; }
    #ow-touch .send button, #ow-touch .grid button, #ow-touch .row button { height: 40px; padding: 0 14px; border-radius: 8px; border: 1px solid #9b7a33;
      background: linear-gradient(#3a2f1a, #1d170c); color: #f0d79a; font: 600 13px 'OW Cinzel', Georgia, serif; letter-spacing: .04em; }
    #ow-touch .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(98px, 1fr)); gap: 8px; overflow-y: auto; }
    #ow-touch .grid button { height: 74px; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 5px; padding: 6px; }
    #ow-touch .grid img:not([src]) { visibility: hidden; }
    #ow-touch .grid img { width: 34px; height: 34px; border-radius: 6px; box-shadow: 0 0 0 1px #9b7a33; }
    #ow-touch .row { display: flex; gap: 8px; flex-wrap: wrap; margin-top: 10px; }
    #ow-touch .row button { flex: 1 1 auto; font-size: 12px; }
    #ow-touch .badge { position: absolute; right: -2px; top: -2px; width: 11px; height: 11px; border-radius: 50%;
      background: #e0402c; box-shadow: 0 0 0 1.5px #1a0d08; display: none; }
    #ow-touch .badge.on { display: block; }
    #ow-touch .closer { left: 50%; top: 8px; height: 34px; padding: 0 16px 0 12px; transform: translateX(-50%); font-size: 13px; }
    #ow-touch .closer svg { width: 14px; height: 14px; }
    #ow-touch .closer:not(.on) { display: none; }
    #ow-touch .strip { position: absolute; display: flex; gap: 8px; transform: translateX(-50%); }
    #ow-touch .strip:empty { display: none; }
    #ow-touch .sbtn { position: relative; width: 40px; height: 40px; border-radius: 50%;
      filter: drop-shadow(0 3px 3px rgba(0, 0, 0, .7)); }
    #ow-touch .sbtn .face { position: absolute; inset: 3px; border-radius: 50%; background: ${ART.emptyFace} center / 118%; }
    #ow-touch .sbtn .ring { position: absolute; inset: 0; background: ${ART.ring} center / contain no-repeat; }
    #ow-touch .sbtn.active .ring { filter: brightness(1.45) drop-shadow(0 0 4px rgba(255, 220, 120, .9)); }
    #ow-touch .sbtn.auto::after { content: ''; position: absolute; inset: -3px; border-radius: 50%; border: 1.5px dashed rgba(255, 220, 120, .85); }
    #ow-touch .sbtn.off .face { filter: grayscale(1) brightness(.55); }
    #ow-touch .gap { width: 10px; }
    #ow-touch .typebar { position: absolute; left: 50%; top: 50px; transform: translateX(-50%); width: min(520px, 86vw); display: none;
      gap: 6px; padding: 6px; border-radius: 10px; background: rgba(13, 11, 18, .94); border: 1px solid #8a6a2a; pointer-events: auto;
      box-shadow: 0 6px 24px #000; }
    #ow-touch .typebar.on { display: flex; }
    #ow-touch .typebar input { flex: 1; min-width: 0; height: 40px; border-radius: 8px; border: 1px solid #6b5326; background: #07060a; color: #f3ead2; padding: 0 10px; font-size: 16px; }
    #ow-touch .typebar button { height: 40px; padding: 0 12px; border-radius: 8px; border: 1px solid #9b7a33; background: linear-gradient(#3a2f1a, #1d170c); color: #f0d79a; font: 600 13px 'OW Cinzel', Georgia, serif; }
    #ow-touch .info { position: absolute; left: 50%; top: 52%; transform: translate(-50%, -50%); width: min(340px, 60vw); display: none; flex-direction: column; gap: 8px;
      padding: 10px 12px; border-radius: 10px; background: rgba(13, 11, 18, .95); border: 1px solid #8a6a2a; box-shadow: 0 8px 28px #000;
      color: #e8dcc0; font: 13px/1.35 system-ui, sans-serif; pointer-events: auto; }
    #ow-touch .info.on { display: flex; }
    #ow-touch .info.fresh { pointer-events: none; }
    #ow-touch.sheet-open .strip, #ow-touch.sheet-open .closer, #ow-touch.sheet-open .typebar, #ow-touch.sheet-open .info { display: none !important; }
    #ow-touch .info .t { white-space: pre-wrap; max-height: 40vh; overflow-y: auto; }
    #ow-touch .info .t b { color: #f0c058; font: 600 14px 'OW Cinzel', Georgia, serif; }
    #ow-touch .info .b { display: flex; gap: 6px; flex-wrap: wrap; }
    #ow-touch .info button, #ow-touch .settings button { flex: 1 1 auto; height: 36px; padding: 0 10px; border-radius: 8px; border: 1px solid #9b7a33;
      background: linear-gradient(#3a2f1a, #1d170c); color: #f0d79a; font: 600 12px 'OW Cinzel', Georgia, serif; }
    #ow-touch .settings { display: flex; gap: 8px; flex-wrap: wrap; margin-top: 8px; }
    #ow-touch .kbd { position: absolute; right: calc(env(safe-area-inset-right, 0px) + 12px); top: calc(env(safe-area-inset-top, 0px) + 12px); display: none; pointer-events: auto; }
    #ow-touch.glue-type .kbd { display: flex; }
    /* on screen (iOS scrolls the page to an input focused off it) and 16 px (iOS zooms the page into a smaller one) */
    #ow-touch .ghost { position: absolute; left: 0; top: 0; width: 1px; height: 1px; opacity: 0; font-size: 16px; border: 0; padding: 0; pointer-events: none; }
    /* what the phone keyboard typed, while it covers the name box */
    #ow-touch .kecho { position: absolute; left: 50%; top: calc(env(safe-area-inset-top, 0px) + 10px); transform: translateX(-50%); display: none;
      max-width: 60vw; padding: 6px 14px; border-radius: 3px; background: rgba(8, 8, 10, .88); border: 1px solid rgba(150, 132, 96, .6);
      color: #fff; font: 15px 'OW Friz', Georgia, serif; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; pointer-events: none; z-index: 4; }
    #ow-touch .kecho:empty::before { content: 'Type with your keyboard'; color: #c9c1b2; }
    #ow-touch.ghosting .kecho { display: block; }
    /* the switch whose toggle gives an iPhone's haptic tick (no navigator.vibrate there) */
    #ow-touch .hap { position: fixed; left: 0; top: 0; width: 1px; height: 1px; overflow: hidden; opacity: 0; pointer-events: none; }
    #ow-touch .opts { display: flex; flex-direction: column; gap: 8px; overflow-y: auto; }
    #ow-touch .opt { text-align: left; padding: 9px 12px; border-radius: 8px; border: 1px solid #5d4720; background: rgba(40, 32, 18, .55);
      color: #e8dcc0; font: 13px/1.35 system-ui, sans-serif; }
    #ow-touch .opt b { display: block; color: #f0d79a; font: 600 14px 'OW Cinzel', Georgia, serif; letter-spacing: .03em; margin-bottom: 2px; }
    #ow-touch .opt.sel { border-color: #f0c058; box-shadow: 0 0 0 1px #f0c058 inset, 0 0 12px rgba(240, 192, 88, .25); background: rgba(70, 52, 20, .6); }
    #ow-touch .opt.sel b::after { content: '  ✓'; color: #f0c058; }
    #ow-touch table.vs { width: 100%; border-collapse: collapse; margin-top: 10px; font: 12px/1.3 system-ui, sans-serif; }
    #ow-touch table.vs th { text-align: left; color: #f0c058; font: 600 12px 'OW Cinzel', Georgia, serif; padding: 4px 6px; border-bottom: 1px solid #5d4720; }
    #ow-touch table.vs td { padding: 4px 6px; border-bottom: 1px solid rgba(93, 71, 32, .45); vertical-align: top; }
    #ow-touch table.vs td:first-child { color: #c9b98f; white-space: nowrap; }
    #ow-touch .restore { position: absolute; right: calc(env(safe-area-inset-right, 0px) + 8px); top: 46%; display: none; pointer-events: auto; opacity: .75; }
    /* no bridge: the screen can't be told apart from the world in an older client, so a button
       switches between the world's controls and the glue screens' taps */
    #ow-touch .flip { position: absolute; right: calc(env(safe-area-inset-right, 0px) + 56px); top: calc(env(safe-area-inset-top, 0px) + 10px); display: none; pointer-events: auto; opacity: .7; }
    #ow-touch.on.keys:not(.loading) .flip { display: flex; }
    /* (5 Nov night) it sat on the open tray's Bags button, and on the character screens under the Controls button */
    #ow-touch.on.keys:not(.loading) .tray.open ~ .flip { display: none; }
    #ow-touch.glue-touch:not(.inworld) .flip { right: calc(env(safe-area-inset-right, 0px) + 12px); top: calc(env(safe-area-inset-top, 0px) + 56px); }
    #ow-touch.pcmode { display: block; }
    #ow-touch.pcmode .restore { display: flex; }
    #ow-touch.pcmode > :not(.restore):not(.toast):not(.sheet) { display: none !important; }
    #ow-touch .toast { position: absolute; left: 50%; top: 28%; transform: translateX(-50%); padding: 8px 14px; border-radius: 8px;
      background: rgba(10, 8, 16, .88); border: 1px solid #8a6a2a; color: #f0d79a; font: 600 13px 'OW Cinzel', Georgia, serif;
      opacity: 0; transition: opacity .3s; pointer-events: none; text-align: center; max-width: 80vw; }
    #ow-touch .toast.on { opacity: 1; }
    #ow-touch .toast, #ow-touch .info { z-index: 8; }
    #ow-touch .rotate { position: absolute; inset: 0; display: none; flex-direction: column; align-items: center; justify-content: center; gap: 14px;
      background: rgba(5, 4, 10, .92); color: #f0d79a; font: 600 16px 'OW Cinzel', Georgia, serif; letter-spacing: .06em; pointer-events: auto; }
    #ow-touch .rotate svg { width: 64px; height: 64px; }
    /* a phone held upright: the game's screens are laid out for landscape (character creation's
       panels pile up over its buttons), so every game screen asks to be turned sideways */
    @media (orientation: portrait) { #ow-touch.on.ingame.phone .rotate { display: flex; } }
    #ow-touch .rotate .go, #ow-touch .fsgo .go { margin-top: 4px; padding: 11px 22px; border-radius: 22px; border: 1px solid #c9a24a;
      background: linear-gradient(#4a3a1c, #21190b); color: #ffd98a; font: 600 15px 'OW Cinzel', Georgia, serif; letter-spacing: .05em;
      box-shadow: 0 4px 14px rgba(0, 0, 0, .6); }
    #ow-touch:not(.canfs) .rotate .go { display: none; }
    #ow-touch .rotate .sub, #ow-touch .fsgo .sub { font: 13px system-ui, sans-serif; color: #c9b98f; letter-spacing: 0; max-width: 80vw; text-align: center; }
    /* sideways but not full screen yet: one tap goes full screen (the browser only allows it from a tap) */
    #ow-touch .fsgo { position: absolute; inset: 0; display: none; flex-direction: column; align-items: center; justify-content: center; gap: 10px;
      background: rgba(5, 4, 10, .72); color: #f0d79a; font: 600 17px 'OW Cinzel', Georgia, serif; letter-spacing: .05em; pointer-events: auto; }
    #ow-touch .fsgo svg { width: 46px; height: 46px; }
    #ow-touch .fsgo .inst, #ow-touch .rotate .inst { display: none; margin-top: 2px; padding: 8px 16px; border-radius: 18px; border: 1px solid #6b5a2e;
      background: rgba(20,16,10,.7); color: #e8d9a8; font: 13px system-ui, sans-serif; }
    #ow-touch.caninstall .fsgo .inst, #ow-touch.caninstall .rotate .inst { display: inline-block; }
    #ow-touch .fsgo .skip { margin-top: 2px; padding: 8px 14px; border: 0; background: none; color: #a99a78; font: 13px system-ui, sans-serif; text-decoration: underline; }
    @media (orientation: landscape) { #ow-touch.on.ingame.phone.canfs.wantfs:not(.pcmode) .fsgo { display: flex; } }
    #ow-touch:not(.inworld) .play, #ow-touch.loading .play { display: none; }
    /* character / realm screens: taps become corrected mouse clicks (the game ignores raw touches there) */
    #ow-touch .glue { position: absolute; inset: 0; display: none; }
    /* the aiming loupe: what is under the finger, magnified, above it */
    #ow-touch .loupe { position: absolute; width: 132px; height: 132px; border-radius: 50%; display: none; pointer-events: none; overflow: hidden;
      border: 3px solid #d9b25a; box-shadow: 0 0 0 1px #000, 0 8px 22px rgba(0, 0, 0, .7); background: #000; z-index: 2; }
    #ow-touch .loupe.on { display: block; }
    #ow-touch .loupe canvas { width: 100%; height: 100%; display: block; }
    /* where each click went: a small gold ring that fades */
    #ow-touch .tapring { position: absolute; width: 26px; height: 26px; margin: -13px 0 0 -13px; border-radius: 50%; pointer-events: none;
      border: 2px solid #ffd76a; box-shadow: 0 0 8px rgba(255, 200, 80, .8); opacity: 0; z-index: 7; }
    #ow-touch .tapring.go { animation: ow-ring .45s ease-out; }
    #ow-touch .tapring.snap { border-color: #7dff9a; box-shadow: 0 0 8px rgba(120, 255, 150, .8); }
    @keyframes ow-ring { from { opacity: 1; transform: scale(.5); } to { opacity: 0; transform: scale(1.5); } }
    #ow-touch .tdebug { position: absolute; left: calc(env(safe-area-inset-left, 0px) + 6px); bottom: calc(env(safe-area-inset-bottom, 0px) + 6px); display: none; padding: 4px 7px; border-radius: 4px; pointer-events: none;
      background: rgba(0, 0, 0, .75); color: #9f9; font: 11px/1.3 ui-monospace, monospace; white-space: pre; z-index: 3; }
    #ow-touch.debug .tdebug { display: block; }
    #ow-touch.glue-touch:not(.inworld):not(.loading) .glue { display: block; }
    /* panels in the look of the Classic+ Quests tracker: near-black glass, a hairline bronze
       border, a slim header strip, gold titles in the game's own Friz Quadrata */
    #ow-touch .sheet, #ow-touch .info, #ow-touch .typebar, #ow-touch .toast {
      background: rgba(8, 8, 10, .8); border: 1px solid rgba(150, 132, 96, .5); border-radius: 3px;
      box-shadow: 0 6px 22px rgba(0, 0, 0, .6); color: #fff; font-family: 'OW Friz', Georgia, serif; }
    #ow-touch .sheet { padding: 0 0 10px; }
    #ow-touch .sheet > :not(h3) { margin-left: 10px; margin-right: 10px; }
    #ow-touch .sheet h3 { margin: 0 0 8px; padding: 5px 6px 5px 10px; font: 14px 'OW Friz', Georgia, serif; letter-spacing: 0;
      color: #ffd100; text-shadow: 0 1px 1px #000; background: linear-gradient(rgba(255, 255, 255, .09), rgba(255, 255, 255, .03));
      border-bottom: 1px solid rgba(150, 132, 96, .4); }
    #ow-touch .sheet .x { width: 28px; height: 24px; font-size: 18px; color: #c9c1b2; }
    #ow-touch .send button, #ow-touch .grid button, #ow-touch .row button, #ow-touch .info button,
    #ow-touch .settings button, #ow-touch .typebar button { border-radius: 3px; border: 1px solid rgba(150, 132, 96, .5);
      background: rgba(255, 255, 255, .05); color: #ffd100; font: 13px 'OW Friz', Georgia, serif; letter-spacing: 0; text-shadow: 0 1px 1px #000; }
    #ow-touch .grid button span { color: #fff; }
    #ow-touch .grid img { border-radius: 2px; box-shadow: 0 0 0 1px rgba(150, 132, 96, .7); }
    #ow-touch .send input, #ow-touch .typebar input { border-radius: 3px; border: 1px solid rgba(150, 132, 96, .5);
      background: rgba(0, 0, 0, .6); color: #fff; font: 16px 'OW Friz', Georgia, serif; }
    #ow-touch .log { font: 13px/1.35 'OW Friz', Georgia, serif; }
    #ow-touch .toast { font: 13px 'OW Friz', Georgia, serif; color: #ffd100; text-shadow: 0 1px 1px #000; padding: 7px 12px; }
    #ow-touch .info { font: 13px/1.35 'OW Friz', Georgia, serif; }
    #ow-touch .info .t b { font: 15px 'OW Friz', Georgia, serif; color: #ffd100; }
    #ow-touch .opt { border-radius: 3px; border: 1px solid rgba(150, 132, 96, .45); background: rgba(255, 255, 255, .04);
      color: #e9e4da; font: 13px/1.35 'OW Friz', Georgia, serif; }
    #ow-touch .opt b { font: 14px 'OW Friz', Georgia, serif; color: #ffd100; letter-spacing: 0; }
    #ow-touch .opt.sel { border-color: #ffd100; box-shadow: inset 0 0 0 1px rgba(255, 209, 0, .6); background: rgba(255, 209, 0, .07); }
    #ow-touch .opt.sel b::after { color: #ffd100; }
    #ow-touch table.vs { font: 12px/1.3 'OW Friz', Georgia, serif; }
    #ow-touch table.vs th { font: 12px 'OW Friz', Georgia, serif; color: #ffd100; border-bottom-color: rgba(150, 132, 96, .5); }
    #ow-touch table.vs td { border-bottom-color: rgba(150, 132, 96, .22); }
    #ow-touch table.vs td:first-child { color: #ffd100; }
    #ow-touch .pill { font-family: 'OW Friz', Georgia, serif; font-weight: 400; letter-spacing: 0; }
    /* Menu > Controls: tools and comfort settings */
    #ow-touch .ctlrow { display: flex; gap: 8px; flex-wrap: wrap; }
    #ow-touch .ctlrow button { flex: 1 1 30%; min-height: 44px; padding: 0 10px; border-radius: 3px; border: 1px solid rgba(150, 132, 96, .5);
      background: rgba(255, 255, 255, .05); color: #ffd100; font: 13px 'OW Friz', Georgia, serif; text-shadow: 0 1px 1px #000; }
    #ow-touch .slider { display: flex; align-items: center; gap: 10px; min-height: 44px; color: #ffd100; font: 13px 'OW Friz', Georgia, serif; }
    #ow-touch .slider input { flex: 1; min-width: 0; height: 32px; accent-color: #d9b25a; }
    #ow-touch .slider .v { width: 44px; text-align: right; color: #fff; }
    #ow-touch textarea.dbg { width: 100%; height: 90px; margin-top: 6px; background: rgba(0, 0, 0, .6); color: #cfc; border: 1px solid rgba(150, 132, 96, .5);
      font: 16px/1.25 ui-monospace, monospace; -webkit-user-select: text; user-select: text; }
    /* the character screens: a corner button for Menu > Controls (the tray only exists in the world) */
    #ow-touch .gear { position: absolute; right: calc(env(safe-area-inset-right, 0px) + 56px); top: calc(env(safe-area-inset-top, 0px) + 12px); display: none; pointer-events: auto; opacity: .8; }
    #ow-touch.glue-touch:not(.inworld):not(.loading) .gear { display: flex; }
    /* the cast bar, over the ring */
    #ow-touch .castbar { position: absolute; width: 168px; height: 16px; margin: -8px 0 0 -84px; display: none; border-radius: 3px; overflow: hidden;
      background: rgba(0, 0, 0, .72); border: 1px solid rgba(150, 132, 96, .7); pointer-events: none; }
    #ow-touch .castbar.on { display: block; }
    #ow-touch .castbar i { position: absolute; left: 0; top: 0; bottom: 0; width: 0; background: linear-gradient(#ffd25a, #c08a1c); }
    #ow-touch .castbar.chan i { background: linear-gradient(#7fd0ff, #2a7ab8); }
    #ow-touch .castbar span { position: absolute; inset: 0; color: #fff; font: 11px/16px 'OW Friz', Georgia, serif; text-align: center; text-shadow: 0 1px 1px #000; white-space: nowrap; overflow: hidden; }
    /* a ground-targeted spell waiting: what to do, and an X to cancel it */
    #ow-touch .gaim { position: absolute; left: 50%; top: calc(env(safe-area-inset-top, 0px) + 54px); transform: translateX(-50%); display: none; align-items: center; gap: 10px;
      padding: 4px 4px 4px 14px; border-radius: 22px; background: rgba(8, 8, 10, .85); border: 1px solid #7dff9a; color: #dfffe6; font: 14px 'OW Friz', Georgia, serif; pointer-events: none; }
    #ow-touch.aimground.inworld .gaim { display: flex; }
    #ow-touch .gaim .gx { width: 44px; height: 44px; border-radius: 50%; border: 1px solid rgba(150, 132, 96, .6); background: rgba(255, 255, 255, .06); pointer-events: auto; display: flex; align-items: center; justify-content: center; }
    #ow-touch .gaim .gx svg { width: 18px; height: 18px; }
    /* the tap test: five targets, one at a time, over everything */
    #ow-touch .taptest { position: absolute; inset: 0; display: none; background: rgba(5, 4, 10, .9); pointer-events: auto; touch-action: none; z-index: 6; }
    #ow-touch .taptest.on { display: block; }
    #ow-touch .tt-dot { position: absolute; width: 44px; height: 44px; margin: -22px 0 0 -22px; border-radius: 50%; border: 3px solid #ffd76a;
      box-shadow: 0 0 0 6px rgba(255, 215, 106, .18), 0 0 14px rgba(255, 200, 80, .7); }
    #ow-touch .tt-dot::after { content: ''; position: absolute; left: 50%; top: 50%; width: 6px; height: 6px; margin: -3px 0 0 -3px; border-radius: 50%; background: #ffd76a; }
    #ow-touch .tt-dot.done { display: none; }
    #ow-touch .tt-msg { position: absolute; left: 50%; top: 50%; transform: translate(-50%, -50%); width: min(420px, 80vw); text-align: center; color: #f0d79a;
      font: 14px/1.4 'OW Friz', Georgia, serif; white-space: pre-line; pointer-events: none; }
    #ow-touch .tt-b { position: absolute; left: 50%; bottom: calc(env(safe-area-inset-bottom, 0px) + 12px); transform: translateX(-50%); display: none; gap: 8px; }
    #ow-touch .taptest.end .tt-b { display: flex; }
    #ow-touch .tt-b button, #ow-touch .editbar button { min-width: 44px; height: 44px; padding: 0 14px; border-radius: 3px; border: 1px solid rgba(150, 132, 96, .6);
      background: rgba(30, 24, 14, .9); color: #ffd100; font: 13px 'OW Friz', Georgia, serif; }
    /* Edit layout: the controls take drags instead of presses; a bar with the rest */
    #ow-touch .editbar { position: absolute; left: 50%; top: calc(env(safe-area-inset-top, 0px) + 6px); transform: translateX(-50%); width: min(620px, 94vw); display: none;
      flex-direction: column; gap: 6px; padding: 6px 8px; border-radius: 3px; background: rgba(8, 8, 10, .88); border: 1px solid rgba(150, 132, 96, .6);
      color: #fff; font: 13px 'OW Friz', Georgia, serif; pointer-events: auto; z-index: 5; }
    #ow-touch.editing .editbar { display: flex; }
    #ow-touch .editbar .eb-b, #ow-touch .editbar .eb-code { display: flex; gap: 6px; flex-wrap: wrap; }
    #ow-touch .editbar button { flex: 1 1 auto; height: 40px; }
    #ow-touch .editbar input { flex: 2 1 120px; min-width: 0; height: 40px; border-radius: 3px; border: 1px solid rgba(150, 132, 96, .5); background: rgba(0, 0, 0, .6); color: #fff; padding: 0 8px; font: 16px ui-monospace, monospace; }
    #ow-touch.editing .stick { pointer-events: auto; opacity: .9; }
    #ow-touch.editing .stick, #ow-touch.editing .attack, #ow-touch.editing .jump, #ow-touch.editing .tray .handle { outline: 2px dashed rgba(125, 255, 154, .8); outline-offset: 3px; border-radius: 50%; }
    #ow-touch.editing .jump { border-radius: 16px; }
    /* a controller took over: only its card and messages show */
    #ow-touch.padmode { display: block; }
    #ow-touch.padmode > :not(.info):not(.toast) { display: none !important; }
    /* the page menu handle moves to the bottom edge, out of the target frame's way */
    body.ow-touch-world #ow-dock { top: auto; bottom: 0; flex-direction: column-reverse; }
    body.ow-touch-world #ow-dock-handle { border-radius: 8px 8px 0 0; border-top: 1px solid rgba(201,162,39,.35); border-bottom: 0; }
    /* Menu > Controls > Opacity: the controls over the world (a press shows the key at full strength) */
    #ow-touch .attack, #ow-touch .key, #ow-touch .target, #ow-touch .dots, #ow-touch .jump, #ow-touch .tray, #ow-touch .strip { opacity: var(--ow-alpha, 1); }
    #ow-touch .play.pressed, #ow-touch.editing .play { opacity: 1; }
    /* a hardware keyboard is playing: the thumb controls step aside until the next touch (the tray stays) */
    #ow-touch.kbdmode .stick, #ow-touch.kbdmode .attack, #ow-touch.kbdmode .key, #ow-touch.kbdmode .target,
    #ow-touch.kbdmode .dots, #ow-touch.kbdmode .jump, #ow-touch.kbdmode .strip { visibility: hidden; }
    /* no page zoom or scrolling under a finger: panels scroll, nothing pinches or double-taps the page bigger,
       no pull-to-refresh or rubber band */
    #ow-touch .sheet, #ow-touch .info, #ow-touch .typebar, #ow-touch .editbar { touch-action: pan-x pan-y; }
    #ow-touch .log, #ow-touch .grid, #ow-touch .opts, #ow-touch .info .t { overscroll-behavior: contain; }
    html.ow-touch-on, body.ow-touch-on { overscroll-behavior: none; }
    body.ow-touch-on canvas { touch-action: none; }
  `;
  document.head.append(s);
}

// -- BLP icons (the game's own art for the action keys and the menu) -----------------------------

const iconCache = new Map();

function dxtColors(c0, c1, out, opaque) {
  const r = (c) => [((c >> 11) & 31) * 255 / 31, ((c >> 5) & 63) * 255 / 63, (c & 31) * 255 / 31];
  const a = r(c0), b = r(c1);
  out[0] = [...a, 255]; out[1] = [...b, 255];
  if (c0 > c1 || opaque) {
    out[2] = [(2 * a[0] + b[0]) / 3, (2 * a[1] + b[1]) / 3, (2 * a[2] + b[2]) / 3, 255];
    out[3] = [(a[0] + 2 * b[0]) / 3, (a[1] + 2 * b[1]) / 3, (a[2] + 2 * b[2]) / 3, 255];
  } else {
    out[2] = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2, (a[2] + b[2]) / 2, 255];
    out[3] = [0, 0, 0, 0];
  }
}

/** Decode a BLP2 (palette, DXT1/3/5 or raw BGRA) to RGBA. */
export function decodeBlp(buf) {
  const v = new DataView(buf), u8 = new Uint8Array(buf);
  if (String.fromCharCode(u8[0], u8[1], u8[2], u8[3]) !== 'BLP2') throw new Error('not BLP2');
  const comp = u8[8], alphaDepth = u8[9], alphaType = u8[10];
  const w = v.getUint32(12, true), h = v.getUint32(16, true);
  const off = v.getUint32(20, true);
  const px = new Uint8ClampedArray(w * h * 4);
  if (comp === 1) {
    const pal = 148;
    for (let i = 0; i < w * h; i++) {
      const p = pal + u8[off + i] * 4;
      px[i * 4] = u8[p + 2]; px[i * 4 + 1] = u8[p + 1]; px[i * 4 + 2] = u8[p]; px[i * 4 + 3] = 255;
    }
    const ao = off + w * h;
    if (alphaDepth === 8) for (let i = 0; i < w * h; i++) px[i * 4 + 3] = u8[ao + i];
    else if (alphaDepth === 1) for (let i = 0; i < w * h; i++) px[i * 4 + 3] = (u8[ao + (i >> 3)] >> (i & 7)) & 1 ? 255 : 0;
    else if (alphaDepth === 4) for (let i = 0; i < w * h; i++) px[i * 4 + 3] = ((u8[ao + (i >> 1)] >> ((i & 1) * 4)) & 15) * 17;
  } else if (comp === 2) {
    const kind = alphaDepth <= 1 ? 1 : (alphaType === 7 ? 5 : 3);
    const bs = kind === 1 ? 8 : 16;
    let p = off;
    const cols = [[], [], [], []];
    for (let by = 0; by < h; by += 4) {
      for (let bx = 0; bx < w; bx += 4) {
        const alpha = new Array(16).fill(255);
        if (kind === 3) {
          for (let i = 0; i < 16; i++) alpha[i] = ((u8[p + (i >> 1)] >> ((i & 1) * 4)) & 15) * 17;
        } else if (kind === 5) {
          const a0 = u8[p], a1 = u8[p + 1];
          const t = [a0, a1];
          if (a0 > a1) for (let i = 1; i < 7; i++) t.push(((7 - i) * a0 + i * a1) / 7);
          else { for (let i = 1; i < 5; i++) t.push(((5 - i) * a0 + i * a1) / 5); t.push(0, 255); }
          let bits = 0n;
          for (let i = 0; i < 6; i++) bits |= BigInt(u8[p + 2 + i]) << BigInt(8 * i);
          for (let i = 0; i < 16; i++) alpha[i] = t[Number((bits >> BigInt(3 * i)) & 7n)];
        }
        const cp = kind === 1 ? p : p + 8;
        const c0 = v.getUint16(cp, true), c1 = v.getUint16(cp + 2, true);
        dxtColors(c0, c1, cols, kind !== 1);
        const idx = v.getUint32(cp + 4, true);
        for (let i = 0; i < 16; i++) {
          const x = bx + (i & 3), y = by + (i >> 2);
          if (x >= w || y >= h) continue;
          const c = cols[(idx >> (2 * i)) & 3];
          const o = (y * w + x) * 4;
          px[o] = c[0]; px[o + 1] = c[1]; px[o + 2] = c[2];
          px[o + 3] = kind === 1 ? c[3] : alpha[i];
        }
        p += bs;
      }
    }
  } else if (comp === 3) {
    for (let i = 0; i < w * h; i++) {
      px[i * 4] = u8[off + i * 4 + 2]; px[i * 4 + 1] = u8[off + i * 4 + 1]; px[i * 4 + 2] = u8[off + i * 4]; px[i * 4 + 3] = u8[off + i * 4 + 3];
    }
  } else throw new Error('BLP compression ' + comp);
  return { w, h, px };
}

/** A game icon ("Interface\\Icons\\X") as a data URL, from the game's own data. */
export function iconUrl(path) {
  if (!path) return Promise.resolve(null);
  const key = path.toLowerCase();
  if (iconCache.has(key)) return iconCache.get(key);
  const p = (async () => {
    const r = await fetch('/data/' + encodeURIComponent(path.replace(/\//g, '\\') + (/\.blp$/i.test(path) ? '' : '.blp')), { credentials: 'same-origin' });
    if (!r.ok) return null;
    const { w, h, px } = decodeBlp(await r.arrayBuffer());
    const c = document.createElement('canvas');
    c.width = w; c.height = h;
    c.getContext('2d').putImageData(new ImageData(px, w, h), 0, 0);
    return c.toDataURL('image/png');
  })().catch(() => null);
  iconCache.set(key, p);
  return p;
}

// The box that has the keyboard: 3.3.5a's engine answers it (GetCurrentKeyBoardFocus).
export const FOCUS = '(GetCurrentKeyBoardFocus and GetCurrentKeyBoardFocus())';

// The action slot behind touch key n (1-12), as 3.3.5a's ActionButton_CalculateAction pages it:
// the bonus bar (stances, stealth, forms) on page 1, the vehicle / possess bar (offset 5: slots
// 121-132) on any page while UnitHasVehicleUI("player"), else the page.
export const SLOT_FN = `local o = GetBonusBarOffset and GetBonusBarOffset() or 0
local pg = (GetActionBarPage and GetActionBarPage()) or CURRENT_ACTIONBAR_PAGE or 1
local np = NUM_ACTIONBAR_PAGES or 6
local veh = UnitHasVehicleUI and UnitHasVehicleUI("player")
local function slot(i) if o > 0 and (pg == 1 or veh) then return i + (np + o - 1) * 12 end return i + (pg - 1) * 12 end`;

/** The slot SLOT_FN's slot(i) gives, in JS (for tests and owdiag). */
export function slotFor(i, { offset = 0, page = 1, pages = 6, vehicle = false } = {}) {
  if (offset > 0 && (page === 1 || vehicle)) return i + (pages + offset - 1) * 12;
  return i + (page - 1) * 12;
}

/** Edit layout's places as a short code to share or move to another device: "L1" + 2 base-36 digits per
 *  fraction (stick x y, ring x y, Jump x y or "zzzz" when it follows the ring, tray x y) + the ring size. */
export function encodeLayout(c) {
  const p = (v) => Math.round(Math.min(1, Math.max(0, v)) * 999).toString(36).padStart(2, '0');
  const xs = [...c.s, ...c.r].map(p).join('') + (c.j ? c.j.map(p).join('') : 'zzzz') + c.t.map(p).join('');
  return 'L1' + xs + Math.round((Math.min(1.5, Math.max(0.8, c.z)) - 0.5) * 100).toString(36).padStart(2, '0');
}
export function decodeLayout(code) {
  const m = /^L1([0-9a-z]{18})$/i.exec(String(code || '').trim().replace(/\s+/g, ''));
  if (!m) return null;
  const d = m[1].toLowerCase().match(/../g);
  const f = (i) => parseInt(d[i], 36) / 999;
  if ([0, 1, 2, 3, 6, 7].some((i) => parseInt(d[i], 36) > 999)) return null;
  const jz = d[4] === 'zz' && d[5] === 'zz';
  if (!jz && (parseInt(d[4], 36) > 999 || parseInt(d[5], 36) > 999)) return null;
  const z = parseInt(d[8], 36) / 100 + 0.5;
  if (!(z >= 0.8 && z <= 1.5)) return null;
  const r4 = (v) => Math.round(v * 1000) / 1000;
  return { s: [r4(f(0)), r4(f(1))], r: [r4(f(2)), r4(f(3))], j: jz ? null : [r4(f(4)), r4(f(5))], t: [r4(f(6)), r4(f(7))], z: Math.round(z * 100) / 100 };
}

// -- Synthetic mouse on the game ----------------------------------------------------------------

function gameCanvas() { return document.getElementById('benilla') || document.querySelector('canvas'); }

// Where a pointer lands in the client: it multiplies the page position by devicePixelRatio and
// reads that as a pixel of its drawing buffer. Measured both ways (17 Oct 2026):
//   * a real Android phone, pixel ratio 2.625: the buffer is full resolution (2400x1080 for a
//     914x411 page), so the product already lands right: factor 1.0 (2.63 sent taps to 38% of
//     where the finger was);
//   * a page-sized buffer (desktop Chrome's phone emulation, some scaled desktops), pixel ratio 2:
//     every tap landed at double its place: factor 2.
// One rule covers both, per axis: pixel ratio x page size / buffer size. ?tapscale=<n> forces a
// factor for testing another device. ow/input.js uses the same rule.
export function pointerFactor(c) {
  const q = new URLSearchParams(location.search).get('tapscale');
  if (q && Number(q) > 0) return [Number(q), Number(q)];
  const dpr = window.devicePixelRatio || 1;
  if (!c || !c.width || !c.height || !c.clientWidth || !c.clientHeight) return [dpr, dpr];
  return [dpr * c.clientWidth / c.width, dpr * c.clientHeight / c.height];
}

// How a tap reaches the game (?tapmode=, for finding what a device's browser accepts):
//   pointer  (default) mouse-type pointer events, pointer 1
//   both     the same, each followed by the matching classic mouse event
//   touchid  press and release sent while the finger is still down, as the finger's own pointer
export const TAPMODE = (new URLSearchParams(location.search).get('tapmode') || 'pointer').toLowerCase();
let tapPid = 1;
// what the game did with the last events sent ('h' = it handled them, '-' = it didn't), for owdiag
export const tapTrace = { last: '', errors: 0, error: '' };
addEventListener('error', (e) => { tapTrace.errors += 1; tapTrace.error = String(e.message || '').slice(0, 80); });
addEventListener('unhandledrejection', (e) => { tapTrace.errors += 1; tapTrace.error = String(e.reason?.message || e.reason || '').slice(0, 80); });
const MOUSE_TWIN = { pointermove: 'mousemove', pointerdown: 'mousedown', pointerup: 'mouseup' };


function mouse(type, x, y, button, buttons) {
  const c = gameCanvas();
  if (!c) return;
  // the game only clicks for a press and release it had focus for (a blur in between read as alt-tab: the button
  // showed pushed and never clicked, desktop touch emulation 3 Nov)
  if (type !== 'pointermove' && document.activeElement !== c) { try { c.focus({ preventScroll: true }); } catch { /* fine */ } }
  const r = c.getBoundingClientRect();
  const [fx, fy] = pointerFactor(c);
  const at = { clientX: r.left + (x - r.left) / fx, clientY: r.top + (y - r.top) / fy };
  const kept = c.dispatchEvent(gamePointerEvent(c, type, {
    bubbles: true, cancelable: true, composed: true, pointerId: tapPid, pointerType: 'mouse', isPrimary: true,
    button, buttons, ...at,
  }));
  let mark = (type === 'pointermove' ? 'm' : type === 'pointerdown' ? 'd' : 'u') + (kept ? '-' : 'h');
  if (TAPMODE === 'both' && MOUSE_TWIN[type]) {
    const k2 = c.dispatchEvent(new MouseEvent(MOUSE_TWIN[type], { bubbles: true, cancelable: true, composed: true,
      button: Math.max(0, button), buttons, ...at, view: window, detail: type === 'pointermove' ? 0 : 1 }));
    mark += k2 ? '-' : 'h';
  }
  if (type !== 'pointermove' || !tapTrace.last.endsWith(mark)) tapTrace.last = (tapTrace.last + ' ' + mark).slice(-24);
}
const moveTo = (x, y, buttons = 0) => mouse('pointermove', x, y, -1, buttons);
// A thumb rolls while it's down: up to TAP_SLOP css px of travel is still a tap, and a short, quick
// gesture that did turn into a drag (SLOPPY_PX, SLOPPY_MS) still ends as a click where it landed.
const TAP_SLOP = 20, SLOPPY_PX = 32, SLOPPY_MS = 450;
const LONG_MS = 500;   // a press held this long is a long-press (hover, tooltips), as on Android
// A game button shows pushed only once the game has said it is under the finger, which on a slow
// phone is 0.2-0.5 s after the touch. Players lift when they see it go in, so a long press on a
// pushed button is timed from when it showed pushed, not from the touch: lifting within HOLD_MS of
// that is the click. Before, a lift ~0.35 s after it went in was past LONG_MS from the touch, and the
// button popped back out without clicking ("only presses in, never clicks", phone, 2026-10-24).
const HOLD_MS = 700;
// and a tap on a window the game hadn't shown pushed yet (still asking what is under the finger)
// is a tap for up to this long: the player saw nothing happen, so they can't have meant a long press
const SLOW_TAP_MS = 1200;
const sloppyTap = (t, x, y) => Math.hypot(x - t.x0, y - t.y0) <= SLOPPY_PX && performance.now() - t.t0 < SLOPPY_MS;
// The game reads the cursor once a frame and only presses what the last frame saw under it, so
// each step waits for a frame the game has drawn (a timer alone can fall inside one slow frame).
// How long one game update takes, measured as we go (page frames, and the bridge's answers when it
// is live): a press and release sent closer together than this may have landed in ONE update.
// window.owTouch.tapTime: { frameMs, resent, force } (force > 0 sets frameMs, for tests and ?tapframe=).
export const TAPTIME = { frameMs: 16.7, resent: 0, force: +(new URLSearchParams(location.search).get('tapframe') || 0) };
const frameMs = () => (TAPTIME.force > 0 ? TAPTIME.force : TAPTIME.frameMs);
const noteFrame = (dt) => { if (dt > 4 && dt < 600) TAPTIME.frameMs = Math.round((TAPTIME.frameMs * 0.8 + dt * 0.2) * 10) / 10; };
let lastRafTs = 0;
const afterFrame = () => new Promise((ok) => requestAnimationFrame((ts) => {
  if (lastRafTs && ts - lastRafTs < 100) noteFrame(ts - lastRafTs);
  lastRafTs = ts;
  setTimeout(ok, 0);
}));
const wait = (ms) => new Promise((ok) => setTimeout(ok, ms));
// A frame the browser drew is not a frame the game ran: on a slow phone the game's update can
// lag the page's animation frame, so a move and the press after it landed in ONE game update,
// the press read the hover from before the move, and the button lit up but never clicked
// ("highlights, no action", Android Chrome on the character screens, 2026-10-24). A Lua call
// through the bridge is answered from inside a game update, so each answer proves the game has
// run once more since the call: two of them = the move is in and the hover it made is computed.
// Falls back to the frame wait when there is no VM (loading screens) or the bridge is slow.
async function gameTicks(n = 2) {
  const w = DRIVER?.live ? DRIVER : null;
  for (let i = 0; i < n; i++) {
    // the game's answer; two page frames only when there is no VM, or the bridge hasn't answered in
    // 0.4 s (busy). Not "whichever comes first": on a phone whose game lags the page, two page frames
    // come before the game's next update, and the press went in before the move did.
    const frames = () => afterFrame().then(afterFrame);
    const t0 = performance.now();
    await (w?.lua ? Promise.race([w.lua('return 1', 1500).then(() => noteFrame(performance.now() - t0), frames), wait(400).then(frames)]) : frames());
  }
}
const settle = () => gameTicks(2);
// One synthetic mouse sequence at a time: a second tap that lands while the first one's press and
// release are still going (two quick taps on Next, a slow phone) waits its turn instead of
// interleaving its move and press with the first's, which the game read as presses on nothing.
let mouseChain = Promise.resolve();
const inTurn = (fn) => (mouseChain = mouseChain.then(fn, fn));
// The release. If it went out within one game update of the press (a phone whose game lags the
// page: the settle between them was over before the game ran again), the game may have read the
// press and the release in the same update and clicked nothing, leaving the button drawn pushed:
// after one more settle the release is sent again, so a release lands in a later update than the press.
async function release(x, y, button, tDown, rec) {
  mouse('pointerup', x, y, button, 0);
  const held = performance.now() - tDown;
  if (rec) rec.held = Math.round(held);
  if (held < frameMs()) {
    await settle();
    mouse('pointerup', x, y, button, 0);
    TAPTIME.resent += 1;
    if (rec) rec.resent = true;
  }
}
// a click, played now (callers already in their turn); click() waits its turn
async function clickNow(x, y, button = 0, rec = null) {
  const bits = button === 2 ? 2 : 1;
  moveTo(x, y);
  await settle();                                  // the game has the cursor there and has hovered it
  mouse('pointerdown', x, y, button, bits);
  const tDown = performance.now();
  await settle();
  await release(x, y, button, tDown, rec);
}
const click = (x, y, button = 0, rec = null) => inTurn(() => clickNow(x, y, button, rec));

// The last 10 taps, with timings, for the ?touchdebug=1 line, Menu > Controls > Copy debug info and
// the tap test (window.owTouch.taps). Each: { at (s since the page opened), screen, how (tap, snap,
// magnet, aim, kept, ground, drag, dropped, window), x, y (the finger), px, py (where the click went),
// finger (ms down to up), held (ms between the press and release sent), resent (the release went twice) }.
export const tapLog = [];
const PAGE_T0 = performance.now();
function noteTap(rec) {
  rec.at = Math.round((performance.now() - PAGE_T0) / 100) / 10;
  for (const k of ['x', 'y', 'px', 'py']) if (rec[k] != null) rec[k] = Math.round(rec[k]);
  tapLog.push(rec);
  while (tapLog.length > 10) tapLog.shift();
  return rec;
}
/** One tap as a few characters: "snap 400,300>402,296 f120 h66 R" */
export function tapShort(r) {
  const to = r.px != null && (r.px !== r.x || r.py !== r.y) ? `>${r.px},${r.py}` : '';
  return `${r.how} ${r.x},${r.y}${to}${r.finger != null ? ' f' + r.finger : ''}${r.held != null ? ' h' + r.held : ''}${r.resent ? ' R' : ''}`;
}

// -- The controller -----------------------------------------------------------------------------

export function installTouch(bridge) {
  if (window.owTouch) return window.owTouch;
  const status = (window.owTouch = { on: false, inworld: false, presses: 0, tapCount: 0, taps: tapLog, tapTime: TAPTIME, drags: 0, errors: 0, layout: null });
  if (!bridge?.hold || !bridge?.fire) { status.state = 'no-bridge'; return status; }
  // the bridge when the client answers it, the canvas's keys when it doesn't (touch-driver.js)
  const wenilla = (DRIVER = makeDriver(bridge, { canvas: gameCanvas }));
  status.driver = wenilla.stats;
  const markPath = () => root.classList.toggle('keys', !wenilla.live);
  Object.defineProperty(status, 'bridge', { get: () => (wenilla.live ? 'live' : 'keys'), enumerable: true });
  style();
  const root = document.createElement('div');
  root.id = 'ow-touch';
  root.innerHTML = `
    <div class="zone glue"></div>
    <div class="loupe"><canvas width="264" height="264"></canvas></div>
    <div class="tapring"></div>
    <div class="tdebug"></div>
    <div class="zone world play"></div>
    <div class="zone stick-zone play"></div>
    <div class="stick play"><div class="knob"></div></div>
    <div class="ctl rbtn target play" aria-label="Nearest enemy">${ICON.crosshair}</div>
    <div class="ctl attack play" aria-label="Attack"><span class="ico">${ROLE_ICON.fight}</span></div>
    ${[0, 1, 2, 3].map((i) => `<div class="ctl key play" data-k="${i}"><div class="face"></div><div class="ring"></div><div class="cd"></div><div class="count"></div></div>`).join('')}
    <div class="ctl dots play" aria-label="Next actions"><i class="on"></i><i></i><i></i></div>
    <div class="ctl pill jump play" aria-label="Jump">${ICON.jump}</div>
    <div class="tray play">
      <div class="ctl rbtn handle" aria-label="Bags, quests, chat and menu"><span class="hico">${ICON.tray}</span><i class="badge"></i></div>
      <div class="ctl rbtn item" data-act="bags" aria-label="Bags">${ICON.bag}</div>
      <div class="ctl rbtn item" data-act="quests" aria-label="Quests">${ICON.quest}</div>
      <div class="ctl rbtn item" data-act="chat" aria-label="Chat">${ICON.chat}<i class="badge"></i></div>
      <div class="ctl rbtn item" data-act="menu" aria-label="Menu">${ICON.menu}</div>
    </div>
    <div class="sheet chat"><h3>Chat <button class="x" aria-label="Close">&times;</button></h3>
      <div class="log"></div>
      <form class="send"><input enterkeyhint="send" autocomplete="off" placeholder="Say something, or /command"><button>Send</button></form></div>
    <div class="sheet menu"><h3>Menu <button class="x" aria-label="Close">&times;</button></h3>
      <div class="grid">${MENU.map((m, i) => `<button data-m="${i}"><img alt=""><span>${m.label}</span></button>`).join('')}</div>
      <div class="row"><button data-x="full">Full screen</button><button data-x="site">Site menu</button><button data-x="off">Hide touch controls</button></div>
    <div class="settings">
        <button data-s="controls">Controls…</button><button data-s="loot">Auto loot: on</button>
        <button data-s="smaller">Interface −</button><button data-s="bigger">Interface +</button></div>
    </div>
    <div class="sheet controls"><h3>Controls <button class="x" aria-label="Close">&times;</button></h3>
      <div class="opts">
        <button class="opt" data-scheme="classic"><b>Classic</b>Stick walks and strafes. Drag the world to turn your character and the camera together, like holding the right mouse button on PC. Best for fighting.</button>
        <button class="opt" data-scheme="simple"><b>Simple</b>Stick walks and turns, like W A S D's turn keys. Drag the world to look around without turning; the camera swings back behind you as you walk. Easiest to learn.</button>
        <button class="opt" data-scheme="pc"><b>Keyboard &amp; mouse</b>No touch controls; the normal PC interface with its action bars and chat. For a tablet with a keyboard, or a phone on a controller. The small phone button in the corner brings touch back.</button>
        <div class="ctlrow"><button data-c="taptest">Tap test</button><button data-c="debug">Copy debug info</button><button data-c="edit">Edit layout</button></div>
        <div class="ctlrow"><button data-c="preset">Layout: Auto</button><button data-c="lefty">Left-handed: off</button>
          <button data-c="haptics">Vibration: on</button><button data-c="softlock">Soft lock: on</button></div>
        <div class="ctlrow"><button data-c="hold">Touch and hold: tooltip</button><button data-c="kbdhide">Keyboard hides touch: on</button></div>
        <label class="slider">Bigger interface <input type="range" min="70" max="140" step="5" data-c="uisize"><span class="v">100%</span></label>
        <label class="slider">Button size <input type="range" min="80" max="140" step="10" data-c="size"><span class="v">100%</span></label>
        <label class="slider">Opacity <input type="range" min="30" max="100" step="10" data-c="alpha"><span class="v">100%</span></label>
        <table class="vs"><tr><th></th><th>Phone</th><th>PC</th></tr>
          <tr><td>Move</td><td>Left stick (double-tap it: auto-run)</td><td>W A S D, Num Lock</td></tr>
          <tr><td>Turn / camera</td><td>Drag the world · pinch to zoom</td><td>Hold right / left mouse · wheel</td></tr>
          <tr><td>Target</td><td>Tap them, or the ⌖ button for the nearest enemy (hold it to clear). Swipe the target's frame for the next or last one</td><td>Click, Tab, Shift-Tab</td></tr>
          <tr><td>Keep your target</td><td>Soft lock: a tap on open ground keeps it</td><td>Click a unit</td></tr>
          <tr><td>Place a spell on the ground</td><td>Tap the spot (the X cancels)</td><td>Click the spot</td></tr>
          <tr><td>Talk, loot, use</td><td>Double-tap them, or tap with two fingers</td><td>Right-click</td></tr>
          <tr><td>Use, equip or sell an item</td><td>Tap it in the bag with two fingers (or set Touch and hold: right-click)</td><td>Right-click it</td></tr>
          <tr><td>Attack</td><td>Attack / Interact</td><td>Right-click the enemy</td></tr>
          <tr><td>Abilities</td><td>Four keys; swipe them for more. Hold one for its details, move or remove it</td><td>1 – 0, - and =</td></tr>
          <tr><td>New ability on a key</td><td>Drag it out of the Spellbook, then tap a key</td><td>Drag it onto the bar</td></tr>
          <tr><td>Tooltips</td><td>Touch and hold</td><td>Hover</td></tr>
          <tr><td>Bags, quests, chat, menu</td><td>The corner tray</td><td>B, L, Enter, Esc</td></tr>
          <tr><td>Close windows</td><td>The Close button at the top</td><td>Esc</td></tr>
          <tr><td>Type in a box</td><td>Tap the bar that appears at the top</td><td>Just type</td></tr></table>
      </div></div>
    <div class="ctl rbtn restore" aria-label="Touch controls">${ICON.phone}</div>
    <div class="ctl rbtn flip" aria-label="Switch between the world's controls and the character screens' taps">${ICON.tray}</div>
    <div class="ctl pill closer play" aria-label="Close windows">${ICON.close}<span>Close</span></div>
    <div class="strip play"></div>
    <form class="typebar"><input enterkeyhint="go" autocomplete="off" autocapitalize="off" spellcheck="false" placeholder="Tap here to type"><button type="submit">Enter</button><button type="button" class="tb-x" aria-label="Stop typing">&times;</button></form>
    <div class="info"><div class="t"></div><div class="b"></div></div>
    <div class="ctl rbtn kbd" aria-label="Keyboard">${ICON.keyboard}</div>
    <div class="ctl rbtn gear" aria-label="Controls, tap test and debug info">${ICON.gear}</div>
    <div class="castbar play" aria-hidden="true"><i></i><span></span></div>
    <div class="gaim play"><span>Tap where it should land</span><button type="button" class="ctl gx" aria-label="Cancel the spell">${ICON.close}</button></div>
    <div class="taptest"><div class="tt-dot"></div><div class="tt-msg"></div>
      <div class="tt-b"><button type="button" data-tt="again">Again</button><button type="button" data-tt="copy">Copy result</button><button type="button" data-tt="close">Close</button></div></div>
    <div class="editbar"><div class="eb-t">Drag the stick, the ring, Jump and the tray</div>
      <div class="eb-b"><button type="button" data-e="smaller">Ring −</button><button type="button" data-e="bigger">Ring +</button>
        <button type="button" data-e="phone">Phone</button><button type="button" data-e="tablet">Tablet</button>
        <button type="button" data-e="reset">Reset</button><button type="button" data-e="done">Done</button></div>
      <form class="eb-code"><input autocomplete="off" autocapitalize="off" spellcheck="false" placeholder="Layout code">
        <button type="button" data-e="copy">Copy code</button><button type="submit" data-e="use">Use code</button></form></div>
    <input class="ghost" autocomplete="off" autocapitalize="off" autocorrect="off" spellcheck="false" enterkeyhint="done" tabindex="-1" aria-hidden="true">
    <div class="kecho"></div>
    <label class="hap" aria-hidden="true"><input type="checkbox" switch tabindex="-1"></label>
    <div class="toast"></div>
    <div class="fsgo">${ICON.full}<div>Tap to play full screen</div><button type="button" class="inst">Add Leyline to your home screen</button><button type="button" class="skip">Not now</button></div>
    <div class="rotate">${ICON.rotate}<div>Turn your phone sideways</div><div class="sub">The game is laid out for landscape</div><button type="button" class="go">Play full screen</button><button type="button" class="inst">Add Leyline to your home screen</button></div>`;
  document.body.append(root);
  const $ = (s) => root.querySelector(s);
  const $$ = (s) => [...root.querySelectorAll(s)];
  const sz = $('.stick-zone');
  const stick = $('.stick'), knob = $('.knob'), keys = $$('.key'), attack = $('.attack'), dots = $$('.dots i');

  let toastTimer = 0;
  const toast = (text, ms = 2600) => {
    const t = $('.toast');
    t.textContent = text;
    t.classList.add('on');
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => t.classList.remove('on'), ms);
  };
  const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
  // A short buzz on a press (Menu > Controls > Vibration turns it off). 12 ms: many phone motors don't start for less
  // (the old 8 ms was felt on few phones). An iPhone has no navigator.vibrate, but Safari (iOS 18 and later) gives a
  // haptic tick when a switch toggles, so a hidden one is toggled instead (WebHaptics' way). Never takes the focus.
  const hapSwitch = $('.hap'), hapInput = hapSwitch.querySelector('input');
  const HAPTIC = typeof navigator.vibrate === 'function' ? 'vibrate' : (iOSDevice() && 'switch' in hapInput ? 'switch' : 'none');
  status.haptics = HAPTIC;
  let lastBuzz = 0;
  const buzz = (ms = 12) => {
    if (LS('ow:haptics') === 'off') return;
    const now = performance.now();
    if (now - lastBuzz < 40) return;          // one tick per press, not a rattle from a burst of them
    lastBuzz = now;
    status.buzzes = (status.buzzes || 0) + 1;
    try {
      if (HAPTIC === 'vibrate') navigator.vibrate(ms);
      else if (HAPTIC === 'switch') {
        const had = document.activeElement;
        hapSwitch.click();
        if (document.activeElement !== had && had?.focus) had.focus({ preventScroll: true });
      }
    } catch { /* none */ }
  };
  const lua = (chunk) => wenilla.lua(chunk, 3000).catch(() => { status.errors += 1; return []; });
  // soft lock (Menu > Controls): a tap on open ground keeps the target
  const softLock = () => LS('ow:softlock') !== 'off';
  // a ground-targeted spell (SpellIsTargeting) waiting for the tap that places it
  let aimGround = false;
  const setAimGround = (on) => {
    on = !!on;
    if (on === aimGround) return;
    aimGround = on;
    status.aimGround = on;
    root.classList.toggle('aimground', on);
    if (on) buzz();
  };

  // ---- layout (CSS positions from the spec, inside the safe area) --------------------------------
  const SAFE = { l: 0, r: 0, t: 0, b: 0 };
  const STICK_HOME = { left: 24, bottom: 24, size: 120 };
  // Layouts (Menu > Controls). Presets: Phone (the compact cluster in the bottom corners) and Tablet (a bigger ring,
  // both thumbs a little higher up the sides, where a tablet is held); Auto picks by screen size. Left-handed mirrors
  // the stick and the ring (the tray stays top right). Edit layout's own places (the stick, the ring round Attack, Jump,
  // the tray, the ring's size) are kept per character, as screen fractions so they survive a turn or a new phone,
  // and always clamped on screen. Everything is worked out right-handed, then mirrored.
  const PRESETS = { phone: { scale: 1, lift: 0 }, tablet: { scale: 1.2, lift: 0.1 } };
  const presetPref = () => { const p = LS('ow:touch-preset'); return p === 'phone' || p === 'tablet' ? p : 'auto'; };
  const presetNow = () => { const p = presetPref(); return p !== 'auto' ? p : (Math.min(innerWidth, innerHeight) >= 600 ? 'tablet' : 'phone'); };
  const lefty = () => LS('ow:lefty') === 'on';
  const clampN = (v, lo, hi) => Math.min(Math.max(v, lo), Math.max(lo, hi));
  const ZMIN = 0.8, ZMAX = 1.5;
  // Menu > Controls > Button size (80-140%): the stick and the preset's ring together (an edited ring keeps its own size)
  const sizePref = () => { const v = +(LS('ow:touch-size') || 1); return Number.isFinite(v) ? clampN(v, 0.8, 1.4) : 1; };
  // Menu > Controls > Opacity (30-100%): the controls over the world
  const alphaPref = () => { const v = +(LS('ow:touch-alpha') || 1); return Number.isFinite(v) ? clampN(v, 0.3, 1) : 1; };
  let stickK = 1;                        // the stick's size now (its range follows)
  let layoutKey = 'ow:touch-layout:_';
  let custom = null;                     // { s, r, j, t: [fx, fy], z } or null (the preset)
  const pair = (v) => Array.isArray(v) && v.length === 2 && v.every((n) => Number.isFinite(n) && n >= 0 && n <= 1);
  const validCustom = (v) => (v && pair(v.s) && pair(v.r) && pair(v.t) && (v.j === null || pair(v.j)) && Number.isFinite(v.z)
    ? { s: v.s, r: v.r, j: v.j, t: v.t, z: clampN(v.z, ZMIN, ZMAX) } : null);
  const loadCustom = () => { try { custom = validCustom(JSON.parse(LS(layoutKey) || 'null')); } catch { custom = null; } };
  const saveCustom = () => LS(layoutKey, custom ? JSON.stringify(custom) : null);
  loadCustom();
  const CENTRES = {};                    // where each group is now (right-handed px), for Edit layout
  function layout() {
    const cs = getComputedStyle(document.documentElement);
    const env = (k) => parseFloat(cs.getPropertyValue(k)) || 0;
    SAFE.l = env('--ow-sl'); SAFE.r = env('--ow-sr'); SAFE.t = env('--ow-st'); SAFE.b = env('--ow-sb');
    const W = innerWidth, H = innerHeight, mir = lefty();
    const sl = mir ? SAFE.r : SAFE.l, sr = mir ? SAFE.l : SAFE.r;   // right-handed insets
    const X = (x) => Math.round(mir ? W - x : x);
    const pname = presetNow(), pre = PRESETS[pname];
    const stickLift = custom ? 0 : Math.round(pre.lift * H);
    const sk = sizePref();
    stickK = sk;
    stick.style.scale = sk === 1 ? '' : String(sk);
    root.style.setProperty('--ow-alpha', String(alphaPref()));
    // the stick's home (where it rests; a touch in its zone moves it under the thumb)
    const sh = 60 * sk, sm = sh + 4;
    let hx = sl + STICK_HOME.left + sh, hy = H - SAFE.b - STICK_HOME.bottom - sh - stickLift;
    if (custom) { hx = custom.s[0] * W; hy = custom.s[1] * H; }
    hx = clampN(hx, sl + sm, W - sr - sm); hy = clampN(hy, SAFE.t + sm, H - SAFE.b - sm);
    CENTRES.s = [hx, hy];
    stick.dataset.hx = X(hx); stick.dataset.hy = Math.round(hy);
    if (!stick.classList.contains('live')) { stick.style.left = X(hx) + 'px'; stick.style.top = Math.round(hy) + 'px'; }
    // its zone: 42% x 62% of the screen round the home (the bottom-left corner for the presets)
    const zw = W * 0.42, zh = H * 0.62;
    const zl = clampN(hx - zw / 2, 0, W - zw), zt = clampN(hy - zh * 0.6, 0, H - zh);
    Object.assign(sz.style, { left: Math.round(mir ? W - zl - zw : zl) + 'px', top: Math.round(zt) + 'px', width: Math.round(zw) + 'px', height: Math.round(zh) + 'px', bottom: 'auto' });
    // right-thumb cluster (owner 3 Nov 01:41: "the buttons are misaligned"), laid out the way radial gamepads do it
    // (RadialGamePad, Lemuroid's controls): every key on one ring round Attack, the same angle apart, each angle
    // worked out from the ring's radius so neighbours keep GAP px between them (Material's 48 + 8 rule), and the
    // whole cluster kept inside the safe area: the reticle on the ring up and right, the four keys on it to the
    // left, Jump under Attack with its own gap, the page dots centred over the reticle. s scales it all.
    const GAP = 8, EDGE = 8, TA = 62;                             // TA: the reticle's angle (up and right)
    const d = $('.dots');
    const deg = (r) => r * 180 / Math.PI;
    const off = (dg, r) => { const a = dg * Math.PI / 180; return [r * Math.cos(a), -r * Math.sin(a)]; };
    const jumpOn = !(custom && custom.j);
    // the ring at size s, lifted by lift px: its geometry and where Attack's centre goes
    function geo(s, lift) {
      const RA = 48 * s, RK = 24 * s, RT = 23 * s, JH = 30 * s, JW = 32 * s;
      // the smallest ring on which the four keys fit between the reticle and the lowest angle a key may take (no key
      // lower than Attack's bottom edge: Jump sits under it), each two the same angle apart with GAP px between them
      let R = RA + GAP + RK + 4, step = 0;
      for (; R < 260; R += 2) {
        step = deg(2 * Math.asin((RK + GAP / 2) / R)) + 1;
        if (TA + 4 * step <= 180 + deg(Math.asin((RA - RK) / R))) break;
      }
      const ANG = [0, 1, 2, 3].map((i) => TA + step * (i + 1));
      const dw = (d.offsetWidth || 52) * s;
      const [tox, toy] = off(TA, R);
      const doy = toy - RT - GAP - 14 * s;
      // the ring's extent round Attack's centre ([dx, dy, half width, half height]), to keep it on screen
      const parts = [[0, 0, RA, RA], [tox, toy, RT, RT], ...ANG.map((a) => [...off(a, R), RK, RK]), [tox, doy, 0, 11 * s]];
      if (jumpOn) parts.push([0, RA + GAP + JH / 2, JW, JH / 2]);
      const minX = Math.min(...parts.map((q) => q[0] - q[2])), maxX = Math.max(...parts.map((q) => q[0] + q[2]));
      const minY = Math.min(...parts.map((q) => q[1] - q[3])), maxY = Math.max(...parts.map((q) => q[1] + q[3]));
      let ax = W - sr - EDGE - Math.max(RA, tox + RT);
      let ay = H - SAFE.b - EDGE - (jumpOn ? JH + GAP : 0) - RA - lift;
      if (custom) { ax = custom.r[0] * W; ay = custom.r[1] * H; }
      ax = Math.round(clampN(ax, sl + EDGE - minX, W - sr - EDGE - maxX));
      ay = Math.round(clampN(ay, SAFE.t + EDGE - minY, H - SAFE.b - EDGE - maxY));
      return { s, RA, RK, RT, JH, JW, R, step, ANG, dw, tox, toy, doy, ax, ay, top: ay + minY };
    }
    // a preset gives back its lift, then its size, until the ring clears the tray (top right) on a short screen
    let s = Math.round(clampN(custom ? custom.z : pre.scale * sk, ZMIN, ZMAX) * 100) / 100, lift = custom ? 0 : Math.round(pre.lift * H);
    let g = geo(s, lift);
    const trayBottom = SAFE.t + 10 + 36 + GAP;
    while (!custom && !mir && g.top < trayBottom && (lift > 0 || s > 1)) {
      if (lift > 0) lift = Math.max(0, lift - 10); else s = Math.max(1, Math.round((s - 0.05) * 100) / 100);
      g = geo(s, lift);
    }
    const { RA, RK, RT, JH, JW, R, step, ANG, dw, tox, toy, doy, ax, ay } = g;
    CENTRES.r = [ax, ay]; CENTRES.z = s;
    const put = (el, x, y) => { el.style.left = X(x) + 'px'; el.style.top = Math.round(y) + 'px'; el.style.scale = s === 1 ? '' : String(s); };
    put(attack, ax, ay);
    const tx = ax + tox, ty = ay + toy;
    put($('.target'), tx, ty);
    keys.forEach((k, i) => { const [ox, oy] = off(ANG[i], R); put(k, ax + ox, ay + oy); });
    put(d, Math.min(tx, W - sr - EDGE - dw / 2), ay + doy);
    let jx = ax, jy = ay + RA + GAP + JH / 2;
    if (!jumpOn) { jx = clampN(custom.j[0] * W, sl + EDGE + JW, W - sr - EDGE - JW); jy = clampN(custom.j[1] * H, SAFE.t + EDGE + JH / 2, H - SAFE.b - EDGE - JH / 2); }
    CENTRES.j = [jx, jy];
    put($('.jump'), jx, jy);
    // the cast bar: over the ring, clear of the dots and keys
    const top = Math.min(ay + doy - 11 * s, ...ANG.map((a) => ay + off(a, R)[1] - RK), ty - RT);
    const cb = $('.castbar');
    cb.style.left = Math.round(clampN(X(ax - R / 2), 84 + EDGE + SAFE.l, W - 84 - EDGE - SAFE.r)) + 'px';
    cb.style.top = Math.round(Math.max(SAFE.t + EDGE + 8, top - GAP - 8)) + 'px';
    status.layout = { W, H, ax: X(ax), ay, R: Math.round(R), step: Math.round(step), tx: X(tx), ty: Math.round(ty), s, stick: sk, alpha: alphaPref(), preset: pname, pref: presetPref(), mirror: mir, custom: !!custom,
      safe: [SAFE.l, SAFE.r, SAFE.t, SAFE.b].map(Math.round) };
    const st = $('.strip');
    st.style.left = (W / 2) + 'px'; st.style.top = (H - SAFE.b - 22 - 40) + 'px';
    // the tray: top right (never mirrored: the player frame has the top left), or where it was put
    const tr = $('.tray');
    let trR = SAFE.r + 10, trT = SAFE.t + 10;
    if (custom) { trR = W - custom.t[0] * W - 18; trT = custom.t[1] * H - 18; }
    trR = Math.round(clampN(trR, SAFE.r + 4, W - 40)); trT = Math.round(clampN(trT, SAFE.t + 4, H - 40));
    tr.style.right = trR + 'px'; tr.style.top = trT + 'px';
    CENTRES.t = [W - trR - 18, trT + 18];
  }
  // safe-area insets as numbers (env() only works in CSS)
  const sa = document.createElement('style');
  sa.textContent = ':root{--ow-sl:env(safe-area-inset-left,0px);--ow-sr:env(safe-area-inset-right,0px);--ow-st:env(safe-area-inset-top,0px);--ow-sb:env(safe-area-inset-bottom,0px)}';
  document.head.append(sa);
  // the controls at once; the game's frames once the resizing stops (an address bar sliding fires a burst of these, and
  // each was a Lua call)
  let resizeTimer = 0;
  addEventListener('resize', () => {
    layout();
    clearTimeout(resizeTimer);
    resizeTimer = setTimeout(() => { if (status.inworld) applyGameLayout(); }, 250);
  });

  // ---- the stick --------------------------------------------------------------------------------
  // sideways on the stick strafes (camera drags turn), or turns like A/D when "Stick: turn" is set
  const HOLD = { up: 'MOVEFORWARD', down: 'MOVEBACKWARD', left: 'STRAFELEFT', right: 'STRAFERIGHT' };
  const setStickMode = (turn) => { HOLD.left = turn ? 'TURNLEFT' : 'STRAFELEFT'; HOLD.right = turn ? 'TURNRIGHT' : 'STRAFERIGHT'; };
  setStickMode(LS('ow:scheme') === 'simple');
  const held = new Set();
  function setHolds(want) {
    for (const k of Object.keys(HOLD)) {
      const on = want.has(k);
      if (on && !held.has(k)) { wenilla.hold(HOLD[k], true); held.add(k); }
      if (!on && held.has(k)) { wenilla.hold(HOLD[k], false); held.delete(k); }
    }
  }
  let stickId = null, sx = 0, sy = 0, lastStickTap = 0, stickT0 = 0, stickFar = 0;
  const RANGE = 60, DEAD = 0.22;
  function stickMove(x, y) {
    let dx = x - sx, dy = y - sy;
    const d = Math.hypot(dx, dy), range = RANGE * stickK;
    stickFar = Math.max(stickFar, d);
    if (d > range) { dx *= range / d; dy *= range / d; }
    // the knob is inside the scaled stick: its offset in the stick's own units
    knob.style.transform = `translate(${dx / stickK}px, ${dy / stickK}px)`;
    const want = new Set();
    if (d / range > DEAD) {
      const a = Math.atan2(-dy, dx) * 180 / Math.PI;   // 0 = right, 90 = up
      if (a > 22.5 && a < 157.5) want.add('up');
      if (a < -22.5 && a > -157.5) want.add('down');
      if (a > 112.5 || a < -112.5) want.add('left');
      if (a < 67.5 && a > -67.5) want.add('right');
    }
    setHolds(want);
  }

  sz.addEventListener('pointerdown', (e) => {
    if (winRects.some(([l, t, r, b]) => e.clientX >= l && e.clientX <= r && e.clientY >= t && e.clientY <= b)) {
      $('.world').dispatchEvent(new PointerEvent('pointerdown', e));
      return;
    }
    // a first finger while another still "holds" the stick: that one's lift was lost (the browser dropped it), so it
    // lets go instead of blocking the stick for good
    if (stickId !== null && !(e.isPrimary && e.pointerType !== 'mouse')) return;
    if (stickId !== null) { stickReset(); status.stale = (status.stale || 0) + 1; }
    stickId = e.pointerId;
    sz.setPointerCapture(e.pointerId);
    stickT0 = performance.now();
    stickFar = 0;
    sx = e.clientX; sy = e.clientY;
    stick.classList.add('live');
    stick.style.left = sx + 'px'; stick.style.top = sy + 'px';
    stickMove(sx, sy);
    status.presses += 1;
  });
  sz.addEventListener('pointermove', (e) => { if (e.pointerId === stickId) stickMove(e.clientX, e.clientY); });
  // the stick back home, nothing held
  function stickReset() {
    stickId = null;
    setHolds(new Set());
    knob.style.transform = '';
    stick.classList.remove('live');
    stick.style.left = stick.dataset.hx + 'px'; stick.style.top = stick.dataset.hy + 'px';
  }
  const stickEnd = (e) => {
    if (e.pointerId !== stickId) return;
    stickReset();
    // two quick taps without moving = auto-run on / off
    const now = performance.now();
    if (now - stickT0 < 220 && stickFar < 10) {
      if (now - lastStickTap < 380) { wenilla.fire('TOGGLEAUTORUN'); toast('Auto-run'); buzz(); lastStickTap = 0; } else lastStickTap = now;
    } else lastStickTap = 0;
  };
  sz.addEventListener('pointerup', stickEnd);
  sz.addEventListener('pointercancel', stickEnd);

  // ---- buttons ----------------------------------------------------------------------------------
  // a hold of half a second on an element (no travel): fn runs once, while the finger is still down
  function longPress(el, fn, ms = 500) {
    let t = 0, x = 0, y = 0;
    el.addEventListener('pointerdown', (e) => { x = e.clientX; y = e.clientY; clearTimeout(t); t = setTimeout(() => { t = 0; buzz(); fn(e); }, ms); });
    el.addEventListener('pointermove', (e) => { if (t && Math.hypot(e.clientX - x, e.clientY - y) > 14) { clearTimeout(t); t = 0; } });
    const stop = () => { clearTimeout(t); t = 0; };
    el.addEventListener('pointerup', stop);
    el.addEventListener('pointercancel', stop);
  }
  function press(el, fn) {
    el.addEventListener('pointerdown', (e) => {
      e.preventDefault();
      el.setPointerCapture?.(e.pointerId);
      el.classList.add('pressed');
      buzz();
      status.presses += 1;
      try { fn(e); } catch { status.errors += 1; }
    });
    const up = () => el.classList.remove('pressed');
    el.addEventListener('pointerup', up);
    el.addEventListener('pointercancel', up);
  }
  // the look and feel of a press (pushed in, a buzz) for a button that acts on the lift (takeTap)
  function pressLook(el) {
    el.addEventListener('pointerdown', () => { el.classList.add('pressed'); buzz(); });
    const up = () => el.classList.remove('pressed');
    el.addEventListener('pointerup', up);
    el.addEventListener('pointercancel', up);
  }
  // the reticle on the ring is the nearest-enemy button: TARGETNEARESTENEMY through the bridge (Tab without it); each
// tap after the first goes on to the next nearest, as Tab does
press($('.target'), () => { wenilla.fire('TARGETNEARESTENEMY'); status.nearest = (status.nearest || 0) + 1; });
  // Jump: held while the thumb is on it (JUMP's binding swims up while held, as Space does).
  // The 1.12 file drew this pill but never wired it.
  let releaseJump = () => {};
  {
    const jump = $('.jump');
    let jumpId = null;
    press(jump, (e) => { jumpId = e.pointerId; wenilla.hold('JUMP', true); });
    const jumpUp = (e) => { if (e.pointerId !== jumpId) return; jumpId = null; wenilla.hold('JUMP', false); };
    jump.addEventListener('pointerup', jumpUp);
    jump.addEventListener('pointercancel', jumpUp);
    releaseJump = () => { if (jumpId === null) return; jumpId = null; wenilla.hold('JUMP', false); jump.classList.remove('pressed'); };
  }
  longPress($('.target'), () => { lua('ClearTarget()'); toast('Target cleared', 1200); });
  let targetState = { exists: false, hostile: false, dead: false };
  let role = '';
  const setRole = (r) => {
    r = ROLE_ICON[r] ? r : 'fight';
    if (r === role) return;
    role = r;
    const ico = attack.querySelector('.ico');
    ico.innerHTML = ROLE_ICON[r];
    ico.className = 'ico ' + (ROLE_TEX[r]?.includes('Gossip') ? 'gossip' : 'cursor') + (r === 'inprogress' ? ' dim' : '') + (r !== 'fight' ? ' tip' : '');
    if (ROLE_TEX[r]) iconUrl(ROLE_TEX[r]).then((u) => { if (u && role === r) ico.innerHTML = `<img alt="" src="${u}">`; });
    attack.setAttribute('aria-label', r === 'fight' ? 'Attack' : ROLE_SAY[r] || 'Interact');
  };
  press(attack, () => {
    if (!targetState.exists) { wenilla.fire('TARGETNEARESTENEMY'); return; }
    if (targetState.hostile && !targetState.dead) { wenilla.fire('ATTACKTARGET'); return; }
    toast(ROLE_SAY[role] || 'Double-tap them to talk, trade or shop');
  });

  // ability keys: four per page, page 0 = action buttons 1-4, 1 = 5-8, 2 = 9-12. A key fires when
  // the thumb lifts without travelling; a sideways swipe across the keys turns the page instead.
  let page = 0;
  const slotOf = (i) => page * 4 + i + 1;
  const setPage = (p) => {
    page = (p + 3) % 3;
    dots.forEach((d, i) => d.classList.toggle('on', i === page));
    keyNumbers();
    refreshActions(true);
    toast('Actions ' + (page * 4 + 1) + '–' + (page * 4 + 4), 1000);
  };
  keys.forEach((k, i) => {
    let down = null;
    k.addEventListener('pointerdown', (e) => {
      e.preventDefault();
      k.setPointerCapture?.(e.pointerId);
      down = { x: e.clientX, y: e.clientY, id: e.pointerId, held: false };
      k.classList.add('pressed');
      const me = down;
      setTimeout(() => { if (down === me && !me.moved) { me.held = true; buzz(); keyInfo(i); } }, 480);
    });
    k.addEventListener('pointermove', (e) => { if (down && e.pointerId === down.id && Math.hypot(e.clientX - down.x, e.clientY - down.y) > 14) down.moved = true; });
    const end = (e) => {
      if (!down || e.pointerId !== down.id) return;
      k.classList.remove('pressed');
      const dx = e.clientX - down.x, dy = e.clientY - down.y;
      const held = down.held;
      down = null;
      if (e.type !== 'pointerup' || held) return;
      if (Math.abs(dx) > 34 && Math.abs(dx) > Math.abs(dy)) { setPage(page + (dx < 0 ? 1 : -1)); buzz(); return; }
      if (Math.hypot(dx, dy) > 24) return;
      buzz();
      status.presses += 1;
      // something on the cursor (a spell dragged out of the spellbook, an item, a moved action):
      // the tap puts it on this key, the way dropping it on an action button does
      if (moving) {
        // a key picked up with Move: swap it with whatever is here (the old one goes where it came from)
        const from = moving; moving = null;
        lua(`${SLOT_FN}\nPlaceAction(slot(${slotOf(i)})) PlaceAction(slot(${from})) ClearCursor()`).then(() => refreshActions(true));
        toast('Moved', 900);
        return;
      }
      if (extras.cursor) {
        lua(`${SLOT_FN}\nPlaceAction(slot(${slotOf(i)})) ClearCursor()`).then(() => refreshActions(true));
        toast('Placed', 900); extras.cursor = false;
        return;
      }
      // through the bridge: the slot itself (independent of which bar frame is showing);
      // without it: the key, whose binding pages like the stock bars do (they stay visible then)
      // A spell that needs a place on the ground (SpellIsTargeting after it) waits for the next tap in the world.
      if (wenilla.live) {
        lua(`${SLOT_FN}\nUseAction(slot(${slotOf(i)}))\nreturn (SpellIsTargeting and SpellIsTargeting()) and 1 or 0`)
          .then((r) => { if (r?.[0] == 1) { setAimGround(true); toast('Tap where it should land', 1800); } });
      } else wenilla.fire('ACTIONBUTTON' + slotOf(i));
      if (!wenilla.live) showCloser();
    };
    k.addEventListener('pointerup', end);
    k.addEventListener('pointercancel', end);
  });
  press($('.dots'), () => setPage(page + 1));
  // no bridge: the keys can't show icons, so each shows the number of the key it presses
  const KEYCAP = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '-', '='];
  function keyNumbers() {
    if (wenilla.live) return;
    keys.forEach((k, i) => { k.classList.add('empty'); k.querySelector('.count').textContent = KEYCAP[slotOf(i) - 1]; });
  }

  // the info card: what is on a key (the game's own tooltip text), with Move and Remove
  const info = $('.info');
  const card = (text, buttons) => {
    const t = info.querySelector('.t'), b = info.querySelector('.b');
    t.innerHTML = text;
    b.replaceChildren(...buttons.map(([label, fn]) => {
      const el = document.createElement('button');
      el.textContent = label;
      el.addEventListener('click', () => { info.classList.remove('on'); fn?.(); });
      return el;
    }));
    info.classList.add('on', 'fresh');
    setTimeout(() => info.classList.remove('fresh'), 450);   // the lifting finger's click must not land on a button
  };
  const TIP_LUA = (n) => `${SLOT_FN}
local s = slot(${n})
if not HasAction(s) then return "" end
GameTooltip:SetOwner(UIParent, "ANCHOR_NONE")
GameTooltip:SetAction(s)
local t = {}
for i = 1, GameTooltip:NumLines() do
  local l, r = getglobal("GameTooltipTextLeft" .. i), getglobal("GameTooltipTextRight" .. i)
  local a = l and l:GetText() or ""
  local b = (r and r:IsShown() and r:GetText()) or ""
  if a ~= "" or b ~= "" then table.insert(t, a .. (b ~= "" and ("   " .. b) or "")) end
end
GameTooltip:Hide()
return table.concat(t, "\\n")`;
  async function keyInfo(i) {
    const n = slotOf(i);
    const r = await lua(TIP_LUA(n));
    const text = String(r?.[0] || '');
    if (!text) {
      card('<b>Empty slot</b>\nOpen the Spellbook (or your bags), drag a spell or item out, then tap this key to put it here.',
        [['Open Spellbook', () => wenilla.fire('TOGGLESPELLBOOK')], ['Close']]);
      return;
    }
    const [title, ...rest] = text.split('\n');
    card(`<b>${esc(title)}</b>\n${esc(rest.join('\n'))}`, [
      ['Move', () => { lua(`${SLOT_FN}\nPickupAction(slot(${n}))`).then(() => refreshActions(true)); moving = n; toast('Now tap the key to move it to (swipe for other pages)', 3500); }],
      ['Remove', () => { lua(`${SLOT_FN}\nPickupAction(slot(${n})) ClearCursor()`).then(() => refreshActions(true)); toast('Removed', 900); }],
      ['Close'],
    ]);
  }

  // drawer and sheets
  const sheets = { chat: $('.sheet.chat'), menu: $('.sheet.menu'), controls: $('.sheet.controls') };
  const openSheet = (name) => {
    for (const [n, el] of Object.entries(sheets)) el.classList.toggle('open', n === name && !el.classList.contains('open'));
    root.classList.toggle('sheet-open', Object.values(sheets).some((el) => el.classList.contains('open')));
    // the chat box takes the focus at once, inside the tap that opened it: a phone only raises its keyboard for a focus
    // made in a user gesture (a timer after it, or a finger's press, is not one)
    if (name === 'chat' && sheets.chat.classList.contains('open')) { $$('.badge').forEach((b) => b.classList.remove('on')); try { sheets.chat.querySelector('input').focus({ preventScroll: true }); } catch { /* fine */ } }
    if (name === 'menu') loadMenuIcons();
  };
  // the tray: one corner button; open, the four buttons slide out along the top edge, folding away after use
  const tray = $('.tray'), handle = $('.tray .handle');
  let trayTimer = 0;
  const setTray = (open) => {
    tray.classList.toggle('open', open);
    handle.querySelector('.hico').innerHTML = open ? ICON.close : ICON.tray;
    clearTimeout(trayTimer);
    if (open) trayTimer = setTimeout(() => setTray(false), 5000);
  };
  press(handle, () => setTray(!tray.classList.contains('open')));
  // on the lift (takeTap): Chat focuses its box, and a phone raises its keyboard only for a focus inside a user gesture,
  // which a finger's press is not; the press still shows pushed and buzzes
  $$('.tray .item').forEach((b) => { pressLook(b); takeTap(b, () => {
    const a = b.dataset.act;
    status.presses += 1;
    setTray(false);
    if (a === 'bags') { wenilla.fire('OPENALLBAGS'); showCloser(); }
    else if (a === 'quests') { wenilla.fire('TOGGLEQUESTLOG'); showCloser(); }
    else openSheet(a);
  }); });
  $$('.sheet .x').forEach((x) => x.addEventListener('click', () => openSheet(null)));
  let menuIcons = false;
  function loadMenuIcons() {
    if (menuIcons) return;
    menuIcons = true;
    $$('.grid button').forEach((b) => iconUrl(MENU[+b.dataset.m].icon).then((u) => { if (u) b.querySelector('img').src = u; }));
  }
  $$('.grid button').forEach((b) => b.addEventListener('click', () => {
    const m = MENU[+b.dataset.m];
    if (m.lua && wenilla.live) lua(m.lua); else if (m.cmd) wenilla.fire(m.cmd); else if (m.chat) wenilla.chat(m.chat);
    if (m.cmd !== 'TOGGLEGAMEMENU') showCloser();
    openSheet(null);
  }));
  // settings: stick strafe/turn, auto loot, interface size
  const sLabel = () => {
    root.querySelector('[data-s="loot"]').textContent = 'Auto loot: ' + (LS('ow:autoloot') === 'off' ? 'off' : 'on');
  };
  sLabel();
  // control schemes: Classic (strafe; drag turns you), Simple (stick turns; drag only looks),
  // Keyboard & mouse (touch off, the stock interface back, a corner button to return)
  const scheme = () => LS('ow:scheme') || 'classic';
  const markScheme = () => $$('.opt').forEach((o) => o.classList.toggle('sel', o.dataset.scheme === scheme()));
  function applyScheme(sc, announce) {
    LS('ow:scheme', sc);
    setStickMode(sc === 'simple');
    markScheme();
    if (sc === 'pc') {
      openSheet(null);
      turnOff();
      root.classList.add('pcmode');
      if (announce) toast('Keyboard & mouse: tap the phone button in the corner for touch controls again', 4500);
      return;
    }
    if (root.classList.contains('pcmode')) { root.classList.remove('pcmode'); turnOn(); }
    if (announce) toast(sc === 'simple' ? 'Simple: the stick turns you, dragging only looks around' : 'Classic: the stick strafes, dragging turns you', 2800);
  }
  root.querySelector('[data-s="controls"]').addEventListener('click', () => { markScheme(); openSheet('controls'); });
  $$('.opt').forEach((o) => o.addEventListener('click', () => applyScheme(o.dataset.scheme, true)));
  press($('.restore'), () => applyScheme('classic', true));
  press($('.flip'), () => {
    const world = wenilla.session() !== 'inworld';
    wenilla.force?.(world ? 'inworld' : 'charselect');
    toast(world ? 'World controls' : 'Character screen taps', 1400);
  });
  markPath();
  setInterval(markPath, 1000);
  root.querySelector('[data-s="loot"]').addEventListener('click', () => { LS('ow:autoloot', LS('ow:autoloot') === 'off' ? 'on' : 'off'); sLabel(); applyAutoLoot(); });
  const uiStep = (d) => {
    const f = Math.min(1.4, Math.max(0.7, Math.round(((+(LS('ow:touch-ui') || 1)) + d) * 10) / 10));
    LS('ow:touch-ui', String(f));
    if (status.inworld) applyGameLayout();
    toast('Interface size ' + Math.round(f * 100) + '%', 1200);
  };
  root.querySelector('[data-s="smaller"]').addEventListener('click', () => { uiStep(-0.1); ctlLabels(); });
  root.querySelector('[data-s="bigger"]').addEventListener('click', () => { uiStep(0.1); ctlLabels(); });

  // Menu > Controls: the tools (tap test, debug info, Edit layout) and the comfort settings
  const ctl = (c) => root.querySelector(`[data-c="${c}"]`);
  const PRESET_SAY = { auto: 'Auto', phone: 'Phone', tablet: 'Tablet' };
  function ctlLabels() {
    ctl('preset').textContent = 'Layout: ' + PRESET_SAY[presetPref()] + (custom ? ' (edited)' : '');
    ctl('lefty').textContent = 'Left-handed: ' + (lefty() ? 'on' : 'off');
    ctl('haptics').textContent = 'Vibration: ' + (LS('ow:haptics') === 'off' ? 'off' : 'on');
    ctl('softlock').textContent = 'Soft lock: ' + (softLock() ? 'on' : 'off');
    ctl('hold').textContent = 'Touch and hold: ' + (holdRight() ? 'right-click' : 'tooltip');
    ctl('kbdhide').textContent = 'Keyboard hides touch: ' + (LS('ow:kbdhide') === 'off' ? 'off' : 'on');
    const f = Math.min(1.4, Math.max(0.7, +(LS('ow:touch-ui') || 1)));
    const slide = (c, v) => { ctl(c).value = String(Math.round(v * 100)); ctl(c).nextElementSibling.textContent = Math.round(v * 100) + '%'; };
    slide('uisize', f); slide('size', sizePref()); slide('alpha', alphaPref());
  }
  // Touch and hold (Menu > Controls): a tooltip (the hover, as before) or a right-click (use an item, talk, loot)
  const holdRight = () => LS('ow:hold') === 'right';
  ctlLabels();
  const onCtl = (c, fn) => ctl(c).addEventListener('click', (e) => { e.preventDefault(); try { fn(); } catch { status.errors += 1; } ctlLabels(); });
  onCtl('taptest', () => { openSheet(null); tapTest.start(); });
  onCtl('debug', () => copyDebug());
  onCtl('edit', () => {
    if (!status.inworld) { toast('Edit the layout once you are in the world', 2200); return; }
    openSheet(null); editMode(true);
  });
  onCtl('preset', () => {
    const order = ['auto', 'phone', 'tablet'];
    const next = order[(order.indexOf(presetPref()) + 1) % order.length];
    LS('ow:touch-preset', next === 'auto' ? null : next);
    if (custom) { custom = null; saveCustom(); }      // a preset is a fresh start for this character
    layout();
    toast('Layout: ' + PRESET_SAY[next] + (next === 'auto' ? ` (${PRESET_SAY[presetNow()]} on this screen)` : ''), 1800);
  });
  onCtl('lefty', () => { LS('ow:lefty', lefty() ? null : 'on'); layout(); toast(lefty() ? 'Left-handed: the stick on the right' : 'Right-handed: the stick on the left', 1800); });
  onCtl('haptics', () => { LS('ow:haptics', LS('ow:haptics') === 'off' ? null : 'off'); buzz(); });
  onCtl('softlock', () => { LS('ow:softlock', softLock() ? 'off' : null); toast(softLock() ? 'Soft lock: a tap on open ground keeps your target' : 'Soft lock off: a tap on open ground clears your target', 2600); });
  onCtl('hold', () => {
    LS('ow:hold', holdRight() ? null : 'right');
    toast(holdRight() ? 'Touch and hold: right-click (use items, talk, loot). Tooltips show while your finger is down' : 'Touch and hold: tooltip. Tap with two fingers to right-click', 3200);
  });
  onCtl('kbdhide', () => {
    LS('ow:kbdhide', LS('ow:kbdhide') === 'off' ? null : 'off');
    if (LS('ow:kbdhide') === 'off') setKbdMode(false);
    toast(LS('ow:kbdhide') === 'off' ? 'A keyboard leaves the touch controls on screen' : 'A keyboard hides the touch controls until you touch the screen', 2600);
  });
  // Button size and Opacity: live as the slider moves
  ctl('size').addEventListener('input', (e) => {
    const v = clampN((+e.target.value || 100) / 100, 0.8, 1.4);
    LS('ow:touch-size', v === 1 ? null : String(v));
    e.target.nextElementSibling.textContent = Math.round(v * 100) + '%';
    layout();
    status.sizeFits = !(v > 1 && !custom && status.layout.s < Math.round(PRESETS[presetNow()].scale * v * 100) / 100);
  });
  ctl('size').addEventListener('change', () => { if (!status.sizeFits) toast('As big as fits on this screen', 1800); else if (custom) toast('Your edited ring keeps its size: Edit layout > Ring − / +', 2600); });
  ctl('alpha').addEventListener('input', (e) => {
    const v = clampN((+e.target.value || 100) / 100, 0.3, 1);
    LS('ow:touch-alpha', v === 1 ? null : String(v));
    e.target.nextElementSibling.textContent = Math.round(v * 100) + '%';
    root.style.setProperty('--ow-alpha', String(v));
  });
  // Bigger interface: the game's own interface size, live as the slider moves (applied when it stops)
  let uiTimer = 0;
  ctl('uisize').addEventListener('input', (e) => {
    const f = Math.min(1.4, Math.max(0.7, (+e.target.value || 100) / 100));
    LS('ow:touch-ui', String(f));
    e.target.nextElementSibling.textContent = Math.round(f * 100) + '%';
    clearTimeout(uiTimer);
    uiTimer = setTimeout(() => { if (status.inworld) applyGameLayout(); }, 250);
  });
  // ---- Edit layout: drag the stick, the ring (by Attack, a key, the reticle or the dots), Jump and the tray ----------
  // While editing every press on the controls is a drag (nothing reaches the game or fires a key); the bar at the top
  // has Ring − / +, the presets, Reset, Done and the layout code. Saved per character as it goes.
  let editing = false, edrag = null;
  const ebar = $('.editbar'), ebIn = ebar.querySelector('input');
  const r3 = (v) => Math.round(v * 1000) / 1000;
  const snapshotCustom = () => {
    const W = innerWidth, H = innerHeight, f = ([x, y]) => [r3(Math.min(1, Math.max(0, x / W))), r3(Math.min(1, Math.max(0, y / H)))];
    return { s: f(CENTRES.s), r: f(CENTRES.r), j: custom?.j ? [...custom.j] : null, t: f(CENTRES.t), z: CENTRES.z };
  };
  function editMode(on) {
    editing = !!on;
    status.editing = editing;
    root.classList.toggle('editing', editing);
    edrag = null;
    if (editing) { info.classList.remove('on'); setTray(false); ebIn.value = ''; toast('Drag the controls where your thumbs want them', 2400); }
    else { saveCustom(); toast(custom ? 'Layout saved for this character' : 'Layout: ' + PRESET_SAY[presetNow()], 1600); }
    ctlLabels();
  }
  status.editMode = editMode;
  const groupOf = (el) => (el.closest('.stick') ? 's' : el.closest('.attack, .key, .target, .dots') ? 'r' : el.closest('.jump') ? 'j' : el.closest('.tray') ? 't' : null);
  root.addEventListener('pointerdown', (e) => {
    if (!editing || e.target.closest('.editbar')) return;
    e.preventDefault(); e.stopPropagation();
    const g = groupOf(e.target);
    if (!g || edrag) return;
    if (!custom) custom = snapshotCustom();
    edrag = { id: e.pointerId, g, x0: e.clientX, y0: e.clientY, c0: [...CENTRES[g]] };
  }, true);
  addEventListener('pointermove', (e) => {
    if (!editing || !edrag || e.pointerId !== edrag.id) return;
    e.preventDefault(); e.stopPropagation();
    const W = innerWidth, H = innerHeight, g = edrag.g;
    // the stick, the ring and Jump are kept right-handed (mirrored on screen when Left-handed is on); the tray never mirrors
    const dx = (e.clientX - edrag.x0) * (g !== 't' && lefty() ? -1 : 1), dy = e.clientY - edrag.y0;
    const x = edrag.c0[0] + dx, y = edrag.c0[1] + dy;
    custom[g] = [r3(Math.min(1, Math.max(0, x / W))), r3(Math.min(1, Math.max(0, y / H)))];
    layout();
  }, true);
  const edEnd = (e) => {
    if (!editing || !edrag || e.pointerId !== edrag.id) return;
    e.stopPropagation();
    edrag = null;
    custom = snapshotCustom();        // what is on screen, clamped, is what is kept
    if (custom.j) custom.j = [r3(CENTRES.j[0] / innerWidth), r3(CENTRES.j[1] / innerHeight)];
    saveCustom(); ctlLabels();
  };
  addEventListener('pointerup', edEnd, true);
  addEventListener('pointercancel', edEnd, true);
  const ed = (k, fn) => takeTap(ebar.querySelector(`[data-e="${k}"]`), () => { try { fn(); } catch { status.errors += 1; } ctlLabels(); });
  const ringSize = (d) => {
    if (!custom) custom = snapshotCustom();
    custom.z = Math.round(Math.min(ZMAX, Math.max(ZMIN, custom.z + d)) * 100) / 100;
    layout(); saveCustom();
    toast('Ring size ' + Math.round(custom.z * 100) + '%', 900);
  };
  ed('smaller', () => ringSize(-0.1));
  ed('bigger', () => ringSize(0.1));
  for (const p of ['phone', 'tablet']) ed(p, () => { LS('ow:touch-preset', p); custom = null; saveCustom(); layout(); toast('Layout: ' + PRESET_SAY[p], 1200); });
  ed('reset', () => { custom = null; saveCustom(); layout(); toast('Layout reset to ' + PRESET_SAY[presetNow()], 1400); });
  ed('done', () => editMode(false));
  ed('copy', () => {
    const code = encodeLayout(custom || snapshotCustom());
    ebIn.value = code;
    copyText(code).then((ok) => toast(ok ? 'Layout code copied: ' + code : 'Layout code: ' + code, 3000));
  });
  ebar.querySelector('.eb-code').addEventListener('submit', (e) => {
    e.preventDefault();
    const v = validCustom(decodeLayout(ebIn.value));
    if (!v) { toast('That is not a layout code (they start with L1)', 2200); return; }
    custom = v; saveCustom(); layout(); ctlLabels();
    ebIn.blur();
    toast('Layout code used', 1200);
  });
  status.layoutCode = () => encodeLayout(custom || snapshotCustom());
  status.useLayoutCode = (code) => { const v = validCustom(decodeLayout(code)); if (!v) return false; custom = v; saveCustom(); layout(); ctlLabels(); return true; };
  Object.defineProperty(status, 'layoutKey', { get: () => layoutKey, enumerable: true });
  // the character's own layout, once the game says who this is
  function loadCharLayout() {
    lua('return UnitName("player") or "", GetRealmName and GetRealmName() or ""').then((r) => {
      const nm = String(r?.[0] || ''), realm = String(r?.[1] || '');
      if (!nm) return;
      const k = 'ow:touch-layout:' + (realm ? realm + '/' : '') + nm;
      if (k === layoutKey) return;
      layoutKey = k; loadCustom(); layout(); ctlLabels();
    });
  }
  // the character screens' corner button: Menu > Controls there too (the tap test works before the world)
  takeTap($('.gear'), () => { markScheme(); openSheet('controls'); });

  // auto loot: 3.3.5a's own "Auto Loot" interface option (the autoLootDefault setting); the player's
  // choice from before touch is kept and put back when touch is turned off
  function applyAutoLoot(restore) {
    if (!wenilla.live) return;
    const want = LS('ow:autoloot') === 'off' ? '0' : '1';
    lua(restore
      ? 'if __OWTOUCH and __OWTOUCH.loot0 then SetCVar("autoLootDefault", __OWTOUCH.loot0) __OWTOUCH.loot0 = nil end'
      : `if not __OWTOUCH then __OWTOUCH = { hidden = {}, orig = {} } end
if not __OWTOUCH.loot0 then __OWTOUCH.loot0 = GetCVar("autoLootDefault") or "0" end
SetCVar("autoLootDefault", "${want}")`);
  }

  // Close: shown while any window is open (the Escape key's job). Without a bridge nothing says
  // which windows are open, so it shows for a while after the tray or a key opened one, and
  // presses Escape (which closes the top window first, as on PC).
  let closerTimer = 0;
  function showCloser() {
    if (wenilla.live) return;
    $('.closer').classList.add('on');
    clearTimeout(closerTimer);
    closerTimer = setTimeout(() => $('.closer').classList.remove('on'), 20000);
  }
  press($('.closer'), () => {
    if (wenilla.live) lua('CloseAllWindows() if CloseAllBags then CloseAllBags() end');
    else wenilla.fire('TOGGLEGAMEMENU');
    $('.closer').classList.remove('on');
  });

  // typing into the game's own text boxes (mail, auction search, names, guild notes…): when one
  // has the keyboard focus a text bar appears; tapping it brings up the phone's keyboard, and what
  // is typed is mirrored into the box; Enter presses the box's Enter
  const typebar = $('.typebar'), tin = typebar.querySelector('input');
  let typing = '';
  const luaStr = (t) => '[==[' + String(t).replace(/\]==\]/g, '') + ']==]';
  tin.addEventListener('input', () => { if (typing) lua(`local f = ${FOCUS} if f then f:SetText(${luaStr(tin.value)}) end`); });
  typebar.addEventListener('submit', (e) => {
    e.preventDefault();
    if (!typing) return;
    lua(`local f = ${FOCUS} if f then f:SetText(${luaStr(tin.value)}) local h = f:GetScript("OnEnterPressed") if h then local old = this this = f h(f) this = old end end`);
    tin.blur();
  });
  typebar.querySelector('.tb-x').addEventListener('click', () => {
    if (typing) lua(`local f = ${FOCUS} if f then local h = f:GetScript("OnEscapePressed") if h then local old = this this = f h(f) this = old else f:ClearFocus() end end`);
    tin.blur(); typebar.classList.remove('on'); typing = '';
  });
  function focusChanged(name, text) {
    // a chat frame's edit box (3.3.5a: ChatFrame1EditBox ...): the chat sheet instead
    if (/^ChatFrame\d+EditBox$/.test(name)) {
      lua(`local f = ${name} if f then if ChatEdit_DeactivateChat then ChatEdit_DeactivateChat(f) else f:ClearFocus() end f:Hide() end`);
      if (!sheets.chat.classList.contains('open')) openSheet('chat');   // (openSheet toggles)
      return;
    }
    if (name === typing) return;
    typing = name;
    typebar.classList.toggle('on', !!name);
    if (name) { tin.value = text || ''; tin.placeholder = 'Tap here to type'; } else tin.blur();
  }

  // stances / forms and the pet bar, as a short row along the bottom edge
  const strip = $('.strip');
  let stripKey = '';
  function renderStrip(ss, pet, leave = '') {
    const key = ss + '#' + pet + '#' + leave;
    if (key === stripKey) return;
    const sameShape = (stripKey.split('#')[2] || '') === leave && stripKey.split('#')[0].split(';').map((x) => x.split('~')[0]).join() === ss.split(';').map((x) => x.split('~')[0]).join()
      && stripKey.split('#')[1]?.split(';').map((x) => x.split('~').slice(0, 2).join()).join() === pet.split(';').map((x) => x.split('~').slice(0, 2).join()).join();
    stripKey = key;
    const items = [];
    // in a vehicle (or possessing): Leave first
    if (leave) items.push({ kind: 'x', n: leave === 'possess' ? 2 : 1, tex: leave === 'possess' ? 'Interface\\Icons\\Spell_Shadow_SacrificialShield' : 'Interface\\Vehicles\\UI-Vehicles-Button-Exit-Up' });
    if (leave && (ss || pet)) items.push({ kind: 'gap' });
    ss.split(';').filter(Boolean).forEach((x, i) => { const [tex, act, ok] = x.split('~'); items.push({ kind: 's', n: i + 1, tex, act: act === '1', off: ok !== '1' }); });
    const pets = pet.split(';').filter(Boolean).map((x) => { const [n, tex, act, auto] = x.split('~'); return { kind: 'p', n: +n, tex, act: act === '1', auto: auto === '1' }; });
    if (items.length && pets.length) items.push({ kind: 'gap' });
    items.push(...pets);
    if (!sameShape) {
      strip.replaceChildren(...items.map((it) => {
        const el = document.createElement('div');
        if (it.kind === 'gap') { el.className = 'gap'; return el; }
        el.className = 'ctl sbtn';
        el.innerHTML = '<div class="face"></div><div class="ring"></div>';
        el.dataset.kind = it.kind; el.dataset.n = it.n;
        if (it.tex) iconUrl(it.tex).then((u) => { if (u) el.querySelector('.face').style.backgroundImage = `url(${u})`; });
        press(el, () => lua(it.kind === 'x' ? (it.n === 2 ? 'if PossessButton2 then PossessButton2:Click() end' : 'VehicleExit()')
          : it.kind === 's' ? `CastShapeshiftForm(${it.n})` : `CastPetAction(${it.n})`));
        if (it.kind === 'x') el.setAttribute('aria-label', 'Leave');
        if (it.kind === 'p') longPress(el, () => { lua(`TogglePetAutocast(${it.n})`); toast('Autocast toggled', 1000); });
        return el;
      }));
    }
    const els = [...strip.querySelectorAll('.sbtn')];
    items.filter((it) => it.kind !== 'gap').forEach((it, i) => {
      const el = els[i];
      if (!el) return;
      el.classList.toggle('active', it.act);
      el.classList.toggle('auto', !!it.auto);
      el.classList.toggle('off', !!it.off);
    });
    layout();
  }

  // the character-creation name (and any glue-screen text box): the keyboard button relays typed
  // keys to the game the way a hardware keyboard would
  const ghost = $('.ghost');
  const keyTo = (key, code) => {
    const c = gameCanvas();
    if (!c) return;
    for (const type of ['keydown', 'keypress', 'keyup']) {
      if (type === 'keypress' && key.length !== 1) continue;
      c.dispatchEvent(new KeyboardEvent(type, { key, code, bubbles: true, cancelable: true }));
    }
  };
  // The box keeps what the keyboard typed (padded with ZWSP so Backspace always has something to delete) and each change
  // goes to the game as its difference (ghostDiff): a keyboard that composes words rewrites the whole word at each
  // letter and its events can't be cancelled, so reading them one by one sent every letter again ("BBoBob", 5 Nov).
  // Autocorrect swapping a word is a few Backspaces and the new letters, as a person would type it.
  // The button focuses the box when the finger LIFTS: a phone raises its keyboard only for a focus made in a user
  // gesture, and a finger's press is not one (the keyboard never came up on Android). A second tap puts it away.
  const kecho = $('.kecho');
  const PAD = '​'.repeat(8);
  let gPrev = PAD, gTyped = '';
  const keyCode = (ch) => (/[a-z]/i.test(ch) ? 'Key' + ch.toUpperCase() : /\d/.test(ch) ? 'Digit' + ch : ch === ' ' ? 'Space' : '');
  const ghostReset = () => { ghost.value = PAD; gPrev = PAD; try { ghost.setSelectionRange(PAD.length, PAD.length); } catch { /* fine */ } };
  pressLook($('.kbd'));
  takeTap($('.kbd'), () => {
    if (document.activeElement === ghost) { ghost.blur(); return; }
    ghostReset(); gTyped = ''; kecho.textContent = '';
    try { ghost.focus({ preventScroll: true }); } catch { /* fine */ }
  });
  ghost.addEventListener('focus', () => root.classList.add('ghosting'));
  ghost.addEventListener('blur', () => root.classList.remove('ghosting'));
  ghost.addEventListener('input', (e) => {
    const v = ghost.value;
    const { back, text } = ghostDiff(gPrev, v);
    for (let k = 0; k < back; k++) { keyTo('Backspace', 'Backspace'); gTyped = gTyped.slice(0, -1); }
    for (const ch of text) {
      if (ch === '\n') { keyTo('Enter', 'Enter'); continue; }
      keyTo(ch, keyCode(ch)); gTyped += ch;
    }
    gPrev = v;
    kecho.textContent = gTyped.slice(-24);
    status.typed = (status.typed || 0) + back + text.length;
    // padding nearly used up, or a long run: a fresh box (never mid-word, which would break the keyboard's word)
    if (!e.isComposing && (!v.startsWith(PAD.slice(0, 3)) || v.length > 64)) ghostReset();
  });
  ghost.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); keyTo('Enter', 'Enter'); ghost.blur(); } });

  root.querySelector('[data-x="full"]').addEventListener('click', () => { fullscreen(); openSheet(null); });
  root.querySelector('[data-x="site"]').addEventListener('click', () => { openSheet(null); document.getElementById('ow-dock-handle')?.click(); });
  root.querySelector('[data-x="off"]').addEventListener('click', () => applyScheme('pc', true));

  // chat: the game's chat lines, and a box that sends like the chat box (/commands too)
  const log = $('.log');
  const COLORS = { SAY: '#fff', YELL: '#ff4040', WHISPER: '#ff80ff', PARTY: '#aaaaff', GUILD: '#40ff40', OFFICER: '#40c040', RAID: '#ff7f00', CHANNEL: '#ffc0c0', SYSTEM: '#ffff00', EMOTE: '#ff8040', MONSTER_SAY: '#ffffa0' };
  const clean = (s) => String(s || '').replace(/\|c[0-9a-fA-F]{8}/g, '').replace(/\|r/g, '').replace(/\|H[^|]*\|h(\[[^\]]*\])\|h/g, '$1');
  wenilla.on?.('chat', (p) => {
    try {
      const kind = String(p?.kind || '').toUpperCase();
      const who = p?.sender ? `[${clean(p.sender)}]: ` : '';
      const ch = p?.channel ? `[${clean(p.channel)}] ` : '';
      const line = document.createElement('div');
      line.style.color = COLORS[kind] || COLORS[kind.split('_')[0]] || '#e8dcc0';
      line.innerHTML = esc(ch + who + clean(p?.text));
      log.append(line);
      while (log.children.length > 80) log.firstChild.remove();
      log.scrollTop = log.scrollHeight;
      // the chat frames are hidden: anything addressed to you lights the tray, whispers pop up
      if (!sheets.chat.classList.contains('open') && /^(WHISPER|PARTY|PARTY_LEADER|RAID|RAID_LEADER|RAID_WARNING|GUILD|OFFICER)$/.test(kind)) {
        $$('.badge').forEach((b) => b.classList.add('on'));
        if (kind === 'WHISPER') toast(`${clean(p.sender)} whispers: ${clean(p.text)}`.slice(0, 140), 4500);
      }
    } catch { /* one bad line */ }
  });
  sheets.chat.querySelector('form').addEventListener('submit', (e) => {
    e.preventDefault();
    const inp = sheets.chat.querySelector('input');
    const text = inp.value.trim();
    if (text) wenilla.chat(text);
    inp.value = '';
  });

  // ---- character / realm screens: a finger works the glue screens like a mouse --------------------
  // Raw touches on the canvas don't register as clicks on these screens, so they are caught here and
  // replayed as mouse input through mouse(). On a phone the glue buttons are about 10 px tall, smaller
  // than a fingertip (and a finger's contact point lands a few px off where its owner aims), so:
  //   * snap: the red glue buttons (Enter World, Create New Character, Accept, Back, Delete, AddOns,
  //     Change Realm, Randomize...) are found on screen by their painted face, read back from the
  //     game's canvas; a tap anywhere near one clicks its exact centre;
  //   * aim: touch and hold shows a loupe above the finger, magnifying what is under it round a
  //     crosshair; slide onto anything (race and class icons, the little arrows, a name in the list)
  //     and lift to click it;
  //   * a quick swipe still drags (sliders, the character list, turning the model);
  //   * every click flashes a small ring where it went (green: snapped to a button).
  // ?touchdebug=1 adds a line of numbers (pixel ratio, buffer size, factor, buttons found, last tap).
  const glue = $('.glue'), loupe = $('.loupe'), ring = $('.tapring'), tdebug = $('.tdebug');
  const lctx = loupe.querySelector('canvas').getContext('2d');
  const lbuf = document.createElement('canvas');   // drawn here first: a blank read never wipes the loupe
  lbuf.width = lbuf.height = 264;
  const lbx = lbuf.getContext('2d', { willReadFrequently: true });
  if (/[?&]touchdebug=1/.test(location.search)) root.classList.add('debug');
  let targets = [], targetsAt = 0, glueLast = '', glueNow = '';
  const rb = document.createElement('canvas');
  const rbx = rb.getContext('2d', { willReadFrequently: true });
  // read inside an animation frame (the only time the game's canvas can be read back), every 0.8 s
  // while a glue screen is up and at each touch
  let scanQueued = false;
  const scanSoon = () => new Promise((done) => {
    if (scanQueued) { setTimeout(done, 40); return; }
    scanQueued = true;
    requestAnimationFrame(() => { scanQueued = false; findButtons(); done(); });
  });
  setInterval(() => { if (status.on && root.classList.contains('glue-touch') && !document.hidden) scanSoon(); }, 800);
  function findButtons() {
    const c = gameCanvas();
    targetsAt = performance.now();
    if (!c || !c.width || !c.clientWidth) { targets = []; return; }
    try {
      const W = c.width, H = c.height, k = W > 1400 ? 0.5 : 1;   // half resolution on big buffers: plenty, 4x cheaper
      const w = Math.round(W * k), h = Math.round(H * k);
      if (rb.width !== w || rb.height !== h) { rb.width = w; rb.height = h; }
      rbx.drawImage(c, 0, 0, w, h);
      const d = rbx.getImageData(0, 0, w, h).data, n = w * h;
      // a WebGPU canvas reads back blank outside the frame it was drawn in: keep what we had
      let lit = 0;
      for (let i = 0; i < d.length && !lit; i += 4096) lit = d[i] | d[i + 1] | d[i + 2];
      if (!lit) return;
      const on = new Uint8Array(n);
      for (let i = 0, p = 0; p < n; p++, i += 4) {
        const r = d[i], g = d[i + 1], b = d[i + 2];
        if (r >= 45 && r <= 170 && g < r * 0.55 && b < r * 0.55 && Math.abs(g - b) < 30) on[p] = 1;   // the buttons' dark red
      }
      const seen = new Uint8Array(n), stack = [], found = [];
      const box = c.getBoundingClientRect(), sx = c.clientWidth / W / k, sy = c.clientHeight / H / k;
      for (let p = 0; p < n; p++) {
        if (!on[p] || seen[p]) continue;
        let x0 = w, y0 = h, x1 = -1, y1 = -1, cnt = 0;
        seen[p] = 1; stack.push(p);
        while (stack.length) {
          const q = stack.pop(), x = q % w, y = (q - x) / w;
          cnt++;
          if (x < x0) x0 = x; if (x > x1) x1 = x; if (y < y0) y0 = y; if (y > y1) y1 = y;
          if (x > 0 && on[q - 1] && !seen[q - 1]) { seen[q - 1] = 1; stack.push(q - 1); }
          if (x < w - 1 && on[q + 1] && !seen[q + 1]) { seen[q + 1] = 1; stack.push(q + 1); }
          if (y > 0 && on[q - w] && !seen[q - w]) { seen[q - w] = 1; stack.push(q - w); }
          if (y < h - 1 && on[q + w] && !seen[q + w]) { seen[q + w] = 1; stack.push(q + w); }
        }
        const tall = (y1 - y0 + 1) * sy;
        if (cnt >= 4 && tall >= 3 && tall <= 48) found.push({ x0, y0, x1, y1, cnt });
      }
      if (found.length > 300) { found.sort((p, q) => q.cnt - p.cnt); found.length = 300; }   // a red-heavy scene: the biggest pieces
      // on a small screen a button's lettering cuts its red face into pieces: join pieces that sit
      // side by side on the same line
      for (let joined = true; joined;) {
        joined = false;
        for (let i = 0; i < found.length && !joined; i++) {
          for (let j = i + 1; j < found.length; j++) {
            const a = found[i], b = found[j];
            const ha = a.y1 - a.y0 + 1, hb = b.y1 - b.y0 + 1, lo = Math.min(ha, hb);
            const overlap = Math.min(a.y1, b.y1) - Math.max(a.y0, b.y0) + 1;
            const gap = Math.max(a.x0, b.x0) - Math.min(a.x1, b.x1) - 1;
            const hi = Math.max(ha, hb), stubs = Math.min(a.x1 - a.x0, b.x1 - b.x0) + 1 < 4 * hi;
            // close pieces join; short stubs (a small button's two ends round its word) join across more
            if (overlap >= lo * 0.6 && gap <= (stubs ? Math.max(6, 2.6 * hi) : Math.max(4, 1.2 * hi))) {
              found[i] = { x0: Math.min(a.x0, b.x0), y0: Math.min(a.y0, b.y0), x1: Math.max(a.x1, b.x1), y1: Math.max(a.y1, b.y1), cnt: a.cnt + b.cnt };
              found.splice(j, 1);
              joined = true;
              break;
            }
          }
        }
      }
      // a glue button: wide, flat and mostly that red (the character's red clothes are none of these)
      targets = found.filter((t) => {
        const bw = t.x1 - t.x0 + 1, bh = t.y1 - t.y0 + 1;
        return bh * sy >= 4 && bw * sx >= 30 && bw >= bh * 2.5 && t.cnt >= bw * bh * 0.35;
      }).map((t) => ({ x: box.left + t.x0 * sx, y: box.top + t.y0 * sy, w: (t.x1 - t.x0 + 1) * sx, h: (t.y1 - t.y0 + 1) * sy }));
    } catch { targets = []; }   // a canvas that can't be read back: plain taps, no snapping
  }
  // the button a point means: inside one, or within a finger's width of its edge (nearest wins)
  // Character creation never snaps (owner 3 Nov 01:42: "it fails to hit buttons on creating character, all of the boxes
  // are misaligned"): Forever UI's creation screen is full of red art (frames, race and class boxes, sliders) that the
  // scan takes for buttons, so a tap went to a neighbour's centre. There a tap clicks where the finger is (the ring shows
  // it; touch and hold for the loupe). Elsewhere a tap snaps only straight above or below a button and close to it.
  function snap(x, y) {
    if (glueNow === 'charcreate') return null;
    let best = null, bd = Infinity;
    for (const t of targets) {
      // fingers land low more than sideways: a zone above and below, none past the ends
      const padY = Math.max(10, t.h * 0.8), padX = 2;
      const dx = Math.max(t.x - x, 0, x - (t.x + t.w)), dy = Math.max(t.y - y, 0, y - (t.y + t.h));
      if (dx > padX || dy > padY) continue;
      const dd = Math.hypot(dx, dy * 1.4);
      if (dd < bd) { bd = dd; best = t; }
    }
    return best;
  }
  status.snapAt = (x, y, ts) => { if (ts) targets = ts; return snap(x, y); };
  // The button magnet (5 Nov, "real phones still often don't press game buttons on character screens even though the tap
  // bubble is in the right place"): a tap that snap() leaves alone but that lands within MAGNET_PX of a scanned button's
  // edge, where the game says nothing is under the cursor (GetMouseFocus() is empty or the screen's own backdrop), is a
  // tap that would click nothing: it goes to the button's centre instead. Only with the bridge (the game must say "no
  // hover"); on character creation never sideways: only a finger already over the button's width, and only its height
  // moves (x stays where the finger is).
  const MAGNET_PX = 10;
  function magnet(x, y) {
    let best = null, bd = Infinity;
    for (const t of targets) {
      const dx = Math.max(t.x - x, 0, x - (t.x + t.w)), dy = Math.max(t.y - y, 0, y - (t.y + t.h));
      if (glueNow === 'charcreate' && dx > 0) continue;
      const d = Math.hypot(dx, dy);
      if (d <= MAGNET_PX && d < bd) { bd = d; best = t; }
    }
    if (!best) return null;
    return { t: best, x: glueNow === 'charcreate' ? x : best.x + best.w / 2, y: best.y + best.h / 2 };
  }
  status.magnetAt = (x, y, ts) => { if (ts) targets = ts; const m = magnet(x, y); return m && { x: m.x, y: m.y }; };
  // what the game has under its cursor: a frame's name, '' for nothing, null for no answer
  const NO_HOVER = /^(|GlueParent|AccountLogin|CharacterSelect|CharacterCreate|RealmList|WorldFrame|UIParent)$/;
  const FOCUS_LUA = 'local f = GetMouseFocus() return f and f.GetName and (f:GetName() or "?") or ""';
  async function hoverName() {
    if (!wenilla.live) return null;
    try { const r = await wenilla.lua(FOCUS_LUA, 1500); return r == null ? null : String(r[0] ?? ''); } catch { return null; }
  }
  function flash(x, y, snapped) {
    ring.style.left = x + 'px'; ring.style.top = y + 'px';
    ring.classList.toggle('snap', snapped);
    ring.classList.remove('go'); void ring.offsetWidth; ring.classList.add('go');
  }
  function debugLine(extra) {
    if (!root.classList.contains('debug')) return;
    const c = gameCanvas(), [fx, fy] = pointerFactor(c);
    tdebug.textContent = `touch ${TOUCH_BUILD} · ${wenilla.session() || '?'} · ${wenilla.live ? 'bridge' : 'keys'}${document.fullscreenElement ? ' · full screen' : ''}\n` +
      `dpr ${devicePixelRatio} buffer ${c?.width}x${c?.height} css ${c?.clientWidth}x${c?.clientHeight}\n` +
      `factor ${fx.toFixed(2)},${fy.toFixed(2)} buttons ${targets.length} taps ${status.tapCount} mode ${TAPMODE}${'onpointerrawupdate' in window ? ' raw' : ''}\n` +
      `${extra || glueLast}\ngame took:${tapTrace.last || ' -'}  errors ${tapTrace.errors}${tapTrace.error ? ': ' + tapTrace.error : ''}\n` +
      `last: ${tapLog.slice(-3).map(tapShort).join(' | ') || '-'}  frame ${frameMs()}ms resent ${TAPTIME.resent}`;
  }
  // The one line Menu > Controls > Copy debug info puts on the clipboard, for the player to paste to us
  function debugReport() {
    const c = gameCanvas(), [fx, fy] = pointerFactor(c);
    const ua = navigator.userAgent.replace(/^Mozilla\/5\.0 /, '').replace(/\(KHTML, like Gecko\) /, '').slice(0, 120);
    const tt = status.tapTest ? ` | tap test ${status.tapTest.summary}` : '';
    return `Leyline touch ${TOUCH_BUILD} | ${wenilla.session() || '?'} ${wenilla.live ? 'bridge' : 'keys'} | ` +
      `dpr ${devicePixelRatio} buffer ${c?.width}x${c?.height} css ${c?.clientWidth}x${c?.clientHeight} page ${innerWidth}x${innerHeight}` +
      `${fsEl() ? ' fullscreen' : ''}${installed() ? ' app' : ''} | factor ${fx.toFixed(2)},${fy.toFixed(2)} mode ${TAPMODE} | ` +
      `buttons ${targets.length} taps ${status.tapCount} dropped ${status.dropped || 0} resent ${TAPTIME.resent} frame ${frameMs()}ms | ` +
      `layout ${status.layout?.preset || '?'}${status.layout?.custom ? ' edited' : ''}${status.layout?.mirror ? ' left-handed' : ''}` +
      ` size ${Math.round(sizePref() * 100)} alpha ${Math.round(alphaPref() * 100)} hold ${holdRight() ? 'right' : 'tip'}${kbdMode ? ' kbdmode' : ''}${root.classList.contains('padmode') ? ' padmode' : ''} | ` +
      `safe ${(status.layout?.safe || [0, 0, 0, 0]).join(',')} vv ${window.visualViewport ? Math.round(visualViewport.width) + 'x' + Math.round(visualViewport.height) : '-'} kb ${status.keyboard || 0}` +
      ` ${screen.orientation?.type || (innerWidth > innerHeight ? 'landscape' : 'portrait')} haptics ${HAPTIC} two ${status.twoTaps || 0} stale ${status.stale || 0} letgo ${status.letGo || 0} | ` +
      `last ${tapLog.map(tapShort).join('; ') || '-'} | game took${tapTrace.last || ' -'} errors ${tapTrace.errors + status.errors}` +
      `${tapTrace.error ? ' ' + tapTrace.error : ''}${tt} | ${ua}`;
  }
  status.debugReport = debugReport;
  async function copyText(text) {
    try { await navigator.clipboard.writeText(text); return true; } catch { /* no clipboard: the old way */ }
    try {
      const ta = document.createElement('textarea');
      ta.value = text; ta.style.cssText = 'position:fixed;left:-9999px;top:0';
      document.body.append(ta); ta.select();
      const ok = document.execCommand('copy');
      ta.remove();
      return ok;
    } catch { return false; }
  }
  async function copyDebug() {
    const line = debugReport();
    status.copied = line;
    if (await copyText(line)) { toast('Debug info copied: paste it to us in Discord', 3500); return; }
    // nothing would copy: show it, selectable, so it can be copied by hand
    card(`<b>Debug info</b>\nTouch and hold the text to copy it, then paste it to us.\n<textarea class="dbg" readonly>${esc(line)}</textarea>`, [['Close']]);
  }
  function glueTap(x, y, rescan, rec = { how: 'tap', x, y }) {
    return inTurn(async () => {
      if (rescan) await scanSoon();                    // the page changed size: find the buttons where they are now
      const t = snap(x, y);
      let px = t ? t.x + t.w / 2 : x, py = t ? t.y + t.h / 2 : y;
      if (t) rec.how = 'snap';
      else if (wenilla.live && TAPMODE !== 'touchid') {
        const m = magnet(x, y);
        if (m) {
          moveTo(x, y);
          await settle();
          const f = await hoverName();
          rec.hover = f;
          if (f !== null && NO_HOVER.test(f)) { px = m.x; py = m.y; rec.how = 'magnet'; }
        }
      }
      rec.screen = glueNow; rec.px = px; rec.py = py;
      flash(px, py, rec.how !== 'tap');
      status.tapCount += 1;
      noteTap(rec);
      glueLast = `tap ${Math.round(x)},${Math.round(y)} -> ${Math.round(px)},${Math.round(py)}${rec.how !== 'tap' ? ' ' + (rec.how === 'snap' ? 'snapped' : rec.how) : ''}`;
      debugLine();
      await clickNow(px, py, 0, rec);
      debugLine();
    });
  }
  // the loupe: 2.5x of the game round the aim point, crosshair, the button it would snap to outlined
  const ZOOM = 2.5, LR = 66;
  let aimRaf = 0;
  function drawLoupe() {
    aimRaf = 0;
    if (!gl || gl.mode !== 'aim') return;
    const c = gameCanvas();
    const x = gl.x, y = gl.y;
    let ly = y - LR - 84;
    if (ly < 4) ly = y + 52;
    loupe.style.left = Math.min(innerWidth - 2 * LR - 4, Math.max(4, x - LR)) + 'px';
    loupe.style.top = ly + 'px';
    try {
      const box = c.getBoundingClientRect(), kx = c.width / c.clientWidth, ky = c.height / c.clientHeight;
      const half = LR / ZOOM;
      lbx.fillStyle = '#000'; lbx.fillRect(0, 0, 264, 264);
      lbx.drawImage(c, (x - box.left - half) * kx, (y - box.top - half) * ky, 2 * half * kx, 2 * half * ky, 0, 0, 264, 264);
      const px = lbx.getImageData(0, 0, 264, 264).data;
      let lit = 0;
      for (let i = 0; i < px.length && !lit; i += 1024) lit = px[i] | px[i + 1] | px[i + 2];
      if (lit) lctx.drawImage(lbuf, 0, 0);
      const t = snap(x, y), toL = (v, o) => (v - o + half) * (264 / (2 * half));
      if (t) {
        lctx.strokeStyle = '#7dff9a'; lctx.lineWidth = 4;
        lctx.strokeRect(toL(t.x, x), toL(t.y, y), t.w * 264 / (2 * half), t.h * 264 / (2 * half));
      }
      lctx.strokeStyle = 'rgba(0,0,0,.8)'; lctx.lineWidth = 5;
      lctx.beginPath(); lctx.moveTo(132, 104); lctx.lineTo(132, 160); lctx.moveTo(104, 132); lctx.lineTo(160, 132); lctx.stroke();
      lctx.strokeStyle = '#ffd76a'; lctx.lineWidth = 2;
      lctx.beginPath(); lctx.moveTo(132, 104); lctx.lineTo(132, 160); lctx.moveTo(104, 132); lctx.lineTo(160, 132); lctx.stroke();
    } catch { /* not readable: the crosshair alone */ }
    // asked for from a task, not from inside this frame: the next frame's callback then runs after
    // the game has drawn (one asked for from inside a frame runs first and reads a blank canvas)
    aimRaf = setTimeout(() => { aimRaf = requestAnimationFrame(drawLoupe); }, 0);
  }
  let gl = null, aimTimer = 0;
  glue.addEventListener('pointerdown', (e) => {
    // one finger at a time; a finger whose lift never arrived (the browser lost it) stops counting
    // after a few seconds instead of blocking every tap after it
    if (gl && gl.id !== e.pointerId && performance.now() - gl.t0 < 4000) return;
    e.preventDefault();
    try { glue.setPointerCapture(e.pointerId); } catch { /* fine */ }
    gl = { id: e.pointerId, x0: e.clientX, y0: e.clientY, x: e.clientX, y: e.clientY, t0: performance.now(), mode: 'tap', w: innerWidth, h: innerHeight };
    if (TAPMODE === 'touchid') {                       // press now, as this finger's own pointer
      const t = snap(e.clientX, e.clientY);
      gl.mode = 'held'; gl.px = t ? t.x + t.w / 2 : e.clientX; gl.py = t ? t.y + t.h / 2 : e.clientY; gl.snapped = !!t;
      tapPid = e.pointerId;
      moveTo(gl.px, gl.py);
      mouse('pointerdown', gl.px, gl.py, 0, 1);
      return;
    }
    scanSoon();
    const hx = e.clientX, hy = e.clientY;
    inTurn(() => moveTo(hx, hy));                      // hover first: buttons light up, tooltips show
    clearTimeout(aimTimer);
    aimTimer = setTimeout(() => {                      // held still: aim with the loupe
      if (!gl || gl.mode !== 'tap') return;
      gl.mode = 'aim'; loupe.classList.add('on'); buzz();
      if (!aimRaf) aimRaf = requestAnimationFrame(drawLoupe);
    }, 260);
  });
  glue.addEventListener('pointermove', (e) => {
    if (!gl || gl.id !== e.pointerId || gl.mode === 'held') return;
    gl.x = e.clientX; gl.y = e.clientY;
    if (gl.mode === 'aim') { moveTo(gl.x, gl.y); return; }
    if (gl.mode === 'tap' && Math.hypot(gl.x - gl.x0, gl.y - gl.y0) > TAP_SLOP) {
      clearTimeout(aimTimer);
      gl.mode = 'drag';                                // a swipe: sliders, the character list, turning the model
      mouse('pointerdown', gl.x0, gl.y0, 0, 1);
    }
    if (gl.mode === 'drag') moveTo(gl.x, gl.y, 1);
  });
  const glueEnd = (e) => {
    if (!gl || gl.id !== e.pointerId) return;
    clearTimeout(aimTimer);
    const g = gl;
    gl = null;
    loupe.classList.remove('on');
    const finger = Math.round(performance.now() - g.t0);
    if (g.mode === 'held') {                           // touchid: release while the finger is still this pointer
      mouse('pointerup', g.px, g.py, 0, 0);
      tapPid = 1;
      flash(g.px, g.py, g.snapped);
      status.tapCount += 1;
      noteTap({ how: g.snapped ? 'snap' : 'tap', screen: glueNow, x: g.x0, y: g.y0, px: g.px, py: g.py, finger, mode: 'touchid' });
      glueLast = `tap ${Math.round(g.x0)},${Math.round(g.y0)} -> ${Math.round(g.px)},${Math.round(g.py)}${g.snapped ? ' snapped' : ''}`;
      debugLine();
      return;
    }
    if (g.mode === 'drag') {
      // a rolled tap that just crossed the slop: let go where it pressed, so the button it landed on clicks
      if (e.type === 'pointerup' && sloppyTap(g, g.x, g.y)) {
        moveTo(g.x0, g.y0, 1); mouse('pointerup', g.x0, g.y0, 0, 0); status.tapCount += 1;
        noteTap({ how: 'drag', screen: glueNow, x: g.x0, y: g.y0, px: g.x0, py: g.y0, finger });
      } else mouse('pointerup', g.x, g.y, 0, 0);
      return;
    }
    // A tap is dropped only when it can't mean what the finger touched: a turn to the other
    // orientation, or the page changing size by a lot (going full screen). Android Chrome changes
    // innerHeight by a few px when its address bar or the visual viewport moves, and a
    // pointercancel can end a plain tap there: both used to drop the tap silently, so the button
    // lit up under the finger and never clicked ("highlights, no action", 2026-10-24).
    const quickCancel = e.type === 'pointercancel' && g.mode === 'tap' && performance.now() - g.t0 < LONG_MS
      && Math.hypot(g.x - g.x0, g.y - g.y0) <= TAP_SLOP;
    const turned = (g.w > g.h) !== (innerWidth > innerHeight);
    const resized = Math.abs(innerWidth - g.w) > g.w * 0.2 || Math.abs(innerHeight - g.h) > g.h * 0.2;
    if ((e.type !== 'pointerup' && !quickCancel) || turned || resized) {
      status.dropped = (status.dropped || 0) + 1;
      glueLast = `dropped: ${e.type !== 'pointerup' && !quickCancel ? e.type : turned ? 'turned' : `resized ${g.w}x${g.h}->${innerWidth}x${innerHeight}`}`;
      noteTap({ how: 'dropped', why: glueLast.slice(9), screen: glueNow, x: g.x0, y: g.y0, finger });
      debugLine();
      return;
    }
    // a small resize (address bar): the same spot of the page, scaled to its new size
    const kx = innerWidth / g.w, ky = innerHeight / g.h;
    // a tap goes where the finger landed (not where it rolled to); an aimed one where the loupe aimed
    if (g.mode === 'aim') glueTap(g.x * kx, g.y * ky, false, { how: 'aim', x: g.x * kx, y: g.y * ky, finger });
    else glueTap(g.x0 * kx, g.y0 * ky, kx !== 1 || ky !== 1, { how: 'tap', x: g.x0 * kx, y: g.y0 * ky, finger });
  };
  glue.addEventListener('pointerup', glueEnd);
  glue.addEventListener('pointercancel', glueEnd);
  status.findButtons = async () => { await scanSoon(); return targets; };

  // ---- the tap test (Menu > Controls > Tap test; ?taptest=1 opens it) ------------------------------
  // Five gold rings, one at a time, over everything; the player taps the centre of each. Each tap says how far the
  // finger landed from the ring (a hit within 22 px) and, with the bridge, where the game's own cursor went for that
  // spot (GetCursorPosition, as a fraction of its screen) against where the finger was: that difference is the
  // pointer factor's error, the thing that makes a tap land beside a button. Nothing is clicked in the game, so it
  // works on the character screens too (the corner Controls button there).
  const CURSOR_LUA = 'local x, y = GetCursorPosition() local P = UIParent or GlueParent if not (x and P) then return "" end local s = P:GetEffectiveScale() return x / (P:GetWidth() * s), y / (P:GetHeight() * s)';
  const tapTest = (() => {
    const el = $('.taptest'), dot = el.querySelector('.tt-dot'), msg = el.querySelector('.tt-msg');
    const SPOTS = [[0.1, 0.16], [0.9, 0.16], [0.5, 0.5], [0.1, 0.84], [0.9, 0.84]];
    const HIT = 22;
    let i = 0, res = [], downId = null;
    const spot = (k) => { const m = 34; return [Math.round(m + SPOTS[k][0] * (innerWidth - 2 * m)), Math.round(m + SPOTS[k][1] * (innerHeight - 2 * m))]; };
    function show() {
      const [x, y] = spot(i);
      dot.style.left = x + 'px'; dot.style.top = y + 'px';
      dot.classList.remove('done');
      msg.style.top = i === 2 ? '28%' : '50%';
      msg.textContent = `Tap test: tap the centre of the gold ring (${i + 1} of ${SPOTS.length})`;
    }
    // where the game's cursor lands for a page spot, as [dx, dy] css px from it (null: no answer)
    function gameOff(x, y) {
      if (!wenilla.live) return Promise.resolve(null);
      return inTurn(async () => {
        moveTo(x, y);
        await settle();
        const r = await lua(CURSOR_LUA);
        const fx = Number(r?.[0]), fy = Number(r?.[1]);
        if (r?.[0] === '' || r?.[0] == null || !Number.isFinite(fx) || !Number.isFinite(fy)) return null;
        const b = gameCanvas()?.getBoundingClientRect();
        if (!b) return null;
        return [Math.round(b.left + fx * b.width - x), Math.round(b.bottom - fy * b.height - y)];
      });
    }
    const avg = (a) => (a.length ? Math.round(a.reduce((p, q) => p + q, 0) / a.length) : 0);
    function finish() {
      const c = gameCanvas(), [fx, fy] = pointerFactor(c);
      const hits = res.filter((r) => r.hit).length;
      const mx = avg(res.map((r) => r.x - r.tx)), my = avg(res.map((r) => r.y - r.ty)), dist = avg(res.map((r) => r.d));
      const g = res.filter((r) => r.game);
      const game = g.length ? [avg(g.map((r) => r.game[0])), avg(g.map((r) => r.game[1]))] : null;
      const dir = (v, a, b) => (v ? `${Math.abs(v)} ${v > 0 ? a : b}` : '');
      const fdir = [dir(mx, 'right', 'left'), dir(my, 'down', 'up')].filter(Boolean).join(', ');
      const gd = game ? Math.round(Math.hypot(game[0], game[1])) : null;
      const summary = `${hits}/${res.length} hits, finger off ${dist}px (x ${mx} y ${my}), factor ${fx.toFixed(2)},${fy.toFixed(2)}, ` +
        (game ? `game cursor off x ${game[0]} y ${game[1]}` : 'game cursor not checked (no bridge)');
      status.tapTest = { hits, n: res.length, dist, finger: [mx, my], game, factor: [+fx.toFixed(2), +fy.toFixed(2)], taps: res, summary };
      msg.style.top = '42%';
      msg.textContent = `${hits} of ${res.length} hit\nYour taps land ${dist} px from where you aim${fdir ? ` (${fdir})` : ''}\n` +
        (game ? (gd > 8 ? `The game puts a tap ${gd} px away from your finger: that is why buttons miss. Copy the result and send it to us.`
          : `The game puts a tap where your finger is (${gd} px off)`) : 'The game\'s side is checked once the game answers (in the world)') +
        `\nFactor ${fx.toFixed(2)}, ${fy.toFixed(2)}`;
      dot.classList.add('done');
      el.classList.add('end');
    }
    el.addEventListener('pointerdown', (e) => {
      if (e.target.closest('button')) return;
      e.preventDefault(); e.stopPropagation();
      downId = e.pointerId;
    });
    el.addEventListener('pointerup', (e) => {
      if (e.target.closest('button')) return;
      e.preventDefault(); e.stopPropagation();
      if (downId !== e.pointerId || el.classList.contains('end') || i >= SPOTS.length) return;
      downId = null;
      const [tx, ty] = spot(i), x = Math.round(e.clientX), y = Math.round(e.clientY);
      const d = Math.round(Math.hypot(x - tx, y - ty));
      const r = { tx, ty, x, y, d, hit: d <= HIT, game: null };
      res.push(r);
      flash(x, y, r.hit);
      buzz();
      i += 1;
      const last = i >= SPOTS.length;
      if (!last) show(); else { dot.classList.add('done'); msg.textContent = 'Checking…'; }
      gameOff(x, y).then((g) => { r.game = g; if (last) finish(); });
    });
    el.addEventListener('pointercancel', () => { downId = null; });
    const api = {
      start() { i = 0; res = []; el.classList.remove('end'); el.classList.add('on'); status.tapTestOpen = true; show(); },
      close() { el.classList.remove('on', 'end'); status.tapTestOpen = false; },
    };
    takeTap(el.querySelector('[data-tt="again"]'), () => api.start());
    takeTap(el.querySelector('[data-tt="close"]'), () => api.close());
    takeTap(el.querySelector('[data-tt="copy"]'), () => copyDebug());
    return api;
  })();
  status.tapTest = null;
  status.startTapTest = () => tapTest.start();
  if (root.classList.contains('debug')) setInterval(() => { if (!document.hidden) debugLine(); }, 1000);   // numbers from the start, not only after a tap
  const GLUE = (st) => st === 'login' || st === 'charselect' || st === 'charcreate' || /realm/i.test(st || '');
  const glueState = (st) => {
    glueNow = st || '';
    root.classList.toggle('glue-touch', GLUE(st));
    root.classList.toggle('ingame', GLUE(st) || st === 'inworld');
  };
  glueState(wenilla.session());
  { const st0 = wenilla.session(); root.classList.toggle('glue-type', st0 === 'charcreate' || st0 === 'charselect' || st0 === 'login'); }

  // ---- the world: taps, double-taps, long-press, camera drags, pinch -----------------------------
  const world = $('.world');
  // A mouse (a touch-screen PC) goes straight to the game: the world layer is for fingers and pens.
  // Its drag-to-look turned the camera from page positions while the game turned it from the mouse
  // as well, and the two fought: the view snapped around. A finger or pen brings the layer back.
  // Only a real mouse moving counts (3 Nov): a desktop browser in phone emulation sends a stray mouse move (when the
  // emulation is switched on, or the pointer crosses the page) that turned the world layer off, and the next taps
  // went to the game as touches it doesn't click with. So: a fine pointer, a move that went somewhere, and no
  // finger in the last 800 ms.
  let lastFinger = 0;
  const fine = () => { try { return matchMedia('(any-pointer: fine)').matches; } catch { return true; } };
  addEventListener('pointermove', (e) => {
    if (!e.isTrusted || e.pointerType !== 'mouse' || e.buttons) return;
    if (performance.now() - lastFinger < 800 || !(e.movementX || e.movementY) || !fine()) return;
    root.classList.add('mouse');
  }, true);
  addEventListener('pointerdown', (e) => {
    if (e.isTrusted && e.pointerType !== 'mouse') { lastFinger = performance.now(); root.classList.remove('mouse'); if (kbdMode) setKbdMode(false); }
  }, true);
  // a finger that reached the game's canvas anyway (input.js stops it there): the layers are back for the next one
  // No compatibility mouse events or scrolling from a finger on our own layers (touch-action covers most browsers;
  // this covers the rest and desktop touch emulation)
  root.addEventListener('touchstart', (e) => { if (e.target.closest && e.target.closest('.zone, .ctl, .world, .glue')) e.preventDefault(); }, { passive: false });
  status.fingerOnGame = () => { lastFinger = performance.now(); root.classList.remove('mouse'); status.strayTouches = (status.strayTouches || 0) + 1; };
  const touches = new Map();     // pointerId -> {x0,y0,x,y,t0,kind}
  let camBtn = 2;
  let lastTap = { t: 0, x: 0, y: 0 }, pinch = null, cam = null, hoverTimer = 0, uiDrag = null;
  world.addEventListener('pointerdown', (e) => {
    e.preventDefault();                               // no compatibility mouse events: they took focus off the game
    world.setPointerCapture(e.pointerId);
    info.classList.remove('on');
    if (moving) { const from = moving; moving = null; lua(`${SLOT_FN}\nPlaceAction(slot(${from})) ClearCursor()`).then(() => refreshActions(true)); toast('Move cancelled', 900); }
    // a first finger (no other on the screen, the browser says) while fingers are still counted: their lifts were lost,
    // so they go (else this touch made a "pinch" with a finger long gone, and every tap after it was swallowed)
    if (e.isPrimary && e.pointerType !== 'mouse' && touches.size) dropTouches();
    touches.set(e.pointerId, { x0: e.clientX, y0: e.clientY, x: e.clientX, y: e.clientY, t0: performance.now(), s0: e.timeStamp, moved: false, focus: null });
    if (pinch) { touches.get(e.pointerId).pinched = true; return; }   // a third finger: part of the pinch
    if (touches.size === 2) {
      const [a, b] = [...touches.values()];
      const d = Math.hypot(a.x - b.x, a.y - b.y);
      // x, y: the first finger, where a two-finger tap right-clicks
      pinch = { d, t0: performance.now(), first: a.t0, x: a.x0, y: a.y0, zoomed: 0, travel: Math.hypot(a.x - a.x0, a.y - a.y0) };
      a.pinched = b.pinched = true;
      for (const o of [a, b]) if (o.pressed && !o.out) { o.out = true; pressOut(o, false); }   // a pinch is no click
      endCam();
      clearTimeout(hoverTimer);
      return;
    }
    const t = touches.get(e.pointerId);
    const id = e.pointerId;
    // on the target frame: a sideways swipe there cycles enemies (a tap still clicks the frame)
    t.tf = !!tfRect && e.clientX >= tfRect[0] && e.clientX <= tfRect[2] && e.clientY >= tfRect[1] && e.clientY <= tfRect[3];
    const moved = inTurn(() => moveTo(e.clientX, e.clientY));   // after any tap still being played out
    // the second touch of a double-tap (a right-click on lift) is never pressed in
    t.maybeDbl = performance.now() - lastTap.t < 320 && Math.hypot(e.clientX - lastTap.x, e.clientY - lastTap.y) < 36;
    // is it a game window under the finger (drag it) or the world (turn the camera)?
    // asked once the cursor is there (a tap still playing out goes first), so the answer is about this spot
    // (no bridge: nothing can be asked, so it counts as the world: taps click, drags turn)
    t.focusP = moved.then(() => (wenilla.live ? lua('local f = GetMouseFocus() return f and f.GetName and (f:GetName() or "?") or ""') : [''])).then((r) => {
      t.focus = r?.[0] ?? '';
      pressIn(t, id);
    });
    hoverTimer = setTimeout(() => {
      t.long = true; moveTo(t.x, t.y);
      // Touch and hold: right-click (Menu > Controls), on a window or in the world, never while placing a ground spell
      // (a right-click there cancels it)
      if (holdRight() && !t.moved && !pinch && !aimGround && touches.get(id) === t) holdClick(t);
    }, LONG_MS);
  });
  // a right-click where the finger rests (Touch and hold: right-click). A window button the finger had pressed in is let
  // out off it first (no left click), then the right-click goes in.
  function holdClick(t) {
    t.right = true;
    buzz(20);
    status.tapCount += 1;
    const rec = noteTap({ how: 'hold', screen: 'inworld', x: t.x0, y: t.y0, px: t.x0, py: t.y0, on: t.focus || '' });
    lastTap = { t: 0, x: 0, y: 0 };
    inTurn(async () => {
      if (t.pressed && !t.out) { t.out = true; await settle(); moveTo(-40, -40, 1); mouse('pointerup', -40, -40, 0, 0); await afterFrame(); }
      await clickNow(t.x0, t.y0, 2, rec);
    });
  }
  // A two-finger tap (both down and up quickly, no zoom, hardly moved): a right-click under the first finger. The way
  // to use, equip or sell a bag item and to talk or loot without a double-tap (a double-tap on a window is two clicks).
  function twoTap(x, y) {
    status.tapCount += 1;
    status.twoTaps = (status.twoTaps || 0) + 1;
    buzz(20);
    lastTap = { t: 0, x: 0, y: 0 };
    flash(x, y, true);
    click(x, y, 2, noteTap({ how: 'two', screen: 'inworld', x, y, px: x, py: y }));
  }
  // every finger the world layer counts goes (lost lifts, or the page hidden): buttons pressed in are let out off them,
  // a camera drag ends
  function dropTouches() {
    endCam();
    for (const o of touches.values()) if (o.pressed && !o.out) { o.out = true; pressOut(o, false); }
    if (touches.size) status.stale = (status.stale || 0) + touches.size;
    touches.clear();
    pinch = null;
    clearTimeout(hoverTimer);
  }
  // A finger on a game button presses it in at once and lets it out on lift, as a mouse button
  // does: the button shows pushed while the finger is on it, and the click is the release (players'
  // reports: "not in-out pressing on a single tap", 2026-10-21). Only once the game has said a window
  // is under the finger, and never for the world, a moving finger or the second tap of a double-tap.
  const onWindow = (t) => !!t.focus && t.focus !== 'WorldFrame';
  function pressIn(t, id) {
    // a quick second tap on a window is another click (paging, +/-, Next): only in the world is it a right-click
    if (!onWindow(t) || t.moved || t.lifted || t.right || pinch || touches.get(id) !== t) return;
    inTurn(async () => {
      // the focus answer came from a game update after the move; one more and its hover is current
      await gameTicks(1);
      if (t.moved || t.lifted || t.right || pinch || touches.get(id) !== t || t.pressed) return;
      moveTo(t.x0, t.y0);
      mouse('pointerdown', t.x0, t.y0, 0, 1);
      t.pressed = performance.now();
    });
  }
  // Let a pressed button out: where it was pressed (a click), or off the screen (no click: a long
  // press is a hover, for the tooltip), then back over it so the tooltip stays.
  function pressOut(t, click, rec = null) {
    return inTurn(async () => {
      await settle();                                  // never the same game update as the press
      if (click) { moveTo(t.x0, t.y0, 1); await release(t.x0, t.y0, 0, t.pressed || 0, rec); return; }
      moveTo(-40, -40, 1); mouse('pointerup', -40, -40, 0, 0);
      await afterFrame(); moveTo(t.x0, t.y0);
    });
  }
  world.addEventListener('pointermove', (e) => {
    const t = touches.get(e.pointerId);
    if (!t) return;
    t.x = e.clientX; t.y = e.clientY;
    if (pinch && touches.size === 2) {
      const [a, b] = [...touches.values()];
      const d = Math.hypot(a.x - b.x, a.y - b.y);
      pinch.travel = Math.max(pinch.travel, Math.hypot(t.x - t.x0, t.y - t.y0));
      while (d - pinch.d > 28) { wenilla.fire('CAMERAZOOMIN'); pinch.d += 28; pinch.zoomed += 1; }
      while (pinch.d - d > 28) { wenilla.fire('CAMERAZOOMOUT'); pinch.d -= 28; pinch.zoomed += 1; }
      return;
    }
    // a finger that was part of a pinch does nothing more (it never turns into a camera drag or a tap)
    if (t.pinched || t.right) return;
    if (t.swiped) return;
    if (t.tf && !t.moved) {
      // Swipe the target frame: left for the next enemy (the nearest one, then on round them, as Tab does), right for
      // the last one (Shift-Tab). Anything else that starts there waits a little longer before it turns into a drag.
      const dx = t.x - t.x0, dy = t.y - t.y0;
      if (Math.abs(dx) > 30 && Math.abs(dx) > 1.5 * Math.abs(dy)) {
        t.swiped = true; t.moved = true;
        clearTimeout(hoverTimer);
        if (t.pressed && !t.out) { t.out = true; pressOut(t, false); }
        wenilla.fire(dx < 0 ? 'TARGETNEARESTENEMY' : 'TARGETPREVIOUSENEMY');
        buzz();
        toast(dx < 0 ? 'Next enemy' : 'Last enemy', 700);
        noteTap({ how: 'swipe', screen: 'inworld', x: t.x0, y: t.y0, px: t.x, py: t.y, dir: dx < 0 ? 'next' : 'last' });
        return;
      }
      if (Math.hypot(dx, dy) <= 40) return;
    }
    if (!t.moved && Math.hypot(t.x - t.x0, t.y - t.y0) > TAP_SLOP) {
      // window or world under the finger? wait for the answer (a few ms) before choosing
      if (t.focus === null || t.focus === undefined) {
        if (!t.waiting) { t.waiting = true; t.focusP?.then(() => { if (touches.get(e.pointerId) === t && !t.moved) { startDrag(t, e.pointerId); moveTo(t.x, t.y, uiDrag === e.pointerId ? 1 : (camBtn === 2 ? 2 : 1)); } }); }
        return;
      }
      startDrag(t, e.pointerId);
    }
    if (t.moved) moveTo(t.x, t.y, uiDrag === e.pointerId ? 1 : cam === e.pointerId ? (camBtn === 2 ? 2 : 1) : 0);
  });
  function startDrag(t, id) {
    t.moved = true;
    clearTimeout(hoverTimer);
    const onUi = t.focus && t.focus !== 'WorldFrame' && t.focus !== '';
    if (onUi) { uiDrag = id; if (!t.pressed) mouse('pointerdown', t.x0, t.y0, 0, 1); }
    else { cam = id; camBtn = LS('ow:scheme') === 'simple' ? 0 : 2; mouse('pointerdown', t.x0, t.y0, camBtn, camBtn === 2 ? 2 : 1); status.drags += 1; }
  }
  function endCam() {
    if (cam !== null) { const t = touches.get(cam); if (t) mouse('pointerup', t.x, t.y, camBtn, 0); cam = null; }
    if (uiDrag !== null) {
      const t = touches.get(uiDrag);
      // a rolled tap on a window: let go where it pressed, so the button it landed on clicks
      const rolled = t && t.lifted && (sloppyTap(t, t.x, t.y)
        || (t.pressed && Math.hypot(t.x - t.x0, t.y - t.y0) <= SLOPPY_PX && performance.now() - t.pressed < HOLD_MS));
      if (t) { if (rolled) { moveTo(t.x0, t.y0, 1); mouse('pointerup', t.x0, t.y0, 0, 0); } else mouse('pointerup', t.x, t.y, 0, 0); }
      uiDrag = null;
    }
  }
  const worldEnd = (e) => {
    const t = touches.get(e.pointerId);
    if (!t) return;
    clearTimeout(hoverTimer);
    // A pinch's fingers: the first lift ends it (a quick one that zoomed nothing is a two-finger tap: a right-click).
    // The last finger's lift does nothing: before, it was taken for a tap and clicked (the target changed after a zoom).
    if (t.pinched) {
      touches.delete(e.pointerId);
      if (pinch && !pinch.ended) {
        pinch.ended = true;
        const now = performance.now();
        if (e.type === 'pointerup' && !pinch.zoomed && pinch.travel < 24 && now - pinch.t0 < 400 && now - pinch.first < 650) twoTap(pinch.x, pinch.y);
      }
      if (![...touches.values()].some((o) => o.pinched)) pinch = null;
      return;
    }
    // a touch and hold that right-clicked: nothing more on the lift
    if (t.right) { touches.delete(e.pointerId); return; }
    t.lifted = e.type === 'pointerup';
    if (t.pressed && !t.out && !t.moved && cam !== e.pointerId && uiDrag !== e.pointerId) {
      // pressed in on a game button: out where it went in. A quick lift is the click; a long press
      // (the tooltip) or a cancelled touch lets go off the button.
      const quick = t.lifted && performance.now() - t.pressed < HOLD_MS;
      const rec = quick ? noteTap({ how: 'window', screen: 'inworld', x: t.x0, y: t.y0, px: t.x0, py: t.y0, finger: Math.round(performance.now() - t.t0), on: t.focus }) : null;
      pressOut(t, quick, rec);
      if (quick) { status.tapCount += 1; lastTap = { t: 0, x: 0, y: 0 }; }   // a window tap never starts a double-tap
      touches.delete(e.pointerId);
      return;
    }
    if (cam === e.pointerId && t.lifted && sloppyTap(t, t.x, t.y)) {
      // a quick little swipe on the world was meant as a tap (target, loot, talk): end the turn, tap
      endCam(); status.tapCount += 1;
      click(t.x0, t.y0, 0, noteTap({ how: 'drag', screen: 'inworld', x: t.x0, y: t.y0, px: t.x0, py: t.y0, finger: Math.round(performance.now() - t.t0) }));
    } else if (cam === e.pointerId || uiDrag === e.pointerId) endCam();
    // a tap: no drag began (or one was still waiting to hear what is under the finger)
    else if (!t.moved && e.type === 'pointerup' && (e.timeStamp - t.s0 < LONG_MS || ((onWindow(t) || t.focus == null) && e.timeStamp - t.s0 < SLOW_TAP_MS))
      && (!t.waiting || sloppyTap(t, t.x, t.y))) {
      t.x = t.x0; t.y = t.y0;                          // the tap is where the finger landed, not where it rolled to
      const now = performance.now();
      status.tapCount += 1;
      // a window or the world? A quick tap can lift before the game has said (a frame): wait for it,
      // so two quick taps on a window's button are two clicks, not a click and a right-click
      const rec = { how: 'tap', screen: 'inworld', x: t.x, y: t.y, px: t.x, py: t.y, finger: Math.round(now - t.t0) };
      const decide = () => {
        // a ground-targeted spell waiting for its place: this tap puts it there (a plain left click, never a right-click)
        if (aimGround && !onWindow(t)) {
          rec.how = 'ground';
          noteTap(rec);
          lastTap = { t: 0, x: 0, y: 0 };
          click(t.x, t.y, 0, rec).then(() => { setAimGround(false); refreshActions(true); });
          flash(t.x, t.y, true);
          return;
        }
        const dbl = !onWindow(t) && now - lastTap.t < 320 && Math.hypot(t.x - lastTap.x, t.y - lastTap.y) < 36;
        if (dbl) rec.how = 'right';
        if (onWindow(t)) { rec.how = 'window'; rec.on = t.focus; }
        noteTap(rec);
        lastTap = dbl || onWindow(t) ? { t: 0, x: 0, y: 0 } : { t: now, x: t.x, y: t.y };
        // Soft lock: a tap on open ground (no unit under it) keeps the target, where the game would clear it. Asked of the
        // game with the cursor already there; with no answer (no bridge) the tap clicks as before.
        if (!dbl && !onWindow(t) && softLock() && targetState.exists && wenilla.live) {
          inTurn(async () => {
            moveTo(t.x, t.y);
            await settle();
            const r = await lua('return UnitExists("mouseover") and 1 or 0');
            if (r?.[0] === 0 || r?.[0] === '0') { rec.how = 'kept'; status.kept = (status.kept || 0) + 1; return; }
            await clickNow(t.x, t.y, 0, rec);
          });
          return;
        }
        click(t.x, t.y, dbl ? 2 : 0, rec);
      };
      if (t.focus === null || t.focus === undefined) Promise.race([t.focusP, wait(300)]).then(decide, decide);
      else decide();
    }
    touches.delete(e.pointerId);
  };
  world.addEventListener('pointerup', worldEnd);
  world.addEventListener('pointercancel', worldEnd);

  // ---- action keys: icons, cooldowns, usability, range -----------------------------------------
  const ACTION_LUA = `${SLOT_FN}
local now = GetTime()
local out = {}
for i = 1, 12 do
  local s = slot(i)
  local tex = GetActionTexture(s) or ""
  local st, du, en = GetActionCooldown(s)
  local rem = 0
  if st and du and en == 1 and du > 1.6 then rem = st + du - now end
  local u, nm = IsUsableAction(s)
  local r = IsActionInRange(s)
  local c = GetActionCount and GetActionCount(s) or 0
  table.insert(out, tex .. "|" .. string.format("%.1f", rem) .. "|" .. (u and 1 or 0) .. (nm and 1 or 0) .. "|" .. tostring(r) .. "|" .. (c or 0))
end
local ex = UnitExists("target") and 1 or 0
local ho = (UnitCanAttack and UnitCanAttack("player", "target")) and 1 or 0
local de = UnitIsDead("target") and 1 or 0
-- what Attack / Interact would do with the target
local role = "fight"
if ex == 1 and not (ho == 1 and de == 0) then
  if de == 1 then role = "loot"
  elseif UnitIsPlayer("target") then role = "player"
  else
    local nm = UnitName("target") or ""
    local T = __OWTOUCH or {}
    role = ""
    -- (the quest marks come from the bridge snapshot's target.questStatus, page side)
    if role == "" then
      T.roles = T.roles or {}
      role = T.roles[nm]
      if not role then
        GameTooltip:SetOwner(UIParent, "ANCHOR_NONE")
        GameTooltip:SetUnit("target")
        local sub = string.lower((GameTooltipTextLeft2 and GameTooltipTextLeft2:GetText()) or "")
        GameTooltip:Hide()
        local function has(p) return string.find(sub, p, 1, true) end
        if has("spirit healer") then role = "spirit"
        elseif has("innkeeper") then role = "inn"
        elseif has("flight") or has("gryphon") or has("hippogryph") or has("wind rider") or has("bat handler") then role = "flight"
        elseif has("repair") or has("armor") or has("blacksmith") or has("weapon") or has("shield") or has("mail") then role = "repair"
        elseif has("trainer") then role = "train"
        elseif has("bank") then role = "bank"
        elseif has("stable") then role = "stable"
        elseif has("battlemaster") then role = "battle"
        elseif has("guild") or has("tabard") then role = "guild"
        elseif has("<") then role = "shop"
        else role = "talk" end
        if __OWTOUCH then T.roles[nm] = role end
      end
    end
  end
end
-- what else the touch layer reacts to: the box with the keyboard focus, open windows,
-- something on the cursor, stances/forms and the pet bar
local kf = ${FOCUS}
if kf and not kf:IsVisible() then kf = nil end
local kfn = kf and (kf:GetName() or "?") or ""
local kft = (kf and kf.GetText and kf:GetText()) or ""
local open = 0
local PANELS = { "left", "center", "doublewide", "fullscreen" }
local function panel(k) return GetUIPanel and GetUIPanel(k) end
for _, k in ipairs(PANELS) do if panel(k) then open = 1 end end
-- every open window's rectangle in CSS px, so a touch on a window over the stick reaches the window
local rects, seen = {}, {}
local ch = (__OWTOUCH and __OWTOUCH.cssH) or 360
local up = UIParent:GetHeight() / ch
local function box(f)
  local s = f:GetEffectiveScale() / UIParent:GetEffectiveScale()
  local l, b, w, h = f:GetLeft() * s, f:GetBottom() * s, f:GetWidth() * s, f:GetHeight() * s
  return math.floor(l / up) .. "," .. math.floor((UIParent:GetHeight() - b - h) / up) .. "," .. math.ceil((l + w) / up) .. "," .. math.ceil((UIParent:GetHeight() - b) / up)
end
local function rect(f)
  if not f or seen[f] or not f:IsVisible() or not f:GetLeft() then return end
  seen[f] = true
  table.insert(rects, box(f))
end
for _, k in ipairs(PANELS) do rect(panel(k)) end
if UISpecialFrames then for _, n in ipairs(UISpecialFrames) do rect(_G[n]) end end
for i = 1, (NUM_CONTAINER_FRAMES or 13) do rect(_G["ContainerFrame" .. i]) end
for _, n in ipairs({ "StaticPopup1", "StaticPopup2", "StaticPopup3", "StaticPopup4", "LootFrame", "GroupLootFrame1", "GroupLootFrame2", "GroupLootFrame3", "GroupLootFrame4", "ItemRefTooltip", "DropDownList1", "DropDownList2" }) do rect(_G[n]) end
if #rects > 0 then open = 1 end
local cur = (GetCursorInfo and GetCursorInfo()) and 1 or 0
local ss = {}
for i = 1, (GetNumShapeshiftForms and GetNumShapeshiftForms() or 0) do
  local tex, nm, act, ok = GetShapeshiftFormInfo(i)
  table.insert(ss, (tex or "") .. "~" .. (act and 1 or 0) .. "~" .. (ok and 1 or 0))
end
local pet = {}
if PetHasActionBar and PetHasActionBar() then
  for i = 1, 10 do
    local nm, sub, tex, tok, act, aok, aon = GetPetActionInfo(i)
    if nm then
      if tok then tex = getglobal(tex) end
      table.insert(pet, i .. "~" .. (tex or "") .. "~" .. (act and 1 or 0) .. "~" .. (aon and 1 or 0))
    end
  end
end
-- a vehicle or possess bar to leave
local leave = ""
if CanExitVehicle and CanExitVehicle() then leave = "vehicle"
elseif PossessBarFrame and PossessBarFrame:IsShown() then leave = "possess" end
-- the target frame (swipe it for the next enemy), the cast bar, a ground-targeted spell waiting for its place
local tf = (TargetFrame and TargetFrame:IsVisible() and TargetFrame:GetLeft()) and box(TargetFrame) or ""
-- UnitCastingInfo: name, sub, text, texture, start ms, end ms, ...; UnitChannelInfo the same
local cast = ""
local cn, _, _, cic, cs, ce = UnitCastingInfo("player")
local chan = 0
if not cn and UnitChannelInfo then cn, _, _, cic, cs, ce = UnitChannelInfo("player") if cn then chan = 1 end end
if cn and cs and ce then cast = cn .. "~" .. (cic or "") .. "~" .. cs .. "~" .. ce .. "~" .. chan .. "~" .. string.format("%.3f", GetTime()) end
local aim = (SpellIsTargeting and SpellIsTargeting()) and 1 or 0
return table.concat(out, "\\n"), ex, ho, de, kfn, kft, open, cur, table.concat(ss, ";"), table.concat(pet, ";"), table.concat(rects, ";"), role, leave, tf, cast, aim`;
  // the bridge snapshot's quest status for the target, over what its tooltip said
  const QUEST_ROLE = { available: 'quest', reward: 'turnin', incomplete: 'inprogress' };
  const questRole = (r) => {
    if (r === 'fight' || r === 'loot' || r === 'player') return r;
    return QUEST_ROLE[wenilla.state?.target?.questStatus] || r;
  };
  // ---- the cast bar (over the ring) and ground-targeted spells --------------------------------------
  // From each poll's UnitCastingInfo / UnitChannelInfo ("name~icon~start ms~end ms~channel~GetTime()"), or the bridge
  // snapshot's state.player.cast ({ name, start, end, channel } in ms of GetTime) when the client sends one; the bar
  // then runs on the page's clock between polls (a channel drains, a cast fills).
  let tfRect = null;                        // the target frame, css px [l, t, r, b] (swipe it for the next enemy)
  const castEl = $('.castbar'), castFill = castEl.querySelector('i'), castName = castEl.querySelector('span');
  let cast = null, castRaf = 0;
  function parseCast(v) {
    const p = String(v || '').split('~');
    if (p.length < 6 || !p[0]) return null;
    const start = +p[2], end = +p[3], now = +p[5];
    if (!(end > start) || !Number.isFinite(now)) return null;
    return { name: p[0], icon: p[1], start, end, channel: p[4] === '1', now: now * 1000 };
  }
  function snapCast() {
    const c = wenilla.state?.player?.cast;
    if (!c || !c.name || !(c.end > c.start)) return null;
    return { name: String(c.name), icon: c.icon || '', start: +c.start, end: +c.end, channel: !!c.channel, now: +(c.now ?? c.start) };
  }
  function castUpdate(c) {
    if (!c) {
      if (cast) { cast = null; castEl.classList.remove('on'); status.cast = null; }
      return;
    }
    const left = c.end - c.now;                       // ms still to go, by the game's clock
    const same = cast && cast.name === c.name && cast.start === c.start;
    cast = { ...c, dur: c.end - c.start, endsAt: performance.now() + left };
    status.cast = { name: c.name, channel: c.channel, left: Math.round(left) };
    if (!same) { castName.textContent = c.name; castEl.classList.toggle('chan', c.channel); }
    castEl.classList.add('on');
    if (!castRaf) castRaf = requestAnimationFrame(castTick);
  }
  function castTick() {
    castRaf = 0;
    if (!cast) return;
    const left = cast.endsAt - performance.now();
    if (left < -150) { castUpdate(null); return; }
    const done = Math.min(1, Math.max(0, 1 - left / cast.dur));
    castFill.style.width = ((cast.channel ? 1 - done : done) * 100).toFixed(1) + '%';
    castRaf = requestAnimationFrame(castTick);
  }
  status.castUpdate = castUpdate;
  status.parseCast = parseCast;
  takeTap($('.gaim .gx'), () => { lua('if SpellStopTargeting then SpellStopTargeting() end'); setAimGround(false); toast('Cancelled', 900); });
  status.setAimGround = setAimGround;

  let actions = [];
  let actBusy = false;
  const extras = { cursor: false };
  let winRects = [];
  let moving = null;   // the action slot (1-12) a Move picked up, until a key takes it
  async function refreshActions(force) {
    if (actBusy && !force) return;
    actBusy = true;
    try {
      const r = await lua(ACTION_LUA);
      if (!r || !r[0]) return;
      actions = String(r[0]).split('\n').map((l) => {
        const [tex, rem, un, range, count] = l.split('|');
        return { tex, rem: +rem, usable: un?.[0] === '1', nomana: un?.[1] === '1', range, count: parseInt(count, 10) || 0 };
      });
      targetState = { exists: r[1] == 1, hostile: r[2] == 1, dead: r[3] == 1 };
      extras.cursor = r[7] == 1;
      $('.closer').classList.toggle('on', r[6] == 1);
      focusChanged(String(r[4] || ''), String(r[5] || ''));
      renderStrip(String(r[8] || ''), String(r[9] || ''), String(r[12] || ''));
      winRects = String(r[10] || '').split(';').filter(Boolean).map((x) => x.split(',').map(Number));
      root.classList.toggle('windows', winRects.length > 0);
      attack.classList.toggle('hostile', targetState.exists && targetState.hostile && !targetState.dead);
      setRole(questRole(String(r[11] || 'fight')));
      tfRect = String(r[13] || '').split(',').map(Number);
      if (tfRect.length !== 4 || tfRect.some((n) => !Number.isFinite(n))) tfRect = null;
      castUpdate(parseCast(r[14]) || snapCast());
      if (r[15] != null) setAimGround(r[15] == 1);
      // four times a second: only what changed is written (an unchanged text set again still costs a layout)
      const setText = (el, v) => { v = String(v); if (el.textContent !== v) el.textContent = v; };
      keys.forEach((k, i) => {
        const a = actions[slotOf(i) - 1];
        const face = k.querySelector('.face');
        if (!a || !a.tex) {
          if (face.dataset.tex || !k.classList.contains('empty')) { face.style.backgroundImage = ''; face.dataset.tex = ''; face.classList.remove('icon'); k.className = 'ctl key play empty'; }
          setText(k.querySelector('.count'), '');
          return;
        }
        k.classList.remove('empty');
        if (face.dataset.tex !== a.tex) {
          face.dataset.tex = a.tex;
          iconUrl(a.tex).then((u) => { if (u && face.dataset.tex === a.tex) { face.style.backgroundImage = `url(${u})`; face.classList.add('icon'); } });
        }
        k.classList.toggle('cooling', a.rem > 0);
        setText(k.querySelector('.cd'), a.rem > 0 ? (a.rem >= 60 ? Math.ceil(a.rem / 60) + 'm' : Math.ceil(a.rem)) : '');
        k.classList.toggle('unusable', !a.usable && !a.nomana);
        k.classList.toggle('nomana', a.nomana);
        k.classList.toggle('far', a.range === '0');
        setText(k.querySelector('.count'), a.count > 1 ? a.count : '');
      });
    } finally { actBusy = false; }
  }

  // ---- the game's own frames, rearranged for a small screen ------------------------------------
  // the stock frames the cluster, the tray and the chat sheet replace (3.3.5a names). The chat
  // frames themselves (ChatFrame1..10, their tabs and edit boxes) go by name pattern.
  // TODO(owner): if Forever UI draws its own action bars or chat, add its frame names here.
  const HIDE_FRAMES = ['MainMenuBar', 'MultiBarBottomLeft', 'MultiBarBottomRight', 'MultiBarLeft', 'MultiBarRight',
    'BonusActionBarFrame', 'ShapeshiftBarFrame', 'PetActionBarFrame', 'PossessBarFrame', 'MultiCastActionBarFrame',
    'VehicleMenuBar', 'MainMenuBarArtFrame', 'GeneralDockManager', 'FriendsMicroButton', 'ChatFrameMenuButton'];
  const luaList = (a) => '{ ' + a.map((n) => JSON.stringify(n)).join(', ') + ' }';
  function gameLayoutLua(on) {
    const c = gameCanvas();
    const cssH = Math.max(200, c?.clientHeight || innerHeight);
    // interface height in game units: 560 on a phone, up to the stock 768 on a tablet
    const size = Math.min(1.4, Math.max(0.7, +(LS('ow:touch-ui') || 1)));
    const vh = Math.round(Math.min(768, Math.max(560, cssH * 1.5)) / size);
    // the notch (landscape: a side inset), in css px: the frames along the top edges stay clear of it
    const sl = Math.round(SAFE.l), sr = Math.round(SAFE.r);
    const drawer = 10 + 36 + 8 + sr;   // the tray button's column right of the minimap
    return `local on = ${on ? 'true' : 'false'}
if not __OWTOUCH then __OWTOUCH = { hidden = {}, orig = {} } end
local T = __OWTOUCH
T.orig = T.orig or {}
T.places = T.places or {}
local function place(name, pt, rel, x, y, s)
  local f = _G[name] if not f then return end
  if not T.orig[name] then
    local a, r, b, ox, oy = f:GetPoint(1)
    T.orig[name] = { a, r, b, ox, oy, f:GetScale() }
  end
  T.places[name] = { pt, rel, x, y, s }
  f:SetScale(s or 1) f:ClearAllPoints() f:SetPoint(pt, UIParent, rel, x / (s or 1), y / (s or 1))
end
-- 3.3.5a's UIParent_ManageFramePositions moves the tracker, the buffs and the bars back whenever
-- a bar or the vehicle UI changes: put ours back right after it (hooked once per interface)
if not T.hookedManage and hooksecurefunc and UIParent_ManageFramePositions then
  T.hookedManage = true
  hooksecurefunc("UIParent_ManageFramePositions", function()
    local t = __OWTOUCH
    if not (t and t.applied and t.places) then return end
    for n, p in pairs(t.places) do
      local f = _G[n]
      if f then f:SetScale(p[5] or 1) f:ClearAllPoints() f:SetPoint(p[1], UIParent, p[2], p[3] / (p[5] or 1), p[4] / (p[5] or 1)) end
    end
    for _, f in ipairs(t.hidden) do if f:IsShown() then f:Hide() end end
    for _, n in ipairs(t.hideNames or {}) do local f = _G[n] if f and f:IsShown() then f:Hide() table.insert(t.hidden, f) end end
  end)
end
local function hide(f) if f and f:IsShown() then f:Hide() table.insert(T.hidden, f) end end
if on then
  if not T.s0 then T.s0 = UIParent:GetScale() end
  -- the interface spans ${vh} units of height whatever the screen (idempotent: re-applying keeps it)
  local k = UIParent:GetScale() * UIParent:GetHeight() / ${vh}
  UIParent:SetScale(k)
  local u = UIParent:GetHeight() / ${cssH}
  T.cssH = ${cssH}
  -- player and target side by side along the top-left edge, one scale, one style
  place("PlayerFrame", "TOPLEFT", "TOPLEFT", (16 + ${sl}) * u - 19, -12 * u + 4, 1)
  place("TargetFrame", "TOPLEFT", "TOPLEFT", (16 + ${sl}) * u - 19 + 238, -12 * u + 4, 1)
  local ms = 100 * u / 140
  place("MinimapCluster", "TOPRIGHT", "TOPRIGHT", -${drawer} * u + 14 * ms, -12 * u + 10 * ms, ms)
  -- buffs: the game hangs BuffFrame off TemporaryEnchantFrame, so that is the one to move
  place("TemporaryEnchantFrame", "TOPRIGHT", "TOPRIGHT", -(${drawer} + 104) * u, -12 * u, 0.8)
  if BuffFrame then if not T.orig.BuffScale then T.orig.BuffScale = BuffFrame:GetScale() end BuffFrame:SetScale(0.8) end
  -- tutorial alert buttons: a tidy row just above the bottom edge, between stick and cluster
  local n = 0
  while _G["TutorialFrameAlertButton" .. (n + 1)] do n = n + 1 end
  for i = 1, n do
    place("TutorialFrameAlertButton" .. i, "BOTTOM", "BOTTOM", ((i - 1) - (n - 1) / 2) * 24 * u, 56 * u, 0.5)
  end
  -- the quest tracker (3.3.5a's WatchFrame): under the minimap on a tablet, under the player frame
  -- on a phone (the cluster needs the right side)
  if ${cssH} >= 480 then
    place("WatchFrame", "TOPRIGHT", "TOPRIGHT", -${drawer} * u, -(12 + 100 + 52) * u, 0.9)
  else
    place("WatchFrame", "TOPLEFT", "TOPLEFT", (16 + ${sl}) * u, -70 * u, 0.85)
  end
  -- the vehicle seat picture would sit on the minimap's spot: below the target frame instead
  place("VehicleSeatIndicator", "TOPLEFT", "TOPLEFT", (16 + ${sl}) * u + 238, -100 * u, 0.7)
  -- the bars, every time (the vehicle and possess bars only show once they are needed)
  T.hideNames = ${luaList(HIDE_FRAMES)}
  for _, n in ipairs(T.hideNames) do hide(_G[n]) end
  if not T.applied then
    local f = EnumerateFrames()
    while f do
      local nm = f:GetName()
      if nm and f:GetParent() == UIParent and string.find(nm, "^ChatFrame") then hide(f) end
      f = EnumerateFrames(f)
    end
    T.applied = true
  end
  return "on", k
else
  UIParent:SetScale(T.s0 or 1)
  T.s0 = nil
  if BuffFrame and T.orig.BuffScale then BuffFrame:SetScale(T.orig.BuffScale) end
  T.orig.BuffScale = nil
  T.places = {}
  for name, o in pairs(T.orig) do
    local f = _G[name]
    if f and o[1] then f:SetScale(o[6] or 1) f:ClearAllPoints() f:SetPoint(o[1], o[2] or UIParent, o[3] or o[1], o[4] or 0, o[5] or 0) end
  end
  T.orig = {}
  local shown = {}
  for _, f in ipairs(T.hidden) do if not shown[f] then shown[f] = true f:Show() end end
  T.hidden = {} T.applied = nil
  -- the vehicle and possess bars only where they belong now
  if VehicleMenuBar and not (UnitHasVehicleUI and UnitHasVehicleUI("player")) then VehicleMenuBar:Hide() end
  if PossessBarFrame and not (GetPossessInfo and GetPossessInfo(1)) then PossessBarFrame:Hide() end
  if UIParent_ManageFramePositions then UIParent_ManageFramePositions() end
  return "off"
end`;
  }
  let layoutBusy = false;
  async function applyGameLayout() {
    if (layoutBusy || !wenilla.live) return;
    layoutBusy = true;
    try {
      const r = await lua(gameLayoutLua(true));
      // (its own field: status.layout is the controls' layout, which Copy debug info reports)
      status.gameLayout = r?.[0] || 'failed';
    } finally { layoutBusy = false; }
  }

  // ---- on / off, world entry -------------------------------------------------------------------
  let actTimer = 0, layTimer = 0, wake = null;
  const fsEl = () => document.fullscreenElement || document.webkitFullscreenElement;
  const el0 = document.documentElement;
  const canFs = !!((document.fullscreenEnabled || document.webkitFullscreenEnabled) && (el0.requestFullscreen || el0.webkitRequestFullscreen));
  // the installed app (Add to Home Screen; the manifest asks for full screen, landscape)
  const installed = () => matchMedia('(display-mode: fullscreen), (display-mode: standalone)').matches || navigator.standalone === true;
  const lockLandscape = () => { try { screen.orientation?.lock?.('landscape').catch(() => {}); } catch { /* not supported */ } };
  function fullscreen() {
    if (fsEl()) { lockLandscape(); return; }
    try {
      const p = el0.requestFullscreen ? el0.requestFullscreen({ navigationUI: 'hide' }) : el0.webkitRequestFullscreen?.();
      Promise.resolve(p).then(lockLandscape).catch(() => {});
    } catch { /* refused */ }
  }
  const phone = () => Math.min(screen.width || innerWidth, screen.height || innerHeight) < 600 || Math.min(innerWidth, innerHeight) < 500;
  let fsSkip = false;
  try { fsSkip = sessionStorage.getItem('ow:fs-skip') === '1'; } catch { /* storage blocked */ }
  function fsState() {
    root.classList.toggle('phone', phone());
    root.classList.toggle('canfs', canFs && !installed());
    root.classList.toggle('wantfs', !fsEl() && !fsSkip);
  }
  // Acted on when the finger LIFTS (5 Nov: "the fullscreen tap to activate sometimes breaks"). A browser lets a page go
  // full screen, lock the orientation or show its install prompt only from a user gesture, and a touch only counts as
  // one on pointerup / touchend (a finger's pointerdown does not; a mouse's does, and it still holds at its pointerup).
  // Run from pointerdown, full screen only worked when an earlier tap had happened to leave the page "activated".
  // The press is still taken (nothing reaches the game under the button).
  function takeTap(el, fn) {
    let downId = null;
    el.addEventListener('pointerdown', (e) => { e.preventDefault(); e.stopPropagation(); downId = e.pointerId; });
    el.addEventListener('pointerup', (e) => {
      e.preventDefault(); e.stopPropagation();
      if (downId !== e.pointerId) return;
      downId = null;
      fn(e);
    });
    el.addEventListener('pointercancel', () => { downId = null; });
  }
  // "Add Leyline to your home screen" (5 Nov): the installed app opens full screen and landscape with the Leyline icon
  // (www-patch.py adds the manifest). Android Chrome hands us its install prompt; iPhone Safari has none, so it says how.
  let installEvt = null;
  const iOS = iOSDevice;
  const installState = () => root.classList.toggle('caninstall', !installed() && (!!installEvt || iOS() || phone()));
  addEventListener('beforeinstallprompt', (e) => { e.preventDefault(); installEvt = e; installState(); });
  addEventListener('appinstalled', () => { installEvt = null; installState(); toast('Leyline is on your home screen: play from the new icon', 5000); });
  function install() {
    if (installEvt) {
      const ev = installEvt;
      installEvt = null;
      try { Promise.resolve(ev.prompt()).catch(() => {}); } catch { /* refused */ }
      installState();
      return;
    }
    if (iOS()) toast('Tap Share (the square with the arrow), then Add to Home Screen. Play from the new Leyline icon', 9000);
    else toast('Open your browser menu, then Add to Home screen (or Install app). Play from the new Leyline icon', 9000);
  }
  status.install = install;
  installState();
  takeTap($('.fsgo'), (e) => {
    if (e.target.closest('.inst')) { install(); return; }
    if (e.target.closest('.skip')) { fsSkip = true; try { sessionStorage.setItem('ow:fs-skip', '1'); } catch { /* fine */ } fsState(); return; }
    fullscreen();
  });
  takeTap($('.rotate .go'), () => fullscreen());
  takeTap($('.rotate .inst'), () => install());   // Android: full screen and turns to landscape by itself
  for (const ev of ['fullscreenchange', 'webkitfullscreenchange']) document.addEventListener(ev, () => { fsState(); layout(); });
  addEventListener('resize', fsState);
  screen.orientation?.addEventListener?.('change', () => { fsState(); if (fsEl()) lockLandscape(); });
  fsState();
  // iPhone Safari has no full screen for pages: the installed app is the way (said once per visit)
  let iosHint = false;
  const iosTip = () => {
    if (iosHint || canFs || installed() || !phone() || !/iPhone|iPod/.test(navigator.userAgent)) return;
    iosHint = true;
    setTimeout(() => toast('For full screen: tap Share, then Add to Home Screen, and play from the new icon', 7000), 1500);
  };

  // ---- the page in the background, the phone's keyboard, page zoom, a hardware keyboard -----------------------------
  // Another app, a call, the lock button or the tab switcher: the lift of a finger still down never comes, so the stick,
  // Jump, a camera drag and a pressed window button are let go here (the character ran on by itself, and a camera
  // drag's button stayed down). Back again: the screen lock is asked for again (the browser drops it when hidden).
  function letGo() {
    if (stickId !== null) stickReset();
    setHolds(new Set());
    releaseJump();
    dropTouches();
    if (gl) { clearTimeout(aimTimer); loupe.classList.remove('on'); if (gl.mode === 'drag' || gl.mode === 'held') mouse('pointerup', gl.x, gl.y, 0, 0); gl = null; }
    try { wenilla.releaseAll?.(); } catch { /* fine */ }
    status.letGo = (status.letGo || 0) + 1;
  }
  status.letGo = 0;
  const askWake = () => {
    if (wake || !status.inworld || document.hidden || !navigator.wakeLock?.request) return;
    navigator.wakeLock.request('screen').then((w) => { wake = w; w.addEventListener?.('release', () => { if (wake === w) wake = null; }); }).catch(() => {});
  };
  // (the browser drops the screen lock of a hidden page itself: forgotten here, so coming back asks again)
  document.addEventListener('visibilitychange', () => { if (document.hidden) { letGo(); wake = null; } else askWake(); });
  addEventListener('pagehide', () => letGo());
  addEventListener('blur', () => { if (stickId !== null || touches.size || held.size) letGo(); });
  status.wake = () => !!wake;

  // The phone's keyboard covers the bottom of the page without resizing it (Android Chrome and iPhone Safari both
  // shrink only the visual viewport): the chat sheet is lifted over it (--ow-kb) and kept to the visible height.
  // iPhone Safari also scrolls the page up to the box and can leave it there: put back when the box lets go.
  const vv = window.visualViewport;
  function vvUpdate() {
    if (!vv) return;
    const kb = Math.max(0, Math.round(innerHeight - vv.height - vv.offsetTop));
    root.style.setProperty('--ow-kb', kb + 'px');
    root.style.setProperty('--ow-vvh', Math.round(vv.height) + 'px');
    root.classList.toggle('kbopen', kb > 80);
    status.keyboard = kb;
  }
  vv?.addEventListener('resize', vvUpdate);
  vv?.addEventListener('scroll', vvUpdate);
  vvUpdate();
  root.addEventListener('focusout', (e) => {
    if (!/^(INPUT|TEXTAREA)$/.test(e.target?.tagName || '')) return;
    setTimeout(() => {
      const a = document.activeElement;
      if (a && /^(INPUT|TEXTAREA)$/.test(a.tagName)) return;          // on to another box
      if (scrollX || scrollY) scrollTo(0, 0);
      vvUpdate();
    }, 60);
  });

  // No page zoom while touch is on: iPhone Safari ignores user-scalable=no and zooms on a pinch or a double-tap
  // anywhere the page lets it (its own gesture events, cancelled here); Android honours the viewport tag, which gets
  // maximum-scale=1,user-scalable=no while touch is on (put back when it is turned off). The game zooms with a pinch.
  const vpMeta = document.querySelector('meta[name="viewport"]');
  const vp0 = vpMeta?.getAttribute('content') ?? null;
  const noZoom = (on) => {
    document.documentElement.classList.toggle('ow-touch-on', on);
    if (!vpMeta || vp0 === null) return;
    vpMeta.setAttribute('content', on && !/user-scalable/i.test(vp0) ? vp0 + ',maximum-scale=1,user-scalable=no' : vp0);
  };
  for (const g of ['gesturestart', 'gesturechange', 'gestureend']) document.addEventListener(g, (e) => { if (status.on) e.preventDefault(); }, { passive: false });

  // A hardware keyboard (a tablet's, a phone's Bluetooth one) moving the character: the thumb controls step aside until
  // the next touch (Menu > Controls > Keyboard hides touch). Only real keys on the game (not a text box, not the keys
  // this layer sends itself).
  let kbdMode = false;
  const HWKEY = /^(w|a|s|d|q|e|arrowup|arrowdown|arrowleft|arrowright| )$/i;
  function setKbdMode(on) {
    on = !!on;
    if (on === kbdMode) return;
    kbdMode = on;
    status.kbdMode = on;
    root.classList.toggle('kbdmode', on);
    if (on) { if (stickId !== null) stickReset(); setTray(false); toast('Keyboard: touch controls hidden. Touch the screen to bring them back', 3200); }
  }
  addEventListener('keydown', (e) => {
    if (!e.isTrusted || kbdMode || !status.on || !status.inworld || LS('ow:kbdhide') === 'off') return;
    const tg = e.target;
    if (tg && (/^(INPUT|TEXTAREA|SELECT)$/.test(tg.tagName) || tg.isContentEditable)) return;
    if (e.ctrlKey || e.metaKey || e.altKey || !HWKEY.test(e.key || '')) return;
    setKbdMode(true);
  }, true);
  status.setKbdMode = setKbdMode;

  function enterWorld() {
    document.body.classList.add('ow-touch-world');
    if (LS('ow:scheme') === 'simple') lua('if SetCVar then SetCVar("cameraSmoothStyle", "1") end');
    if (!status.on || status.inworld) return;
    status.inworld = true;
    root.classList.add('inworld');
    layout();
    loadCharLayout();
    setTimeout(applyGameLayout, 1200);
    setTimeout(applyAutoLoot, 1500);
    keyNumbers();
    clearInterval(layTimer);
    // re-apply after a /reload (a fresh interface comes back at its stock layout)
    layTimer = setInterval(async () => {
      if (document.hidden) return;                     // nothing to keep tidy on a page nobody sees (battery)
      // anything an addon showed again (its own upkeep) goes back out of the way
      const r = await lua(`local T = __OWTOUCH
if not (T and T.applied) then return "no" end
for _, f in ipairs(T.hidden) do if f:IsShown() then f:Hide() end end
for _, n in ipairs(T.hideNames or {}) do local f = _G[n] if f and f:IsShown() then f:Hide() table.insert(T.hidden, f) end end
return "yes"`);
      if (r?.[0] === 'no' && wenilla.live) applyGameLayout();
    }, 5000);
    clearInterval(actTimer);
    actTimer = setInterval(() => { if (!document.hidden) refreshActions(); }, 250);
    // warm the menu sheet's icons once the world has settled, so it opens fully drawn
    setTimeout(() => { MENU.forEach((m) => iconUrl(m.icon).catch(() => {})); Object.values(ROLE_TEX).forEach((t) => iconUrl(t).catch(() => {})); }, 8000);
    askWake();
    const seen = +(LS('ow:touch-hints') || 0);
    if (seen < 3) { LS('ow:touch-hints', String(seen + 1)); setTimeout(() => toast('Drag to look · Tap to target · Double-tap to talk and loot · Menu ▸ Controls to change how it plays', 6000), 2500); }
  }
  function leaveWorld() {
    document.body.classList.remove('ow-touch-world');
    status.inworld = false;
    root.classList.remove('inworld');
    clearInterval(actTimer); clearInterval(layTimer);
    setHolds(new Set());
    wake?.release?.().catch(() => {}); wake = null;
  }
  function turnOn() {
    if (status.on) return;
    status.on = true;
    root.classList.add('on');
    document.body.classList.add('ow-touch-on');
    noZoom(true);
    layout();
    if (wenilla.session() === 'inworld') enterWorld();
  }
  function turnOff() {
    if (!status.on) return;
    if (status.inworld) { lua(gameLayoutLua(false)); applyAutoLoot(true); }
    leaveWorld();
    status.on = false;
    root.classList.remove('on');
    document.body.classList.remove('ow-touch-on', 'ow-touch-world');
    noZoom(false);
    setKbdMode(false);
  }
  // the Lua this layer sends, for the tests (tests/lua_chunks.mjs runs them under luac / a stub)
  status.luaChunks = () => ({ action: ACTION_LUA, tip: TIP_LUA(1), layoutOn: gameLayoutLua(true), layoutOff: gameLayoutLua(false), cursor: CURSOR_LUA, focus: FOCUS_LUA });
  status.turnOn = () => { LS('ow:touch', 'on'); turnOn(); };
  status.turnOff = () => { LS('ow:touch', 'off'); turnOff(); };

  // hidden behind the loading screen (stages.js: 'world-loading' until 'world-ready')
  let loadSafety = 0;
  addEventListener('ow:progress', (e) => {
    if (e.detail === 'world-loading') {
      root.classList.add('loading');
      clearTimeout(loadSafety);
      loadSafety = setTimeout(() => root.classList.remove('loading'), 180000);
    } else if (e.detail === 'world-ready') {
      clearTimeout(loadSafety);
      root.classList.remove('loading');
      if (status.inworld) setTimeout(applyGameLayout, 300);
    }
  });

  wenilla.on?.('state', (s) => {
    const st = s?.state;
    if (st === 'inworld') enterWorld(); else if (status.inworld) leaveWorld();
    root.classList.toggle('glue-type', st === 'charcreate' || st === 'charselect' || st === 'login');
    glueState(st);
    if (GLUE(st)) iosTip();
  });

  // a controller takes over: the bridge's gamepad mapper, touch controls out of the way
  // The first time (per device) a card says so, with the buttons' jobs: the mapper's own list when the bridge gives one
  // (wenilla.gamepad.mapping: [[button, what it does], ...] or { button: what }), else the standard layout below.
  // Only the card and messages show while the controller has the game (padmode); unplugging it brings touch back.
  // TODO: check PAD_MAP against the bridge's gamepad mapper once it is in the 3.3.5a client.
  const PAD_MAP = [['Left stick', 'Move'], ['Right stick', 'Camera'], ['A / Cross', 'Jump'], ['B / Circle', 'Interact (right-click)'],
    ['X / Square', 'Action 1'], ['Y / Triangle', 'Action 2'], ['LB / L1', 'Nearest enemy'], ['RB / R1', 'Next enemy'],
    ['LT / L2 + face buttons', 'Actions 3 to 6'], ['D-pad', 'Actions 7 to 10'], ['Start / Options', 'Game menu'], ['Back / Share', 'Bags']];
  function padMap() {
    const m = wenilla.gamepad?.mapping;
    if (Array.isArray(m) && m.length && m.every((r) => Array.isArray(r) && r.length >= 2)) return m.map((r) => [String(r[0]), String(r[1])]);
    if (m && typeof m === 'object' && !Array.isArray(m) && Object.keys(m).length) return Object.entries(m).map(([k, v]) => [k, String(v)]);
    return PAD_MAP;
  }
  function padCard(pad) {
    const name = String(pad?.id || '').replace(/\s*\(.*$/, '').slice(0, 48);
    const rows = padMap().map(([b, w]) => `<tr><td>${esc(b)}</td><td>${esc(w)}</td></tr>`).join('');
    card(`<b>Controller connected</b>\n${name ? esc(name) + ': ' : ''}touch controls are hidden while it plays. Unplug it (or switch it off) for touch again.` +
      `<table class="vs">${rows}</table>`, [['Got it']]);
    status.padCard = (status.padCard || 0) + 1;
  }
  addEventListener('gamepadconnected', (e) => {
    try { wenilla.gamepad?.start?.(); } catch { /* mapper missing */ }
    if (!status.on) return;
    turnOff();
    status.byPad = true;
    root.classList.add('padmode');
    if (LS('ow:pad-card') !== '1') { LS('ow:pad-card', '1'); padCard(e.gamepad); } else toast('Controller connected: touch controls hidden');
  });
  addEventListener('gamepaddisconnected', () => {
    if (!status.byPad) return;
    status.byPad = false;
    root.classList.remove('padmode');
    info.classList.remove('on');
    if (wanted()) { turnOn(); toast('Controller gone: touch controls are back', 2200); }
  });

  if (wanted()) { if (LS('ow:scheme') === 'pc') root.classList.add('pcmode'); else turnOn(); }
  // ?taptest=1: straight into the tap test (a link we can send a player)
  if (/[?&]taptest=1/.test(location.search) && status.on) setTimeout(() => tapTest.start(), 600);
  return status;
}
