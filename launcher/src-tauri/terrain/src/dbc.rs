//! The game's own tables (WDBC files), read from the player's archives and edited on the player's PC: the Den's sky
//! (since 3 Nov 10:29 Elwynn Forest's: [`elwynn_sky`], Light only; before, a white one: LightIntBand, LightParams,
//! found through Light) and its name (AreaTable). Nothing here is ever shipped: the
//! launcher reads the player's own tables, changes a few existing rows, and puts them in the Den's patch-T (which is
//! taken out whenever the player plays anywhere else).
//!
//! The form (wowdev wiki "DBC", public): "WDBC", record count, field count, record size, string block size; the
//! records; the string block (zero-terminated strings, offset 0 the empty string).
//!
//! The tables' columns (wowdev wiki, 3.3.5a: "Light.dbc", "LightParams.dbc", "LightIntBand.dbc", "AreaTable.dbc"),
//! checked against each file's own field count and record size before anything is changed; a table that doesn't have
//! the expected shape is left alone (the Den is then white on the ground only):
//! * Light (15 fields): id, map, position x / y / z (placement space times 36: x = 17066.67 - world y, y = height,
//!   z = 17066.67 - world x; "inches"), falloff start, falloff end (inches), 8 LightParams ids (clear, clear under
//!   water, storm, storm under water, death, and 3 more). A light at 0, 0, 0 is the map's default (its whole sky).
//! * LightParams: id, highlight sky, skybox id, ...
//! * LightIntBand (34 fields): id, count, 16 times, 16 colours; LightParams P owns ids P*18-17 ..= P*18 (global
//!   diffuse, global ambient, the sky's five bands top to horizon, fog, ... sun, sun halo, clouds, ocean, river).
//! * AreaTable (36 fields): id, map, parent, ..., the name in 16 locales at fields 11 ..= 26, its flags at 27.
//!
//! Only rows that are already there are changed, none added: if another archive the game reads after ours hides one
//! of these tables, the game still has every row any other table points at (never a missing id, never a crash).

use crate::{err, Result, PLACE};

#[derive(Debug, Clone, PartialEq)]
pub struct Dbc {
    pub fields: u32,
    pub record_size: u32,
    pub records: Vec<Vec<u8>>,
    pub strings: Vec<u8>,
}

impl Dbc {
    pub fn parse(b: &[u8]) -> Result<Dbc> {
        if b.len() < 20 || &b[..4] != b"WDBC" {
            return err("That isn't a game table (no WDBC).");
        }
        let u = |at: usize| u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]);
        let (n, fields, size, sn) = (u(4) as usize, u(8), u(12) as usize, u(16) as usize);
        if size == 0 || size > 4096 || n > 4_000_000 || 20 + n * size + sn != b.len() {
            return err("That game table's sizes don't add up.");
        }
        let records = (0..n).map(|i| b[20 + i * size..20 + (i + 1) * size].to_vec()).collect();
        let strings = b[20 + n * size..].to_vec();
        Ok(Dbc { fields, record_size: size as u32, records, strings })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut o = Vec::with_capacity(20 + self.records.len() * self.record_size as usize + self.strings.len());
        o.extend_from_slice(b"WDBC");
        for v in [self.records.len() as u32, self.fields, self.record_size, self.strings.len() as u32] {
            o.extend_from_slice(&v.to_le_bytes());
        }
        self.records.iter().for_each(|r| o.extend_from_slice(r));
        o.extend_from_slice(&self.strings);
        o
    }

    /// Every field 4 bytes, `fields` of them (the tables used here are so).
    pub fn shaped(&self, fields: u32) -> bool {
        self.fields == fields && self.record_size == fields * 4
    }

    pub fn get(r: &[u8], f: usize) -> u32 {
        r.get(f * 4..f * 4 + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]])).unwrap_or(0)
    }
    pub fn getf(r: &[u8], f: usize) -> f32 {
        f32::from_bits(Self::get(r, f))
    }
    pub fn set(r: &mut [u8], f: usize, v: u32) {
        if let Some(s) = r.get_mut(f * 4..f * 4 + 4) {
            s.copy_from_slice(&v.to_le_bytes());
        }
    }
    pub fn setf(r: &mut [u8], f: usize, v: f32) {
        Self::set(r, f, v.to_bits())
    }

    pub fn row(&self, id: u32) -> Option<&Vec<u8>> {
        self.records.iter().find(|r| Self::get(r, 0) == id)
    }
    pub fn row_mut(&mut self, id: u32) -> Option<&mut Vec<u8>> {
        self.records.iter_mut().find(|r| Self::get(r, 0) == id)
    }
    pub fn max_id(&self) -> u32 {
        self.records.iter().map(|r| Self::get(r, 0)).max().unwrap_or(0)
    }

    /// The string at an offset of the string block.
    pub fn string(&self, off: u32) -> String {
        let rest = self.strings.get(off as usize..).unwrap_or(&[]);
        let end = rest.iter().position(|&c| c == 0).unwrap_or(rest.len());
        String::from_utf8_lossy(&rest[..end]).to_string()
    }

    /// Adds a string to the block (offset 0 stays the empty string) and returns its offset.
    pub fn add_string(&mut self, s: &str) -> u32 {
        if self.strings.is_empty() {
            self.strings.push(0);
        }
        let at = self.strings.len() as u32;
        self.strings.extend_from_slice(s.as_bytes());
        self.strings.push(0);
        at
    }
}

