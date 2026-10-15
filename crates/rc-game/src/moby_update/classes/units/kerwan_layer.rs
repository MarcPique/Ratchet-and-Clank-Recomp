//! Kerwan's blob layers, class **578** (level03 `0x2c9eb8`, 5 placed, 3 created by the spawn test) and the blobs
//! **627** they drop (level03 `0x2cbea8`, created only; the name is descriptive [L]). A 578 stands on its path's first
//! point until Ratchet (or a decoy, the search out to 100) comes within 14 (28 for `ticks(240)` after an alert) and 4
//! in height, rears up (sequence 1, then 2) and then patrols its path's points in a loop, turning back when the next
//! point would lead it away from him; every 300 ticks it drops five blobs 627 around itself (class sound 1 of 627),
//! and when he is 25 away at a point it goes back to waiting. Hits knock it back (sequences 3 / 4) or kill it (the
//! death flight, sequence 5); it falls with gravity, and below z 5 it explodes (`SpawnBeamExplosion`, its bolts).
//! A blob flies off on its own ballistic arc, lands (sequence 1) and waits: Ratchet stepping on it, a creature other
//! than its layer touching it, or any hit from another class makes it **burst** (an area hit of 3 within 2, smoke in
//! four kinds, class sound 0, the camera shake, the bomb's light); its layer walking within 3 (once the blob's
//! `ticks(60)` are out) takes it back (it flies to the layer and vanishes); after `ticks(3600)` without a living layer
//! (or `ticks(5400)` in any case) it shrinks away. Read from the level03 disassembly (the decompile lost the spawns'
//! arguments and both jump tables). Native `f32`; the draws at the game's points.
//!
//! **578 pvars** (0x1e0): +0x20 the damage record (health 2.0 at +0x20), +0x38 the alert request, +0x40 the walker's
//! out vector, +0x60 the flash record (+0x67 its red), +0x70 the knock record, +0xd0 the walker (`SeedJumpPattern`),
//! +0x120 the path point, +0x124 s8 the direction, +0x134 the path, +0x150 the turn velocity, +0x154 s16 the drop
//! timer, +0x156 s16 the turn-back timer, +0x15c the alert timer, +0x160 the big-head record. **627 pvars** (0x70):
//! +0x00 the knock record, +0x60 zeroed each tick, +0x64 the timer, +0x68 the layer (index + 1), +0x6c the age.
//!
//! ## Coverage: 578 `0x2c9eb8`
//! | address | what | port |
//! |---|---|---|
//! | | `0x24e830(100, m, &t)` (= `0x274b78`, the target); `0x251d00(4.5, m, 0, +0x160)` (= `0x278720`, the big-head cheat); `0x2ca808` the hits; +0x38 ≠ 0 → 0, +0x15c = `ticks(240)`; range 14, 28 while `FastDecTimer(+0x15c)` runs | [`update_578`] (`target::acquire`, `manip::big_head`) |
//! | state 0 | `SeedJumpPattern(+0xd0)`; +0x5a = +0x58 = 8; J+0x44 2π·dt, J+0x40 6π·dt², J+0x24 5·dt, J+0x3c 2π·dt²; health 2.0, +0x24 = 2, +0x28 = 1, J+0xc = J+8 = 2.0, +0x29 = 0; position = point 0 of path +0x134; point 1; → 1; scale ·= gp−0x52e0 (1.0); sequence ≠ 0 → `MobyAnimBlend(0, 0, 0)` | [`update_578`] |
//! | state 1 | `SpringTurn2(atan(t − pos), 0.01, 0.3, 0.1, m, +0x150)`; xy distance < range and \|Δz\| < 4 → 2, sequence 1 in `ticks(10)` | [`update_578`] (`turn::spring_turn2_pvar`) |
//! | state 2 | anim wrapped (+0x70 & 2) → 3, +0x156 = `ticks(120)`, sequence 2 in `ticks(10)`, anim speed (+0x58) gp−0x52e4 (1.0), +0x154 = `ticks(120)` | [`update_578`] |
//! | state 3 | `0x247c10(m, J, point, +0x40)` (= `0x26de80`, the walk); anim speed 1.0; `FastDecTimer(+0x156)`; within 1 (3-D) of the point: timer out and Ratchet beyond 25 (xy) → 1, sequence 0; next = (point + n + dir) % n; timer out and the next point nearer to him than to it → dir = −dir, +0x156 = `ticks(120)`; point = (point + n + dir) % n | [`update_578`] (`walker::walk_to`) |
//! | | `FastDecTimer(+0x154)` out → +0x154 = `ticks(300)` (gp−0x52e8), `PlayClassSoundByClass(1, 0, m, 627)`; for i in −2..2: s = i·`randf(0.9, 1.1)`, p = pos + row 1·(i/2), z + 2; v = row 0·`randf(−1, 1)`·dt + row 1·2·dt·s, v.z = 6·dt·`randf(0.9, 1.1)` (gp−0x52f0 / −0x52ec); `0x2cc888(m, p, v)` | [`drop_blobs`], [`spawn_627`] |
//! | state 4 | `0x24b0e0` (= `0x271558`, the flight) & 0x60 → sequence 0 in `ticks(10)`, 1; else z < 5 → the death | [`update_578`] (`knock::update`) |
//! | state 5 | the flight & 0x60 or z < 5 → the death | [`update_578`] |
//! | states < 4 | J+0x18 += 9.8·dt² (gp−0x7e90); z −= J+0x18 − 2; ground = `GroundHeight(0.5, pos)`; z −= 2; below the ground → z = ground, J+0x18 = 0; else z < 5 → the death | [`update_578`] |
//! | the death | `SpawnBeamExplosion(0, 0, 2, 1, 9, 1, 15, m, +0x40, pos + (0, 0, 0.5), 5, 2, 4, −1, 1, 1, −1, 0)`, `SetDeathBits(m, 0, −1)`, `DeleteMoby` | [`die_578`] |
//! | | drawn and within 32 of the camera → `0x248ba8` (= `0x26f020`) | [`update_578`] (`shadows::probe_down`) |
//!
//! ## Coverage: 578's hits `0x2ca808`
//! | address | what | port |
//! |---|---|---|
//! | | `MobyGetHitMessage(m, 0x330000, 0)`, the resolver (`0x248f00` = `0x26f378`, record +0x20, column 4); out5 = 1 or state 5 → no reaction | [`hits`] (`damage::resolve`) |
//! | | `FastArcTan` from the attacker / Ratchet: a dead store; health −= damage, ≤ 0 → reaction 1 | [`hits`] |
//! | | 1: K gravity 60·dt², `0x250a78(6, 2.5, K)`, K+0x3d = 0, K flags 9, the aim `0x2495d0` (= `0x26fa48`) along the push (speed, up), the flight `0x24afa0` (= `0x271418`) from `atan(pos − Ratchet)`, sequence 5, keys 8 / 15, not targetable, red 250, → 5 | [`hits`], [`flight`] |
//! | | 3 / 6: gravity 70·dt², arc (2.5, 1.5), sequence 3, keys 5 / 10, → 4; 4 / 5: sequence 4, keys 7.5 / 15, → 4; then the flash `0x24bea0` (= `0x272318`) | [`hits`], [`flight`] (`flash::start`) |
//! | | +0xa4 = 0xff; the flash `0x24bf80` (= `0x2723f8`) | [`hits`] (`flash::update`) |
//!
//! ## Coverage: 627 `0x2cbea8` and `0x2cc888`
//! | address | what | port |
//! |---|---|---|
//! | `0x2cc888(m, p, v)` | `CreateMoby(627)`: draw distance = the layer's, drawn, update distance its low byte, light words (+0x38) the layer's; position p; K gravity 0.008, drag 0.0005 (·0x15ed64), speed \|v.xy\|, up v.z, radius 0x400, +0x28 1, +0x30 0.75, +0x34 0.5, +0x38 0.65, +0x48 0.2, +0x4c 0.01, +0x3d 0, flags 1; the flight from `atan(v.x, v.y)`, sequence 0; +0x68 the layer, +0x6c 0; → 1 | [`spawn_627`] |
//! | `0x2cc828` | `MobyGetHitMessage(m, 0x130000, 0)` with an attacker of another class → 4; +0xa4 = 0xff | [`update_627`] |
//! | | the light template 0x1e32b0 copied (the bomb's: `fx::LIGHT_BOMB`); +0x60 = 0; +0x6c += 1 | [`update_627`] |
//! | state 1 | the flight & 1 (landed) → 3, sequence 1 in `ticks(10)`, +0x64 = `ticks(60)`; else z < 5 → `DeleteMoby` | [`update_627`] |
//! | state 2 | to the layer: within 0.25 → `DeleteMoby`; else pos += unit(layer − pos)·14·dt + (0, 0, 4·dt) | [`update_627`] |
//! | state 3 | `FastDecTimer(+0x64)`; the layer alive, within 3 (xy) and the timer out → 2, `PlayClassSound(2, 0, m)`; age > `ticks(3600)` and (no living layer or age > `ticks(5400)`) → 5; Ratchet's moby within 0.5 (xy) and 0.25 in height → 4; else `coll_sphere_mobys(1, pos, 0x10, m)`: a targetable moby (mode & 0x1000) not the layer, or the layer with the timer out → 4 | [`update_627`] |
//! | state 4 | d = the camera's xy distance; `coll_sphere_mobys(2, pos)` and `0x249480(3, 0.25, 1.5, m, pos, list, n, 0, 0x810001, 4, 1)` (= `0x26f8f8`); `(int)(15·d)` particles: k = `randi(4)`, v = (`randf(±dt/2)`, `randf(±dt/2)`, `randf(4·dt, (14, 22, 12, 0)[k]·dt)`), r = `randf(0, 0.25)` (k 1: 0.5), a = `rand_angle`, p = pos + (cos a, sin a)·r, z −= r·cos 45°, v.z −= 8·r·dt; type 16 by k (sizes, colours, lives below; k 3 also v = `0x251130(randf(0, 1)·dt, a, rand_angle)` and colours tweened `0x248e88` (= `0x26f300`, `FastTweenColor`)) | [`burst`] (`attack::area_hit`, `projectile::part16`) |
//! | | `PlayClassSound(0, 0, m)`; the shake 0x166ee0 (= 0x167260) = 0.4 − 0.0175·d (0.05 from 20), `ticks(25)`; the light `0x2ce470` (= `0x2f3570`); `0x24e248(m, 0x1b08b0)` (the mines' list, level01's 0x1b0c30: the port's is the table); `DeleteMoby` | [`burst`] (`fx::light_spawn`) |
//! | state 5 | scale ·= 1 − 0.074·speed; below the class scale·0.01 → the list, `DeleteMoby` | [`update_627`] |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::classes::units::hero_pos;
use crate::moby_update::creature::{self as c, attack, damage, flash, fx, knock, projectile, target, turn, walker, DT, DT2};
use crate::moby_update::services::{pf, pv, World};
use crate::particles::type16;

