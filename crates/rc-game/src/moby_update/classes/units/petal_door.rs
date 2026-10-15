//! Blarg's petal doors, class 1021: level06 0x2f4f00 (census U212; 11 created instances). A placed door creates seven
//! more of its class, its petals, turned 45°, 90° … 315° about x; the door and each petal stand 3.109 out along
//! their own row 2 from the placed point. When the door is triggered (Ratchet within 6 in xy with no link; else by the
//! link +0x1c: a 1448 with Ratchet within 20 of it, a 586 with its +0xbc set, any other class past state 2) and all
//! seven petals are waiting, it plays sound 0 and all eight swing open by 60° about x (a braking turn, 270°/s²,
//! 360°/s², at most 180°/s), then hide. States 8 / 9 swing them shut again (entered by other code). Read from the
//! level06 decomp and disassembly (0x2f4f00; the trigger test at 0x2f5114). Native `f32`.
//!
//! **Pvar block**: +0x00 the placed position, +0x10 the placed x angle, +0x14 the swing, +0x18 0, +0x1c the link
//! (moby index, −1 none), +0x20 the swing velocity, +0x24..+0x3f the seven petals (moby index + 1 here; pointers in
//! the game).
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 (the door) | +0x10 = rot.x, +0x00 = position, → 2; seven `CreateMoby(0x3fd)`: update distance 0x40, draw distance 0x40, drawn, the door's light word and ambient (+0x38), mode, position and Euler, rot.x += (k + 1)·45°, state 1, the rotation rows (`fun_001fa030`) | [`update`] |
//! | state 1 (a petal) | +0x10 = rot.x, +0x00 = position, → 3, then as state 3 | [`update`] |
//! | state 2 | +0xbc = 0: triggered (module doc; gp−0x4ebc = 0 compared with 0.0: always true) and every petal in state 3 → `PlayClassSound(0, 0, m)`, → 4, +0x18 = +0x14 = 0, the petals → 5 | [`update`] |
//! | states 2, 3 | position = +0x00 + row 2 (+0xe0) set to 3.109 (gp−0x4eb8) | [`update`] |
//! | states 4, 5 | `0x270cc0(60°, 270°·dt², 360°·dt², 180°·dt, +0x14, +0x20)` (gp−0x4ec8 / −0x4ec4 / −0x4ec0); rot.x = +0x10 − +0x14; at 60°: not drawn, mode \| 1, → 6 (door) / 7 (petal) | [`update`] (`turn::turn_toward`) |
//! | states 8, 9 | the same turn toward 0; at 0 → 2 / 3 | [`update`] |
//! | `0x2f5360` / `0x2f53e8` | the open / close other classes call (Blarg's NPCs 1105 / 1109) | [`open`] / [`close`] |
//! | | no particle, light, hit, save flag | n/a |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::creature::turn::turn_toward;
use crate::moby_update::creature::{self as c, DT, DT2};
use crate::moby_update::services::World;

/// The update in the level06 class table.
pub const UPDATE_FN: u32 = 0x2f_4f00;
pub const REFERENCE_LEVEL: u32 = 6;
pub const CLASSES: [i16; 1] = [1021];

/// gp−0x4eb8: the distance out along row 2.
pub const OUT: f32 = f32::from_bits(0x4046_f9db);
/// The open angle (0x3f860a92, 60°).
pub const OPEN: f32 = f32::from_bits(0x3f86_0a92);
/// gp−0x4ec8 / −0x4ec4 / −0x4ec0: the turn's acceleration, braking (degrees/s²) and top speed (degrees/s).
const ACCEL: f32 = 270.0;
const BRAKE: f32 = 360.0;
const VMAX: f32 = 180.0;
const DEG: f32 = 0.017_453_292;
/// The trigger link classes.
pub const LINK_NEAR: i16 = 0x5a8;
pub const LINK_CMD: i16 = 0x24a;
const PETALS: usize = 7;
const PETAL: usize = 0x24;

fn petal(w: &World, id: MobyId, k: usize) -> Option<MobyId> {
    usize::try_from(c::pi32(w, id, PETAL + 4 * k) - 1).ok().filter(|&m| m < w.table.mobys.len())
}

fn triggered(w: &World, id: MobyId) -> bool {
    let link = c::pi32(w, id, 0x1c);
    let hero = super::hero_pos(w);
    match usize::try_from(link).ok().and_then(|l| w.table.mobys.get(l)) {
        None => c::dist2(w.m(id).position, hero) < 6.0,
        Some(m) if m.o_class == LINK_NEAR => c::dist2(hero, m.position) < 20.0,
        Some(m) if m.o_class == LINK_CMD => m.cmd != 0,
        Some(m) => 2 < m.state,
    }
}

