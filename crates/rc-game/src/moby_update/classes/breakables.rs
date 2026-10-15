//! Breakable props: the game's one shared break template, on every level (docs/plan/breakables.md).
//!
//! **The template.** Every breakable prop class the survey found (22 classes on 13 levels, table below) runs a copy of
//! the same update, linked once per class with its own constants: state 0 → 1; in state 1 a hit record with flags
//! 0x10000 (the wrench, `MobyGetHitMessage(m, 0x10000, 0)`) and damage > 0 sets state 2; state 2 breaks it, in this
//! order:
//! 1. `PlayClassSound(0, 0, m)` (the class's break sound);
//! 2. on the classes that are kept broken (levels 16 / 17), `SetDeathBits(m, 0, −1)` (their placed bolts,
//!    `crate_::set_death_bits`, and the death bits: the loader does not create them again);
//! 3. `BreakFxA(m)` (level01 0x2787a0) = `BoltBurst(m, 4, 7, Ratchet within 7 ? 2 : 0, −1)`: 4..7 bolts, and none
//!    when Ratchet is farther than 7 (flags 0 makes `BoltBurst` return);
//! 4. the pieces ([`Piece`]): `BreakFxB` pieces at the prop's place (`fx::break_piece`), the Novalis pot's ring of
//!    four, or `BreakFxBurst` (level02 0x265408, absent from level 01; [`burst`]): consecutive piece classes at the
//!    prop's place and up to N more at random points of its collision volume ([`volume_points`] = level01 0x26fba0);
//! 5. the remains ([`Remains`]): `BreakFxC(m, class)` (0x278e20, scaled like the prop) or the same written inline
//!    without the scale (levels 6 / 7);
//! 6. `DeleteMoby(m)`.
//!
//! The pieces run `FxGroupUpdate` (`fx::piece_update`); the burst's pieces carry flag 2 and end in the quieter
//! explosion `fx::piece_explosion` (0x2742a8). The remains' updates are empty.
//!
//! **Registration.** Each copy is its own function (the class numbers are immediates in the code), so each is one
//! [`Recipe`]: the reference level and address of the copy, its classes there, and its constants. The registry
//! (`ClassUpdate::Breakable`) matches each recipe's function in every level's class table by code identity like
//! every other port, so a copy shared by two levels runs on both. All recipes run [`update`].
//!
//! | level | fn | classes | kept | pieces | remains |
//! |---|---|---|---|---|---|
//! | 01 | 0x2fd9a0 | 754 | | ring of four 1817 | 1816 (C) |
//! | 01 | 0x30d0f0 | 1813 | | 1815 | 1816 (C) |
//! | 02 | 0x2f1f60 | 1819 | | burst 1821, 5 | 1820 (C) |
//! | 03 | 0x2d3cd8 | 817 | | 1831, 1832, 1833 | – |
//! | 03 | 0x2e2948 | 1827 | | burst 1829, 1 | 1828 (C) |
//! | 04 | 0x2e79a8 | 1834 | | burst 1836, 1 | 1835 (C) |
//! | 06 | 0x309860 / 0x309e08 / 0x309fa8 | 1655 / 1660 / 1664 | | 1657–1659 / 1662 ×2 / 1666 ×2 | 1656 / 1661 / 1665 (inline) |
//! | 07 | 0x31f3d8 / 0x31f5b0 / 0x31f780 | 1650 / 1663, 1677 / 1686 | | 1652–1654 / 1678–1680 / 1688–1690 | 1651 / 1677 (inline) / – |
//! | 08 | 0x30bc00 | 1844 | | burst 1846 + 1847, 1 | 1845 (C) |
//! | 09 | 0x30a678 | 1849 | | burst 1851, 7 | 1850 (C) |
//! | 10 | 0x2eb6f0 / 0x2eb7e0 | 1855 / 1856 | | burst 1858, 11 / – | – / 1857 (C) |
//! | 11 | 0x31d7d8 | 1859 | | burst 1861, 11 | 1860 (C) |
//! | 12 | 0x30b6c8 | 1862 | | burst 1864, 11 | 1863 (C) |
//! | 16 | 0x2d6740 | 800 | yes | burst 1870, 11 | – |
//! | 17 | 0x2f5260 / 0x2f5390 | 1873 / 1876 | yes | burst 1875, 11 + burst 1878, 1 / 1875 + burst 1878, 1 | 1874 / 1877 (C) |
//!
//! Standard `f32`; the draws, their order and the constants are the game's.

