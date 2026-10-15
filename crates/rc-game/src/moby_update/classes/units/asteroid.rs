//! Gemlik's asteroids, classes 212 (the placed rocks, 130 instances) and 1412 (the pieces they split into): level13
//! 0x2e1638, one update for both (census unit U408), with its two private helpers, the split 0x2e1348 and the piece
//! spawn 0x2e1140. Read from the level13 decomp and disassembly. Native `f32`.
//!
//! **Pvar block** (P, bytes): +0x00 velocity (per tick), +0x10 home (xyz, w = 20: the sphere the respawn tests),
//! +0x20 the rotation row 0 at init with w = the initial scale, +0x30 spin per tick (x, y, z), +0x40 i32 "a piece"
//! (1), +0x44 i32 the despawn timer, +0x48 the moby that made the piece (the port stores `id + 1`, 0 = none: the
//! game's pointer), +0x4c s16 the split cool-down, +0x4e s16 splits left.
//!
//! Every tick starts with `d = |pos − Ratchet|` (0x13f3d0) and ends by clamping the position into [20, 1003] on each
//! axis and adding the spin to the rotation (`fast_add_rotations`).
//!
//! * **0** init: home = pos (w 20), row = rows[0] (w = scale), splits = `rand_range(2, 3)`; +0x72 = 0 when the scale
//!   is below the class scale; → **3**.
//! * **3** reset: pos = home, spin 0, then twice `a = randf(−30, 30)` (then `randf(−180, 180)`) °·dt on the axis
//!   `rand() % 3`; the rotation three `rand_angle`s; velocity 0; scale = row w; +0x72 = 100 when above the class
//!   scale; → **1**.
//! * **1** idle: only while Ratchet is in state 0x32 and the moby at 0x140940 is class 0x45 (see below), and the
//!   rock is drawn or within 40: velocity = `randf(1, 10)·dt` along the init's rotation row 0 (`FastVecNormalize`),
//!   → **2**.
//! * **2** drifting: cool-down −1, pos += velocity. Undrawn and 40 or more away: the despawn timer runs; when it
//!   ends a piece is deleted, a rock hides (→ **4**: collision off, mode |= 1, undrawn, scale = row w). Else the
//!   timer restarts (`ticks(360)` a rock, `ticks(120)` a piece), and once the cool-down is 0: collision on; on the
//!   ticks whose parity is the moby index's, `coll_sphere(pos, 6.2·f, 0, self)` (f = scale / class scale): a hit on
//!   anything but the maker (+0x48) reflects the velocity off the normal (`FUN_00221570`), moves to the pushed
//!   centre and, unless the hit moby is a 212, **splits**; otherwise (other parity or no contact) a hit message
//!   (mask 0x330000) from anything but a 212 adds its push (clamped to 20·dt) to the velocity and **splits**; +0xa4
//!   = 0xff.
//! * **4** hidden: waits until the home sphere (r 20) is out of view at the draw distance, then pos = home, collision
//!   on, drawn, mode &= ~1, → **3**.
//!
//! **Split** (0x2e1348, `f`): `PlayClassSound(0, 0)`; with the cool-down 0, `SpawnBeamExplosion(7f, 10f, 4f, 2f, 9, 1,
//! 15f, moby, pos, streaks 10f, sparks 3f, puffs 16f, no sound, no shake, debris 1, −1, 0)` (damage radius and damage
//! 0 when Ratchet is over 20 away); +0x72 = 0. No splits left: a rock hides (→ 4), a piece is deleted. Else, while
//! `i < rand_range(1, 2)` (drawn every pass): `s = randf(0.25, 0.75)·scale`, `t = rand_vec(5dt, 10dt)`, velocity
//! ·= 0.75, `t += velocity`, a piece at `pos + t + 0.5·t̂` with velocity `t`, scale `s`, splits − 1; scale −= s,
//! splits −= 1; stop after the last split or when scale / class scale ≤ 0.2. Then collision off and the cool-down
//! `ticks(30)`.
//!
//! **Piece** (0x2e1140): `CreateMoby(1412)`, update distance 0xff, drawn, the maker's draw distance, state 2,
//! +0xbc = 0, +0x72 = 0, the position, velocity, scale, maker, "a piece", cool-down `ticks(30)`, splits, collision
//! off, the spin and rotation draws of state 3, `MobyBuildMatrix`, Ratchet's light (`FUN_00272078`).
//!
//! The idle gate's second half reads the moby at 0x140940 (the ridden-vehicle record, [`crate::vehicle::Record`]): Gemlik's
//! ship 69 is class 0x45. **Not modelled**: the explosion's debris burst (`param_16`) is counted, not spawned
//! (`fx::beam_explosion`).