pub const REFERENCE_LEVEL: u32 = 3;
pub const LAYER_FN: u32 = 0x2c_9eb8;
pub const BLOB_FN: u32 = 0x2c_bea8;
pub const LAYER: i16 = 578;
pub const BLOB: i16 = 627;
pub const LAYER_CLASSES: [i16; 1] = [LAYER];
pub const BLOB_CLASSES: [i16; 1] = [BLOB];

/// 578's pvars (module doc).
mod lp {
    pub const D: usize = 0x20;
    pub const LURE: usize = 0x38;
    pub const OUT: usize = 0x40;
    pub const F: usize = 0x60;
    pub const K: usize = 0x70;
    pub const J: usize = 0xd0;
    pub const J_VZ: usize = J + 0x18;
    pub const POINT: usize = 0x120;
    pub const DIR: usize = 0x124;
    pub const PATH: usize = 0x134;
    pub const TURN_V: usize = 0x150;
    pub const DROP_T: usize = 0x154;
    pub const BACK_T: usize = 0x156;
    pub const ALERT: usize = 0x15c;
    pub const HEAD: usize = 0x160;
    pub const SIZE: usize = 0x1e0;
}

/// 627's pvars (module doc).
mod bp {
    pub const K: usize = 0x00;
    pub const ZERO: usize = 0x60;
    pub const TIMER: usize = 0x64;
    pub const LAYER: usize = 0x68;
    pub const AGE: usize = 0x6c;
    pub const SIZE: usize = 0x70;
}

