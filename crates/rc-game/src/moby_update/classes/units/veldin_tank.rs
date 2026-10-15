//! **The tanks of Veldin's last level, class 1454** (level18 `0x2f7c40`, its prologue `0x2f8050` and drive `0x2f8658`;
//! census U598; three placed, #904..#906), **their treads 331** (`0x2ce7d0`, created by the tank) and **their shells
//! 41** (`0x2a7220`, its spawner `0x2a75f0`). A tank drives round its path (pvar +0x100), picking a random next point
//! at each, slowing to a crawl while it turns more than 15° off; its treads run at its speed. Within 30 of Ratchet (or
//! a decoy) it switches to its firing animation and its turret (joint list 0's manipulator) tracks the target with a
//! slight sway, firing a shell from either barrel at animation keys 4 and 10 with a puff of muzzle smoke; a shell
//! within 10 homes on Ratchet. Six damage blows it up (a beam explosion, nine pieces, the treads gone); a hit only
//! flashes it. Read from the level18 decomp and data (gp−0x4788 4.0, gp−0x4784 20.0, gp−0x4780 4.0). Native `f32`.
//!
//! **Pvars** (0x140): +0x20 the damage record (health 6.0, +0x24 6, +0x28 3), +0x40 (the explosion's), +0x60 the flash
//! record, +0x70 the target record (+0x70 its point, +0xb0 the moby, +0xb4 the kind; 2 none), +0xc0 the turret's
//! manipulator record, +0x100 the path, +0x110 / +0x114 the turret's yaw and its velocity, +0x118 / +0x11c its pitch,
//! +0x120 the path point, +0x124 `ticks(240)` (unread), +0x128 the body's turn velocity, +0x12c the speed, +0x130 the
//! aim reach, +0x134 the sway's phase, +0x138 / +0x13c the treads (the port: moby index + 1).
//!
//! ## Coverage
//! | address | what | port |
//! |---|---|---|
//! | `0x2f8050` (every tick, first) | `MobyGetHitMessage(m, 0x330000)`, `0x26f378(m, hit, +0x20, 0, …, col 4)`: not "none" (out ≠ 1) and state ≠ 0 → health −= damage; ≤ 0: `SpawnBeamExplosion(0, 0, 4, 2, 9, 1, 15, m, +0x40, position + (0, 0, 1), 20, 8, 20, −1, shake)`, `BreakFxB` pieces 0x700, 0x701, 0x702, 0x704, 0x6fe ×2, 0x6ff ×3 at its position / Euler, `SetDeathBits(m, 0, −1)`, the tank and both treads deleted; else flash +0x67 = 0x78, `0x272318(m, +0x60)` | [`prologue`] |
//! | | +0xa4 = 0xff; `0x274b78(30, m)` into +0x70: a target and Ratchet within 10 (xy) → it is Ratchet at his position; else a target farther than 30 (xy) → kind 2 | [`prologue`] (`target::acquire`) |
//! | | kind 2: the turret's yaw and pitch turned (`0x270cc0`: 4π·dt², 4π·dt², 3π·dt) to the body's yaw / 0; else d = target − position, z − 3 (unit beyond +0x130); yaw = atan d + sin(+0x134)·5°, pitch = −atan(\|d.xy\|, d.z) in [−45°, 20°] | [`prologue`] (`turn::turn_toward`) |
//! | | the record's rotation = `fun_001fa3c0(Q(pitch, y), Q(yaw − body yaw, z))` (`FUN_00221e38` = L01 `0x221e38`); `0x2723f8(m, +0x60)` | [`prologue`] (`manip::set_quat`, `flash::update`) |
//! | state 0 | `AttachManipulator(m, 0, +0xc0)`; the damage record (6.0, 6, 3); +0x124 = `ticks(240)`, +0x120 = 0; each missing tread: `CreateMoby(0x14b)`, update / draw distance 0x40, drawn, Ratchet's light / ambient (+0x38), mode 0x4000 / 0x8000, the tank's position, collision off, sequence 0 (`0x212f90`), its rows (`0x20def8`); blend sequence 1 over `ticks(10)` (unless playing), → 1 | [`update`] |
//! | state 1 | `0x2f8658`; a target (kind ≠ 2): +0x134 = 0, +0x130 = 10; sequence 2 → 2, else blend 2 over `ticks(20)` and → 2 | [`update`] |
//! | state 2 | `0x2f8658`; `Approach(30, 6·dt, +0x130)`; +0x134 += 2π·dt; key 4 or 10 passed (`0x2765b0`): the barrel (key time < 7: joint list 1, else 2), v = 20·dt along (yaw +0x110, pitch +0x118) (z negated sine), class sound 0, the muzzle smoke (`0x264c10`), a shell (`0x2a75f0`, homing within 10 of Ratchet); kind 2 → blend sequence 1 over `ticks(20)` (unless playing), → 1 | [`update`] (`fx::muzzle_smoke`) |
//! | `0x2f8658` | the path point (0x1b0eb0[+0x100] + 0x10 + 16·+0x120); body yaw turned to it (`0x270cc0`: π·dt², π·dt², 2π·dt, +0x128); speed `Approach(4·dt (dt beyond 15° off), dt², +0x12c)`; move = row 0 at the speed, `0x26d610(0.5, 0.5, 0, m, move, 0)`; within 1 (3-D) of the point: the next = (+0x120 + `randi(n − 1)` + 1) % n; the treads: anim speed `clamp(speed / (4·dt), 0, 2)`, the tank's Euler and position | [`drive`] (`walker::move_collide`) |
//! | `0x2ce7d0` (331) | a placed moby (below the first dynamic slot 0x15ffdc): anim speed 0, mode \| 0x40; the tank's created treads: nothing | [`tread_update`] |
//! | `0x2a75f0` (41) | `CreateMoby(0x29)`: update / draw distance 0xff, drawn, state 0, scale ·3, the tank's light / ambient; position = the barrel; +0x00 = v, +0x10 (s16) = `ticks(240)`, +0x14 the tank, +0x12 the homing flag; yaw = atan v, pitch +0x44 = −atan(\|v.xy\|, v.z) | [`spawn_shell`] |
//! | `0x2a7220` (41) | rot x += 2π·dt; homing: l = \|v\|, v = Ratchet's aim point (0x13f410) − position; longer than l → length l, yaw / pitch from it; shorter than 0.1 → deleted at once | [`shell_update`] |
//! | | the hit: dir = (position − the tank) flat at length 2 with the push marker (the owner a class 0x2e alive), else v at length 2; `0x26e808(2, tmpl, m, 1, dir)`, +0x1a the class; `CollLine_Fix(position, position + v, 0, the owner, tmpl)` | [`shell_update`] (`services::line_hit_in`) |
//! | | no hit and the timer (`FastDecTimer` +0x10) running: position += v; the trail (as the boss shell's `0x2d5488`, both puffs `ticks(10)`); else `SpawnBeamExplosion(1.5, 1, 1, 0.5, 9, 0.5, 0, m, v, position, 10, 6, 4, sound 0, 0)`, deleted | [`shell_update`] (`veldin_shots::trail_for`) |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::creature::{self as c, damage, flash, fx, ground, target, turn, walker, DT, DT2};
use crate::moby_update::manip;
use crate::moby_update::services::{self as sv, pvar as p, HitTemplate, World};
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 18;
pub const UPDATE_FN: u32 = 0x2f_7c40;
pub const CLASSES: [i16; 1] = [1454];
pub const TREAD_FN: u32 = 0x2c_e7d0;
pub const TREAD_CLASSES: [i16; 1] = [TREAD];
pub const SHELL_FN: u32 = 0x2a_7220;
pub const SHELL_CLASSES: [i16; 1] = [SHELL];

