//! The wandering point lights, class 1504 (levels 01: 1, 06: 4 created instances): level01 `WanderingLightUpdate`
//! 0x30b618, the same code on 06 (0x308868; census U88). A light moby that owns one of the eight dynamic point
//! lights (`crate::point_lights`, `WritePointLight_B` / `FreePointLight`) while the camera is within its radius (and
//! in its cuboid, when it has one): the light's colour wanders between the moby's 2–4 colour keys, one random hop at
//! a time over a random number of ticks (eased by a quarter sine), and fades out over the outer quarter of the radius.
//! A linked moby that dies deletes the light. Read from the level01 decomp of 0x30b618 (`FUN_002705b8` = per-lane
//! lerp). Native `f32`.
//!
//! **Pvar block** (words): +0x00..+0x3f up to four colour keys (r, g, b, intensity; ÷256 at the init), +0x40 the
//! current colour, +0x50 / +0x54 s32 the hop duration range (ticks), +0x58 s32 the key count, +0x5c the radius
//! (camera, xy), +0x60 the light's radius, +0x68 s32 the light slot (−1 none), +0x6c s32 the timer, +0x70 s32 the
//! hop's duration, +0x74 / +0x78 s32 the keys hopped from / to, +0x7c s32 the easing half, +0x80 s32 the cuboid (−1
//! none), +0x84 s32 the linked moby (−1 none).
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | count > 1: every key ÷256 (0.00390625), slot −1, update distance +0x30 = trunc(radius) + 4 when below it, → 1; count ≤ 1 → `DeleteMoby` | [`update`] |
//! | state 1 | near = `VecDistance2`(position, camera 0x167240) < radius, and (cuboid −1, or the camera in the cuboid: `PointInCuboid`) | [`update`] |
//! | | link −1 or the link alive (state ≥ 0): not near → the slot freed (`FreePointLight` 0x252850), −1; return | [`update`] (`PointLights::free`) |
//! | | `FastDecTimer(+0x6c)` out → d = `rand_range(+0x50, +0x54)`, from = to, timer = duration = d, k = `rand_range(1, count − 1)`, half = !half, to = (to + k) % count | [`update`] |
//! | | t = (duration − timer)/duration·π/2 (+ π/2 in the second half); s = `fast_sin(t)`; colour = lerp(key A, key B, s) (A = from, B = to; the second half A = to, B = from) | [`update`] |
//! | | xy distance > 0.75·radius → colour = lerp(colour, 0, (d − 0.75r)/(r − 0.75r)) | [`update`] |
//! | | slot ≠ −1: the slot's position = position, radius = +0x60, colour (r, g, b, intensity) = the colour; else slot = `WritePointLight_B(+0x60, intensity, r, g, b, position)` (0x252750: none above frame load 0.8 or with eight taken) | [`update`] (`PointLights::set` / `alloc`) |
//! | | the link dead (state < 0) → the slot freed, `DeleteMoby` | [`update`] |
//! | | no sound, particle, hit, flag | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::gold_bolt::fast_dec_timer;
use crate::moby_update::creature as c;
use crate::moby_update::services::World;
use crate::point_lights::PointLight;

/// The update in the level01 class table.
pub const UPDATE_FN: u32 = 0x30_b618;
pub const REFERENCE_LEVEL: u32 = 1;
pub const CLASSES: [i16; 1] = [1504];

pub mod pv {
    pub const KEYS: usize = 0x00;
    pub const COLOUR: usize = 0x40;
    pub const HOP_LO: usize = 0x50;
    pub const HOP_HI: usize = 0x54;
    pub const COUNT: usize = 0x58;
    pub const RADIUS: usize = 0x5c;
    pub const LIGHT_R: usize = 0x60;
    pub const SLOT: usize = 0x68;
    pub const TIMER: usize = 0x6c;
    pub const DUR: usize = 0x70;
    pub const FROM: usize = 0x74;
    pub const TO: usize = 0x78;
    pub const HALF: usize = 0x7c;
    pub const CUBOID: usize = 0x80;
    pub const LINK: usize = 0x84;
    pub const SIZE: usize = 0x88;
}

fn lerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] { std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t) }