fn state(w: &World, id: MobyId) -> u8 { w.m(id).state }
fn set_state(w: &mut World, id: MobyId, s: u8) { w.mm(id).state = s; }
fn spline(w: &World, path: i32) -> Option<usize> { usize::try_from(path).ok().filter(|&p| p < w.svc.splines.len() && !w.svc.splines[p].is_empty()) }
fn point(w: &World, p: usize, i: i32) -> c::V { let s = &w.svc.splines[p]; s[(i.max(0) as usize).min(s.len() - 1)].map(f32::from_bits) }
fn blend(w: &mut World, id: MobyId, seq: u8, t: i32) { c::blend_to(w, id, seq, 0, t); }
fn alive(w: &World, m: MobyId) -> bool { let s = w.m(m).state; s != 0xfe && s != 0xfd }
fn link(w: &World, id: MobyId, o: usize) -> Option<MobyId> {
    usize::try_from(c::pi32(w, id, o) - 1).ok().filter(|&m| m < w.table.mobys.len())
}

/// The knockback / death flight of a hit (module table).
#[allow(clippy::too_many_arguments)]
fn flight(w: &mut World, id: MobyId, dir: c::V, gravity: f32, dist: f32, height: f32, seq: u8, keys: (f32, f32)) {
    let kr = lp::K;
    c::set_pf(w, id, kr + knock::k::GRAVITY, gravity * DT2);
    knock::ballistic(w, id, dist, height, kr);
    c::set_pu8(w, id, kr + 0x3d, 0);
    c::set_pi32(w, id, kr + knock::k::FLAGS, 9);
    let (mut sp, mut up) = (c::pf(w, id, kr + knock::k::SPEED), c::pf(w, id, kr + knock::k::UP));
    knock::aim(dir, &mut sp, &mut up);
    c::set_pf(w, id, kr + knock::k::SPEED, sp);
    c::set_pf(w, id, kr + knock::k::UP, up);
    let (p, h) = (c::pos(w, id), hero_pos(w));
    let yaw = c::atan(p[0] - h[0], p[1] - h[1]);
    knock::start(w, id, kr, yaw, seq, 1, 0);
    c::set_pf(w, id, kr + knock::k::KEY_APEX, keys.0);
    c::set_pf(w, id, kr + knock::k::KEY_LAND, keys.1);
}