const TREAD: i16 = 0x14b;
const SHELL: i16 = 0x29;
/// The class whose shells push away from it (`0x2e`).
const PUSH_OWNER: i16 = 0x2e;
const PIECES: [i16; 9] = [0x700, 0x701, 0x702, 0x704, 0x6fe, 0x6fe, 0x6ff, 0x6ff, 0x6ff];
/// gp−0x4788 / −0x4784 / −0x4780.
const DRIVE: f32 = 4.0;
const SHELL_SPEED: f32 = 20.0;
const TREAD_RATE: f32 = 4.0;

mod pv {
    pub const LEN: usize = 0x140;
    pub const D: usize = 0x20;
    pub const F: usize = 0x60;
    pub const T_POS: usize = 0x70;
    pub const T_MOBY: usize = 0xb0;
    pub const T_KIND: usize = 0xb4;
    pub const MANIP: usize = 0xc0;
    pub const PATH: usize = 0x100;
    pub const YAW: usize = 0x110;
    pub const YAW_V: usize = 0x114;
    pub const PITCH: usize = 0x118;
    pub const PITCH_V: usize = 0x11c;
    pub const NODE: usize = 0x120;
    pub const T124: usize = 0x124;
    pub const BODY_V: usize = 0x128;
    pub const SPEED: usize = 0x12c;
    pub const REACH: usize = 0x130;
    pub const SWAY: usize = 0x134;
    pub const TREADS: usize = 0x138;
}

