//! **Gaspar's cannons, class 1201** (level09 `0x304c80`, census U308; 3 placed: #787–#789), **their bases 324**
//! (`0x2efe40`: an empty update) and **their shells 1258** (`0x307d68`, made by `0x307ba8`). Ratchet steps onto a
//! cannon, it sinks a little and turns toward him with a blinking seat light; standing in its seat it takes him (hero
//! state 0x32), the script camera behind the barrels, and he turns it with the stick and fires with ○, the two
//! barrels in turn; △ lets him off.
//!
//! **The cannon** (`0x304c80`): every tick mode bit 4 off and the seat = joint 3 + 0.4 up.
//! * 0: the manipulator on joint list 0 (+0x00), the base 324 made at its position (`0x2efda8`: its distances,
//!   rotation and light), → 4; +0x5c the rest height, the pitch +0x44 and yaw 0, the light −1, the timers 0.
//! * The mount (states ≥ 4, cmd +0xbc): 0 and Ratchet standing on it within 1.2 of the seat, its timer +0x42 out
//!   (not in movement groups 0x14 / 7 or state 0x3d) → cmd 1, `SetState(0x32, 1)`. 1: Ratchet in the seat (0.7 behind,
//!   the cannon's yaw + π/2, his motion cleared); △ → `CameraScript2(0)`, `SetState(0, 1)`, cmd 2, → 4, anim 0;
//!   else → 5 (on entering: `CameraScript(pos, rot, 0, 0, 0)`, the camera delay +0x52 = 2, anim 5 over `ticks(5)`).
//!   2: once Ratchet is off it or 1.2 from the seat: cmd 0, +0x42 = `ticks(60)`.
//! * 4 (idle): Ratchet 4 or more away (xy) and not on it: it rises back to its rest height by dt a tick; the light
//!   freed (sound 2). Else it sinks toward rest − 1 (·0.11), turns toward him (+0x60 = 0.9·(+0x60 + 0.01·(bearing −
//!   yaw + π/2))) and the seat light (radius 0.4, colour 4) sits at rows·(0, −0.3, −0.1) + the seat (sound 1 when
//!   lit), blinking for its first 40 ticks.
//! * 5 (manned): it rises back by dt; the stick (y flipped with the camera setting; at most 1 long) speeds the yaw
//!   (π·dt² ·−x) and the pitch (π/4·dt² ·−y, held within ±30°), both slowing 3% a tick, the barrel joint turned by
//!   −pitch (`0x21fc08`); ○ held: pressed → anim 4 (3 after the left barrel), else in anim 3 / 4 at its key 1 (±0.25):
//!   a shell from joint 1 / 2 (`polar(60·dt, yaw + π/2, pitch)`, life `ticks(90)`), sound 0, the camera shakes 0.03
//!   for `ticks(10)` / `ticks(7)`, five sparks and a flash (type 27); past the key with the anim done: the other
//!   anim. ○ not held: anim 5 when the anim is done. The camera (after +0x52): matrix built, mode |= 4, its target
//!   joint 0 + `polar(−0.1, yaw + π/2, pitch)` + 0.2 up looking along (0, −pitch, yaw + π/2). The crosshair (FX 0xd,
//!   0xff0f0fff) at the screen centre.
//!
//! **The shell** (`0x307ba8`): ×4 the class scale, alpha 0x40, mode 0x200, facing its velocity, two type-26 glows
//! (400000 0x2f7f4f4f, 160000 0x4f7f7f7f, `5·life/4`); it whizzes (sound 0) once it starts moving away from Ratchet
//! within 3; deleted out of [2, 1021] or beyond 255 (xy) of the camera; armed (cmd 0) with the hit template (away from
//! Ratchet, flags 0x70001, damage 3) until its life runs out (then harmless for a quarter of it more); a hit along its
//! motion (`CollLine` ignoring its cannon): five sparks off the face (armed), a small beam explosion, the damage sphere
//! 1.1 (`coll_sphere_mobys` flags 0x10), deleted.
//!
//! Read from the level09 decomp and disassembly (the camera script's and the explosion's arguments). The crosshair
//! goes through `HeroFields::marker`. Native `f32`.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x304c80` | the cannon (module doc) | [`update`] |
//! | `0x2efda8(m, p)` / `0x2efe40` | the base 324 / its update | `base` / `empty::update` |
//! | `0x307ba8(m, p, v, life)` / `0x307d68` | the shell / its update | `shell` / [`shell_update`] |

