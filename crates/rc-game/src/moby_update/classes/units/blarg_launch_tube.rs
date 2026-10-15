//! **Blarg's launch tube, class 1118** (level06 `0x304098` with its spark burst `0x304818`; 1 placed; census U244; the
//! name is descriptive [L]). Once other code opens it (state 2: it turns open with its two doors), Ratchet can step in
//! within 3.7 and press △ at the prompt (0x1783): a fade, then the script camera rises up the tube from cuboid +0x00
//! (18 a unit of the ride) as he shoots up through it, sparks bursting at the nine cuboids +0x08..+0x28 as he passes
//! them; at the top he is set down at cuboid +0x04 with a fade, a camera shake and the camera looking down on him, and
//! the moby +0x34 is told to go on (its state 2). Read from the level06 decomp; its words gp−0x4a68..−0x4a48. Native
//! `f32`.
//!
//! **Pvars**: +0x00 the ride's camera cuboid, +0x04 the arrival cuboid, +0x08..+0x28 the nine spark cuboids, +0x2c the
//! ride (0 → 2), +0x30 its speed, +0x34 the moby woken at the end, +0x38 / +0x3c the doors, +0x40 the timer, +0x44
//! (`ticks(3600)` at the end, not read), +0x48 (−1).
//!
//! | state | what | port |
//! |---|---|---|
//! | 0 | update distance 0xff; rot.x −= 45°; → 1; +0x48 −1 | [`update`] |
//! | 1 | (opened by other code: state 2) | — |
//! | 2 | the ride spring (`0x26a780(1, dt², dt², dt)` = `0x270830`); rot.x += speed·45°, each door's −= it (their matrices rebuilt); at 1 → 3, sound 1, flag 0x13d3aa | [`update`] (`turn::spring`) |
//! | 3 | Ratchet within 3.7 (xy): the prompt (owner 4, 0x1783); △ with it → sound 2, `PromptRelease(4)`, 4, timer `ticks(30)` | [`update`] |
//! | 4 | the timer out → 5, the ride 0, `FadeToBlack(ticks(15))`, `CameraScript(cuboid +0x00, its Euler, 2, ticks(300), 0)`, the springs (`0x312b40(0.0001, 1, 0, 0.001, 1, 0)`), the targets, the letterbox on; under `ticks(10)` left: the fade 1 − left/`ticks(10)` | [`update`] |
//! | 5 | the fade back (+1 a tick to `ticks(10)`); the ride spring (2, …, dt/2); the camera at cuboid +0x00 up 18·ride; each spark cuboid whose Euler z lies between the last and this ride ·100° → a burst (`0x304818`) and `0x29aec0(0)` (the level def 2 + 0, `World::play_level_def`); at 1.075 → `HeroTeleport(cuboid +0x04, 0x72, 0)`, `FadeToBlack(ticks(10))`, 6, `CameraScript2(3)`, `CameraScript(+0x04's centre − 2.5 y + 1 z, (0, −15°, Ratchet's yaw), 2, ticks(300), 0)`, a shake (0.3 up, `ticks(45)`), `0x29aec0(0)` | [`update`] |
//! | 6 | the ride spring (2, …, 2·dt); the camera targets as set; at 2 → 7, +0x44 `ticks(3600)`, `HeroTeleport(cuboid +0x04, 0, 0)`, `CameraScript2(3)`, the moby +0x34's state 2 | [`update`] |
//! | `0x304818(m, p)` | 8 type-11 spark pairs at p (`SPARK_A` / `SPARK_B` by `randi(6)`, 4000000, `randf(8, 10)`·dt·10, lives `rand_range(ticks(15), ticks(20))` / `rand_range(ticks(25), ticks(30))`) | [`burst`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, fx, turn, DT, DT2};
use crate::moby_update::services::{pf, pv as v4, World};
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 6;
pub const UPDATE_FN: u32 = 0x30_4098;
pub const CLASSES: [i16; 1] = [1118];
const PROMPT_OWNER: i32 = 4;
const PROMPT_MSG: i32 = 0x1783;
/// gp−0x4a68..−0x4a50: the script camera's springs; −0x4a4c (10): the sparks' size; −0x4a48 (8): their pairs.
const SPRINGS: [f32; 6] = [f32::from_bits(0x38d1_b717), 1.0, 0.0, f32::from_bits(0x3a83_126f), 1.0, 0.0];
const SPARK_SIZE: f32 = 10.0;
const SPARKS: i32 = 8;
const PITCH: f32 = f32::from_bits(0xbe86_0a92);

