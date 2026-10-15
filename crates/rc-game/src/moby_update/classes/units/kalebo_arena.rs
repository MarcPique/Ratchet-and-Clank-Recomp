//! **Kalebo's arena triggers, class 654** (level16 `0x2d5ef8`, 4 placed; census U552; the name is descriptive [L]).
//! Each watches a cuboid. With a camera spline: when Ratchet steps in, the barriers of its group (class 647,
//! [`super::extending_piece`]) slide in and the script camera takes over where the follow camera is; a second later the
//! arena's enemies (class 541, [`super::kalebo_trooper`]) wake, and while they fight the camera slides along the
//! two-point spline as Ratchet crosses the cuboid; once they are all gone (and a second has passed) Ratchet is put at a
//! second cuboid with the follow camera back, anything near it is hit, the arena's flag is set and the barriers slide
//! out. Without a camera spline it stays hidden and only wakes the enemies and holds the barriers until they are gone.
//! In a fight, the spline wall (`Hero::wall_spline`, pvar +0x14) keeps Ratchet in. Read from the level16 decomp and its
//! words gp−0x5180 / −0x5174.
//!
//! **Pvars**: +0x00 the cuboid, +0x04 the barriers' group, +0x08 the enemies' group, +0x0c the camera spline (−1:
//! none), +0x10 the cuboid Ratchet is put at, +0x14 the wall spline (−1: none), +0x18 the flag (114 + it, 0x13d3fa),
//! +0x1c / +0x20 the camera's slide and its velocity, +0x24 the timer.
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | no pvars or +0x00 = −1 → `DeleteMoby`; a camera spline → 1, the barriers set out at once (`0x2d5ad0(group, 1)`); else → 5, no collision, hidden (mode \|= 1, +0x31 = 0) | [`update`] |
//! | state 1 | Ratchet in the cuboid → the timer `ticks(60)`, → 2, the wall; `CameraScript(the camera 0x1671c0, its Euler 0x1671d0, 1, 0, 0)`; the barriers in (`0x2d5bc8(group, 0)`): `PlayClassSound(1, 0)` on the one nearest the camera | [`update`] |
//! | state 2 | the wall; the timer out → 3, the timer `ticks(60)`, hidden, the enemies woken (`0x2ceca8`: each 541 of the group drawn, state 2, mode &= ~1) | [`update`] |
//! | state 3 | the camera (`0x2d6260`): l = (Ratchet − cuboid centre)·its inverse rows; `0x257268((l.y + 1)/2, dt², dt², 2·dt, &+0x1c, &+0x20)`; position = the spline's points 0 → 1 at +0x1c, Euler (0, 0.35, yaw) (`0x2f1d80` / `0x2f1dd8`); the wall; `MobyGroupCount(enemies, −1)` = 0 and the timer out → drawn, → 4, shown, `CameraScript2(3)`, `HeroTeleport(cuboid +0x10 centre, its Euler, 0, 1)`, `0x2551e8(5, 50, 1, m, that centre, 0x10000, 0, 1, 0)`, the flag; the barriers out (`0x2d5ad0(group, 0)`): sound 1 on the nearest | [`update`] (`creature::attack::sphere_hit`) |
//! | state 5 | Ratchet in the cuboid → the barriers in (no sound), the enemies woken, → 6 | [`update`] |
//! | state 6 | Ratchet in the cuboid → the wall; the enemies all gone → 4, the barriers out (sound 1 on the nearest), the flag | [`update`] |
//! | `0x2d5ad0` / `0x2d5bc8` | each 647 of the group: state 1, its target +0x14 = 1.5 / 0 (with `1`: its extension +0x18 too); returns the one nearest the camera within 1000 (`vec_distance`) | [`barriers`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, turn, DT, DT2};
use crate::moby_update::services::{fv, World};

