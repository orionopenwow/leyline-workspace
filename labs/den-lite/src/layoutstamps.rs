//! Your Den's layout kept as stamps (owner 5 Nov: "remember the stamps and how things are placed and a few dressed
//! buildings and areas as stamps for testing"), before the den-test empties the Den (`.den clear`, world.rs):
//!
//! * one stamp of the whole layout ("My Den 5 Nov (whole layout)");
//! * a few dressed groups: each big piece (a tent, a building: a footprint of [`BIG`] yards or more) with the small
//!   props that sit round it (within its half width plus [`NEAR`] yards: its lanterns, banners, fences), named by the
//!   main piece ("Tent with lanterns"); small props standing together with no big piece near (at most [`LINK`] yards
//!   apart, three or more) make a group named by their most common piece ("Banner row", "Lantern group").
//!
//! Each piece keeps its exact place relative to the group (x, y from the group's middle, z from its lowest foot), its
//! turn, tilt, roll and size, as `stamp save` does (buildtools.rs `save_stamp`). They go in `<dens>/stamps-mine` (the
//! gallery's Stamps tab, "Mine"), credited to the Den's owner. Done once: a group whose content is already a stamp
//! (the same `layout_hash`) is left as it is, so running it again writes nothing.

use crate::den::{self, Den, DenDoc, Placed, Prop};
use serde_json::{json, Map, Value};
use std::path::Path;

/// A piece this wide or deep (yards, at its size) is a group's main piece.
pub const BIG: f32 = 4.0;
/// How far round a main piece's edge a small prop still belongs to it (yards).
pub const NEAR: f32 = 3.0;
/// Small props at most this far apart stand together (yards).
pub const LINK: f32 = 2.5;
/// At most this many dressed groups (the biggest first).
pub const GROUPS_MAX: usize = 8;

/// One stamp from the layout: its name, its title, how many props, and whether it was written now (false: already
/// there with the same content).
#[derive(Debug, Clone, PartialEq)]
pub struct Made {
    pub name: String,
    pub title: String,
    pub props: usize,
    pub new: bool,
}

/// A piece's footprint (the larger of its width and depth, yards, at its size).
fn footprint(p: &Prop) -> f32 {
    let s = crate::stamps::size(p.entry as u32, p.scale.map(|v| v as f32));
    s.w.max(s.d)
}

fn name_of(p: &Prop) -> &'static str {
    den::template(p.entry as u32).map(|t| t.name).unwrap_or("Prop")
}

/// The groups: (main piece or None, the indexes of its pieces, the main piece first). Pure, on (x, y, footprint).
pub fn clusters(pts: &[([f64; 2], f32)]) -> Vec<(Option<usize>, Vec<usize>)> {
    let d = |a: [f64; 2], b: [f64; 2]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt() as f32;
    let big: Vec<usize> = (0..pts.len()).filter(|i| pts[*i].1 >= BIG).collect();
    let mut owner: Vec<Option<usize>> = vec![None; pts.len()];
    for (i, p) in pts.iter().enumerate() {
        if p.1 >= BIG {
            continue;
        }
        // the nearest big piece whose reach covers it (its half width plus NEAR)
        owner[i] = big
            .iter()
            .copied()
            .filter(|b| d(pts[*b].0, p.0) <= pts[*b].1 / 2.0 + NEAR)
            .min_by(|a, b| d(pts[*a].0, p.0).partial_cmp(&d(pts[*b].0, p.0)).unwrap_or(std::cmp::Ordering::Equal));
    }
    let mut out: Vec<(Option<usize>, Vec<usize>)> = Vec::new();
    for b in &big {
        let mut v = vec![*b];
        v.extend((0..pts.len()).filter(|i| owner[*i] == Some(*b)));
        if v.len() > 1 {
            out.push((Some(*b), v));
        }
    }
    // the rest: small props standing together (single link), three or more
    let rest: Vec<usize> = (0..pts.len()).filter(|i| pts[*i].1 < BIG && owner[*i].is_none()).collect();
    let mut seen = vec![false; pts.len()];
    for &s in &rest {
        if seen[s] {
            continue;
        }
        seen[s] = true;
        let mut group = vec![s];
        let mut k = 0;
        while k < group.len() {
            let at = pts[group[k]].0;
            for &o in &rest {
                if !seen[o] && d(at, pts[o].0) <= LINK {
                    seen[o] = true;
                    group.push(o);
                }
            }
            k += 1;
        }
        if group.len() >= 3 {
            group.sort_unstable();
            out.push((None, group));
        }
    }
    // the biggest groups first; a few only
    out.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.1[0].cmp(&b.1[0])));
    out.truncate(GROUPS_MAX);
    out
}