/// `0x2ca808`: the hits (module table).
fn hits(w: &mut World, id: MobyId) {
    let hit = w.get_hit(id, 0x33_0000, false);
    let res = damage::resolve(w, id, hit, lp::D, 0, 4);
    if let Some(h) = res.hit.filter(|_| res.out5 != 1 && state(w, id) != 5) {
        let hp = c::pf(w, id, lp::D) - res.damage;
        c::set_pf(w, id, lp::D, hp);
        let reaction = if hp <= 0.0 { 1 } else { res.reaction };
        let dir = h.dir.map(|x| f32::from_bits(x.0));
        match reaction {
            1 => {
                flight(w, id, dir, 60.0, 6.0, 2.5, 5, (8.0, 15.0));
                w.mm(id).mode &= !mode::TARGETABLE;
                c::set_pu8(w, id, lp::F + 7, 250);
                set_state(w, id, 5);
            }
            3 | 6 => {
                flight(w, id, dir, 70.0, 2.5, 1.5, 3, (5.0, 10.0));
                set_state(w, id, 4);
            }
            4 | 5 => {
                flight(w, id, dir, 70.0, 2.5, 1.5, 4, (7.5, 15.0));
                set_state(w, id, 4);
            }
            _ => {}
        }
        flash::start(w, id, lp::F);
    }
    w.mm(id).hit_slot = 0xff;
    flash::update(w, id, lp::F);
}

/// 578's death (module table).
fn die_578(w: &mut World, id: MobyId) {
    let mut at = c::pos(w, id);
    at[2] += 0.5;
    let b = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 2.0, flash2: 1.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 5, sparks: 2, puffs: 4, debris: 1, sound: -1, shake: true };
    fx::beam_explosion(w, &b, Some(id), at);
    set_death_bits(w, id, 0, -1);
    w.delete_moby(id);
}

