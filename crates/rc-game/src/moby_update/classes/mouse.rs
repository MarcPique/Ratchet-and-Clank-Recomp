//! The Sonic Summoner's helper: the "house" (class 604, `CommandedNpcUpdate` level01 0x2f2b68), the "mouse" (class
//! 1818, `MouseCritterUpdate` 0x30df40, with its helpers 0x30d208 / 0x30d260 / 0x30d5f0 / 0x30d880 / 0x30dda0 /
//! 0x30ded0 and the target search 0x30d308) and its shot (class 1633, 0x30c9a8, created by 0x30c898). The same code
//! on levels 1–6, 8, 11, 12, 14 (one house and one mouse each; the object's own debug strings call them "mouse" and
//! "start house"). Read from the level01 decomp and, for the shot, the disassembly (Ghidra, read-only). Every `$gp`
//! constant the functions load is the same on the ten levels (checked per level copy). Native `f32`.
//!
//! **What it is.** With the Sonic Summoner (head item 5, `0x1404a8`; owned `0x13d4c5`) worn, the house turns to
//! face Ratchet and opens when he stands in the mouse's cuboid; with a clear line from the house the mouse is
//! summoned: Ratchet is held (state 0x1f) while it runs out of the house to him, then it flies beside him (1.1 to his
//! left, 2 up), turning to and shooting the enemies near it (up to 100 shots, for 90 s), and when the time or the
//! shots run out, or the Summoner comes off, it vanishes in a burst of sparks and reappears at home.
//!
//! **House (604)**, pvar P[0] the yaw it turns to (written by the mouse every tick), P[1] its turn velocity;
//! `moby+0xbc` the mouse's command. With the Summoner owned and worn the house `SpringTurn`s its yaw to P[0]
//! (2π·dt², 330°·dt², no cap). States: **0** anim 0, P[0] = yaw → **1** closed: command 3..5 → anim 1 (blend 10)
//! → **2** when the anim loops, anim 2 (blend 3) → **3** open: command 4 / 5 → anim 3 → **4** → anim 4 → **5**
//! (command 3 / 6 / 7 / 1) → anim 5 → **6** → anim 2 → 3; from 3 / 6 a command 7 or 1 closes it: anim 6 (blend 3)
//! → **7** → anim 0 → 1.
//!
//! **Mouse (1818)**, pvar (bytes): +0x00 the move target, +0x10 the velocity, +0x20 / +0x30 the spawn point (home),
//! +0x40 the summon cuboid, +0x48 the house's moby index (−1: none), +0x50 s16 stuck ticks, +0x52 s16 hold / vanish
//! timer, +0x54 s16 shot cooldown, +0x56 muzzle toggle, +0x58 the life timer, +0x5c shots fired, +0x60 the target
//! (the port stores moby index + 1, 0 none; the game a pointer), +0x64 the loop voice, +0x70 / +0x80 joint points
//! 3 / 4. The walker record the game keeps in the global `0x1deb38` (one mouse per level) is kept at
//! [`WALKER`] in the mouse's own pvar block (the port's layout).
//!
//! The glow sprites of draw callback 0x30de68 ([`glow_quads`]) and the jet particles (type 74, `0x30dda0`, [`jets`]) are
//! ported, and the blob shadow `0x26eec8(0.3)` (`crate::shadows::blob`). The
//! Summoner moby's state writes (1 at the summon, 2 at the end) have no reader: class 0x1b1's update is empty. The big-head cheat 0x15edb0 (states ≥ 3: P+0x90 on list 5 at 2.1) is
//! `manip::actor_big_head`. The camera-mode check (`0x167280`+0x86 == 0x14) is taken as
//! false (no camera mode 0x14 in the port).

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::creature::{self as c, add_rot, atan, diff_rots, turn, walker, DT, DT2, SPEED, V};
use crate::moby_update::services::{pv, pvar as p, HeroCall, HitTemplate, World};
use std::f32::consts::{PI, TAU};

/// The house's update address in the level01 class table.
pub const HOUSE_FN: u32 = 0x2f2b68;
/// The mouse's glow draw callback and the level its addresses are from.
pub const GLOW_FN: u32 = 0x30_de68;
pub const REFERENCE_LEVEL: u32 = 1;
/// The jets' type-74 descriptors (gp−0x4b20, −0x4b10, −0x4b00 on level 01).
const JETS: [crate::particles::type74::Desc; 3] = [
    crate::particles::type74::Desc { rgba0: 0xcf00_00ff, rgba1: 0x0000_00cf, size_range: -10, size_base: 20, life: 11 },
    crate::particles::type74::Desc { rgba0: 0x6000_ffff, rgba1: 0x0000_0080, size_range: -10, size_base: 30, life: 12 },
    crate::particles::type74::Desc { rgba0: 0xefff_7f4f, rgba1: 0x00ff_0000, size_range: -12, size_base: 12, life: 7 },
];
pub const HOUSE_CLASSES: [i16; 1] = [604];
/// The mouse's.
pub const MOUSE_FN: u32 = 0x30df40;
pub const MOUSE_CLASSES: [i16; 1] = [1818];
/// The shot's (`CreateMoby(0x661)` in 0x30c898).
pub const SHOT_FN: u32 = 0x30c9a8;
pub const SHOT_CLASSES: [i16; 1] = [1633];
pub const SHOT_CLASS: i16 = 1633;

/// The Sonic Summoner (head item, `0x13d4c0 + 5`).
pub const SUMMONER: i32 = 5;
/// The Summoner's moby class (`0x1b1`), put in its summon state by the mouse.
pub const SUMMONER_CLASS: i16 = 0x1b1;
/// Ratchet's hold state during the summon.
pub const HOLD_STATE: i32 = 0x1f;

