//! Flow chutes, class 679: `FlowUpdate` level01 0x2f6328 (Ghidra `WaterCurrent679Update`; the catalogue's "water
//! current" twin of 613). It carries the hero down the chutes of the sinking floor (surface 4, state 0x31, group
//! 0x10) along its splines. Spec: docs/plan/hero_states.md P1 / "Hero follow-ups", docs/plan/triggers.md §5.
//! Native `f32`.
//!
//! **Levels.** The function is identical (clusters.tsv hash 78bc85ff…) in the four overlays that have the class:
//! level01 0x2f6328, level05 0x3067b8, level08 0x2f74c0, level15 0x2d95e8 ([`LEVEL_FNS`]); the registry maps the
//! class, so one port serves them all. Instances: Novalis 2 (the chute from (268.9, 199.3, 93.2) down to the landing
//! plateau at (176.6, 133.0, 59.5), splines 43 and 44; the second instance has no spline and never runs), Rilgar 1,
//! Blarg 2, Orxon 1. No geometry (headerless class).
//!
//! **Pvars** (0x80): +0x00.. the spline indices (s32; +0x00 = −1: inactive), +0x60 the flow speed (f32, u/tick;
//! level 1 recomputes it), +0x64 the reach (the hero within this of a spline), +0x68 initialised, +0x70 the spline
//! count.
//!
//! **Update.** Once: measure the splines (each point's w = its segment length, [`measure`]) and set the
//! update distance 0xff (always active). Then only while the hero is in group 0x10 (the sinking floor):
//! 1. For each spline the nearest point to the feet 0x13f3d0 (`0x272e28` = [`spline::nearest`], range 999, step 5,
//!    3-D); the nearest spline by the 2-D distance (`0x221398`) wins (from 9999). The position 0.3 further along it
//!    (`0x2726c8` = [`spline::advance`]). Farther than the reach (+0x64) → nothing.
//! 2. Level 1 only: the speed from the spline length left after that position: < 8 → 3 u/s, < 15 → 4, < 30 → 5,
//!    else 9 (×dt, into +0x60).
//! 3. The position ran off the end of the spline: the carried momentum 0x13f4a0 ×(1 − 0.01) becomes the push
//!    0x13f440 (yaw 0x13f44c = 0): the hero slides out of the chute.
//! 4. Otherwise, with `d` = ahead − nearest: the flow pitch 0x13fd24 = atan2(d.z, |d.xy|), yaw 0x13fd20 =
//!    atan2(d.y, d.x); the flow speed 0x13fd2c approaches +0x60 by 6·dt² a tick; the push 0x13f440 = d.xy at that
//!    speed (when |d.xy| > 0.001), plus the pull back to the spline: the pull speed 0x13fd30 is a spring (target 0,
//!    k 0.008, d 0.3, max 5·dt) on the xy distance to the nearest point, capped by that distance, and the xy offset
//!    to the nearest point at that length is added; 0x13f44c = 0; the momentum 0x13f4a0 = the push; the sinking
//!    floor's hold 0x13f530 = at least 30 ticks.
//!
//! The hero reads the push in 0x31's physics (turns toward it, `hero::surface`) and the move adds it to the
//! position; 0x13f530 keeps him in 0x31 for 30 ticks after the last push. The writes go through
//! [`World::hero_fields_mut`] (the tick applies them before the hero update).

use crate::moby_runtime::MobyId;
use crate::moby_update::services::{pvar as p, World};
use crate::spline::{self, Cursor, Point};

/// The flow update address in the level01 class table.
pub const UPDATE_FN: u32 = 0x2f6328;

/// Classes that run [`update`].
pub const CLASSES: [i16; 1] = [679];

/// The overlays that compile this function (level, address): the same code (clusters.tsv 78bc85ff…).
pub const LEVEL_FNS: [(u32, u32); 4] = [(1, 0x2f6328), (5, 0x3067b8), (8, 0x2f74c0), (15, 0x2d95e8)];

const DT: f32 = 1.0 / 60.0;
const DT2: f32 = DT * DT;