#![allow(clippy::needless_range_loop, clippy::manual_clamp)] // lane loops; the game's two compares keep a NaN (no clamp).

use super::{class_collision, class_scale, hero_pos, take_hero_light};
use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, fx, DT};
use crate::moby_update::services::{pf, pv, reflect, fv, World};

pub const UPDATE_FN: u32 = 0x2e_1638;
pub const REFERENCE_LEVEL: u32 = 13;
pub const ROCK: i16 = 212;
pub const PIECE: i16 = 1412;
pub const CLASSES: [i16; 2] = [ROCK, PIECE];

/// Pvar offsets (module doc).
pub mod pv_ {
    pub const VEL: usize = 0x00;
    pub const HOME: usize = 0x10;
    pub const ROW: usize = 0x20;
    pub const SPIN: usize = 0x30;
    pub const PIECE: usize = 0x40;
    pub const DESPAWN: usize = 0x44;
    pub const MAKER: usize = 0x48;
    pub const COOL: usize = 0x4c;
    pub const SPLITS: usize = 0x4e;
    pub const LEN: usize = 0x50;
}
use pv_ as o;

/// The position box (0x41a00000, 0x447ac000).
pub const LO: f32 = 20.0;
pub const HI: f32 = 1003.0;
/// Ratchet's state and the class of the moby at 0x140940 that wake the rocks.
pub const WAKE_STATE: i32 = 0x32;
pub const WAKE_CLASS: i16 = 0x45;

/// `0x140940`'s class (the ridden-vehicle record's moby).
fn wake_moby_class(w: &World) -> Option<i16> { w.svc.vehicle.moby.filter(|&v| v < w.table.mobys.len()).map(|v| w.m(v).o_class) }

/// Level13 0x2e1638 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < o::LEN { return; }
    let d = c::dist3(c::pos(w, id), hero_pos(w));
    let oc = w.m(id).o_class;
    match w.m(id).state {
        0 => {
            let m = w.mm(id);
            let mut home = m.position;
            home[3] = LO;
            let mut row = m.rows[0];
            row[3] = m.scale;
            let s = m.scale;
            c::set_pv4(w, id, o::HOME, home);
            c::set_pv4(w, id, o::ROW, row);
            let n = w.rng.rand_range(2, 3);
            c::set_pi16(w, id, o::SPLITS, n as i16);
            if s < class_scale(w, oc) { w.mm(id).b72 = 0; }
            w.mm(id).state = 3;
        }
        1 => {
            let woken = w.hero.state == WAKE_STATE && wake_moby_class(w) == Some(WAKE_CLASS);
            if woken && (w.m(id).visible != 0 || d < 40.0) {
                let v = w.rng.randf(1.0, 10.0);
                let row = c::pv4(w, id, o::ROW);
                let vel = c::set_len3([row[0], row[1], row[2], 0.0], v * DT);
                c::set_pv4(w, id, o::VEL, vel);
                w.mm(id).state = 2;
            }
        }
        2 => {
            if drift(w, id, d) { return; }
        }
        3 => {
            let home = c::pv4(w, id, o::HOME);
            w.mm(id).position = home;
            c::set_pv4(w, id, o::SPIN, [0.0; 4]);
            random_spin(w, id);
            c::set_pv4(w, id, o::VEL, [0.0; 4]);
            let s = c::pv4(w, id, o::ROW)[3];
            w.mm(id).scale = s;
            if class_scale(w, oc) < s { w.mm(id).b72 = 100; }
            w.mm(id).state = 1;
        }
        4 => {
            let home = c::pv4(w, id, o::HOME);
            let dd = w.m(id).draw_dist as f32;
            if !fx::in_view(w, dd, home, home[3]) {
                let coll = class_collision(w, oc);
                let m = w.mm(id);
                m.position = home;
                m.visible = 1;
                m.has_collision = coll;
                m.mode &= !1;
                m.state = 3;
            }
        }
        _ => {}
    }
    let spin = c::pv4(w, id, o::SPIN);
    let m = w.mm(id);
    for k in 0..3 {
        if m.position[k] < LO { m.position[k] = LO; }
        if HI < m.position[k] { m.position[k] = HI; }
        m.rotation[k] = c::add_rot(m.rotation[k], spin[k]);
    }
}