/// Level01 `.lit` (gp−0x4b80..): the shot cooldown (8 ticks), the search (range 20, yaw cone 180°, pitch cone 90°),
/// the muzzle offset and the default aim height (0.3), the life (5400 ticks), the shot budget (100), the vanish
/// (30 ticks, life `rand_range(20, 40)`, jitter 0.3, size `randf(0.5, 3)`, alpha 0xff, gravity −4.5), the turn to the
/// target (1220°, 90°, 1220°) and the tilt (5, 2, 0.1, 0.05). Identical on the ten levels.
pub mod k {
    pub const COOLDOWN: i32 = 8;
    pub const RANGE: f32 = 20.0;
    pub const YAW_CONE: f32 = 180.0;
    pub const PITCH_CONE: f32 = 90.0;
    pub const MUZZLE: f32 = 0.3;
    pub const AIM_HEIGHT: f32 = 0.3;
    pub const LIFE: i32 = 5400;
    pub const SHOTS: i32 = 100;
    pub const VANISH: i32 = 30;
    pub const SPARK_LIFE: (i32, i32) = (20, 40);
    pub const SPARK_JITTER: f32 = 0.3;
    pub const SPARK_SIZE: (f32, f32) = (0.5, 3.0);
    pub const SPARK_ALPHA: u32 = 0xff;
    pub const SPARK_GRAVITY: f32 = -4.5;
    pub const AIM_TURN: (f32, f32, f32) = (1220.0, 90.0, 1220.0);
    pub const ROLL_K: f32 = 5.0;
    pub const PITCH_K: f32 = 2.0;
    pub const ROLL_EASE: f32 = 0.1;
    pub const PITCH_EASE: f32 = 0.05;
}

/// Mouse pvar offsets.
pub mod pvo {
    pub const TARGET: usize = 0x00;
    pub const VEL: usize = 0x10;
    pub const SPAWN: usize = 0x20;
    pub const HOME: usize = 0x30;
    pub const CUBOID: usize = 0x40;
    pub const HOUSE: usize = 0x48;
    pub const STUCK: usize = 0x50;
    pub const HOLD: usize = 0x52;
    pub const COOLDOWN: usize = 0x54;
    pub const MUZZLE: usize = 0x56;
    pub const LIFE: usize = 0x58;
    pub const SHOTS: usize = 0x5c;
    pub const AIM: usize = 0x60;
    pub const VOICE: usize = 0x64;
    pub const JOINTS: usize = 0x70;
    /// The disc block's size.
    pub const LEN: usize = 0xd0;
}

/// The walker record (the game's global `0x1deb38`), in the port kept after the mouse's own pvar block.
pub const WALKER: usize = pvo::LEN;
/// The pvar block's size in the port (the disc block and the walker record).
pub const MOUSE_LEN: usize = WALKER + 0x50;

/// The mouse globals ([`crate::moby_update::Services::mouse`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Globals {
    /// `0x1deb88`: the mouse Ratchet summoned (set at the summon, cleared when it is home again).
    pub summoned: Option<MobyId>,
}

fn house_of(w: &World, id: MobyId) -> Option<MobyId> {
    let h = c::pi32(w, id, pvo::HOUSE);
    usize::try_from(h).ok().filter(|&h| h < w.table.mobys.len())
}

fn hero_feet(w: &World) -> V { let q = w.hero_point(); [q[0], q[1], q[2], 0.0] }

/// Ratchet's moby rows 1 and 2 (`0x1413d0` +0xd0 / +0xe0): from his moby, else his yaw (upright).
fn hero_rows(w: &World) -> (V, V) {
    if let Some(h) = w.hero_moby {
        let r = w.m(h).rows;
        return (r[1], r[2]);
    }
    let yaw = crate::moby_update::services::fl(w.hero.rot[2]);
    ([-yaw.sin(), yaw.cos(), 0.0, 0.0], [0.0, 0.0, 1.0, 0.0])
}

fn owned(w: &World, item: i32) -> bool { w.hero.owned.0.get(item as usize).is_some_and(|&b| b != 0) }

fn voice_keep(w: &mut World, id: MobyId) {
    let v = c::pi32(w, id, pvo::VOICE);
    if !w.sound_alive(v, id) {
        let s = w.play_sound(1, 4, id);
        c::set_pi32(w, id, pvo::VOICE, s);
    }
}

fn voice_release(w: &mut World, id: MobyId) {
    let v = c::pi32(w, id, pvo::VOICE);
    if v != -1 { w.release_sound(v, id); }
    c::set_pi32(w, id, pvo::VOICE, -1);
}

fn blend(w: &mut World, id: MobyId, seq: u8, t: i32) { let t = w.ticks(t); c::blend_to(w, id, seq, 0, t); }

fn looped(w: &World, id: MobyId) -> bool { w.m(id).anim.flags & 2 != 0 }

// ---------------------------------------------------------------------------------------------------------------
// The house (604)

/// `CommandedNpcUpdate` 0x2f2b68 (module doc).
pub fn house_update(w: &mut World, id: MobyId) {
    let has_pvars = w.m(id).pvars.len() >= 8;
    if has_pvars && w.m(id).state != 0 && owned(w, SUMMONER) {
        if w.hero.head_slot.id != SUMMONER { return house_states(w, id); }
        let target = c::pf(w, id, 0);
        let mut v = c::pf(w, id, 4);
        let y = turn::spring_turn(c::yaw(w, id), target, DT2 * TAU, DT2 * 5.759_586_3, 0.0, &mut v);
        c::set_pf(w, id, 4, v);
        c::set_yaw(w, id, y);
    }
    house_states(w, id)
}

