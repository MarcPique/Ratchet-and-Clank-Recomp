//! **The rail chooser of Veldin's last level, class 582** (level18 `0x2d62e8`, census U581; one placed, #204). Four of
//! the level's grind rails are laid twice, once each way: the start of one lies within 0.5 of the end of the other.
//! The chooser pairs them at its first update and from then on keeps only one rail of each pair grindable, the one
//! whose start is nearer to Ratchet (on the ground plane), so he always rides it away from where he got on; the
//! rail he is grinding is left as it is. It also widens the grind catch every tick (0x13f51a = 5) and, at its
//! first update, puts a nanotech cluster at the centre of each of its two cuboids. Read from the level18 decomp.
//! Native `f32`.
//!
//! **Pvars** (0x48): +0x00 + 8k / +0x04 + 8k the pair k's two rails (the game: their records `0x15f70c + i·0x20`;
//! the port: the grind path indices), +0x20 + 8k / +0x24 + 8k their radii (record +0x0c, the bounding sphere's),
//! +0x40 / +0x44 the two cuboids of the clusters.
//!
//! ## Coverage (`0x2d62e8`)
//! | address | what | port |
//! |---|---|---|
//! | state 0 | +0x30 = 0xff, → 1 | [`update`] |
//! | | until four pairs are found: every grind path i with a radius: pair k's first rail = i (its radius kept); the first path j with a radius whose last point lies within 0.5 (3-D, `0x1ff450`) of i's first point: pair k's second rail = j (its radius kept), both radii 0, k += 1 | [`update`], [`pair_rails`] |
//! | | `0x2e22c0(cuboid +0x30)` for the cuboids +0x40, +0x44: `CreateMoby(0x326)`, +0x30 = 0x40, state 0, pvar +0x0c = itself, collision off, mode \| 0x41, position = the cuboid's centre, Euler 0 (`0x208048`), its matrix (`0x1fa030`) | [`update`], [`cluster`] |
//! | state 1 | 0x13f51a = 5 | [`update`] (`HeroFields::rail_reach`) |
//! | | each pair, unless Ratchet grinds (group 0x1413dc = 0xf) on one of its rails (0x13f8b0): the rail whose first point is nearer to him in xy (`0x1ff488`) gets its radius back, the other 0 (ties: the second) | [`update`] (`HeroFields::set_rail_radius` for the hero's rails; `Services::volumes` for the moby loop's copy) |
//!
//! The game scans the paths again and again until it has four pairs (a level with fewer would hang); the port stops
//! after a scan that finds none. Level 18's data pairs exactly four.

use crate::moby_runtime::MobyId;
use crate::moby_update::services::{pvar as p, World};

pub const REFERENCE_LEVEL: u32 = 18;
pub const UPDATE_FN: u32 = 0x2d_62e8;
pub const CLASSES: [i16; 1] = [582];

const LEN: usize = 0x48;
const PAIRS: usize = 4;
const CUBOIDS: usize = 0x40;
/// The nanotech cluster (`0x326`).
const CLUSTER: i16 = 0x326;
/// Ratchet's grind group.
const GRIND: i32 = 0xf;

/// Level18 `0x2d62e8` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    crate::moby_update::story::pvars(w, id, LEN);
    match w.m(id).state {
        0 => {
            {
                let m = w.mm(id);
                m.update_dist = 0xff;
                m.state = 1;
            }
            pair_rails(w, id);
            for k in 0..2 {
                let c = p::i32(&w.m(id).pvars, CUBOIDS + 4 * k);
                cluster(w, c);
            }
        }
        1 => {
            w.hero_fields_mut().rail_reach = Some(5);
            let (group, rail) = (w.hero.group, w.hero.boots.rail);
            let hero = w.hero_point();
            for k in 0..PAIRS {
                let (a, b) = rails(w, id, k);
                if group == GRIND && (rail == Some(a) || rail == Some(b)) { continue; }
                let (Some(sa), Some(sb)) = (first(w, a), first(w, b)) else { continue };
                let (da, db) = (crate::spline::dist2(hero, sa), crate::spline::dist2(hero, sb));
                let pv = &w.m(id).pvars;
                let (ra, rb) = (p::ff(pv, 0x20 + 8 * k), p::ff(pv, 0x24 + 8 * k));
                if da < db {
                    set_radius(w, a, ra);
                    set_radius(w, b, 0.0);
                } else {
                    set_radius(w, a, 0.0);
                    set_radius(w, b, rb);
                }
            }
        }
        _ => {}
    }
}

