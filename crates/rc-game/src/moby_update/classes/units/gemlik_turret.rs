//! Gemlik's gun turrets (level 13), census U407: the turret 29 (level13 `0x2b41b8`, 9 created) with its private
//! tick `0x2b48d8`, its fire `0x2b47a0` and the rider spawner `0x2b4958`; the **rider** 36 (update `0x2b4c80`, its hit
//! `0x2b4a58`) the turret creates in its first update; the **shot** 1238 (spawner `0x3061c8`, update `0x306300`) it
//! fires. The rider and the shot are created by code, so the census (placed classes) never saw them. Read from the
//! level13 decomp and the disassembly where the decompile hides a stack argument (docs/plan/creatures.md §11).
//!
//! The turret never takes a hit itself: the rider (a creature: damage record, flash) does. A turret faces its home
//! yaw; a target within 20 (xy), within 90° of that yaw and (with a cuboid) Ratchet's body in the cuboid wakes it:
//! it tracks the target within ±+0x6c° of home and fires a shot every 2 ticks (a spread shrinking with the distance,
//! the shot's reach ramping from 8 to 20). A hit on the rider from its front half (within 95° of the home yaw) with
//! the splash flag 0x800000 does nothing; its one health gone, the rider falls (sequence 3) and bursts, and the turret
//! blows up; a turret whose rider is gone blows up a second later.
//!
//! **System or not.** All three are per-class code (one level, one caller each). They call the engine's shared
//! functions: the target search (`0x26bd00` = `0x274b78`), the turn (`0x267e48` = `0x270cc0`), `Approach`, the hit
//! resolver, the flash, `SetDeathBits`, `SpawnBeamExplosion` (level13 `0x26a498`: its decompile is `0x273310`'s word
//! for word, only the callees' addresses differ — census cluster d7574994a069, re-tagged), `PointInCuboid`, the hit
//! record writer `0x265af0` (= `World::deliver_hit`), the Blaster shot's five impact sparks (the same loop inline:
//! `blaster_shot::impact_sparks`) and the drones' slots (0x141346 / 0x141370).
//!
//! ## Coverage: the turret 29 (`0x2b41b8`, `0x2b48d8`, `0x2b47a0`, `0x2b4958`)
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | `0x2b48d8`: `0x26bd00(20, or 22 in state 3, m, +0x00)` (= `0x274b78`); no moby → Ratchet's; +0xbc > 3 → state = +0xbc | the target search; an external state | [`tick`] (`target::acquire`) |
//! | the rider (class 0x24, state < 0x80): `0x26f7a8(2.7, rider, 1, +0x70)` | the big-head cheat manipulator | [`update`] (`manip::big_head`) |
//! | case 0: scale = class scale · 1.5; the rider (`0x2b4958`); home yaw; blend 0 (10 ticks); → 1 | init | [`update`], [`spawn_rider`] |
//! | case 1: the target within 90° of home, 20 (xy), cuboid +0x50 (−1: none) holding Ratchet's body point (0x13f420) → reach 8, blend 1 (frame `rand_range(0, 3)`, `rand_range(7, 13)` ticks), 3; turn to home (12.57·dt², 12.57·dt², 12.57·dt, +0x60); the rider gone → 5 (blend 0), +0x64 = 1 | idle | [`update`] |
//! | case 3: in range (22): `Approach(20, 6·dt, reach)`; the yaw toward the target clamped to ±+0x6c° of home; `0x2b47a0`; the rider gone → 5; out of range → 1 (blend 0) | track and fire | [`update`], [`fire`] |
//! | `0x2b47a0`: every gp−0x5874 (2) ticks: joint 0 (`0x25b450` = `0x2645a8`) + (cos a, sin a)·40·dt, a = yaw + `randf(−s, s)`° with s = (20 − d)/20·5 + 2.5 (d: xy to the target); `0x3061c8(reach, point, that step, m)` | fire | [`fire`], [`spawn_shot`] |
//! | case 4: `0x263ac8(0.5, 1)` (= `rand_vec`), `SetDeathBits(m, 0, −1)`, `SpawnBeamExplosion(0, 0, 2, 1, 9, 1, 15, m, v, pos, 5, 2, 4, −1, 1, 1, −1, 0)`, deleted | the death: bolts, save bit, the blast | [`update`] (`crate_::set_death_bits`, `fx::beam_explosion`) |
//! | case 5: turn home (25.13·dt); +0x64 −= 1 → 4 | the rider gone | [`update`] |
//! | tail: the rider alive → its Euler and rows = the turret's; not a 0x24 / deleted → +0x5c = 0 | the rider rides | [`update`] |
//! | `0x2b4958`: `CreateMoby(0x24)`: pvar header (D +0x20, flash +0x60), +0x70 the turret; update / draw distance, drawn, the header mode 0x20, state 0, +0xbc 0, the turret's position, scale 36's · the turret's / 29's; D+0x10 0.9, health 1, +0x29 1, +0x2a 3, +0x24 1, column 0; `fun_0020def8` (the matrix), `0x269200` (= `0x272078`: Ratchet's light) | | [`spawn_rider`] (`units::take_hero_light`) |
//!
//! ## Coverage: the rider 36 (`0x2b4c80`, `0x2b4a58`)
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | case 0: +0x29 1, → 1, +0x58 8, +0x5a 6 | init | [`rider_update`] |
//! | case 1: the turret firing → blend 2 (frame `rand_range(0, 3)`, `rand_range(7, 12)`); else past sequence 1 or wrapped: `randi(7)` = 0 → blend 1 else 0 (`rand_range(7, 13)`), speed `randf(0.8, 1.2)`; the hit | | [`rider_update`] |
//! | `0x2b4a58` (state ≠ 2): `MobyGetHitMessage(m, 0x330000, 0)`, `0x266500(m, hit, +0x20, 0, &out5, …, col 4)`; flag 0x800000: no attacker, or one within 95° of the turret's home yaw → damage 0, out5 1 | the shield: splash from the front does nothing | [`rider_hit`] (`damage::resolve`) |
//! | reaction 1 / 2 → health 0; out5 > 1: damage < health → health −, flash 0xfa, +0x26 `ticks(60)`; else health 0, untargetable, the turret (29, not deleted) → state 4, `SetDeathBits(rider, 0, −1)`, flash 0x78, blend 3 from frame 1, → 2 | hurt, killed: the turret blows up | [`rider_hit`] |
//! | +0xa4 0xff; `0x269580` (= `0x2723f8`) | | [`rider_hit`] |
//! | case 2 wrapped: joint 0; `SpawnBeamExplosion(0, 0, 10, 7, 20, 2, 40, m, camera 0x1670c0 − pos, joint, 20, 9, 32, −1, 0, 1, −1, 0)`, deleted | the rider's burst | [`rider_update`] |
//!
//! ## Coverage: the shot 1238 (`0x3061c8`, `0x306300`)
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | `0x3061c8`: `CreateMoby(0x4d6)`; Euler (0, −atan(\|v.xy\|, v.z), atan(v)); pos; +0x10 start, +0x00 velocity; blend 1 (`ticks(5)`); mode 0x208; +0x24 1, +0x20 the shooter, +0x28 the reach, +0x23 0x10, +0x2c life `trunc(reach / \|v\| + 1)`; the matrix | spawn | [`spawn_shot`] |
//! | pos += velocity; the push = (v.xy normalised, 1) | fly | [`shot_update`] |
//! | drones alive (0x141346), not intercepted yet (+0x0c ≠ 31337.6), within 2 of Ratchet's feet: back 0.5, stop, life 5, the marker; the first drone slot (0x141370): its record for it → damage + 1, type 1 / 1, class, flags 0x10001; else the template (`0x265990` = `0x26e808`: damage 1, flags 0x10001, type 1 / 1, class) written (`0x265af0`) | a drone takes the shot | [`shot_update`] (`drone::Globals`, `World::deliver_hit`) |
//! | within the reach of the start (xy), life left: `CollLine_Fix(old, new, flags +0x24 = 0, shooter, template)`; a hit: at the point, drawn → five type-27 sparks off the face, a hit on Ratchet (class 0) → class sound 0; deleted; no hit: outside [2, 1021]³ → deleted | the hit on Ratchet (his moby's record) | [`shot_update`] (`services::line_hit_in`, `blaster_shot::impact_sparks`) |
//! | out of reach or life: drawn → three type-51 puffs (`0x27d8d0`: a random direction plus 0.1 of it, speed `randf(2·dt, 4·dt)`); deleted | the fizzle | [`shot_update`] (`particles::type51::spawn`) |
//! | `0x3060b8` / `0x306148` | two `SpawnBeamExplosion` wrappers nothing on level 13 calls | n/a (no caller) |
//! | reaction tables (level13 0x1f615c, the shared no-op table) | no Suck Cannon reaction | the Suck Cannon's shot is a weapon hit |
//!
//! Native `f32`; the rand draws at the game's points. [L]: the fire's step vector's w lane (stack, never written;
//! 0 here) becomes the shot velocity's w (read by the drone test's marker compare only).

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::classes::units::{class_scale, hero_pos, take_hero_light};
use crate::moby_update::creature::{self as c, damage, flash, fx, target, turn};
use crate::moby_update::services::{HitTemplate, World};
use crate::ps2v::Pf;