pub mod novalis;

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::crate_::{bolt_burst, set_death_bits};
use crate::moby_update::creature::fx::break_piece;
use crate::moby_update::services::World;
use crate::ps2v::Pf;

type V = [f32; 4];

/// `BreakFxB` piece flag 2: the piece ends in `fx::piece_explosion` (0x2742a8).
pub const PIECE_QUIET: u32 = 2;

/// One piece spawn of a break.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Piece {
    /// `BreakFxB(0, m, class, m.pos, m.rot, 0, 0, 0, 0)`.
    Single(i16),
    /// The Novalis pot's ring (0x2fd9a0): four pieces at 90° steps, 0.57·s out and 1.2·s up (s = scale / class
    /// scale), each turned 50° more.
    Ring4(i16),
    /// `BreakFxBurst(m, a, na, b, nb, n, flag)` (level02 0x265408, [`burst`]).
    Burst { a: i16, na: i32, b: i16, nb: i32, n: i32, flag: i32 },
}

/// What a break leaves in the prop's place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Remains {
    None,
    /// `BreakFxC(m, class)` 0x278e20.
    Scaled(i16),
    /// The inline copy of levels 6 / 7: `CreateMoby(class)`, drawn, draw distance 0xff, the prop's position,
    /// rotation, light and ambient; no scale, update distance or mode.
    Plain(i16),
}

/// One copy of the template (module doc).
#[derive(Clone, Copy, Debug)]
pub struct Recipe {
    /// The level whose overlay holds [`Recipe::func`].
    pub level: u32,
    pub func: u32,
    /// The classes that level's table runs it for.
    pub classes: &'static [i16],
    /// `SetDeathBits(m, 0, −1)` after the sound.
    pub keep_broken: bool,
    pub pieces: &'static [Piece],
    pub remains: Remains,
}

const fn burst(a: i16, b: i16, n: i32) -> Piece { Piece::Burst { a, na: 1, b, nb: 1, n, flag: 2 } }

