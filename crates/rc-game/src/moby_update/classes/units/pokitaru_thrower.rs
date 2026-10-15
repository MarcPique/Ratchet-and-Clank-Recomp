//! U369 / U373 (census ids vary by run): class 1231, Pokitaru's ball throwers (level11 `0x310180`: 7 created
//! instances, the only copy), and the energy ball 1297 each makes (level11 `0x318b30`; created by code, none placed).
//! A creature of the shared layer (mode 0x20 header: damage record +0x20, flash +0x60, knockback +0x70) at twice its
//! class scale that stands (3) facing Ratchet; with a target in its area path within 24 (`0x274df8`) it winds up (4),
//! grows a ball in its hand at key 15 (`0x318d10`: 1297, 50 converging sparkles), throws it at key 33 of the next
//! sequence (5: along its facing at 30 u/s; `0x318e98`) and recovers (6 → 3). The ball flies until its timer (60
//! ticks) runs out, a line from its last position hits the world or a moby (`CollLine_Fix`), or a sphere of 0.333
//! touches a moby but its thrower; then `SpawnBeamExplosion` with its damage sphere (radius 1.5, damage 2) and the
//! ball is deleted. Health 3: a hit knocks it back (7; landing outside its area path, or out of the world: the small
//! explosion, `SetDeathBits`, deleted); the last one sends it flying (8: the explosion at the end, `SetDeathBits`).
//! A held ball is dropped (deleted) by any damaging hit.
//!
//! **Pvars 1231** (0x220): +0x20 damage record (health 3, +0x24 3, +0x28 column 2, +0x2a 6), +0x58 / +0x5a bytes 8 /
//! 8, +0x60 flash, +0x70 knockback, +0xd0 the target record (+0x110 the moby, the port stores moby + 1; +0x114 kind),
//! +0x130 / +0x134 a start path (none on the disc), +0x13c the area path, +0x140 the range (24), +0x14c the turn
//! velocity, +0x158 a timer (counted down; the knockback adds 60 ticks; read by nothing), +0x160 the held ball (moby
//! + 1), +0x1e0 the big-head cheat's.
//!
//! **Pvars 1297** (0x80, zeroed by `CreateMoby`): +0x00 the velocity, +0x10..+0x13 the glow sprites' angles, +0x14..
//! +0x17 their spins (±1), +0x18..+0x1b their sizes (10, 20, 30, 40), +0x1c the glow scale, +0x20 the grow / flight
//! timer, +0x24 the thrower, +0x28 the hold timer (5 ticks: the thrower renews it every tick).
//!
//! The level11 `$gp` words (gp = 0x166c00): gp−0x4c78 20 (the flights' gravity ×dt²), −0x4c74 / −0x4c70 4 / 4 (the
//! knockback up / out ×dt), −0x4c6c / −0x4c68 8 / 8 (the death flight's), −0x4c64 2 (the scale factor), −0x4c38 2 /
//! −0x4c34 1 (the hand: 2 ahead, 1 up); the ball's −0x4880 / −0x487c the glow colours 0x50000080 / 0x50007070, −0x4878
//! 2 (the glow scale), −0x4874.. the sparkles (0x4000a0ff / 0x800040ff, phases 5 / 20 / 1, sizes 0.1 / 0.5, def
//! 0x10008), −0x4854.. the trail (speeds 1 / 3 ×dt, sizes 0.2 / 0.1, colours 0x8000ffff..0x800020ff, phases 10 / 10 /
//! 10, def 0x10008).
//!
//! ## Coverage 1231
//!
//! **The tick** `0x310698`:
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | `FastDecTimer(+0x158)`; scale = class scale × 2 | | [`pre`] |
//! | a held ball: at pos + 2·row 0 + 1 z, its hold timer 5 (`0x318e78`) | | [`pre`] |
//! | `MobyGetHitMessage(m, 0x330000, 0)`, `0x280358(…, col 4)` | every weapon's hit | [`pre`] |
//! | a hit (out5 ≠ 1) out of 8: health −= damage (≤ 0 → 1); K radius 0x200, height 0.5, gravity 20·dt², drag 0.0005, flags 9, +0xad 0 | | [`pre`] |
//! | 1 / 2: untargetable; keys 4 / 10; 8·dt / 8·dt; aim; `0x2823f8(…, 6, 1, 0)`; 8; flash 0xf0; the ball deleted (`0x3191a0`); `SetDeathBits(m, 0, −1)` | the death flight | [`pre`] |
//! | 3–8: keys 3 / 6; 4·dt / 4·dt; aim; start (5); 7; flash 0x78; the ball deleted; +0x158 += `ticks(60)` | the knockback | [`pre`] |
//! | 9 / 10: flash 0xfa; `0x2832f8` flash start | | [`pre`] |
//! | +0xa4 = 0xff; `0x2833d8` flash update | | [`pre`] |
//! | range 24; `0x2854f8(24, m, +0xd0, 0, 0, area path)`; found farther than 24 (xy) or 3 in z → kind 2; no moby → Ratchet | the target search | [`pre`] (`target::acquire_in`) |
//!
//! **The update** `0x310180`:
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | `0x288f40(2.5, m, 0, +0x1e0)` (= `0x278720`) | the big-head cheat manipulator | [`update`] (`manip::big_head`) |
//! | drawn within 28 of the camera: `0x280000` shadow probe, +0x7f = 0x16 | | [`update`] |
//! | 0: meter 3, health 3, column 2, +0x2a 6, +0x58 8, +0x5a 8, 3 (blend 0); a start path (+0x130) → 1 at its first point (no instance) | init | [`update`] |
//! | 1 / 2 | no case (a start-path thrower stays) | n/a |
//! | 3: turn to Ratchet's moby (2π·dt² / 2π·dt, `0x281ca0`); a target → 4 (blend 2) | stand | [`update`] |
//! | 4: turn to the target; key 15 (`0x286cb0`): the ball at pos + 2·row 0 + 1 z (`0x318d10`, hold 5); else wrapped → 5 (blend 3) | wind up | [`update`] ([`make_ball`]) |
//! | 5: turn; a ball and key 33: thrown along row 0 at 30·dt (`0x318e98`); wrapped → 6 (blend 4) | throw | [`update`] ([`throw`]) |
//! | 6: wrapped → 3 (blend 0) | recover | [`update`] |
//! | 7: `0x282538(m, K)` & 0x41: inside the area path → 3 (blend 0), else as out of the world; & 0x120: `SetDeathBits(m, 0, −1)`, `0x284c28(0.5, 10, m, pos, −1)` (= `0x2742a8`), `DeleteMoby` | the knockback's end | [`update`] |
//! | 8: & 0x160 → `0x284c28(1, 10, m, pos, −1)`, `SetDeathBits`, `DeleteMoby` | the death | [`update`] |
//! | the table (level11 0x21bfd8: the default wrappers) | no Suck Cannon reaction | n/a |
//!
//! ## Coverage 1297 (`0x318b30`, the glow `0x319028`, the trail `0x3191c0`, the birth `0x318d10` / `0x318ed8`)
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | `0x318d10`: `CreateMoby(0x511)`: update distance 0xff, draw distance 0x40, drawn, scale 0, state 1, the thrower's light word / ambient and Euler (rows `0x1fa030`), pvars zeroed; +0x24 the thrower, +0x20 `ticks(5)`, +0x28 5; per sprite `randi(255)` angle, spin 1 / −1 / 1 / −1, size 10..40; 50 sparkles (`0x318ed8`: from a random point 2.5–3 off in its row 1 / row 2 plane toward it over `ticks(26)`: `PartType02Spawn`) | | [`make_ball`] |
//! | every tick `0x319028`: four `PartType59Spawn(size·scale·0.025, p, tween(i/3, 0x50000080, 0x50007070), angle, 53, soft, 2, 0)` from 0.3 toward the camera, 0.1 apart, each angle += spin | the glow | [`glow`] (`particles::type59`) |
//! | 1 (held): the timer out → the scale grows to its class scale (by a twentieth of it a tick, `ticks(20)`), the glow to 2 (`0x281708` = `Approach`); the hold timer out → `DeleteMoby` | | [`ball_update`] |
//! | 2 (thrown): the trail (`0x3191c0`: `PartType02Spawn(pos, rand_vec(dt, 3·dt) ×2, tween(randf) ×2, 10, 10, 10, def 0x10008)`); pos += v; the timer out, or `CollLine_Fix(old, pos, 0, self)`, or `coll_sphere_mobys(0.333, pos, 0, thrower, no template)` → `SpawnBeamExplosion(1.5, 2, 4, 2, 9, 1, 15, m, v, pos, 10, 3, 16, −1, 1, …)`, `DeleteMoby` | the flight | [`ball_update`] (`fx::beam_explosion`) |
//! | `0x3191a0` | `DeleteMoby(ball)` | [`drop_ball`] |
//!
//! Native `f32`; the rand draws at the game's points.

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::creature::{self as c, damage, flash, fx, ground, knock, region, target, turn};
use crate::moby_update::services::{pf, pv as v4, World};
use std::f32::consts::PI;

