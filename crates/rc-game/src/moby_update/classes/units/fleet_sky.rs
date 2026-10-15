//! **The Fleet's space backdrop, class 1772** (level17 `0x2f3848`, draw `0x2f32f0`; census U617). It owns the level's
//! death height (0x15f638): 150 while Ratchet stands in its cuboid (+0x4c) on the fleet, 10 elsewhere and while he
//! flies the ship 1379 (hero state 0x32); and it draws the sky: two long glowing lanes from its place out into space,
//! and five huge slowly turning, slowly breathing nebula squares 1000 units past the lanes' far end. The animation tables are the level's
//! own statics (shared by every 1772; the overlay's data, fresh on each load). Read from the level17 decomp; native `f32`.
//!
//! **Pvar block**: +0x00 the lanes' near end (the placed point), +0x10 their far end (its y forced to −8000 by each
//! draw), +0x28 the far end's half width, +0x44 the second leg's heading, +0x4c the cuboid.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2f3848` | +0x4c ≠ −1: death height 10.0 when hero state 0x32 with vehicle (0x140940) class 0x563 or Ratchet not in cuboid +0x4c, else 150.0 (gp−0x75c8 = 0x15f638) | [`update`] |
//! | state 0 | the turn angles 0x1de0d0[i] = i·25; near = position; far = near + `0x25ce50`((25000 + 11500)/50, yaw +0x48, 0) + `0x25ce50`(11500/50, +0x44, 0) (gp−0x482c..−0x4828); far.x −= 300; update distance 0xff; → 1 | [`update`] |
//! | else | the five breaths 0x1de040 += speed (0x1de070)·dir (0x1de058)·dt, bouncing off 0 / 1; the turns 0x1de0d0 += 0x1ddfe0·0.1·dt (`fast_add_rotations`); the lanes' scrolls 0x1de0a0 += (−0.5, −1) (gp−0x4840)·dt, wrapped into [0, 1); `RegisterDrawCallback(0x2f32f0)` | [`update`] |
//! | `0x2f32f0` | TEST 0x513f1 (no z write) round the whole draw; the view's far plane 0x16d3e4 = 8 192 000 (`UpdateViewContext` projects the next frames with it) | n/a [L]: the effect quads write no depth; [`frame`] (`Services::view_far`) |
//! | | far.y = −8000 (+0x14 = 0xc5fa0000); two lanes k: e = near − far; a = unit((near − cam) × e)·(20, 10)[k] (0x1de0b8), b = unit((far − cam) × e)·+0x28; quad near ∓ a, far ∓ b; ST (−8, 0) (−8, 1) (8, 0) (8, 1) (0x1e6fa0) with s + scroll[k]; RGBA 0x807e4931 (gp−0x4848); FX 0xe (0x1de088); ALPHA 0x48; then scroll[k] += speed·dt (unwrapped) | [`frame`], [`fx_quad_groups`] |
//! | | five squares i (FX 0x2f): side (3500, 4000, 2000, 3000, 3300) (0x1ddff8) centred on (far.x, far.y + 1000, far.z) in the x–z plane; ST ½ + ½·(sin, cos) of turn + (3π/2, π, 0, π/2); RGBA `FastTweenColor`(breath, 0x1de010, 0x1de028) | [`fx_quad_groups`] |

use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::{self as c, fx, DT};
use crate::moby_update::services::{self as sv, Services, World};
use crate::moby_update::story;

use super::{FxQuad, FxQuads};

pub const REFERENCE_LEVEL: u32 = 17;
pub const UPDATE_FN: u32 = 0x2f_3848;
pub const DRAW_FN: u32 = 0x2f_32f0;
pub const CLASSES: [i16; 1] = [1772];

mod o {
    pub const NEAR: usize = 0x00;
    pub const FAR: usize = 0x10;
    pub const FAR_WIDTH: usize = 0x28;
    pub const HEADING: usize = 0x44;
    pub const CUBOID: usize = 0x4c;
    pub const SIZE: usize = 0x50;
}