mod shell {
    pub const LEN: usize = 0x18;
    pub const VEL: usize = 0x00;
    pub const TIMER: usize = 0x10;
    pub const HOMING: usize = 0x12;
    pub const OWNER: usize = 0x14;
}

fn link(w: &World, id: MobyId, o: usize) -> Option<MobyId> {
    let v = c::pi32(w, id, o);
    usize::try_from(v - 1).ok().filter(|&m| m < w.table.mobys.len())
}

/// Level18 `0x2f7c40` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    crate::moby_update::story::pvars(w, id, pv::LEN);
    if !prologue(w, id) { return; }
    let st = w.m(id).state;
    match st {
        0 => {
            manip::attach(w, id, 0, id, pv::MANIP);
            {
                let pvs = &mut w.mm(id).pvars;
                p::set_u8(pvs, pv::D + 8, 3);
                p::set_i16(pvs, pv::D + 4, 6);
                p::set_ff(pvs, pv::D, 6.0);
            }
            let t = w.ticks(0xf0);
            c::set_pi32(w, id, pv::T124, t);
            c::set_pi32(w, id, pv::NODE, 0);
            for k in 0..2 {
                if link(w, id, pv::TREADS + 4 * k).is_none() { make_tread(w, id, k); }
            }
            blend(w, id, 1, 10);
            w.mm(id).state = 1;
        }
        1 => {
            drive(w, id);
            if c::pi32(w, id, pv::T_KIND) == 2 { return; }
            c::set_pf(w, id, pv::SWAY, 0.0);
            c::set_pf(w, id, pv::REACH, 10.0);
            if w.m(id).anim.seq_b == 2 {
                w.mm(id).state = 2;
                return;
            }
            let t = w.ticks(0x14);
            w.anim_blend(id, 2, 0, t);
            w.mm(id).state = 2;
        }
        2 => {
            drive(w, id);
            let mut r = c::pf(w, id, pv::REACH);
            turn::approach(30.0, DT * 6.0, &mut r);
            c::set_pf(w, id, pv::REACH, r);
            let s = c::add_rot(c::pf(w, id, pv::SWAY), DT * std::f32::consts::TAU);
            c::set_pf(w, id, pv::SWAY, s);
            if ground::passed_frame(w, id, 4.0) || ground::passed_frame(w, id, 10.0) {
                fire(w, id);
            }
            if c::pi32(w, id, pv::T_KIND) != 2 { return; }
            blend(w, id, 1, 0x14);
            w.mm(id).state = 1;
        }
        _ => {}
    }
}

/// `fun_00212f90(m, seq, 0, ticks(n))` unless the sequence already plays.
fn blend(w: &mut World, id: MobyId, seq: u8, n: i32) {
    if w.m(id).anim.seq_b == seq { return; }
    let t = w.ticks(n);
    w.anim_blend(id, seq, 0, t);
}