/// A plain word's plural ("lantern" -> "lanterns", "bench" -> "benches").
fn plural(w: &str) -> String {
    let l = w.to_lowercase();
    if l.ends_with('s') || l.ends_with("sh") || l.ends_with("ch") || l.ends_with('x') {
        format!("{l}es")
    } else if l.ends_with('y') && !l.ends_with("ey") && !l.ends_with("ay") && !l.ends_with("oy") {
        format!("{}ies", &l[..l.len() - 1])
    } else {
        format!("{l}s")
    }
}

/// The last word of a piece's name ("Wooden Chair" -> "Chair"), what a title calls it.
fn short(name: &str) -> &str {
    name.split_whitespace().last().unwrap_or(name)
}

/// The most common name among the pieces, how many.
fn most_common<'a>(names: &[&'a str]) -> Option<(&'a str, usize)> {
    let mut best: Option<(&str, usize)> = None;
    for n in names {
        let c = names.iter().filter(|m| *m == n).count();
        if best.map_or(true, |b| c > b.1 || (c == b.1 && *n < b.0)) {
            best = Some((*n, c));
        }
    }
    best
}

/// A group's title: "Tent with lanterns", "Banner row", "Lantern group" (at most 30 letters).
pub fn title_of(main: Option<&str>, others: &[&str], in_a_line: bool) -> String {
    let t = match (main, most_common(others)) {
        (Some(m), Some((o, _))) => format!("{} with {}", short(m), plural(short(o))),
        (Some(m), None) => short(m).to_string(),
        (None, Some((o, _))) => format!("{} {}", short(o), if in_a_line { "row" } else { "group" }),
        (None, None) => "Group".into(),
    };
    let mut c = t.chars();
    let first = c.next().map(|f| f.to_uppercase().collect::<String>()).unwrap_or_default();
    format!("{first}{}", c.as_str()).chars().take(30).collect()
}

/// Do the points lie along one line (each within 1 yard of the line through the two farthest apart)?
fn in_a_line(pts: &[[f64; 2]]) -> bool {
    if pts.len() < 3 {
        return false;
    }
    let mut far = (0, 1, -1.0f64);
    for i in 0..pts.len() {
        for j in i + 1..pts.len() {
            let dd = (pts[i][0] - pts[j][0]).powi(2) + (pts[i][1] - pts[j][1]).powi(2);
            if dd > far.2 {
                far = (i, j, dd);
            }
        }
    }
    let (a, b, len2) = far;
    if len2 < 1e-6 {
        return false;
    }
    let (ax, ay, bx, by) = (pts[a][0], pts[a][1], pts[b][0], pts[b][1]);
    let len = len2.sqrt();
    pts.iter().all(|p| ((bx - ax) * (ay - p[1]) - (ax - p[0]) * (by - ay)).abs() / len <= 1.0)
}

/// The pieces made a stamp: x, y from their middle, z from the lowest foot (`stamp save`'s `centre_of`), each
/// otherwise as it is (turn, tilt, roll, size, model, Solid; not its group, lock or role: buildtools.rs `copy_of`).
pub fn relative(props: &[&Prop]) -> Vec<Prop> {
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for p in props {
        for (k, v) in [p.x, p.y, p.z].into_iter().enumerate() {
            lo[k] = lo[k].min(v);
            hi[k] = hi[k].max(v);
        }
    }
    let mid = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, lo[2]];
    props
        .iter()
        .map(|p| {
            let mut c = (*p).clone();
            c.group = None;
            c.locked = None;
            c.role = None;
            c.hidden = None;
            c.x -= mid[0];
            c.y -= mid[1];
            c.z -= mid[2];
            c
        })
        .collect()
}