const SHIP_STATE: i32 = 0x32;
const SHIP_CLASS: i16 = 0x563;
/// Level17 0x1ddfe0: the squares' turn rates; 0x1ddff8: their sides; 0x1de010 / 0x1de028: their colours.
const TURN: [f32; 5] = [0.5, -0.24, 0.3, 1.0, -0.6];
const SIDE: [f32; 5] = [3500.0, 4000.0, 2000.0, 3000.0, 3300.0];
const RGBA_A: [u32; 5] = [0x8025_2525, 0x8020_2020, 0x8025_2525, 0x8010_1010, 0x8015_1515];
const RGBA_B: [u32; 5] = [0x8010_1030, 0x8030_1010, 0x8010_1015, 0x8015_1010, 0x8015_1015];
/// 0x1de070: the breaths' speeds.
const BREATH_SPEED: [f32; 5] = [0.1, 0.15, 0.26, 0.17, 0.05];
/// The lanes: FX (0x1de088), near half width (0x1de0b8), scroll speed (gp−0x4840), colour (gp−0x4848).
const LANE_FX: usize = 0xe;
const LANE_WIDTH: [f32; 2] = [20.0, 10.0];
const SCROLL_SPEED: [f32; 2] = [-0.5, -1.0];
const LANE_RGBA: u32 = 0x807e_4931;
const LANE_ST: [[f32; 2]; 4] = [[-8.0, 0.0], [-8.0, 1.0], [8.0, 0.0], [8.0, 1.0]];
const SQUARE_FX: usize = 0x2f;

/// The level's statics 0x1de040.. (`Globals::fleet_sky`) and what the draw saw.
#[derive(Clone, Debug)]
pub struct Sky {
    breath: [f32; 5],
    dir: [f32; 5],
    turn: [f32; 5],
    scroll: [f32; 2],
    /// The scrolls the last draw used, and its camera (0x1676c0).
    drawn: [f32; 2],
    cam: [f32; 3],
}

impl Default for Sky {
    fn default() -> Self {
        Sky { breath: [0.0, 1.0, 0.33, 0.55, 0.22], dir: [1.0; 5], turn: [0.0; 5], scroll: [0.0, 0.1], drawn: [0.0, 0.1], cam: [0.0; 3] }
    }
}

/// Level17 `0x2f3848` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, o::SIZE);
    let cub = c::pi32(w, id, o::CUBOID);
    if cub != -1 {
        let flying = w.hero.state == SHIP_STATE && w.svc.vehicle.moby.filter(|&v| v < w.table.mobys.len()).is_some_and(|v| w.m(v).o_class == SHIP_CLASS);
        let z = if flying || !story::hero_in(w, cub) { 10.0 } else { 150.0 };
        w.hero_fields_mut().death_z = Some(z);
    }
    if w.m(id).state == 0 {
        w.mm(id).state = 1;
        let sky = &mut w.svc.units.fleet_sky;
        for (i, t) in sky.turn.iter_mut().enumerate() { *t = i as f32 * 25.0; }
        let near = c::pos(w, id);
        c::set_pv4(w, id, o::NEAR, near);
        let a = fx::polar((25_000.0 + 11_500.0) / 50.0, w.m(id).rotation[2], 0.0);
        let b = fx::polar(11_500.0 / 50.0, c::pf(w, id, o::HEADING), 0.0);
        let mut far = c::add(near, c::add(a, b));
        far[3] = near[3];
        w.mm(id).update_dist = 0xff;
        far[0] -= 300.0;
        c::set_pv4(w, id, o::FAR, far);
        return;
    }
    let sky = &mut w.svc.units.fleet_sky;
    for ((b, d), v) in sky.breath.iter_mut().zip(sky.dir.iter_mut()).zip(BREATH_SPEED) {
        *b += v * *d * DT;
        if *b > 1.0 {
            *b = 1.0;
            *d = -1.0;
        }
        if *b < 0.0 {
            *b = 0.0;
            *d = 1.0;
        }
    }
    for (t, r) in sky.turn.iter_mut().zip(TURN) { *t = c::add_rot(*t, r * 0.1 * DT); }
    for (s, v) in sky.scroll.iter_mut().zip(SCROLL_SPEED) {
        *s += v * DT;
        if *s >= 1.0 { *s -= 1.0; }
        if *s < 0.0 { *s += 1.0; }
    }
    if let Some(row) = super::row(REFERENCE_LEVEL, DRAW_FN) {
        w.svc.draw_callbacks.register(Callback::UnitFrame(row), id);
        w.svc.draw_callbacks.register(Callback::UnitQuads(row), id);
    }
}