pub const UPDATE_FN: u32 = 0x2b_41b8;
pub const RIDER_FN: u32 = 0x2b_4c80;
pub const SHOT_FN: u32 = 0x30_6300;
pub const REFERENCE_LEVEL: u32 = 13;
pub const CLASSES: [i16; 1] = [29];
pub const RIDER_CLASSES: [i16; 1] = [0x24];
pub const SHOT_CLASSES: [i16; 1] = [0x4d6];

/// The turret's pvars.
pub mod pv {
    pub const TGT: usize = 0x00;
    pub const TGT_MOBY: usize = 0x40;
    pub const TGT_KIND: usize = 0x44;
    pub const CUBOID: usize = 0x50;
    pub const HOME_YAW: usize = 0x54;
    pub const RIDER: usize = 0x5c;
    pub const TURN_V: usize = 0x60;
    pub const COUNTDOWN: usize = 0x64;
    pub const REACH: usize = 0x68;
    pub const CONE: usize = 0x6c;
    pub const SIZE: usize = 0x80;
}

/// The rider's pvars (a zeroed 0x80 block from `CreateMoby`).
pub mod rv {
    pub const D: usize = 0x20;
    pub const COOLDOWN: usize = 0x26;
    pub const FLASH: usize = 0x60;
    pub const TURRET: usize = 0x70;
}

