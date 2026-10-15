//! **Oltanis's lightning, class 1224** (level14 `0x3015d0`; census U512, 24 placed). A cuboid (+0x00) that throws
//! a lightning bolt along its local y axis, end to end, with four branches, over and over; Ratchet within 10 of it
//! (xy) is hit where the bolt's line runs. Read from the level14 decomp; native `f32`.
//!
//! * **Every tick** (from state 1 on, only when the sphere at it with the cuboid's y half-length is in view within
//!   52): 0 → a bolt, `ticks(+0x0c)`, 2, both ends open; 2 → the timer out: 3, `ticks(+0x04)`, a bolt; 3 → while the
//!   timer runs a new bolt whenever the last has faded; out: `ticks(+0x04)`, or with +0x06 set `ticks(+0x06)` and back
//!   to 2. Then Ratchet within 10 → the hit (`0x302538`); a bolt alive → it fades (`0x301de8`) and is drawn.
//! * **A bolt** (`0x3017f8`): its life `rand_range(ticks(10), ticks(40))`; 32 points from +len to −len along y (len
//!   = the cuboid's y half-length), points 3..28 jittered by `rand_vec(0, 0.3)` with drifts `rand_vec(0, dt)`, then
//!   both averaged with their neighbours (the ends kept); four branches from points 4 + `randf(6k, 6k + 6)`, each
//!   three steps of one `rand_vec(0.3, 0.3)` and `rand_vec(0.1, 0.1)` (drifts `rand_vec(0, dt)`). With the camera 4
//!   or more from the cuboid: sparks (type 2: size 2, colours 0x10806080 / 0x10806060, phases (i/2, 15, `ticks(20 +
//!   randi(10))`), def 53 or 24 by `randi(2)`) at points 0, 8, 16, 24 and the first two branches' tips (def 53; a `randi(2)` drawn and dropped).
//! * **The fade** (`0x301de8`): life − 1; the first 23 points and the branch steps move by their drifts, the branches
//!   follow their roots; the colour fades 0x80403040 → 0x00401040 down to life 15, then → 0x00400000.
//! * **The hit** (`0x302538`, with a beam width +0x1c): each end open along ±row 1 from the centre unless a line
//!   test meets something (then 0.98 of the way to it, closed for good); four lines end to end, offset by ±width
//!   along rows 0 and 2, hit (flags 1) for 1 (0x10001, type 3 / 1) pushed along its facing toward Ratchet.
//! * **The draw** (`0x301fa8`, FX 0xe additive, the colour): in the cuboid's frame (its Euler rows, its centre),
//!   the bolt as 31 quads, each 0.1 either side of `(p[i] − p[i + 2]) × (p[i + 1] − camera)` (none for the last two;
//!   the first starts pinched), s stepping a quarter every point (`(i mod 4)/4` added to the table 0x1e5fd0), t 0..1;
//!   each live branch (root < 31) as three quads the same way.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x3015d0` | the update | [`update`] |
//! | `0x3017f8` / `0x301de8` / `0x302538` | bolt, fade, hit | [`bolt`], [`fade`], [`hit`] |
//! | `0x301fa8` | the draw | [`frame`], [`fx_quads`] |
//!
//! [L] The level's debug words (gp 0x1620b8.., all 0) leave the paths they gate out. The draw is built from the camera
//! the frame part saw (+0x650).

use super::{FxQuad, FxQuads};
use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::{self as c, fx, DT};
use crate::moby_update::services::{self as sv, euler_rows, pv, pvar as p, HitTemplate, Services, World};
use crate::moby_update::story;
use crate::particles::type02;
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 14;
pub const UPDATE_FN: u32 = 0x30_15d0;
pub const DRAW_FN: u32 = 0x30_1fa8;
pub const CLASSES: [i16; 1] = [1224];