/// Every copy the survey found (docs/plan/breakables.md §2), by level.
pub const RECIPES: [Recipe; 21] = [
    Recipe { level: 1, func: 0x2fd9a0, classes: &[754], keep_broken: false, pieces: &[Piece::Ring4(0x719)], remains: Remains::Scaled(0x718) },
    Recipe { level: 1, func: 0x30d0f0, classes: &[1813], keep_broken: false, pieces: &[Piece::Single(0x717)], remains: Remains::Scaled(0x718) },
    Recipe { level: 2, func: 0x2f1f60, classes: &[1819], keep_broken: false, pieces: &[burst(0x71d, 0x71d, 5)], remains: Remains::Scaled(0x71c) },
    Recipe { level: 3, func: 0x2d3cd8, classes: &[817], keep_broken: false, pieces: &[Piece::Single(0x727), Piece::Single(0x728), Piece::Single(0x729)], remains: Remains::None },
    Recipe { level: 3, func: 0x2e2948, classes: &[1827], keep_broken: false, pieces: &[burst(0x725, 0x725, 1)], remains: Remains::Scaled(0x724) },
    Recipe { level: 4, func: 0x2e79a8, classes: &[1834], keep_broken: false, pieces: &[burst(0x72c, 0x72c, 1)], remains: Remains::Scaled(0x72b) },
    Recipe { level: 6, func: 0x309860, classes: &[1655], keep_broken: false, pieces: &[Piece::Single(0x679), Piece::Single(0x67a), Piece::Single(0x67b)], remains: Remains::Plain(0x678) },
    Recipe { level: 6, func: 0x309e08, classes: &[1660], keep_broken: false, pieces: &[Piece::Single(0x67e), Piece::Single(0x67e)], remains: Remains::Plain(0x67d) },
    Recipe { level: 6, func: 0x309fa8, classes: &[1664], keep_broken: false, pieces: &[Piece::Single(0x682), Piece::Single(0x682)], remains: Remains::Plain(0x681) },
    Recipe { level: 7, func: 0x31f3d8, classes: &[1650], keep_broken: false, pieces: &[Piece::Single(0x674), Piece::Single(0x675), Piece::Single(0x676)], remains: Remains::Plain(0x673) },
    Recipe { level: 7, func: 0x31f5b0, classes: &[1663, 1677], keep_broken: false, pieces: &[Piece::Single(0x68e), Piece::Single(0x68f), Piece::Single(0x690)], remains: Remains::Plain(0x68d) },
    Recipe { level: 7, func: 0x31f780, classes: &[1686], keep_broken: false, pieces: &[Piece::Single(0x698), Piece::Single(0x699), Piece::Single(0x69a)], remains: Remains::None },
    Recipe { level: 8, func: 0x30bc00, classes: &[1844], keep_broken: false, pieces: &[burst(0x736, 0x737, 1)], remains: Remains::Scaled(0x735) },
    Recipe { level: 9, func: 0x30a678, classes: &[1849], keep_broken: false, pieces: &[burst(0x73b, 0x73b, 7)], remains: Remains::Scaled(0x73a) },
    Recipe { level: 10, func: 0x2eb6f0, classes: &[1855], keep_broken: false, pieces: &[burst(0x742, 0x742, 0xb)], remains: Remains::None },
    Recipe { level: 10, func: 0x2eb7e0, classes: &[1856], keep_broken: false, pieces: &[], remains: Remains::Scaled(0x741) },
    Recipe { level: 11, func: 0x31d7d8, classes: &[1859], keep_broken: false, pieces: &[burst(0x745, 0x745, 0xb)], remains: Remains::Scaled(0x744) },
    Recipe { level: 12, func: 0x30b6c8, classes: &[1862], keep_broken: false, pieces: &[burst(0x748, 0x748, 0xb)], remains: Remains::Scaled(0x747) },
    Recipe { level: 16, func: 0x2d6740, classes: &[800], keep_broken: true, pieces: &[burst(0x74e, 0x74e, 0xb)], remains: Remains::None },
    Recipe { level: 17, func: 0x2f5260, classes: &[1873], keep_broken: true, pieces: &[burst(0x753, 0x753, 0xb), burst(0x756, 0x756, 1)], remains: Remains::Scaled(0x752) },
    Recipe { level: 17, func: 0x2f5390, classes: &[1876], keep_broken: true, pieces: &[Piece::Single(0x753), burst(0x756, 0x756, 1)], remains: Remains::Scaled(0x755) },
];

/// The remains classes' update (level01 0x30d200, 1816: `jr ra`; the other levels' remains have the same empty
/// update). Registered for 1816 so the level-01 table's entry resolves; the others run nothing either way.
pub const REMAINS_FN: u32 = 0x30d200;
pub const REMAINS_CLASSES: [i16; 1] = [1816];

/// The recipe indices (`ClassUpdate::Breakable(i)`).
pub fn ids() -> impl Iterator<Item = u8> { 0..RECIPES.len() as u8 }

/// The recipe a class runs by its number on the recipe's own level (tests, diagnostics).
pub fn recipe_of(level: u32, o_class: i16) -> Option<&'static Recipe> { RECIPES.iter().find(|r| r.level == level && r.classes.contains(&o_class)) }

/// The template update (module doc) with recipe `i`.
pub fn update(w: &mut World, id: MobyId, i: u8) {
    let r = &RECIPES[i as usize];
    let hit = w.get_hit(id, 0x1_0000, false);
    match w.m(id).state {
        0 => w.mm(id).state = 1,
        1 => {
            if hit.is_some_and(|h| Pf::ZERO < h.damage) { w.mm(id).state = 2; }
        }
        2 => break_now(w, id, r),
        _ => {}
    }
}

/// State 2 of the template.
fn break_now(w: &mut World, id: MobyId, r: &Recipe) {
    w.play_sound(0, 0, id);
    if r.keep_broken { set_death_bits(w, id, 0, -1); }
    bolts(w, id);
    for p in r.pieces { piece(w, id, *p); }
    match r.remains {
        Remains::None => {}
        Remains::Scaled(c) => remains(w, id, c, true),
        Remains::Plain(c) => remains(w, id, c, false),
    }
    w.delete_moby(id);
}

