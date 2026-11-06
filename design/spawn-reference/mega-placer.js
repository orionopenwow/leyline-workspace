// 1editor1/mega-placer.js: EXPERIMENT. One behavior that does the editor's core hands in a single file:
//   1. cursor over terrain: a ring sits where the mouse meets the ground (or a floor/deck), every frame
//   2. place: with a piece armed (player.state.mp.arm = a spec), a left click drops a copy on the ring, one commit = one undo
//   3. move: with nothing armed, press a placed piece and drag: it rides the ground under the cursor, one commit when you let go
//   4. turn: mpTurnL / mpTurnR spin the piece under the cursor (or the one you're dragging) 15° a press
// Written fresh for study (2026-11-05); the live editor still runs from scripts/build-tool-core.js, ed-gizmo-core.js and
// terrain-editor-core.js. Not bound to anything yet. To try it: add "scripts/mega-placer.js" (copy it there) to the player
// template's behaviors, and declare in world.config.yaml inputs.actions:  mpTurnL: { keys: [z] }  mpTurnR: { keys: [x] }
// (left click is the existing "select" action). Editors only.
import { raycast, getInputRay } from 'builtin/physics';

const MP = {
  reach: 220,          // m, how far the cursor ray looks (same as the Build palette)
  floorNormal: 0.6,    // a hit facing up more than this is a floor you can stand things on; steeper falls to the terrain below
  turnStep: 15,        // degrees per turn press
  ringRadius: 0.6,     // m, the cursor ring
  snap: 0.05,          // m, positions rounded to this so files stay tidy
  ignoreTags: ['mp-ring', 'bld-ghost', 'te-ret', 'player'],
};
const snap = (v) => Math.round(v / MP.snap) * MP.snap;
const yawOf = (r) => typeof r === 'number' ? r : (r && typeof r.yaw === 'number' ? r.yaw : 0);

// ---- 1. the cursor's point on the ground ------------------------------------------------------------
// The engine hands the pointer's ray (getInputRay); raycast finds the first static thing it meets. A wall or a
// cliff face isn't a place to stand a barrel, so a steep hit drops to the terrain height under it.
function aim(ctx, input, ignore = []) {
  const r = getInputRay(ctx, input);
  if (!r || r.source !== 'pointer') return null;
  const h = raycast(ctx, r.origin, r.direction, {
    distance: MP.reach, bodies: 'all', hits: 'both',
    ignoreEntities: [ctx.self.id, ...ignore], excludeTags: MP.ignoreTags,
  });
  if (!h) return null;
  let y = h.position.y;
  if (!(h.normal?.y > MP.floorNormal)) {
    const t = ctx.place.terrain?.heightAt?.(h.position.x, h.position.z);
    if (typeof t === 'number' && isFinite(t)) y = t;
  }
  return { x: snap(h.position.x), y: snap(y), z: snap(h.position.z), id: h.id ?? null, up: h.normal?.y ?? 1 };
}

// what a hit id belongs to: the topmost object of that thing (a cottage, not its door leaf); terrain and players never
function pickable(ctx, hid) {
  const raw = String(hid ?? '');
  if (!raw || raw.startsWith('terrain/')) return null;
  const root = raw.split('__')[0].split('/')[0];
  if (root === ctx.self.id || ctx.place.players.some((p) => p.id === root)) return null;
  const o = ctx.getObject(root);
  if (!o || o.tags?.some?.((t) => MP.ignoreTags.includes(t))) return null;
  return root;
}

// ---- the ring: one local-only object, so nobody else sees your cursor -----------------------------------
function ring(ctx, p, color) {
  const S = ctx.session.mp;
  if (!p) { if (S.ring && ctx.getObject(S.ring)) ctx.getObject(S.ring).visible = false; return; }
  if (!S.ring || !ctx.getObject(S.ring)) {
    S.ring = ctx.spawn({
      tags: ['mp-ring'], audience: 'local', scope: 'session', physics: 'none', castShadow: false,
      primitive: { kind: 'torus', radius: MP.ringRadius, tube: 0.04, radialSegments: 6, tubularSegments: 32 },
      rotation: { yaw: 0, pitch: 90, roll: 0 },
      material: { color, emissive: color, emissiveIntensity: 1.2 },
      feetPosition: { x: p.x, y: p.y + 0.05, z: p.z },
    });
    return;
  }
  const o = ctx.getObject(S.ring);
  o.visible = true;
  o.feetPosition = { x: p.x, y: p.y + 0.05, z: p.z };
  if (S.ringColor !== color) { o.material = { color, emissive: color, emissiveIntensity: 1.2 }; S.ringColor = color; }
}