use crate::follow_camera::{ShakeAxis, ShakeRequest};
use crate::moby_runtime::MobyId;
use crate::moby_update::creature::fx::{self, polar, Beam};
use crate::moby_update::creature::{self as c, add, add_rot, atan, len3, set_len3, sub, sub_rot, V, DT, DT2};
use crate::moby_update::manip;
use crate::moby_update::services::{pf, pv, reflect, HeroPose, HitTemplate, World};
use crate::moby_update::story;
use crate::point_lights::PointLight;
use crate::targeting::Marker;
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

pub const REFERENCE_LEVEL: u32 = 9;
pub const UPDATE_FN: u32 = 0x30_4c80;
pub const BASE_FN: u32 = 0x2e_fe40;
pub const SHELL_FN: u32 = 0x30_7d68;
pub const CLASSES: [i16; 1] = [1201];
pub const BASE_CLASSES: [i16; 1] = [324];
pub const SHELL_CLASSES: [i16; 1] = [1258];
/// Level 10's copy of the shell (`0x2e6a78`, spawner `0x2e68b8`; the same code: Orxon's turret 1346 fires it).
pub const ORXON_LEVEL: u32 = 10;
pub const ORXON_SHELL_FN: u32 = 0x2e_6a78;

const BASE: i16 = 0x144;
const SHELL: i16 = 0x4ea;
const LEN: usize = 0x70;
/// 30°.
const PITCH_MAX: f32 = std::f32::consts::FRAC_PI_6;
/// gp−0x4f40 (0, −0.3, −0.1): the seat light's offset; gp−0x4f50 / −0x4f4c: the camera's −0.1 back, 0.2 up.
const LIGHT_AT: [f32; 3] = [0.0, -0.3, -0.1];
const CAM_BACK: f32 = -0.1;
const CAM_UP: f32 = 0.2;

mod pvo {
    pub const MANIP: usize = 0x00;
    pub const T40: usize = 0x40;
    pub const MOUNT_T: usize = 0x42;
    pub const PITCH: usize = 0x44;
    pub const P48: usize = 0x48;
    pub const P4C: usize = 0x4c;
    pub const CAM_T: usize = 0x52;
    pub const BARREL: usize = 0x54;
    pub const REST: usize = 0x5c;
    pub const YAW_V: usize = 0x60;
    pub const PITCH_V: usize = 0x64;
    pub const LIGHT: usize = 0x68;
    pub const BLINK: usize = 0x6c;
}

mod spo {
    pub const VEL: usize = 0x00;
    pub const SPIN: usize = 0x10;
    pub const OWNER: usize = 0x18;
    pub const LIFE: usize = 0x1c;
    pub const LIFE0: usize = 0x1e;
    pub const DIST: usize = 0x20;
    pub const NEARED: usize = 0x24;
    pub const WHIZZED: usize = 0x28;
    pub const LEN: usize = 0x30;
}

fn hero_moby_pos(w: &World) -> V { w.hero_moby.map_or([0.0; 4], |h| w.m(h).position) }
fn hero_point(w: &World) -> V {
    let h = w.hero_point();
    [h[0], h[1], h[2], 0.0]
}
fn on_it(w: &World, id: MobyId) -> bool { w.hero.ground_moby == Some(id) }

/// `0x2efda8(m, p)`: the base 324.
fn base(w: &mut World, id: MobyId, p: V) {
    let Some(b) = w.create_moby(BASE) else { return };
    let (ud, rot, light, amb) = (w.m(id).update_dist, w.m(id).rotation, w.m(id).light, w.m(id).ambient);
    {
        let m = w.mm(b);
        m.update_dist = ud;
        m.visible = 1;
        m.draw_dist = ud as i16;
        m.position = p;
        m.rotation = rot;
        m.light = light;
        m.ambient = amb;
    }
    w.build_matrix(b);
}