pub const REFERENCE_LEVEL: u32 = 11;
pub const UPDATE_FN: u32 = 0x31_0180;
pub const CLASSES: [i16; 1] = [1231];
pub const BALL_FN: u32 = 0x31_8b30;
pub const BALL_CLASSES: [i16; 1] = [1297];

/// Pvar offsets of 1231 (module doc).
pub mod pv {
    pub const D: usize = 0x20;
    pub const FLASH: usize = 0x60;
    pub const K: usize = 0x70;
    pub const TGT: usize = 0xd0;
    pub const TGT_MOBY: usize = 0x110;
    pub const TGT_KIND: usize = 0x114;
    pub const START: usize = 0x130;
    pub const START_PATH: usize = 0x134;
    pub const AREA: usize = 0x13c;
    pub const RANGE: usize = 0x140;
    pub const TURN_V: usize = 0x14c;
    pub const TIMER: usize = 0x158;
    pub const BALL: usize = 0x160;
    pub const SIZE: usize = 0x1e0;
}

/// Pvar offsets of the ball 1297.
pub mod bv {
    pub const VEL: usize = 0x00;
    pub const ANGLE: usize = 0x10;
    pub const SPIN: usize = 0x14;
    pub const SIZE: usize = 0x18;
    pub const GLOW: usize = 0x1c;
    pub const TIMER: usize = 0x20;
    pub const THROWER: usize = 0x24;
    pub const HOLD: usize = 0x28;
    pub const LEN: usize = 0x2c;
}

