//! **The Visibomb's guided missile**, class 172 (`0xac`; level01 update `0x2cbda8`, which has no Ghidra function: read
//! from the disassembly, 674 instructions), its launch `0x2cb540`, the end of its flight `0x2cb788`, the missile
//! view's look `0x2cb338` / `0x2cb458`, its glow `0x2cb808` and its explosion `0x2cb968` (docs/plan/hero_gameplay.md
//! §17, with the coverage table). The launcher is the hand item (`crate::hero::visibomb`, item 13, gun class 163); the
//! view is the type-6 camera (`crate::follow_camera::type6`); Ratchet waits in state 0x1d (`crate::hero::scripted`);
//! the range limiter 832 (`super::rc_range`) shows the static near the edge and ends flights that stay out.
//!
//! **Pvars** (the game's layout): +0x00 the gun (the hand item: not a table moby in the port), +0x04 the speed, +0x10
//! the flight timer (`ticks(3000)` at the launch; −1 / −`ticks(100)` after the explosion, counting down to
//! −`ticks(100)`), +0x14 the camera (1 while the type-6 camera is ours), +0x18 / +0x1c the smoothed stick x / y,
//! +0x20 the glow (moby + 1), +0x24 / +0x28 the fins' joint manipulators ([`fins`]), +0x2c the loop
//! voice, +0x30..+0x42 the saved fog (kept by the engine, [`View`]), +0x44 the height above the ground (0.4), +0x48
//! the target speed. Moby +0xbc: explode now (1 a face or ○, 2 a moby).
//!
//! **Launch** ([`launch`], `0x2cb540(yaw, pitch, gun, point)`): `CreateMoby(0xac)`, 0x17e988 = 1 (the HUD skips a
//! frame), 0x141330 = the missile, distances 0xff, state 1, visible, rotation (0, pitch, yaw), at `point`; speed 0
//! (a moving ground moby's speed: the port never has one), timer `ticks(3000)`, +0x44 = 0.4; the missile view's look
//! ([`view_on`], `0x2cb338`); `MobyBuildMatrix`; the type-6 camera switched in (`0x317d88`); `SetState(0x1d, 1)` (made by
//! the gun right after its update: `crate::hero::visibomb`); the glow 179 (`0x2d0fc8`, [`super::pyro_glow::init`]);
//! target speed 24·dt; Ratchet's light and ambient (`0x272078`); the loop voice (class sound 0, flags 4); a line from
//! the gun (at the point's height) to the point that hits (flags 0, Ratchet ignored) starts it exploding there
//! (+0xbc = 1); 0x15f608 = 1 (the occlusion shows everything this frame); `force_help_message(6, 0)`.
//!
//! **Update** ([`update`], `0x2cbda8`), every tick: 0x15f608 = 1, 0x15f458 = 1 (`Globals::missile_view_at`: the liquid grids keep the level fog that frame), the help box suspended
//! (`0x225a28`), 0x17e988 = 1, `force_help_message(6, 0)`; the timer steps; ended ([`end_flight`]) when it reached
//! −`ticks(100)`, when Ratchet is hurt, dead or held (states 0x16 / 0x3d / 0x72 / 0x65), in movement groups 2, 3, 4,
//! 5, 7, 0xa, 0x10 or ≥ 0xd, or not on the ground (0x13f650 = 0). After the flight (timer < 0) the camera orbits the
//! blast (`0x317aa0`), the look is restored (`0x2cb458(m, 0)`) and the glow follows. In flight: the target speed eases
//! 10 % a tick to 24·dt (34·dt with ✕ held), the speed 1 % a tick toward it (at most 34·dt); the loop voice's pitch
//! bend `trunc((target − 24dt/(34dt − 24dt))·100)` (the game's expression); after the first 5 ticks the stick steers:
//! the smoothed stick eases 10 % a tick, the roll 6 % a tick toward the smoothed x, the pitch turns by y·0.7° a tick
//! (within ±89.94°), the yaw by x·0.7° a tick; the blob shadow (0x26eec8(0.2): `crate::shadows::blob`). The step
//! `polar(speed, yaw, −pitch)`; for the first 60 ticks it rises over the ground (below ground + 0.4: up at 6·dt a
//! tick and pitched up by `atan(speed, gap)`, at most 90°·dt, unless over a moby that is not a creature); out of
//! 4.5..1018.5 on any axis the flight ends. The glow; the path is tested (`CollLine_Fix(old, new, 0, Ratchet for the
//! first 40 ticks then itself, tmpl)`: push along the step with z 1, 5627.92; flags 0x830000, type 3 / 2, damage 6,
//! class 172): a moby → explode (2; through Ratchet: no blast), a face → explode (1); either releases the loop voice;
//! nothing with the timer at 0 → the flight ends quietly. After 60 ticks ○ explodes it (1). The explosion: unless it
//! hit Ratchet, `SpawnBeamExplosion` (flashes 4 / 2, light 20, 10 streaks, 3 spark pairs, 16 puffs, one fireball, the
//! class sound 1, the camera shake: [`crate::moby_update::creature::fx::beam_explosion`]) and the blast (sphere 4:
//! damage 6, push 1 / 1 up, flags 0x830000, type 3 / 2: [`crate::moby_update::creature::attack::area_hit`]); the
//! missile hidden; speed 0.02, stick x ·0.01 (the orbit's steps); timer −`ticks(100)` within 30 ticks of the launch,
//! else −1; five times the explosion effects ([`explosion`]) at points within 1.5 of the blast. Last, the camera
//! tracks (`0x317aa0`).
//!
//! **End of the flight** ([`end_flight`], `0x2cb788`, also from the range limiter): 0x141330 = 0, the camera handed back
//! (`0x317e70`), `SetState(0, 1)`, the look restored with the far distances (`0x2cb458(m, 1)`), the help box resumed
//! (`0x225a88`), the missile deleted (its loop voice with it), the underwater flag 0x167494 cleared.
//!
//! Native `f32`; the game's `rand` draws in its order.