/// The shot's pvars.
pub mod sv {
    pub const VEL: usize = 0x00;
    pub const START: usize = 0x10;
    pub const SHOOTER: usize = 0x20;
    pub const FLAGS0: usize = 0x24;
    pub const REACH: usize = 0x28;
    pub const LIFE: usize = 0x2c;
}

pub mod st {
    pub const INIT: u8 = 0;
    pub const IDLE: u8 = 1;
    pub const FIRE: u8 = 3;
    pub const DEAD: u8 = 4;
    pub const ORPHAN: u8 = 5;
}

/// Level13 `$gp` (0x166c00) −0x5874 (0x16138c) 2: the ticks between shots; −0x5878 (0x161388) 40: the muzzle step.
const FIRE_EVERY: u64 = 2;
const MUZZLE: f32 = 40.0;
/// The shot's marker once a drone took it (0x46f4d334).
const INTERCEPTED: f32 = 31_337.602;

/// Case 4's blast: `SpawnBeamExplosion(0, 0, 2, 1, 9, 1, 15, …, 5, 2, 4, −1, 1, 1, −1, 0)`.
pub const TURRET_BLAST: fx::Beam = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 2.0, flash2: 1.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 5, sparks: 2, puffs: 4, debris: 1, sound: -1, shake: true };
/// The rider's burst: `SpawnBeamExplosion(0, 0, 10, 7, 20, 2, 40, …, 20, 9, 32, −1, 0, 1, −1, 0)`.
pub const RIDER_BLAST: fx::Beam = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 10.0, flash2: 7.0, flash_dist: 20.0, scale: 2.0, light: 40.0, streaks: 20, sparks: 9, puffs: 32, debris: 1, sound: -1, shake: false };