/// The states of 1231.
pub mod st {
    pub const INIT: u8 = 0;
    pub const PATH: u8 = 1;
    pub const STAND: u8 = 3;
    pub const WIND: u8 = 4;
    pub const THROW: u8 = 5;
    pub const RECOVER: u8 = 6;
    pub const KNOCKED: u8 = 7;
    pub const DYING: u8 = 8;
}

/// The ball's states.
pub mod bst {
    pub const HELD: u8 = 1;
    pub const FLYING: u8 = 2;
}

/// The ball's blast: `SpawnBeamExplosion(1.5, 2, 4, 2, 9, 1, 15, m, v, pos, 10, 3, 16, −1, 1, …)`.
pub const BALL_BLAST: fx::Beam = fx::Beam { damage_r: 1.5, damage: 2.0, flash: 4.0, flash2: 2.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 10, sparks: 3, puffs: 16, debris: 1, sound: -1, shake: true };

fn state(w: &World, id: MobyId) -> u8 { w.m(id).state }
fn set_state(w: &mut World, id: MobyId, s: u8) { w.mm(id).state = s; }
fn blend(w: &mut World, id: MobyId, seq: u8, n: i32) {
    let t = w.ticks(n);
    c::blend_to(w, id, seq, 0, t);
}
fn wrapped(w: &World, id: MobyId) -> bool { w.m(id).anim.flags & 2 != 0 }
fn moby_ref(w: &World, id: MobyId, o: usize) -> Option<MobyId> {
    let v = c::pi32(w, id, o);
    (v > 0).then(|| (v - 1) as MobyId).filter(|&m| m < w.table.mobys.len())
}
fn path(w: &World, id: MobyId, o: usize) -> Option<usize> { usize::try_from(c::pi32(w, id, o)).ok().filter(|&p| p < w.svc.splines.len()) }
fn turn_to(w: &mut World, id: MobyId, to: c::V) {
    let p = c::pos(w, id);
    let r = 2.0 * PI;
    turn::turn_toward_pvar(w, id, c::atan(to[0] - p[0], to[1] - p[1]), c::DT2 * r, c::DT2 * r, c::DT * r, pv::TURN_V);
}
/// The hand: pos + 2·row 0 + 1 z.
fn hand(w: &World, id: MobyId) -> c::V {
    let m = w.m(id);
    let mut p = c::add(c::set_len3(m.rows[0], 2.0), m.position);
    p[2] += 1.0;
    p
}

