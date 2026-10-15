//! Buried bolt caches, class 605: `NearestAreaMarkerUpdate` level01 0x2f2eb8, the same code on levels 1–18 (every
//! level but Veldin 00; 358 placed instances, ~20 per level). The Metal Detector's targets: an invisible moby per
//! cache that, every tick, offers itself as "the nearest cache" to the Metal Detector (item 27, the hand item
//! moby class 585, `FUN_002f2280`) and to the HUD's nearby-bolt alert (element 7, `HudBoltAlertShow` 0x227d90).
//! Read from the level01 decomp (level01 0x2f2eb8) and its two
//! readers; the level copies were matched with `rc_formats::level_overlay::Relocation` (all 18: one function).
//!
//! **Pvar block** (P, s32 words): P[0..3] the cache point (the moby's position, copied at init), P[4] a path
//! (−1: none) and P[5] a cuboid (−1: none) that bound where the detector can find it, P[6] the cache's bolt value.
//! `moby+0xbc`: the cache's number on its level (1..), the index of its nibble in the level's save bytes.
//!
//! **Init** (state 0; no pvars → `DeleteMoby`): update distance 0xff, P[0..3] = position, state 1, mode |= 0x41
//! (hidden, no animation). Once per cache (+0xbc = 0) and on levels < 20: the running counter (gp−0x5110, reset
//! to 0 by any cache's update in state ≠ 0, so it numbers the caches in load-pass order) picks the nibble
//! `0x14bf10[level·16 + n/2]` (the high nibble for even n, the low for odd), +0xbc = ++n, and the value drops by
//! 5 bolts per dig already made (`P[6] −= nibble·5`); a cache with nothing left (`P[6] < 1`) is deleted. The
//! nibbles are save chunk 3008 (`GameState::levels[l].metal_detector`), written by the detector's dig
//! (`FUN_002f21c0`: +1, saturating at 15).
//!
//! **Tick** (the init falls through to it): the probe point is the detector's tip (the hand item moby's pvar+0x10)
//! while the hand item is the Metal Detector (`0x1403e0 ≠ 0`, `0x140408 == 0x1b`), else Ratchet's feet 0x13f3d0.
//! The cache takes part when the point is in its path (`PointInPathPolygon`), or in its cuboid, or it has neither;
//! then with d = `VecDistance2` (xy) from the cache: d < the nearest distance 0x141394 → nearest 0x141390 = this
//! cache, distance = d, and the alert flag 0x141398 = 1 when d < 20.
//!
//! **The alert frame** (`HudBoltAlertShow` 0x227d90, in the hero's frame after the moby pass: [`alert_frame`]):
//! with the Metal Detector owned (`0x13d4db`, item 27) the flag is `nearest ≠ 0 && distance < 20` and element 7 is
//! requested (`queue_animation_update(7, 0x753a, …)`: `Services::hud`, drawn by `crate::hud`'s bolt alert), then the
//! nearest is cleared (distance 100000; the values kept in [`Globals::seen`] for the detector, which the game runs
//! before this frame part); without it, flag 0 and nothing is cleared. The detector's aim, beep, dig (bolts from the
//! cache, the nibble write [`dig`], `DeleteMoby`) and scan draw ([`scan_frame`]) are `crate::hero::metal_detector`
//! (2026-10-01, G-WPN-006).

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::services::{pvar as p, World};

/// The update address in the level01 class table.
pub const UPDATE_FN: u32 = 0x2f2eb8;
/// Classes that run [`update`] (level01's table; the same function runs 605 on levels 1–18).
pub const CLASSES: [i16; 1] = [605];

/// The Metal Detector's item id (`0x13d4c0 + 0x1b` = `0x13d4db`; hand item `0x140408 == 0x1b`).
pub const METAL_DETECTOR: usize = 27;
/// The alert's range (`20.0`, both the update and `HudBoltAlertShow`).
pub const ALERT_RANGE: f32 = 20.0;
/// The distance the alert frame resets the search to (`100000.0`).
pub const FAR: f32 = 100000.0;
/// Bolts per dig (`nibble · 5`).
pub const BOLTS_PER_DIG: i32 = 5;

/// Pvar offsets.
pub mod pv {
    pub const POINT: usize = 0x00;
    pub const PATH: usize = 0x10;
    pub const CUBOID: usize = 0x14;
    pub const VALUE: usize = 0x18;
    pub const LEN: usize = 0x1c;
}