const DT: f32 = c::DT;
const DT2: f32 = c::DT2;
const DEG: f32 = 0.017_453_292;
const TURN: f32 = 12.566_371;

fn state(w: &World, id: MobyId) -> u8 { w.m(id).state }
fn set_state(w: &mut World, id: MobyId, s: u8) { w.mm(id).state = s; }
fn seq(w: &World, id: MobyId) -> u8 { w.m(id).anim.seq_b }
fn wrapped(w: &World, id: MobyId) -> bool { w.m(id).anim.flags & 2 != 0 }
fn moby_ref(w: &World, id: MobyId, o: usize) -> Option<MobyId> { usize::try_from(c::pi32(w, id, o) - 1).ok().filter(|&m| m < w.table.mobys.len()) }
fn set_ref(w: &mut World, id: MobyId, o: usize, m: Option<MobyId>) { c::set_pi32(w, id, o, m.map_or(0, |m| m as i32 + 1)) }
fn deleted(w: &World, m: MobyId) -> bool { matches!(w.m(m).state, 0xfe | 0xfd) }

/// The turret's rider when it is still a live 0x24.
fn rider(w: &World, id: MobyId) -> Option<MobyId> { moby_ref(w, id, pv::RIDER).filter(|&r| w.m(r).o_class == RIDER_CLASSES[0] && !deleted(w, r)) }

/// `0x2b48d8` (module doc).
fn tick(w: &mut World, id: MobyId) {
    let range = if state(w, id) == st::FIRE { 22.0 } else { 20.0 };
    let t = target::acquire(w, id, range);
    c::set_pv4(w, id, pv::TGT, t.pos);
    c::set_pv4(w, id, pv::TGT + 0x10, t.rot);
    c::set_pv4(w, id, pv::TGT + 0x20, t.aim);
    c::set_pv4(w, id, pv::TGT + 0x30, t.body);
    set_ref(w, id, pv::TGT_MOBY, t.moby);
    c::set_pi32(w, id, pv::TGT_KIND, t.kind as i32);
    if c::pi32(w, id, pv::TGT_MOBY) == 0 {
        let h = w.hero_moby;
        set_ref(w, id, pv::TGT_MOBY, h);
    }
    let cmd = w.m(id).cmd;
    if 3 < cmd { set_state(w, id, cmd); }
}

/// The target moby's position.
fn tgt_pos(w: &World, id: MobyId) -> c::V { moby_ref(w, id, pv::TGT_MOBY).map_or_else(|| hero_pos(w), |m| w.m(m).position) }

/// Turn the yaw toward `h` (12.57·dt², 12.57·dt², `vmax`).
fn turn_to(w: &mut World, id: MobyId, h: f32, vmax: f32) { turn::turn_toward_pvar(w, id, h, DT2 * TURN, DT2 * TURN, vmax, pv::TURN_V); }

/// The target is in front (90°), within `range` (xy) and, with a cuboid, Ratchet's body point is in it.
fn engaged(w: &World, id: MobyId, range: f32, inclusive: bool) -> bool {
    let p = c::pos(w, id);
    let t = tgt_pos(w, id);
    let h = c::atan(t[0] - p[0], t[1] - p[1]);
    if std::f32::consts::FRAC_PI_2 <= c::diff_rots(h, c::pf(w, id, pv::HOME_YAW)) { return false; }
    let d = c::dist2(p, t);
    if if inclusive { range < d } else { range <= d } { return false; }
    let cub = c::pi32(w, id, pv::CUBOID);
    cub == -1 || w.in_cuboid(w.hero_body_point(), cub)
}