/// The Den's name where the game shows the zone (AreaTable).
pub const DEN_NAME: &str = "Your Den";
/// The Den's light colours (0x00RRGGBB; greys, so the channel order doesn't matter): sky, fog, sun and clouds white;
/// the ground lit nearly all from around (ambient) and a little from the sun, so the white ground stays white and
/// models keep some shading.
pub const WHITE: u32 = 0x00FF_FFFF;
pub const AMBIENT: u32 = 0x00D8_D8D8;
pub const DIFFUSE: u32 = 0x0080_8080;
pub const WATER: u32 = 0x00F0_F0F0;
/// The LightParams of a light that are made white: clear weather and storm (fields 7 and 9 of Light). Under water
/// and the others are left as they are.
const WHITE_SLOTS: [usize; 2] = [7, 9];

fn colour_of(band: u32) -> u32 {
    match band {
        0 => DIFFUSE,
        1 => AMBIENT,
        14..=17 => WATER,
        _ => WHITE,
    }
}

/// The Den's sky tables, edited.
#[derive(Debug, Clone, PartialEq)]
pub struct Sky {
    pub int_band: Dbc,
    /// LightParams with its skybox models taken off the white params; None: nothing in it changed.
    pub params: Option<Dbc>,
    /// The lights whose sky is now white (the map's default and the ones that reach the Den), and their params.
    pub lights: Vec<u32>,
    pub params_ids: Vec<u32>,
}