use crate::follow_camera::type6::{Call, Orbit, Target};
use crate::menus::screen_static::StaticDraw;
use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{atan, fx, turn::approach};
use crate::moby_update::manip;
use crate::moby_update::services::{self as sv, pvar as p, HitTemplate, World};
use crate::ps2v::Pf;
use crate::targeting::polar;
use std::f32::consts::{FRAC_PI_2, PI};

pub const UPDATE_FN: u32 = 0x2cbda8;
pub const CLASS: i16 = 172;
pub const CLASSES: [i16; 1] = [CLASS];
/// `0x15ed6c` dt and `0x15ed60` the speed scale (NTSC).
const DT: f32 = 1.0 / 60.0;
const SPEED_K: f32 = 1.0;

pub mod pv {
    pub const SPEED: usize = 0x04;
    pub const TIMER: usize = 0x10;
    pub const CAMERA: usize = 0x14;
    pub const STICK_X: usize = 0x18;
    pub const STICK_Y: usize = 0x1c;
    pub const GLOW: usize = 0x20;
    pub const VOICE: usize = 0x2c;
    pub const HEIGHT: usize = 0x44;
    pub const TARGET_SPEED: usize = 0x48;
    /// The fins' two manipulator records: the game keeps them in a global pair (+0x24 / +0x28 point at base + 0x1f60 /
    /// + 0x1fa0); the port keeps them past the game's block (one missile flies at a time).
    pub const FINS: [usize; 2] = [0x50, 0x90];
    pub const SIZE: usize = 0xd0;
}

/// gp−0x573c / gp−0x5738: the cruise and the ✕ speeds (u/s); gp−0x5734 / gp−0x5730 their easing; gp−0x5740 the pitch
/// bend's scale.
pub const CRUISE: f32 = 24.0;
pub const BOOST: f32 = 34.0;
/// The steering rate 0.7°·[0x15ed60] a tick (0x3c490fdb).
pub const TURN: f32 = 0.012_271_847;
/// The pitch limit (0x3fc8ef16) past ±90° (0x3fc90fdb).
pub const PITCH_LIMIT: f32 = 1.569_8;
/// The flight's length (`ticks(3000)`), the steering delay (`ticks(5)`), the terrain-following / ○ window
/// (`ticks(60)`), Ratchet ignored by the path test (40 raw ticks), the short-flight window (`ticks(30)`), the end of
/// the orbit (`ticks(100)`).
pub const FLIGHT: i32 = 3000;
/// The bounds of the level box (4.5 .. 1018.5).
pub const BOX: (f32, f32) = (4.5, 1018.5);
/// The camera centre's offset along the missile's up row (`0x317aa0`: (−0, 0, 0.12)).
pub const CENTRE_UP: f32 = 0.12;
/// The glow's offset along its combined y row (`0x2cb808`: (0, 0.11, 0)) and its scale target.
pub const GLOW_UP: f32 = 0.11;
pub const GLOW_SCALE: f32 = 0.45;
/// `0x2cb338`'s fog, clear colour and far distances.
pub const FOG_COLOUR: [u8; 3] = [0x40, 0x60, 0x40];
pub const FOG_FAR: f32 = 131_072.0;
pub const FOG_NEAR_F: f32 = 255.0;
pub const FAR_UNITS: f32 = 144.0;
/// `0x20aab8` / `0x20aad0`: the explosion rings' colours (`randi(6)` each).
const COL_A: [u32; 6] = fx::SPARK_A;
const COL_B: [u32; 6] = fx::SPARK_B;

/// A write of the missile view's look into the level fog globals 0x15f444..0x15f454 (the engine owns them).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FogWrite {
    #[default]
    None,
    /// `0x2cb338`: the globals saved, then the green fog ([`FOG_COLOUR`], near 0, far [`FOG_FAR`], F 255 → 0).
    Swap,
    /// `0x2cb458(m, full)`: the saved globals back; `full` also the far distances; `underwater_off` the end of the
    /// flight's 0x167494 = 0.
    Restore { full: bool, underwater_off: bool },
}