/// The rider gone → 5 (blend 0 over `rand_range(7, 13)`), countdown 1.
fn orphan(w: &mut World, id: MobyId) {
    if seq(w, id) != 0 {
        let t = w.rng.rand_range(7, 0xd);
        w.anim_blend(id, 0, 0, t);
    }
    set_state(w, id, st::ORPHAN);
    c::set_pf(w, id, pv::COUNTDOWN, 1.0);
}

/// `0x2b4958`: the rider (module doc).
pub fn spawn_rider(w: &mut World, id: MobyId) -> Option<MobyId> {
    let r = w.create_moby(RIDER_CLASSES[0])?;
    let (upd, draw, pos, scale, oc) = { let m = w.m(id); (m.update_dist, m.draw_dist, m.position, m.scale, m.o_class) };
    let s36 = class_scale(w, RIDER_CLASSES[0]);
    let s29 = class_scale(w, oc);
    {
        let m = w.mm(r);
        if m.pvars.len() < 0x80 { m.pvars.resize(0x80, 0); }
        m.update_dist = upd;
        m.visible = 1;
        m.draw_dist = draw;
        m.mode |= c::PVAR_HEADER;
        m.state = 0;
        m.cmd = 0;
        m.position = pos;
        m.scale = s36 * scale / s29;
    }
    set_ref(w, r, rv::TURRET, Some(id));
    c::set_pi32(w, r, 0x00, rv::D as i32);
    c::set_pi32(w, r, 0x0c, rv::FLASH as i32);
    c::set_pf(w, r, rv::D + 0x10, f32::from_bits(0x3f66_6666));
    c::set_pf(w, r, rv::D, 1.0);
    c::set_pu8(w, r, rv::D + 9, 1);
    c::set_pu8(w, r, rv::D + 0xa, 3);
    c::set_pi16(w, r, rv::D + 4, 1);
    c::set_pu8(w, r, rv::D + 8, 0);
    w.build_matrix(r);
    take_hero_light(w, r);
    Some(r)
}

/// `0x2b47a0` (module doc).
fn fire(w: &mut World, id: MobyId) {
    if !w.counter.is_multiple_of(FIRE_EVERY) { return; }
    let mut j = w.joint_point(id, 0);
    let d = c::dist2(c::pos(w, id), tgt_pos(w, id));
    let s = ((20.0 - d) / 20.0) * 5.0 + 2.5;
    let off = w.rng.randf(-s, s);
    let a = c::add_rot(off * DEG, c::yaw(w, id));
    let (cs, sn) = c::cs(a);
    let v = [cs * MUZZLE * DT, sn * MUZZLE * DT, 0.0, 0.0];
    j = c::add(j, v);
    let reach = c::pf(w, id, pv::REACH);
    spawn_shot(w, reach, j, v, id);
}

/// `0x3061c8(reach, pos, v, shooter)` (module doc).
pub fn spawn_shot(w: &mut World, reach: f32, pos: c::V, v: c::V, shooter: MobyId) -> Option<MobyId> {
    let s = w.create_moby(SHOT_CLASSES[0])?;
    {
        let m = w.mm(s);
        if m.pvars.len() < 0x30 { m.pvars.resize(0x80, 0); }
        m.rotation[0] = 0.0;
        m.draw_dist = 0x7f;
        m.update_dist = 0xff;
        m.visible = 1;
        m.rotation[1] = -c::atan(c::len2(v), v[2]);
        m.rotation[2] = c::atan(v[0], v[1]);
        m.position = pos;
    }
    c::set_pv4(w, s, sv::START, pos);
    c::set_pv4(w, s, sv::VEL, v);
    let t = w.ticks(5);
    w.anim_blend(s, 1, 0, t);
    {
        let m = w.mm(s);
        m.mode = 0x208;
        m.alpha = 0x10;
    }
    c::set_pi32(w, s, sv::FLAGS0, 1);
    set_ref(w, s, sv::SHOOTER, Some(shooter));
    c::set_pf(w, s, sv::REACH, reach);
    let life = (reach / c::len3(v) + 1.0) as i32;
    c::set_pi32(w, s, sv::LIFE, life);
    w.build_matrix(s);
    Some(s)
}