fn swing(w: &mut World, id: MobyId, target: f32) -> f32 {
    let (mut a, mut v) = (c::pf(w, id, 0x14), c::pf(w, id, 0x20));
    turn_toward(target, ACCEL * DEG * DT2, BRAKE * DEG * DT2, VMAX * DEG * DT, &mut a, &mut v);
    c::set_pf(w, id, 0x14, a);
    c::set_pf(w, id, 0x20, v);
    let x = c::sub_rot(c::pf(w, id, 0x10), a);
    w.mm(id).rotation[0] = x;
    a
}

/// Level06 0x2f4f00 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x40 { return; }
    match w.m(id).state {
        0 => {
            let (x, p) = (w.m(id).rotation[0], w.m(id).position);
            c::set_pf(w, id, 0x10, x);
            c::set_pv4(w, id, 0, p);
            w.mm(id).state = 2;
            for k in 0..PETALS {
                let Some(s) = w.create_moby(CLASSES[0]) else { c::set_pi32(w, id, PETAL + 4 * k, 0); continue };
                c::set_pi32(w, id, PETAL + 4 * k, s as i32 + 1);
                let d = w.m(id).clone();
                let m = w.mm(s);
                if m.pvars.len() < 0x40 { m.pvars.resize(0x80, 0); }
                m.update_dist = 0x40;
                m.draw_dist = 0x40;
                m.visible = 1;
                m.light = d.light;
                m.ambient = d.ambient;
                m.mode = d.mode;
                m.position = d.position;
                m.rotation = d.rotation;
                m.rotation[0] = c::add_rot(m.rotation[0], ((k as i32 + 1) * 45) as f32 * DEG);
                m.state = 1;
                let r = rc_formats::moby_light::rotation_rows([m.rotation[0], m.rotation[1], m.rotation[2]]);
                for (row, q) in m.rows.iter_mut().zip(r.iter()) { *row = q.map(f32::from_bits); }
            }
        }
        1 => {
            let (x, p) = (w.m(id).rotation[0], w.m(id).position);
            c::set_pf(w, id, 0x10, x);
            c::set_pv4(w, id, 0, p);
            w.mm(id).state = 3;
            stand_out(w, id);
        }
        2 => {
            if w.m(id).cmd == 0 && triggered(w, id) && (0..PETALS).all(|k| petal(w, id, k).is_some_and(|p| w.m(p).state == 3)) {
                w.play_sound(0, 0, id);
                w.mm(id).state = 4;
                c::set_pf(w, id, 0x18, 0.0);
                c::set_pf(w, id, 0x14, 0.0);
                for k in 0..PETALS {
                    if let Some(p) = petal(w, id, k) { w.mm(p).state = 5; }
                }
            }
            stand_out(w, id);
        }
        3 => stand_out(w, id),
        s @ (4 | 5) => {
            if swing(w, id, OPEN) < std::f32::consts::FRAC_PI_3 { return; }
            let m = w.mm(id);
            m.visible = 0;
            m.mode |= mode::HIDDEN;
            m.state = if s == 4 { 6 } else { 7 };
        }
        s @ (8 | 9) => {
            if 0.0 < swing(w, id, 0.0) { return; }
            w.mm(id).state = if s == 8 { 2 } else { 3 };
        }
        _ => {}
    }
}

/// `0x2f5360(door)`: the door opened by another class (Blarg's scientist 1105): +0xbc = 1; a closed door (state 2)
/// plays sound 0, → 4, +0x18 = +0x14 = 0, its petals → 5.
pub fn open(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x40 { return; }
    w.mm(id).cmd = 1;
    if w.m(id).state != 2 { return; }
    w.play_sound(0, 0, id);
    w.mm(id).state = 4;
    c::set_pf(w, id, 0x18, 0.0);
    c::set_pf(w, id, 0x14, 0.0);
    for k in 0..PETALS {
        if let Some(p) = petal(w, id, k) { w.mm(p).state = 5; }
    }
}

/// `0x2f53e8(door)`: the door closed again by another class: +0xbc = 1; an open door (state 6) plays sound 0, is drawn
/// and shown, → 8, +0x18 = 0, its petals → 9, shown and drawn.
pub fn close(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x40 { return; }
    w.mm(id).cmd = 1;
    if w.m(id).state != 6 { return; }
    w.play_sound(0, 0, id);
    let m = w.mm(id);
    m.visible = 1;
    m.state = 8;
    m.mode &= !mode::HIDDEN;
    c::set_pf(w, id, 0x18, 0.0);
    for k in 0..PETALS {
        if let Some(p) = petal(w, id, k) {
            let m = w.mm(p);
            m.state = 9;
            m.mode &= !mode::HIDDEN;
            m.visible = 1;
        }
    }
}

/// Position = +0x00 + row 2 set to [`OUT`].
fn stand_out(w: &mut World, id: MobyId) {
    let r2 = w.m(id).rows[2];
    let off = c::set_len3([r2[0], r2[1], r2[2], 0.0], OUT);
    let home = c::pv4(w, id, 0);
    c::set_pos(w, id, c::add(off, home));
}