/// The state part of the draw `0x2f32f0`: the view's far plane, far.y = −8000, the camera, the scrolls used and then
/// advanced.
pub fn frame(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < o::SIZE { return; }
    w.svc.view_far = 8_192_000.0;
    c::set_pf(w, id, o::FAR + 4, -8000.0);
    let cam = crate::hero::physics::to_f32x3(w.camera);
    let sky = &mut w.svc.units.fleet_sky;
    sky.cam = cam;
    sky.drawn = sky.scroll;
    for (s, v) in sky.scroll.iter_mut().zip(SCROLL_SPEED) { *s += v * DT; }
}

/// `FastVecCross(out, x, y)` = y × x.
fn cross_yx(x: [f32; 3], y: [f32; 3]) -> [f32; 3] { [y[1] * x[2] - y[2] * x[1], y[2] * x[0] - y[0] * x[2], y[0] * x[1] - y[1] * x[0]] }

fn set_len(v: [f32; 3], l: f32) -> [f32; 3] {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if n == 0.0 { [0.0; 3] } else { v.map(|q| q * l / n) }
}

/// Level17 `0x2f32f0` for `id` (module doc): the two lanes, then the five squares.
pub fn fx_quad_groups(table: &MobyTable, svc: &Services, id: MobyId) -> Vec<FxQuads> {
    let Some(m) = table.mobys.get(id) else { return Vec::new() };
    if m.pvars.len() < o::SIZE { return Vec::new(); }
    let sky = &svc.units.fleet_sky;
    let near = sv::pvar::v4f(&m.pvars, o::NEAR);
    let far = sv::pvar::v4f(&m.pvars, o::FAR);
    let w1 = sv::pvar::ff(&m.pvars, o::FAR_WIDTH);
    let cam = sky.cam;
    let d3 = |a: [f32; 4], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let along = [near[0] - far[0], near[1] - far[1], near[2] - far[2]];
    let lanes = (0..2).map(|k| {
        let a = set_len(cross_yx(d3(near, cam), along), LANE_WIDTH[k]);
        let b = set_len(cross_yx(d3(far, cam), along), w1);
        let corners = [
            [near[0] - a[0], near[1] - a[1], near[2] - a[2]],
            [near[0] + a[0], near[1] + a[1], near[2] + a[2]],
            [far[0] - b[0], far[1] - b[1], far[2] - b[2]],
            [far[0] + b[0], far[1] + b[1], far[2] + b[2]],
        ];
        let st = LANE_ST.map(|[s, t]| [s + sky.drawn[k], t]);
        FxQuad { corners, st, rgba: [LANE_RGBA; 4] }
    }).collect();
    let squares = (0..5).map(|i| {
        let (s, h) = (SIDE[i], SIDE[i] * 0.5);
        let (x0, y, z0) = (far[0] - h, far[1] + 1000.0, far[2] + h);
        let corners = [[x0, y, z0], [x0, y, z0 - s], [x0 + s, y, z0], [x0 + s, y, z0 - s]];
        let a = sky.turn[i];
        let st = [3.0 * std::f32::consts::FRAC_PI_2, std::f32::consts::PI, 0.0, std::f32::consts::FRAC_PI_2].map(|d| {
            let r = c::add_rot(a, d);
            [r.sin() * 0.5 + 0.5, r.cos() * 0.5 + 0.5]
        });
        let rgba = crate::particles::tween_color(sky.breath[i].to_bits(), RGBA_A[i], RGBA_B[i]);
        FxQuad { corners, st, rgba: [rgba; 4] }
    }).collect();
    vec![
        FxQuads { fx: LANE_FX, additive: true, subtract: false, quads: lanes },
        FxQuads { fx: SQUARE_FX, additive: true, subtract: false, quads: squares },
    ]
}