/// Level13 `0x2b41b8`: the turret 29 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    tick(w, id);
    // 0x26f7a8(2.7, rider, 1, +0x70) on a rider of class 0x24 whose state is below 0x80: the big-head cheat (0x15edb7).
    if let Some(r) = rider(w, id).filter(|&r| w.m(r).state < 0x80) { crate::moby_update::manip::big_head(w, 2.7, r, 1, id, 0x70); }
    let t = tgt_pos(w, id);
    let s = state(w, id);
    let mut next = None;
    match s {
        st::INIT => {
            let o = w.m(id).o_class;
            w.mm(id).scale = class_scale(w, o) * 1.5;
            let r = spawn_rider(w, id);
            set_ref(w, id, pv::RIDER, r);
            let y = c::yaw(w, id);
            c::set_pf(w, id, pv::HOME_YAW, y);
            if seq(w, id) != 0 { w.anim_blend(id, 0, 0, 10); }
            next = Some(st::IDLE);
        }
        st::IDLE => {
            if engaged(w, id, 20.0, false) {
                c::set_pf(w, id, pv::REACH, 8.0);
                if seq(w, id) != 1 {
                    let f = w.rng.rand_range(0, 3);
                    let n = w.rng.rand_range(7, 0xd);
                    w.anim_blend(id, 1, f, n);
                }
                set_state(w, id, st::FIRE);
            }
            let hy = c::pf(w, id, pv::HOME_YAW);
            turn_to(w, id, hy, DT * TURN);
            if rider(w, id).is_none() || w.m(rider(w, id).unwrap_or(id)).state == 2 {
                orphan(w, id);
            }
        }
        st::FIRE => {
            if engaged(w, id, 22.0, true) {
                let mut r = c::pf(w, id, pv::REACH);
                turn::approach(20.0, DT * 6.0, &mut r);
                c::set_pf(w, id, pv::REACH, r);
                let p = c::pos(w, id);
                let hy = c::pf(w, id, pv::HOME_YAW);
                let d = c::sub_rot(c::atan(t[0] - p[0], t[1] - p[1]), hy);
                let lim = c::pf(w, id, pv::CONE) * DEG;
                let d = if d <= lim { if d < -lim { -lim } else { d } } else { lim };
                turn_to(w, id, c::add_rot(d, hy), DT * TURN);
                fire(w, id);
                if rider(w, id).is_none() || w.m(rider(w, id).unwrap_or(id)).state == 2 { orphan(w, id); }
            } else {
                if seq(w, id) != 0 {
                    let n = w.rng.rand_range(7, 0xd);
                    w.anim_blend(id, 0, 0, n);
                }
                next = Some(st::IDLE);
            }
        }
        st::DEAD => {
            let v = w.rng.rand_vec(0.5, 1.0);
            let _ = v;
            set_death_bits(w, id, 0, -1);
            let p = c::pos(w, id);
            fx::beam_explosion(w, &TURRET_BLAST, Some(id), p);
            w.delete_moby(id);
            return;
        }
        st::ORPHAN => {
            let hy = c::pf(w, id, pv::HOME_YAW);
            turn_to(w, id, hy, DT * 25.132_742);
            let n = c::pf(w, id, pv::COUNTDOWN) - 1.0;
            c::set_pf(w, id, pv::COUNTDOWN, n);
            if n <= 0.0 { next = Some(st::DEAD); }
        }
        _ => {}
    }
    if let Some(n) = next { set_state(w, id, n); }
    match moby_ref(w, id, pv::RIDER) {
        Some(r) if w.m(r).o_class == RIDER_CLASSES[0] && !deleted(w, r) => {
            let (rot, rows) = { let m = w.m(id); (m.rotation, m.rows) };
            let m = w.mm(r);
            m.rotation = rot;
            m.rows[0] = rows[0];
            m.rows[1] = rows[1];
            m.rows[2] = rows[2];
        }
        _ => c::set_pi32(w, id, pv::RIDER, 0),
    }
}