/// `0x3191a0(ball)`: `DeleteMoby`.
pub fn drop_ball(w: &mut World, id: MobyId) {
    if let Some(b) = moby_ref(w, id, pv::BALL) { w.delete_moby(b); }
    c::set_pi32(w, id, pv::BALL, 0);
}

/// `0x310698`: the tick (module doc).
fn pre(w: &mut World, id: MobyId) {
    c::dec_timer_pvar_i32(w, id, pv::TIMER);
    let o = w.m(id).o_class;
    w.mm(id).scale = crate::moby_update::classes::units::class_scale(w, o) * 2.0;
    if let Some(b) = moby_ref(w, id, pv::BALL) {
        let p = hand(w, id);
        c::set_pos(w, b, p);
        if w.m(b).pvars.len() >= bv::LEN { c::set_pi32(w, b, bv::HOLD, 5); }
    }
    let hit = w.get_hit(id, 0x33_0000, false);
    let res = damage::resolve(w, id, hit, pv::D, 0, 4);
    if res.out5 != 1 && state(w, id) != st::DYING {
        let hp = c::pf(w, id, pv::D) - res.damage;
        c::set_pf(w, id, pv::D, hp);
        let reaction = if hp <= 0.0 { 1 } else { res.reaction };
        let kr = pv::K;
        c::set_pi32(w, id, kr + knock::k::RADIUS, 0x200);
        c::set_pf(w, id, kr + knock::k::ZOFF, 0.5);
        c::set_pf(w, id, kr + knock::k::GRAVITY, 20.0 * c::DT2);
        c::set_pf(w, id, kr + knock::k::DRAG, f32::from_bits(0x3a03_126f));
        c::set_pi32(w, id, kr + knock::k::FLAGS, 9);
        c::set_pu8(w, id, kr + 0x3d, 0);
        let dir = res.hit.map(|h| h.dir.map(|x| f32::from_bits(x.0))).unwrap_or([0.0; 4]);
        let fly = |w: &mut World, out: f32, up: f32, keys: (f32, f32), seq: u8| {
            c::set_pf(w, id, kr + knock::k::KEY_APEX, keys.0);
            c::set_pf(w, id, kr + knock::k::KEY_LAND, keys.1);
            let (mut sp, mut u) = (out * c::DT, up * c::DT);
            c::set_pf(w, id, kr + knock::k::SPEED, sp);
            c::set_pf(w, id, kr + knock::k::UP, u);
            let a = knock::aim(dir, &mut sp, &mut u);
            c::set_pf(w, id, kr + knock::k::SPEED, sp);
            c::set_pf(w, id, kr + knock::k::UP, u);
            knock::start(w, id, kr, a, seq, 1, 0);
        };
        match reaction {
            1 | 2 => {
                w.mm(id).mode &= !mode::TARGETABLE;
                fly(w, 8.0, 8.0, (4.0, 10.0), 6);
                set_state(w, id, st::DYING);
                c::set_pu8(w, id, pv::FLASH + 7, 0xf0);
                if c::pi32(w, id, pv::BALL) != 0 { drop_ball(w, id); }
                set_death_bits(w, id, 0, -1);
            }
            3..=8 => {
                fly(w, 4.0, 4.0, (3.0, 6.0), 5);
                set_state(w, id, st::KNOCKED);
                c::set_pu8(w, id, pv::FLASH + 7, 0x78);
                if c::pi32(w, id, pv::BALL) != 0 { drop_ball(w, id); }
                let t = c::pi32(w, id, pv::TIMER) + w.ticks(0x3c);
                c::set_pi32(w, id, pv::TIMER, t);
            }
            9 | 10 => c::set_pu8(w, id, pv::FLASH + 7, 0xfa),
            _ => {}
        }
        flash::start(w, id, pv::FLASH);
    }
    w.mm(id).hit_slot = 0xff;
    flash::update(w, id, pv::FLASH);
    c::set_pf(w, id, pv::RANGE, 24.0);
    let t = target::acquire_in(w, id, 24.0, path(w, id, pv::AREA));
    c::set_pv4(w, id, pv::TGT, t.pos);
    c::set_pv4(w, id, pv::TGT + 0x10, t.rot);
    c::set_pv4(w, id, pv::TGT + 0x20, t.aim);
    c::set_pv4(w, id, pv::TGT + 0x30, t.body);
    c::set_pi32(w, id, pv::TGT_MOBY, t.moby.map_or(0, |m| m as i32 + 1));
    c::set_pi32(w, id, pv::TGT_KIND, t.kind as i32);
    if t.kind != 2 {
        let p = c::pos(w, id);
        if !(c::dist2(p, t.pos) <= 24.0 && (p[2] - t.pos[2]).abs() <= 3.0) { c::set_pi32(w, id, pv::TGT_KIND, 2); }
    }
    if c::pi32(w, id, pv::TGT_MOBY) == 0 {
        let hm = w.hero_moby;
        c::set_pi32(w, id, pv::TGT_MOBY, hm.map_or(0, |m| m as i32 + 1));
    }
}