/// The missile view's render state (the globals `0x2cb338` / `0x2cb458` write), for the engine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct View {
    /// 0x15f30c: the scanline overlay (`DrawWorld`'s `0x21b9f8` with the record 0x16cc00 / 0x16cc30) and the range
    /// static are drawn.
    pub overlay: bool,
    /// 0x16a478 = 0: no sky; the frame is cleared with the background colour.
    pub no_sky: bool,
    /// `SetBackgroundColor`: Some while the view is on (the level's colour otherwise).
    pub background: Option<[u8; 3]>,
    /// 0x160f80 / 0x160fe0 / 0x1604a4 / 0x15fff0 at 144 units (until the full restore).
    pub short_far: bool,
    /// The fog writes: `fog_seq` counts them, `fog` is the last ([`Globals::fog_log`] keeps the recent ones for the
    /// engine, which applies each new one in order).
    pub fog_seq: u32,
    pub fog: FogWrite,
}

/// Fog writes kept in [`Globals::fog_log`] (the engine reads them every frame; a frame runs at most two ticks).
pub const FOG_LOG: usize = 16;

/// The Visibomb's globals the rest of the frame reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Globals {
    /// 0x141330: the missile in flight.
    pub missile: Option<MobyId>,
    /// 0x17e988 = 1 in the tick with this counter: `HudDraw` skips the HUD (and clears it) that frame.
    pub hud_off_at: Option<u64>,
    /// 0x15f458 = 1 in the tick with this counter (`UpdateFog` clears it every frame): the liquid grids' set-up keeps the
    /// level fog and its colour (`rc-engine` sea_render; level09 `0x2c1978`).
    pub missile_view_at: Option<u64>,
    pub view: View,
    /// The range limiter's static (`0x302438`), the quads its draw made in the frame of `static_at`.
    pub static_quads: Vec<StaticDraw>,
    pub static_at: Option<u64>,
    /// The last [`FOG_LOG`] fog writes with their sequence numbers ([`View::fog_seq`]).
    pub fog_log: Vec<(u32, FogWrite)>,
}

impl Globals {
    fn fog(&mut self, f: FogWrite) {
        self.view.fog_seq = self.view.fog_seq.wrapping_add(1);
        self.view.fog = f;
        if self.fog_log.len() == FOG_LOG { self.fog_log.remove(0); }
        self.fog_log.push((self.view.fog_seq, f));
    }

    /// The fog writes after sequence number `seen`, oldest first.
    pub fn fog_since(&self, seen: u32) -> impl Iterator<Item = &(u32, FogWrite)> { self.fog_log.iter().filter(move |(q, _)| q.wrapping_sub(seen) as i32 > 0) }
}

fn v3(a: [f32; 4]) -> [f32; 3] { [a[0], a[1], a[2]] }
fn f4(a: [f32; 3]) -> [Pf; 4] { [Pf::f(a[0]), Pf::f(a[1]), Pf::f(a[2]), Pf::ZERO] }
/// `fast_add_rotations` / `fast_subtract_rotations`: wrapped into [−π, π).
fn wrap(a: f32) -> f32 {
    let t = std::f32::consts::TAU;
    let mut x = a % t;
    if x >= PI { x -= t; }
    if x < -PI { x += t; }
    x
}

/// `0x2cb338`: the missile view's look (module docs).
pub fn view_on(w: &mut World) {
    let g = &mut w.svc.visibomb;
    g.view.overlay = true;
    g.view.no_sky = true;
    g.view.short_far = true;
    g.view.background = Some(FOG_COLOUR);
    g.fog(FogWrite::Swap);
}

/// `0x2cb458(m, full)`: the look restored.
pub fn view_off(w: &mut World, full: bool, underwater_off: bool) {
    let g = &mut w.svc.visibomb;
    g.view.overlay = false;
    g.view.no_sky = false;
    g.view.background = None;
    if full { g.view.short_far = false; }
    g.fog(FogWrite::Restore { full, underwater_off });
}

/// The per-frame flags every missile tick sets (0x15f608, 0x17e988; the help box suspended; the context prompt held).
fn frame_flags(w: &mut World) {
    let c = w.counter;
    w.svc.occlusion_fallback = Some((c, 1));
    w.svc.help.suspend();
    w.svc.visibomb.hud_off_at = Some(c);
    w.svc.interact.force_prompt(6, 0);
}