/// A white sky over the Den on `map`: the map's default light (the sky everywhere no other light reaches) and every
/// other light of the map whose reach takes in the Den's point (world `x`, `y`) get white colour bands for clear
/// weather and storms, and no skybox model. Existing rows only (see the module's notes).
pub fn white_sky(light: &Dbc, params: Option<&Dbc>, int_band: &Dbc, map: u32, x: f32, y: f32) -> Result<Sky> {
    if !light.shaped(15) {
        return err(format!("Light.dbc has {} fields of {} bytes, not the 15 Leyline knows.", light.fields, light.record_size));
    }
    if !int_band.shaped(34) {
        return err("LightIntBand.dbc isn't the shape Leyline knows.");
    }
    let (px, pz) = ((PLACE - y) * 36.0, (PLACE - x) * 36.0);
    let mut lights = Vec::new();
    let mut ids: Vec<u32> = Vec::new();
    for r in light.records.iter().filter(|r| Dbc::get(r, 1) == map) {
        let default = (2..5).all(|f| Dbc::getf(r, f) == 0.0);
        let (dx, dz) = (Dbc::getf(r, 2) - px, Dbc::getf(r, 4) - pz);
        let reach = Dbc::getf(r, 6).max(Dbc::getf(r, 5));
        if default || (reach > 0.0 && (dx * dx + dz * dz).sqrt() < reach) {
            lights.push(Dbc::get(r, 0));
            for f in WHITE_SLOTS {
                let p = Dbc::get(r, f);
                if p != 0 && !ids.contains(&p) {
                    ids.push(p);
                }
            }
        }
    }
    if lights.is_empty() {
        return err(format!("Light.dbc has no light for map {map}."));
    }
    let mut ib = int_band.clone();
    let mut n = 0;
    for &p in &ids {
        for k in 0..18u32 {
            let Some(r) = ib.row_mut(p * 18 - 17 + k) else { continue };
            (1..34).for_each(|f| Dbc::set(r, f, 0));
            Dbc::set(r, 1, 1); // one value, all day
            Dbc::set(r, 18, colour_of(k));
            n += 1;
        }
    }
    if n == 0 {
        return err("The map's lights have no colours in LightIntBand.dbc.");
    }
    let params = params.filter(|t| t.fields >= 3 && t.record_size == t.fields * 4).and_then(|t| {
        let mut t = t.clone();
        let mut changed = false;
        for &p in &ids {
            if let Some(r) = t.row_mut(p) {
                if Dbc::get(r, 2) != 0 {
                    Dbc::set(r, 2, 0); // no skybox model: the sky is the colour bands
                    changed = true;
                }
            }
        }
        changed.then_some(t)
    });
    Ok(Sky { int_band: ib, params, lights, params_ids: ids })
}

/// A point in Elwynn Forest on map 0 (Goldshire, world x / y): the light that reaches it is Elwynn's.
pub const ELWYNN: (u32, f32, f32) = (0, -9460.0, 60.0);

/// The Den's sky made Elwynn Forest's (owner 3 Nov 10:29: "do blue forest sky not grey green").
#[derive(Debug, Clone, PartialEq)]
pub struct ForestSky {
    /// Light.dbc with the Den's lights pointing at Elwynn's LightParams (its colours and skybox).
    pub light: Dbc,
    /// The Den's lights now Elwynn's (the map's default and the ones that reach the Den).
    pub lights: Vec<u32>,
    /// Elwynn's light whose LightParams they got.
    pub from: u32,
}

/// The lights of `map` that make the sky at world (`x`, `y`): the map's default (a light at 0, 0, 0) and every one
/// whose reach takes in the point, nearest-reaching last.
fn lights_at(light: &Dbc, map: u32, x: f32, y: f32) -> Vec<(u32, f32, bool)> {
    let (px, pz) = ((PLACE - y) * 36.0, (PLACE - x) * 36.0);
    let mut v = Vec::new();
    for r in light.records.iter().filter(|r| Dbc::get(r, 1) == map) {
        let default = (2..5).all(|f| Dbc::getf(r, f) == 0.0);
        let (dx, dz) = (Dbc::getf(r, 2) - px, Dbc::getf(r, 4) - pz);
        let reach = Dbc::getf(r, 6).max(Dbc::getf(r, 5));
        if default || (reach > 0.0 && (dx * dx + dz * dz).sqrt() < reach) {
            v.push((Dbc::get(r, 0), if default { f32::MAX } else { reach }, default));
        }
    }
    v
}

