//! The breakables' effect mobys, one shared system for every breakable (crates, TNT, and any later caller of
//! the same spawners): the flying **debris pieces** and the **explosion flashes**. The game has one spawner
//! and one update for each, used by every breakable; the caller only passes arguments (class, velocity,
//! pose, scale range, bounce count; flash colour, size, duration). Spec: `docs/plan/moby_update_catalogue.md`
//! "In the port: debris pieces and explosion flashes".
//!
//! | what | level01 fn | port |
//! |---|---|---|
//! | debris spawn | `DebrisSpawn` 0x2c5080 | [`spawn`] |
//! | debris update, classes 149, 348, 349, 354–359, 363, 364 | `DebrisUpdate` 0x2c5218 | [`update`] |
//! | flash spawn | `FlashSpawn` 0x2c20e0, `0x309a68` (class 1192) | [`flash_spawn_as`] ([`FlashKind`]) |
//! | flash update, classes 112 (0x70), 1192 | `FlashUpdate` 0x2c22a8 | [`flash_update`] |
//!
//! The class lists are the level class-update tables' (0x20bb00 in level01): every level ELF whose table was
//! read (01, 05, 06, 07, 09, 11, 12) maps exactly these classes to its copy of each function, so the registry
//! holds one list per function, not per level or per breakable. The crates' "chunk" classes of 503, 504 and
//! 506–510 (351/352, 360/361, 366/367, 369/370) have no table entry in any of them: the game spawns them
//! without an update (mode 2), and so does the port.
//!
//! **Debris pvar block** (written by [`spawn`]): 0x00 parent (`index + 1`), 0x04 f32 spawn z, 0x08 f32
//! collision radius (scale factor / 4), 0x0c s16 bounces left, 0x0e s16 no-collision timer (`ticks(20)`),
//! 0x10 vec velocity (units per tick), 0x20 / 0x24 / 0x28 f32 spin per tick (x, y, z).
//!
//! **Debris behaviour** (0x2c5218, every tick, both states): gravity `20·dt²` on v.z, `pos += v`,
//! `rot += spin` (wrapped to ±π). State 0 (flying): once 3 below the spawn height → state 1. Otherwise, after
//! the timer has run out and while bounces are left, a collision sphere (`0x212960(radius, pos, 2, none)`:
//! world mesh and moby meshes) against a face the piece moves into (`v·n < 0`) puts the piece at the pushed-out
//! centre, reflects v off the face and scales it by 0.75, and draws a new spin (`x` 0, `y` / `z`
//! `randf_sym(0, 360)°·dt`: 2 draws). With no bounces left the piece falls through the floor. State 1
//! (fading): scale ×0.9 and alpha −4 per tick; deleted when alpha < 4 (33 ticks from 0x80).
//!
//! **Flash pvar block** (written by [`flash_spawn`]): 0x00 vec (unused by the update), 0x10 parent
//! (`index + 1`), 0x14 s32 duration T, 0x18 f32 full size, 0x1c s16 timer, 0x1e s16 start alpha.
//!
//! **Flash behaviour** (0x2c22a8): `scale = (T − timer)·size / T` (grows linearly to full size), each Euler
//! angle `+= π·dt`, then the timer ticks down: deleted when it runs out; in its second half the alpha is
//! `timer·alpha₀ / (T/2)` (integer division). No RNG in the update.
//!
//! Standard `f32` arithmetic throughout (the `Pf` values of the callers are converted at the boundary).

use crate::moby_runtime::MobyId;
use crate::moby_update::services::{self as sv, pvar as p, World};
use std::f32::consts::{PI, TAU};

/// The debris update address in the level01 class table.
pub const UPDATE_FN: u32 = 0x2c5218;
/// The flash update address in the level01 class table.
pub const FLASH_UPDATE_FN: u32 = 0x2c22a8;

/// Classes the level tables map to the debris update (the same list in every level table read).
pub const CLASSES: [i16; 11] = [149, 348, 349, 354, 355, 356, 357, 358, 359, 363, 364];
/// Classes the level tables map to the flash update.
pub const FLASH_CLASSES: [i16; 2] = [0x70, 1192];

/// The class `FlashSpawn` creates.
pub const FLASH_CLASS: i16 = 0x70;