/// The spin (two draws on random axes) and the random rotation of state 3 and the piece spawn.
fn random_spin(w: &mut World, id: MobyId) {
    for (lo, hi) in [(-30.0, 30.0), (-180.0, 180.0)] {
        let a = w.rng.randf(lo, hi) * 0.017453292 * DT;
        let axis = w.rng.rand() % 3;
        if (0..3).contains(&axis) { c::set_pf(w, id, o::SPIN + 4 * axis as usize, a); }
    }
    for k in 0..3 {
        let a = w.rng.rand_angle();
        w.mm(id).rotation[k] = a;
    }
}

/// → state 4 (a rock out of splits, or undrawn too long).
fn hide(w: &mut World, id: MobyId) {
    let s = c::pv4(w, id, o::ROW)[3];
    let m = w.mm(id);
    m.state = 4;
    m.has_collision = false;
    m.mode |= 1;
    m.visible = 0;
    m.scale = s;
}

fn maker(w: &World, id: MobyId) -> Option<MobyId> { usize::try_from(c::pi32(w, id, o::MAKER) - 1).ok() }

/// State 2; true when the moby was deleted.
fn drift(w: &mut World, id: MobyId, d: f32) -> bool {
    c::dec_timer_pvar_s16(w, id, o::COOL);
    let p = c::add(c::pos(w, id), c::pv4(w, id, o::VEL));
    w.mm(id).position = p;
    let piece = c::pi32(w, id, o::PIECE);
    if w.m(id).visible == 0 && 40.0 <= d {
        if c::dec_timer_pvar_i32(w, id, o::DESPAWN) != 0 {
            if piece == 1 {
                w.delete_moby(id);
                return true;
            }
            hide(w, id);
        }
        return false;
    }
    let t = w.ticks(if piece == 0 { 0x168 } else { 0x78 });
    c::set_pi32(w, id, o::DESPAWN, t);
    if c::pi16(w, id, o::COOL) != 0 { return false; }
    let oc = w.m(id).o_class;
    let coll = class_collision(w, oc);
    w.mm(id).has_collision = coll;
    let f = w.m(id).scale / class_scale(w, oc);
    let contact = if (w.counter & 1) as usize == id & 1 { w.coll_sphere(pv(c::pos(w, id)), pf(f * 6.2), 0, Some(id)) } else { None };
    if let Some(hit) = contact {
        if hit.moby != maker(w, id) {
            let n = [hit.normal[0], hit.normal[1], hit.normal[2], 0.0];
            let vel = fv(reflect(pv(c::pv4(w, id, o::VEL)), pv(n)));
            c::set_pv4(w, id, o::VEL, vel);
            if let Some(q) = hit.pushed_centre {
                let m = w.mm(id);
                m.position = [q[0], q[1], q[2], m.position[3]];
            }
            if hit.moby.is_none_or(|h| w.m(h).o_class != ROCK) { split(w, id, f); }
        }
    } else {
        if let Some(h) = w.get_hit(id, 0x33_0000, false) {
            if h.attacker.is_none_or(|a| w.m(a).o_class != ROCK) {
                let push = c::clamp_len3(fv(h.dir), DT * 20.0);
                let vel = c::add(c::pv4(w, id, o::VEL), [push[0], push[1], push[2], 0.0]);
                c::set_pv4(w, id, o::VEL, vel);
                let f = w.m(id).scale / class_scale(w, oc);
                split(w, id, f);
            }
        }
        w.mm(id).hit_slot = 0xff;
    }
    false
}