/// Level03 `0x2c9eb8`, the layer 578 (module table).
pub fn update_578(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < lp::SIZE { w.mm(id).pvars.resize(lp::SIZE, 0); }
    let t = target::acquire(w, id, 100.0);
    crate::moby_update::manip::big_head(w, 4.5, id, 0, id, lp::HEAD);
    hits(w, id);
    if c::pi32(w, id, lp::LURE) != 0 {
        c::set_pi32(w, id, lp::LURE, 0);
        let a = w.ticks(240);
        c::set_pi32(w, id, lp::ALERT, a);
    }
    let range = if c::dec_timer_pvar_i32(w, id, lp::ALERT) == 0 { 28.0 } else { 14.0 };
    let seq_b = w.m(id).anim.seq_b;
    match state(w, id) {
        0 => {
            walker::seed(&mut w.mm(id).pvars, lp::J);
            let j = lp::J;
            c::set_pu8(w, id, 0x5a, 8);
            c::set_pu8(w, id, 0x58, 8);
            c::set_pf(w, id, j + 0x44, DT * f32::from_bits(0x40c9_0fdb));
            c::set_pf(w, id, j + 0x40, DT2 * f32::from_bits(0x4196_cbe4));
            c::set_pf(w, id, j + 0x24, DT * 5.0);
            c::set_pf(w, id, j + 0x3c, DT2 * f32::from_bits(0x40c9_0fdb));
            c::set_pf(w, id, lp::D, 2.0);
            c::set_pi16(w, id, lp::D + 4, 2);
            c::set_pu8(w, id, lp::D + 8, 1);
            c::set_pf(w, id, j + 0xc, 2.0);
            c::set_pf(w, id, j + 8, 2.0);
            c::set_pu8(w, id, lp::D + 9, 0);
            if let Some(p) = spline(w, c::pi32(w, id, lp::PATH)) {
                let p0 = point(w, p, 0);
                w.mm(id).position = p0;
            }
            c::set_pi32(w, id, lp::POINT, 1);
            set_state(w, id, 1);
            if seq_b != 0 { w.anim_blend(id, 0, 0, 0); }
        }
        1 => {
            let p = c::pos(w, id);
            let yaw = c::atan(t.pos[0] - p[0], t.pos[1] - p[1]);
            turn::spring_turn2_pvar(w, id, yaw, f32::from_bits(0x3c23_d70a), f32::from_bits(0x3e99_999a), f32::from_bits(0x3dcc_cccd), lp::TURN_V);
            if c::dist2(p, t.pos) < range && (t.pos[2] - p[2]).abs() < 4.0 {
                set_state(w, id, 2);
                if seq_b != 1 {
                    let tk = w.ticks(10);
                    w.anim_blend(id, 1, 0, tk);
                }
            }
        }
        2 => {
            if w.m(id).anim.flags & 2 != 0 {
                set_state(w, id, 3);
                let tk = w.ticks(120);
                c::set_pi16(w, id, lp::BACK_T, tk as i16);
                if seq_b != 2 {
                    let tk = w.ticks(10);
                    w.anim_blend(id, 2, 0, tk);
                }
                w.mm(id).anim.speed = 1.0;
                let tk = w.ticks(120);
                c::set_pi16(w, id, lp::DROP_T, tk as i16);
            }
        }
        3 => patrol(w, id, &t),
        4 => {
            let r = knock::update(w, id, lp::K);
            if r & 0x60 != 0 {
                let tk = w.ticks(10);
                blend(w, id, 0, tk);
                set_state(w, id, 1);
            } else if c::pos(w, id)[2] < 5.0 {
                return die_578(w, id);
            }
        }
        5 => {
            let r = knock::update(w, id, lp::K);
            if r & 0x60 != 0 || c::pos(w, id)[2] < 5.0 { return die_578(w, id); }
        }
        _ => {}
    }
    if state(w, id) < 4 {
        let vz = c::pf(w, id, lp::J_VZ) + DT2 * f32::from_bits(0x411c_cccd);
        c::set_pf(w, id, lp::J_VZ, vz);
        w.mm(id).position[2] -= vz - 2.0;
        let p = c::pos(w, id);
        let g = w.ground_height(crate::ps2v::Pf::b(0x3f00_0000), pv(p), 0).to_f32();
        let z = w.m(id).position[2] - 2.0;
        w.mm(id).position[2] = z;
        if z < g {
            w.mm(id).position[2] = g;
            c::set_pf(w, id, lp::J_VZ, 0.0);
        } else if z < 5.0 {
            return die_578(w, id);
        }
    }
    if w.m(id).visible != 0 {
        let cam = w.camera.map(|x| f32::from_bits(x.0));
        if c::dist3(c::pos(w, id), cam) < 32.0 { crate::shadows::probe_down(w, id); }
    }
}