fn house_states(w: &mut World, id: MobyId) {
    let cmd = w.m(id).cmd;
    let next = match w.m(id).state {
        0 => {
            if w.m(id).anim.seq_b != 0 { let t = w.ticks(0); w.anim_blend(id, 0, 0, t); }
            let y = c::yaw(w, id);
            w.mm(id).state = 1;
            if w.m(id).pvars.len() >= 4 { c::set_pf(w, id, 0, y); }
            return;
        }
        1 if (3..=5).contains(&cmd) => (1, 10, 2),
        2 if looped(w, id) => (2, 3, 3),
        3 => match cmd {
            4 | 5 => (3, 3, 4),
            7 | 1 => (6, 3, 7),
            _ => return,
        },
        4 if looped(w, id) => (4, 3, 5),
        5 if matches!(cmd, 3 | 6 | 7 | 1) => (5, 3, 6),
        6 if looped(w, id) => if cmd == 7 || cmd == 1 { (6, 3, 7) } else { (2, 3, 3) },
        7 if looped(w, id) => (0, 3, 1),
        _ => return,
    };
    let (seq, t, state) = next;
    blend(w, id, seq, t);
    w.mm(id).state = state;
}

// ---------------------------------------------------------------------------------------------------------------
// The mouse (1818)

/// `SeedJumpPattern(0x1deb38)` and the mouse's own values (state 0).
fn seed_walker(w: &mut World, id: MobyId) {
    let t4 = w.ticks(4);
    let pv = &mut w.mm(id).pvars;
    walker::seed(pv, WALKER);
    let f = |pv: &mut Vec<u8>, word: usize, x: f32| p::set_ff(pv, WALKER + 4 * word, x);
    f(pv, 16, DT2 * PI);
    f(pv, 5, SPEED * 0.4);
    f(pv, 9, DT * 2.7);
    f(pv, 4, SPEED * 0.01);
    f(pv, 15, DT2 * TAU);
    f(pv, 17, DT * 11.519_173);
    let fl = p::u32(pv, WALKER + 0x38) | 8;
    p::set_u32(pv, WALKER + 0x38, fl);
    p::set_i32(pv, WALKER, 500);
    f(pv, 3, 0.15);
    f(pv, 1, 0.3);
    f(pv, 12, 0.005);
    f(pv, 2, 0.15);
    f(pv, 7, SPEED * 0.01);
    p::set_i32(pv, WALKER + 0x48, t4);
    f(pv, 10, PI);
}

fn jw(w: &World, id: MobyId, word: usize) -> f32 { c::pf(w, id, WALKER + 4 * word) }

/// `SpringTurn2(a, J15, J16, J17, m, &J5)`: the mouse's turn.
fn turn_to(w: &mut World, id: MobyId, a: f32) {
    let (acc, damp, max) = (jw(w, id, 15), jw(w, id, 16), jw(w, id, 17));
    turn::spring_turn2_pvar(w, id, a, acc, damp, max, WALKER + 0x14);
}

fn heading_to(w: &World, id: MobyId, q: V) -> f32 { let m = c::pos(w, id); atan(q[0] - m[0], q[1] - m[1]) }

/// `0x30d208`: the target = Ratchet's feet + his row 1 (unit).
fn target_beside(w: &mut World, id: MobyId) {
    let (r1, _) = hero_rows(w);
    let t = c::add(c::set_len3(r1, 1.0), hero_feet(w));
    c::set_pv4(w, id, pvo::TARGET, t);
}

/// `0x30d260`: the flying target = 1.1·row 1 + row 2 + Ratchet's feet, 1 higher.
fn target_hover(w: &mut World, id: MobyId) {
    let (r1, r2) = hero_rows(w);
    let mut t = c::add(c::add(c::set_len3(r1, 1.1), c::set_len3(r2, 1.0)), hero_feet(w));
    t[2] += 1.0;
    c::set_pv4(w, id, pvo::TARGET, t);
}

/// `0x26de80(m, J, target, &out)` on the mouse's walker record: [`walker::walk_to`] (shared with the Glove of Doom's
/// bots).
pub fn walk_to(w: &mut World, id: MobyId, target: V, out: &mut V) -> u32 { walker::walk_to(w, id, WALKER, target, out) }