mod o {
    pub const CUBOID: usize = 0x00;
    pub const REPEAT: usize = 0x04;
    pub const ALT: usize = 0x06;
    pub const OPEN_A: usize = 0x08;
    pub const OPEN_B: usize = 0x0a;
    pub const FIRST: usize = 0x0c;
    pub const TIMER: usize = 0x10;
    pub const LIFE: usize = 0x12;
    pub const COLOUR: usize = 0x14;
    pub const ROOTS: usize = 0x18;
    pub const WIDTH: usize = 0x1c;
    pub const PTS: usize = 0x20;
    pub const DRIFT: usize = 0x220;
    pub const BR: usize = 0x420;
    pub const BR_DRIFT: usize = 0x520;
    pub const LIFE0: usize = 0x620;
    pub const LEN: usize = 0x624;
    pub const END_A: usize = 0x630;
    pub const END_B: usize = 0x640;
    pub const CAM: usize = 0x650;
    pub const SIZE: usize = 0x660;
}
/// Level14 gp words (0x1620bc..0x162120).
const LIFE_LO: i32 = 10;
const LIFE_HI: i32 = 40;
const FADE_MID: i32 = 15;
const SPREAD: f32 = 0.3;
const COL_A: u32 = 0x0040_1040;
const COL_B: u32 = 0x8040_3040;
const COL_C: u32 = 0x0040_0000;

fn cuboid(w: &World, id: MobyId) -> Option<rc_formats::volumes::Shape> {
    w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, c::pi32(w, id, o::CUBOID)).copied()
}
fn v3(v: [f32; 4]) -> c::V { [v[0], v[1], v[2], 0.0] }
fn rand_vec(w: &mut World, lo: f32, hi: f32) -> c::V { let v = w.rng.rand_vec(lo, hi); [v[0], v[1], v[2], 0.0] }
/// The branch point (k, j) offset.
fn br(k: usize, j: usize) -> usize { 0x10 * k + 0x40 * j }
/// The cuboid's frame: its Euler rows and centre.
fn frame_of(s: &rc_formats::volumes::Shape) -> [[f32; 3]; 4] {
    let r = euler_rows(pv([s.euler[0], s.euler[1], s.euler[2], 0.0])).map(|row| row.map(|x| f32::from_bits(x.0)));
    [[r[0][0], r[0][1], r[0][2]], [r[1][0], r[1][1], r[1][2]], [r[2][0], r[2][1], r[2][2]], s.centre()]
}
fn through(f: &[[f32; 3]; 4], q: [f32; 4]) -> [f32; 3] { std::array::from_fn(|k| f[0][k] * q[0] + f[1][k] * q[1] + f[2][k] * q[2] + f[3][k]) }