/// `0x318d10(thrower, p, 5)`: the ball in the hand (module doc).
pub fn make_ball(w: &mut World, id: MobyId, p: c::V, hold: i32) -> Option<MobyId> {
    let b = w.create_moby(BALL_CLASSES[0])?;
    let (light, ambient, rot) = { let m = w.m(id); (m.light, m.ambient, m.rotation) };
    {
        let m = w.mm(b);
        if m.pvars.len() < 0x80 { m.pvars.resize(0x80, 0); }
        m.update_dist = 0xff;
        m.draw_dist = 0x40;
        m.visible = 1;
        m.scale = 0.0;
        m.state = bst::HELD;
        m.light = light;
        m.ambient = ambient;
        m.position = p;
        m.rotation = rot;
        for x in m.pvars.iter_mut() { *x = 0; }
    }
    c::set_pi32(w, b, bv::THROWER, id as i32 + 1);
    let t = w.ticks(hold);
    c::set_pi32(w, b, bv::TIMER, t);
    c::set_pi32(w, b, bv::HOLD, 5);
    let rows = crate::moby_update::services::euler_rows(v4(rot));
    {
        let m = w.mm(b);
        for (i, r) in rows.iter().enumerate().take(3) { m.rows[i] = r.map(|x| x.to_f32()); }
    }
    for i in 0..4usize {
        let a = w.rng.randi(0xff) as u8;
        let m = w.mm(b);
        m.pvars[bv::ANGLE + i] = a;
        m.pvars[bv::SPIN + i] = if i * (i & 1) == 0 { 1 } else { 0xff };
        m.pvars[bv::SIZE + i] = 10 * (i as u8 + 1);
    }
    for _ in 0..50 { sparkle(w, b); }
    Some(b)
}

/// `0x318ed8(ball)`: one converging sparkle (module doc).
fn sparkle(w: &mut World, b: MobyId) {
    let r = w.rng.randf(2.5, 3.0);
    let a = w.rng.rand_angle();
    let rows = w.m(b).rows;
    let v90 = c::set_len3(rows[1], r * a.sin());
    let va0 = c::add(c::set_len3(rows[2], r * a.cos()), v90);
    let from = c::sub(c::pos(w, b), va0);
    let n = w.ticks(5 + 20 + 1);
    let v1 = c::scale(va0, 1.0 / n as f32);
    let s = crate::particles::type02::Spawn { pos: from, v1: [v1[0], v1[1], v1[2], 0.1], v2: [v1[0], v1[1], v1[2], 0.5], c1: 0x4000_a0ff, c2: 0x8000_40ff, t: [5, 20, 1], def: 0x1_0008 };
    fx::part02(w, &s);
}