/// Degrees to radians (`0x3c8efa35`).
const DEG: f32 = PI / 180.0;
/// Debris gravity, units per second² (`v.z −= 20·dt²` per tick).
const GRAVITY: f32 = 20.0;
/// How far below its spawn height a flying piece starts to fade.
const FADE_DROP: f32 = 3.0;
/// Speed kept on a bounce.
const BOUNCE_KEEP: f32 = 0.75;
/// Spin range of a bounce (and of the spawn), degrees per second (`gp−0x57b8` / `gp−0x57b4` in level01).
const SPIN_LO: f32 = 0.0;
const SPIN_HI: f32 = 360.0;
/// Per-tick fade of a falling-away piece (`gp−0x5790` scale factor, `gp−0x578c` alpha step, level01).
const FADE_SCALE: f32 = 0.9;
const FADE_ALPHA: u8 = 4;

/// Debris states (moby +0x20).
const FLYING: u8 = 0;
const FADING: u8 = 1;

fn dt() -> f32 { sv::DT.to_f32() }
fn dt2() -> f32 { sv::DT2.to_f32() }

/// `fast_add_rotations`: `a + b` wrapped into [−π, π).
fn add_rot(a: f32, b: f32) -> f32 {
    let s = a + b;
    if s >= PI {
        s - TAU
    } else if s < -PI {
        s + TAU
    } else {
        s
    }
}

fn dot3(a: [f32; 4], b: [f32; 3]) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }

/// `FUN_00221570`: `v` reflected off the plane with normal direction `n` (any length) when it moves into it.
fn reflect(v: [f32; 4], n: [f32; 3]) -> [f32; 4] {
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if l == 0.0 { return v; }
    let nh = [n[0] / l, n[1] / l, n[2] / l];
    let d = dot3(v, nh);
    if d >= 0.0 { return v; }
    [v[0] - 2.0 * d * nh[0], v[1] - 2.0 * d * nh[1], v[2] - 2.0 * d * nh[2], v[3]]
}

/// A spin component of a debris piece: `randf_sym(0, 360)` degrees per second as radians per tick (1 draw).
fn random_spin(w: &mut World) -> f32 { w.rng.randf_sym(SPIN_LO, SPIN_HI) * DEG * dt() }

// ---------------------------------------------------------------------------------------------------
// Debris pieces

/// `DebrisSpawn(a, b, parent, vel, pos, rot, class, bounces)` 0x2c5080: a debris piece of `class` at `pos` with
/// Euler `rot`, velocity `vel` (units per tick), `bounces` bounces, its scale multiplied by `randf(a, b)`,
/// lit like `parent`. Six draws when a moby is created (3 × `rand_angle`, overwritten by `rot`; 2 ×
/// `randf_sym(0, 360)` spin; `randf(a, b)`), none otherwise.
#[allow(clippy::too_many_arguments)]
pub fn spawn(w: &mut World, a: f32, b: f32, parent: MobyId, vel: [f32; 4], pos: [f32; 4], rot: [f32; 4], class: i16, bounces: i32) -> Option<MobyId> {
    let m = w.create_moby(class)?;
    w.svc.fx.debris += 1;
    let (light, ambient) = { let p = w.m(parent); (p.light, p.ambient) };
    let t20 = w.ticks(20);
    {
        let mo = w.mm(m);
        p::set_i32(&mut mo.pvars, 0, parent as i32 + 1);
        mo.draw_dist = 0x7e;
        mo.visible = 1;
        mo.update_dist = 0xff;
        mo.light = light;
        mo.ambient = ambient;
        mo.state = FLYING;
    }
    for c in 0..3 {
        let r = w.rng.rand_angle();
        w.mm(m).rotation[c] = r;
    }
    let s1 = random_spin(w);
    let s2 = random_spin(w);
    let s = w.rng.randf(a, b);
    let mo = w.mm(m);
    mo.position = pos;
    mo.rotation = rot;
    p::set_v4f(&mut mo.pvars, 0x10, vel);
    p::set_ff(&mut mo.pvars, 4, pos[2]);
    p::set_i16(&mut mo.pvars, 0xe, t20 as i16);
    p::set_v4f(&mut mo.pvars, 0x20, [0.0, s1, s2, 0.0]);
    mo.scale *= s;
    p::set_ff(&mut mo.pvars, 8, s * 0.25);
    p::set_i16(&mut mo.pvars, 0xc, bounces as i16);
    w.build_matrix(m);
    Some(m)
}

