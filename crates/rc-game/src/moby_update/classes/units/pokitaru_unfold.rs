//! **Pokitaru's unfolding machine, class 1156** (level11 `0x30c788` with its pose `0x30d1b0`, 1 placed; census U384).
//! The name is descriptive [L]. A hidden machine of 18 pieces it creates (six 0x480, four 0x481 with four 0x482 hinged
//! on them, four 0x483 platforms) that unfolds in a cutaway once its floor button (pvar+0x00, a 1179:
//! [`super::pokitaru_button`]) is pressed: the screen fades to black, Ratchet is put on the ground by the button facing
//! the machine and held (state 0x72), the script camera cuts to a cuboid's view and glides to a second one while the
//! pieces fold out in six staged springs (two clicks) and the platforms slide apart; then a fade hands Ratchet back
//! (state 0) and flag 86 is set. With flag 86 set any pressed button ends a later showing early. Sibling of the
//! cutaway machine 1157 ([`super::pokitaru_cutaway`]). Read from the level11 decomp and the words gp−0x4d54..−0x4cf8.
//! Native `f32`.
//!
//! **Pvars**: +0x00 the button's moby index, +0x04 / +0x08 the cuboids of the camera's start and end view, +0x0c the
//! fade t. **Level globals** (one machine; kept as the unit words at their addresses, `Globals::word`): the pieces
//! 0x1da710[6] (0x480), 0x161f20[4] (0x481), 0x161f30[4] (0x482), 0x161f10[4] (0x483); the stages p0..p5 0x161ec4..
//! 0x161ed8, the camera's glide g 0x161edc and their spring velocities 0x161ee0..0x161ef8.
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | hidden (+0x31 = 0, collision 0, mode \|= 1); the stages and g = 0, +0xbc = 0; the 18 pieces: `CreateMoby`, draw distance 0x40, the hero moby's light word, the machine's position and Euler, mode \|= 1, not drawn, no collision; the platforms also mode \|= 0x20, pvar+0x08 = the block (+0x20), its flags +0x5c \|= 1; the pose; → 1 | [`update`] |
//! | state 1 | the button (pvar+0x00 ≠ −1) is a 1179 in state 2 (`0x30f270`) → 2 | [`update`] |
//! | state 2 | `Approach(1, 4·dt, &t)`; fade 0x15f3fc = t; t ≥ 1 → 3: p = the button + (0, 0, 5), z = `GroundHeight(0.5, p, 0)`; `HeroTeleport(p, (0, 0, FastArcTan(machine − p)), 0x72, 1)`; `CameraScript(cuboid +0x04 centre, cuboid +0x08 Euler, 1, 0, 0)`; the pieces shown (mode &= ~1, drawn, the class collision); `PlayClassSound(0, 0)` | [`update`] |
//! | state 3 | the stages: p0 < 1 → `0x281810(1, dt², dt², dt)`; p0 > 0.9 and p1 = 0 → `PlayClassSound(1, 0)`; p0 > 0.9 and p1 < 1 → p1 (2dt², 2dt², 2dt); p1 > 0.5 and p2 = 0 → the click; p1 > 0.5 and p2 < 1 → p2; p1 ≥ 1 and p3 < 1 → p3; p2 ≥ 1 and p4 < 1 → p4; p4 > 0.9 and p5 < 1 → p5 (2dt², 4dt², 2dt); the pose | [`update`] (`turn::spring`) |
//! | | flag 86 (0x13d3de) and any button pressed (0x13cae4) → +0xbc = 1; p5 ≥ 1 or +0xbc → `Approach(1, 4·dt, &t)`, else `Approach(0, 4·dt, &t)`; fade = t; g on the spring (1, 0.2dt², 0.2dt², 0.2dt); the camera at `lerp(cuboid +0x04, cuboid +0x08, g)` (centres and Eulers, `fun_001f9a40`); t ≥ 1 → every stage 1, the pose, → 4, `CameraScript2(3)`, `HeroTeleport(0x13f3d0, 0x13f3e0, 0, 1)`, flag 86 = 1 | [`update`] (`cinematic::camera_targets`) |
//! | state 4 | `Approach(0, 4·dt, &t)`; fade = t; t = 0 → 5 | [`update`] |
//! | `0x30d1b0` | the pose ([`pose`]) | [`pose`] |
//!
//! **The pose** `0x30d1b0`, f = 90·(1 − p0), o = the machine's row 0 at length 14 with z − 14, plus its position:
//! * 0x480 k (0..5), side s = +1 for k > 2 else −1: the machine's Euler with yaw + s·π/2, pitch (y) 30°, roll (x)
//!   (k mod 3)·30° − (f + 30)°; position = its own row 2 at 22 + the machine's row 1 at −6s + o;
//! * 0x481 u (0..3), s = +1 for u > 1, e = +1 for even u: the same with roll (u mod 2)·30° − (f + 15)°, then roll +=
//!   `sub_rot(e·15°, 80°)`·(p1 for e = −1, else p2) + 80°;
//! * 0x482 u on 0x481 u: position = that one's + its row 2 at −5.32 + its row 1 at −0.89, its Euler with pitch +
//!   240°·(p3 for odd u, else p4);
//! * 0x483 u, s = +1 for u > 1: position = the machine's + its row 0 at s·min((u mod 2)·7.5 + 3.75 + 13.25·(1 − p5),
//!   20) + 14 with z −0.2, the machine's Euler.
//!
//! Each piece then `MobyBuildMatrix`.

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, turn, DT, DT2};
use crate::moby_update::services::World;