/// `0x319028(ball)`: the glow's four sprites (type 59) this tick.
fn glow(w: &mut World, b: MobyId) {
    let cam = w.camera.map(|x| f32::from_bits(x.0));
    let p = c::pos(w, b);
    let d = c::set_len3(c::sub(cam, p), -0.3);
    let step = c::set_len3(d, 0.1);
    let mut at = c::add(d, p);
    let g = c::pf(w, b, bv::GLOW);
    for i in 0..4usize {
        let col = crate::particles::tween_color((i as f32 * 0.333).to_bits(), 0x5000_0080, 0x5000_7070);
        let m = w.mm(b);
        let a = m.pvars[bv::ANGLE + i].wrapping_add(m.pvars[bv::SPIN + i]);
        m.pvars[bv::ANGLE + i] = a;
        let size = g * m.pvars[bv::SIZE + i] as f32 * 0.025;
        *w.svc.fx.part_spawns.entry(59).or_default() += 1;
        if let Some(sys) = w.particles.as_deref_mut() {
            if crate::particles::type59::spawn(sys, size, at, col, a, 0x35, true, 2, 0).is_none() { w.svc.fx.part_failed += 1; }
        }
        at = c::add(at, step);
    }
}

/// `0x3191c0(ball)`: one trail blob (type 2).
fn trail(w: &mut World, b: MobyId) {
    let a = w.rng.rand_vec(c::DT, 3.0 * c::DT);
    let v = w.rng.rand_vec(c::DT, 3.0 * c::DT);
    let f1 = w.rng.randf(0.0, 1.0);
    let c1 = crate::particles::tween_color(f1.to_bits(), 0x8000_ffff, 0x8000_20ff);
    let f2 = w.rng.randf(0.0, 1.0);
    let c2 = crate::particles::tween_color(f2.to_bits(), 0x8000_ffff, 0x8000_20ff);
    let p = c::pos(w, b);
    let s = crate::particles::type02::Spawn { pos: p, v1: [a[0], a[1], a[2], 0.2], v2: [v[0], v[1], v[2], 0.1], c1, c2, t: [10, 10, 10], def: 0x1_0008 };
    fx::part02(w, &s);
}

/// `0x318e98(ball, v)`: thrown: state 2, velocity `v`, the flight timer `ticks(60)`.
pub fn throw(w: &mut World, b: MobyId, v: c::V) {
    set_state(w, b, bst::FLYING);
    c::set_pv4(w, b, bv::VEL, v);
    let t = w.ticks(0x3c);
    c::set_pi32(w, b, bv::TIMER, t);
}

/// `0x318b30`: the ball (module doc).
pub fn ball_update(w: &mut World, b: MobyId) {
    if w.m(b).pvars.len() < bv::LEN { return; }
    glow(w, b);
    match state(w, b) {
        bst::HELD => {
            if c::dec_timer_pvar_i32(w, b, bv::TIMER) != 0 {
                // `0x281708(class +0x24, class +0x24 / ticks(20), &scale)`: the moby's own class scale.
                let ts = crate::moby_update::classes::units::class_scale(w, w.m(b).o_class);
                let n = w.ticks(0x14) as f32;
                let mut s = w.m(b).scale;
                turn::approach(ts, ts / n, &mut s);
                w.mm(b).scale = s;
                let n = w.ticks(0x14) as f32;
                let mut g = c::pf(w, b, bv::GLOW);
                turn::approach(2.0, 2.0 / n, &mut g);
                c::set_pf(w, b, bv::GLOW, g);
            }
            if c::dec_timer_pvar_i32(w, b, bv::HOLD) != 0 { w.delete_moby(b); }
        }
        bst::FLYING => {
            trail(w, b);
            let old = c::pos(w, b);
            let v = c::pv4(w, b, bv::VEL);
            let p = c::add(old, v);
            c::set_pos(w, b, p);
            let thrower = moby_ref(w, b, bv::THROWER);
            let done = c::dec_timer_pvar_i32(w, b, bv::TIMER) != 0
                || w.coll_line(v4(old), v4(p), 0, Some(b)).is_some()
                || w.sphere_mobys(pf(f32::from_bits(0x3eaa_7efa)), v4(p), 0, thrower, None) != 0;
            if done {
                fx::beam_explosion(w, &BALL_BLAST, Some(b), p);
                w.delete_moby(b);
            }
        }
        _ => {}
    }
}