fn free(w: &mut World, id: MobyId) {
    let s = c::pi32(w, id, pv::SLOT);
    if s != -1 {
        if let Ok(i) = usize::try_from(s) { w.svc.point_lights.free(i); }
        c::set_pi32(w, id, pv::SLOT, -1);
    }
}

/// Level01 0x30b618 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    match w.m(id).state {
        0 => {
            let n = c::pi32(w, id, pv::COUNT);
            if n <= 1 {
                w.delete_moby(id);
                return;
            }
            for k in 0..n.clamp(0, 4) as usize {
                let v = c::pv4(w, id, pv::KEYS + 0x10 * k).map(|x| x * 0.003_906_25);
                c::set_pv4(w, id, pv::KEYS + 0x10 * k, v);
            }
            c::set_pi32(w, id, pv::SLOT, -1);
            let r = c::pf(w, id, pv::RADIUS) as i32;
            if (w.m(id).update_dist as i32) < r + 4 { w.mm(id).update_dist = (r + 4) as u8; }
            w.mm(id).state = 1;
        }
        1 => {
            let cam = w.camera_point();
            let pos = w.m(id).position;
            let radius = c::pf(w, id, pv::RADIUS);
            let d = c::dist2(pos, [cam[0], cam[1], cam[2], 0.0]);
            let mut near = d < radius;
            let cub = c::pi32(w, id, pv::CUBOID);
            if near && cub != -1 && !w.in_cuboid(cam, cub) { near = false; }
            let link = c::pi32(w, id, pv::LINK);
            let alive = link == -1 || usize::try_from(link).ok().and_then(|l| w.table.mobys.get(l)).is_some_and(|m| (m.state as i8) >= 0);
            if !alive {
                free(w, id);
                w.delete_moby(id);
                return;
            }
            if !near {
                free(w, id);
                return;
            }
            let mut t = c::pi32(w, id, pv::TIMER);
            if fast_dec_timer(&mut t) != 0 {
                let dur = w.rng.rand_range(c::pi32(w, id, pv::HOP_LO), c::pi32(w, id, pv::HOP_HI));
                let to = c::pi32(w, id, pv::TO);
                c::set_pi32(w, id, pv::FROM, to);
                t = dur;
                c::set_pi32(w, id, pv::DUR, dur);
                let n = c::pi32(w, id, pv::COUNT);
                let k = w.rng.rand_range(1, n - 1);
                let half = (c::pi32(w, id, pv::HALF) == 0) as i32;
                c::set_pi32(w, id, pv::HALF, half);
                c::set_pi32(w, id, pv::TO, (to + k) % n);
            }
            c::set_pi32(w, id, pv::TIMER, t);
            let dur = c::pi32(w, id, pv::DUR);
            let half = c::pi32(w, id, pv::HALF) != 0;
            let mut a = ((dur - t) as f32 / dur as f32) * std::f32::consts::PI * 0.5;
            if half { a += std::f32::consts::FRAC_PI_2; }
            let s = a.sin();
            let key = |w: &World, o: usize| c::pv4(w, id, pv::KEYS + 0x10 * (c::pi32(w, id, o).clamp(0, 3) as usize));
            let (ka, kb) = if half { (key(w, pv::TO), key(w, pv::FROM)) } else { (key(w, pv::FROM), key(w, pv::TO)) };
            let mut col = lerp(ka, kb, s);
            let r75 = radius * 0.75;
            if r75 < d { col = lerp(col, [0.0; 4], (d - r75) / (radius - r75)); }
            c::set_pv4(w, id, pv::COLOUR, col);
            let l = PointLight { color: [col[0], col[1], col[2]], intensity: col[3], pos: [pos[0], pos[1], pos[2]], radius: c::pf(w, id, pv::LIGHT_R) };
            let slot = c::pi32(w, id, pv::SLOT);
            if slot != -1 {
                if let Ok(i) = usize::try_from(slot) { w.svc.point_lights.set(i, l); }
            } else {
                let load = f32::from_bits(w.svc.frame_load[1].0);
                let got = w.svc.point_lights.alloc(l, load).map_or(-1, |i| i as i32);
                c::set_pi32(w, id, pv::SLOT, got);
            }
        }
        _ => {}
    }
}