pub const REFERENCE_LEVEL: u32 = 11;
pub const UPDATE_FN: u32 = 0x30_c788;
pub const CLASSES: [i16; 1] = [1156];
/// The pieces' classes and their tables.
pub const PIECES: [(i16, u32, usize); 4] = [(0x480, 0x1d_a710, 6), (0x481, 0x16_1f20, 4), (0x482, 0x16_1f30, 4), (0x483, 0x16_1f10, 4)];
/// The button class (0x49b) and its pressed state.
pub const BUTTON: i16 = 1179;
/// The flag of the first showing (0x13d3de).
pub const SHOWN: usize = 86;
/// Ratchet's state while it runs.
pub const HELD: i32 = 0x72;
/// The stages p0..p5 and the glide g (0x161ec4..), their velocities (0x161ee0..).
const STAGE: u32 = 0x16_1ec4;
const GLIDE: u32 = 0x16_1edc;
const STAGE_VEL: u32 = 0x16_1ee0;
const DEG: f32 = 0.017_453_292;
const RIGHT: f32 = 1.570_796_4;
const THIRTY: f32 = std::f32::consts::FRAC_PI_6;
const FIFTEEN: f32 = 0.261_799_4;
/// 0x3fb2b8c2 = 80°.
const EIGHTY: f32 = f32::from_bits(0x3fb2_b8c2);
/// gp−0x4d54 / −0x4d50 / −0x4d4c / −0x4d48 / −0x4d44 (0x161eac..): −14, 22, −6, 90, 30.
const DROP: f32 = -14.0;
const ARM: f32 = 22.0;
const SIDE: f32 = -6.0;
const FOLD: f32 = 90.0;
const PITCH: f32 = 30.0;
/// gp−0x4d04 / −0x4d00 / −0x4cfc / −0x4cf8 (0x161efc..): −5.32, −0.89, −240, −0.2.
const HINGE_A: f32 = -5.32;
const HINGE_B: f32 = -0.89;
const SWING: f32 = -240.0;
const PLATFORM_Z: f32 = -0.2;

/// Pvar offsets (module doc).
pub mod pv {
    pub const BUTTON: usize = 0x00;
    pub const CUBOID_A: usize = 0x04;
    pub const CUBOID_B: usize = 0x08;
    pub const T: usize = 0x0c;
}