/// State 3: the patrol and the drops (module table).
fn patrol(w: &mut World, id: MobyId, t: &target::Target) {
    let Some(p) = spline(w, c::pi32(w, id, lp::PATH)) else { return };
    let n = w.svc.splines[p].len() as i32;
    let i = c::pi32(w, id, lp::POINT);
    let goal = point(w, p, i);
    let mut out = c::pv4(w, id, lp::OUT);
    walker::walk_to(w, id, lp::J, goal, &mut out);
    c::set_pv4(w, id, lp::OUT, out);
    w.mm(id).anim.speed = 1.0;
    c::dec_timer_pvar_s16(w, id, lp::BACK_T);
    let pos = c::pos(w, id);
    if c::dist3(pos, goal) < 1.0 {
        let out_t = c::pi16(w, id, lp::BACK_T) == 0;
        if out_t && 25.0 < c::dist2(pos, t.pos) {
            set_state(w, id, 1);
            if w.m(id).anim.seq_b != 0 {
                let tk = w.ticks(10);
                w.anim_blend(id, 0, 0, tk);
            }
        } else {
            let step = |w: &World, id: MobyId| (c::pi32(w, id, lp::POINT) + n + (c::pu8(w, id, lp::DIR) as i8) as i32) % n;
            if out_t {
                let next = point(w, p, step(w, id));
                if c::dist2(t.pos, next) < c::dist2(pos, next) {
                    let d = c::pu8(w, id, lp::DIR) as i8;
                    c::set_pu8(w, id, lp::DIR, d.wrapping_neg() as u8);
                    let tk = w.ticks(120);
                    c::set_pi16(w, id, lp::BACK_T, tk as i16);
                }
            }
            let s = step(w, id);
            c::set_pi32(w, id, lp::POINT, s);
        }
    }
    if c::dec_timer_pvar_s16(w, id, lp::DROP_T) != 0 {
        let tk = w.ticks(300);
        c::set_pi16(w, id, lp::DROP_T, tk as i16);
        drop_blobs(w, id);
    }
}

/// The five blobs of a drop (module table), the draws in the game's order.
fn drop_blobs(w: &mut World, id: MobyId) {
    w.play_sound_as(1, 0, id, BLOB);
    let (rows, pos) = { let m = w.m(id); (m.rows, m.position) };
    let r0 = [rows[0][0], rows[0][1], rows[0][2], rows[0][3]];
    let r1 = [rows[1][0], rows[1][1], rows[1][2], rows[1][3]];
    for i in -2i32..=2 {
        let s = i as f32 * w.rng.randf(0.9, 1.1);
        let mut p = c::add(c::scale(r1, i as f32 * 0.5), pos);
        p[2] += 2.0;
        let side = c::scale(r0, w.rng.randf(-1.0, 1.0) * DT);
        let mut v = c::scale(c::scale(r1, 2.0 * DT), s);
        v = c::add(v, side);
        v[2] = 6.0 * DT * w.rng.randf(0.9, 1.1);
        spawn_627(w, id, p, v);
    }
}

