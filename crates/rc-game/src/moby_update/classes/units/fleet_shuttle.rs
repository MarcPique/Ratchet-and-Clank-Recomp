//! **The Fleet's shuttle, class 1448** (level17 `0x2f1940`; census U614, one placed). Ratchet's ship parked at one of
//! two landing spots (the cuboids +0x78 / +0x7c); standing on it he presses △ (the context prompt, owner 10) and it
//! flies him to the other spot along its route's three paths (camera, camera look, ship) with the script camera,
//! shrinking or growing on the way and rolling over near the end; he is set down behind it. Once it has flown (its
//! death bits), it waits at whichever spot is nearer to him. Blarg's shuttle 1109 (`blarg_shuttle`) is the same code
//! with five routes. Read from the level17 decomp (`0x2f1940`, the prompt `0x2f20d8`, the flight `0x2f2180`); native
//! `f32`.
//!
//! **Pvar block** (0xb0): +0x20 the riders' carry block, +0x60 / +0x68 / +0x70 the two routes' camera / look / ship
//! paths, +0x78 / +0x7c the spots' cuboids, +0x80 the offer, +0x84 Ratchet's seat cuboid, +0x88 / +0x8c the roll and
//! its rate, +0x94 the spot it is at, +0x98 the route, +0x9c / +0xa0 the scale at its start / end, +0xa4 the flight's
//! progress, +0xa8 leaving, +0xac the idle hum played.
//!
//! | address | what | port |
//! |---|---|---|
//! | entry | scale = class scale·2.2 (gp−0x4854) | [`update`] |
//! | 0 | (dead: +0x94 = +0x80 = +0x98 = 0, overwritten below); update 0xff, draw 0x80, Ratchet's light / ambient (+0x38); → 1; blend 2; +0x80 = +0x94 = +0x98 = 1; at cuboid +0x7c (centre and Euler, with their w) | [`update`] |
//! | 1 | seq 2 wrapped (+0x70 & 2) and +0xac = 0: `PlayClassSound(1, 0)` | [`update`] |
//! | 1 | Ratchet on it (0x13f64c = m, 0x13f65e = 0): the prompt (`0x2f20d8`: +0x80 = +0x94 when 0 / 1; 0 → `try_set_help_message(10, 0x426c)`, 1 / 4 → 0x426b, 2 → 0x1786, 3 → 0x1784); △ (0x13cae4 & 0x10) with the prompt (owner gp−0x766c = 10) and +0x80 ≥ 0: leaving; 0 → +0x94 = 1, +0x98 = 0, 2.2 → 0.6; 1 → +0x98 = 1, 0.6 → 2.2, +0x94 = 0 | [`update`] |
//! | 1 | not leaving, dead (`0x1bbf84[id]` or the persistent bit): the other spot nearer to Ratchet (`VecDistance2`, the cuboids' centres) → there (+0x80, +0x98, +0x94 flipped; rows; class scale) | [`update`] |
//! | 1 | leaving: `PromptRelease(10)`, → 2, `PlayClassSound(2, 0)`, blend 0, `FadeToBlack(ticks(10))`, roll 0; `CameraScript(the camera path's start, (0, −pitch, yaw) toward the look path's start, 2, ticks(300), 0)`, the springs (0.001, 1, 1, 0.001, 1, 1) (`0x2fe2c8`), the targets; `HeroTeleport(cuboid +0x84, 0x72, 0)` | [`update`] |
//! | 2 | the flight (`0x2f2180`) done: → 3, +0xa4 = 0; at the spot +0x94; rows; class scale; `HeroTeleport(position − 2·unit(row 0) + row 2, (0, 0, yaw + π), 0, 0)`; `CameraScript2(0)`; its death bits | [`update`] |
//! | 3 | → 1; blend 2 over `ticks(30)`; `PlayClassSound(0, 0)`; +0xac = 1 | [`update`] |
//! | tail | `CarryRiders(+0x20, position − old, old rotation, rotation)` | [`update`] |
//! | `0x2f2180` | progress `Approach(1, 1/ticks(900))`; the camera on its path; the ship on its own (the fraction less the camera path's index, the game's); `HeroTeleport(cuboid +0x84, 0x72, 0)` each tick; yaw / pitch blended between segments, pitch ·cos(roll), yaw + roll; the look from the look path at `min(1.02·t, 1)`; scale from +0x9c to +0xa0; past 0.8 the roll (`0x2566b0(π, 4π·dt², 4π·dt², 2π·dt)`); done at 1 | [`flight`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::turn::{approach, turn_toward};
use crate::moby_update::creature::{self as c, add, add_rot, atan, dist2, set_len3, sub, sub_rot, DT, DT2};
use crate::moby_update::services::World;
use crate::moby_update::{story, triggers};
use std::f32::consts::{PI, TAU};