/// `0x3017f8` (module doc).
pub fn bolt(w: &mut World, id: MobyId) {
    let Some(s) = cuboid(w, id) else { return };
    let (lo, hi) = (w.ticks(LIFE_LO), w.ticks(LIFE_HI));
    let life = w.rng.rand_range(lo, hi) as i16;
    c::set_pi32(w, id, o::LIFE0, life as i32);
    c::set_pi16(w, id, o::LIFE, life);
    let len = c::len3(v3(s.matrix[1]));
    let mut a = [[0.0f32; 4]; 32];
    let mut b = [[0.0f32; 4]; 32];
    for i in 0..32 {
        let f = 0.5 - i as f32 / 31.0;
        if (3..29).contains(&i) {
            a[i] = rand_vec(w, 0.0, SPREAD);
            b[i] = rand_vec(w, 0.0, DT);
        }
        a[i][3] = 1.0;
        a[i][1] += f * (len + len);
    }
    for i in 0..32 {
        let (pt, dr) = if i == 0 || i == 31 {
            (a[i], b[i])
        } else {
            let sum = |v: &[[f32; 4]; 32]| -> [f32; 4] { c::scale(c::add(c::add(v[i - 1], v[i]), v[i + 1]), f32::from_bits(0x3eaa_7efa)) };
            (sum(&a), sum(&b))
        };
        c::set_pv4(w, id, o::PTS + 0x10 * i, pt);
        c::set_pv4(w, id, o::DRIFT + 0x10 * i, dr);
    }
    for k in 0..4 {
        let step = rand_vec(w, SPREAD, SPREAD);
        let r = w.rng.randf(k as f32 * 6.0, (k + 1) as f32 * 6.0);
        let idx = r as i32 + 4;
        c::set_pu8(w, id, o::ROOTS + k, idx as u8);
        let (root, rd) = (c::pv4(w, id, o::PTS + 0x10 * idx as usize), c::pv4(w, id, o::DRIFT + 0x10 * idx as usize));
        c::set_pv4(w, id, o::BR + br(k, 0), root);
        c::set_pv4(w, id, o::BR_DRIFT + br(k, 0), rd);
        for j in 0..3 {
            let jit = rand_vec(w, 0.1, 0.1);
            let d = rand_vec(w, 0.0, DT);
            c::set_pv4(w, id, o::BR_DRIFT + br(k, j + 1), d);
            let prev = c::pv4(w, id, o::BR + br(k, j));
            c::set_pv4(w, id, o::BR + br(k, j + 1), c::add(c::add(prev, step), jit));
        }
    }
    let cam = w.camera.map(|x| x.to_f32());
    let ctr = s.centre();
    if c::dist2([ctr[0], ctr[1], ctr[2], 0.0], cam) < 4.0 { return; }
    let f = frame_of(&s);
    let spark = |w: &mut World, at: [f32; 4], vel: [f32; 4], t0: i32, def: i32| {
        let p = through(&f, at);
        let v = [vel[0], vel[1], vel[2], 2.0];
        let n = w.rng.randi(10);
        let life = w.ticks(n + 0x14);
        fx::part02(w, &type02::Spawn { pos: [p[0], p[1], p[2], 1.0], v1: v, v2: v, c1: 0x1080_6080, c2: 0x1080_6060, t: [t0, 0xf, life], def });
    };
    for i in [0usize, 8, 16, 24] {
        let (at, vel) = (c::pv4(w, id, o::PTS + 0x10 * i), c::pv4(w, id, o::DRIFT + 0x10 * i));
        let def = if w.rng.randi(2) == 0 { 0x35 } else { 0x18 };
        spark(w, at, vel, (i >> 1) as i32, def);
    }
    for k in 0..2 {
        let (at, vel) = (c::pv4(w, id, o::BR + br(k, 3)), c::pv4(w, id, o::BR_DRIFT + br(k, 3)));
        w.rng.randi(2);
        let root = c::pu8(w, id, o::ROOTS + k) as i32;
        spark(w, at, vel, root >> 1, 0x35);
    }
}

/// `0x301de8` (module doc).
pub fn fade(w: &mut World, id: MobyId) {
    let life = c::pi16(w, id, o::LIFE);
    if life != 0 {
        c::set_pi16(w, id, o::LIFE, life - 1);
        for i in 0..23 {
            let q = c::add(c::pv4(w, id, o::PTS + 0x10 * i), c::pv4(w, id, o::DRIFT + 0x10 * i));
            c::set_pv4(w, id, o::PTS + 0x10 * i, q);
        }
        for k in 0..4 {
            for j in 1..4 {
                let q = c::add(c::pv4(w, id, o::BR + br(k, j)), c::pv4(w, id, o::BR_DRIFT + br(k, j)));
                c::set_pv4(w, id, o::BR + br(k, j), q);
            }
        }
        for k in 0..4 {
            let idx = c::pu8(w, id, o::ROOTS + k) as usize;
            let q = c::pv4(w, id, (o::PTS + 0x10 * idx).min(o::DRIFT - 0x10));
            c::set_pv4(w, id, o::BR + br(k, 0), q);
        }
    }
    let life = c::pi16(w, id, o::LIFE) as i32;
    let col = if FADE_MID < life {
        let f = (life - FADE_MID) as f32 / (c::pi32(w, id, o::LIFE0) - FADE_MID) as f32;
        crate::hud::tween_color(f, COL_B, COL_A)
    } else if 0 < life {
        crate::hud::tween_color(life as f32 / FADE_MID as f32, COL_C, COL_B)
    } else {
        0
    };
    c::set_pi32(w, id, o::COLOUR, col as i32);
}