/// The spline indices of the pvars (count +0x70, up to 24 words before +0x60).
fn splines(pv: &[u8]) -> Vec<i32> {
    let n = p::i32(pv, 0x70).clamp(0, 0x18) as usize;
    (0..n).map(|k| p::i32(pv, 4 * k)).collect()
}

/// The init's segment-length pass: each point's w = |p(k+1) − p(k)| (`VecDistance` 0x221360), the last point's
/// back to point 0, written into the level's spline (`0x1b0930[i]`, shared: the game writes it in place too).
pub fn measure(s: &mut [[u32; 4]]) {
    let xyz: Vec<[f32; 3]> = s.iter().map(|q| [f32::from_bits(q[0]), f32::from_bits(q[1]), f32::from_bits(q[2])]).collect();
    for (q, m) in s.iter_mut().zip(spline::with_lengths(&xyz)) { q[3] = m[3].to_bits(); }
}

fn points(s: &[[u32; 4]]) -> Vec<Point> { s.iter().map(|q| q.map(f32::from_bits)).collect() }

/// `Spring(0, k, d, max, &x, &v)` 0x270780 on `f32` (see `hero::physics::spring`).
pub(crate) fn spring(k: f32, d: f32, max: f32, x: &mut f32, v: &mut f32) {
    let e = -*x;
    let mut nv = *v + (k * e - d * *v);
    if 0.0 < max { nv = nv.clamp(-max, max); }
    let ae = e.abs();
    nv = nv.clamp(-ae, ae);
    *v = nv;
    *x += nv;
    if x.abs() < max * 0.01 {
        *x = 0.0;
        *v = 0.0;
    }
}