pub const REFERENCE_LEVEL: u32 = 17;
pub const UPDATE_FN: u32 = 0x2f_1940;
pub const CLASSES: [i16; 1] = [1448];

mod o {
    pub const CARRY: usize = 0x20;
    pub const CAM_PATH: usize = 0x60;
    pub const LOOK_PATH: usize = 0x68;
    pub const PATH: usize = 0x70;
    pub const SPOTS: usize = 0x78;
    pub const OFFER: usize = 0x80;
    pub const SEAT: usize = 0x84;
    pub const ROLL: usize = 0x88;
    pub const ROLL_V: usize = 0x8c;
    pub const SPOT: usize = 0x94;
    pub const ROUTE: usize = 0x98;
    pub const SCALE0: usize = 0x9c;
    pub const SCALE1: usize = 0xa0;
    pub const T: usize = 0xa4;
    pub const LEAVING: usize = 0xa8;
    pub const HUMMED: usize = 0xac;
    pub const SIZE: usize = 0xb0;
}

/// gp−0x4858 / −0x4854: the small and large scales.
const SMALL: f32 = f32::from_bits(0x3f19_999a);
const LARGE: f32 = f32::from_bits(0x400c_cccd);
const PROMPT_OWNER: i32 = 10;

fn pi(w: &World, id: MobyId, o: usize) -> i32 { c::pi32(w, id, o) }
fn seti(w: &mut World, id: MobyId, o: usize, v: i32) { c::set_pi32(w, id, o, v) }

fn path(w: &World, i: i32) -> Vec<[f32; 4]> {
    usize::try_from(i).ok().and_then(|i| w.svc.splines.get(i)).map_or(Vec::new(), |p| p.iter().map(|q| q.map(f32::from_bits)).collect())
}
fn point(p: &[[f32; 4]], i: i32) -> [f32; 4] { usize::try_from(i).ok().and_then(|i| p.get(i)).copied().unwrap_or([0.0; 4]) }
fn lerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] { std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t) }

fn spot(w: &World, c: i32) -> Option<rc_formats::volumes::Shape> { w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, c).copied() }
/// The moby at cuboid `c`'s centre with its Euler (+0x30.. / +0x70.., their w words too).
fn place(w: &mut World, id: MobyId, c: i32) {
    if let Some(s) = spot(w, c) {
        let p = s.centre();
        let m = w.mm(id);
        m.position = [p[0], p[1], p[2], s.matrix[3][3]];
        m.rotation = [s.euler[0], s.euler[1], s.euler[2], s.unused_7c];
    }
}
fn blend(w: &mut World, id: MobyId, seq: u8, ticks: i32) {
    if w.m(id).anim.seq_b != seq { w.anim_blend(id, seq, 0, ticks); }
}
/// `0x1bbf84[id]` or the persistent death bit.
fn flown(w: &World, id: MobyId) -> bool {
    let sid = w.m(id).spawn_id;
    w.svc.save.collected.get(&sid).is_some_and(|&b| b != 0) || w.svc.save.death.contains(&(w.svc.level, sid))
}

/// Level17 `0x2f1940` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, o::SIZE);
    let (old_pos, old_rot) = (w.m(id).position, w.m(id).rotation);
    let cs = super::class_scale(w, w.m(id).o_class);
    w.mm(id).scale = cs * LARGE;
    match w.m(id).state {
        0 => {
            let (l, a) = w.hero_moby.map_or((0, [0; 4]), |h| (w.m(h).light, w.m(h).ambient));
            {
                let m = w.mm(id);
                m.update_dist = 0xff;
                m.draw_dist = 0x80;
                m.light = l;
                m.ambient = a;
                m.state = 1;
            }
            blend(w, id, 2, 0);
            seti(w, id, o::OFFER, 1);
            seti(w, id, o::SPOT, 1);
            seti(w, id, o::ROUTE, 1);
            place(w, id, pi(w, id, o::SPOTS + 4));
        }
        1 => stop(w, id),
        2 => {
            let route = pi(w, id, o::ROUTE);
            if flight(w, id, route) {
                w.mm(id).state = 3;
                c::set_pf(w, id, o::T, 0.0);
                let k = pi(w, id, o::SPOT);
                place(w, id, pi(w, id, o::SPOTS + 4 * k.clamp(0, 1) as usize));
                w.build_matrix(id);
                w.mm(id).scale = cs;
                let m = w.m(id);
                let p = add(add(m.position, set_len3(m.rows[0], -2.0)), m.rows[2]);
                let e = [0.0, 0.0, add_rot(m.rotation[2], PI)];
                crate::cinematic::hero_teleport(w, [p[0], p[1], p[2]], e, 0, false);
                crate::cinematic::camera_script2(w, 0);
                story::death_bits(w, id);
            }
        }
        3 => {
            w.mm(id).state = 1;
            let t = w.ticks(0x1e);
            blend(w, id, 2, t);
            w.play_sound(0, 0, id);
            seti(w, id, o::HUMMED, 1);
        }
        _ => {}
    }
    let (p, r) = (w.m(id).position, w.m(id).rotation);
    triggers::carry_riders(&mut w.mm(id).pvars, o::CARRY, sub(p, old_pos), old_rot, r);
}