/// Level13 0x2e1348 (module doc).
fn split(w: &mut World, id: MobyId, f: f32) {
    let d = c::dist3(c::pos(w, id), hero_pos(w));
    let (mut r, mut dmg) = (f * 7.0, f * 10.0);
    if 20.0 < d { r = 0.0; dmg = 0.0; }
    w.play_sound(0, 0, id);
    if c::pi16(w, id, o::COOL) == 0 {
        let b = fx::Beam { damage_r: r, damage: dmg, flash: f * 4.0, flash2: f + f, flash_dist: 9.0, scale: 1.0, light: f * 15.0, streaks: (f * 10.0) as i32, sparks: (f * 3.0) as i32, puffs: (f * 16.0) as i32, debris: 1, sound: -1, shake: false };
        let p = c::pos(w, id);
        fx::beam_explosion(w, &b, Some(id), p);
    }
    w.mm(id).b72 = 0;
    if c::pi16(w, id, o::SPLITS) < 1 {
        if c::pi32(w, id, o::PIECE) == 0 { hide(w, id) } else { w.delete_moby(id) }
        return;
    }
    let oc = w.m(id).o_class;
    let mut i = 0;
    while i < w.rng.rand_range(1, 2) {
        let s = w.rng.randf(0.25, 0.75) * w.m(id).scale;
        let r = w.rng.rand_vec(DT * 5.0, DT * 10.0);
        let vel = c::scale(c::pv4(w, id, o::VEL), 0.75);
        c::set_pv4(w, id, o::VEL, vel);
        let t = c::add([r[0], r[1], r[2], 0.0], vel);
        let at = c::add(c::add(c::set_len3(t, 0.5), c::pos(w, id)), t);
        let n = c::pi16(w, id, o::SPLITS);
        spawn_piece(w, s, id, at, t, n - 1);
        w.mm(id).scale -= s;
        c::set_pi16(w, id, o::SPLITS, n - 1);
        if n == 1 { break; }
        i += 1;
        if w.m(id).scale / class_scale(w, oc) <= 0.2 { break; }
    }
    w.mm(id).has_collision = false;
    let t = w.ticks(30);
    c::set_pi16(w, id, o::COOL, t as i16);
}

/// Level13 0x2e1140 (module doc).
fn spawn_piece(w: &mut World, s: f32, maker: MobyId, at: c::V, vel: c::V, splits: i16) -> Option<MobyId> {
    let n = w.create_moby(PIECE)?;
    if w.m(n).pvars.len() < o::LEN { w.mm(n).pvars.resize(0x80, 0); }
    let dd = w.m(maker).draw_dist;
    let m = w.mm(n);
    m.update_dist = 0xff;
    m.visible = 1;
    m.draw_dist = dd;
    m.state = 2;
    m.cmd = 0;
    m.b72 = 0;
    m.position = at;
    m.scale = s;
    m.has_collision = false;
    c::set_pv4(w, n, o::VEL, vel);
    c::set_pi32(w, n, o::MAKER, maker as i32 + 1);
    c::set_pi32(w, n, o::PIECE, 1);
    let t = w.ticks(30);
    c::set_pi16(w, n, o::COOL, t as i16);
    c::set_pi16(w, n, o::SPLITS, splits);
    random_spin(w, n);
    w.build_matrix(n);
    take_hero_light(w, n);
    Some(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};

    fn rock() -> Moby { Moby { o_class: ROCK, scale: 1.0, draw_dist: 200, position: [100.0, 100.0, 100.0, 1.0], pvars: vec![0; o::LEN], hit_slot: 0xff, ..Moby::default() } }

    #[test]
    fn init_reset_and_idle() {
        let mut t = MobyTable::new(vec![rock()], 8);
        let hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = crate::moby_update::Services::new();
        let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 0);
        update(&mut w, 0);
        assert_eq!(w.m(0).state, 3);
        assert_eq!(c::pv4(&w, 0, o::HOME), [100.0, 100.0, 100.0, LO]);
        assert!((2..=3).contains(&c::pi16(&w, 0, o::SPLITS)));
        update(&mut w, 0);
        assert_eq!(w.m(0).state, 1);
        let spin = c::pv4(&w, 0, o::SPIN);
        assert!(spin.iter().take(3).any(|&a| a != 0.0) && spin.iter().all(|a| a.abs() <= 180.0 * 0.017453292 * DT));
        // Idle: stays (Ratchet not in state 0x32), spins, clamps into the box.
        w.mm(0).position = [5.0, 2000.0, 100.0, 1.0];
        let r0 = w.m(0).rotation;
        update(&mut w, 0);
        assert_eq!(w.m(0).state, 1);
        assert_eq!(&w.m(0).position[..3], &[LO, HI, 100.0]);
        assert_ne!(w.m(0).rotation, r0);
    }
}