/// Level09 `0x304c80` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, LEN);
    w.mm(id).mode &= !4;
    let mut seat = w.joint_point(id, 3);
    seat[2] += 0.4;
    let mut state = w.m(id).state;
    if 4 <= state {
        let out = c::dec_timer_pvar_s16(w, id, pvo::MOUNT_T) != 0;
        let busy = w.hero.group == 0x14 || w.hero.group == 7;
        let cmd = w.m(id).cmd;
        if out && cmd == 1 && !busy && w.hero.state != 0x3d {
            seated(w, id, seat);
            state = w.m(id).state;
        } else if cmd == 2 {
            let near = c::dist3(seat, hero_moby_pos(w)) <= 1.2;
            if !(near && on_it(w, id)) {
                w.mm(id).cmd = 0;
                let t = w.ticks(0x3c) as i16;
                c::set_pi16(w, id, pvo::MOUNT_T, t);
            }
        } else if !busy && w.hero.state != 0x3d && on_it(w, id) && c::dist3(seat, hero_moby_pos(w)) < 1.2 && c::pi16(w, id, pvo::MOUNT_T) == 0 {
            w.mm(id).cmd = 1;
            crate::cinematic::hero_state(w, 0x32, true);
        }
    }
    match state {
        0 => init(w, id),
        4 => idle(w, id, seat),
        5 => manned(w, id),
        _ => {}
    }
}

fn init(w: &mut World, id: MobyId) {
    manip::attach(w, id, 0, id, pvo::MANIP);
    let p = w.m(id).position;
    base(w, id, p);
    w.mm(id).state = 4;
    c::set_pf(w, id, pvo::REST, p[2]);
    c::set_pf(w, id, pvo::PITCH, 0.0);
    w.mm(id).rotation[2] = 0.0;
    c::set_pi32(w, id, pvo::LIGHT, -1);
    c::set_pi16(w, id, pvo::T40, 0);
    c::set_pi16(w, id, pvo::MOUNT_T, 0);
    c::set_pi32(w, id, pvo::P48, 0);
    c::set_pi32(w, id, pvo::P4C, 0);
}

/// cmd 1: Ratchet in the seat (module doc).
fn seated(w: &mut World, id: MobyId, seat: V) {
    let rot = w.m(id).rotation;
    let yaw = add_rot(FRAC_PI_2, rot[2]);
    {
        let f = w.hero_fields_mut();
        f.pose = Some(HeroPose { pos: [seat[0] + yaw.cos() * -0.7, seat[1] + yaw.sin() * -0.7, seat[2]], yaw, target_yaw: yaw });
        f.clear_motion = true;
    }
    if w.hero.loop_in.pad.pressed & crate::pad::button::TRIANGLE == 0 {
        if w.m(id).state != 5 {
            c::set_pi16(w, id, pvo::CAM_T, 2);
            let p = c::pos(w, id);
            crate::cinematic::camera_script(w, [p[0], p[1], p[2]], [rot[0], rot[1], rot[2]], 0, 0, false);
            if w.m(id).anim.seq_b != 5 {
                let t = w.ticks(5);
                w.anim_blend(id, 5, 0, t);
            }
        }
        w.mm(id).state = 5;
        return;
    }
    crate::cinematic::camera_script2(w, 0);
    crate::cinematic::hero_state(w, 0, true);
    w.mm(id).cmd = 2;
    w.mm(id).state = 4;
    w.anim_blend(id, 0, 0, 1);
}

fn rise(w: &mut World, id: MobyId) {
    let rest = c::pf(w, id, pvo::REST);
    let z = w.m(id).position[2];
    w.mm(id).position[2] = if z < rest { z + DT } else { rest };
}