/// `0x30d880`: the flight toward the target (module doc of the game's function in the port's words: aim, steer, move
/// with collision, keep 1.1 from Ratchet, tilt).
fn fly(w: &mut World, id: MobyId) {
    let tgt = c::pv4(w, id, pvo::TARGET);
    let mut to = c::sub(tgt, c::pos(w, id));
    let refp = if w.m(id).state == 6 { c::pv4(w, id, pvo::HOME) } else { hero_feet(w) };
    let d_ref = c::dist2(c::pos(w, id), refp);
    let cmd = w.m(id).cmd;
    if cmd == 2 {
        let aim = c::pi32(w, id, pvo::AIM);
        let k = match usize::try_from(aim - 1).ok().filter(|&a| aim > 0 && a < w.table.mobys.len()) {
            None => {
                let a = heading_to(w, id, tgt);
                turn_to(w, id, a);
                SPEED
            }
            Some(t) => {
                let a = heading_to(w, id, w.m(t).position);
                let r = PI / 180.0;
                turn::spring_turn2_pvar(w, id, a, k::AIM_TURN.0 * r * DT2, k::AIM_TURN.1 * r * DT2, k::AIM_TURN.2 * r * DT, WALKER + 0x14);
                SPEED
            }
        };
        to = c::scale(to, k * 0.04);
        let vel = c::pv4(w, id, pvo::VEL);
        to = c::sub(to, vel);
        to = if 1.3 < d_ref { c::scale(to, SPEED * 0.13) } else { c::scale(to, SPEED * 0.25) };
        to[2] *= SPEED * -0.100_000_024 + 1.0;
        let mut v = c::add(vel, to);
        if w.m(id).state == 6 { v = c::clamp_len3(v, DT * 2.7); }
        let radius = 500.0 / 1024.0;
        let down = jw(w, id, 12);
        let hit = walker::move_collide(w, id, 1.0, radius, down, &mut v, 0);
        let l = c::len3(v);
        if hit == 0 {
            c::set_pi16(w, id, pvo::STUCK, 0);
        } else if l < 0.005 {
            let n = c::pi16(w, id, pvo::STUCK).wrapping_add(1);
            c::set_pi16(w, id, pvo::STUCK, n);
            if (w.ticks(0x14) as i16) < n && w.m(id).visible == 0 {
                w.mm(id).position = tgt;
                c::set_pi16(w, id, pvo::STUCK, 0);
            }
        }
        if d_ref < 1.1 {
            let h = hero_feet(w);
            let a = heading_to(w, id, h);
            v[0] -= (1.1 - d_ref) * a.cos() * SPEED * 0.1;
            v[1] -= (1.1 - d_ref) * a.sin() * SPEED * 0.1;
        }
        c::set_pv4(w, id, pvo::VEL, v);
    } else {
        if cmd == 0 {
            let a = heading_to(w, id, tgt);
            turn_to(w, id, a);
        } else {
            w.mm(id).cmd = 0;
        }
        if 1.3 < d_ref { w.mm(id).cmd = 2; }
    }
    // The tilt: roll from the turn rate, pitch from the speed (`0x2731d0` wraps into [−π, π)).
    let wrap = |x: f32| ((x + PI) / TAU - ((x + PI) / TAU).floor()) * TAU - PI;
    let roll_t = wrap(jw(w, id, 5) * k::ROLL_K);
    let m = w.mm(id);
    m.rotation[0] += (roll_t - m.rotation[0]) * k::ROLL_EASE;
    let pitch_t = wrap(c::len3(c::pv4(w, id, pvo::VEL)) * k::PITCH_K);
    let m = w.mm(id);
    m.rotation[1] += (pitch_t - m.rotation[1]) * k::PITCH_EASE;
}

/// `0x30d308`: the best enemy for a shot from `muzzle` (module doc of [`crate::targeting`]; this is the mouse's own
/// search): over the target list, a creature (class type byte +0x46 = 5) with health, within `range`, inside the
/// yaw / pitch cones of the mouse's facing (or the near cones within `near`), scored `dyaw²·dpitch²·√d` (lowest
/// wins) with a clear line from the muzzle (or a line ending on another creature).
pub fn search(w: &World, id: MobyId, muzzle: V) -> Option<MobyId> {
    let r = PI / 180.0;
    let (yc, pc, range, near, nyc, npc) = (k::YAW_CONE * r, k::PITCH_CONE * r, k::RANGE, 2.0f32, PI, PI);
    let rot = w.m(id).rotation;
    // The target list 0x1abe80 is built with this tick's run list (the scheduler's list build, redone here).
    let (_, targets) = crate::moby_update::scheduler::build_active_list(w.table, w.camera, &w.svc.groups);
    let enemy = |t: MobyId| -> bool {
        let m = w.m(t);
        m.has_class && w.classes.info(m.o_class).is_some_and(|i| i.ty == 5)
    };
    let mut best = 1e9f32;
    let mut out = None;
    for t in targets {
        let m = w.m(t);
        if m.draw_dist == 0 { continue; }
        let Some(rec) = crate::targeting::record(m) else { continue };
        if !enemy(t) || p::ff(&m.pvars, rec) <= 0.0 { continue; }
        let d = c::dist3(muzzle, m.position);
        if d >= range { continue; }
        let aim = [m.position[0], m.position[1], m.position[2] + 0.4, 0.0];
        let dy = diff_rots(rot[2], atan(aim[0] - muzzle[0], aim[1] - muzzle[1]));
        let dy2 = dy * dy;
        if !(dy2 < yc * yc || (d < near && dy2 < nyc * nyc)) { continue; }
        let dp = diff_rots(rot[1], atan(d, aim[2] - muzzle[2]));
        let dp2 = dp * dp;
        if !(dp2 < pc * pc || (d < near && dp2 < npc * npc)) { continue; }
        let score = dy2 * dp2 * c::dist3(muzzle, aim).sqrt();
        if score >= best { continue; }
        let ok = match w.coll_line(pv(muzzle), pv(aim), 0, Some(id)) {
            None => true,
            Some(h) => h.moby.is_some_and(|hm| Some(hm) != w.hero_moby && enemy(hm)),
        };
        if ok {
            best = score;
            out = Some(t);
        }
    }
    out
}

/// `0x30d5f0`: the gun (module doc).
fn shoot(w: &mut World, id: MobyId) {
    if c::dec_timer_pvar_s16(w, id, pvo::COOLDOWN) == 0 { return; }
    let side = (c::pu8(w, id, pvo::MUZZLE) & 1) as usize + 1;
    let mut muzzle = w.joint_point(id, side);
    let b = c::pu8(w, id, pvo::MUZZLE) ^ 1;
    c::set_pu8(w, id, pvo::MUZZLE, b);
    let Some(t) = search(w, id, muzzle) else {
        c::set_pi32(w, id, pvo::AIM, 0);
        return;
    };
    c::set_pi32(w, id, pvo::AIM, t as i32 + 1);
    if c::pos(w, id)[2] <= w.hero_point()[2] + 1.1 { return; }
    let tp = w.m(t).position;
    let a = atan(tp[0] - c::pos(w, id)[0], tp[1] - c::pos(w, id)[1]);
    if diff_rots(c::yaw(w, id), a) >= std::f32::consts::FRAC_PI_6 || w.svc.game_mode == 2 { return; }
    let h = crate::targeting::aim_height(w.m(t)).unwrap_or(k::AIM_HEIGHT);
    let aim = [tp[0], tp[1], tp[2] + h, 0.0];
    let off = c::set_len3(c::sub(aim, muzzle), k::MUZZLE);
    muzzle = c::add(muzzle, off);
    let yaw = atan(aim[0] - muzzle[0], aim[1] - muzzle[1]);
    let pitch = -atan(c::dist2(muzzle, aim), aim[2] - muzzle[2]);
    create_shot(w, id, yaw, pitch, muzzle);
    w.play_sound(0, 0, id);
    let cd = w.ticks(k::COOLDOWN) as i16;
    c::set_pi16(w, id, pvo::COOLDOWN, cd);
    let n = c::pi32(w, id, pvo::SHOTS) + 1;
    c::set_pi32(w, id, pvo::SHOTS, n);
}

