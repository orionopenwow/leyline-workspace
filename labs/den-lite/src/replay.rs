//! The replay harness (docs/DEN-NEXT.md, "The mouse, proven"): a `.moves` script (movemodel.rs) is played against Den
//! Lite's own editor (gizmo.rs `cmd`, exactly the verbs Gizmo.lua sends: `hover u v aspect zoom`, `click ... carry`,
//! `drag`, `preview <entry>`, `preview turn`, `commit`, `preview off`) on a headless Den, and after EVERY line the copy
//! in hand and the props are compared with the expected model (movemodel.rs). No game, no window, no network: it runs
//! in `cargo test` (Control: den-lite-test, den-lite-all, den-replay) in about a second.
//!
//! A frame fails when the editor and the model disagree on whether something is in hand, on where its copy is (2 cm,
//! a third of a degree), or on where the props are after a commit, Esc or undo. The scripts in `moves/` are the
//! owner's flows (the video's card, click-to-carry, press-and-drag, turning, snapping, the edge); [`fuzz`] plays
//! thousands of random ones (movemodel.rs fuzz_script, seeded: a failure names its seed and line, and replays the same).

use crate::den::{self, Den, WorldObject};
use crate::gizmo::{self, Aids, Camera};
use crate::movemodel::{self, Cmd, MProp, Model};
use crate::ovmod::{Here, Sel};

/// Where they may differ (yards, radians): the Den keeps a prop's place to the millimetre and its turn to a
/// thousandth of a degree.
pub const TOL_AT: f32 = 0.02;
pub const TOL_YAW: f32 = 0.006;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Report {
    pub frames: usize,
    pub checked: usize,
    pub failures: Vec<String>,
}

impl Report {
    pub fn ok(&self) -> bool {
        self.failures.is_empty()
    }
    pub fn line(&self) -> String {
        format!("{} frames, {} compared, {} failed{}", self.frames, self.checked, self.failures.len(), self.failures.first().map(|f| format!(": {f}")).unwrap_or_default())
    }
}

fn turn_diff(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(std::f32::consts::TAU);
    d.min(std::f32::consts::TAU - d)
}

/// The model's view of a prop: its pick box and footprint, from the Den's own geometry for its model and size
/// (gizmo.rs body_of / world_extent), at the given place. Only sizes come from the Den; where it is comes from the model.
fn mprop(id: u64, entry: u32, display: u32, scale: f32, at: [f32; 4]) -> MProp {
    let o = WorldObject { guid: id, entry, display, kind: 5, scale, x: at[0], y: at[1], z: at[2], o: at[3], pitch: 0.0, roll: 0.0 };
    let b = gizmo::body_of(&o);
    let (hx, hy, _) = gizmo::world_extent(&o);
    MProp { id, at: [at[0], at[1], at[2]], o: at[3], half: b.half, bottom: b.bottom, height: b.height, ext: [hx, hy] }
}

fn entry_of(word: &str) -> Option<(u32, u32, f32)> {
    den::find_objects(word).into_iter().next().map(|o| (o.0, o.2, o.4))
}