/// State 1: at a spot (module doc).
fn stop(w: &mut World, id: MobyId) {
    if w.m(id).anim.seq_b == 2 && w.m(id).anim.flags & 2 != 0 && pi(w, id, o::HUMMED) == 0 {
        w.play_sound(1, 0, id);
        seti(w, id, o::HUMMED, 0);
    }
    if w.hero.ground_moby == Some(id) && w.hero.air_ticks == 0 {
        prompt(w, id);
        if w.hero.loop_in.pad.pressed & 0x10 != 0 && w.svc.interact.prompt.owner == PROMPT_OWNER {
            match pi(w, id, o::OFFER) {
                0 => {
                    seti(w, id, o::LEAVING, 1);
                    seti(w, id, o::SPOT, 1);
                    c::set_pf(w, id, o::SCALE0, LARGE);
                    c::set_pf(w, id, o::SCALE1, SMALL);
                    seti(w, id, o::ROUTE, 0);
                }
                1 => {
                    seti(w, id, o::LEAVING, 1);
                    seti(w, id, o::ROUTE, 1);
                    c::set_pf(w, id, o::SCALE0, SMALL);
                    c::set_pf(w, id, o::SCALE1, LARGE);
                    seti(w, id, o::SPOT, 0);
                }
                r if r >= 0 => seti(w, id, o::LEAVING, 1),
                _ => {}
            }
        }
    }
    if pi(w, id, o::LEAVING) != 0 {
        depart(w, id);
        return;
    }
    if !flown(w, id) { return; }
    let k = pi(w, id, o::SPOT).clamp(0, 1);
    let h = story::hero4(w);
    let centre = |w: &World, k: i32| spot(w, pi(w, id, o::SPOTS + 4 * k as usize)).map_or([0.0; 4], |s| { let c = s.centre(); [c[0], c[1], c[2], 0.0] });
    let (here, other) = (dist2(h, centre(w, k)), dist2(h, centre(w, k ^ 1)));
    if other < here {
        seti(w, id, o::OFFER, pi(w, id, o::OFFER) ^ 1);
        seti(w, id, o::ROUTE, pi(w, id, o::ROUTE) ^ 1);
        seti(w, id, o::SPOT, k ^ 1);
        place(w, id, pi(w, id, o::SPOTS + 4 * (k ^ 1) as usize));
        w.build_matrix(id);
        let cs = super::class_scale(w, w.m(id).o_class);
        w.mm(id).scale = cs;
    }
}

/// The departure (module doc).
fn depart(w: &mut World, id: MobyId) {
    seti(w, id, o::LEAVING, 0);
    let up = w.svc.interact.prompt_hud;
    w.svc.interact.prompt.release(PROMPT_OWNER, up);
    w.mm(id).state = 2;
    w.play_sound(2, 0, id);
    if w.m(id).anim.seq_b != 0 { w.anim_blend(id, 0, 0, 0); }
    let t = w.ticks(10);
    crate::cinematic::fade_to_black(w, t);
    c::set_pf(w, id, o::ROLL, 0.0);
    let k = pi(w, id, o::ROUTE).clamp(0, 1) as usize;
    let a = point(&path(w, pi(w, id, o::CAM_PATH + 4 * k)), 0);
    let b = point(&path(w, pi(w, id, o::LOOK_PATH + 4 * k)), 0);
    let e = [0.0, -atan(dist2(a, b), b[2] - a[2]), atan(b[0] - a[0], b[1] - a[1])];
    let at = [a[0], a[1], a[2]];
    let t = w.ticks(300);
    crate::cinematic::camera_script(w, at, e, 2, t, false);
    crate::cinematic::script_springs(w, [f32::from_bits(0x3a83_126f), 1.0, 1.0, f32::from_bits(0x3a83_126f), 1.0, 1.0]);
    crate::cinematic::camera_targets(w, Some(at), Some(e));
    story::teleport_to(w, pi(w, id, o::SEAT), 0x72, false);
}