fn gf(w: &World, addr: u32) -> f32 { f32::from_bits(w.svc.units.word(addr)) }
fn sf(w: &mut World, addr: u32, v: f32) { w.svc.units.set_word(addr, v.to_bits()); }
fn stage(w: &World, k: u32) -> f32 { gf(w, STAGE + 4 * k) }

/// Piece `k` of table `t` (an index into the table; None: not created).
fn piece(w: &World, t: usize, k: usize) -> Option<MobyId> {
    let i = w.svc.units.word(PIECES[t].1 + 4 * k as u32) as i32;
    usize::try_from(i).ok().filter(|&m| m < w.table.mobys.len())
}

/// A level cuboid's centre (+0x30, four lanes) and Euler (+0x70).
fn cuboid(w: &World, i: i32) -> Option<([f32; 4], [f32; 3])> {
    let s = w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, i)?;
    Some((s.matrix[3], s.euler))
}

fn spring_stage(w: &mut World, k: u32, accel: f32, decel: f32, vmax: f32) {
    let (mut x, mut v) = (stage(w, k), gf(w, STAGE_VEL + 4 * k));
    turn::spring(1.0, accel, decel, vmax, &mut x, &mut v);
    sf(w, STAGE + 4 * k, x);
    sf(w, STAGE_VEL + 4 * k, v);
}

fn fade_toward(w: &mut World, id: MobyId, target: f32) -> f32 {
    let mut t = c::pf(w, id, pv::T);
    turn::approach(target, DT * 4.0, &mut t);
    c::set_pf(w, id, pv::T, t);
    crate::cinematic::set_fade(w, t);
    t
}

fn rows_of(e: [f32; 4]) -> [[f32; 4]; 3] {
    let r = rc_formats::moby_light::rotation_rows([e[0], e[1], e[2]]);
    [0, 1, 2].map(|k| r[k].map(f32::from_bits))
}

/// Level11 `0x30d1b0` (module doc).
pub fn pose(w: &mut World, id: MobyId) {
    let me = w.m(id);
    let (pos, rot, row0, row1) = (me.position, me.rotation, me.rows[0], me.rows[1]);
    let f = FOLD * (1.0 - stage(w, 0));
    let mut o = c::set_len3(row0, 14.0);
    o[2] += DROP;
    let o = c::add(o, pos);
    let arm = |e: [f32; 4], s: f32| {
        let a = c::set_len3(rows_of(e)[2], ARM);
        let b = c::set_len3(row1, s * SIDE);
        c::add(c::add(a, b), o)
    };
    for k in 0..6 {
        let Some(p) = piece(w, 0, k) else { continue };
        let s = if 2 < k { 1.0 } else { -1.0 };
        let mut e = rot;
        e[2] = c::add_rot(e[2], s * RIGHT);
        e[1] = PITCH * DEG;
        e[0] = (k % 3) as f32 * THIRTY - (f + 30.0) * DEG;
        let at = arm(e, s);
        let m = w.mm(p);
        m.rotation = e;
        m.position = at;
        w.build_matrix(p);
    }
    let (p1, p2) = (stage(w, 1), stage(w, 2));
    for u in 0..4 {
        let Some(p) = piece(w, 1, u) else { continue };
        let even = if u & 1 == 0 { 1.0 } else { -1.0 };
        let s = if 1 < u { 1.0 } else { -1.0 };
        let mut e = rot;
        e[2] = c::add_rot(e[2], s * RIGHT);
        e[1] = PITCH * DEG;
        e[0] = (u % 2) as f32 * THIRTY - (f + 15.0) * DEG;
        let at = arm(e, s);
        let r = c::sub_rot(even * FIFTEEN, EIGHTY);
        let q = if even == -1.0 { p1 } else { p2 };
        e[0] = c::add_rot(e[0], c::add_rot(r * q, EIGHTY));
        let m = w.mm(p);
        m.position = at;
        m.rotation = e;
        w.build_matrix(p);
    }
    let (p3, p4) = (stage(w, 3), stage(w, 4));
    for u in 0..4 {
        let (Some(b), Some(p)) = (piece(w, 1, u), piece(w, 2, u)) else { continue };
        let bm = w.m(b);
        let off = c::add(c::set_len3(bm.rows[2], HINGE_A), c::set_len3(bm.rows[1], HINGE_B));
        let (at, mut e) = (c::add(bm.position, off), bm.rotation);
        let q = if u & 1 != 0 { p3 } else { p4 };
        e[1] = c::add_rot(e[1], -(SWING * DEG) * q);
        let m = w.mm(p);
        m.position = at;
        m.rotation = e;
        w.build_matrix(p);
    }
    let p5 = stage(w, 5);
    for u in 0..4 {
        let Some(p) = piece(w, 3, u) else { continue };
        let s = if 1 < u { 1.0 } else { -1.0 };
        let mut g = (u & 1) as f32 * 7.5 + 3.75 + (1.0 - p5) * 13.25;
        if 20.0 < g { g = 20.0; }
        let mut d = c::set_len3(row0, g * s + 14.0);
        d[2] = PLATFORM_Z;
        let m = w.mm(p);
        m.position = c::add(d, pos);
        m.rotation = rot;
        w.build_matrix(p);
    }
}