/// `0x310180`.
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    pre(w, id);
    let tm = moby_ref(w, id, pv::TGT_MOBY);
    // 0x288f40(2.5, m, 0, +0x1e0): the big-head cheat (0x15edb7).
    crate::moby_update::manip::big_head(w, 2.5, id, 0, id, 0x1e0);
    if w.m(id).visible != 0 {
        let cam = w.camera.map(|x| f32::from_bits(x.0));
        if c::dist3(c::pos(w, id), cam) < 28.0 {
            crate::shadows::probe_down(w, id);
            w.mm(id).b7f = 0x16;
        }
    }
    let tpos = |w: &World| tm.map(|m| c::pos(w, m)).unwrap_or([0.0; 4]);
    match state(w, id) {
        st::INIT => {
            c::set_pi16(w, id, pv::D + 4, 3);
            c::set_pf(w, id, pv::D, 3.0);
            c::set_pu8(w, id, pv::D + 8, 2);
            c::set_pu8(w, id, pv::D + 0xa, 6);
            c::set_pu8(w, id, 0x58, 8);
            c::set_pu8(w, id, 0x5a, 8);
            set_state(w, id, st::STAND);
            blend(w, id, 0, 10);
            if c::pi32(w, id, pv::START) != 0 {
                set_state(w, id, st::PATH);
                let p0 = path(w, id, pv::START_PATH).and_then(|p| w.svc.splines[p].first().copied()).map(|q| q.map(f32::from_bits)).unwrap_or([0.0; 4]);
                c::set_pos(w, id, p0);
                return;
            }
            set_state(w, id, st::STAND);
            blend(w, id, 0, 10);
        }
        st::STAND => {
            let h = w.hero_moby.map(|m| c::pos(w, m)).unwrap_or([0.0; 4]);
            turn_to(w, id, h);
            if c::pi32(w, id, pv::TGT_KIND) == 2 { return; }
            set_state(w, id, st::WIND);
            blend(w, id, 2, 10);
        }
        st::WIND => {
            let t = tpos(w);
            turn_to(w, id, t);
            if !ground::passed_frame(w, id, 15.0) {
                if wrapped(w, id) {
                    set_state(w, id, st::THROW);
                    blend(w, id, 3, 10);
                }
            } else {
                let p = hand(w, id);
                let b = make_ball(w, id, p, 5);
                c::set_pi32(w, id, pv::BALL, b.map_or(0, |b| b as i32 + 1));
            }
        }
        st::THROW => {
            let t = tpos(w);
            turn_to(w, id, t);
            if let Some(b) = moby_ref(w, id, pv::BALL) {
                if ground::passed_frame(w, id, 33.0) {
                    let v = c::set_len3(w.m(id).rows[0], c::DT * 30.0);
                    throw(w, b, v);
                    c::set_pi32(w, id, pv::BALL, 0);
                }
            }
            if wrapped(w, id) {
                set_state(w, id, st::RECOVER);
                blend(w, id, 4, 10);
            }
        }
        st::RECOVER => {
            if wrapped(w, id) {
                set_state(w, id, st::STAND);
                blend(w, id, 0, 10);
            }
        }
        st::KNOCKED => {
            let mut r = knock::update(w, id, pv::K);
            if r & 0x41 != 0 {
                let p = c::pos(w, id);
                if path(w, id, pv::AREA).is_some_and(|a| region::point_in_polygon(w, a, p)) {
                    set_state(w, id, st::STAND);
                    blend(w, id, 0, 10);
                    return;
                }
                r = 0x100;
            }
            if r & 0x120 != 0 {
                set_death_bits(w, id, 0, -1);
                let p = c::pos(w, id);
                fx::piece_explosion(w, 0.5, 10.0, Some(id), p);
                w.delete_moby(id);
            }
        }
        st::DYING if knock::update(w, id, pv::K) & 0x160 != 0 => {
            let p = c::pos(w, id);
            fx::piece_explosion(w, 1.0, 10.0, Some(id), p);
            set_death_bits(w, id, 0, -1);
            w.delete_moby(id);
        }
        _ => {}
    }
}