pub const REFERENCE_LEVEL: u32 = 16;
pub const UPDATE_FN: u32 = 0x2d_5ef8;
pub const CLASSES: [i16; 1] = [654];
/// The barriers' and the enemies' classes.
pub const BARRIER: i16 = 0x287;
pub const ENEMY: i16 = 0x21d;
/// The first arena flag (0x13d3fa).
pub const FLAG_BASE: usize = 114;
/// gp−0x5180: the barriers' extension.
pub const OUT: f32 = 1.5;
/// gp−0x5174: the camera's pitch.
pub const PITCH: f32 = f32::from_bits(0x3eb3_3333);

/// Pvar offsets (module doc).
pub mod pv {
    pub const CUBOID: usize = 0x00;
    pub const BARRIERS: usize = 0x04;
    pub const ENEMIES: usize = 0x08;
    pub const SPLINE: usize = 0x0c;
    pub const PUT: usize = 0x10;
    pub const WALL: usize = 0x14;
    pub const FLAG: usize = 0x18;
    pub const SLIDE: usize = 0x1c;
    pub const SLIDE_V: usize = 0x20;
    pub const TIMER: usize = 0x24;
    pub const SIZE: usize = 0x28;
}

fn group(w: &World, g: i32) -> Vec<MobyId> {
    usize::try_from(g).ok().and_then(|g| w.svc.groups.lists.get(g).cloned().flatten()).unwrap_or_default().into_iter().map(usize::from).filter(|&k| k < w.table.mobys.len()).collect()
}

/// `0x2d5ad0` (`target` 1.5) / `0x2d5bc8` (0) (module doc).
pub fn barriers(w: &mut World, g: i32, target: f32, both: bool) -> Option<MobyId> {
    let cam = fv(w.camera);
    let (mut best, mut near) = (1000.0f32, None);
    for k in group(w, g) {
        if w.m(k).o_class != BARRIER { continue; }
        let p = w.m(k).position;
        let d = ((cam[0] - p[0]).powi(2) + (cam[1] - p[1]).powi(2) + (cam[2] - p[2]).powi(2)).sqrt();
        if d < best {
            best = d;
            near = Some(k);
        }
        w.mm(k).state = 1;
        if w.m(k).pvars.len() >= 0x1c {
            c::set_pf(w, k, 0x14, target);
            if both { c::set_pf(w, k, 0x18, target); }
        }
    }
    near
}

/// `0x2ceca8(group)`: each 541 of the group drawn, state 2, shown.
fn wake(w: &mut World, g: i32) {
    for k in group(w, g) {
        if w.m(k).o_class != ENEMY { continue; }
        let m = w.mm(k);
        m.visible = 1;
        m.state = 2;
        m.mode &= !1;
    }
}

fn wall(w: &mut World, id: MobyId) {
    let s = c::pi32(w, id, pv::WALL);
    if 0 <= s { w.hero_fields_mut().wall_spline = Some(s as i16); }
}

fn flag(w: &mut World, id: MobyId) {
    let k = c::pi32(w, id, pv::FLAG);
    if let Ok(k) = usize::try_from(k) { crate::moby_update::interact::set_global_flag(w, FLAG_BASE + k, 1); }
}

fn barriers_out(w: &mut World, id: MobyId) {
    if let Some(b) = barriers(w, c::pi32(w, id, pv::BARRIERS), OUT, false) { w.play_sound(1, 0, b); }
}

fn hero_in(w: &World, id: MobyId) -> bool { w.in_cuboid(w.hero_point(), c::pi32(w, id, pv::CUBOID)) }

fn enemies_gone(w: &World, id: MobyId) -> bool { crate::moby_update::scheduler::group_count(w, c::pi32(w, id, pv::ENEMIES), -1) == 0 }