fn show_pieces(w: &mut World) {
    for (t, &(_, _, n)) in PIECES.iter().enumerate() {
        for k in 0..n {
            let Some(p) = piece(w, t, k) else { continue };
            let coll = super::class_collision(w, w.m(p).o_class);
            let m = w.mm(p);
            m.mode &= !1;
            m.visible = 1;
            m.has_collision = coll;
        }
    }
}

/// Level11 `0x30c788` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x10 { return; }
    let state = w.m(id).state;
    match state {
        0 => {
            let m = w.mm(id);
            m.visible = 0;
            m.has_collision = false;
            m.mode |= 1;
            m.cmd = 0;
            for k in 0..6 { sf(w, STAGE + 4 * k, 0.0); }
            sf(w, GLIDE, 0.0);
            let (pos, rot) = (w.m(id).position, w.m(id).rotation);
            let light = w.hero_moby.map(|h| (w.m(h).light, w.m(h).ambient));
            for (t, &(class, table, n)) in PIECES.iter().enumerate() {
                for k in 0..n {
                    let p = w.create_moby(class);
                    w.svc.units.set_word(table + 4 * k as u32, p.map_or(-1, |p| p as i32) as u32);
                    let Some(p) = p else { continue };
                    let m = w.mm(p);
                    m.draw_dist = 0x40;
                    if let Some((l, a)) = light { m.light = l; m.ambient = a; }
                    m.position = pos;
                    m.rotation = rot;
                    m.mode |= 1;
                    if t == 3 {
                        m.mode |= 0x20;
                        if m.pvars.len() < 0x60 { m.pvars.resize(0x60, 0); }
                        c::set_pi32(w, p, 8, 0x20);
                        let f = c::pi32(w, p, 0x5c) | 1;
                        c::set_pi32(w, p, 0x5c, f);
                    }
                    let m = w.mm(p);
                    m.visible = 0;
                    m.has_collision = false;
                }
            }
            pose(w, id);
            w.mm(id).state = 1;
        }
        1 => {
            let b = c::pi32(w, id, pv::BUTTON);
            let pressed = usize::try_from(b).ok().and_then(|i| w.table.mobys.get(i)).is_some_and(|m| m.o_class == BUTTON && m.state == 2);
            if b != -1 && pressed { w.mm(id).state = 2; }
        }
        2 => {
            if fade_toward(w, id, 1.0) < 1.0 { return; }
            w.mm(id).state = 3;
            if let Some(b) = usize::try_from(c::pi32(w, id, pv::BUTTON)).ok().filter(|&b| b < w.table.mobys.len()) {
                let bp = w.m(b).position;
                let mut p = [bp[0], bp[1], bp[2] + 5.0, bp[3]];
                p[2] = crate::moby_update::services::fl(w.ground_height(crate::moby_update::services::pf(0.5), crate::moby_update::services::pv(p), 0));
                let me = w.m(id).position;
                let yaw = c::atan(me[0] - p[0], me[1] - p[1]);
                crate::cinematic::hero_teleport(w, [p[0], p[1], p[2]], [0.0, 0.0, yaw], HELD, true);
            }
            let (a, b) = (cuboid(w, c::pi32(w, id, pv::CUBOID_A)), cuboid(w, c::pi32(w, id, pv::CUBOID_B)));
            if let (Some((pa, _)), Some((_, eb))) = (a, b) {
                crate::cinematic::camera_script(w, [pa[0], pa[1], pa[2]], eb, 1, 0, false);
            }
            show_pieces(w);
            w.play_sound(0, 0, id);
        }
        3 => {
            if stage(w, 0) < 1.0 { spring_stage(w, 0, DT2, DT2, DT); }
            let (two2, two) = (DT2 + DT2, DT + DT);
            if 0.9 < stage(w, 0) && stage(w, 1) == 0.0 { w.play_sound(1, 0, id); }
            if 0.9 < stage(w, 0) && stage(w, 1) < 1.0 { spring_stage(w, 1, two2, two2, two); }
            if 0.5 < stage(w, 1) && stage(w, 2) == 0.0 { w.play_sound(1, 0, id); }
            if 0.5 < stage(w, 1) && stage(w, 2) < 1.0 { spring_stage(w, 2, two2, two2, two); }
            if 1.0 <= stage(w, 1) && stage(w, 3) < 1.0 { spring_stage(w, 3, two2, two2, two); }
            if 1.0 <= stage(w, 2) && stage(w, 4) < 1.0 { spring_stage(w, 4, two2, two2, two); }
            if 0.9 < stage(w, 4) && stage(w, 5) < 1.0 { spring_stage(w, 5, two2, DT2 * 4.0, two); }
            pose(w, id);
            if super::hints::flag(w, SHOWN) && w.hero.loop_in.pad.pressed != 0 { w.mm(id).cmd = 1; }
            let target = if 1.0 <= stage(w, 5) || w.m(id).cmd != 0 { 1.0 } else { 0.0 };
            let t = fade_toward(w, id, target);
            let (mut g, mut v) = (gf(w, GLIDE), gf(w, STAGE_VEL + 4 * 6));
            turn::spring(1.0, DT2 * 0.2, DT2 * 0.2, DT * 0.2, &mut g, &mut v);
            sf(w, GLIDE, g);
            sf(w, STAGE_VEL + 4 * 6, v);
            let (a, b) = (cuboid(w, c::pi32(w, id, pv::CUBOID_A)), cuboid(w, c::pi32(w, id, pv::CUBOID_B)));
            if let (Some((pa, ea)), Some((pb, eb))) = (a, b) {
                let p: [f32; 4] = std::array::from_fn(|k| pa[k] + (pb[k] - pa[k]) * g);
                let e: [f32; 3] = std::array::from_fn(|k| ea[k] + (eb[k] - ea[k]) * g);
                crate::cinematic::camera_targets(w, Some([p[0], p[1], p[2]]), Some(e));
            }
            if 1.0 <= t {
                for k in 0..6 { sf(w, STAGE + 4 * k, 1.0); }
                pose(w, id);
                w.mm(id).state = 4;
                crate::cinematic::camera_script2(w, 3);
                let h = super::hero_pos(w);
                let r = w.hero.rot.map(|x| f32::from_bits(x.0));
                crate::cinematic::hero_teleport(w, [h[0], h[1], h[2]], [r[0], r[1], r[2]], 0, true);
                crate::moby_update::interact::set_global_flag(w, SHOWN, 1);
            }
        }
        4 if fade_toward(w, id, 0.0) == 0.0 => w.mm(id).state = 5,
        _ => {}
    }
}