/// `0x30c898(yaw, pitch, yaw, pitch, mouse, muzzle)`: `CreateMoby(0x661)`, P[2] / P[3] the aim, P[4] the mouse,
/// life ticks(35) (+0x14 and +0x16), update distance 0xff, draw distance 0x7f, drawn, light word of the mouse,
/// Euler (0, pitch, yaw), alpha 0x40, mode |= 0xa00, half scale.
fn create_shot(w: &mut World, id: MobyId, yaw: f32, pitch: f32, at: V) -> Option<MobyId> {
    let s = w.create_moby(SHOT_CLASS)?;
    let life = w.ticks(0x23) as i16;
    let (light, ambient) = (w.m(id).light, w.m(id).ambient);
    let m = w.mm(s);
    if m.pvars.len() < 0x18 { m.pvars.resize(0x80, 0); }
    p::set_i32(&mut m.pvars, 0x10, id as i32);
    m.update_dist = 0xff;
    m.draw_dist = 0x7f;
    m.visible = 1;
    m.light = light;
    m.ambient = ambient;
    m.rotation = [0.0, pitch, yaw, m.rotation[3]];
    m.position = at;
    p::set_ff(&mut m.pvars, 8, yaw);
    p::set_ff(&mut m.pvars, 0xc, pitch);
    p::set_ff(&mut m.pvars, 0, 0.0);
    p::set_ff(&mut m.pvars, 4, 0.0);
    p::set_i16(&mut m.pvars, 0x16, life);
    p::set_i16(&mut m.pvars, 0x14, life);
    m.alpha = 0x40;
    m.mode |= 0xa00;
    m.scale *= 0.5;
    Some(s)
}

/// The house's +0x94 around the mouse's update (cleared at the start, the class's collision at the end).
fn house_collision(w: &mut World, house: Option<MobyId>, on: bool) {
    let Some(h) = house else { return };
    let coll = on && w.classes.info(w.m(h).o_class).is_some_and(|i| i.has_collision && !i.no_header);
    w.mm(h).has_collision = coll;
}

/// `MouseCritterUpdate` 0x30df40 (module doc).
pub fn mouse_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pvo::LEN { return; }
    if w.m(id).pvars.len() < MOUSE_LEN { w.mm(id).pvars.resize(MOUSE_LEN, 0); }
    let house = house_of(w, id);
    if let Some(h) = house {
        // The house turns to Ratchet (to the mouse once it is out).
        let (a, b) = if w.m(id).state < 5 { (w.hero_point(), w.m(h).position) } else { let m = w.m(id).position; ([m[0], m[1], m[2]], w.m(h).position) };
        let y = atan(a[0] - b[0], a[1] - b[1]);
        if w.m(h).pvars.len() >= 4 { p::set_ff(&mut w.mm(h).pvars, 0, y); }
        house_collision(w, house, false);
    }
    if w.m(id).state >= 3 {
        // The actors' big-head cheat 0x15edb0: the record P+0x90 on list 5 at 2.1. 0x30ded0: joint points 3 / 4.
        crate::moby_update::manip::actor_big_head(w, 2.1, id, 5, id, 0x90);
        let (a, b) = (w.joint_point(id, 3), w.joint_point(id, 4));
        c::set_pv4(w, id, pvo::JOINTS, a);
        c::set_pv4(w, id, pvo::JOINTS + 0x10, b);
        // `RegisterDrawCallback(0x30de68)`: the glow sprites at the two joint points ([`glow_quads`]).
        if let Some(r) = crate::moby_update::classes::units::row(REFERENCE_LEVEL, GLOW_FN) {
            w.svc.draw_callbacks.register(crate::moby_update::classes::draw_callbacks::Callback::UnitGlow(r), id);
        }
    }
    match w.m(id).state {
        0 => init(w, id, house),
        1 => idle(w, id, house),
        2 => {
            match house {
                None => {
                    c::set_pi16(w, id, pvo::HOLD, 0);
                    w.mm(id).draw_dist = 0xff;
                    blend(w, id, 0, 0x14);
                    if c::pi32(w, id, pvo::VOICE) == -1 { let s = w.play_sound(1, 4, id); c::set_pi32(w, id, pvo::VOICE, s); }
                    w.mm(id).state = 3;
                }
                Some(h) if w.m(h).state == 5 => {
                    blend(w, id, 0, 10);
                    w.mm(id).state = 3;
                    c::set_pi16(w, id, pvo::HOLD, 0);
                    w.mm(id).draw_dist = 0xff;
                }
                Some(_) => {}
            }
        }
        3 => run_out(w, id),
        4 => {
            voice_keep(w, id);
            w.hero_fields_mut().call(HeroCall::SetState { id: 0, play: true });
            w.mm(id).mode &= !(mode::HIDDEN | mode::NO_ANIM);
            if let Some(h) = house { w.mm(h).mode &= !(mode::HIDDEN | mode::NO_ANIM); }
            w.mm(id).state = 5;
        }
        5 => follow(w, id, house),
        6 => vanish(w, id),
        7 => {
            voice_release(w, id);
            c::set_pv4(w, id, pvo::VEL, [0.0; 4]);
            let t = house.map(|h| w.m(h).position).unwrap_or(c::pv4(w, id, pvo::HOME));
            c::set_pv4(w, id, pvo::TARGET, t);
            w.svc.mouse.summoned = None;
            let a = heading_to(w, id, t);
            turn_to(w, id, a);
            blend(w, id, 0, 6);
            if let Some(h) = house { w.mm(h).cmd = 1; }
            let m = w.mm(id);
            m.state = 1;
            m.draw_dist = 0;
        }
        _ => {}
    }
    if w.m(id).state >= 0x80 { return; }
    if w.m(id).state >= 2 {
        crate::shadows::blob(w, f32::from_bits(0x3e99_999a), id);
        jets(w, id);
    }
    house_collision(w, house, true);
}