fn pi(w: &World, id: MobyId, o: usize) -> i32 { c::pi32(w, id, o) }

/// `0x304818(m, p)` (module doc).
fn burst(w: &mut World, p: [f32; 3]) {
    for _ in 0..SPARKS {
        let sp = w.rng.randf(8.0, 10.0) * DT;
        let a = w.rng.randi(6) as usize;
        let b = w.rng.randi(6) as usize;
        let (t15, t20) = (w.ticks(0xf), w.ticks(0x14));
        let life = w.rng.rand_range(t15, t20);
        let (t25, t30) = (w.ticks(0x19), w.ticks(0x1e));
        let t1 = w.rng.rand_range(t25, t30);
        w.part11(pf(SPARK_SIZE * 400_000.0), pf(sp * SPARK_SIZE), v4([p[0], p[1], p[2], 1.0]), v4([0.0; 4]), fx::SPARK_A[a], fx::SPARK_B[b], life, t1, 0, 0);
    }
}

fn cam_at(w: &World, cub: i32, up: f32) -> Option<([f32; 3], [f32; 3])> {
    story::cuboid(w, cub).map(|(p, e)| ([p[0], p[1], p[2] + up], e))
}

/// The arrival camera: cuboid +0x04's centre 2.5 back in y and 1 up, looking 15° down along Ratchet's yaw.
fn arrival_cam(w: &World, id: MobyId) -> Option<([f32; 3], [f32; 3])> {
    let (p, _) = story::cuboid(w, pi(w, id, 0x04))?;
    let yaw = f32::from_bits(w.hero.rot[2].0);
    Some(([p[0], p[1] - 2.5, p[2] + 1.0], [0.0, PITCH, yaw]))
}