/// `0x2b4a58(m, pvars)` (module doc).
fn rider_hit(w: &mut World, id: MobyId) {
    if state(w, id) != 2 {
        let hit = w.get_hit(id, 0x33_0000, false);
        let res = damage::resolve(w, id, hit, rv::D, 0, 4);
        let mut out5 = res.out5;
        let mut dmg = res.hit.map_or(0.0, |h| h.damage.to_f32());
        if let Some(h) = res.hit.filter(|h| h.flags & 0x80_0000 != 0) {
            let front = match h.attacker {
                None => true,
                Some(a) => {
                    let (ap, p) = (w.m(a).position, c::pos(w, id));
                    let hy = moby_ref(w, id, rv::TURRET).map_or(0.0, |t| c::pf(w, t, pv::HOME_YAW));
                    c::diff_rots(c::atan(ap[0] - p[0], ap[1] - p[1]), hy) < 1.658_062_8
                }
            };
            if front {
                dmg = 0.0;
                out5 = 1;
            }
        }
        if matches!(res.reaction, 1 | 2) { c::set_pf(w, id, rv::D, 0.0); }
        if 1 < out5 {
            let hp = c::pf(w, id, rv::D);
            if dmg < hp {
                c::set_pf(w, id, rv::D, hp - dmg);
                c::set_pu8(w, id, rv::FLASH + 7, 0xfa);
                let t = w.ticks(0x3c);
                c::set_pi16(w, id, rv::COOLDOWN, t as i16);
                flash::start(w, id, rv::FLASH);
            } else {
                c::set_pf(w, id, rv::D, 0.0);
                w.mm(id).mode &= !mode::TARGETABLE;
                if let Some(t) = moby_ref(w, id, rv::TURRET).filter(|&t| w.m(t).o_class == CLASSES[0] && !deleted(w, t)) {
                    w.mm(t).state = st::DEAD;
                }
                set_death_bits(w, id, 0, -1);
                c::set_pu8(w, id, rv::FLASH + 7, 0x78);
                flash::start(w, id, rv::FLASH);
                if seq(w, id) != 3 { w.anim_blend(id, 3, 1, 0); }
                set_state(w, id, 2);
            }
        }
    }
    w.mm(id).hit_slot = 0xff;
    flash::update(w, id, rv::FLASH);
}

/// Level13 `0x2b4c80`: the rider 36 (module doc).
pub fn rider_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x80 { return; }
    match state(w, id) {
        0 => {
            c::set_pu8(w, id, rv::D + 9, 1);
            set_state(w, id, 1);
            c::set_pu8(w, id, 0x58, 8);
            c::set_pu8(w, id, 0x5a, 6);
        }
        1 => {
            let firing = moby_ref(w, id, rv::TURRET).is_some_and(|t| w.m(t).state == st::FIRE);
            if firing {
                if seq(w, id) != 2 {
                    let f = w.rng.rand_range(0, 3);
                    let n = w.rng.rand_range(7, 0xc);
                    w.anim_blend(id, 2, f, n);
                }
            } else if 1 < seq(w, id) || wrapped(w, id) {
                if w.rng.randi(7) == 0 {
                    if seq(w, id) != 1 {
                        let n = w.rng.rand_range(7, 0xd);
                        w.anim_blend(id, 1, 0, n);
                    }
                } else if seq(w, id) != 0 {
                    let n = w.rng.rand_range(7, 0xd);
                    w.anim_blend(id, 0, 0, n);
                }
                let sp = w.rng.randf(0.8, 1.2);
                w.mm(id).anim.speed = sp;
            }
            rider_hit(w, id);
        }
        2 if wrapped(w, id) => {
            let j = w.joint_point(id, 0);
            fx::beam_explosion(w, &RIDER_BLAST, Some(id), j);
            w.delete_moby(id);
        }
        _ => {}
    }
}