/// Plays a script; the report says every frame that disagreed.
pub fn run(text: &str) -> Report {
    let mut r = Report::default();
    let cmds = match movemodel::parse_script(text) {
        Ok(c) => c,
        Err(e) => {
            r.failures.push(e);
            return r;
        }
    };
    let mut d = Den::new(den::Spot::default());
    let spot = d.spot;
    let mut sel = Sel::default();
    let mut a = Aids::new(Camera::default());
    let mut m = Model::new([spot.x, spot.y], spot.z, spot.reach());
    let mut here = Here { x: spot.x, y: spot.y, z: spot.z, o: 0.0 };
    m.me = [here.x, here.y, here.z, here.o];
    let mut now: u64 = 1_000;
    let mut zoom: Option<f32> = None;
    // the model's own undo history: the props before each change that is one undo step
    let mut history: Vec<Vec<MProp>> = Vec::new();
    // the entry of the card in hand (for the new prop's box)
    let mut card: Option<(u32, u32, f32)> = None;
    let mut next_id: u64 = 1;
    // each model prop's entry, display and size: its box and turned footprint are made again wherever it is put down
    // (the footprint over the world's axes turns with it: a carry's offset is held within it)
    let mut kinds: std::collections::HashMap<u64, (u32, u32, f32)> = std::collections::HashMap::new();
    for (line, c) in cmds {
        r.frames += 1;
        now += 40;
        let tail = |zoom: Option<f32>, aspect: f32| match zoom {
            Some(z) => format!("{aspect} {z}"),
            None => format!("{aspect}"),
        };
        let fail = |r: &mut Report, what: String| r.failures.push(format!("line {line}: {what}"));
        match &c {
            Cmd::Cam(cam) => {
                a.cam = Camera { fov: cam.fov, pivot: cam.pivot, dist: cam.dist, pitch: cam.pitch, far_pivot: 0.0 };
                m.cam = *cam;
            }
            Cmd::At(x, y, o) => {
                here = Here { x: spot.x + x, y: spot.y + y, z: d.floor_at(spot.x + x, spot.y + y), o: o.to_radians() };
                m.me = [here.x, here.y, here.z, here.o];
            }
            Cmd::Grid(g) => {
                sel.grid = *g as f64;
                m.grid = *g;
            }
            Cmd::Zoom(z) => {
                zoom = Some(*z);
                m.zoom = Some(*z);
            }
            Cmd::Aspect(x) => m.aspect = *x,
            Cmd::Prop(word, x, y, o) => {
                let Some((entry, display, _)) = entry_of(word) else {
                    fail(&mut r, format!("no prop called {word}"));
                    continue;
                };
                let (wx, wy) = (spot.x + x, spot.y + y);
                match d.spawn(entry, wx, wy, d.floor_at(wx, wy), o.to_radians()) {
                    Ok(obj) => {
                        history.push(m.props.clone());
                        m.props.push(mprop(next_id, entry, display, obj.scale, [obj.x, obj.y, obj.z, obj.o]));
                        kinds.insert(next_id, (entry, display, obj.scale));
                        next_id += 1;
                    }
                    Err(e) => fail(&mut r, format!("setup prop {word}: {e}")),
                }
            }
            Cmd::Mode(b) => {
                gizmo::cmd(&mut d, &mut sel, &mut a, here, if *b { "mode build" } else { "mode play" }, true, now);
                if !*b {
                    m.cancel();
                }
            }
            Cmd::Card(word, uv) => {
                let Some(e) = entry_of(word) else {
                    fail(&mut r, format!("no prop called {word}"));
                    continue;
                };
                card = Some(e);
                let text = match uv {
                    Some([u, v]) => format!("preview {} {u} {v} {}", e.0, tail(zoom, m.aspect)),
                    None => format!("preview {}", e.0),
                };
                gizmo::cmd(&mut d, &mut sel, &mut a, here, &text, true, now);
                m.card(*uv);
            }
            Cmd::Hover(u, v) => {
                gizmo::cmd(&mut d, &mut sel, &mut a, here, &format!("hover {u} {v} {}", tail(zoom, m.aspect)), true, now);
                m.hover(*u, *v);
            }
            Cmd::Click(u, v, carry) => {
                let text = format!("click {u} {v} {}{}", tail(zoom, m.aspect), if *carry { " carry" } else { "" });
                gizmo::cmd(&mut d, &mut sel, &mut a, here, &text, true, now);
                if *carry {
                    m.grab(*u, *v, true);
                }
            }
            Cmd::Drag(u, v) => {
                gizmo::cmd(&mut d, &mut sel, &mut a, here, &format!("drag {u} {v} {}", tail(zoom, m.aspect)), true, now);
                m.grab(*u, *v, false);
            }
            Cmd::Turn(deg) => {
                gizmo::cmd(&mut d, &mut sel, &mut a, here, &format!("preview turn {deg}"), true, now);
                m.turn(*deg);
            }
            Cmd::Commit => {
                let before = m.props.clone();
                let moving = match m.armed {
                    Some(movemodel::MArmed::Move { id, .. }) => m.props.iter().find(|p| p.id == id).copied(),
                    _ => None,
                };
                gizmo::cmd(&mut d, &mut sel, &mut a, here, "commit", true, now);
                let made = m.commit(next_id, |at| match (moving, card) {
                    (Some(p), _) => match kinds.get(&p.id) {
                        Some(&(entry, display, size)) => mprop(p.id, entry, display, size, at),
                        None => MProp { at: [at[0], at[1], at[2]], o: at[3], ..p },
                    },
                    (None, Some((entry, display, size))) => mprop(0, entry, display, size, at),
                    (None, None) => mprop(0, 0, 0, 1.0, at),
                });
                if made.is_some() {
                    history.push(before);
                    if made == Some(next_id) {
                        if let Some(c) = card.filter(|_| moving.is_none()) {
                            kinds.insert(next_id, c);
                        }
                        next_id += 1;
                    }
                }
            }
            Cmd::Esc => {
                gizmo::cmd(&mut d, &mut sel, &mut a, here, "preview off", true, now);
                m.cancel();
            }
            Cmd::Undo => {
                let ch = d.undo();
                if let Some(h) = history.pop() {
                    m.props = h;
                } else if !ch.created.is_empty() || !ch.destroyed.is_empty() {
                    fail(&mut r, "undo changed the Den with nothing to undo in the model".into());
                }
            }
            Cmd::Wait(ms) => now += ms,
            Cmd::ExpectGhost(p, deg, tol) => {
                r.checked += 1;
                match a.at.filter(|_| a.armed.is_some()) {
                    None => fail(&mut r, "expected a copy in hand, there is none".into()),
                    Some(at) => {
                        let want = [spot.x + p[0], spot.y + p[1], spot.z + p[2]];
                        if (0..3).any(|k| (at[k] - want[k]).abs() > *tol) || deg.is_some_and(|g| turn_diff(at[3], g.to_radians()) > TOL_YAW.max(tol.to_radians())) {
                            fail(&mut r, format!("the copy is at {:.2} {:.2} {:.2} {:.1}, expected {p:?} {deg:?}", at[0] - spot.x, at[1] - spot.y, at[2] - spot.z, at[3].to_degrees()));
                        }
                    }
                }
            }
            Cmd::ExpectProp(which, p, deg, tol) => {
                r.checked += 1;
                let o = match which {
                    None => d.props.last(),
                    Some(k) => d.props.get(k - 1),
                }
                .map(|x| x.obj.clone());
                match o {
                    None => fail(&mut r, format!("no prop {which:?}")),
                    Some(o) => {
                        let got = [o.x - spot.x, o.y - spot.y, o.z - spot.z];
                        if (0..3).any(|k| (got[k] - p[k]).abs() > *tol) || deg.is_some_and(|g| turn_diff(o.o, g.to_radians()) > TOL_YAW.max(tol.to_radians())) {
                            fail(&mut r, format!("prop {which:?} is at {:.2} {:.2} {:.2} {:.1}, expected {p:?} {deg:?}", got[0], got[1], got[2], o.o.to_degrees()));
                        }
                    }
                }
            }
            Cmd::ExpectProps(n) => {
                r.checked += 1;
                if d.props.len() != *n {
                    fail(&mut r, format!("{} props, expected {n}", d.props.len()));
                }
            }
            Cmd::ExpectNothing => {
                r.checked += 1;
                if a.armed.is_some() {
                    fail(&mut r, "something is still in hand".into());
                }
            }
        }
        // every frame: the editor against the model
        r.checked += 1;
        if a.armed.is_some() != m.armed.is_some() {
            fail(&mut r, format!("in hand: the editor {}, the model {}", a.armed.is_some(), m.armed.is_some()));
            m.armed = None;
            m.at = None;
            continue;
        }
        match (a.at.filter(|_| a.armed.is_some()), m.at.filter(|_| m.armed.is_some())) {
            (Some(g), Some(w)) => {
                let off = ((g[0] - w[0]).powi(2) + (g[1] - w[1]).powi(2) + (g[2] - w[2]).powi(2)).sqrt();
                if off > TOL_AT || turn_diff(g[3], w[3]) > TOL_YAW {
                    fail(&mut r, format!("the copy: the editor {:.3} {:.3} {:.3} {:.1}, the model {:.3} {:.3} {:.3} {:.1} ({off:.3} yd apart)", g[0] - spot.x, g[1] - spot.y, g[2] - spot.z, g[3].to_degrees(), w[0] - spot.x, w[1] - spot.y, w[2] - spot.z, w[3].to_degrees()));
                    m.at = Some(g);
                }
            }
            (None, None) => {}
            (g, w) => fail(&mut r, format!("the copy shown: the editor {}, the model {}", g.is_some(), w.is_some())),
        }
        // the props: the same places (in any order)
        let mut left: Vec<[f32; 4]> = d.props.iter().map(|p| [p.obj.x, p.obj.y, p.obj.z, p.obj.o]).collect();
        if left.len() != m.props.len() {
            fail(&mut r, format!("{} props in the Den, {} in the model", left.len(), m.props.len()));
            continue;
        }
        for p in &m.props {
            let k = left.iter().position(|q| ((q[0] - p.at[0]).powi(2) + (q[1] - p.at[1]).powi(2) + (q[2] - p.at[2]).powi(2)).sqrt() <= TOL_AT && turn_diff(q[3], p.o) <= TOL_YAW);
            match k {
                Some(k) => {
                    left.remove(k);
                }
                None => {
                    fail(&mut r, format!("the model's prop at {:.2} {:.2} {:.2} isn't in the Den", p.at[0] - spot.x, p.at[1] - spot.y, p.at[2] - spot.z));
                    break;
                }
            }
        }
    }
    r
}