/// The first update's pairing (module table): the pairs into the pvars, their radii set to 0.
fn pair_rails(w: &mut World, id: MobyId) {
    let mut radius: Vec<f32> = w.svc.volumes.grind_paths.iter().map(|g| g.bsphere[3]).collect();
    let ends: Vec<Option<([f32; 3], [f32; 3])>> = w.svc.volumes.grind_paths.iter()
        .map(|g| Some((xyz(*g.points.first()?), xyz(*g.points.last()?))))
        .collect();
    let mut k = 0;
    while k < PAIRS {
        let before = k;
        for i in 0..radius.len() {
            if radius[i] == 0.0 { continue; }
            if k < PAIRS {
                let pv = &mut w.mm(id).pvars;
                p::set_i32(pv, 8 * k, i as i32);
                p::set_ff(pv, 0x20 + 8 * k, radius[i]);
            }
            let Some((start, _)) = ends[i] else { continue };
            for j in 0..radius.len() {
                let Some((_, end)) = ends[j] else { continue };
                if radius[j] != 0.0 && crate::spline::dist3(start, end) < 0.5 {
                    if k < PAIRS {
                        let pv = &mut w.mm(id).pvars;
                        p::set_i32(pv, 4 + 8 * k, j as i32);
                        p::set_ff(pv, 0x24 + 8 * k, radius[j]);
                    }
                    k += 1;
                    radius[j] = 0.0;
                    radius[i] = 0.0;
                    break;
                }
            }
        }
        if k == before { break; }
    }
    for (i, r) in radius.iter().enumerate() {
        if w.svc.volumes.grind_paths[i].bsphere[3] != *r { set_radius(w, i, *r); }
    }
}

/// `0x2e22c0`: a nanotech cluster at cuboid `c`'s centre, its own crate (a free cluster from its first update).
fn cluster(w: &mut World, c: i32) {
    let Some(shape) = (c >= 0).then(|| w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, c)).flatten().cloned() else { return };
    let Some(i) = w.create_moby(CLUSTER) else { return };
    let m = w.mm(i);
    m.update_dist = 0x40;
    m.state = 0;
    p::set_i32(&mut m.pvars, 0xc, i as i32 + 1);
    m.has_collision = false;
    m.mode |= 0x41;
    m.position = shape.matrix[3];
    m.rotation = [0.0; 4];
}

/// Pair `k`'s two rails.
fn rails(w: &World, id: MobyId, k: usize) -> (usize, usize) {
    let pv = &w.m(id).pvars;
    (p::i32(pv, 8 * k) as usize, p::i32(pv, 4 + 8 * k) as usize)
}

/// Rail `i`'s first point.
fn first(w: &World, i: usize) -> Option<[f32; 3]> { w.svc.volumes.grind_paths.get(i).and_then(|g| g.points.first()).map(|q| xyz(*q)) }

fn xyz(q: [f32; 4]) -> [f32; 3] { [q[0], q[1], q[2]] }

/// Record +0x0c of rail `i` = `r`: the moby loop's copy now, the hero's through the tick.
fn set_radius(w: &mut World, i: usize, r: f32) {
    if let Some(g) = std::sync::Arc::make_mut(&mut w.svc.volumes).grind_paths.get_mut(i) { g.bsphere[3] = r; }
    w.hero_fields_mut().set_rail_radius(i, r);
}