/// The mouse's jets (`0x30dda0`): drawn → at joint list 0, a velocity `(0, 0, −randf(1, 2)·dt)` and three type-74
/// puffs with the descriptors gp−0x4b20 / −0x4b10 / −0x4b00 (additive).
fn jets(w: &mut World, id: MobyId) {
    if w.m(id).visible == 0 { return; }
    let r = w.rng.randf(1.0, 2.0);
    let v = [0.0, 0.0, -(r * DT)];
    let p = w.joint_point(id, 0);
    for d in JETS { crate::moby_update::creature::fx::part74(w, id, p, v, d, true); }
}

/// `0x30de68` (draw only): two glow quads (size 0.1, pull 0.08, 0x2528aa28) at the joint points +0x70 / +0x80.
pub fn glow_quads(table: &crate::moby_runtime::MobyTable, id: MobyId) -> Vec<crate::moby_update::classes::units::GlowQuad> {
    let Some(m) = table.mobys.get(id).filter(|m| m.pvars.len() >= pvo::JOINTS + 0x20) else { return Vec::new() };
    (0..2).map(|k| {
        let q = p::v4f(&m.pvars, pvo::JOINTS + 0x10 * k);
        crate::moby_update::classes::units::GlowQuad { size: f32::from_bits(0x3dcc_cccd), pull: f32::from_bits(0x3da3_d70a), point: [q[0], q[1], q[2]], rgba: 0x2528_aa28 }
    }).collect()
}

fn init(w: &mut World, id: MobyId, house: Option<MobyId>) {
    c::set_pv4(w, id, pvo::TARGET, [0.0; 4]);
    c::set_pv4(w, id, pvo::VEL, [0.0; 4]);
    c::set_pi16(w, id, pvo::STUCK, 0);
    c::set_pi16(w, id, pvo::HOLD, 0);
    if w.m(id).anim.seq_b != 0 { let t = w.ticks(10); w.anim_blend(id, 0, 0, t); }
    w.mm(id).update_dist = 0xff;
    w.mm(id).draw_dist = 0xff;
    // "no start house, killing mouse"
    let Some(h) = house else { return w.delete_moby(id) };
    let mut pos = w.m(h).position;
    pos[2] += 0.02;
    w.mm(id).position = pos;
    // "mouse already completed": its spawn id collected, or its death bit on this level.
    let (sid, level) = (w.m(id).spawn_id, w.svc.level);
    if w.svc.save.collected.get(&sid).is_some_and(|&b| b != 0) || w.svc.save.death.contains(&(level, sid)) { return w.delete_moby(id); }
    c::set_pv4(w, id, pvo::SPAWN, pos);
    c::set_pv4(w, id, pvo::HOME, pos);
    seed_walker(w, id);
    let m = w.mm(id);
    m.state = 1;
    m.draw_dist = 0;
}

/// State 1: at home; the summon (module doc).
fn idle(w: &mut World, id: MobyId, house: Option<MobyId>) {
    if !owned(w, SUMMONER) || w.hero.head_slot.id != SUMMONER { return; }
    c::set_pi32(w, id, pvo::SHOTS, 0);
    let life = w.ticks(k::LIFE);
    c::set_pi32(w, id, pvo::LIFE, life);
    if w.m(id).visible != 0 && looped(w, id) { blend(w, id, 2, 10); }
    let feet = w.hero_point();
    let cub = c::pi32(w, id, pvo::CUBOID);
    let in_area = w.in_cuboid(feet, cub);
    if let Some(h) = house { w.mm(h).cmd = if in_area { 3 } else { 1 }; }
    if w.hero.worn.head.is_none() { return; }
    let a = heading_to(w, id, hero_feet(w));
    turn_to(w, id, a);
    if !in_area { return; }
    // A clear line from the house (0.5 up) to Ratchet (0.2 up); a hit on Ratchet's own moby counts as clear.
    if let Some(h) = house {
        let hp = w.m(h).position;
        let from = [hp[0], hp[1], hp[2] + 0.5, 0.0];
        let to = [feet[0], feet[1], feet[2] + 0.2, 0.0];
        if let Some(hit) = w.coll_line(pv(from), pv(to), 0, Some(id)) {
            if hit.moby.is_none() || hit.moby != w.hero_moby { return; }
        }
    }
    target_beside(w, id);
    // The Summoner moby's state = 1 (0x140480 +0x20): no reader (class 0x1b1's update 0x2e30b0 is `jr ra`, and no other
    // code reads it), so the port keeps no state on the worn head moby.
    w.svc.mouse.summoned = Some(id);
    let blend6 = w.ticks(6) as f32;
    let f = w.hero_fields_mut();
    f.call(HeroCall::SetState { id: HOLD_STATE, play: false });
    f.call(HeroCall::SetAnim { blend: blend6, seq: 0, frame: 0 });
    let Some(h) = house else {
        // (Unreachable: the init deletes a mouse without a house.)
        w.mm(id).draw_dist = 0xff;
        blend(w, id, 0, 10);
        w.mm(id).state = 3;
        return;
    };
    w.mm(h).cmd = 5;
    w.mm(id).state = 2;
}

