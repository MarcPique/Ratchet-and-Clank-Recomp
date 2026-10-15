//! **The Thruster-Pack's flames**, class 0xa7 (level01: creation `FUN_002c9da0`, update `0x2c9e00`, emitter
//! `0x2c98b8`, draw callback `0x2c9290` on list 2; read from the decompiler C and the disassembly; the class has no
//! model and is in every level's class list, its table entry a copy of the same code). `HeroItemsCreate` 0x22f3c0
//! makes two (sides 0 and 1) whenever it creates the back pack for back item 3 with Clank shown
//! (`crate::hero::packs::flames_on_create`); each follows one nozzle of the Thruster-Pack and burns while the hero
//! thrusts. docs/plan/hero_states.md "Thruster flames".
//!
//! **Pvars** ([`pv`]): +0x00 timer, +0x04 side, +0x08 the pack moby (0x1404d0; here 1 while the back has one), +0x0c
//! "last smoke point valid", +0x10 the last smoke point, +0x20..+0x2c the sizes (the mesh 0.0125, two unused words 0.2
//! and 0.05, the quads 0.025), +0x30..+0x3c their per-tick steps; the port adds +0x40..+0x4c, the four corner jitters
//! the draw callback drew ([`draw_callback`]).
//!
//! **The update** (`0x2c9e00`, in the moby loop: it reads the hero as the last hero update left him). "On" (only with
//! back item 3, 0x1404f8, and in the update's states 1 and 3): hero state 0xb..0xe, or the ledge climb 0x1c on its key
//! times 6..16 with no blend running — both only while not descending (0x13f76e = 0); the Thruster long jump 0x10 before
//! tick 44; the stomp 0x22 before tick 15 or, still airborne (0x13f65e), after tick 33; the glide 8 and the hover
//! 0x81. States (+0x20):
//! * 0 → 1: update distance 0xff, hidden (mode |= 1), z + 0.5.
//! * 1: the back's ready item (`GetClankModule(3)`) not 3 → `DeleteMoby`; on → 2: the sizes grow from 0 over
//!   `ticks(2)`.
//! * 2: grow, register the callback; the timer out → 3 with the full sizes and no last smoke point.
//! * 3: register; on: the mesh size back to 0.0125 and the exhaust ([`emit`]); off → 4: shrink over `ticks(8)`.
//! * 4: shrink and register until the timer runs out → 1.
//!
//! **The exhaust** (`0x2c98b8`, every "on" tick of state 3), in this order of draws: a spark velocity
//! `rot((0.05·cos a, 0.05·sin b, −randf(0.01, 0.03)))` (`rand_angle` twice, then `randf`), one type-21 spark at the
//! nozzle (size 10000, orange 0x4f007fff → white 0x1fffffff, `ticks(10)`, splitting; in the hover 0x81 only on ticks
//! divisible by 3) and, with a last smoke point, one more halfway between it and the nozzle; `randi(ticks(10))` into
//! the timer; the smoke point `nozzle + rot((0, 0, −0.3))` with the velocity `rot((0, 0, −0.075))` whose z is replaced
//! by 0.025 (the glide 8: 0.025 − 5·dt; the hover: 0.025 − 3·dt); one type-23 puff there (jitter 0.05, growth 1.01 ..
//! 1.075 — 1.0165 in the glide, 1.037 in the hover —, size 30000, spin 6, grey 0x808080 — 0xa0a0a0 in the glide and
//! the hover —, ALPHA 0x44: blended smoke, not additive) and, with a last smoke point, 3 more puffs (1 in the glide,
//! none in the hover) at 1/6, 2/6, 3/6 of the way back toward it; the smoke point is kept. `rot` is the flame's rows
//! (+0xc0), set by the draw callback: z along the nozzle, pointing into the pack.
//!
//! **The draw callback** (`0x2c9290`, list 2; state part [`draw_callback`], run by
//! `super::draw_callbacks::run_frame` at the place of the frame render): with the pack moby of class 0x260 (608), the
//! world points of the pack's joint lists (2, 3) — (0, 1) for side 1 — `p0`, `p1` (`0x264630`); the flame's frame:
//! Z = unit(p0 − p1), X = unit(Z × (0, 1, 0)), Y = X × Z, at `p1`; then two quads whose far corners are pushed back by
//! `randf(0, 0.1)` each (four draws, [`pv::JITTER`]). The drawing itself (the quads, a camera-facing glow, the
//! 196-vertex flame mesh with its scrolling texture) is `rc-engine`'s (thruster_render). Standard `f32`.

