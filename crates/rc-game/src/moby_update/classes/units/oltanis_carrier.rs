//! **Oltanis's missile carriers and their missiles, classes 921 and 922** (level14; read from the level14 decomp and
//! disassembly; native `f32`).
//!
//! * **921 the carrier** (`0x2fcbb0`, census U507): it rides the path +0x68 (its chords into the points' w at the
//!   start, `0x2fd808`) at 12 a tick, springing (0.01 / 0.2) its position onto the path point, its yaw and pitch
//!   along the way and its roll to 50° × the turn left over (÷ 1.45°, clamped to ±1; 0.003 / 0.2), its jets
//!   (`0x2fd310`) and hum (sound 0, flags 4) going.
//!   - Already marked dead: its missile (+0x8c) and the missile's target deleted, and itself.
//!   - Start (`+0x68` path, the three missions +0x98..+0xa0 set): with no missile (+0x8c = 0) it flies (state 2,
//!     update distance 0x80); with one, hidden with it (mode | 0x41, no collision) until, with the three missions
//!     done, Ratchet on the ground (no moby under him, 0x13f65c 0) in the cuboid +0xa4 and the missile's target (4
//!     above it) within 28° (yaw) and 20° (pitch) of the camera and in its sight (`CollLine_Fix` 0x12): marked dead,
//!     both shown, the fly-by camera record +0x90 armed (`0x314168`), the target updated and drawn from afar, state 2.
//!   - Carrying: the missile held 0.21 ahead and 1.24 below with its rotation; past segment 0x37 it is let go
//!     (state 2). The path's end, or the fly-by camera gone (camera class ≠ 19): the camera told (`0x314190`), the
//!     hum released and deleted.
//!   - With no missile (+0x8c = −1) a hit of 0x800000 (once): the explosion (`0x273f50(5, 13)`), hidden, the hum
//!     released, state 3 — and on level 14 the third such hit (0x15ee08) gives skill point 0x18 (0x13d420; the
//!     jingle, banner 0x53d6). After `ticks(500)` it comes back at the path's start (state 2).
//! * **922 the missile** (`0x2fdbb8`, U508): placed with a cuboid (+0x00) and a target (+0x04), raised 1.5, waiting
//!   (state 1, its carrier holding it). Let go (state 2) it rolls 2° a tick and homes on the cuboid's centre at 24 a
//!   tick (pitch and yaw `lerp_rot` 0.025 toward it, moving along its row 0) with its jet (`0x2fded0`) and hum (sound
//!   1, flags 4). Within a step: the hum released, the target deleted, the beam explosion at the centre (flashes 10 /
//!   6 beyond 9, scale 1, light 30, 20 streaks, 10 sparks, 12 puffs, 10 debris, sound 0, shake), deleted.
//!
//! * **908 the fighters** (`0x2fc0f0`, U506, 5 placed): 921's flight round the closed path +0x68 from the start
//!   (update and draw distance 0xff), its hum; shot down (any 0x800000 hit; the same level-14 count and skill point)
//!   → back at the path's start after `ticks(500)`. Its jets (`0x2fc3e8`, while drawn): two nozzles 3.75 back, 1.05
//!   up, ±0.35 aside, no smoke, every record's byte 9 = its draw distance / 32 · 16 + 4.
//!
//! **The jets** (`0x2fd310` / `0x2fded0`): at the nozzle (921: two, ∓5.75 back, 0.3 up, ±0.5 aside; 922: one, 0.3
//! back) two flames (type 23: 0.2, 1 / 0.9, 200000, spin ±`randi(16)`, 0x7f204080, `ticks(12)`, fading from 0x7f),
//! 0.25 on three cores (0.05, 1 / 1, 100000 − 20000k, spin ±16, white, `ticks(2) << k`, a random rotation) and,
//! 0.5 back from the nozzle (921: the right one only), two times in three a smoke puff (300000, spin ±`randi(4)`,
//! grey `rand_range(0x40, 0x80)`, `ticks(60)`, half the time blend 0x44, fading from 0x40).
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2fcbb0` / `0x2fd808` / `0x2fd918` / `0x2fe238` | 921 | [`carrier_update`] |
//! | `0x2fdbb8` | 922 | [`missile_update`] |
//! | `0x2fc0f0` / `0x2fc890` / `0x2fc9a0` / `0x2fc3e8` | 908 | [`fighter_update`] |
//! | `0x2fd310` / `0x2fded0` | the jets | [`jet`] |
//!
//! The fly-by camera record (`0x314168` / `0x314190`, camera class 19) through `cinematic::flyby_arm` / `flyby_end`.

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, fx};
use crate::moby_update::services::World;
use crate::moby_update::story;
use crate::particles::rec;
use crate::ps2v::Pf;
use crate::spline::{advance, Cursor};

pub const REFERENCE_LEVEL: u32 = 14;
pub const CARRIER_FN: u32 = 0x2f_cbb0;
pub const CARRIER_CLASSES: [i16; 1] = [921];
pub const MISSILE_FN: u32 = 0x2f_dbb8;
pub const MISSILE_CLASSES: [i16; 1] = [922];
pub const FIGHTER_FN: u32 = 0x2f_c0f0;
pub const FIGHTER_CLASSES: [i16; 1] = [908];

mod o {
    pub const SEG: usize = 0x60;
    pub const T: usize = 0x64;
    pub const PATH: usize = 0x68;
    pub const VOICE: usize = 0x6c;
    pub const VEL: usize = 0x70;
    pub const TIMER: usize = 0x7c;
    pub const ROLL_V: usize = 0x80;
    pub const PITCH_V: usize = 0x84;
    pub const YAW_V: usize = 0x88;
    pub const MISSILE: usize = 0x8c;
    pub const LET_GO: usize = 0x94;
    pub const MISSIONS: usize = 0x98;
    pub const CUBOID: usize = 0xa4;
    pub const SIZE: usize = 0xa8;
}
/// The level-wide count of carriers shot down (0x15ee08).
const SHOT_DOWN: u32 = 0x15_ee08;
const FLAME: u32 = 0x7f20_4080;

fn link(w: &World, i: i32) -> Option<MobyId> { usize::try_from(i).ok().filter(|&m| m < w.table.mobys.len()) }
fn path(w: &World, id: MobyId) -> Option<usize> { usize::try_from(c::pi32(w, id, o::PATH)).ok().filter(|&p| p < w.svc.splines.len()) }
fn points(w: &World, p: usize) -> Vec<[f32; 4]> { w.svc.splines[p].iter().map(|q| q.map(f32::from_bits)).collect() }
fn release(w: &mut World, id: MobyId, off: usize) {
    let v = c::pi32(w, id, off);
    if v != -1 && w.sound_owner(v) == Some(id) { w.release_sound(v, id); }
    c::set_pi32(w, id, off, -1);
}
fn rows(w: &World, id: MobyId) -> [c::V; 3] {
    let r = w.m(id).rows;
    [0, 1, 2].map(|i| [r[i][0], r[i][1], r[i][2], 0.0])
}
fn spin(w: &mut World, n: i32) -> i32 {
    let k = w.rng.randi(n);
    if w.rng.randi(2) == 0 { k } else { -k }
}

/// Two flames at `at` (module doc); `b9`: the record's byte 9 (908's draw-distance nibble).
fn flames(w: &mut World, at: c::V, b9: Option<u8>) {
    for _ in 0..2 {
        let s = spin(w, 0x10);
        let Some(i) = fx::part23_rec(w, [f32::from_bits(0x3e4c_cccd), 1.0, f32::from_bits(0x3f66_6666), 200_000.0], at, s, [0.0; 4], FLAME) else { continue };
        let t = w.ticks(0xc);
        if let Some(r) = fx::rec_mut(w, i) {
            rec::set_i16(r, 0xa, t as i16);
            if let Some(b) = b9 { r[9] = b; }
            rec::set_u32(r, 0x24, 2);
            r[0x2a] = 0x7f;
            r[0x2b] = r[0xa];
        }
    }
}

/// Three cores at `at` (module doc).
fn cores(w: &mut World, at: c::V, b9: Option<u8>) {
    let (mut s, mut life, mut size) = (0x10, w.ticks(2) as i16, 100_000.0f32);
    for _ in 0..3 {
        if let Some(i) = fx::part23_rec(w, [f32::from_bits(0x3d4c_cccd), 1.0, 1.0, size], at, s, [0.0; 4], 0x7fff_ffff) {
            let rot = w.rng.randi(0xff) as u8;
            if let Some(r) = fx::rec_mut(w, i) {
                rec::set_i16(r, 0xa, life);
                if let Some(b) = b9 { r[9] = b; }
                r[8] = rot;
                rec::set_u32(r, 0x24, 2);
                r[0x2a] = 0x7f;
                r[0x2b] = r[0xa];
            }
        }
        s = -s;
        life <<= 1;
        size -= 20000.0;
    }
}

/// The smoke puff at `at`, two times in three (module doc).
fn smoke(w: &mut World, at: c::V) {
    let g = w.rng.rand_range(0x40, 0x80) as u32;
    if w.rng.randi(3) == 0 { return; }
    let s = spin(w, 4);
    let Some(i) = fx::part23_rec(w, [f32::from_bits(0x3d4c_cccd), 1.0, 1.0, 300_000.0], at, s, [0.0; 4], g | g << 16 | g << 8 | 0x7f00_0000) else { return };
    let t = w.ticks(0x3c);
    let blend = w.rng.randi(2) != 0;
    if let Some(r) = fx::rec_mut(w, i) {
        rec::set_i16(r, 0xa, t as i16);
        if blend { r[3] = 0x44; }
        r[0x2a] = 0x40;
        rec::set_u32(r, 0x24, 2);
        r[0x2b] = t as u8;
    }
}

/// `0x2fd310` (921: `side` 0.5) / `0x2fded0` (922: none) / `0x2fc3e8` (908: no smoke, `b9`): the jets (module doc).
pub fn jet(w: &mut World, id: MobyId, back: f32, up: f32, side: Option<f32>, smoky: bool, b9: Option<u8>) {
    let [r0, r1, r2] = rows(w, id);
    let nozzle = c::add(c::add(c::pos(w, id), c::scale(r0, back)), c::scale(r2, up));
    let a = c::add(nozzle, c::scale(r1, side.unwrap_or(0.0)));
    flames(w, a, b9);
    cores(w, c::add(a, c::scale(r0, 0.25)), b9);
    if smoky { smoke(w, c::add(nozzle, c::scale(r0, -0.5))); }
    if let Some(k) = side {
        let b = c::sub(nozzle, c::scale(r1, k));
        flames(w, b, b9);
        cores(w, c::add(b, c::scale(r0, 0.25)), b9);
    }
}

/// `0x2fd808`: the path's chords, the cursor and springs cleared, at the path's start facing along it.
fn to_start(w: &mut World, id: MobyId) {
    let Some(p) = path(w, id) else { return };
    let n = w.svc.splines[p].len();
    for i in 0..n.saturating_sub(1) {
        let pts = points(w, p);
        w.svc.splines[p][i][3] = c::dist3(pts[i], pts[i + 1]).to_bits();
    }
    c::set_pi32(w, id, o::SEG, 0);
    c::set_pf(w, id, o::T, 0.0);
    restart(w, id, p);
}

fn restart(w: &mut World, id: MobyId, p: usize) {
    let pts = points(w, p);
    let (p0, p1) = (pts.first().copied().unwrap_or([0.0; 4]), pts.get(1).copied().unwrap_or([0.0; 4]));
    let m = w.mm(id);
    m.position = p0;
    m.rotation[2] = c::atan(p1[0] - p0[0], p1[1] - p0[1]);
    for k in [o::ROLL_V, o::VEL + 8, o::VEL + 4, o::VEL, o::YAW_V, o::PITCH_V] { c::set_pf(w, id, k, 0.0); }
}

/// `0x2fd918`: along the path; true when the carrier was deleted (module doc).
fn follow(w: &mut World, id: MobyId) -> bool {
    let Some(p) = path(w, id) else { return false };
    let missile = c::pi32(w, id, o::MISSILE);
    let pts = points(w, p);
    let mut cur = Cursor { seg: c::pi32(w, id, o::SEG), t: c::pf(w, id, o::T) };
    let (q, ended) = advance(&pts, missile < 0, 12.0 * c::DT, &mut cur);
    c::set_pi32(w, id, o::SEG, cur.seg);
    c::set_pf(w, id, o::T, cur.t);
    if missile != 0 && (ended || w.camera_class != 0x13) {
        // 0x314190: the fly-by record +0x90 ended while it is current (camera class 19).
        let rec = c::pi32(w, id, 0x90);
        crate::cinematic::flyby_end(w, rec);
        release(w, id, o::VOICE);
        w.delete_moby(id);
        return true;
    }
    steer(w, id, q);
    false
}

/// The springs onto the path point `q` (position, yaw, pitch, roll; module doc).
fn steer(w: &mut World, id: MobyId, q: [f32; 3]) {
    let (k, d) = (Pf::f(0.01), Pf::f(0.2));
    for (a, &t) in q.iter().enumerate() {
        let (mut x, mut v) = (Pf::f(w.m(id).position[a]), Pf::f(c::pf(w, id, o::VEL + 4 * a)));
        crate::hero::physics::spring(Pf::f(t), k, d, Pf::ZERO, &mut x, &mut v);
        w.mm(id).position[a] = x.to_f32();
        c::set_pf(w, id, o::VEL + 4 * a, v.to_f32());
    }
    let pos = c::pos(w, id);
    let e = [q[0] - pos[0], q[1] - pos[1], q[2] - pos[2], 0.0];
    if 0.01 < e[0].abs() && 0.01 < e[1].abs() {
        let yaw = c::atan(e[0], e[1]);
        let turn = c::sub_rot(w.m(id).rotation[2], yaw);
        c::turn::spring_turn2_pvar(w, id, yaw, 0.01, 0.2, 0.0, o::YAW_V);
        let pitch = -c::atan(c::len2(e), e[2]);
        for (axis, t, kk, vo) in [(1, pitch, 0.01f32, o::PITCH_V), (0, (turn / f32::from_bits(0x3ccf_5134)).clamp(-1.0, 1.0) * f32::from_bits(0x3f5f_66f3), f32::from_bits(0x3b44_9ba6), o::ROLL_V)] {
            let (mut x, mut v) = (Pf::f(w.m(id).rotation[axis]), Pf::f(c::pf(w, id, vo)));
            crate::hero::physics::spring(Pf::f(t), Pf::f(kk), d, Pf::ZERO, &mut x, &mut v);
            w.mm(id).rotation[axis] = x.to_f32();
            c::set_pf(w, id, vo, v.to_f32());
        }
    }
}

fn dead(w: &World, id: MobyId) -> bool {
    let sid = w.m(id).spawn_id;
    w.svc.save.collected.get(&sid).is_some_and(|&b| b != 0) || w.svc.save.death.contains(&(w.svc.level, sid))
}

fn set_hidden(w: &mut World, m: MobyId, hidden: bool) {
    let coll = super::class_collision(w, w.m(m).o_class);
    let mm = w.mm(m);
    if hidden {
        mm.mode |= 0x41;
        mm.has_collision = false;
    } else {
        mm.mode &= 0xffbe;
        mm.has_collision = coll;
    }
}

/// Level14 `0x2fcbb0`: 921 (module doc).
pub fn carrier_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, o::SIZE);
    let missile = link(w, c::pi32(w, id, o::MISSILE));
    if w.m(id).state == 0 && dead(w, id) {
        // 0x2fe238: the missile's target, then the missile.
        if let Some(mi) = missile {
            if let Some(t) = w.m(mi).pvars.get(4..8).and_then(|b| link(w, i32::from_le_bytes([b[0], b[1], b[2], b[3]]))) {
                if (w.m(t).state as i8) >= 0 { w.delete_moby(t); }
            }
            if (w.m(mi).state as i8) >= 0 { w.delete_moby(mi); }
        }
        w.delete_moby(id);
        return;
    }
    let hit = w.get_hit(id, 0x80_0000, false).is_some();
    w.mm(id).hit_slot = 0xff;
    if hit && c::pi32(w, id, o::MISSILE) == -1 && w.m(id).state != 3 { shot_down(w, id, 3); }
    let st = w.m(id).state;
    match st {
        0 => {
            let ok = path(w, id).is_some() && (0..3).all(|k| -1 < c::pi32(w, id, o::MISSIONS + 4 * k));
            if !ok {
                w.delete_moby(id);
                return;
            }
            to_start(w, id);
            if c::pi32(w, id, o::MISSILE) == 0 {
                let m = w.mm(id);
                m.state = 2;
                m.update_dist = 0x80;
            } else {
                w.mm(id).state = 1;
                w.mm(id).update_dist = 0xff;
                set_hidden(w, id, true);
                if let Some(mi) = missile { w.mm(mi).mode |= 0x41; }
                c::set_pi32(w, id, o::LET_GO, 0);
            }
            w.mm(id).draw_dist = 0x80;
            c::set_pi32(w, id, o::VOICE, -1);
            let t = w.ticks(500);
            c::set_pi32(w, id, o::TIMER, t);
        }
        1 => arm(w, id, missile),
        2 => {
            if follow(w, id) { return; }
            let lg = c::pi32(w, id, o::LET_GO);
            if c::pi32(w, id, o::MISSILE) != 0 && lg < 2 {
                if lg == 0 && 0x37 < c::pi32(w, id, o::SEG) { c::set_pi32(w, id, o::LET_GO, 1); }
                let [r0, _, r2] = rows(w, id);
                let at = c::add(c::pos(w, id), c::add(c::scale(r0, f32::from_bits(0x3e57_0a3d)), c::scale(r2, f32::from_bits(0xbf9e_b852))));
                if let Some(mi) = missile {
                    let rot = w.m(id).rotation;
                    let mm = w.mm(mi);
                    mm.position = at;
                    mm.rotation = rot;
                    if c::pi32(w, id, o::LET_GO) == 1 {
                        w.mm(mi).state = 2;
                        c::set_pi32(w, id, o::LET_GO, 2);
                    }
                }
            }
        }
        3 if c::dec_timer_pvar_i32(w, id, o::TIMER) != 0 => {
            set_hidden(w, id, false);
            let t = w.ticks(500);
            c::set_pi32(w, id, o::TIMER, t);
            w.mm(id).state = 2;
            c::set_pi32(w, id, o::SEG, 0);
            c::set_pf(w, id, o::T, 0.0);
            if let Some(p) = path(w, id) { restart(w, id, p); }
        }
        _ => {}
    }
    if w.m(id).state == 2 {
        jet(w, id, -5.75, 0.3, Some(0.5), true, None);
        if !w.sound_alive(c::pi32(w, id, o::VOICE), id) {
            let v = w.play_sound(0, 4, id);
            c::set_pi32(w, id, o::VOICE, v);
        }
    }
}

/// Shot down (921 / 908): on level 14 the third gives skill point 0x18; the explosion, hidden, the hum released,
/// state `st`.
fn shot_down(w: &mut World, id: MobyId, st: u8) {
    if w.svc.level == 14 {
        let n = w.svc.units.word(SHOT_DOWN) as i32 + 1;
        w.svc.units.set_word(SHOT_DOWN, n as u32);
        if 2 < n && !story::skill_point(w, 0x18) {
            story::set_skill_point(w, 0x18);
            w.play_level_sound(story::SKILL_SOUND, 0, None);
            crate::cinematic::show_banner(w, story::SKILL_BANNER, -1);
        }
    }
    w.mm(id).state = st;
    let p = c::pos(w, id);
    fx::death_explosion(w, 5.0, 13.0, Some(id), p, -1);
    set_hidden(w, id, true);
    release(w, id, o::VOICE);
}

/// Level14 `0x2fc0f0`: 908 (module doc).
pub fn fighter_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, o::SIZE);
    let hit = w.get_hit(id, 0x80_0000, false).is_some();
    w.mm(id).hit_slot = 0xff;
    if hit && w.m(id).state != 2 { shot_down(w, id, 2); }
    let st = w.m(id).state;
    match st {
        0 => {
            if path(w, id).is_none_or(|p| w.svc.splines[p].is_empty()) {
                w.delete_moby(id);
                return;
            }
            to_start(w, id);
            let m = w.mm(id);
            m.state = 1;
            m.update_dist = 0xff;
            m.draw_dist = 0xff;
            c::set_pi32(w, id, o::VOICE, -1);
            let t = w.ticks(500);
            c::set_pi32(w, id, o::TIMER, t);
        }
        1 => {
            // 0x2fc9a0: the closed loop.
            if let Some(p) = path(w, id) {
                let pts = points(w, p);
                let mut cur = Cursor { seg: c::pi32(w, id, o::SEG), t: c::pf(w, id, o::T) };
                let (q, _) = advance(&pts, true, 12.0 * c::DT, &mut cur);
                c::set_pi32(w, id, o::SEG, cur.seg);
                c::set_pf(w, id, o::T, cur.t);
                steer(w, id, q);
            }
        }
        2 if c::dec_timer_pvar_i32(w, id, o::TIMER) != 0 => {
            set_hidden(w, id, false);
            let t = w.ticks(500);
            c::set_pi32(w, id, o::TIMER, t);
            w.mm(id).state = 1;
            c::set_pi32(w, id, o::SEG, 0);
            c::set_pf(w, id, o::T, 0.0);
            if let Some(p) = path(w, id) { restart(w, id, p); }
        }
        _ => {}
    }
    let st = w.m(id).state;
    if st != 0 && st != 2 && w.m(id).visible != 0 {
        let b9 = (((w.m(id).draw_dist >> 5) << 4) as u8).wrapping_add(4);
        jet(w, id, -3.75, 1.05, Some(0.35), false, Some(b9));
    }
    if st != 0 && st != 2 && !w.sound_alive(c::pi32(w, id, o::VOICE), id) {
        let v = w.play_sound(0, 4, id);
        c::set_pi32(w, id, o::VOICE, v);
    }
}

/// State 1: the launch's conditions (module doc).
fn arm(w: &mut World, id: MobyId, missile: Option<MobyId>) {
    if !(0..3).all(|k| story::mission_done(w, c::pi32(w, id, o::MISSIONS + 4 * k))) { return; }
    if w.hero.f65c != 0 || w.hero.ground_moby.is_some() { return; }
    if !w.in_cuboid(w.hero_point(), c::pi32(w, id, o::CUBOID)) { return; }
    let Some(mi) = missile else { return };
    let Some(t) = w.m(mi).pvars.get(4..8).and_then(|b| link(w, i32::from_le_bytes([b[0], b[1], b[2], b[3]]))) else { return };
    let tp = c::pos(w, t);
    let aim = [tp[0], tp[1], tp[2] + 4.0, tp[3]];
    let cam = w.camera.map(|x| x.to_f32());
    let pitch = c::diff_rots(w.hero.loop_in.cam_euler[1], -c::atan(c::dist2(cam, aim), aim[2] - cam[2]));
    let yaw = c::diff_rots(w.camera_yaw, c::atan(aim[0] - cam[0], aim[1] - cam[1]));
    if !(yaw < 0.488_692_2 && pitch < 0.349_065_84) { return; }
    if w.coll_line(w.camera, crate::moby_update::services::pv(aim), 0x12, Some(t)).is_some() { return; }
    story::death_bits(w, id);
    set_hidden(w, id, false);
    w.mm(mi).mode &= 0xffbe;
    w.mm(id).state = 2;
    // 0x314168: the fly-by record +0x90 armed (camera class 19).
    let rec = c::pi32(w, id, 0x90);
    crate::cinematic::flyby_arm(w, rec);
    let tm = w.mm(t);
    tm.draw_dist = 0xff;
    tm.update_dist = 0xff;
}

/// Level14 `0x2fdbb8`: 922 (module doc).
pub fn missile_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x10);
    match w.m(id).state {
        0 => {
            if c::pi32(w, id, 0) < 0 || c::pi32(w, id, 4) < 0 {
                w.delete_moby(id);
                return;
            }
            let m = w.mm(id);
            m.state = 1;
            m.update_dist = 0xff;
            m.position[2] += 1.5;
            c::set_pi32(w, id, 8, -1);
            let t = w.ticks(0x1e);
            c::set_pi32(w, id, 0xc, t);
        }
        2 => {
            let centre = w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, c::pi32(w, id, 0)).map(|s| s.centre()).unwrap_or([0.0; 3]);
            let goal = [centre[0], centre[1], centre[2], 1.0];
            let v = c::sub(goal, c::pos(w, id));
            let d = c::len3(v);
            c::dec_timer_pvar_i32(w, id, 0xc);
            let step = 24.0 * c::DT;
            if step <= d && 0.0 < d {
                let m = w.mm(id);
                m.rotation[0] = c::add_rot(m.rotation[0], f32::from_bits(0x3d0e_fa35));
                let pitch = c::atan(c::len2(v), v[2]);
                let r1 = c::lerp_rot(w.m(id).rotation[1], -pitch, 0.025);
                let r2 = c::lerp_rot(w.m(id).rotation[2], c::atan(v[0], v[1]), 0.025);
                let m = w.mm(id);
                m.rotation[1] = r1;
                m.rotation[2] = r2;
                let r0 = w.m(id).rows[0];
                let p = c::add(c::pos(w, id), c::scale([r0[0], r0[1], r0[2], 0.0], step));
                w.mm(id).position = p;
                jet(w, id, -0.3, 0.0, None, true, None);
                if !w.sound_alive(c::pi32(w, id, 8), id) {
                    let v = w.play_sound(1, 4, id);
                    c::set_pi32(w, id, 8, v);
                }
                return;
            }
            release(w, id, 8);
            if let Some(t) = link(w, c::pi32(w, id, 4)) { w.delete_moby(t); }
            w.mm(id).position = goal;
            let b = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 10.0, flash2: 6.0, flash_dist: 9.0, scale: 1.0, light: 30.0, streaks: 0x14, sparks: 10, puffs: 0xc, debris: 10, sound: 0, shake: true };
            fx::beam_explosion(w, &b, Some(id), goal);
            w.delete_moby(id);
        }
        _ => {}
    }
}