/// A tread (`CreateMoby(0x14b)`, module table); `k` 0 → +0x138 (mode 0x4000), 1 → +0x13c (mode 0x8000).
fn make_tread(w: &mut World, id: MobyId, k: usize) {
    let Some(t) = w.create_moby(TREAD) else { return };
    let (light, ambient) = w.hero_moby.map_or((0, [0x40; 4]), |h| (w.m(h).light, w.m(h).ambient));
    let pos = w.m(id).position;
    {
        let m = w.mm(t);
        m.update_dist = 0x40;
        m.draw_dist = 0x40;
        m.visible = 1;
        m.light = light;
        m.ambient = ambient;
        m.mode = if k == 0 { 0x4000 } else { 0x8000 };
        m.position = pos;
        m.has_collision = false;
    }
    w.anim_blend(t, 0, 0, 0);
    w.build_matrix(t);
    c::set_pi32(w, id, pv::TREADS + 4 * k, t as i32 + 1);
}

/// `0x2f8050` (module table); false when the tank blew up.
fn prologue(w: &mut World, id: MobyId) -> bool {
    let hit = w.get_hit(id, 0x33_0000, false);
    let res = damage::resolve(w, id, hit, pv::D, 0, 4);
    if res.out5 != 1 && w.m(id).state != 0 {
        let h = c::pf(w, id, pv::D) - res.damage;
        c::set_pf(w, id, pv::D, h);
        if h <= 0.0 {
            blow_up(w, id);
            return false;
        }
        p::set_u8(&mut w.mm(id).pvars, pv::F + 7, 0x78);
        flash::start(w, id, pv::F);
    }
    w.mm(id).hit_slot = 0xff;
    let me = c::pos(w, id);
    let t = target::acquire(w, id, 30.0);
    let (mut tpos, mut tmoby, mut kind) = (t.pos, t.moby, t.kind);
    if kind != 2 {
        if let Some(h) = w.hero_moby.filter(|_| c::dist2(me, hero4(w)) < 10.0) {
            tmoby = Some(h);
            tpos = w.m(h).position;
        } else if let Some(m) = tmoby {
            if 30.0 < c::dist2(me, w.m(m).position) { kind = 2; }
        }
    }
    {
        let pvs = &mut w.mm(id).pvars;
        p::set_v4f(pvs, pv::T_POS, tpos);
        p::set_i32(pvs, pv::T_MOBY, tmoby.map_or(0, |m| m as i32 + 1));
        p::set_u32(pvs, pv::T_KIND, kind);
    }
    let (acc, vmax) = (DT2 * 12.566371, DT * 9.424778);
    let body = w.m(id).rotation[2];
    let (yaw_t, pitch_t) = if kind == 2 {
        (body, 0.0)
    } else {
        let mut d = c::sub(tpos, me);
        d[2] -= 3.0;
        let yaw = c::add_rot(c::atan(d[0], d[1]), c::pf(w, id, pv::SWAY).sin() * 0.08726646);
        let pitch = (-c::atan((d[0] * d[0] + d[1] * d[1]).sqrt(), d[2])).clamp(-std::f32::consts::FRAC_PI_4, 0.34906584);
        (yaw, pitch)
    };
    turn_pvar(w, id, yaw_t, acc, vmax, pv::YAW, pv::YAW_V);
    turn_pvar(w, id, pitch_t, acc, vmax, pv::PITCH, pv::PITCH_V);
    let (yaw, pitch) = (c::pf(w, id, pv::YAW), c::pf(w, id, pv::PITCH));
    let q = sv::quat_mul(sv::axis_quat(Pf::f(pitch), 1), sv::axis_quat(Pf::f(c::sub_rot(yaw, body)), 2));
    manip::set_quat(w, id, id, pv::MANIP, sv::fv(q));
    flash::update(w, id, pv::F);
    true
}

/// Ratchet's feet (0x13f3d0) as a vector.
fn hero4(w: &World) -> c::V { let h = w.hero_point(); [h[0], h[1], h[2], 0.0] }

/// `0x270cc0(target, acc, acc, vmax, &+a, &+v)` on two pvar floats.
fn turn_pvar(w: &mut World, id: MobyId, target: f32, acc: f32, vmax: f32, a: usize, v: usize) {
    let (mut x, mut vel) = (c::pf(w, id, a), c::pf(w, id, v));
    turn::turn_toward(target, acc, acc, vmax, &mut x, &mut vel);
    c::set_pf(w, id, a, x);
    c::set_pf(w, id, v, vel);
}