/// The Den's sky from the player's own Light.dbc: Elwynn Forest's light (the one with the smallest reach that takes
/// in [`ELWYNN`], else map 0's default) gives its eight LightParams (clear, under water, storm, ... : its colours
/// and its skybox) to the Den's lights on `map` at world (`x`, `y`): the map's default and every light reaching the
/// Den. Only Light.dbc changes, only rows already there, and only to ids the game already has (Elwynn's own), so a
/// table hidden by a later archive can never leave a missing id. LightParams, LightIntBand and the zone stay the
/// game's.
pub fn elwynn_sky(light: &Dbc, map: u32, x: f32, y: f32) -> Result<ForestSky> {
    if !light.shaped(15) {
        return err(format!("Light.dbc has {} fields of {} bytes, not the 15 Leyline knows.", light.fields, light.record_size));
    }
    let mut forest = lights_at(light, ELWYNN.0, ELWYNN.1, ELWYNN.2);
    forest.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    let Some(&(from, ..)) = forest.first() else { return err("Light.dbc has no light over Elwynn Forest.") };
    let params: Vec<u32> = (7..15).map(|f| light.row(from).map(|r| Dbc::get(r, f)).unwrap_or(0)).collect();
    if params[0] == 0 {
        return err("Elwynn Forest's light has no sky in Light.dbc.");
    }
    let lights: Vec<u32> = lights_at(light, map, x, y).into_iter().map(|l| l.0).filter(|&id| id != from).collect();
    if lights.is_empty() {
        return err(format!("Light.dbc has no light for map {map}."));
    }
    let mut t = light.clone();
    for &id in &lights {
        if let Some(r) = t.row_mut(id) {
            for (k, &p) in params.iter().enumerate() {
                // a slot Elwynn leaves empty keeps the Den's own (never an id of 0 where the game had one)
                if p != 0 {
                    Dbc::set(r, 7 + k, p);
                }
            }
        }
    }
    Ok(ForestSky { light: t, lights, from })
}

/// The area's name in every locale changed to `name` (AreaTable, 36 fields, names at 11 ..= 26).
pub fn rename_area(area: &Dbc, id: u32, name: &str) -> Result<Dbc> {
    if !area.shaped(36) {
        return err(format!("AreaTable.dbc has {} fields, not the 36 Leyline knows.", area.fields));
    }
    let mut a = area.clone();
    if a.row(id).is_none() {
        return err(format!("The game has no area {id}."));
    }
    let off = a.add_string(name);
    let r = a.row_mut(id).unwrap();
    (11..27).for_each(|f| Dbc::set(r, f, off));
    Ok(a)
}