pub fn update(w: &mut World, id: MobyId) {
    let pv = &w.m(id).pvars;
    if pv.len() < 0x74 || p::i32(pv, 0) == -1 || p::i32(pv, 0x70) == 0 { return; }
    let list = splines(pv);
    let valid = |w: &World, s: i32| usize::try_from(s).ok().filter(|&i| i < w.svc.splines.len());
    if p::i32(pv, 0x68) == 0 {
        p::set_i32(&mut w.mm(id).pvars, 0x68, 1);
        for &s in &list {
            if let Some(i) = valid(w, s) { measure(&mut w.svc.splines[i]); }
        }
        w.mm(id).update_dist = 0xff;
    }
    if w.hero.group != 0x10 { return; }
    let h = w.hero.pos.map(|x| x.to_f32());
    let feet = [h[0], h[1], h[2]];
    // 1. The nearest spline (2-D distance to its nearest point). The result slots persist across the splines, as
    // the game's stack slots do (a failed search keeps the previous one's).
    let mut best = 9999.0f32;
    let mut out = [0.0f32; 3];
    let mut cur = Cursor::default();
    let mut hit: Option<(Vec<Point>, [f32; 3], Cursor)> = None;
    for &s in &list {
        let Some(i) = valid(w, s) else { continue };
        let pts = points(&w.svc.splines[i]);
        if let Some((o, c)) = spline::nearest(&pts, false, 999.0, 5.0, 0.0, feet) { (out, cur) = (o, c); }
        let d = spline::dist2(feet, out);
        if d < best {
            best = d;
            hit = Some((pts, out, cur));
        }
    }
    let Some((pts, nearest, mut cur)) = hit else { return };
    let (ahead, ended) = spline::advance(&pts, false, 0.3, &mut cur);
    if p::ff(&w.m(id).pvars, 0x64) < best { return; }
    // 2. Level 1: the speed by the length left.
    if w.svc.level == 1 {
        let left: f32 = (cur.seg.max(0) as usize..pts.len().saturating_sub(1)).map(|k| pts[k][3]).sum();
        let k = if left < 8.0 { 3.0 } else if left < 15.0 { 4.0 } else if left < 30.0 { 5.0 } else { 9.0 };
        p::set_ff(&mut w.mm(id).pvars, 0x60, DT * k);
    }
    let speed = p::ff(&w.m(id).pvars, 0x60);
    let f = w.hero_fields_mut();
    if ended {
        // 3. Off the end: the momentum, slowly decaying, keeps pushing.
        let k = 1.0 + f32::from_bits(0xbc23_d700);
        for c in 0..3 { f.momentum[c] *= k; }
        f.platform = f.momentum;
        f.platform[3] = 0.0;
        return;
    }
    // 4. Along the flow, and back toward the spline.
    let mut d = [ahead[0] - nearest[0], ahead[1] - nearest[1], ahead[2] - nearest[2]];
    let dxy = (d[0] * d[0] + d[1] * d[1]).sqrt();
    f.flow[1] = d[2].atan2(dxy);
    f.flow[0] = d[1].atan2(d[0]);
    d[2] = 0.0;
    let step = DT2 * 6.0;
    f.flow[3] += (speed - f.flow[3]).clamp(-step, step);
    let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
    if f32::from_bits(0x3a83_126f) < l {
        let q = f.flow[3] / l;
        f.platform = [d[0] * q, d[1] * q, 0.0, f.platform[3]];
    }
    let mut to = [nearest[0] - feet[0], nearest[1] - feet[1], 0.0];
    let dist = (to[0] * to[0] + to[1] * to[1]).sqrt();
    let mut x = -dist;
    spring(f32::from_bits(0x3c03_126f), f32::from_bits(0x3e99_999a), DT * 5.0, &mut x, &mut f.flow[4]);
    if dist < f.flow[4] { f.flow[4] = dist; }
    if f.flow[4] < dist {
        let q = f.flow[4] / dist;
        to = [to[0] * q, to[1] * q, 0.0];
    }
    for (q, t) in f.platform.iter_mut().zip(to) { *q += t; }
    f.platform[3] = 0.0;
    f.momentum = f.platform;
    if f.sink_hold < 30 { f.sink_hold = 30; }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hero::Hero;
    use crate::moby_runtime::{Moby, MobyTable};
    use crate::moby_update::services::{ClassTable, Services};
    use crate::rng::Rng;

    fn pvars(splines: &[i32], reach: f32, speed: f32) -> Vec<u8> {
        let mut pv = vec![0u8; 0x80];
        for (k, &s) in splines.iter().enumerate() { p::set_i32(&mut pv, 4 * k, s); }
        p::set_ff(&mut pv, 0x60, speed);
        p::set_ff(&mut pv, 0x64, reach);
        p::set_i32(&mut pv, 0x70, splines.len() as i32);
        pv
    }

    /// A straight chute along +x from (0, 0, 10) sloping down, 11 points 2 apart.
    fn chute() -> Vec<[f32; 4]> { (0..11).map(|k| [2.0 * k as f32, 0.0, 10.0 - 0.5 * k as f32, -1.0]).collect() }

    fn run(hero: &Hero, svc: &mut Services, table: &mut MobyTable, counter: u64) {
        let classes = ClassTable::default();
        let mut rng = Rng::new();
        let mut w = World::new(table, hero, &mut rng, &classes, svc, counter);
        update(&mut w, 0);
    }

    fn setup(level: u32) -> (Services, MobyTable) {
        let mut svc = Services::new();
        svc.level = level;
        svc.set_splines(&[chute()]);
        let mut m = Moby::zeroed();
        m.pvars = pvars(&[0], 10.0, 0.2);
        (svc, MobyTable::new(vec![m], 0))
    }

    /// The init's measure: each w the chord to the next point, the last back to the first.
    #[test]
    fn measure_writes_segment_lengths() {
        let mut s: Vec<[u32; 4]> = chute().iter().map(|p| p.map(f32::to_bits)).collect();
        measure(&mut s);
        assert!((f32::from_bits(s[0][3]) - (4.0f32 + 0.25).sqrt()).abs() < 1e-6);
        assert!((f32::from_bits(s[10][3]) - (400.0f32 + 25.0).sqrt()).abs() < 1e-4);
        assert_eq!(f32::from_bits(s[3][0]), 6.0, "positions untouched");
    }

    /// Init measures the spline and sets the update distance; nothing happens outside the sinking floor; in it the
    /// hero gets the push along the chute, the pull back to it, the 30-tick hold, the flow angles; off the end the
    /// decaying momentum.
    #[test]
    fn flow_pushes_the_sinking_hero_along() {
        let (mut svc, mut table) = setup(5);
        let mut hero = Hero::spawn([5.0, 1.0, 8.0], 0.0);
        run(&hero, &mut svc, &mut table, 1);
        assert_eq!(table.mobys[0].update_dist, 0xff);
        assert_eq!(p::i32(&table.mobys[0].pvars, 0x68), 1);
        assert!(f32::from_bits(svc.splines[0][0][3]) > 0.0, "measured");
        assert!(svc.hero_writes.is_none(), "not in the sinking floor: no write");
        hero.group = 0x10;
        run(&hero, &mut svc, &mut table, 2);
        let f = svc.take_hero_writes().expect("the flow wrote the hero");
        assert_eq!(f.sink_hold, 30);
        assert!((f.flow[3] - 6.0 * DT2).abs() < 1e-7, "speed ramps by 6·dt²: {}", f.flow[3]);
        assert!(f.flow[0].abs() < 1e-5, "flow yaw along +x");
        assert!(f.flow[1] < 0.0, "flow pitch downhill");
        assert!(f.platform[0] > 0.0 && f.platform[1] < 0.0, "along +x and back toward y = 0: {:?}", f.platform);
        assert_eq!(f.platform[3], 0.0);
        assert_eq!(f.momentum, f.platform);
        // A second class in the same tick sees the first one's writes (the read-through view).
        let classes = ClassTable::default();
        let mut rng = Rng::new();
        {
            let mut w = World::new(&mut table, &hero, &mut rng, &classes, &mut svc, 3);
            w.hero_fields_mut().sink_hold = 7;
            assert_eq!(w.hero_fields().sink_hold, 7);
            update(&mut w, 0);
            assert_eq!(w.hero_fields().sink_hold, 30);
        }
        // Applied to the hero; a stale write (an earlier tick's) is not read back.
        svc.take_hero_writes().unwrap().apply(&mut hero);
        assert_eq!(hero.f530, 30);
        svc.hero_writes = Some((1, crate::moby_update::services::HeroFields::of(&Hero::spawn([0.0; 3], 0.0))));
        {
            let w = World::new(&mut table, &hero, &mut rng, &classes, &mut svc, 9);
            assert_eq!(w.hero_fields().sink_hold, 30);
        }
        // Past the end of the chute: the momentum ×0.99 becomes the push.
        let mut end_hero = hero.clone();
        end_hero.pos = [20.5, 0.0, 5.0, 0.0].map(crate::ps2v::Pf::f);
        end_hero.momentum = [0.1, 0.0, 0.0, 0.0].map(crate::ps2v::Pf::f);
        run(&end_hero, &mut svc, &mut table, 10);
        let f = svc.take_hero_writes().unwrap();
        assert!((f.platform[0] - 0.1 * (1.0 + f32::from_bits(0xbc23_d700))).abs() < 1e-7, "{:?}", f.platform);
        assert_eq!(f.sink_hold, 30, "the hold is not renewed off the end");
    }

    /// Level 1's speed by the length left: 9 u/s far from the end, 3 u/s within 8 of it.
    #[test]
    fn level1_speed_by_length_left() {
        let (mut svc, mut table) = setup(1);
        let mut hero = Hero::spawn([1.0, 0.0, 9.5], 0.0);
        hero.group = 0x10;
        run(&hero, &mut svc, &mut table, 1);
        assert!((p::ff(&table.mobys[0].pvars, 0x60) - 5.0 * DT).abs() < 1e-7, "~19 left: 5 u/s");
        hero.pos[0] = crate::ps2v::Pf::f(16.0);
        run(&hero, &mut svc, &mut table, 2);
        assert!((p::ff(&table.mobys[0].pvars, 0x60) - 3.0 * DT).abs() < 1e-7, "~4 left: 3 u/s");
        // Beyond the reach: nothing written.
        svc.hero_writes = None;
        hero.pos[1] = crate::ps2v::Pf::f(11.0);
        run(&hero, &mut svc, &mut table, 3);
        assert!(svc.hero_writes.is_none());
    }
}