/// The death of `0x2f8050` (module table).
fn blow_up(w: &mut World, id: MobyId) {
    let me = c::pos(w, id);
    let up = [me[0], me[1], me[2] + 1.0, me[3]];
    let b = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 4.0, flash2: 2.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 20, sparks: 8, puffs: 20, debris: 0, sound: -1, shake: true };
    fx::beam_explosion(w, &b, Some(id), up);
    let rot = w.m(id).rotation;
    for class in PIECES { fx::break_piece(w, id, class, me, rot, 0, 0); }
    crate::moby_update::classes::crate_::set_death_bits(w, id, 0, -1);
    let treads = [link(w, id, pv::TREADS), link(w, id, pv::TREADS + 4)];
    w.delete_moby(id);
    for t in treads.into_iter().flatten() { w.delete_moby(t); }
}

/// `0x2f8658` (module table).
fn drive(w: &mut World, id: MobyId) {
    let path = c::pi32(w, id, pv::PATH);
    let Some(pts) = usize::try_from(path).ok().and_then(|i| w.svc.volumes.paths.get(i)).cloned() else { return };
    if pts.is_empty() { return; }
    let n = pts.len() as i32;
    let k = c::pi32(w, id, pv::NODE).rem_euclid(n) as usize;
    let node = pts[k];
    let me = c::pos(w, id);
    let yaw_t = c::atan(node[0] - me[0], node[1] - me[1]);
    {
        let mut yaw = w.m(id).rotation[2];
        let mut v = c::pf(w, id, pv::BODY_V);
        turn::turn_toward(yaw_t, DT2 * std::f32::consts::PI, DT2 * std::f32::consts::PI, DT * std::f32::consts::TAU, &mut yaw, &mut v);
        w.mm(id).rotation[2] = yaw;
        c::set_pf(w, id, pv::BODY_V, v);
    }
    let mut want = DRIVE * DT;
    if 0.2617994 < c::diff_rots(yaw_t, w.m(id).rotation[2]) { want = DT; }
    let mut speed = c::pf(w, id, pv::SPEED);
    turn::approach(want, DT2, &mut speed);
    c::set_pf(w, id, pv::SPEED, speed);
    let row0 = w.m(id).rows[0];
    let mut mv = c::set_len3(row0, speed);
    walker::move_collide(w, id, 0.5, 0.5, 0.0, &mut mv, 0);
    if c::dist3(c::pos(w, id), [node[0], node[1], node[2], 0.0]) < 1.0 {
        let r = w.rng.randi(n - 1);
        c::set_pi32(w, id, pv::NODE, (k as i32 + r + 1) % n);
    }
    let f = (speed / (TREAD_RATE * DT)).clamp(0.0, 2.0);
    let (rot, pos) = (w.m(id).rotation, w.m(id).position);
    for t in [link(w, id, pv::TREADS), link(w, id, pv::TREADS + 4)].into_iter().flatten() {
        let m = w.mm(t);
        m.anim.speed = f;
        m.rotation = rot;
        m.position = pos;
    }
}

/// State 2's shot (module table).
fn fire(w: &mut World, id: MobyId) {
    let list = if ground::key_time(w, id) < 7.0 { 1 } else { 2 };
    let muzzle = w.joint_point(id, list);
    let k = SHELL_SPEED * DT;
    let (yaw, pitch) = (c::pf(w, id, pv::YAW), c::pf(w, id, pv::PITCH));
    let v = [yaw.cos() * pitch.cos() * k, yaw.sin() * pitch.cos() * k, -pitch.sin() * k, 0.0];
    w.play_sound(0, 0, id);
    fx::muzzle_smoke(w, id, muzzle, None);
    let near = c::dist2(c::pos(w, id), hero4(w)) <= 10.0;
    spawn_shell(w, id, muzzle, v, near);
}

