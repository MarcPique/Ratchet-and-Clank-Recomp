//! **Rilgar's sewer fog switch, class 841** (level05 `0x30e1f8`, 1 placed; census U198; the name is descriptive [L]).
//! Entering its "in" cuboid (pvar +0x2c) swaps the level fog (0x15f444..0x15f454) for its own murky green one; its
//! "out" cuboid (+0x28) swaps the saved level fog back, and while Ratchet is in that cuboid his water level 0x13f640
//! is 500 (all of it water). Inside its third cuboid (+0x38) the underwater tint 0x161200 blends across it, from the
//! tint the level had at load to a brown. Read from the level05 decomp.
//!
//! **Pvars**: the saved fog (+0x00 near, +0x04 near F, +0x08 far, +0x0c far F, +0x20..+0x22 its colour), the sewer
//! fog (+0x10 near 0, +0x14 near F 255, +0x18 far 33792, +0x1c far F 0, +0x23..+0x25 its colour (0x49, 0x53, 0x0c)),
//! the cuboids +0x28 out / +0x2c in / +0x38 the blend, the tints +0x30 (the load's) / +0x34 (0x40408070).
//!
//! | address | what | port |
//! |---|---|---|
//! | every tick | Ratchet (0x13f3d0) in +0x28 (`PointInCuboid` 0x289c68) → 0x13f640 = 500 | [`update`] (`HeroFields::water_level`) |
//! | state 0 | the level fog saved; the sewer fog set up; +0x30 = the tint word 0x161200; → 1 | [`update`] |
//! | state 1 | Ratchet in +0x2c → the level fog = the sewer fog, → 2 | [`update`] (`WaterWorld::store_fog`) |
//! | state 2 | Ratchet in +0x28 → the level fog = the saved one, → 1 | [`update`] |
//! | every tick | Ratchet in +0x38: l = (Ratchet − its centre)·its inverse rows; the tint 0x161200 = `FastTweenColor((l.x + 1)/2, +0x30, +0x34)` | [`update`] |

use crate::fog_zones::FogGlobals;
use crate::moby_runtime::MobyId;
use crate::moby_update::creature as c;
use crate::moby_update::services::World;

pub const REFERENCE_LEVEL: u32 = 5;
pub const UPDATE_FN: u32 = 0x30_e1f8;
pub const CLASSES: [i16; 1] = [841];
/// Ratchet's water level in the out cuboid.
pub const WATER: f32 = 500.0;
pub const SEWER_COLOUR: [u8; 3] = [0x49, 0x53, 0x0c];
pub const SEWER_FAR: f32 = 33792.0;
pub const BROWN: u32 = 0x4040_8070;

/// Pvar offsets (module doc).
pub mod pv {
    pub const SAVED: usize = 0x00;
    pub const SEWER: usize = 0x10;
    pub const SAVED_RGB: usize = 0x20;
    pub const SEWER_RGB: usize = 0x23;
    pub const OUT: usize = 0x28;
    pub const IN: usize = 0x2c;
    pub const TINT_A: usize = 0x30;
    pub const TINT_B: usize = 0x34;
    pub const BLEND: usize = 0x38;
    pub const SIZE: usize = 0x3c;
}

/// A fog record: near, near F, far, far F at `o`, the colour at `rgb`.
fn read(w: &World, id: MobyId, o: usize, rgb: usize) -> FogGlobals {
    FogGlobals {
        color: [c::pu8(w, id, rgb), c::pu8(w, id, rgb + 1), c::pu8(w, id, rgb + 2)],
        near_dist: c::pf(w, id, o),
        near_intensity: c::pf(w, id, o + 4),
        far_dist: c::pf(w, id, o + 8),
        far_intensity: c::pf(w, id, o + 0xc),
    }
}

fn write(w: &mut World, id: MobyId, o: usize, rgb: usize, g: &FogGlobals) {
    for k in 0..3 { c::set_pu8(w, id, rgb + k, g.color[k]); }
    c::set_pf(w, id, o, g.near_dist);
    c::set_pf(w, id, o + 4, g.near_intensity);
    c::set_pf(w, id, o + 8, g.far_dist);
    c::set_pf(w, id, o + 0xc, g.far_intensity);
}

fn hero_in(w: &World, id: MobyId, o: usize) -> bool { w.in_cuboid(w.hero_point(), c::pi32(w, id, o)) }

/// Level05 `0x30e1f8` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { w.mm(id).pvars.resize(pv::SIZE, 0); }
    if hero_in(w, id, pv::OUT) { w.hero_fields_mut().water_level = WATER; }
    match w.m(id).state {
        0 => {
            if let Some(g) = w.svc.water.fog { write(w, id, pv::SAVED, pv::SAVED_RGB, &g); }
            let sewer = FogGlobals { color: SEWER_COLOUR, near_dist: 0.0, far_dist: SEWER_FAR, near_intensity: 255.0, far_intensity: 0.0 };
            write(w, id, pv::SEWER, pv::SEWER_RGB, &sewer);
            c::set_pi32(w, id, pv::TINT_B, BROWN as i32);
            let t = w.svc.water.look.tint;
            c::set_pi32(w, id, pv::TINT_A, u32::from_le_bytes(t) as i32);
            w.mm(id).state = 1;
        }
        1 if hero_in(w, id, pv::IN) => {
            let g = read(w, id, pv::SEWER, pv::SEWER_RGB);
            let t = w.counter;
            w.svc.water.store_fog(t, g);
            w.mm(id).state = 2;
        }
        2 if hero_in(w, id, pv::OUT) => {
            let g = read(w, id, pv::SAVED, pv::SAVED_RGB);
            let t = w.counter;
            w.svc.water.store_fog(t, g);
            w.mm(id).state = 1;
        }
        _ => {}
    }
    if hero_in(w, id, pv::BLEND) {
        let Some(s) = w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, c::pi32(w, id, pv::BLEND)).copied() else { return };
        let h = super::hero_pos(w);
        let d = [h[0] - s.matrix[3][0], h[1] - s.matrix[3][1], h[2] - s.matrix[3][2]];
        let lx = d[0] * s.inverse[0][0] + d[1] * s.inverse[1][0] + d[2] * s.inverse[2][0];
        let (a, b) = (c::pi32(w, id, pv::TINT_A) as u32, c::pi32(w, id, pv::TINT_B) as u32);
        let t = crate::particles::tween_color(((lx + 1.0) * 0.5).to_bits(), a, b);
        w.svc.water.look.tint = t.to_le_bytes();
    }
}