fn idle(w: &mut World, id: MobyId, seat: V) {
    let p = c::pos(w, id);
    let h = hero_point(w);
    let d = ((p[0] - h[0]).powi(2) + (p[1] - h[1]).powi(2)).sqrt();
    if 4.0 <= d && !on_it(w, id) {
        rise(w, id);
        let slot = c::pi32(w, id, pvo::LIGHT);
        if slot == -1 { return; }
        w.play_sound(2, 0, id);
        w.svc.point_lights.free(slot as usize);
        c::set_pi32(w, id, pvo::LIGHT, -1);
        return;
    }
    let rest = c::pf(w, id, pvo::REST);
    let z = p[2] + ((rest - 1.0) - p[2]) * 0.11;
    w.mm(id).position[2] = z;
    let rz = w.m(id).rotation[2];
    let turn = add_rot(sub_rot(atan(h[0] - p[0], h[1] - p[1]), rz), FRAC_PI_2);
    let v = add_rot(c::pf(w, id, pvo::YAW_V), turn * 0.01) * 0.9;
    c::set_pf(w, id, pvo::YAW_V, v);
    w.mm(id).rotation[2] = add_rot(rz, v);
    let r = w.m(id).rows;
    let at: [f32; 3] = std::array::from_fn(|l| r[0][l] * LIGHT_AT[0] + r[1][l] * LIGHT_AT[1] + r[2][l] * LIGHT_AT[2] + seat[l]);
    let slot = c::pi32(w, id, pvo::LIGHT);
    if slot == -1 {
        c::set_pi32(w, id, pvo::BLINK, 0);
        let l = PointLight { color: [4.0, 4.0, 4.0], intensity: 0.0, pos: at, radius: f32::from_bits(0x3ecc_cccd) };
        let load = f32::from_bits(w.svc.frame_load[1].0);
        let got = w.svc.point_lights.alloc(l, load).map_or(-1, |i| i as i32);
        c::set_pi32(w, id, pvo::LIGHT, got);
        w.play_sound(1, 0, id);
        return;
    }
    let n = c::pi32(w, id, pvo::BLINK) + 1;
    c::set_pi32(w, id, pvo::BLINK, n);
    let Some(Some(l)) = w.svc.point_lights.slots.get_mut(slot as usize) else { return };
    l.pos = at;
    if n < 0x29 && n & 4 != 0 && (n < 0x19 || n & 10 != 10) {
        l.radius = 0.0;
        return;
    }
    l.color[0] = 4.0;
    l.radius = f32::from_bits(0x3ecc_cccd);
}

fn manned(w: &mut World, id: MobyId) {
    rise(w, id);
    let [sx, sy] = w.hero.stick.map(|x| f32::from_bits(x.0));
    let sy = if w.svc.help.cam_pitch_word == 0 { -sy } else { sy };
    let mut s = [sx, sy];
    let l = (s[0] * s[0] + s[1] * s[1]).sqrt();
    if 1.0 < l { s = [s[0] / l, s[1] / l]; }
    let yv = c::pf(w, id, pvo::YAW_V) + DT2 * PI * -s[0];
    let mut pv_ = c::pf(w, id, pvo::PITCH_V) + DT2 * FRAC_PI_4 * -s[1];
    let rz = add_rot(w.m(id).rotation[2], yv);
    w.mm(id).rotation[2] = rz;
    let mut pitch = add_rot(c::pf(w, id, pvo::PITCH), pv_);
    if !(-PITCH_MAX..=PITCH_MAX).contains(&pitch) {
        pitch = pitch.clamp(-PITCH_MAX, PITCH_MAX);
        pv_ = 0.0;
    }
    c::set_pf(w, id, pvo::PITCH, pitch);
    c::set_pf(w, id, pvo::PITCH_V, pv_ * 0.97);
    c::set_pf(w, id, pvo::YAW_V, yv * 0.97);
    manip::set_axis(w, id, id, pvo::MANIP, -pitch, 1);
    let (held, pressed) = (w.hero.loop_in.pad.held, w.hero.loop_in.pad.pressed);
    let (seq, seq_b) = (w.m(id).anim.seq_a, w.m(id).anim.seq_b);
    let done = w.m(id).anim.flags & 2 != 0;
    if held & 0x20 == 0 {
        if seq_b != 5 && done { w.anim_blend(id, 5, 0, 1); }
    } else if pressed & 0x20 != 0 {
        let s = if c::pi16(w, id, pvo::BARREL) != 0 { 3 } else { 4 };
        w.anim_blend(id, s, 0, 1);
    } else if seq == 3 || seq == 4 {
        let key = crate::moby_update::creature::ground::key_time(w, id);
        if (key - 1.0).abs() <= 0.25 {
            fire(w, id, if seq == 3 { 1 } else { 2 });
        } else if done {
            w.anim_blend(id, if seq == 3 { 4 } else { 3 }, 0, 1);
        }
    }
    if c::dec_timer_pvar_s16(w, id, pvo::CAM_T) != 0 {
        w.build_matrix(id);
        w.mm(id).mode |= 4;
        let jp = w.joint_point(id, 0);
        let yaw = add_rot(FRAC_PI_2, w.m(id).rotation[2]);
        let mut o = polar(CAM_BACK, yaw, pitch);
        o[2] += CAM_UP;
        let at = [jp[0] + o[0], jp[1] + o[1], jp[2] + o[2]];
        crate::cinematic::camera_targets(w, Some(at), Some([0.0, -pitch, yaw]));
    }
    let tick = w.counter;
    w.hero_fields_mut().marker = Some((tick, Marker { size: 1.0, angle: 0.0, rgba: 0xff0f_0fff, at: None, fx: 0xd }));
}