/// `0x2a75f0(m, p, v, homing)` (module table).
pub fn spawn_shell(w: &mut World, owner: MobyId, p: c::V, v: c::V, homing: bool) -> Option<MobyId> {
    let s = w.create_moby(SHELL)?;
    let (light, ambient) = (w.m(owner).light, w.m(owner).ambient);
    let life = w.ticks(0xf0);
    let m = w.mm(s);
    m.update_dist = 0xff;
    m.draw_dist = 0xff;
    m.visible = 1;
    m.state = 0;
    m.scale *= 3.0;
    m.light = light;
    m.ambient = ambient;
    m.position = p;
    if m.pvars.len() < shell::LEN { m.pvars.resize(shell::LEN, 0); }
    p::set_v4f(&mut m.pvars, shell::VEL, v);
    p::set_i16(&mut m.pvars, shell::TIMER, life as i16);
    p::set_i32(&mut m.pvars, shell::OWNER, owner as i32 + 1);
    p::set_i16(&mut m.pvars, shell::HOMING, homing as i16);
    m.rotation[2] = c::atan(v[0], v[1]);
    m.rotation[1] = -c::atan((v[0] * v[0] + v[1] * v[1]).sqrt(), v[2]);
    Some(s)
}

/// Level18 `0x2a7220` (module table).
pub fn shell_update(w: &mut World, id: MobyId) {
    crate::moby_update::story::pvars(w, id, shell::LEN);
    let r = c::add_rot(w.m(id).rotation[0], DT * std::f32::consts::TAU);
    w.mm(id).rotation[0] = r;
    let me = c::pos(w, id);
    let mut v = p::v4f(&w.m(id).pvars, shell::VEL);
    if p::i16(&w.m(id).pvars, shell::HOMING) != 0 {
        let l0 = c::len3(v);
        let aim = sv::fv(w.hero.shadow_point);
        v = c::sub(aim, me);
        let l1 = c::len3(v);
        if l0 < l1 {
            v = c::set_len3(v, l0);
            let m = w.mm(id);
            m.rotation[2] = c::atan(v[0], v[1]);
            m.rotation[1] = -c::atan((v[0] * v[0] + v[1] * v[1]).sqrt(), v[2]);
        } else if l1 < 0.1 {
            w.delete_moby(id);
            return;
        }
        p::set_v4f(&mut w.mm(id).pvars, shell::VEL, v);
    }
    let next = c::add(me, v);
    let owner = link(w, id, shell::OWNER);
    let dir = match owner.filter(|&o| { let m = w.m(o); m.o_class == PUSH_OWNER && m.state != 0xfe && m.state != 0xfd }) {
        Some(o) => {
            let mut d = c::sub(me, w.m(o).position);
            d[2] = 0.0;
            let d = c::set_len3(d, 2.0);
            [d[0], d[1], d[2], f32::from_bits(0x45af_df66)]
        }
        None => { let d = c::set_len3(v, 2.0); [d[0], d[1], d[2], v[3]] }
    };
    let tmpl = HitTemplate { dir: sv::pv(dir), attacker: Some(id), flags: 1, damage: Pf::f(2.0), w20: 1, h1a: w.m(id).o_class as u16, ..Default::default() };
    let hit = sv::line_hit_in(w.table, w.svc, w.classes, w.coll, sv::pv([me[0], me[1], me[2], 1.0]), sv::pv([next[0], next[1], next[2], 1.0]), 0, owner, &tmpl);
    let running = hit.is_none() && c::dec_timer_pvar_s16(w, id, shell::TIMER) == 0;
    if running {
        w.mm(id).position = next;
        super::veldin_shots::trail_for(w, id, v, 10);
        return;
    }
    let b = fx::Beam { damage_r: 1.5, damage: 1.0, flash: 1.0, flash2: 0.5, flash_dist: 9.0, scale: 0.5, light: 0.0, streaks: 10, sparks: 6, puffs: 4, debris: 0, sound: 0, shake: false };
    fx::beam_explosion(w, &b, Some(id), me);
    w.delete_moby(id);
}

/// Level18 `0x2ce7d0` (module table).
pub fn tread_update(w: &mut World, id: MobyId) {
    if id < w.table.first_dynamic {
        let m = w.mm(id);
        m.anim.speed = 0.0;
        m.mode |= mode::NO_ANIM;
    }
}