/// `0x2cb540(yaw, pitch, gun, point)` (module docs). `gun`: the hand item's position (+0x10). Returns the missile.
pub fn launch(w: &mut World, yaw: f32, pitch: f32, gun: [f32; 3], point: [f32; 3]) -> Option<MobyId> {
    let id = w.create_moby(CLASS)?;
    w.svc.visibomb.hud_off_at = Some(w.counter);
    w.svc.visibomb.missile = Some(id);
    let t3000 = w.ticks(FLIGHT);
    {
        let m = w.mm(id);
        if m.pvars.len() < 0x80 { m.pvars.resize(0x80, 0); }
        m.update_dist = 0xff;
        m.draw_dist = 0xff;
        m.state = 1;
        m.rotation = [0.0, pitch, yaw, m.rotation[3]];
        m.visible = 1;
        m.position = [point[0], point[1], point[2], m.position[3]];
        p::set_ff(&mut m.pvars, pv::SPEED, 0.0);
        p::set_i32(&mut m.pvars, pv::TIMER, t3000);
        p::set_ff(&mut m.pvars, pv::STICK_X, 0.0);
        p::set_ff(&mut m.pvars, pv::STICK_Y, 0.0);
        p::set_i32(&mut m.pvars, pv::CAMERA, 0);
        p::set_ff(&mut m.pvars, pv::HEIGHT, f32::from_bits(0x3ecc_cccd));
    }
    view_on(w);
    w.build_matrix(id);
    let t = target(w, id, t3000);
    crate::cinematic::camera_type6(w, Call::Switch(t));
    p::set_i32(&mut w.mm(id).pvars, pv::CAMERA, 1);
    // SetState(0x1d, 1): made by the gun right after its update (crate::hero::visibomb). The fins' records cleared
    // (`0x220f50(rec, 0, 0x40)`) and attached on the joint lists 0 / 1.
    if w.m(id).pvars.len() < pv::SIZE { w.mm(id).pvars.resize(pv::SIZE, 0); }
    for (list, ofs) in pv::FINS.into_iter().enumerate() {
        w.mm(id).pvars[ofs..ofs + manip::rec::NODE].fill(0);
        manip::attach(w, id, list as u8, id, ofs);
    }
    if let Some(g) = w.create_moby(super::pyro_glow::CLASS) {
        let rows = w.m(id).rows;
        super::pyro_glow::init(w.mm(g), [0, 1, 2].map(|i| [rows[i][0], rows[i][1], rows[i][2]]), None);
        p::set_i32(&mut w.mm(id).pvars, pv::GLOW, g as i32 + 1);
    }
    p::set_ff(&mut w.mm(id).pvars, pv::TARGET_SPEED, CRUISE * DT);
    // 0x272078: Ratchet's light word and ambient.
    if let Some(h) = w.hero_moby {
        let (l, a) = (w.m(h).light, w.m(h).ambient);
        let m = w.mm(id);
        m.light = l;
        m.ambient = a;
    }
    let voice = w.play_sound(0, 4, id);
    p::set_i32(&mut w.mm(id).pvars, pv::VOICE, voice);
    let from = [gun[0], gun[1], point[2]];
    if let Some(h) = w.coll_line(f4(from), f4(point), 0, w.hero_moby) {
        let m = w.mm(id);
        m.cmd = 1;
        m.position = [h.point[0], h.point[1], h.point[2], m.position[3]];
    }
    w.svc.occlusion_fallback = Some((w.counter, 1));
    w.svc.interact.force_prompt(6, 0);
    Some(id)
}

/// What the type-6 camera's tracking reads of the missile (`0x317aa0`): the centre 0.12 up its up row (the rows as
/// its last `MobyBuildMatrix` left them), its Euler and timer.
fn target(w: &World, id: MobyId, timer: i32) -> Target {
    let m = w.m(id);
    let up = [m.rows[2][0], m.rows[2][1], m.rows[2][2]];
    let pos = v3(m.position);
    Target { centre: std::array::from_fn(|k| pos[k] + up[k] * CENTRE_UP), euler: [m.rotation[0], m.rotation[1], m.rotation[2]], timer, orbit: Orbit::default() }
}

/// `0x317aa0(camera, missile)` as the missile makes it: in flight the target; after, the orbit's steps from the
/// missile's decaying pvars (+0x18 the roll step ×0.97 a call; +0x04 the push. With Ratchet or the hand item (the
/// nearer) within 0.77 of the camera (xy): the distance pulled in by `0.7 − d` and the push zeroed when the camera
/// looks away from him (`FastDiffRots(atan(camera − Ratchet), camera yaw) < 90°`), else pushed out by `0.7 − d`;
/// otherwise pushed by the push, ×0.97 a call).
fn track(w: &mut World, id: MobyId) {
    if p::i32(&w.m(id).pvars, pv::CAMERA) == 0 { return; }
    let timer = p::i32(&w.m(id).pvars, pv::TIMER);
    let mut t = target(w, id, timer);
    if timer < 1 {
        let roll = p::ff(&w.m(id).pvars, pv::STICK_X);
        p::set_ff(&mut w.mm(id).pvars, pv::STICK_X, roll * 0.97);
        let cam = v3(w.camera.map(|x| x.to_f32()));
        let hero = crate::hero::physics::to_f32x3(w.hero.pos);
        let a = atan(cam[0] - hero[0], cam[1] - hero[1]);
        let away = crate::moby_update::creature::diff_rots(a, w.hero.loop_in.cam_euler[2]) < FRAC_PI_2;
        // `VecDistance2` 0x221398 (fun_001f9b80): the xy distance.
        let dist = |q: [f32; 3]| ((q[0] - cam[0]).powi(2) + (q[1] - cam[1]).powi(2)).sqrt();
        let mut near = dist(hero);
        if let Some(it) = w.hero.items.slot.item.as_ref() {
            let d = dist(it.position);
            if d <= near { near = d; }
        }
        let speed = p::ff(&w.m(id).pvars, pv::SPEED);
        let step = if near < 0.77 {
            if away {
                p::set_ff(&mut w.mm(id).pvars, pv::SPEED, 0.0);
                -(0.7 - near)
            } else {
                0.7 - near
            }
        } else {
            p::set_ff(&mut w.mm(id).pvars, pv::SPEED, speed * 0.97);
            speed
        };
        t.orbit = Orbit { roll, dist: step };
    }
    crate::cinematic::camera_type6(w, Call::Track(t));
}