use super::draw_callbacks::Callback;
use crate::moby_runtime::MobyId;
use crate::moby_update::services::{pvar, World};
use crate::particles::{type21, type23};

/// The update address in the level01 class table.
pub const UPDATE_FN: u32 = 0x2c9e00;
/// The draw callback (`FUN_0021b198(0x2c9290, moby)`).
pub const DRAW_FN: u32 = 0x2c9290;
/// `CreateMoby(0xa7)`.
pub const CLASS: i16 = 0xa7;
pub const CLASSES: [i16; 1] = [CLASS];
/// The Thruster-Pack's class (0x260) and back item.
pub const PACK_CLASS: i16 = 0x260;
pub const THRUSTER: i32 = 3;

/// Pvar offsets (module doc).
pub mod pv {
    pub const TIMER: usize = 0x00;
    pub const SIDE: usize = 0x04;
    pub const PACK: usize = 0x08;
    pub const PREV_OK: usize = 0x0c;
    pub const PREV: usize = 0x10;
    /// +0x20 the mesh size, +0x24 / +0x28 unused sizes, +0x2c the quads' size.
    pub const SIZES: usize = 0x20;
    pub const STEPS: usize = 0x30;
    /// The port's: the four corner jitters of the last callback (quad 1 corners 1, 3; quad 2 corners 1, 3).
    pub const JITTER: usize = 0x40;
}

/// The full sizes of state 3 (mesh, two unused, quads).
pub const SIZES: [f32; 4] = [0.0125, 0.2, 0.05, 0.025];
/// The joint lists of the pack (`0x1614b0` for side ≠ 0, `0x1614b8` for side 0): (p0, p1).
pub const LISTS: [[usize; 2]; 2] = [[2, 3], [0, 1]];

/// `FUN_002c9da0(side)` after `CreateMoby(0xa7)`: pvars cleared with the side, +0x30 = 0xff, draw distance 0x7e,
/// +0x31 = 0, state 0.
pub fn fill(m: &mut crate::moby_runtime::Moby, side: i32) {
    if m.pvars.len() < 0x50 { m.pvars.resize(0x50, 0); }
    m.pvars.fill(0);
    pvar::set_i32(&mut m.pvars, pv::SIDE, side);
    m.update_dist = 0xff;
    m.draw_dist = 0x7e;
    m.visible = 0;
    m.state = 0;
}

/// Whether the hero thrusts (the update's `bVar2`, module doc).
pub fn thrusting(h: &crate::hero::Hero, ticks: impl Fn(i32) -> i32) -> bool {
    if h.back_slot.slot.id != THRUSTER { return false; }
    let (s, t, v) = (h.state, h.timer, &h.fx.view);
    let mut on = false;
    if (0xb..=0xe).contains(&s) || (s == 0x1c && v.seq_a == v.seq_b && 6.0 <= v.frame && v.frame <= 16.0) {
        on = h.jump.descending == 0;
    }
    if s == 0x10 && t < ticks(0x2c) { on = true; }
    if s == 0x22 && (t < ticks(0xf) || (ticks(0x21) < t && h.air_ticks != 0)) { on = true; }
    if s == 8 || s == 0x81 { on = true; }
    on
}