/// The content's fingerprint (FNV-1a over each piece's entry and numbers at a millimetre, in a fixed order): the
/// same layout gives the same hash on any run.
pub fn hash(props: &[Prop]) -> String {
    let mut lines: Vec<String> = props
        .iter()
        .map(|p| {
            let r = |v: f64| (v * 1000.0).round() as i64;
            format!("{}:{}:{}:{}:{}:{}:{}:{}", p.entry, r(p.x), r(p.y), r(p.z), r(p.yaw), r(p.pitch.unwrap_or(0.0)), r(p.roll.unwrap_or(0.0)), r(p.scale.unwrap_or(0.0)))
        })
        .collect();
    lines.sort();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in lines.join(";").bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// The hashes of the stamps already in `<dens>/stamps-mine`.
fn hashes_there(dir: &Path) -> Vec<String> {
    den::list_names(dir)
        .into_iter()
        .filter_map(|n| den::read_file(&dir.join(format!("{n}.den.json"))).ok())
        .filter_map(|d| d.extra.get("layout_hash").and_then(Value::as_str).map(str::to_string))
        .collect()
}

/// A stamp name from a title: lower case letters and digits joined by '-', at most 24 with `-<n>` free.
fn slug(prefix: &str, title: &str) -> String {
    let mut s = String::from(prefix);
    for w in title.to_lowercase().split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()) {
        if s.len() + 1 + w.len() > 20 {
            break;
        }
        if !s.is_empty() {
            s.push('-');
        }
        s.push_str(w);
    }
    s
}

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// The Den's layout as stamps in `<dens>/stamps-mine` (see the module's note). `who`: the credit; `unix`: now.
pub fn from_layout(d: &Den, dens: &Path, who: &str, unix: u64) -> Result<Vec<Made>, String> {
    let placed: Vec<&Placed> = d.props.iter().filter(|p| p.prop.hidden != Some(true)).collect();
    if placed.is_empty() {
        return Err("Your Den has no props to keep as stamps.".into());
    }
    let dir = dens.join(den::MINE_DIR);
    let mut known = hashes_there(&dir);
    let (_, mo, day, ..) = den::civil(unix);
    let when = format!("{day} {}", MONTHS[(mo as usize).saturating_sub(1).min(11)]);
    let mut out = Vec::new();
    // the whole layout (when it fits a stamp: den.rs STAMP_SPAN from its middle, LOCAL_MAX pieces)
    let all: Vec<&Prop> = placed.iter().map(|p| &p.prop).collect();
    let whole = relative(&all);
    if whole.len() <= den::LOCAL_MAX && whole.iter().all(|p| p.x.abs() <= den::STAMP_SPAN && p.y.abs() <= den::STAMP_SPAN && p.z.abs() <= den::STAMP_SPAN) {
        let title = format!("My Den {when} (whole layout)");
        let name = format!("my-den-{}-{}", day, MONTHS[(mo as usize).saturating_sub(1).min(11)].to_lowercase());
        out.push(write(d, &dir, &name, &title, whole, who, unix, &mut known)?);
    }
    // the dressed groups
    let pts: Vec<([f64; 2], f32)> = all.iter().map(|p| ([p.x, p.y], footprint(p))).collect();
    let mut used: Vec<String> = out.iter().map(|m| m.name.clone()).collect();
    for (main, idx) in clusters(&pts) {
        let names: Vec<&str> = idx.iter().filter(|i| Some(**i) != main).map(|i| name_of(all[*i])).collect();
        let line = main.is_none() && in_a_line(&idx.iter().map(|i| pts[*i].0).collect::<Vec<_>>());
        let title = title_of(main.map(|m| name_of(all[m])), &names, line);
        let base = slug("", &title);
        let base = if base.is_empty() { "group".to_string() } else { base };
        let mut name = base.clone();
        let mut n = 2;
        while used.contains(&name) {
            name = format!("{base}-{n}");
            n += 1;
        }
        used.push(name.clone());
        let pieces: Vec<&Prop> = idx.iter().map(|i| all[*i]).collect();
        out.push(write(d, &dir, &name, &title, relative(&pieces), who, unix, &mut known)?);
    }
    Ok(out)
}