/// `0x2d6260`: the camera's slide along its spline (module doc).
fn camera(w: &mut World, id: MobyId) {
    let Some(cub) = w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, c::pi32(w, id, pv::CUBOID)).copied() else { return };
    let h = super::hero_pos(w);
    let d = [h[0] - cub.matrix[3][0], h[1] - cub.matrix[3][1], h[2] - cub.matrix[3][2]];
    let ly = d[0] * cub.inverse[0][1] + d[1] * cub.inverse[1][1] + d[2] * cub.inverse[2][1];
    let (mut s, mut v) = (c::pf(w, id, pv::SLIDE), c::pf(w, id, pv::SLIDE_V));
    turn::spring((ly + 1.0) * 0.5, DT2, DT2, DT + DT, &mut s, &mut v);
    c::set_pf(w, id, pv::SLIDE, s);
    c::set_pf(w, id, pv::SLIDE_V, v);
    let pts = usize::try_from(c::pi32(w, id, pv::SPLINE)).ok().and_then(|i| w.svc.splines.get(i)).filter(|p| p.len() >= 2).map(|p| [p[0], p[1]].map(|q| q.map(f32::from_bits)));
    let Some([a, b]) = pts else { return };
    let pos: [f32; 3] = std::array::from_fn(|k| a[k] + (b[k] - a[k]) * s);
    let yaw = w.m(id).rotation[2];
    crate::cinematic::camera_targets(w, Some(pos), Some([0.0, PITCH, yaw]));
}

/// Level16 `0x2d5ef8` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE || c::pi32(w, id, pv::CUBOID) == -1 {
        if w.m(id).state == 0 { w.delete_moby(id); }
        return;
    }
    match w.m(id).state {
        0 => {
            if c::pi32(w, id, pv::SPLINE) != -1 {
                w.mm(id).state = 1;
                barriers(w, c::pi32(w, id, pv::BARRIERS), OUT, true);
            } else {
                let m = w.mm(id);
                m.state = 5;
                m.has_collision = false;
                m.mode |= 1;
                m.visible = 0;
            }
        }
        1 => {
            if !hero_in(w, id) { return; }
            let t = w.ticks(60);
            c::set_pi32(w, id, pv::TIMER, t);
            w.mm(id).state = 2;
            wall(w, id);
            let (p, e) = (fv(w.camera), w.hero.loop_in.cam_euler);
            crate::cinematic::camera_script(w, [p[0], p[1], p[2]], e, 1, 0, false);
            if let Some(b) = barriers(w, c::pi32(w, id, pv::BARRIERS), 0.0, false) { w.play_sound(1, 0, b); }
        }
        2 => {
            wall(w, id);
            if c::dec_timer_pvar_i32(w, id, pv::TIMER) != 0 {
                let t = w.ticks(60);
                c::set_pi32(w, id, pv::TIMER, t);
                let m = w.mm(id);
                m.state = 3;
                m.visible = 0;
                m.mode |= 1;
                wake(w, c::pi32(w, id, pv::ENEMIES));
            }
        }
        3 => {
            camera(w, id);
            wall(w, id);
            if !enemies_gone(w, id) || c::dec_timer_pvar_i32(w, id, pv::TIMER) == 0 { return; }
            let m = w.mm(id);
            m.visible = 1;
            m.state = 4;
            m.mode &= !1;
            crate::cinematic::camera_script2(w, 3);
            if let Some(s) = w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, c::pi32(w, id, pv::PUT)).copied() {
                let at = s.centre();
                crate::cinematic::hero_teleport(w, at, s.euler, 0, true);
                crate::moby_update::creature::attack::sphere_hit(w, 5.0, 50.0, 1.0, id, [at[0], at[1], at[2], s.matrix[3][3]], 0x1_0000, 0, 1, 0);
            }
            flag(w, id);
            barriers_out(w, id);
        }
        5 => {
            if !hero_in(w, id) { return; }
            barriers(w, c::pi32(w, id, pv::BARRIERS), 0.0, false);
            wake(w, c::pi32(w, id, pv::ENEMIES));
            w.mm(id).state = 6;
        }
        6 => {
            if hero_in(w, id) { wall(w, id); }
            if !enemies_gone(w, id) { return; }
            w.mm(id).state = 4;
            barriers_out(w, id);
            flag(w, id);
        }
        _ => {}
    }
}