/// Random scripts from `seeds`: the first failure of each (its seed and line).
pub fn fuzz(seeds: std::ops::Range<u64>, steps: usize) -> Vec<String> {
    let mut out = Vec::new();
    for s in seeds {
        let script = format!("prop table 4 0\nprop chair -3 5 face 40\n{}", movemodel::fuzz_script(s, steps, &["chair", "lantern", "barrel"]));
        let r = run(&script);
        if let Some(f) = r.failures.first() {
            out.push(format!("seed {s}: {f}"));
        }
    }
    out
}

/// The owner's flows (labs/den-lite/moves/*.moves), built in so `cargo test` needs no files.
pub const SCRIPTS: &[(&str, &str)] = &[
    ("card-glides", include_str!("../moves/card-glides.moves")),
    ("carry-and-drop", include_str!("../moves/carry-and-drop.moves")),
    ("press-and-drag", include_str!("../moves/press-and-drag.moves")),
    ("turn-snap-edge", include_str!("../moves/turn-snap-edge.moves")),
    ("undo-and-esc", include_str!("../moves/undo-and-esc.moves")),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_owners_flows_match_the_model_frame_by_frame() {
        for (name, text) in SCRIPTS {
            let r = run(text);
            assert!(r.ok(), "{name}: {}\n{}", r.line(), r.failures.join("\n"));
            assert!(r.checked > r.frames, "{name}: every frame compared");
        }
    }

    #[test]
    fn a_wrong_expectation_is_caught() {
        // the harness itself: a script that expects the wrong spot fails on that line
        let r = run("mode build\ncard chair 0 -0.3\nexpect ghost 40 40 0\n");
        assert!(!r.ok() && r.failures[0].starts_with("line 3"), "{:?}", r.failures);
    }

    #[test]
    fn random_scripts_match_the_model() {
        let fails = fuzz(0..300, 120);
        assert!(fails.is_empty(), "{} of 300 seeds disagreed:\n{}", fails.len(), fails.iter().take(10).cloned().collect::<Vec<_>>().join("\n"));
    }
}