/// `BreakFxA(m)` 0x2787a0: `BoltBurst(m, 4, 7, d < 7 ? 2 : 0, −1)`, d = `VecDistance2(m.pos, Ratchet)` (xy).
pub fn bolts(w: &mut World, id: MobyId) {
    let p = w.m(id).position;
    let h = crate::hero::physics::to_f32x3(w.hero.pos);
    let d = ((p[0] - h[0]).powi(2) + (p[1] - h[1]).powi(2)).sqrt();
    bolt_burst(w, id, 4, 7, if d < 7.0 { 2 } else { 0 }, -1);
}

fn piece(w: &mut World, id: MobyId, p: Piece) {
    match p {
        Piece::Single(c) => {
            let (pos, rot) = { let m = w.m(id); (m.position, m.rotation) };
            break_piece(w, id, c, pos, rot, 0, 0);
        }
        Piece::Ring4(c) => {
            let (pos, oc, scale) = { let m = w.m(id); (m.position, m.o_class, m.scale) };
            let s = scale / w.class_scale(oc).to_f32();
            for i in 0..4 {
                let a = i as f32 * 1.570_796_4;
                let rot = [0.0, 0.0, add_rot(a, f32::from_bits(0x3f5f_66f3)), 0.0];
                let at = [a.cos() * -0.57 * s + pos[0], a.sin() * -0.57 * s + pos[1], s * 1.2 + pos[2], pos[3]];
                break_piece(w, id, c, at, rot, 0, 0);
            }
        }
        Piece::Burst { a, na, b, nb, n, flag } => burst_pieces(w, id, a, na, b, nb, n, flag),
    }
}

/// `BreakFxBurst(m, a, na, b, nb, n, flag)` (level02 0x265408, read from its disassembly): with `a > 0`, pieces of
/// the classes `a .. a + na` at the prop's place and rotation; then, with `nb > 0` and `n > 0`, up to `n` points of
/// the prop's collision volume ([`volume_points`] at 1000 points a cubic unit, every primitive), each one a piece
/// of class `b + randi(nb)` turned by three `rand_angle`s. Every piece carries `flag ≠ 0 ? 2 : 0`.
#[allow(clippy::too_many_arguments)]
pub fn burst_pieces(w: &mut World, id: MobyId, a: i16, na: i32, b: i16, nb: i32, n: i32, flag: i32) {
    let fl = if flag != 0 { PIECE_QUIET } else { 0 };
    if a <= 0 { return; }
    let (pos, rot) = { let m = w.m(id); (m.position, m.rotation) };
    for k in 0..na.max(0) {
        break_piece(w, id, a + k as i16, pos, rot, 0, fl);
    }
    if nb <= 0 { return; }
    let pts = volume_points(w, id, 1000.0, n, 0xffff);
    for p in pts.into_iter().take(n.max(0) as usize) {
        let r = [w.rng.rand_angle(), w.rng.rand_angle(), w.rng.rand_angle(), 0.0];
        let c = b + w.rng.randi(nb) as i16;
        break_piece(w, id, c, p, r, 0, fl);
    }
}