/// `0x302538` (module doc).
pub fn hit(w: &mut World, id: MobyId) {
    let width = c::pf(w, id, o::WIDTH);
    if width == 0.0 { return; }
    let Some(s) = cuboid(w, id) else { return };
    let y = c::yaw(w, id);
    let mut u = [y.cos(), y.sin(), 0.0, 0.0];
    let h = super::hero_pos(w);
    let pos = c::pos(w, id);
    let d = [h[0] - pos[0], h[1] - pos[1], 0.0, 0.0];
    let mut dir = [0.0, 0.0, 1.0, f32::from_bits(0x45af_df66)];
    if c::len3(d) != 0.0 {
        if c::dot3(u, d) < 0.0 { u = c::scale(u, -1.0); }
        dir[0] = u[0];
        dir[1] = u[1];
    }
    let tmpl = HitTemplate { dir: pv(dir), attacker: Some(id), flags: 0x1_0001, b18: 3, b19: 1, h1a: w.m(id).o_class as u16, damage: Pf::ONE, w20: 1 };
    let ctr = s.centre();
    let ctr = [ctr[0], ctr[1], ctr[2], 1.0];
    for (flag, end, sign) in [(o::OPEN_A, o::END_A, 1.0f32), (o::OPEN_B, o::END_B, -1.0)] {
        if c::pi16(w, id, flag) == 0 { continue; }
        let tip = c::add(ctr, c::scale(v3(s.matrix[1]), sign));
        match w.coll_line(pv(ctr), pv(tip), 0, Some(id)) {
            None => c::set_pv4(w, id, end, tip),
            Some(hit) => {
                let v = c::sub([hit.point[0], hit.point[1], hit.point[2], ctr[3]], ctr);
                let l = c::len3(v);
                let v = c::scale(v, (l * 0.98) / l);
                c::set_pv4(w, id, end, c::add(ctr, v));
                c::set_pi16(w, id, flag, 0);
            }
        }
    }
    let (a, b) = (c::pv4(w, id, o::END_A), c::pv4(w, id, o::END_B));
    for row in [0usize, 2] {
        let off = c::set_len3(v3(s.matrix[row]), width);
        sv::line_hit_in(w.table, w.svc, w.classes, w.coll, pv(c::add(a, off)), pv(c::add(b, off)), 1, Some(id), &tmpl);
        sv::line_hit_in(w.table, w.svc, w.classes, w.coll, pv(c::sub(a, off)), pv(c::sub(b, off)), 1, Some(id), &tmpl);
    }
}

/// Level14 `0x3015d0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, o::SIZE);
    if w.m(id).state != 0 {
        let p = c::pos(w, id);
        let sphere = [p[0], p[1], p[2], c::pf(w, id, o::LEN)];
        if !w.view.is_some_and(|v| !v.culled(52.0, sphere)) { return; }
    }
    match w.m(id).state {
        0 => {
            bolt(w, id);
            let t = w.ticks(c::pi32(w, id, o::FIRST));
            c::set_pi16(w, id, o::TIMER, t as i16);
            w.mm(id).state = 2;
            let len = cuboid(w, id).map_or(0.0, |s| c::len3(v3(s.matrix[1])));
            c::set_pf(w, id, o::LEN, len);
            c::set_pi16(w, id, o::OPEN_B, 1);
            c::set_pi16(w, id, o::OPEN_A, 1);
        }
        2 => {
            if c::dec_timer_pvar_s16(w, id, o::TIMER) != 0 {
                w.mm(id).state = 3;
                let t = w.ticks(c::pi16(w, id, o::REPEAT) as i32);
                c::set_pi16(w, id, o::TIMER, t as i16);
                bolt(w, id);
            }
        }
        3 => {
            if c::dec_timer_pvar_s16(w, id, o::TIMER) == 0 {
                if c::pi16(w, id, o::LIFE) == 0 { bolt(w, id); }
            } else {
                let alt = c::pi16(w, id, o::ALT);
                let n = if alt == 0 {
                    c::pi16(w, id, o::REPEAT)
                } else {
                    w.mm(id).state = 2;
                    alt
                };
                let t = w.ticks(n as i32);
                c::set_pi16(w, id, o::TIMER, t as i16);
            }
        }
        _ => {}
    }
    if c::dist2(c::pos(w, id), super::hero_pos(w)) < 10.0 { hit(w, id); }
    if c::pi16(w, id, o::LIFE) != 0 {
        fade(w, id);
        if 0 < c::pi16(w, id, o::LIFE) {
            if let Some(row) = super::row(REFERENCE_LEVEL, DRAW_FN) {
                w.svc.draw_callbacks.register(Callback::UnitFrame(row), id);
                w.svc.draw_callbacks.register(Callback::UnitQuads(row), id);
            }
        }
    }
}