fn sizes(w: &World, id: MobyId, o: usize) -> [f32; 4] { std::array::from_fn(|k| pvar::ff(&w.m(id).pvars, o + 4 * k)) }
fn set_sizes(w: &mut World, id: MobyId, o: usize, v: [f32; 4]) { for (k, x) in v.into_iter().enumerate() { pvar::set_ff(&mut w.mm(id).pvars, o + 4 * k, x); } }

/// +0x08 = the pack moby 0x1404d0 (1 while the back has one) and the callback on list 2.
fn keep_pack_and_register(w: &mut World, id: MobyId) {
    let pack = w.hero.back.as_ref().is_some_and(|b| b.state != 0) as i32;
    pvar::set_i32(&mut w.mm(id).pvars, pv::PACK, pack);
    w.svc.draw_callbacks.register2(Callback::ThrusterFlame, id);
}

/// `FastDecTimer__FRi` on +0x00: non-zero when it was 0 or runs out now.
fn dec_timer(w: &mut World, id: MobyId) -> bool {
    let mut t = pvar::i32(&w.m(id).pvars, pv::TIMER);
    let r = crate::hero::idle::dec_timer(&mut t);
    pvar::set_i32(&mut w.mm(id).pvars, pv::TIMER, t);
    r != 0
}

/// Sizes and steps for a ramp over `n` ticks (up from 0 in state 2, down in state 4).
fn ramp(w: &mut World, id: MobyId, n: i32) {
    pvar::set_i32(&mut w.mm(id).pvars, pv::TIMER, n);
    let steps = SIZES.map(|x| x / n as f32);
    set_sizes(w, id, pv::STEPS, steps);
}

/// `0x2c9e00`.
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x50 { w.mm(id).pvars.resize(0x50, 0); }
    let ticks = |n: i32| w.svc.timing.ticks(n);
    let st = w.m(id).state;
    let on = matches!(st, 1 | 3) && thrusting(w.hero, ticks);
    match st {
        0 => {
            let m = w.mm(id);
            m.state = 1;
            m.update_dist = 0xff;
            m.mode |= 1;
            m.position[2] += 0.5;
        }
        1 => {
            if w.hero.back_module() != THRUSTER {
                w.delete_moby(id);
                return;
            }
            if on {
                w.mm(id).state = 2;
                let n = w.svc.timing.ticks(2);
                ramp(w, id, n);
                set_sizes(w, id, pv::SIZES, [0.0; 4]);
            }
        }
        2 => {
            let (s, d) = (sizes(w, id, pv::SIZES), sizes(w, id, pv::STEPS));
            set_sizes(w, id, pv::SIZES, std::array::from_fn(|k| s[k] + d[k]));
            keep_pack_and_register(w, id);
            if dec_timer(w, id) {
                w.mm(id).state = 3;
                set_sizes(w, id, pv::SIZES, SIZES);
                pvar::set_i32(&mut w.mm(id).pvars, pv::PREV_OK, 0);
            }
        }
        3 => {
            keep_pack_and_register(w, id);
            if on {
                pvar::set_ff(&mut w.mm(id).pvars, pv::SIZES, SIZES[0]);
                emit(w, id);
            } else {
                w.mm(id).state = 4;
                let n = w.svc.timing.ticks(8);
                ramp(w, id, n);
            }
        }
        4 => {
            if !dec_timer(w, id) {
                let (s, d) = (sizes(w, id, pv::SIZES), sizes(w, id, pv::STEPS));
                set_sizes(w, id, pv::SIZES, std::array::from_fn(|k| s[k] - d[k]));
                keep_pack_and_register(w, id);
            } else {
                w.mm(id).state = 1;
            }
        }
        _ => {}
    }
}

fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn len3(a: [f32; 3]) -> f32 { (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt() }
/// `FastVecNormalize(len, out, v)`: `v` scaled to length `len` (0 stays 0).
fn with_len(a: [f32; 3], l: f32) -> [f32; 3] {
    let n = len3(a);
    if n == 0.0 { [0.0; 3] } else { a.map(|x| x * (l / n)) }
}
/// `v` through the flame's rows (+0xc0): `x·r0 + y·r1 + z·r2`.
fn rot(rows: &[[f32; 4]; 3], v: [f32; 3]) -> [f32; 3] { std::array::from_fn(|k| rows[0][k] * v[0] + rows[1][k] * v[1] + rows[2][k] * v[2]) }
fn v4(p: [f32; 3]) -> [f32; 4] { [p[0], p[1], p[2], 1.0] }

/// The exhaust of one tick in state 3, `0x2c98b8` (module doc).
pub fn emit(w: &mut World, id: MobyId) {
    let (rows, pos) = { let m = w.m(id); ([m.rows[0], m.rows[1], m.rows[2]], [m.position[0], m.position[1], m.position[2]]) };
    let hs = w.hero.state;
    let a = w.rng.rand_angle();
    let x = a.cos() * 0.05;
    let b = w.rng.rand_angle();
    let y = b.sin() * 0.05;
    let z = -w.rng.randf(0.01, 0.03);
    let sv = rot(&rows, [x, y, z]);
    let t10 = w.svc.timing.ticks(10);
    let prev_ok = pvar::i32(&w.m(id).pvars, pv::PREV_OK) != 0;
    let prev = { let p = pvar::v4f(&w.m(id).pvars, pv::PREV); [p[0], p[1], p[2]] };
    let spark = |w: &mut World, p: [f32; 3]| {
        if let Some(sys) = w.particles.as_deref_mut() {
            type21::spawn_rng(sys, w.rng, 10000.0, v4(p), [sv[0], sv[1], sv[2], 0.0], 0x4f00_7fff, 0x1fff_ffff, t10, 1);
        }
    };
    if hs != 0x81 || w.counter.is_multiple_of(3) { spark(w, pos); }
    if prev_ok {
        let d = len3(sub3(prev, pos));
        spark(w, add3(pos, with_len(sub3(prev, pos), d * 0.5)));
    }
    let r = w.rng.randi(t10);
    pvar::set_i32(&mut w.mm(id).pvars, pv::TIMER, r);
    let smoke = add3(pos, rot(&rows, [0.0, 0.0, -0.3]));
    let mut vel = rot(&rows, [0.0, 0.0, -0.075]);
    vel[2] = 0.025;
    let dt = 1.0 / 60.0;
    let (mut rgba, mut hi) = (0x80_8080u32, f32::from_bits(0x3f89_999a));
    if hs == 8 {
        (rgba, hi) = (0xa0_a0a0, f32::from_bits(0x3f82_1cac));
        vel[2] = 0.025 - dt * 5.0;
    }
    if hs == 0x81 {
        (rgba, hi) = (0xa0_a0a0, f32::from_bits(0x3f84_bc6a));
        vel[2] -= dt * 3.0;
    }
    let puff = |w: &mut World, p: [f32; 3]| {
        let Some(sys) = w.particles.as_deref_mut() else { return };
        if let Some(i) = type23::spawn(sys, w.rng, f32::from_bits(0x3d4c_cccd), f32::from_bits(0x3f81_47ae), hi, 30000.0, v4(p), 6, [vel[0], vel[1], vel[2], 0.0], rgba) {
            sys.pool.recs[i][3] = 0x44;
        }
    };
    puff(w, smoke);
    if prev_ok {
        let n = match hs { 8 => 2, 0x81 => 1, _ => 4 };
        let d = len3(sub3(prev, smoke));
        for i in 1..n { puff(w, add3(smoke, with_len(sub3(prev, smoke), d * 0.16667 * i as f32))); }
    }
    let p = &mut w.mm(id).pvars;
    pvar::set_v4f(p, pv::PREV, v4(smoke));
    pvar::set_i32(p, pv::PREV_OK, 1);
}

/// The flame's frame from the pack's joint points `p0`, `p1` (module doc): rows X, Y, Z and the point `p1`.
pub fn frame(p0: [f32; 3], p1: [f32; 3]) -> [[f32; 4]; 4] {
    let z = with_len(sub3(p0, p1), 1.0);
    // FastVecCross(out, a, b) = b × a: X = Z × (0, 1, 0), Y = X × Z.
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let x = with_len(cross(z, [0.0, 1.0, 0.0]), 1.0);
    let y = cross(x, z);
    [[x[0], x[1], x[2], 0.0], [y[0], y[1], y[2], 0.0], [z[0], z[1], z[2], 0.0], [p1[0], p1[1], p1[2], 1.0]]
}

/// The state part of the draw callback `0x2c9290` (module doc): the flame's frame from the pack's pose, then the four
/// corner jitters (`randf(0, 0.1)` each).
pub fn draw_callback(w: &mut World, id: MobyId) {
    if pvar::i32(&w.m(id).pvars, pv::PACK) == 0 { return; }
    let h = w.hero;
    if h.back.as_ref().is_none_or(|b| b.state == 0 || b.pack_o_class != PACK_CLASS) { return; }
    let side = (pvar::i32(&w.m(id).pvars, pv::SIDE) != 0) as usize;
    let [la, lb] = LISTS[side];
    let (Some(p0), Some(p1)) = (crate::hero::fx::pack_point(h, la), crate::hero::fx::pack_point(h, lb)) else { return };
    let f = frame([p0[0], p0[1], p0[2]], [p1[0], p1[1], p1[2]]);
    let m = w.mm(id);
    m.rows[0] = f[0];
    m.rows[1] = f[1];
    m.rows[2] = f[2];
    m.position = f[3];
    for k in 0..4 {
        let j = w.rng.randf(0.0, 0.1);
        pvar::set_ff(&mut w.mm(id).pvars, pv::JITTER + 4 * k, j);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hero::Hero;
    use crate::moby_runtime::{ClassInfo, Moby, MobyTable};
    use crate::moby_update::scheduler::{self, Scheduler};
    use crate::moby_update::services::{ClassTable, Services};
    use crate::particles::Particles;
    use crate::rng::Rng;

    /// Runs a flame for `n` ticks with the hero `setup` gives per tick; per tick the flame's state and the type-21 /
    /// type-23 records created.
    fn run(n: u64, mut setup: impl FnMut(u64, &mut Hero)) -> Vec<(u8, usize, usize)> {
        let mut h = Moby::init_instance(0, 0, Some(&ClassInfo { scale: 1.0, ..Default::default() }));
        h.mode |= crate::moby_runtime::mode::NO_UPDATE;
        let mut table = MobyTable::new(vec![h], 16);
        let mut ct = ClassTable::default();
        ct.classes.insert(CLASS, (ClassInfo { slot: 1, update_fn: scheduler::port_update_fn(CLASS), ..Default::default() }, None));
        let f = table.create(CLASS, ct.classes.get(&CLASS).map(|c| &c.0), 0).unwrap();
        fill(&mut table.mobys[f], 0);
        let mut hero = Hero::new();
        hero.back_slot.slot.id = THRUSTER;
        hero.back_slot.slot.state = 2;
        let (mut rng, mut svc, mut sched) = (Rng::new(), Services::new(), Scheduler::new());
        let mut parts = Particles::new(None, Vec::new());
        let count = |p: &Particles, ty: u8| p.pool.recs.iter().enumerate().filter(|(i, r)| r[0] == ty && p.pool.bitmap[i / 8] & (1 << (i % 8)) != 0).count();
        let mut out = Vec::new();
        for counter in 0..n {
            setup(counter, &mut hero);
            let (s0, p0) = (count(&parts, 21), count(&parts, 23));
            table.free_slot_pass(counter);
            let mut w = crate::moby_update::services::World::new(&mut table, &hero, &mut rng, &ct, &mut svc, counter);
            w.particles = Some(&mut parts);
            sched.tick(&mut w);
            let st = if table.mobys[f].state >= 0xfd { 0xff } else { table.mobys[f].state };
            out.push((st, count(&parts, 21) - s0, count(&parts, 23) - p0));
        }
        out
    }

    /// `0x2c9e00` through a Thruster long jump: 0 → 1 → 2 (grow for `ticks(2)`) → 3 (burning: the first tick one spark
    /// and one puff, then two sparks and four puffs a tick) → 4 at tick 44 (shrink for `ticks(8)`) → 1; the pack
    /// put away in state 1 deletes it.
    #[test]
    fn long_jump_states_and_exhaust_rate() {
        let rows = run(70, |t, h| {
            (h.state, h.timer) = (0x10, t as i32 - 2);
            if t >= 64 { h.back_slot.slot.state = 3; }
        });
        let states: Vec<u8> = rows.iter().map(|r| r.0).collect();
        assert_eq!(&states[..5], &[1, 2, 2, 3, 3], "{states:?}");
        assert_eq!((rows[4].1, rows[4].2), (1, 1), "first burning tick");
        assert!(rows[5..46].iter().all(|r| r.0 == 3 && (r.1, r.2) == (2, 4)), "{:?}", &rows[5..46]);
        // Timer 44 (tick 46): off → 4 for ticks(8), then 1; nothing more is emitted.
        assert_eq!(&states[46..55], &[4, 4, 4, 4, 4, 4, 4, 4, 1], "{states:?}");
        assert!(rows[46..].iter().all(|r| (r.1, r.2) == (0, 0)));
        // The pack being put away (tick 64): the flame deletes itself.
        assert_eq!(states[63], 1);
        assert_eq!(rows.last().unwrap().0, 0xff, "deleted once the Thruster-Pack is no longer the ready back item");
    }

    /// The hover 0x81: one puff a tick (the smoke rises less: 0xa0a0a0), the nozzle spark every third tick, the
    /// midpoint spark every tick; the glide 8: two puffs a tick.
    #[test]
    fn hover_and_glide_rates() {
        let hover = run(30, |_, h| (h.state, h.timer) = (0x81, 5));
        for (t, r) in hover.iter().enumerate().skip(6) {
            assert_eq!((r.1, r.2), (1 + (t % 3 == 0) as usize, 1), "tick {t}");
        }
        let glide = run(30, |_, h| (h.state, h.timer) = (8, 5));
        assert!(glide[6..].iter().all(|r| (r.1, r.2) == (2, 2)), "{glide:?}");
    }

    /// Not thrusting (a plain jump that is descending): the flame waits in state 1.
    #[test]
    fn idle_flame_waits() {
        let rows = run(20, |_, h| {
            h.state = 0xb;
            h.jump.descending = 1;
        });
        assert!(rows[1..].iter().all(|r| r.0 == 1 && (r.1, r.2) == (0, 0)), "{rows:?}");
    }

    /// The frame: z along p0 − p1, x ⟂ (0, 1, 0), a right-handed orthonormal set at p1.
    #[test]
    fn frame_is_orthonormal_along_the_nozzle() {
        let f = frame([1.0, 2.0, 5.0], [1.0, 2.0, 3.0]);
        assert_eq!(f[2], [0.0, 0.0, 1.0, 0.0]);
        assert_eq!(f[3], [1.0, 2.0, 3.0, 1.0]);
        let (x, y) = (f[0], f[1]);
        assert!((x[0] * x[0] + x[1] * x[1] + x[2] * x[2] - 1.0).abs() < 1e-6);
        assert!(x[1].abs() < 1e-6 && (y[0] * x[0] + y[1] * x[1] + y[2] * x[2]).abs() < 1e-6);
    }
}