/// `0x26fba0(density, m, max, out, prims)`: random world points inside the moby's collision primitives (class header
/// +0x10, `rc_formats::moby_collision`). Each primitive whose list index is set in `prims` and whose mask has bit 1
/// counts with its volume (s = moby scale / 1024): sphere / joint sphere `4.1887903·r³`, vertical cylinder
/// `4.187743·r³ + π/2·r²·h`, joint capsule `π/2·r²·|b − a|` (the game's constants, spheres of radius r and height h
/// in world units); `min(trunc(density·V + 0.5), max)` points follow, each in a primitive picked by volume
/// (`randf(0, V)`): a sphere's points by rejection in its cube (three `randf(−r, r)` a try), turned by the moby's rows
/// and moved to its position; a cylinder's `(√randf(0, r²), randf(0, h), rand_angle)` about its base centre, which is
/// not turned; a capsule's on a random point of its axis (`randf(0, 1)`), then as a sphere. Joint primitives read the
/// posed joints (`fun_0020fa90`, `collision_query::pose_joints`).
pub fn volume_points(w: &mut World, id: MobyId, density: f32, max: i32, prims: u32) -> Vec<V> {
    let m = w.m(id);
    let Some(coll) = w.svc.coll_classes.get(&m.o_class).cloned() else { return Vec::new() };
    let (scale, pos, rows) = (m.scale, m.position, m.rows);
    let joints: Vec<V> = if coll.joint_counts[1] != 0 {
        let snap = w.svc.snapshots.get(id).and_then(Option::as_ref);
        crate::collision_query::pose_joints(w.classes.anim(m.o_class), &m.anim, snap, coll.joint_counts[1] as usize).into_iter().map(|j| j.map(f32::from_bits)).collect()
    } else {
        Vec::new()
    };
    let joint = |j: i32| -> V { usize::try_from(j).ok().and_then(|j| joints.get(j)).copied().unwrap_or([0.0; 4]) };
    let k = scale * (1.0 / 1024.0);
    let fb = f32::from_bits;
    // The volume table (at most 20 primitives in the game's buffers).
    let mut vols: Vec<(usize, f32)> = Vec::new();
    for (i, p) in coll.prims.iter().enumerate() {
        let w0 = p.raw[0];
        if (i < 32 && (prims >> i) & 1 == 1) && w0 & 0x2_0000 != 0 {
            let r_of = |x: f32| x * k;
            let v = match p.kind() {
                3 => {
                    let r = r_of(fb(p.raw[7]));
                    r * 4.187_743 * r * r + r * 1.570_796_4 * r * (fb(p.raw[1]) * scale * (1.0 / 1024.0))
                }
                0..=2 => {
                    let r = if p.kind() == 2 { r_of(fb(p.raw[3])) } else { r_of(fb(p.raw[7])) };
                    r * 4.188_790_3 * r * r
                }
                4 => {
                    let [a, b] = p.joints();
                    let (a, b) = (joint(a as i32), joint(b as i32));
                    let e = [(a[0] - b[0]) * k, (a[1] - b[1]) * k, (a[2] - b[2]) * k];
                    let len = (e[0] * e[0] + e[1] * e[1] + e[2] * e[2]).sqrt();
                    let r = r_of(fb(p.raw[3]));
                    r * 1.570_796_4 * r * len
                }
                _ => 0.0,
            };
            vols.push((i, v));
        }
        if (w0 as i32) < 0 { break; }
    }
    let total: f32 = vols.iter().map(|v| v.1).sum();
    let n = ((density * total + 0.5) as i32).min(max);
    let mut out = Vec::new();
    for _ in 0..n.max(0) {
        let mut x = w.rng.randf(0.0, total);
        let mut pick = 0usize;
        if !vols.is_empty() && vols[0].1 <= x {
            pick = 1;
            let mut cur = vols[0].1;
            loop {
                x -= cur;
                if pick >= vols.len() { break; }
                cur = vols[pick].1;
                if x < cur { break; }
                pick += 1;
            }
        }
        let Some(&(pi, _)) = vols.get(pick.min(vols.len().saturating_sub(1))) else { break };
        let p = coll.prims[pi];
        let turn = |v: [f32; 3]| -> V {
            let t: [f32; 3] = std::array::from_fn(|l| rows[0][l] * v[0] + rows[1][l] * v[1] + rows[2][l] * v[2]);
            [t[0] + pos[0], t[1] + pos[1], t[2] + pos[2], pos[3]]
        };
        let ball = |w: &mut World, r: f32| -> [f32; 3] {
            loop {
                let v = [w.rng.randf(-r, r), w.rng.randf(-r, r), w.rng.randf(-r, r)];
                if (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() <= r { return v; }
            }
        };
        let q = match p.kind() {
            3 => {
                let c = [fb(p.raw[4]) * k, fb(p.raw[5]) * k, fb(p.raw[6]) * k];
                let r = fb(p.raw[7]) * k;
                let h = fb(p.raw[1]) * k;
                let d = w.rng.randf(0.0, r * r).sqrt();
                let z = w.rng.randf(0.0, h);
                let a = w.rng.rand_angle();
                [a.cos() * d + pos[0] + c[0], a.sin() * d + pos[1] + c[1], z + pos[2] + c[2], pos[3]]
            }
            0..=2 => {
                let (c, r) = if p.kind() == 2 {
                    let j = joint(p.word4());
                    ([(j[0] + fb(p.raw[4])) * k, (j[1] + fb(p.raw[5])) * k, (j[2] + fb(p.raw[6])) * k], fb(p.raw[3]) * k)
                } else {
                    ([fb(p.raw[4]) * k, fb(p.raw[5]) * k, fb(p.raw[6]) * k], fb(p.raw[7]) * k)
                };
                let v = ball(w, r);
                turn([v[0] + c[0], v[1] + c[1], v[2] + c[2]])
            }
            _ => {
                let [a, b] = p.joints();
                let (a, b) = (joint(a as i32), joint(b as i32));
                let t = w.rng.randf(0.0, 1.0);
                let c: [f32; 3] = std::array::from_fn(|l| ((b[l] * scale - a[l] * scale) * t + a[l] * scale) * (1.0 / 1024.0));
                let r = fb(p.raw[3]) * k;
                let v = ball(w, r);
                turn([v[0] + c[0], v[1] + c[1], v[2] + c[2]])
            }
        };
        out.push(q);
    }
    out
}

/// `BreakFxC(m, class)` 0x278e20 (`scaled`) or the inline copy of levels 6 / 7: a remains moby of `class` in `m`'s
/// place.
fn remains(w: &mut World, id: MobyId, class: i16, scaled: bool) {
    let (pos, rot, light, amb, scale, oc) = { let m = w.m(id); (m.position, m.rotation, m.light, m.ambient, m.scale, m.o_class) };
    let cs = w.class_scale(oc).to_f32();
    let Some(r) = w.create_moby(class) else { return };
    let m = w.mm(r);
    m.draw_dist = 0xff;
    m.visible = 1;
    m.position = pos;
    m.rotation = rot;
    m.light = light;
    m.ambient = amb;
    if scaled {
        m.scale *= scale / cs;
        m.update_dist = 0;
        m.mode = 0;
    }
    w.build_matrix(r);
}

/// The remains' update (0x30d200): nothing.
pub fn remains_update(_w: &mut World, _id: MobyId) {}

/// `fast_add_rotations` 0x221ff8: `a + b` wrapped once into [−π, π).
fn add_rot(a: f32, b: f32) -> f32 {
    use std::f32::consts::PI;
    let s = a + b;
    if s >= PI || s.is_nan() { (s - PI) - PI } else if s < -PI { (s + PI) + PI } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipes_are_distinct_and_their_piece_ops_are_sane() {
        let mut seen = std::collections::HashSet::new();
        for r in RECIPES {
            assert!(seen.insert((r.level, r.func)), "{:#x}", r.func);
            assert!(!r.classes.is_empty());
            for p in r.pieces {
                if let Piece::Burst { na, nb, n, .. } = p { assert!(*na >= 1 && *nb >= 1 && *n >= 1); }
            }
        }
        // Every class number belongs to one recipe only (the class-number fallback registry stays unambiguous).
        let mut cls = std::collections::HashSet::new();
        for r in RECIPES { for c in r.classes { assert!(cls.insert(*c), "class {c}"); } }
    }

    #[test]
    fn every_port_address_names_one_port() {
        use crate::moby_update::classes::ClassUpdate;
        let all: Vec<ClassUpdate> = ClassUpdate::every().collect();
        for u in &all {
            assert_eq!(ClassUpdate::from_address(u.key()), Some(*u), "{u:?} {:#x}", u.key());
        }
        for (i, r) in RECIPES.iter().enumerate() {
            // The class-number registry is level 01's (`classes::for_class`): the other levels' copies run through
            // their tables (`LevelPorts`, tests/classes/breakables_levels.rs).
            for c in r.classes {
                let got = crate::moby_update::classes::for_class(*c);
                if r.level == 1 { assert_eq!(got, Some(ClassUpdate::Breakable(i as u8)), "{c}") } else { assert_ne!(got, Some(ClassUpdate::Breakable(i as u8)), "{c}") }
            }
        }
    }

    #[test]
    fn add_rot_wraps_once() {
        assert!((add_rot(3.0, 1.0) - (4.0 - 2.0 * std::f32::consts::PI)).abs() < 1e-6);
        assert_eq!(add_rot(0.5, 0.25), 0.75);
    }
}