/// `0x2cb808(m, pvars)`: the glow eased to 0.45 of its size, turned −90° about its z and placed 0.11 along the
/// combined y row from the missile; its owner fields (the missile's state and hidden bit, the hand slot's ready ticks).
fn glow_frame(w: &mut World, id: MobyId) {
    let Some(g) = (p::i32(&w.m(id).pvars, pv::GLOW) as usize).checked_sub(1) else { return };
    use crate::hero::pyrocitor::glow_pv as gp;
    let m = w.m(id);
    let (rows, pos, state, hidden) = (m.rows, v3(m.position), m.state, (m.mode & 1) as u8);
    let ready = w.hero.items.slot.ticks_ready.clamp(0, i16::MAX as i32) as i16;
    let Some(gm) = w.table.mobys.get_mut(g) else { return };
    if gm.pvars.len() < 0x10 { gm.pvars.resize(0x10, 0); }
    let s = p::ff(&gm.pvars, gp::SCALE);
    if GLOW_SCALE < s {
        p::set_ff(&mut gm.pvars, gp::SCALE, s - SPEED_K * 0.05);
    } else if s < GLOW_SCALE {
        p::set_ff(&mut gm.pvars, gp::SCALE, s + SPEED_K * 0.05);
    }
    gm.rotation[2] = SPEED_K * -FRAC_PI_2;
    let e = crate::follow_camera::script::euler_rows([gm.rotation[0], gm.rotation[1], gm.rotation[2]]);
    let r: [[f32; 3]; 3] = [0, 1, 2].map(|i| [rows[i][0], rows[i][1], rows[i][2]]);
    let c: [[f32; 3]; 3] = std::array::from_fn(|i| std::array::from_fn(|k| e[i][0] * r[0][k] + e[i][1] * r[1][k] + e[i][2] * r[2][k]));
    for (row, ci) in gm.rows.iter_mut().zip(c) { *row = [ci[0], ci[1], ci[2], 0.0]; }
    gm.position = [pos[0] + c[1][0] * GLOW_UP, pos[1] + c[1][1] * GLOW_UP, pos[2] + c[1][2] * GLOW_UP, gm.position[3]];
    gm.pvars[gp::OWNER_STATE] = state;
    gm.pvars[gp::OWNER_HIDDEN] = hidden;
    gm.pvars[gp::READY..gp::READY + 2].copy_from_slice(&ready.to_le_bytes());
}

/// Releases the loop voice when it still plays the missile's sound (`release_voice_slot` behind the owner / state
/// test); +0x2c = −1.
fn release_voice(w: &mut World, id: MobyId) {
    let v = p::i32(&w.m(id).pvars, pv::VOICE);
    if v != -1 { w.release_sound(v, id); }
    p::set_i32(&mut w.mm(id).pvars, pv::VOICE, -1);
}

/// `0x2cb788(m, pvars)` (module docs).
pub fn end_flight(w: &mut World, id: MobyId) {
    w.svc.visibomb.missile = None;
    if p::i32(&w.m(id).pvars, pv::CAMERA) != 0 { crate::cinematic::camera_type6(w, Call::Release); }
    p::set_i32(&mut w.mm(id).pvars, pv::CAMERA, 0);
    crate::cinematic::hero_state(w, 0, true);
    view_off(w, true, true);
    w.svc.help.resume();
    // The glow reads its owner's state next tick (deleted: it deletes itself).
    if let Some(g) = (p::i32(&w.m(id).pvars, pv::GLOW) as usize).checked_sub(1) {
        if let Some(gm) = w.table.mobys.get_mut(g) {
            if gm.pvars.len() > crate::hero::pyrocitor::glow_pv::OWNER_STATE { gm.pvars[crate::hero::pyrocitor::glow_pv::OWNER_STATE] = 0xfd; }
        }
    }
    w.delete_moby(id);
}