/// State 3: out of the house to Ratchet (on the ground near home, flying further out).
fn run_out(w: &mut World, id: MobyId) {
    voice_keep(w, id);
    let home = c::pv4(w, id, pvo::HOME);
    let d = c::dist2(c::pos(w, id), home);
    let r = if d <= 0.6 || c::pi16(w, id, pvo::HOLD) != 0 {
        target_beside(w, id);
        let t = c::pv4(w, id, pvo::TARGET);
        let a = heading_to(w, id, t);
        turn_to(w, id, a);
        let mut v = c::pv4(w, id, pvo::VEL);
        v[2] = 0.0;
        let r = walk_to(w, id, t, &mut v);
        c::set_pv4(w, id, pvo::VEL, v);
        r
    } else {
        c::set_pf(w, id, WALKER + 4, 0.7);
        target_hover(w, id);
        fly(w, id);
        if c::pi16(w, id, pvo::STUCK) < 0x15 { 0 } else { 2 }
    };
    let arrived = r & 6 != 0 || c::dist3(c::pos(w, id), c::pv4(w, id, pvo::TARGET)) < 0.25;
    if arrived {
        blend(w, id, 2, 10);
        c::set_pv4(w, id, pvo::VEL, [0.0; 4]);
        w.mm(id).state = 4;
        let t = w.ticks(0x78) as i16;
        c::set_pi16(w, id, pvo::HOLD, t);
    }
    if looped(w, id) { flap(w, id); }
}

/// On an anim loop: anim 1 (blend `rand_range(7, 12)`), speed `randf(0.95, 1.05)`.
fn flap(w: &mut World, id: MobyId) {
    if w.m(id).anim.seq_b != 1 {
        let n = w.rng.rand_range(7, 0xc);
        let t = w.ticks(n);
        w.anim_blend(id, 1, 0, t);
    }
    let s = w.rng.randf(f32::from_bits(0x3f73_3333), f32::from_bits(0x3f86_6666));
    w.mm(id).anim.speed = s;
}

/// State 5: beside Ratchet, shooting (module doc).
fn follow(w: &mut World, id: MobyId, house: Option<MobyId>) {
    voice_keep(w, id);
    let hold = c::pi16(w, id, pvo::HOLD);
    let t20 = w.ticks(0x14) as i16;
    if hold == t20 || k::SHOTS - 5 < c::pi32(w, id, pvo::SHOTS) {
        blend(w, id, 2, 5);
    } else if t20 < hold && looped(w, id) {
        flap(w, id);
    }
    let h = w.hero;
    // In the game's order (the life timer only counts when nothing before it ended the run).
    let done = w.hero.worn.head.is_none() || h.head_slot.id != SUMMONER || h.group == 0x16 || h.group == 0x14 || h.mode != 0
        || c::dec_timer_pvar_i32(w, id, pvo::LIFE) != 0 || k::SHOTS <= c::pi32(w, id, pvo::SHOTS);
    if done {
        c::set_pv4(w, id, pvo::VEL, [0.0; 4]);
        c::set_pi16(w, id, pvo::HOLD, k::VANISH as i16);
        voice_release(w, id);
        w.play_sound(2, 0, id);
        w.mm(id).state = 6;
        if let Some(hs) = house { w.mm(hs).cmd = 5; }
    } else {
        target_hover(w, id);
        fly(w, id);
    }
    shoot(w, id);
}