/// One stamp written (`<dir>/<name>.den.json`, as `stamp save` writes it), unless one with the same content is there.
#[allow(clippy::too_many_arguments)]
fn write(d: &Den, dir: &Path, name: &str, title: &str, props: Vec<Prop>, who: &str, unix: u64, known: &mut Vec<String>) -> Result<Made, String> {
    let h = hash(&props);
    let n = props.len();
    if known.contains(&h) {
        return Ok(Made { name: name.into(), title: title.into(), props: n, new: false });
    }
    // another stamp of that name with other content: the next free name (never overwritten)
    let mut name = name.to_string();
    let mut k = 2;
    while dir.join(format!("{name}.den.json")).is_file() {
        name = format!("{}-{k}", name.trim_end_matches(|c: char| c.is_ascii_digit() || c == '-').chars().take(20).collect::<String>());
        k += 1;
    }
    let mut extra = Map::new();
    extra.insert("category".into(), json!("mine"));
    extra.insert("credit".into(), json!({ "by": who.chars().filter(|c| c.is_alphanumeric() || " '-_.".contains(*c)).take(24).collect::<String>(), "made": den::iso_utc(unix) }));
    extra.insert("layout_hash".into(), json!(h));
    extra.insert("from_layout".into(), json!(true));
    // a shared piece's credit index points into this list (den.rs credit_map)
    if props.iter().any(|p| p.credit.is_some()) {
        if let Some(c) = d.extra.get("credits") {
            extra.insert("credits".into(), c.clone());
        }
    }
    let doc = DenDoc {
        format: den::FORMAT.into(),
        version: den::VERSION,
        kind: Some("stamp".into()),
        name: Some(name.clone()),
        title: Some(title.into()),
        saved: Some(den::iso_utc(unix)),
        props,
        extra,
        ..Default::default()
    };
    let text = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{name}.den.json"));
    let tmp = dir.join(format!(".{name}.den.json.tmp"));
    std::fs::create_dir_all(dir).and_then(|_| std::fs::write(&tmp, text.as_bytes())).and_then(|_| std::fs::rename(&tmp, &path)).map_err(|_| "A stamp can't be written in your Dens folder.".to_string())?;
    known.push(h);
    Ok(Made { name, title: title.into(), props: n, new: true })
}