/// The game globals of the caches ([`crate::moby_update::Services::buried`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Globals {
    /// `0x141390`: the nearest cache this tick (0 = none).
    pub nearest: Option<MobyId>,
    /// `0x141394`: its xy distance (the hero block clear 0x226b70 leaves 0; the alert frame resets it to 100000).
    pub distance: f32,
    /// `0x141398`: a cache within 20 (the HUD's element 7 condition).
    pub alert: bool,
    /// gp−0x5110 (`0x161af0` on level01): the init's running cache counter.
    pub counter: u8,
    /// Element-7 requests made by [`alert_frame`] (`queue_animation_update(7, 0x753a, …)`), for the HUD port.
    pub alert_requests: u64,
    /// 0x141390 / 0x141394 as the moby pass left them, kept by [`alert_frame`] before it resets the search: what the
    /// Metal Detector's update reads later in the frame (`HudBoltAlertShow` runs after `HeroItemsUpdate` in the game,
    /// after the moby pass in the port; nothing between them reads or writes the two words).
    pub seen: Option<(MobyId, f32)>,
    /// The Metal Detector's scan draw `0x2f1e28` ([`Scan`]).
    pub scan: Scan,
}

/// The Metal Detector's scan (its item pvars +0x10 / +0x24 / +0x28 / +0x30 and colour +0x90 as the draw callback
/// `0x2f1e28` reads them, `crate::hero::metal_detector`): the state part runs in the frame's callbacks ([`scan_frame`]),
/// the renderer draws [`Scan::quads`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scan {
    /// +0x24 / +0x28: the phase (0..1, `+= dt` per draw) and how far it has reached since ○ was pressed.
    pub phase: f32,
    pub reach: f32,
    /// +0x10: the tip; +0x30: the head's axis.
    pub origin: [f32; 3],
    pub axis: [f32; 3],
    /// +0x90: the item's colour.
    pub glow: u32,
    /// The squares of the last draw (corners, colour), FX texture [`SCAN_FX`], additive.
    pub quads: Vec<([[f32; 3]; 4], u32)>,
}

/// The scan's FX texture (gp−0x5158 = 8).
pub const SCAN_FX: usize = 8;
/// The scan squares' ST.
pub const SCAN_ST: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];

/// `0x2f1e28` (registered on Ratchet's moby by the Metal Detector's update): the phase steps by dt (wrapping at 1), the
/// reach follows it, and the squares are laid out back from the phase in steps of 0.1 for one turn (module doc of
/// `crate::hero::metal_detector`).
pub fn scan_frame(w: &mut World) {
    let dt = crate::moby_update::services::DT.to_f32();
    let s = &mut w.svc.buried.scan;
    s.phase += dt;
    if 1.0 < s.phase { s.phase -= 1.0; }
    if s.reach < s.phase { s.reach = s.phase; }
    s.quads.clear();
    let unit = |a: [f32; 3]| {
        let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
        if l == 0.0 { [0.0; 3] } else { a.map(|x| x / l) }
    };
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let ax = unit(s.axis);
    let a = unit(cross(ax, [1.0, 0.0, 0.0]));
    let b = unit(cross(ax, a));
    let (mut d, mut k) = (s.phase, s.phase);
    while s.phase - 1.0 < k {
        if d <= s.reach {
            let half = d * 0.5 + 0.23;
            let c: [f32; 3] = std::array::from_fn(|i| s.origin[i] - ax[i] * d);
            let corner = |sa: f32, sb: f32| -> [f32; 3] { std::array::from_fn(|i| c[i] + a[i] * half * sa + b[i] * half * sb) };
            // v0 = c − b − a, v1 = c + b − a, v2 = c − b + a, v3 = c + b + a (the disassembly's order).
            let quad = [corner(-1.0, -1.0), corner(-1.0, 1.0), corner(1.0, -1.0), corner(1.0, 1.0)];
            let alpha = ((1.0 - d) * 255.0) as i32 as u32;
            s.quads.push((quad, (s.glow & 0x00ff_ffff) | (alpha << 24)));
        }
        d -= 0.1;
        if d < 0.0 { d += 1.0; }
        k -= 0.1;
    }
}