/// The fins in flight (0x2cc324..0x2cc3ec, after the glow): with the smoothed stick (x +0x18, y +0x1c) and the roll
/// (moby +0x40), `FUN_00221e38(−0.7·(0.5·y + |x/3| ± 1.1·(x − roll)), fin + 0x10, 1)` for the fins on lists 0 (+) and
/// 1 (−): they tilt about y against the steering.
pub fn fins(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { w.mm(id).pvars.resize(pv::SIZE, 0); }
    let m = w.m(id);
    let (x, y, roll) = (p::ff(&m.pvars, pv::STICK_X), p::ff(&m.pvars, pv::STICK_Y), m.rotation[0]);
    let (half, k, s) = (0.5f32, f32::from_bits(0x3f8c_cccd), f32::from_bits(0x3f33_3333));
    for (ofs, sign) in pv::FINS.into_iter().zip([1.0f32, -1.0]) {
        let a = -((y * half + (x / 3.0).abs()) + sign * ((x - roll) * k)) * s;
        manip::set_axis(w, id, id, ofs, a, 1);
    }
}

/// `0x2cbda8` (module docs).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x80 { w.mm(id).pvars.resize(0x80, 0); }
    frame_flags(w);
    w.svc.visibomb.missile_view_at = Some(w.counter);
    let timer = p::i32(&w.m(id).pvars, pv::TIMER) - 1;
    p::set_i32(&mut w.mm(id).pvars, pv::TIMER, timer);
    let (state, group) = (w.hero.state, w.hero.group);
    let held_state = matches!(state, 0x16 | 0x3d | 0x72 | 0x65);
    let bad_group = matches!(group, 2 | 3 | 4 | 5 | 7 | 0xa | 0x10) || 0xd <= group;
    if timer <= -w.ticks(100) || held_state || bad_group || w.hero.grounded_ticks == 0 {
        end_flight(w, id);
        return;
    }
    if timer < 0 {
        track(w, id);
        view_off(w, false, false);
        glow_frame(w, id);
        return;
    }
    // The speed.
    let boost = w.hero.loop_in.pad.held & crate::pad::button::CROSS != 0;
    let want = if boost { BOOST * DT } else { CRUISE * DT };
    {
        let pvs = &mut w.mm(id).pvars;
        let ts = p::ff(pvs, pv::TARGET_SPEED);
        let ts = ts + (want - ts) * f32::from_bits(0x3dcc_cccd);
        p::set_ff(pvs, pv::TARGET_SPEED, ts);
        let mut s = p::ff(pvs, pv::SPEED);
        s += (ts - s) * f32::from_bits(0x3c23_d70a);
        if BOOST * DT < s { s = BOOST * DT; }
        p::set_ff(pvs, pv::SPEED, s);
    }
    // SoundSetPitchBend(voice, trunc((target − 24dt / (34dt − 24dt))·100)).
    let ts = p::ff(&w.m(id).pvars, pv::TARGET_SPEED);
    let bend = ((ts - (CRUISE * DT) / (BOOST * DT - CRUISE * DT)) * 100.0) as i32;
    let voice = p::i32(&w.m(id).pvars, pv::VOICE);
    w.set_pitch_bend(voice, bend);
    let t3000 = w.ticks(FLIGHT);
    // Steering.
    if timer > 0 && timer < t3000 - w.ticks(5) {
        let st = [w.hero.stick[0].to_f32(), w.hero.stick[1].to_f32()];
        let m = w.mm(id);
        let (mut sx, mut sy) = (p::ff(&m.pvars, pv::STICK_X), p::ff(&m.pvars, pv::STICK_Y));
        sy += (st[1] - sy) * (SPEED_K * 0.1);
        sx += (st[0] - sx) * (SPEED_K * 0.1);
        p::set_ff(&mut m.pvars, pv::STICK_Y, sy);
        p::set_ff(&mut m.pvars, pv::STICK_X, sx);
        m.rotation[0] += (sx - m.rotation[0]) * (SPEED_K * f32::from_bits(0x3d75_c28f));
        let mut pitch = wrap(m.rotation[1] - sy * (SPEED_K * TURN));
        if FRAC_PI_2 <= pitch {
            pitch = PITCH_LIMIT;
        } else if pitch <= -FRAC_PI_2 {
            pitch = -PITCH_LIMIT;
        }
        m.rotation[1] = pitch;
        m.rotation[2] = wrap(m.rotation[2] - sx * (SPEED_K * TURN));
        // 0x26eec8(0.2, m): the blob shadow.
        crate::shadows::blob(w, 0.2, id);
    }
    // The step.
    let (speed, pitch, yaw) = { let m = w.m(id); (p::ff(&m.pvars, pv::SPEED), m.rotation[1], m.rotation[2]) };
    let vel = polar(speed, yaw, -pitch);
    let prev = v3(w.m(id).position);
    let mut pos = [prev[0] + vel[0], prev[1] + vel[1], prev[2] + vel[2]];
    { let m = w.mm(id); m.position = [pos[0], pos[1], pos[2], m.position[3]]; }
    // Over the ground for the first 60 ticks.
    if t3000 - w.ticks(60) < timer {
        let a = [Pf::f(pos[0]), Pf::f(pos[1]), Pf::f(pos[2] + 0.5), Pf::ZERO];
        let b = [Pf::f(pos[0]), Pf::f(pos[1]), Pf::b(0x3c23_d70a), Pf::ZERO];
        let hit = w.coll_line(a, b, 2, None);
        let gz = hit.as_ref().map_or(0.0, |h| h.point[2]);
        let floor = gz + p::ff(&w.m(id).pvars, pv::HEIGHT);
        let over = match hit.and_then(|h| h.moby) {
            None => true,
            Some(mb) => w.classes.info(w.m(mb).o_class).is_some_and(|i| i.ty == 5),
        };
        if pos[2] < floor && over {
            let mut z = pos[2];
            let gap = approach(floor, DT * 6.0, &mut z);
            pos[2] = z;
            let mut a = atan(speed, gap);
            if DT * FRAC_PI_2 < a { a = DT * FRAC_PI_2; }
            let m = w.mm(id);
            m.position[2] = z;
            m.rotation[1] -= a;
        }
    }
    if pos.iter().any(|&c| c < BOX.0 || BOX.1 < c) {
        end_flight(w, id);
        return;
    }
    glow_frame(w, id);
    fins(w, id);
    // The path.
    let n = crate::hero::guns::with_len(vel, 1.0);
    let tmpl = HitTemplate { dir: [Pf::f(n[0]), Pf::f(n[1]), Pf::ONE, Pf::b(0x45af_df66)], attacker: Some(id), flags: 0x83_0000, b18: 3, b19: 2, h1a: CLASS as u16, damage: Pf::f(6.0), w20: 1 };
    let ignore = if t3000 - 40 < timer { w.hero_moby } else { Some(id) };
    let mut on_hero = false;
    match sv::line_hit_in(w.table, w.svc, w.classes, w.coll, f4(prev), f4(pos), 0, ignore, &tmpl) {
        Some(h) if h.moby.is_some() || 0 < h.kind => {
            let hero_moby = w.hero_moby;
            let m = w.mm(id);
            if h.moby.is_some() {
                m.cmd = 2;
                on_hero = h.moby == hero_moby;
            } else {
                m.cmd = 1;
            }
            m.position = [h.point[0], h.point[1], h.point[2], m.position[3]];
            // The hit face's edge, the step's reflection (2·dt) and the normal: SpawnBeamExplosion's param_9 and a
            // stack vector nothing reads.
            release_voice(w, id);
        }
        Some(_) => {}
        None => {
            if timer == 0 {
                end_flight(w, id);
                return;
            }
        }
    }
    // ○ after the first 60 ticks.
    if timer < t3000 - w.ticks(60) && w.hero.loop_in.pad.pressed & crate::pad::button::CIRCLE != 0 {
        w.mm(id).cmd = 1;
        release_voice(w, id);
    }
    if w.m(id).cmd != 0 { explode(w, id, on_hero, timer); }
    track(w, id);
}