/// A shot from joint `j` (module doc).
fn fire(w: &mut World, id: MobyId, j: usize) {
    let yaw = add_rot(FRAC_PI_2, w.m(id).rotation[2]);
    let pitch = c::pf(w, id, pvo::PITCH);
    let o = polar(DT * 60.0, yaw, pitch);
    let vel = [o[0], o[1], o[2], 0.0];
    let muzzle = w.joint_point(id, j);
    let life = w.ticks(0x5a);
    shell(w, id, muzzle, vel, life);
    w.play_sound(0, 0, id);
    let amp = f32::from_bits(0x3cf5_c28f);
    let (t10, t7) = (w.ticks(10), w.ticks(7));
    w.shake_camera(ShakeRequest { axis: ShakeAxis::Up, amp, ticks: t10 });
    w.shake_camera(ShakeRequest { axis: ShakeAxis::Forward, amp, ticks: t7 });
    c::set_pi16(w, id, pvo::BARREL, if j == 1 { 0 } else { 1 });
    let speed = len3(vel);
    for _ in 0..5 {
        let x = w.rng.randf(-1.0, 1.0);
        let y = w.rng.randf(-1.0, 1.0);
        let z = w.rng.randf(-1.0, 1.0);
        let d = add(set_len3([x, y, z, 0.0], speed * f32::from_bits(0x3f26_6666)), vel);
        let s = w.rng.randf(DT * 3.0, DT * 6.0);
        let d = set_len3(d, s);
        let (a, b) = (w.ticks(5), w.ticks(10));
        let t = w.rng.rand_range(a, b);
        fx::part27(w, f32::from_bits(0x4743_5000), muzzle, d, 0x5f2f_4f6f, t);
    }
    let d = set_len3(vel, DT * 0.5);
    let (a, b) = (w.ticks(4), w.ticks(7));
    let t = w.rng.rand_range(a, b);
    fx::part27(w, f32::from_bits(0x48f4_2400), muzzle, d, 0x2f4f_7f7f, t);
}

/// `0x307ba8(m, p, v, life)`: a shell.
pub fn shell(w: &mut World, owner: MobyId, p: V, v: V, life: i32) {
    let Some(m) = w.create_moby(SHELL) else { return };
    story::pvars(w, m, spo::LEN);
    c::set_pi32(w, m, spo::OWNER, owner as i32 + 1);
    let lxy = (v[0] * v[0] + v[1] * v[1]).sqrt();
    let hp = hero_point(w);
    {
        let mo = w.mm(m);
        mo.rotation[0] = 0.0;
        mo.update_dist = 0xff;
        mo.draw_dist = 0xff;
        mo.visible = 1;
        mo.scale *= 4.0;
        mo.rotation[1] = -atan(lxy, v[2]);
        mo.rotation[2] = atan(v[0], v[1]);
        mo.position = p;
    }
    c::set_pv4(w, m, spo::VEL, v);
    let k = DT * FRAC_PI_2;
    let s = w.rng.randf(-k, k);
    c::set_pf(w, m, spo::SPIN, s);
    c::set_pi32(w, m, spo::SPIN + 4, 0);
    c::set_pi16(w, m, spo::LIFE, life as i16);
    c::set_pi16(w, m, spo::LIFE0, life as i16);
    c::set_pf(w, m, spo::DIST, c::dist3(p, hp));
    c::set_pi32(w, m, spo::WHIZZED, 0);
    c::set_pi32(w, m, spo::NEARED, 0);
    w.mm(m).mode |= 0x200;
    w.mm(m).alpha = 0x40;
    let n = life * 5 / 4;
    crate::moby_update::creature::projectile::part26(w, f32::from_bits(0x48c3_5000), m, 0x2f7f_4f4f, n);
    crate::moby_update::creature::projectile::part26(w, f32::from_bits(0x481c_4000), m, 0x4f7f_7f7f, n);
    w.build_matrix(m);
}