/// `0x2cc888(layer, p, v)`: one blob (module table).
pub fn spawn_627(w: &mut World, layer: MobyId, p: c::V, v: c::V) -> Option<MobyId> {
    let b = w.create_moby(BLOB)?;
    let (draw, light, ambient) = { let m = w.m(layer); (m.draw_dist, m.light, m.ambient) };
    {
        let m = w.mm(b);
        if m.pvars.len() < bp::SIZE { m.pvars.resize(bp::SIZE, 0); }
        m.draw_dist = draw;
        m.visible = 1;
        m.update_dist = draw as u8;
        (m.light, m.ambient) = (light, ambient);
        m.position = p;
    }
    let k = bp::K;
    c::set_pf(w, b, k + knock::k::GRAVITY, 0.008);
    c::set_pf(w, b, k + knock::k::DRAG, 0.0005);
    c::set_pf(w, b, k + knock::k::SPEED, (v[0] * v[0] + v[1] * v[1]).sqrt());
    c::set_pf(w, b, k + knock::k::UP, v[2]);
    c::set_pi32(w, b, k + knock::k::RADIUS, 0x400);
    c::set_pf(w, b, k + knock::k::ZOFF, 1.0);
    c::set_pf(w, b, k + knock::k::BOUNCE, 0.75);
    c::set_pf(w, b, k + 0x34, 0.5);
    c::set_pf(w, b, k + 0x38, f32::from_bits(0x3f26_6666));
    c::set_pf(w, b, k + knock::k::YAW_MAX, f32::from_bits(0x3e4c_cccd));
    c::set_pf(w, b, k + knock::k::AIR_SPEED, f32::from_bits(0x3c23_d70a));
    c::set_pu8(w, b, k + 0x3d, 0);
    c::set_pi32(w, b, k + knock::k::FLAGS, 1);
    let yaw = c::atan(v[0], v[1]);
    knock::start(w, b, k, yaw, 0, 1, 0);
    c::set_pi32(w, b, bp::LAYER, layer as i32 + 1);
    c::set_pi32(w, b, bp::AGE, 0);
    set_state(w, b, 1);
    Some(b)
}

/// Level03 `0x2cbea8`, the blob 627 (module table).
pub fn update_627(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < bp::SIZE { w.mm(id).pvars.resize(bp::SIZE, 0); }
    // 0x2cc828: a hit from another class bursts it.
    if let Some(h) = w.get_hit(id, 0x13_0000, false) {
        if h.attacker.is_some_and(|a| w.m(a).o_class != w.m(id).o_class) { set_state(w, id, 4); }
    }
    w.mm(id).hit_slot = 0xff;
    c::set_pi32(w, id, bp::ZERO, 0);
    let age = c::pi32(w, id, bp::AGE) + 1;
    c::set_pi32(w, id, bp::AGE, age);
    let layer = link(w, id, bp::LAYER).filter(|&l| alive(w, l));
    match state(w, id) {
        1 => {
            let r = knock::update(w, id, bp::K);
            if r & 1 != 0 {
                set_state(w, id, 3);
                if w.m(id).anim.seq_b != 1 {
                    let tk = w.ticks(10);
                    w.anim_blend(id, 1, 0, tk);
                }
                let tk = w.ticks(60);
                c::set_pi32(w, id, bp::TIMER, tk);
            } else if c::pos(w, id)[2] < 5.0 {
                w.delete_moby(id);
            }
        }
        2 => {
            let Some(l) = link(w, id, bp::LAYER) else { return w.delete_moby(id) };
            let pos = c::pos(w, id);
            let d = c::sub(c::pos(w, l), pos);
            if c::len3(d) < 0.25 { return w.delete_moby(id); }
            let mut mv = c::set_len3(d, DT * 14.0);
            mv[2] += DT * 4.0;
            w.mm(id).position = c::add(pos, mv);
        }
        3 => {
            c::dec_timer_pvar_i32(w, id, bp::TIMER);
            let pos = c::pos(w, id);
            if let Some(l) = layer {
                if c::dist2(pos, c::pos(w, l)) < 3.0 && c::pi32(w, id, bp::TIMER) == 0 {
                    set_state(w, id, 2);
                    w.play_sound(2, 0, id);
                }
            }
            if w.ticks(3600) < age && (layer.is_none() || w.ticks(5400) < age) { set_state(w, id, 5); }
            let hero = w.hero_moby.map(|h| c::pos(w, h)).unwrap_or(hero_pos(w));
            if c::dist2(hero, pos) < 0.5 && (pos[2] - hero_pos(w)[2]).abs() < 0.25 {
                set_state(w, id, 4);
                return;
            }
            let near = crate::moby_update::services::sphere_mobys_in(w.table, w.svc, w.classes, pf(1.0), pv(pos), 0x10, Some(id), None);
            let raw_layer = link(w, id, bp::LAYER);
            for m in near {
                if w.m(m).mode & mode::TARGETABLE == 0 { continue; }
                if Some(m) != raw_layer || c::pi32(w, id, bp::TIMER) == 0 {
                    set_state(w, id, 4);
                    return;
                }
            }
        }
        4 => burst(w, id),
        5 => {
            let s = w.m(id).scale * (c::SPEED * f32::from_bits(0xbd97_8d50) + 1.0);
            w.mm(id).scale = s;
            let base = w.classes.info(BLOB).map_or(1.0, |i| i.scale);
            if s < base * 0.01 { w.delete_moby(id); }
        }
        _ => {}
    }
}