/// State 6: the vanish (sparks, fade, spin), then home.
fn vanish(w: &mut World, id: MobyId) {
    voice_release(w, id);
    if c::dec_timer_pvar_s16(w, id, pvo::HOLD) == 0 {
        let m = c::pos(w, id);
        let mut q = [m[0], m[1], m[2], 0.0];
        for c in q.iter_mut().take(3) { *c += w.rng.randf(-k::SPARK_JITTER, k::SPARK_JITTER); }
        let r1 = w.rng.rand() as u32;
        let r2 = w.rng.rand() as u32;
        let r3 = w.rng.rand() as u32;
        let r4 = w.rng.rand();
        let t0 = if c::pi16(w, id, pvo::HOLD) & 1 != 0 { -(r4 % 2) } else { r4 % 2 };
        let life = w.rng.rand_range(k::SPARK_LIFE.0, k::SPARK_LIFE.1);
        let size = w.rng.randf(k::SPARK_SIZE.0, k::SPARK_SIZE.1);
        let rgba = (r1.wrapping_add(0x30) & 0x3f) | (r2.wrapping_add(0x20) & 0x3f) << 8 | (r3 & 0x2f) << 16 | k::SPARK_ALPHA << 24;
        let pf = crate::moby_update::services::pf;
        w.part53(pf(size), pf(f32::from_bits(0x3a83_126f)), pf(k::SPARK_GRAVITY * DT2), pv(q), life, rgba, 2, t0 as i8, pv([0.0; 4]));
        let y = add_rot(c::yaw(w, id), DT * 25.132_742);
        let m = w.mm(id);
        m.mode |= 0x200;
        m.alpha = m.alpha.wrapping_sub((0x80 / k::VANISH) as u8);
        m.rotation[2] = y;
    } else {
        let home = c::pv4(w, id, pvo::HOME);
        let m = w.mm(id);
        m.alpha = 0x80;
        m.position = home;
        m.state = 7;
        m.mode &= !0x200;
        c::set_pv4(w, id, pvo::TARGET, home);
        c::set_pv4(w, id, pvo::VEL, [0.0; 4]);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The shot (1633)

/// 0x30c9a8: the mouse's shot: grows to the class scale (0.21·scale a tick), turns to its aim after the first tick
/// (`SpringTurn` 1.5π·dt² / π·dt² / π·dt on yaw and pitch), flies 40 u/s, and is deleted outside the world box, more
/// than 64 (xy) from the camera or when its 35 ticks run out. Each step is a line from the old to the new position
/// with its hit template (dir = (xy away from Ratchet, 1), attacker the shot, flags 0x10001, type 1 / 1, damage
/// 0.5, +0x20 = 1) ignoring the mouse: a hit on a surface other than water ends it there; a water hit shortens its
/// life to 3 ticks.
pub fn shot_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x18 { return; }
    let class_scale = w.classes.info(w.m(id).o_class).map(|i| i.scale).unwrap_or(w.m(id).scale);
    if w.m(id).scale < class_scale { w.mm(id).scale += SPEED * 0.21 * class_scale; }
    let life = c::pi16(w, id, 0x14) as i32;
    let (ty, tp) = if life < w.ticks(0x23) - w.ticks(1) { (c::pf(w, id, 8), c::pf(w, id, 0xc)) } else { (c::yaw(w, id), w.m(id).rotation[1]) };
    let mut v0 = c::pf(w, id, 0);
    let yaw = turn::spring_turn(c::yaw(w, id), ty, DT2 * 4.712_389, DT2 * PI, DT * PI, &mut v0);
    c::set_pf(w, id, 0, v0);
    c::set_yaw(w, id, yaw);
    let mut v1 = c::pf(w, id, 4);
    let pitch = turn::spring_turn(w.m(id).rotation[1], tp, DT2 * 4.712_389, DT2 * PI, DT * PI, &mut v1);
    c::set_pf(w, id, 4, v1);
    w.mm(id).rotation[1] = pitch;
    let step = crate::moby_update::creature::fx::polar(DT * 40.0, yaw, -pitch);
    let old = c::pos(w, id);
    let new = c::add(old, step);
    w.mm(id).position = new;
    let cam = [crate::moby_update::services::fl(w.camera[0]), crate::moby_update::services::fl(w.camera[1]), 0.0, 0.0];
    let inside = crate::moby_update::creature::projectile::in_world(new) && c::dist2(new, cam) <= 64.0;
    if !inside { return w.delete_moby(id); }
    let mut dir = c::set_len2(c::sub(new, hero_feet(w)), 1.0);
    dir[2] = 1.0;
    dir[3] = f32::from_bits(0x45af_df66);
    let tmpl = HitTemplate { dir: pv(dir), attacker: Some(id), flags: 0x10001, b18: 1, b19: 1, h1a: w.m(id).o_class as u16, damage: crate::moby_update::services::pf(0.5), w20: 1 };
    if c::dec_timer_pvar_s16(w, id, 0x14) != 0 { return w.delete_moby(id); }
    let mouse = usize::try_from(c::pi32(w, id, 0x10)).ok();
    let Some(h) = crate::moby_update::services::line_hit_in(w.table, w.svc, w.classes, w.coll, pv(old), pv(new), 0, mouse, &tmpl) else { return };
    if h.surface_id() != 0 {
        w.mm(id).position = [h.point[0], h.point[1], h.point[2], new[3]];
        return w.delete_moby(id);
    }
    let t3 = w.ticks(3) as i16;
    if t3 < c::pi16(w, id, 0x14) { c::set_pi16(w, id, 0x14, t3); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};

    fn world_parts() -> (crate::hero::Hero, crate::rng::Rng, crate::moby_update::ClassTable, crate::moby_update::Services) {
        (crate::hero::Hero::new(), crate::rng::Rng::new(), crate::moby_update::ClassTable::default(), crate::moby_update::Services::new())
    }

    fn house(cmd: u8) -> Moby {
        Moby { o_class: 604, pvars: vec![0; 16], cmd, ..Moby::default() }
    }

    /// The house opens on command 3, stays open, closes on 1.
    #[test]
    fn house_opens_and_closes() {
        let mut t = MobyTable::new(vec![house(0)], 4);
        let (hero, mut rng, classes, mut svc) = world_parts();
        let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 0);
        house_update(&mut w, 0);
        assert_eq!(w.m(0).state, 1);
        house_update(&mut w, 0);
        assert_eq!(w.m(0).state, 1, "no command");
        w.mm(0).cmd = 3;
        house_update(&mut w, 0);
        assert_eq!(w.m(0).state, 2);
        w.mm(0).anim.flags |= 2;
        house_update(&mut w, 0);
        assert_eq!(w.m(0).state, 3);
        w.mm(0).cmd = 1;
        house_update(&mut w, 0);
        assert_eq!(w.m(0).state, 7);
        house_update(&mut w, 0);
        assert_eq!(w.m(0).state, 1);
    }

    /// Without a house the mouse deletes itself; with one it goes home and waits (hidden: draw distance 0).
    #[test]
    fn mouse_home() {
        let mut m = Moby { o_class: 1818, pvars: vec![0; pvo::LEN], ..Moby::default() };
        p::set_i32(&mut m.pvars, pvo::HOUSE, -1);
        let mut t = MobyTable::new(vec![m.clone()], 4);
        let (hero, mut rng, classes, mut svc) = world_parts();
        let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 0);
        mouse_update(&mut w, 0);
        assert!(w.m(0).state >= 0x80, "no start house: deleted");
        let mut h = house(0);
        h.position = [10.0, 20.0, 30.0, 1.0];
        p::set_i32(&mut m.pvars, pvo::HOUSE, 1);
        let mut t = MobyTable::new(vec![m, h], 4);
        let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 0);
        mouse_update(&mut w, 0);
        assert_eq!((w.m(0).state, w.m(0).draw_dist), (1, 0));
        assert_eq!(w.m(0).position[..3], [10.0, 20.0, 30.02]);
        assert_eq!(p::i32(&w.m(0).pvars, WALKER), 500);
        // Without the Summoner nothing happens.
        for _ in 0..10 { mouse_update(&mut w, 0); }
        assert_eq!(w.m(0).state, 1);
    }
}