/// The explosion branch of `0x2cbda8` (module docs).
fn explode(w: &mut World, id: MobyId, on_hero: bool, timer: i32) {
    let pos = w.m(id).position;
    if !on_hero {
        let b = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 4.0, flash2: 2.0, flash_dist: 0.0, scale: 1.0, light: 20.0, streaks: 10, sparks: 3, puffs: 16, debris: 1, sound: 1, shake: true };
        fx::beam_explosion(w, &b, Some(id), pos);
        crate::moby_update::creature::attack::area_hit(w, 4.0, pos, id, 6.0, 1.0, 1.0, None, 0x83_0000, 3, 2);
    }
    let t3000 = w.ticks(FLIGHT);
    let short = t3000 - w.ticks(30) < timer;
    let t100 = w.ticks(100);
    {
        let m = w.mm(id);
        m.mode |= 1;
        p::set_ff(&mut m.pvars, pv::SPEED, SPEED_K * 0.02);
        let sx = p::ff(&m.pvars, pv::STICK_X);
        p::set_ff(&mut m.pvars, pv::STICK_X, sx * 0.01);
        p::set_i32(&mut m.pvars, pv::TIMER, if short { -t100 } else { -1 });
    }
    for _ in 0..5 {
        let mut q = pos;
        fx::jitter(w, 1.5, &mut q);
        w.mm(id).position = q;
        explosion(w, id);
        w.mm(id).position = pos;
    }
    w.mm(id).cmd = 0;
}