/// The area's name (the first locale that has one).
pub fn area_name(area: &Dbc, id: u32) -> Option<String> {
    let r = area.row(id)?;
    (11..27).map(|f| area.string(Dbc::get(r, f))).find(|s| !s.is_empty())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn table(fields: u32, rows: &[Vec<u32>], strings: &[u8]) -> Dbc {
        Dbc {
            fields,
            record_size: fields * 4,
            records: rows
                .iter()
                .map(|r| {
                    let mut v: Vec<u8> = r.iter().flat_map(|x| x.to_le_bytes()).collect();
                    v.resize(fields as usize * 4, 0);
                    v
                })
                .collect(),
            strings: strings.to_vec(),
        }
    }

    /// Synthetic light tables: map 1's default light (id 1: params 12 clear, 13 under water, 14 storm), a far light
    /// (id 2, params 20), one whose reach takes in the Den (id 3, params 21) and another map's default (id 4, 22).
    pub fn lights() -> (Dbc, Dbc, Dbc) {
        let f = |v: f32| v.to_bits();
        let den = ((PLACE - 16257.0) * 36.0, (PLACE - 16226.2) * 36.0);
        let light = table(
            15,
            &[
                vec![1, 1, 0, 0, 0, 0, 0, 12, 13, 14, 13, 12, 12, 12, 12],
                vec![2, 1, f(500_000.0), 0, f(500_000.0), f(3600.0), f(7200.0), 20, 20, 20, 20, 20, 20, 20, 20],
                vec![3, 1, f(den.0 + 3600.0), f(400.0), f(den.1), f(1800.0), f(7200.0), 21, 21, 21, 21, 21, 21, 21, 21],
                vec![4, 0, 0, 0, 0, 0, 0, 22, 22, 22, 22, 22, 22, 22, 22],
            ],
            &[0],
        );
        let params = table(9, &[vec![12, 0, 77, f(0.5)], vec![13, 1, 5], vec![14, 0, 0], vec![21, 0, 0]], &[0]);
        let mut ib = Vec::new();
        for p in [12u32, 13, 14, 20, 21, 22] {
            for k in 0..18 {
                let mut r = vec![p * 18 - 17 + k, 3, 0, 720, 1440];
                r.resize(18, 0);
                r.extend([0x123456, 0x234567, 0x345678]);
                ib.push(r);
            }
        }
        (light, params, table(34, &ib, &[0]))
    }

    #[test]
    fn tables_round_trip_and_bad_ones_are_refused() {
        let (l, ..) = lights();
        let b = l.write();
        assert_eq!(&b[..4], b"WDBC");
        assert_eq!(u32::from_le_bytes(b[4..8].try_into().unwrap()), 4);
        assert_eq!(Dbc::parse(&b).unwrap(), l);
        assert!(Dbc::parse(&b[..b.len() - 1]).is_err() && Dbc::parse(b"WDBC").is_err() && Dbc::parse(b"nope nope nope nope nope").is_err());
        let mut s = table(2, &[vec![1, 0]], b"");
        let off = s.add_string("Your Den");
        assert_eq!((off, s.string(off).as_str(), s.string(0).as_str()), (1, "Your Den", ""));
        assert_eq!(s.max_id(), 1);
    }

    #[test]
    fn the_dens_white_sky() {
        let (l, p, ib) = lights();
        let s = white_sky(&l, Some(&p), &ib, 1, 16226.2, 16257.0).unwrap();
        assert_eq!(s.lights, vec![1, 3], "the default and the one that reaches the Den; not the far one, not map 0's");
        assert_eq!(s.params_ids, vec![12, 14, 21], "clear weather and storm; under water (13) as it was");
        assert_eq!(s.int_band.records.len(), ib.records.len(), "no rows added");
        for p in [12u32, 14, 21] {
            for k in 0..18u32 {
                let b = s.int_band.row(p * 18 - 17 + k).unwrap();
                assert_eq!((Dbc::get(b, 1), Dbc::get(b, 3), Dbc::get(b, 19)), (1, 0, 0), "one value, all day");
                assert_eq!(Dbc::get(b, 18), match k { 0 => DIFFUSE, 1 => AMBIENT, 14..=17 => WATER, _ => WHITE }, "params {p} band {k}");
            }
        }
        for p in [13u32, 20, 22] {
            assert_eq!(s.int_band.row(p * 18 - 17).unwrap(), ib.row(p * 18 - 17).unwrap(), "params {p} untouched");
        }
        // the default's skybox model off; under water's (13) kept
        let pt = s.params.as_ref().unwrap();
        assert_eq!((Dbc::get(pt.row(12).unwrap(), 2), Dbc::getf(pt.row(12).unwrap(), 3)), (0, 0.5));
        assert_eq!(Dbc::get(pt.row(13).unwrap(), 2), 5);
        assert_eq!(Dbc::parse(&s.int_band.write()).unwrap(), s.int_band);
        assert_eq!(Dbc::parse(&pt.write()).unwrap(), *pt);
        // no skybox to take off: LightParams isn't rewritten; unknown LightParams shape: left alone
        let mut p2 = p.clone();
        Dbc::set(p2.row_mut(12).unwrap(), 2, 0);
        assert!(white_sky(&l, Some(&p2), &ib, 1, 16226.2, 16257.0).unwrap().params.is_none());
        assert!(white_sky(&l, None, &ib, 1, 16226.2, 16257.0).unwrap().params.is_none());
        // a table of another shape, a map without lights, lights without colours: refused, nothing guessed
        assert!(white_sky(&table(14, &[vec![1]], &[0]), Some(&p), &ib, 1, 0.0, 0.0).is_err());
        assert!(white_sky(&l, Some(&p), &table(33, &[], &[0]), 1, 0.0, 0.0).is_err());
        assert!(white_sky(&l, Some(&p), &ib, 571, 0.0, 0.0).is_err());
        assert!(white_sky(&l, Some(&p), &table(34, &[vec![1, 1]], &[0]), 1, 16226.2, 16257.0).is_err());
    }

    #[test]
    fn the_dens_sky_is_elwynns() {
        let (mut l, ..) = lights();
        let f = |v: f32| v.to_bits();
        let elw = ((PLACE - ELWYNN.2) * 36.0, (PLACE - ELWYNN.1) * 36.0);
        // map 0: Elwynn's own light (id 5, params 40..47) and a wide one that also reaches it (id 6, params 50)
        l.records.push(table(15, &[vec![5, 0, f(elw.0 + 100.0), f(0.0), f(elw.1), f(9000.0), f(20000.0), 40, 41, 42, 43, 44, 45, 46, 47]], &[0]).records.remove(0));
        l.records.push(table(15, &[vec![6, 0, f(elw.0), f(0.0), f(elw.1 - 3000.0), f(90000.0), f(200000.0), 50, 50, 50, 50, 50, 50, 50, 50]], &[0]).records.remove(0));
        let s = elwynn_sky(&l, 1, 16226.2, 16257.0).unwrap();
        assert_eq!(s.from, 5, "the nearest-reaching light over Goldshire, not the wide one or map 0's default (4)");
        assert_eq!(s.lights, vec![1, 3], "the Den's: map 1's default and the one reaching the Den; not the far one");
        for id in [1u32, 3] {
            let r = s.light.row(id).unwrap();
            assert_eq!((7..15).map(|k| Dbc::get(r, k)).collect::<Vec<_>>(), (40..48).collect::<Vec<u32>>(), "light {id}");
            assert_eq!(Dbc::get(r, 1), 1, "still map 1");
        }
        assert_eq!(s.light.row(2).unwrap(), l.row(2).unwrap(), "the far light as it was");
        assert_eq!(s.light.row(4).unwrap(), l.row(4).unwrap(), "map 0's default as it was");
        assert_eq!(s.light.records.len(), l.records.len(), "no rows added");
        assert_eq!(Dbc::parse(&s.light.write()).unwrap(), s.light);
        // no light of Elwynn's own: map 0's default gives its sky
        let (l0, ..) = lights();
        let s0 = elwynn_sky(&l0, 1, 16226.2, 16257.0).unwrap();
        assert_eq!(s0.from, 4);
        assert_eq!(Dbc::get(s0.light.row(1).unwrap(), 7), 22);
        // a slot Elwynn leaves empty keeps the Den's own
        let mut l2 = l.clone();
        Dbc::set(l2.row_mut(5).unwrap(), 8, 0);
        assert_eq!(Dbc::get(elwynn_sky(&l2, 1, 16226.2, 16257.0).unwrap().light.row(1).unwrap(), 8), 13);
        // another shape, no map 0, no map 1: refused
        assert!(elwynn_sky(&table(14, &[vec![1]], &[0]), 1, 0.0, 0.0).is_err());
        let only1 = table(15, &[vec![1, 1, 0, 0, 0, 0, 0, 12, 13, 14, 13, 12, 12, 12, 12]], &[0]);
        assert!(elwynn_sky(&only1, 1, 0.0, 0.0).is_err());
        assert!(elwynn_sky(&l, 571, 0.0, 0.0).is_err());
    }

    #[test]
    fn the_area_is_called_your_den() {
        let mut row = vec![0u32; 36];
        row[0] = 876;
        row[1] = 1;
        (11..27).for_each(|f| row[f] = 1);
        let a = table(36, &[vec![1, 0], row], b"\0GM Island\0");
        assert_eq!(area_name(&a, 876).as_deref(), Some("GM Island"));
        let b = rename_area(&a, 876, DEN_NAME).unwrap();
        assert_eq!(area_name(&b, 876).as_deref(), Some("Your Den"));
        assert!((11..27).all(|f| b.string(Dbc::get(b.row(876).unwrap(), f)) == "Your Den"), "every locale");
        assert_eq!(Dbc::get(b.row(876).unwrap(), 1), 1, "the rest as it was");
        assert_eq!(b.records.len(), a.records.len());
        assert_eq!(Dbc::parse(&b.write()).unwrap(), b);
        assert!(rename_area(&a, 5, DEN_NAME).is_err());
        assert!(rename_area(&table(35, &[], &[0]), 876, DEN_NAME).is_err());
    }
}