/// The shot's hit template (`0x265990(1, …, shot, 0x10001, &dir)`, type 1 / 1, its class).
fn shot_template(w: &World, id: MobyId, dir: c::V) -> HitTemplate {
    HitTemplate { dir: dir.map(Pf::f), attacker: Some(id), flags: 0x1_0001, b18: 1, b19: 1, h1a: w.m(id).o_class as u16, damage: Pf::ONE, w20: 1 }
}

/// Level13 `0x306300`: the shot 1238 (module doc).
pub fn shot_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x30 { return; }
    let old = c::pos(w, id);
    let v = c::pv4(w, id, sv::VEL);
    c::set_pos(w, id, c::add(old, v));
    let mut dir = c::set_len2([v[0], v[1], 0.0, v[3]], 1.0);
    dir[2] = 1.0;
    let drones = &w.svc.drones;
    if drones.count != 0 && v[3] != INTERCEPTED && c::dist3(c::pos(w, id), hero_pos(w)) < 2.0 {
        let first = drones.slots.iter().flatten().copied().next();
        let back = c::set_len3(v, 0.5);
        let p = c::sub(c::pos(w, id), back);
        c::set_pos(w, id, p);
        c::set_pv4(w, id, sv::VEL, [0.0, 0.0, 0.0, INTERCEPTED]);
        c::set_pi32(w, id, sv::LIFE, 5);
        if let Some(d) = first {
            let tmpl = shot_template(w, id, dir);
            let slot = w.m(d).hit_slot as usize;
            match w.svc.hits.records.get_mut(slot).filter(|r| slot != 0xff && r.target == d) {
                Some(r) => {
                    r.damage = Pf::f(r.damage.to_f32() + 1.0);
                    r.b28 = 1;
                    r.b29 = 1;
                    r.h2a = tmpl.h1a;
                    r.flags = 0x1_0001;
                }
                None => w.deliver_hit(d, &tmpl),
            }
        }
    }
    let tmpl = shot_template(w, id, dir);
    let p = c::pos(w, id);
    let start = c::pv4(w, id, sv::START);
    if c::dist2(p, start) <= c::pf(w, id, sv::REACH) && c::dec_timer_pvar_i32(w, id, sv::LIFE) == 0 {
        let flags = (c::pi32(w, id, sv::FLAGS0) == 0) as u32;
        let shooter = moby_ref(w, id, sv::SHOOTER);
        match crate::moby_update::services::line_hit_in(w.table, w.svc, w.classes, w.coll, old.map(Pf::f), p.map(Pf::f), flags, shooter, &tmpl) {
            None => {
                if !crate::moby_update::creature::projectile::in_world(p) { w.delete_moby(id); }
                return;
            }
            Some(h) => {
                let hp = [h.point[0], h.point[1], h.point[2], p[3]];
                c::set_pos(w, id, hp);
                if w.m(id).visible != 0 {
                    let vel = c::pv4(w, id, sv::VEL);
                    crate::moby_update::classes::blaster_shot::impact_sparks(w, h.point, [vel[0], vel[1], vel[2]], h.normal);
                }
                if h.moby.is_some_and(|m| w.m(m).o_class == 0) { w.play_sound(0, 0, id); }
                w.delete_moby(id);
                return;
            }
        }
    }
    if w.m(id).visible != 0 {
        for _ in 0..3 {
            let x = w.rng.randf(-1.0, 1.0);
            let y = w.rng.randf(-1.0, 1.0);
            let z = w.rng.randf(-1.0, 1.0);
            let r = [x, y, z, 0.0];
            let a = c::add(c::set_len3(r, c::len3(r) * 0.1), r);
            let s = w.rng.randf(DT + DT, DT * 4.0);
            let v = c::set_len3(a, s);
            let p = c::pos(w, id);
            *w.svc.fx.part_spawns.entry(crate::particles::type51::TYPE).or_default() += 1;
            if let Some(sys) = w.particles.as_deref_mut() {
                if crate::particles::type51::spawn(sys, p, v).is_none() { w.svc.fx.part_failed += 1; }
            }
        }
    }
    w.delete_moby(id);
}