/// The draw's frame part: the camera kept.
pub fn frame(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < o::SIZE { return; }
    let cam = crate::hero::physics::to_f32x3(w.camera);
    c::set_pv4(w, id, o::CAM, [cam[0], cam[1], cam[2], 1.0]);
}

/// `FastVecCross(out, a, b)` = b × a.
fn fast_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [b[1] * a[2] - b[2] * a[1], b[2] * a[0] - b[0] * a[2], b[0] * a[1] - b[1] * a[0]] }
fn len_to(v: [f32; 3], l: f32) -> [f32; 3] { let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(); if n == 0.0 { [0.0; 3] } else { v.map(|x| x * l / n) } }

/// One strip of quads along `pts` (local), each 0.1 either side (module doc); `pinch`: the first quad starts with no
/// width (the main bolt), else with its own.
fn strip(pts: &[[f32; 3]], cam: [f32; 3], sides: usize, pinch: bool) -> Vec<[([f32; 3], [f32; 2]); 4]> {
    let s_tab = [0.0f32, 0.0, 0.25, 0.25];
    let t_tab = [0.0f32, 1.0, 0.0, 1.0];
    let mut prev = [0.0f32; 3];
    let mut out = Vec::new();
    for i in 0..pts.len() - 1 {
        let side = if i < sides {
            let a = [pts[i][0] - pts[i + 2][0], pts[i][1] - pts[i + 2][1], pts[i][2] - pts[i + 2][2]];
            let b = [pts[i + 1][0] - cam[0], pts[i + 1][1] - cam[1], pts[i + 1][2] - cam[2]];
            len_to(fast_cross(b, a), 0.1)
        } else {
            [0.0; 3]
        };
        if i == 0 && !pinch { prev = side; }
        let ds = (i % 4) as f32 * 0.25;
        let v = |p: [f32; 3], s: [f32; 3], k: f32| -> [f32; 3] { std::array::from_fn(|n| p[n] + s[n] * k) };
        let corners = [v(pts[i], prev, -1.0), v(pts[i], prev, 1.0), v(pts[i + 1], side, -1.0), v(pts[i + 1], side, 1.0)];
        out.push(std::array::from_fn(|k| (corners[k], [s_tab[k] + ds, t_tab[k]])));
        prev = side;
    }
    out
}

/// Level14 `0x301fa8` (module doc).
pub fn fx_quads(table: &MobyTable, svc: &Services, id: MobyId) -> Option<FxQuads> {
    let m = table.mobys.get(id).filter(|m| m.pvars.len() >= o::SIZE)?;
    let s = svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, p::i32(&m.pvars, o::CUBOID))?;
    let f = frame_of(s);
    let pt = |off: usize| -> [f32; 3] { [p::ff(&m.pvars, off), p::ff(&m.pvars, off + 4), p::ff(&m.pvars, off + 8)] };
    let cw = pt(o::CAM);
    let d = [cw[0] - f[3][0], cw[1] - f[3][1], cw[2] - f[3][2]];
    let cam: [f32; 3] = std::array::from_fn(|k| f[k][0] * d[0] + f[k][1] * d[1] + f[k][2] * d[2]);
    let rgba = p::u32(&m.pvars, o::COLOUR);
    let to_world = |q: [f32; 3]| through(&f, [q[0], q[1], q[2], 1.0]);
    let mut quads = Vec::new();
    let main: Vec<[f32; 3]> = (0..32).map(|i| pt(o::PTS + 0x10 * i)).collect();
    let mut strips = strip(&main, cam, 30, true);
    for k in 0..4 {
        if 31 <= m.pvars[o::ROOTS + k] { continue; }
        let b: Vec<[f32; 3]> = (0..4).map(|j| pt(o::BR + br(k, j))).collect();
        strips.extend(strip(&b, cam, 2, false));
    }
    for q in strips {
        quads.push(FxQuad { corners: q.map(|(c, _)| to_world(c)), st: q.map(|(_, st)| st), rgba: [rgba; 4] });
    }
    Some(FxQuads { fx: 0xe, additive: true, subtract: false, quads })
}