/// `0x2f21c0` (the dig, `crate::hero::metal_detector`): cache `n` (its +0xbc, 1..) of `level` dug once more: the save
/// nibble + 1, at most 15 (the saved game through `GameWrite::MetalDetectorDig`, and the copy the caches' init reads).
pub fn dig(w: &mut World, level: u32, n: u8) {
    let k = n.wrapping_sub(1);
    if let Some(bytes) = w.svc.interact.game.metal_detector_bits.get_mut(level as usize) {
        let b = &mut bytes[(k / 2) as usize % 16];
        *b = if k & 1 != 0 { (*b & 0xf0) | ((*b & 0xf) + 1).min(15) } else { (*b & 0x0f) | (((*b >> 4) + 1).min(15) << 4) };
    }
    w.svc.interact.writes.push(crate::moby_update::interact::GameWrite::MetalDetectorDig { level: level as usize, cache: n });
}

/// The dug-count nibble of cache `n` (0-based) in the level's 16 save bytes (`0x14bf10 + level·16`).
pub fn dug_nibble(bytes: &[u8; 16], n: u8) -> u8 {
    let b = bytes.get(n as usize / 2).copied().unwrap_or(0);
    if n & 1 == 0 { b >> 4 } else { b & 0xf }
}

/// `NearestAreaMarkerUpdate` 0x2f2eb8 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).state == 0 {
        if w.m(id).pvars.len() < pv::LEN {
            w.delete_moby(id);
            return;
        }
        let level = w.svc.level;
        let m = w.mm(id);
        m.update_dist = 0xff;
        let pos = m.position;
        p::set_v4f(&mut m.pvars, pv::POINT, pos);
        m.state = 1;
        m.mode |= mode::HIDDEN | mode::NO_ANIM;
        if m.cmd == 0 && level < 20 {
            let n = w.svc.buried.counter;
            let bytes = w.svc.interact.game.metal_detector_bits.get(level as usize).copied().unwrap_or([0; 16]);
            let dug = dug_nibble(&bytes, n) as i32;
            w.svc.buried.counter = n.wrapping_add(1);
            let m = w.mm(id);
            m.cmd = n.wrapping_add(1);
            let v = p::i32(&m.pvars, pv::VALUE).wrapping_sub(dug * BOLTS_PER_DIG);
            p::set_i32(&mut m.pvars, pv::VALUE, v);
            if v < 1 {
                w.delete_moby(id);
                return;
            }
        }
    } else {
        w.svc.buried.counter = 0;
    }
    // The probe point: the Metal Detector's tip (its pvar +0x10, `crate::hero::metal_detector`) while it is the hand
    // item, else Ratchet's feet.
    let slot = &w.hero.items.slot;
    let q = if slot.item.is_some() && slot.id == METAL_DETECTOR as i32 { w.hero.gadgets.detector.tip } else { crate::hero::physics::to_f32x3(w.hero.pos) };
    let (path, cuboid) = (p::i32(&w.m(id).pvars, pv::PATH), p::i32(&w.m(id).pvars, pv::CUBOID));
    let inside = (path != -1 && w.in_path(q, path)) || (cuboid != -1 && w.in_cuboid(q, cuboid)) || (path == -1 && cuboid == -1);
    if !inside { return; }
    let pos = w.m(id).position;
    let d = ((pos[0] - q[0]).powi(2) + (pos[1] - q[1]).powi(2)).sqrt();
    let g = &mut w.svc.buried;
    if d < g.distance {
        g.nearest = Some(id);
        g.distance = d;
        if d < ALERT_RANGE { g.alert = true; }
    }
}

/// `HudBoltAlertShow` 0x227d90 (module doc): run once per tick after the moby pass.
pub fn alert_frame(w: &mut World) {
    // 0x13d4db: the item-owned byte as the hero code reads it (Ratchet's mirror).
    let owned = w.hero.owned.0.get(METAL_DETECTOR).is_some_and(|&b| b != 0);
    let g = &mut w.svc.buried;
    if !owned {
        g.alert = false;
        return;
    }
    g.alert = g.nearest.is_some() && g.distance < ALERT_RANGE;
    g.seen = g.nearest.map(|n| (n, g.distance));
    g.distance = FAR;
    g.nearest = None;
    if g.alert {
        g.alert_requests += 1;
        // `queue_animation_update(7, 0x753a, 0x24b458, 0x24ee20, 0x24eed8, 0x141398, 1)`: the HUD's element 7.
        w.svc.hud.queue(crate::hud::Request::BOLT_ALERT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nibbles_high_then_low() {
        let mut b = [0u8; 16];
        b[0] = 0x31;
        b[1] = 0x0f;
        assert_eq!((dug_nibble(&b, 0), dug_nibble(&b, 1), dug_nibble(&b, 2), dug_nibble(&b, 3)), (3, 1, 0, 15));
        assert_eq!(dug_nibble(&b, 40), 0);
    }
}