/// Level09 `0x307d68`: the shell (module doc).
pub fn shell_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, spo::LEN);
    let old = c::pos(w, id);
    let hp = hero_point(w);
    let d = c::dist3(old, hp);
    let v = c::pv4(w, id, spo::VEL);
    let p = add(old, v);
    c::set_pos(w, id, p);
    let prev = c::pf(w, id, spo::DIST);
    if c::pi32(w, id, spo::WHIZZED) == 0 && c::pi32(w, id, spo::NEARED) != 0 && prev < d && d < 3.0 {
        w.play_sound(0, 0, id);
        c::set_pi32(w, id, spo::WHIZZED, 1);
    }
    if d < prev { c::set_pi32(w, id, spo::NEARED, 1); }
    c::set_pf(w, id, spo::DIST, d);
    let inside = |x: f32| (2.0..=1021.0).contains(&x);
    let cam = w.camera.map(|x| f32::from_bits(x.0));
    let near = ((p[0] - cam[0]).powi(2) + (p[1] - cam[1]).powi(2)).sqrt() <= 255.0;
    if !(inside(p[0]) && inside(p[1]) && inside(p[2]) && near) {
        w.delete_moby(id);
        return;
    }
    let armed = w.m(id).cmd == 0;
    let tmpl = armed.then(|| {
        let mut dir = set_len3(sub(p, hp), 1.0);
        dir[2] = 1.0;
        dir[3] = f32::from_bits(0x45af_df66);
        HitTemplate { dir: pv(dir), attacker: Some(id), flags: 0x7_0001, b18: 1, b19: 3, h1a: w.m(id).o_class as u16, damage: pf(3.0), w20: 1 }
    });
    let mut life = c::pi16(w, id, spo::LIFE);
    let out = crate::moby_update::services::fast_dec_timer_s16(&mut life) != 0;
    c::set_pi16(w, id, spo::LIFE, life);
    if out {
        if !armed {
            w.delete_moby(id);
            return;
        }
        w.mm(id).cmd = 1;
        let l0 = c::pi16(w, id, spo::LIFE0);
        c::set_pi16(w, id, spo::LIFE, l0 / 4);
    }
    let owner = usize::try_from(c::pi32(w, id, spo::OWNER) - 1).ok().filter(|&m| m < w.table.mobys.len());
    let h = match &tmpl {
        Some(t) => crate::moby_update::services::line_hit_in(w.table, w.svc, w.classes, w.coll, pv(old), pv(p), 0, owner, t),
        None => w.coll_line(pv(old), pv(p), 0, owner),
    };
    let Some(h) = h else { return };
    let hit = [h.point[0], h.point[1], h.point[2], p[3]];
    c::set_pos(w, id, hit);
    if w.m(id).cmd == 0 {
        for _ in 0..5 {
            let x = w.rng.randf(-1.0, 1.0);
            let y = w.rng.randf(-1.0, 1.0);
            let z = w.rng.randf(-1.0, 1.0);
            let r = reflect(pv(v), pv([h.normal[0], h.normal[1], h.normal[2], 0.0])).map(|q| f32::from_bits(q.0));
            let mut q = set_len3([x, y, z, 0.0], len3(r) * 0.5);
            q = add(q, r);
            let s = w.rng.randf(DT * 3.0, DT * 6.0);
            q = set_len3(q, s);
            let (a, b) = (w.ticks(10), w.ticks(0xf));
            let t = w.rng.rand_range(a, b);
            fx::part27(w, f32::from_bits(0x476a_6000), hit, q, 0x7f2f_4f6f, t);
        }
    }
    const B: Beam = Beam { damage_r: 0.0, damage: 0.0, flash: f32::from_bits(0x3f8c_cccd), flash2: f32::from_bits(0x3f19_999a), flash_dist: 1.0, scale: f32::from_bits(0x3f66_6666), light: 0.0, streaks: 4, sparks: 3, puffs: 6, debris: 1, sound: -1, shake: false };
    fx::beam_explosion(w, &B, Some(id), hit);
    w.sphere_mobys(pf(f32::from_bits(0x3f8c_cccd)), pv(hit), 0x10, Some(id), tmpl.as_ref());
    w.delete_moby(id);
}