// ---- 2. place ----------------------------------------------------------------------------------------
function place(ctx, spec, p, yaw) {
  let id = null;
  ctx.commit(`Placed ${spec.state?.label ?? 'piece'}`, () => {
    id = ctx.spawn({
      ...spec,
      tags: [...new Set([...(spec.tags ?? []), 'placed'])],
      feetPosition: { x: p.x, y: p.y, z: p.z },
      rotation: { yaw },
    });
  });
  return id;
}

// ---- 3. move: the drag lives in session; only the drop writes the file -----------------------------------
function dragStart(ctx, id, p) {
  const o = ctx.getObject(id); if (!o) return;
  const f = o.worldFeetPosition ?? o.feetPosition;
  ctx.session.mp.drag = {
    id, from: { x: f.x, y: f.y, z: f.z }, yaw0: yawOf(o.rotation),
    off: { x: f.x - p.x, z: f.z - p.z },          // grab point kept, so the piece doesn't jump to the cursor
    lift: f.y - p.y,                               // its height above the ground it stood on, kept while it rides
  };
}
function dragMove(ctx, p) {
  const d = ctx.session.mp.drag, o = d && ctx.getObject(d.id); if (!o || !p) return;
  o.feetPosition = { x: snap(p.x + d.off.x), y: snap(p.y + d.lift), z: snap(p.z + d.off.z) };   // live, this machine's picture
}
function dragEnd(ctx) {
  const d = ctx.session.mp.drag; ctx.session.mp.drag = null;
  const o = d && ctx.getObject(d.id); if (!o) return;
  const to = o.feetPosition, rot = o.rotation;
  // put it back, then write the end inside one commit, so the undo returns it to where the press began
  o.feetPosition = d.from; o.rotation = { yaw: d.yaw0 };
  ctx.commit('Moved piece', () => { o.feetPosition = to; o.rotation = rot; });
}

// ---- 4. turn ------------------------------------------------------------------------------------------
function turn(ctx, id, deg) {
  const o = ctx.getObject(id); if (!o) return;
  const yaw = (yawOf(o.rotation) + deg + 360) % 360;
  if (ctx.session.mp.drag?.id === id) { o.rotation = { yaw }; return; }   // mid-drag: lands with the drop
  ctx.commit('Turned piece', () => { o.rotation = { yaw }; });
}

// ---- the hands ----------------------------------------------------------------------------------------
export function onInput(ctx, input) {
  const me = ctx.self;
  if (!me.isEditor) return;
  const S = (ctx.session.mp ??= { ring: null, drag: null, yaw: 0 });
  const arm = me.state.mp?.arm ?? null;                       // a spec to place, set by a palette (or run_script) 
  const dragging = S.drag?.id;
  const p = aim(ctx, input, dragging ? [dragging] : []);
  const over = !dragging && p ? pickable(ctx, p.id) : null;

  // ring colour says what a click will do: gold = place, teal = grab, grey = nothing
  ring(ctx, p, arm ? 'oklch(0.85 0.15 85)' : over ? 'oklch(0.8 0.12 190)' : 'oklch(0.75 0 0 / 0.6)');

  if (input.pressed?.mpTurnL) { if (dragging || over) turn(ctx, dragging || over, -MP.turnStep); else S.yaw = (S.yaw - MP.turnStep + 360) % 360; }
  if (input.pressed?.mpTurnR) { if (dragging || over) turn(ctx, dragging || over, MP.turnStep); else S.yaw = (S.yaw + MP.turnStep) % 360; }

  if (input.pressed?.select && p) {
    if (arm) { place(ctx, arm, p, S.yaw); ctx.emit('playSound', { clip: 'cdn/moodboard-lowpoly-cozy/sfx-place-wood-thunk.mp3', position: p, volume: 0.5 }); }
    else if (over) dragStart(ctx, over, p);
  }
  if (dragging && input.held?.select) dragMove(ctx, p);
  if (dragging && input.released?.select) dragEnd(ctx);
  if (input.pressed?.clear) { if (me.state.mp?.arm) me.state.mp = { ...me.state.mp, arm: null }; }   // Esc disarms
}

// try it from run_script once bound:  getObject(<your player id>).state.mp = { arm: { primitive: { kind: 'box' }, scale: 1, physics: 'static', state: { label: 'crate' } } }