/// Level06 `0x304098` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x4c { w.mm(id).pvars.resize(0x4c, 0); }
    match w.m(id).state {
        0 => {
            let m = w.mm(id);
            m.update_dist = 0xff;
            m.rotation[0] = c::sub_rot(m.rotation[0], std::f32::consts::FRAC_PI_4);
            m.state = 1;
            c::set_pi32(w, id, 0x48, -1);
        }
        2 => {
            let (mut t, mut v) = (c::pf(w, id, 0x2c), c::pf(w, id, 0x30));
            turn::spring(1.0, DT2, DT2, DT, &mut t, &mut v);
            c::set_pf(w, id, 0x2c, t);
            c::set_pf(w, id, 0x30, v);
            let a = v * std::f32::consts::FRAC_PI_4;
            w.mm(id).rotation[0] = c::add_rot(w.m(id).rotation[0], a);
            for o in [0x38, 0x3c] {
                if let Some(d) = story::link(w, pi(w, id, o)) {
                    w.mm(d).rotation[0] = c::sub_rot(w.m(d).rotation[0], a);
                    w.build_matrix(d);
                }
            }
            if 1.0 <= t {
                w.mm(id).state = 3;
                w.play_sound(1, 0, id);
                story::set_flag(w, story::flag_index(0x13d3aa), 1);
            }
        }
        3 => {
            if 3.7 <= c::dist2(super::hero_pos(w), c::pos(w, id)) { return; }
            let r = w.svc.interact.try_prompt(PROMPT_OWNER, PROMPT_MSG);
            if w.hero.loop_in.pad.pressed & 0x10 == 0 || r == 0 { return; }
            w.play_sound(2, 0, id);
            let up = w.svc.interact.prompt_hud;
            w.svc.interact.prompt.release(PROMPT_OWNER, up);
            w.mm(id).state = 4;
            let t = w.ticks(30);
            c::set_pi32(w, id, 0x40, t);
        }
        4 => {
            if c::dec_timer_pvar_i32(w, id, 0x40) != 0 {
                if let Some((p, e)) = cam_at(w, pi(w, id, 0x00), 0.0) {
                    w.mm(id).state = 5;
                    c::set_pf(w, id, 0x2c, 0.0);
                    c::set_pf(w, id, 0x30, 0.0);
                    let f = w.ticks(0xf);
                    crate::cinematic::fade_to_black(w, f);
                    let t = w.ticks(300);
                    crate::cinematic::camera_script(w, p, e, 2, t, false);
                    crate::cinematic::script_springs(w, SPRINGS);
                    crate::cinematic::camera_targets(w, Some(p), Some(e));
                    crate::cinematic::letterbox(w, true);
                }
            }
            let (t10, left) = (w.ticks(10), c::pi32(w, id, 0x40));
            if left < t10 { crate::cinematic::set_fade(w, 1.0 - left as f32 / t10 as f32); }
        }
        5 => {
            let t10 = w.ticks(10);
            let n = c::pi32(w, id, 0x40);
            if n < t10 {
                c::set_pi32(w, id, 0x40, n + 1);
                crate::cinematic::set_fade(w, 1.0 - (n + 1) as f32 / t10 as f32);
            }
            let old = c::pf(w, id, 0x2c);
            let (mut t, mut v) = (old, c::pf(w, id, 0x30));
            turn::spring(2.0, DT2, DT2, DT * 0.5, &mut t, &mut v);
            c::set_pf(w, id, 0x2c, t);
            c::set_pf(w, id, 0x30, v);
            if let Some((p, e)) = cam_at(w, pi(w, id, 0x00), t * 18.0) { crate::cinematic::camera_targets(w, Some(p), Some(e)); }
            let (lo, hi) = (old * 100.0 * 0.017_453_292, t * 100.0 * 0.017_453_292);
            for k in 0..9 {
                let Some((p, e)) = story::cuboid(w, pi(w, id, 0x08 + 4 * k)) else { continue };
                if lo < e[2] && e[2] < hi {
                    burst(w, p);
                    w.play_level_def(0, 0, id);
                }
            }
            if t < 1.075 { return; }
            story::teleport_to(w, pi(w, id, 0x04), 0x72, false);
            let f = w.ticks(10);
            crate::cinematic::fade_to_black(w, f);
            w.mm(id).state = 6;
            crate::cinematic::camera_script2(w, 3);
            if let Some((p, e)) = arrival_cam(w, id) {
                let t = w.ticks(300);
                crate::cinematic::camera_script(w, p, e, 2, t, false);
            }
            let ticks = w.ticks(0x2d);
            w.shake_camera(crate::follow_camera::ShakeRequest { axis: crate::follow_camera::ShakeAxis::Up, amp: f32::from_bits(0x3e99_999a), ticks });
            w.play_level_def(0, 0, id);
        }
        6 => {
            let (mut t, mut v) = (c::pf(w, id, 0x2c), c::pf(w, id, 0x30));
            turn::spring(2.0, DT2, DT2, DT + DT, &mut t, &mut v);
            c::set_pf(w, id, 0x2c, t);
            c::set_pf(w, id, 0x30, v);
            if let Some((p, e)) = arrival_cam(w, id) { crate::cinematic::camera_targets(w, Some(p), Some(e)); }
            if t < 2.0 { return; }
            w.mm(id).state = 7;
            let n = w.ticks(0xe10);
            c::set_pi32(w, id, 0x44, n);
            story::teleport_to(w, pi(w, id, 0x04), 0, false);
            crate::cinematic::camera_script2(w, 3);
            if let Some(m) = story::link(w, pi(w, id, 0x34)) { w.mm(m).state = 2; }
        }
        _ => {}
    }
}