/// State 4, the burst (module table), its draws in the game's order.
fn burst(w: &mut World, id: MobyId) {
    let pos = c::pos(w, id);
    let cam = w.camera.map(|x| f32::from_bits(x.0));
    let d = c::dist2(pos, cam);
    attack::area_hit(w, 2.0, pos, id, 3.0, 0.25, 1.5, None, 0x81_0001, 4, 1);
    let n = (d * 15.0) as i32;
    let tops = [14.0 * DT, 22.0 * DT, 12.0 * DT, 0.0];
    for _ in 0..n.max(0) {
        let k = w.rng.randi(4);
        let vx = w.rng.randf(DT * -0.5, DT * 0.5);
        let vy = w.rng.randf(DT * -0.5, DT * 0.5);
        let vz = w.rng.randf(DT * 4.0, tops[k as usize & 3]);
        let mut vel = [vx, vy, vz, 0.0];
        let r = w.rng.randf(0.0, if k == 1 { 0.5 } else { 0.25 });
        let a = w.rng.rand_angle();
        let cs = |x: f32| crate::hero::physics::fast_cos(crate::ps2v::Pf::f(x)).to_f32();
        let sn = |x: f32| crate::hero::physics::fast_sin(crate::ps2v::Pf::f(x)).to_f32();
        let mut p = c::add([cs(a) * r, sn(a) * r, 0.0, 0.0], pos);
        p[2] -= r * cs(f32::from_bits(0x3f49_0fd8));
        vel[2] -= r * DT * 8.0;
        let spawn = match k {
            1 => {
                let size = w.rng.randf(50000.0, 100000.0);
                let life = w.ticks(180);
                Some(type16::Spawn { size, pos: p, vel, c1: 0x3f08_1020, c2: 0x0f08_1020, life, kind: 1 })
            }
            0 => {
                let size = w.rng.randf(200000.0, 300000.0);
                let (a0, b0) = (w.ticks(180), w.ticks(240));
                let life = w.rng.rand_range(a0, b0);
                Some(type16::Spawn { size, pos: p, vel, c1: 0x1f10_1820, c2: 0x0010_1010, life, kind: 0 })
            }
            2 => {
                let c1 = if w.rng.randi(100) > 39 { 0x2f48_6078 } else { 0x5ff8_f8f8 };
                let (a0, b0) = (w.ticks(30), w.ticks(45));
                let life = w.rng.rand_range(a0, b0);
                Some(type16::Spawn { size: 150000.0, pos: p, vel, c1, c2: 0x0f00_0020, life, kind: 2 })
            }
            3 => {
                let t1 = w.rng.randf(0.25, 1.0);
                let c1 = crate::hud::tween_color(t1, 0x7f00_0000, 0x7f18_2030);
                let t2 = w.rng.randf(0.5, 1.0);
                let c2 = crate::hud::tween_color(t2, 0, 0x005f_5f5f);
                let sp = w.rng.randf(0.0, 1.0) * DT;
                let b = w.rng.rand_angle();
                vel = fx::polar(sp, a, b);
                let size = w.rng.randf(200000.0, 300000.0);
                let (a0, b0) = (w.ticks(240), w.ticks(300));
                let life = w.rng.rand_range(a0, b0);
                Some(type16::Spawn { size, pos: p, vel, c1, c2, life, kind: 3 })
            }
            _ => None,
        };
        if let Some(s) = spawn { projectile::part16(w, &s); }
    }
    w.play_sound(0, 0, id);
    let amp = if d < 20.0 { 0.4 - d * f32::from_bits(0x3c8f_5c29) } else { f32::from_bits(0x3d4c_ccd0) };
    let t = w.ticks(25);
    w.shake_camera(crate::follow_camera::ShakeRequest { axis: crate::follow_camera::ShakeAxis::Up, amp, ticks: t });
    fx::light_spawn(w, &fx::LIGHT_BOMB, pos);
    w.delete_moby(id);
}