/// `0x2f20d8` (module doc).
fn prompt(w: &mut World, id: MobyId) {
    match pi(w, id, o::SPOT) {
        0 => seti(w, id, o::OFFER, 0),
        1 => seti(w, id, o::OFFER, 1),
        _ => {}
    }
    let msg = match pi(w, id, o::OFFER) {
        0 => 0x426c,
        1 | 4 => 0x426b,
        2 => 0x1786,
        3 => 0x1784,
        _ => return,
    };
    w.svc.interact.try_prompt(PROMPT_OWNER, msg);
}

/// Level17 `0x2f2180(route)`: one tick of the flight; true at its end (module doc).
fn flight(w: &mut World, id: MobyId, route: i32) -> bool {
    let r = route.clamp(0, 1) as usize;
    let cam_path = path(w, pi(w, id, o::CAM_PATH + 4 * r));
    let look_path = path(w, pi(w, id, o::LOOK_PATH + 4 * r));
    let ship_path = path(w, pi(w, id, o::PATH + 4 * r));
    let mut t = c::pf(w, id, o::T);
    let n = w.ticks(900);
    approach(1.0, 1.0 / n as f32, &mut t);
    c::set_pf(w, id, o::T, t);
    let fc = t * (cam_path.len() as f32 - 1.0);
    let i = fc as i32;
    let fl = (t * 1.02).min(1.0) * (look_path.len() as f32 - 1.0);
    let il = fl as i32;
    let frl = fl - il as f32;
    let fs = t * (ship_path.len() as f32 - 1.0);
    let frs = fs - i as f32;
    let is = fs as i32;
    let cam = lerp(point(&cam_path, i), point(&cam_path, i + 1), fc - i as f32);
    let p = lerp(point(&ship_path, is), point(&ship_path, is + 1), frs);
    w.mm(id).position = [p[0], p[1], p[2], w.m(id).position[3]];
    crate::cinematic::camera_targets(w, Some([cam[0], cam[1], cam[2]]), None);
    story::teleport_to(w, pi(w, id, o::SEAT), 0x72, false);
    if is < 1 {
        let (a, b) = (point(&ship_path, 0), point(&ship_path, 1));
        let m = w.mm(id);
        m.rotation[1] = -atan(dist2(a, b), b[2] - a[2]);
        m.rotation[2] = atan(b[0] - a[0], b[1] - a[1]);
    } else {
        let roll = c::pf(w, id, o::ROLL);
        let (a, b, d) = (point(&ship_path, is - 1), point(&ship_path, is), point(&ship_path, is + 1));
        let y0 = atan(b[0] - a[0], b[1] - a[1]);
        let y1 = atan(d[0] - b[0], d[1] - b[1]);
        let yaw = add_rot(sub_rot(y1, y0) * frs, y0);
        let e0 = atan(dist2(a, b), b[2] - a[2]);
        let e1 = atan(dist2(b, d), d[2] - b[2]);
        let pitch = add_rot(sub_rot(-e1, -e0) * frs, -e0) * roll.cos();
        let m = w.mm(id);
        m.rotation[1] = pitch;
        m.rotation[2] = add_rot(yaw, roll);
    }
    if 0 < i && 0 < il {
        let (a, b) = (point(&look_path, il), point(&look_path, il + 1));
        let y0 = atan(a[0] - cam[0], a[1] - cam[1]);
        let y1 = atan(b[0] - cam[0], b[1] - cam[1]);
        let yaw = add_rot(sub_rot(y1, y0) * frl, y0);
        let e0 = atan(dist2(cam, a), a[2] - cam[2]);
        let e1 = atan(dist2(cam, b), b[2] - cam[2]);
        let pitch = add_rot(sub_rot(-e1, -e0) * frl, -e0);
        crate::cinematic::camera_targets(w, None, Some([0.0, pitch, yaw]));
    }
    let cs = super::class_scale(w, w.m(id).o_class);
    let (s0, s1) = (c::pf(w, id, o::SCALE0), c::pf(w, id, o::SCALE1));
    w.mm(id).scale = cs * (s0 - (s0 - s1) * t);
    if 0.8 < t {
        let (mut a, mut v) = (c::pf(w, id, o::ROLL), c::pf(w, id, o::ROLL_V));
        turn_toward(PI, DT2 * 12.566_371, DT2 * 12.566_371, DT * TAU, &mut a, &mut v);
        c::set_pf(w, id, o::ROLL, a);
        c::set_pf(w, id, o::ROLL_V, v);
    }
    t == 1.0
}