/// The answer's line: "Kept 4 stamps from your layout (Stamps > Mine): ...; 2 were already there."
pub fn line(made: &[Made]) -> String {
    let new: Vec<String> = made.iter().filter(|m| m.new).map(|m| format!("{} ({})", m.title, m.props)).collect();
    let old = made.iter().filter(|m| !m.new).count();
    let mut s = if new.is_empty() { "Your layout's stamps are already kept (Stamps > Mine).".to_string() } else { format!("Kept {} stamps from your layout (Stamps > Mine): {}.", new.len(), new.join(", ")) };
    if old > 0 && !new.is_empty() {
        s.push_str(&format!(" {old} were already there."));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::den::Spot;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("den-lite-layoutstamps-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn big_pieces_take_the_small_ones_round_them_and_small_ones_together_make_a_group() {
        // a tent (6 yd) with two lanterns by it; three banners in a row away from it; one lone crate
        let pts = vec![
            ([0.0, 0.0], 6.0),
            ([4.0, 0.5], 0.5),
            ([-3.5, 1.0], 0.5),
            ([20.0, 0.0], 1.0),
            ([22.0, 0.0], 1.0),
            ([24.0, 0.0], 1.0),
            ([10.0, 15.0], 1.0),
        ];
        let c = clusters(&pts);
        assert_eq!(c.len(), 2, "{c:?}");
        assert!(c.contains(&(Some(0), vec![0, 1, 2])));
        assert!(c.contains(&(None, vec![3, 4, 5])));
        assert!(in_a_line(&[[20.0, 0.0], [22.0, 0.0], [24.0, 0.3]]) && !in_a_line(&[[0.0, 0.0], [2.0, 0.0], [1.0, 2.0]]));
        assert_eq!(title_of(Some("Large Tent"), &["Lantern", "Lantern", "Banner"], false), "Tent with lanterns");
        assert_eq!(title_of(None, &["Red Banner", "Red Banner", "Red Banner"], true), "Banner row");
        assert_eq!(title_of(None, &["Bench", "Bench", "Crate"], false), "Bench group");
        assert_eq!(plural("Bench"), "benches");
    }

    #[test]
    fn the_layout_is_kept_exactly_once_and_its_stamps_place_where_they_stood() {
        let dens = tmp("once");
        let mut d = Den::new(Spot::default());
        let s = d.spot;
        // a few props, one turned, one sized, one tilted
        let a = d.spawn(2413, s.x + 2.0, s.y + 1.0, s.z, 0.5).unwrap().guid;
        let b = d.spawn(2413, s.x + 3.5, s.y + 1.0, s.z, 1.0).unwrap().guid;
        let c = d.spawn(2413, s.x + 5.0, s.y + 1.0, s.z, 0.0).unwrap().guid;
        d.edit(b, |p| p.scale = Some(1.3)).unwrap();
        d.edit(c, |p| p.pitch = Some(12.0)).unwrap();
        let made = from_layout(&d, &dens, "Orion", 1_793_923_200).unwrap(); // 2026-11-06T00:00Z
        let whole = made.iter().find(|m| m.title.ends_with("(whole layout)")).expect("the whole layout");
        assert!(whole.new && whole.props == 3 && whole.name.starts_with("my-den-"), "{whole:?}");
        // the row is the whole layout here: the same content, so it isn't written twice
        assert!(made.iter().any(|m| m.title == "Chair row" && m.props == 3 && !m.new), "{made:?}");
        // exact: the pieces' places relative to each other, turn, size and tilt
        let doc = den::read_file(&dens.join(den::MINE_DIR).join(format!("{}.den.json", whole.name))).unwrap();
        let pa = d.placed(a).unwrap().prop.clone();
        let pb = d.placed(b).unwrap().prop.clone();
        let sa = doc.props.iter().find(|p| (p.yaw - pa.yaw).abs() < 1e-9).unwrap();
        let sb = doc.props.iter().find(|p| p.scale == Some(1.3)).unwrap();
        assert!(((sb.x - sa.x) - (pb.x - pa.x)).abs() < 1e-9 && ((sb.y - sa.y) - (pb.y - pa.y)).abs() < 1e-9);
        assert!(doc.props.iter().any(|p| p.pitch == Some(12.0)));
        assert_eq!(doc.extra.get("category").and_then(Value::as_str), Some("mine"));
        assert_eq!(doc.extra.get("credit").and_then(|c| c.get("by")).and_then(Value::as_str), Some("Orion"));
        // again: nothing new written
        let again = from_layout(&d, &dens, "Orion", 1_793_923_300).unwrap();
        assert!(again.iter().all(|m| !m.new), "{again:?}");
        assert_eq!(den::list_names(&dens.join(den::MINE_DIR)).len(), made.iter().filter(|m| m.new).count());
        assert!(line(&again).contains("already kept"));
        // the whole layout as a stamp, in an empty Den: the same pieces, one undo
        let mut e = Den::new(Spot::default());
        let ch = e.stamp(&doc, s.x, s.y, s.z, 0.0);
        assert_eq!(ch.created.len(), 3);
        e.undo();
        assert!(e.props.is_empty());
        let _ = std::fs::remove_dir_all(&dens);
    }

    #[test]
    fn an_empty_den_has_nothing_to_keep() {
        let dens = tmp("empty");
        assert!(from_layout(&Den::new(Spot::default()), &dens, "Orion", 0).is_err());
        let _ = std::fs::remove_dir_all(&dens);
    }
}