/// `0x2cb968(m, 1)`: six smoke puffs (type 13) 0.3 above it, then by the camera's distance d up to ten rings (type 11:
/// `trunc(d) + 2` within 8, slower by `7 − d` within 7) and the flashes (the white and orange ones only under light
/// load).
pub fn explosion(w: &mut World, id: MobyId) {
    let pos = w.m(id).position;
    let p13 = [Pf::f(pos[0]), Pf::f(pos[1]), Pf::f(pos[2] + 0.3), Pf::ZERO];
    let (j, lo, g, size) = (Pf::b(0x3e99_999a), Pf::b(0x3f81_47ae), Pf::b(0x3d4c_cccd), Pf::f(50000.0));
    w.part13(j, lo, Pf::b(0x3f89_999a), g, size, p13, 1, 0x4080_8080);
    for _ in 0..5 { w.part13(j, lo, Pf::b(0x3f88_f5c3), g, size, p13, 1, 0x4080_8080); }
    let cam = v3(w.camera.map(|x| x.to_f32()));
    let d = ((cam[0] - pos[0]).powi(2) + (cam[1] - pos[1]).powi(2) + (cam[2] - pos[2]).powi(2)).sqrt();
    let n = if d < 8.0 { d as i32 + 2 } else { 10 };
    let slow = if d < 7.0 { 7.0 - d } else { 0.0 };
    let pp = pos.map(Pf::f);
    let zero = [Pf::ZERO; 4];
    for _ in 0..n.max(0) {
        let s = w.rng.randf(8.0, 10.0) * DT;
        let speed = s - slow * DT;
        let c1 = COL_A[w.rng.randi(6) as usize];
        let c2 = COL_B[w.rng.randi(6) as usize];
        let (t15, t20) = (w.ticks(15), w.ticks(20));
        let life = w.rng.rand_range(t15, t20);
        let (t25, t30) = (w.ticks(25), w.ticks(30));
        let t1 = w.rng.rand_range(t25, t30);
        w.part11(Pf::f(400_000.0), Pf::f(speed), pp, zero, c1, c2, life, t1, 0, 0);
    }
    let dv = [0.0; 4];
    if fx::frame_load(w).0 < 0.95 {
        let t = w.ticks(15);
        super::debris::flash_spawn(w, 3.0, id, pos, dv, t, 0x7f, 0x7f, 0x7f, 0x20);
        let t = w.ticks(24);
        super::debris::flash_spawn(w, 2.0, id, pos, dv, t, 0x7f, 0x20, 0, 0x20);
    }
    let t = w.ticks(20);
    super::debris::flash_spawn(w, 3.0, id, pos, dv, t, 0x7f, 0x40, 0, 0x30);
    let t = w.ticks(27);
    super::debris::flash_spawn(w, 2.0, id, pos, dv, t, 0x60, 0x10, 0, 0x40);
    let t = w.ticks(29);
    super::debris::flash_spawn(w, 1.0, id, pos, dv, t, 0x20, 0, 0, 0x20);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pitch_bend_is_the_games_expression() {
        // (24dt − 2.4)·100 at cruise (−199.99…, truncated), (34dt − 2.4)·100 flat out.
        let f = |ts: f32| ((ts - (CRUISE * DT) / (BOOST * DT - CRUISE * DT)) * 100.0) as i32;
        assert_eq!((f(CRUISE * DT), f(BOOST * DT)), (-199, -183));
    }

    /// The fins: attached on lists 0 / 1 (list 1's node first), turned about y by −0.7·(0.5·y + |x/3| ± 1.1·(x − roll)).
    #[test]
    fn fins_tilt_against_the_steering() {
        use crate::moby_runtime::{Moby, MobyTable};
        let mut m = Moby { o_class: CLASS, pvars: vec![0; pv::SIZE], rotation: [0.1, 0.0, 0.0, 0.0], ..Moby::default() };
        p::set_ff(&mut m.pvars, pv::STICK_X, 0.6);
        p::set_ff(&mut m.pvars, pv::STICK_Y, -0.4);
        let mut t = MobyTable::new(vec![m], 4);
        let hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = crate::moby_update::Services::new();
        svc.joint_targets.insert(CLASS, vec![2, 3]);
        let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 0);
        for (list, ofs) in pv::FINS.into_iter().enumerate() { manip::attach(&mut w, 0, list as u8, 0, ofs); }
        fins(&mut w, 0);
        let (k, s) = (f32::from_bits(0x3f8c_cccd), f32::from_bits(0x3f33_3333));
        let a0 = -((-0.4 * 0.5 + (0.6f32 / 3.0).abs()) + (0.6 - 0.1) * k) * s;
        let a1 = -((-0.4 * 0.5 + (0.6f32 / 3.0).abs()) - (0.6 - 0.1) * k) * s;
        let mods = &w.m(0).joint_mods;
        assert_eq!(mods.iter().map(|m| m.joint).collect::<Vec<_>>(), vec![3, 2]);
        assert_eq!((mods[1].quat, mods[0].quat), (crate::hero::idle::axis_quat(a0, 1), crate::hero::idle::axis_quat(a1, 1)));
    }

    #[test]
    fn view_writes_count() {
        let mut g = Globals::default();
        g.fog(FogWrite::Swap);
        g.fog(FogWrite::Restore { full: false, underwater_off: false });
        assert_eq!((g.view.fog_seq, g.view.fog), (2, FogWrite::Restore { full: false, underwater_off: false }));
        assert_eq!(g.fog_since(1).map(|x| x.1).collect::<Vec<_>>(), [FogWrite::Restore { full: false, underwater_off: false }]);
        for _ in 0..40 { g.fog(FogWrite::Swap); }
        assert_eq!(g.fog_log.len(), FOG_LOG);
    }
}