/// `DebrisUpdate` 0x2c5218.
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x30 { w.mm(id).pvars.resize(0x30, 0); }
    let dt2 = dt2();
    let vel = {
        let m = w.mm(id);
        let mut v = p::v4f(&m.pvars, 0x10);
        v[2] -= dt2 * GRAVITY;
        p::set_v4f(&mut m.pvars, 0x10, v);
        for (x, d) in m.position.iter_mut().zip(&v).take(3) { *x += d; }
        let spin = p::v4f(&m.pvars, 0x20);
        for (r, s) in m.rotation.iter_mut().zip(&spin).take(3) { *r = add_rot(*r, *s); }
        v
    };
    match w.m(id).state {
        FLYING => {
            let m = w.mm(id);
            if m.position[2] < p::ff(&m.pvars, 4) - FADE_DROP {
                m.state = FADING;
                return;
            }
            let mut t = p::i16(&m.pvars, 0xe);
            let expired = sv::fast_dec_timer_s16(&mut t) != 0;
            p::set_i16(&mut m.pvars, 0xe, t);
            if !expired || p::i16(&m.pvars, 0xc) == 0 { return; }
            let (pos, r) = (m.position, p::ff(&m.pvars, 8));
            let Some(h) = w.coll_sphere(sv::pv(pos), sv::pf(r), 2, None) else { return };
            let into = dot3(vel, h.normal);
            if into.is_nan() || into >= 0.0 { return; }
            let v = reflect(vel, h.normal).map(|x| x * BOUNCE_KEEP);
            let m = w.mm(id);
            if let Some(c) = h.pushed_centre { m.position = [c[0], c[1], c[2], m.position[3]]; }
            let n = p::i16(&m.pvars, 0xc) - 1;
            p::set_i16(&mut m.pvars, 0xc, n);
            p::set_v4f(&mut m.pvars, 0x10, v);
            let sy = random_spin(w);
            let sz = random_spin(w);
            p::set_v4f(&mut w.mm(id).pvars, 0x20, [0.0, sy, sz, 0.0]);
        }
        FADING => {
            let m = w.mm(id);
            m.scale *= FADE_SCALE;
            if m.alpha < FADE_ALPHA {
                w.delete_moby(id);
            } else {
                m.alpha -= FADE_ALPHA;
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------------
// Explosion flashes

/// Which of the game's two flash spawners (docs/plan/explosions.md §F): the same code with two constants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlashKind {
    /// The class created.
    pub class: i16,
    /// The head start in ticks (`ticks(head)` taken off the timer at the spawn; 0: none, the size starts at 0).
    pub head: i32,
}

/// `FlashSpawn` 0x2c20e0: class 0x70, a `ticks(4)` head start (the shells of `SpawnBeamExplosion`, the death
/// explosions, the crates, the Visibomb, the Devastator's missile, the gunship).
pub const FLASH_SHELL: FlashKind = FlashKind { class: FLASH_CLASS, head: 4 };
/// `0x309a68` (level13 copy 0x305718, same code): class 1192 (0x4a8), no head start: the Bomb Glove's explosion
/// family (`super::bomb::blast`).
pub const FLASH_BOMB: FlashKind = FlashKind { class: 1192, head: 0 };

/// `FlashSpawn(size, parent, pos, vec, T, r, g, b, a)` 0x2c20e0: a flash (class 0x70) at `pos`, tinted
/// `(r, g, b)` with alpha `a`, growing to `size` × the class scale over `T` ticks (it starts `ticks(4)` in).
/// Three draws when created (`randf(−π, π)` Euler angles), none otherwise.
#[allow(clippy::too_many_arguments)]
pub fn flash_spawn(w: &mut World, size: f32, parent: MobyId, pos: [f32; 4], vec: [f32; 4], t: i32, r: u8, g: u8, b: u8, a: u8) -> Option<MobyId> {
    flash_spawn_as(w, &FLASH_SHELL, size, parent, pos, vec, t, r, g, b, a)
}

/// The flash spawners 0x2c20e0 / 0x309a68 (one code, [`FlashKind`]): `CreateMoby(class)`, three `randf(−π, π)`,
/// state 0, alpha `a`, drawn, distances 0xff, ambient `(r, g, b)` (`0x2650d0`), +0x18 = class scale · `size`, scale 0,
/// +0x10 the parent (+1), the position (the whole quadword), +0x00 `vec`, the rotation, +0x14 = T; the timer +0x1c =
/// T − `ticks(head)` and the scale `(T − timer)·full / T` (0x2c20e0), or T and 0 (0x309a68); +0x1e = a; the matrix.
#[allow(clippy::too_many_arguments)]
pub fn flash_spawn_as(w: &mut World, kind: &FlashKind, size: f32, parent: MobyId, pos: [f32; 4], vec: [f32; 4], t: i32, r: u8, g: u8, b: u8, a: u8) -> Option<MobyId> {
    let m = w.create_moby(kind.class)?;
    w.svc.fx.flashes += 1;
    let e = [w.rng.randf(-PI, PI), w.rng.randf(-PI, PI), w.rng.randf(-PI, PI), 0.0];
    let head = if kind.head != 0 { Some(w.ticks(kind.head)) } else { None };
    let mo = w.mm(m);
    mo.state = 0;
    mo.alpha = a;
    mo.visible = 1;
    mo.update_dist = 0xff;
    mo.draw_dist = 0xff;
    mo.ambient = [r, g, b, 0];
    if mo.pvars.len() < 0x20 { mo.pvars.resize(0x20, 0); }
    let full = mo.scale * size;
    p::set_ff(&mut mo.pvars, 0x18, full);
    p::set_i32(&mut mo.pvars, 0x10, parent as i32 + 1);
    mo.position = pos;
    p::set_v4f(&mut mo.pvars, 0, vec);
    mo.rotation = e;
    p::set_i32(&mut mo.pvars, 0x14, t);
    match head {
        Some(t4) => {
            let timer = (t as u16).wrapping_sub(t4 as u16) as i16;
            p::set_i16(&mut mo.pvars, 0x1c, timer);
            mo.scale = flash_scale(t, timer, full);
        }
        None => {
            p::set_i16(&mut mo.pvars, 0x1c, t as i16);
            mo.scale = 0.0;
        }
    }
    p::set_i16(&mut mo.pvars, 0x1e, a as i16);
    w.build_matrix(m);
    Some(m)
}

/// The flash size at `timer` of `t`: `(t − timer)·full / t`.
fn flash_scale(t: i32, timer: i16, full: f32) -> f32 {
    if t == 0 { return full; }
    (t - timer as i32) as f32 * full / t as f32
}

/// `FlashUpdate` 0x2c22a8.
pub fn flash_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x20 { w.mm(id).pvars.resize(0x20, 0); }
    let step = dt() * PI;
    let m = w.mm(id);
    let t = p::i32(&m.pvars, 0x14);
    m.scale = flash_scale(t, p::i16(&m.pvars, 0x1c), p::ff(&m.pvars, 0x18));
    for k in 0..3 { m.rotation[k] = add_rot(m.rotation[k], step); }
    let mut timer = p::i16(&m.pvars, 0x1c);
    let done = sv::fast_dec_timer_s16(&mut timer) != 0;
    p::set_i16(&mut m.pvars, 0x1c, timer);
    if done {
        w.delete_moby(id);
        return;
    }
    let half = t / 2;
    if half != 0 && (timer as i32) < half {
        m.alpha = (timer as i32 * p::i16(&m.pvars, 0x1e) as i32 / half) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hero::testkit;
    use crate::hero::Hero;
    use crate::moby_runtime::{ClassInfo, Moby, MobyTable};
    use crate::moby_update::scheduler::{self, Scheduler};
    use crate::moby_update::services::{ClassTable, Services};
    use crate::rng::{Rng, LEVEL_SEED};
    use rc_formats::collision::Collision;

    const PIECE: i16 = 357;
    /// The test floor's height (the grid helpers want cells away from the origin).
    const FLOOR: f32 = 100.0;

    fn seeded() -> Rng { let mut r = Rng::new(); r.srand(LEVEL_SEED); r }

    /// Number of `rand()` calls between two states of the stream.
    fn draws(from: &Rng, to: &Rng) -> usize {
        let mut r = *from;
        for n in 0..10_000 {
            if r.state == to.state { return n; }
            r.rand();
        }
        panic!("state not reached");
    }

    fn classes() -> ClassTable {
        let mut t = ClassTable::default();
        for (slot, oc) in [(1u8, PIECE), (1, PIECE + 1), (2, FLASH_CLASS)] {
            let info = ClassInfo { slot, update_fn: scheduler::port_update_fn(oc), scale: 1.0, ..Default::default() };
            t.classes.insert(oc, (info, None));
        }
        t
    }

    /// The hero (index 0, no update) and a parent moby (index 1) with `dynamic` free slots after them.
    fn table(dynamic: usize) -> MobyTable {
        let mut h = Moby::init_instance(0, 0, Some(&ClassInfo { scale: 1.0, ..Default::default() }));
        h.mode |= crate::moby_runtime::mode::NO_UPDATE;
        let mut parent = Moby::init_instance(1, 500, Some(&ClassInfo { scale: 1.0, ..Default::default() }));
        parent.mode |= crate::moby_runtime::mode::NO_UPDATE;
        parent.light = 0x1234;
        MobyTable::new(vec![h, parent], dynamic)
    }

    struct Sim {
        table: MobyTable,
        classes: ClassTable,
        hero: Hero,
        rng: Rng,
        svc: Services,
        sched: Scheduler,
        mesh: Collision,
        counter: u64,
    }

    impl Sim {
        fn new(mesh: Collision) -> Sim {
            let mut hero = Hero::new();
            hero.pos = crate::hero::physics::v4(100.0, 100.0, 0.0);
            Sim { table: table(8), classes: classes(), hero, rng: seeded(), svc: Services::new(), sched: Scheduler::new(), mesh, counter: 0 }
        }
        fn with<R>(&mut self, f: impl FnOnce(&mut World) -> R) -> R {
            let mut w = World::new(&mut self.table, &self.hero, &mut self.rng, &self.classes, &mut self.svc, self.counter);
            w.coll = Some(&self.mesh);
            f(&mut w)
        }
        fn tick(&mut self) {
            self.table.free_slot_pass(self.counter);
            let mut w = World::new(&mut self.table, &self.hero, &mut self.rng, &self.classes, &mut self.svc, self.counter);
            w.coll = Some(&self.mesh);
            self.sched.tick(&mut w);
            self.counter += 1;
        }
    }

    #[test]
    fn registry_maps_every_debris_and_flash_class() {
        for oc in CLASSES { assert_eq!(scheduler::port_update_fn(oc), Some(UPDATE_FN), "class {oc}"); }
        for oc in FLASH_CLASSES { assert_eq!(scheduler::port_update_fn(oc), Some(FLASH_UPDATE_FN), "class {oc}"); }
        // The crate chunk classes without a table entry stay without an update.
        for oc in [351i16, 352, 360, 361, 366, 367, 369, 370] { assert_eq!(scheduler::port_update_fn(oc), None, "class {oc}"); }
    }

    /// The level01 class table (read from the level 01 overlay when `extracted/` is present) maps exactly
    /// [`CLASSES`] to 0x2c5218 and [`FLASH_CLASSES`] to 0x2c22a8.
    #[test]
    fn level_table_lists_the_shared_classes() {
        let path = rc_formats::test_data::level_dir(1).join("overlay.bin");
        let Ok(ov) = std::fs::read(path) else { eprintln!("skipped: no extracted/levels/01/overlay.bin"); return };
        let (mut d, mut f) = (Vec::new(), Vec::new());
        for e in rc_formats::level_overlay::LevelOverlay::parse(&ov).unwrap().vtbl() {
            let oc = e.o_class;
            match e.update {
                UPDATE_FN => d.push(oc as i16),
                FLASH_UPDATE_FN => f.push(oc as i16),
                _ => {}
            }
        }
        d.sort_unstable();
        f.sort_unstable();
        assert_eq!(d, CLASSES);
        assert_eq!(f, FLASH_CLASSES);
    }

    /// Integration of the rules above in f64, written independently: (z per tick while flying, bounce tick).
    fn model_fall(z0: f64, vz0: f64, floor: f64, radius: f64, bounces: i32, timer: i32) -> (Vec<f64>, Option<usize>) {
        let (dt, g) = (1.0 / 60.0, 20.0);
        let (mut z, mut vz, mut left, mut t) = (z0, vz0, bounces, timer);
        let mut zs = Vec::new();
        let mut bounce = None;
        for tick in 0..400 {
            vz -= g * dt * dt;
            z += vz;
            if z < z0 - 3.0 { break; }
            let expired = if t == 0 { true } else { t -= 1; t < 1 };
            if expired && left != 0 && z - radius < floor && vz < 0.0 {
                z = floor + radius;
                vz = -vz * 0.75;
                left -= 1;
                if bounce.is_none() { bounce = Some(tick); }
            }
            zs.push(z);
        }
        (zs, bounce)
    }

    #[test]
    fn piece_falls_bounces_fades_and_is_deleted() {
        let mut s = Sim::new(testkit::floor(FLOOR, 98, 108, 98, 108));
        let start = s.rng;
        let vel = [0.02, 0.0, 6.0 / 60.0, 0.0];
        let id = s.with(|w| spawn(w, 1.0, 1.0, 1, vel, [400.5, 400.5, FLOOR + 0.5, 0.0], [0.0; 4], PIECE, 1)).unwrap();
        assert_eq!(draws(&start, &s.rng), 6, "spawn: 3 angles, 2 spins, 1 scale");
        {
            let m = &s.table.mobys[id];
            assert_eq!((m.state, m.update_dist, m.draw_dist, m.light), (FLYING, 0xff, 0x7e, 0x1234));
            assert_eq!(p::i32(&m.pvars, 0), 2, "parent index + 1");
            assert_eq!(p::i16(&m.pvars, 0xe), 20);
            assert!((p::ff(&m.pvars, 8) - 0.25).abs() < 1e-6);
            assert_eq!(p::ff(&m.pvars, 0x20), 0.0);
        }
        let (model, model_bounce) = model_fall(f64::from(FLOOR) + 0.5, 6.0 / 60.0, f64::from(FLOOR), 0.25, 1, 20);
        let mb = model_bounce.expect("the model bounces");
        let mut zs = Vec::new();
        let mut bounce = None;
        let mut fade_at = None;
        let mut deleted_at = None;
        for tick in 0..400 {
            let before = s.rng;
            let left = p::i16(&s.table.mobys[id].pvars, 0xc);
            s.tick();
            let m = &s.table.mobys[id];
            if m.is_deleted() { deleted_at = Some(tick); break; }
            if m.state == FLYING { zs.push(m.position[2] as f64); }
            if p::i16(&m.pvars, 0xc) != left {
                bounce = Some(tick);
                assert_eq!(draws(&before, &s.rng), 2, "a bounce draws two spins");
                assert!(p::ff(&m.pvars, 0x18) > 0.0, "moving up after the bounce");
                assert_eq!(p::ff(&m.pvars, 0x20), 0.0);
            } else {
                assert_eq!(draws(&before, &s.rng), 0, "no draws without a bounce");
            }
            if m.state == FADING && fade_at.is_none() { fade_at = Some(tick); }
        }
        let b = bounce.expect("bounced on the floor");
        assert!((b as i64 - mb as i64).abs() <= 1, "bounce at tick {b}, model {mb}");
        for (k, (a, e)) in zs.iter().zip(&model).enumerate().take(b.min(mb)) {
            assert!((a - e).abs() < 1e-4, "tick {k}: z {a} vs model {e}");
        }
        assert!(b >= 19, "no collision while the spawn timer runs");
        // No bounces left: it falls through the floor, fades 3 below its spawn, and is deleted 33 ticks later.
        let f = fade_at.expect("fades");
        assert_eq!(deleted_at, Some(f + 33), "alpha 0x80 − 4 per tick");
    }

    #[test]
    fn piece_without_bounces_falls_through() {
        let mut s = Sim::new(testkit::floor(FLOOR, 98, 108, 98, 108));
        let id = s.with(|w| spawn(w, 1.0, 1.0, 1, [0.0; 4], [400.5, 400.5, FLOOR + 0.5, 0.0], [0.0; 4], PIECE, 0)).unwrap();
        let mut n = 0;
        while !s.table.mobys[id].is_deleted() {
            s.tick();
            n += 1;
            assert!(n < 400);
        }
        // Falls 3 (v = −20·dt²·k: ~33 ticks), then 33 fade updates.
        let model = (1..).find(|&k: &i32| 0.5 - 20.0 / 3600.0 * f64::from(k * (k + 1) / 2) < -2.5 /* relative to the spawn */).unwrap();
        assert_eq!(n, model + 33, "update {model} starts the fade; the 33rd after it deletes");
    }

    #[test]
    fn scale_range_and_spin_come_from_the_stream() {
        let mut s = Sim::new(Collision::default());
        let mut r = s.rng;
        let id = s.with(|w| spawn(w, 0.25, 0.5, 1, [0.0; 4], [0.0; 4], [0.1, 0.2, 0.3, 0.0], PIECE + 1, 3)).unwrap();
        for _ in 0..3 { r.rand_angle(); }
        let s1 = r.randf_sym(0.0, 360.0) * DEG * dt();
        let s2 = r.randf_sym(0.0, 360.0) * DEG * dt();
        let k = r.randf(0.25, 0.5);
        let m = &s.table.mobys[id];
        assert_eq!(r.state, s.rng.state);
        assert_eq!(m.rotation, [0.1, 0.2, 0.3, 0.0]);
        assert_eq!(p::v4f(&m.pvars, 0x20), [0.0, s1, s2, 0.0]);
        assert_eq!(m.scale, k);
        assert_eq!(p::i16(&m.pvars, 0xc), 3);
        // One update later the spin is applied.
        s.tick();
        let m = &s.table.mobys[id];
        assert!((m.rotation[1] - add_rot(0.2, s1)).abs() < 1e-7 && (m.rotation[2] - add_rot(0.3, s2)).abs() < 1e-7);
    }

    #[test]
    fn flash_grows_fades_and_is_deleted() {
        let mut s = Sim::new(Collision::default());
        let start = s.rng;
        let id = s.with(|w| flash_spawn(w, 4.0, 1, [1.0, 2.0, 3.0, 0.0], [0.0; 4], 24, 0x7f, 0x20, 0, 0x20)).unwrap();
        assert_eq!(draws(&start, &s.rng), 3);
        {
            let m = &s.table.mobys[id];
            assert_eq!((m.o_class, m.alpha, m.ambient), (FLASH_CLASS, 0x20, [0x7f, 0x20, 0, 0]));
            assert_eq!(p::i16(&m.pvars, 0x1c), 20, "starts ticks(4) in");
            assert!((m.scale - 4.0 * 4.0 / 24.0).abs() < 1e-6);
        }
        let mut last_scale = s.table.mobys[id].scale;
        let mut n = 0;
        let after = s.rng;
        loop {
            s.tick();
            n += 1;
            let m = &s.table.mobys[id];
            if m.is_deleted() { break; }
            assert!(m.scale >= last_scale, "grows");
            last_scale = m.scale;
            let timer = p::i16(&m.pvars, 0x1c) as i32;
            if timer < 12 { assert_eq!(m.alpha as i32, timer * 0x20 / 12); } else { assert_eq!(m.alpha, 0x20); }
        }
        assert_eq!(n, 20, "deleted when the timer runs out");
        assert!((last_scale - 4.0 * 22.0 / 24.0).abs() < 1e-5, "the last update sizes with timer 2");
        assert_eq!(draws(&after, &s.rng), 0, "the update draws nothing");
    }

    #[test]
    fn two_runs_are_identical() {
        let run = || {
            let mut s = Sim::new(testkit::floor(FLOOR, 98, 108, 98, 108));
            for k in 0..4 {
                let v = [0.01 * k as f32, -0.01, 0.08, 0.0];
                s.with(|w| spawn(w, 0.25, 0.5, 1, v, [400.5, 400.5, FLOOR + 0.5, 0.0], [0.0; 4], PIECE + 1, 3));
            }
            let mut out = Vec::new();
            for _ in 0..120 {
                s.tick();
                out.push((s.rng.state, s.table.mobys.iter().map(|m| (m.state, m.position.map(f32::to_bits))).collect::<Vec<_>>()));
            }
            out
        };
        assert_eq!(run(), run());
    }
}
