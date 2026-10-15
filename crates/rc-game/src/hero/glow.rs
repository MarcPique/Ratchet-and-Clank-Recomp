//! **The hero's glow sprites** (level01): the glow-sprite list 0x141120 (`FUN_00229738(size, pull, point, rgba)`: up to
//! eight points 0x141120 + 16k, colours 0x1411a0, sizes 0x1411c0, pulls 0x1411e0, count 0x141200; cleared at the start
//! of the hero update 0x228870), its writer in `HeroItemsUpdate` (`0x2297b0`: the O2 Mask's two lights, the glowing hand
//! items, Ratchet's own glow word) and the hero's draw callback `0x229440` (registered once a frame by `0x229400`): in
//! the scene / space modes 2 / 6 the list only; on foot the red dot over Clank's antenna (0x1410d0, the antenna glow
//! 1204's point `HeroItemsAttach` writes) while the Heli- or Thruster-Pack is on and Clank shows, then the list; Clank
//! (body 1) his antenna dot, its size sprung; Giant Clank (body 2) his; the disguise (body 3) its three glow points
//! (`super::hologuise`, `Callback::DisguiseGlow`). Each sprite is the glow quad `0x2781d0` (`rc-engine` `fx_draw::
//! glow_quad`). Native `f32`.
//!
//! | address | call / branch | port |
//! |---|---|---|
//! | `0x2297b0` head | the head item O2 Mask (0x1404a8 = 5) shown: two sprites (0.057, 0.15, 0x301eaa1e) at its joint lists 0 / 1 | [`items_glow`] |
//! | | the phase 0x140978 + 130°/s, Ratchet's glow word = tween(sin·½ + ½, 0x806e8c6e, 0x805a3232) | `items_glow` |
//! | | no hand moby or it is hidden: return | `items_glow` |
//! | case 0xd (Visibomb) | its phase 0x14040c + 270°/s; the word toward 0x2814d214 (0x281414d2 within 0..120°) by 0.05; a sprite (0.1, 0.03) at joint list 0 in the word | `items_glow` |
//! | case 0x10 (Pyrocitor) | after 50 ticks ready (+0x20): the phase + 180°/s (270° with a weapon flag 0x1413f8), the colour (30 + 20s, 150 + 90s, 30 + 20s; red and blue swapped with the flag) at alpha 0x20, the word toward it by 0.13; a sprite (0.15, 0.04) at joint list 1 at alpha min(ticks − 50, 32) | `items_glow` |
//! | case 0x11 (Mine Glove) / 0x14 (Glove of Doom) | the phase + 250°/s; the word toward 0x28c8c8c8 / 0xdcdcdc (0x280a0a0a / 0xa0a0a within 0..130° / 140°) by 0.07; a sprite (0.15 / 0.17, 0.1) at the item's local (0.0174, −0.0392, 0.0962) / (0.0171, −0.0939, 0.1410) in a colour of the word's bytes | `items_glow` |
//! | case 0x13 (Tesla Claw) | the phase + 130°/s (420° with the flag), the word toward (100 + 70s, 70 + 40s, 70 + 40s) (with the flag (50 + 30s, 50 + 30s, 170 + 90s)) by 0.15; no sprite | `items_glow` |
//! | `0x229440` | modes 2 / 6: the list; body 0: the dot (0.057, 0.08, the antenna point + 0.015 up, 0x280000c0) with the back item 2 / 3 and Clank shown, the list; body 2: the dot (0.25, 0.08, joint list 6 − 0.07, 0x300000c0); body 1: the size sprung (`0x270728`, 0.003) to 0.07 (0.1015 while the command flash runs; ×1.7 with the cheat 0x15edb3), the dot (size, 0.08, joint list 7 − 0.02 (0.01 with the cheat), 0x141650) | [`body_dot`], [`ratchet_dot`]; `rc-engine` fx_draw |
//!
//! Not here: the Map-o-Matic / Bolt Grabber / Persuader mobys' glow words (`0x249ea0`; they only show on a scene's
//! Ratchet actor, G-CUT-009) and the scene / space Clank actors' sprites (`FUN_00228bc0`, G-CUT-009).

use super::Hero;
use crate::hud::tween_color;

/// Where a sprite sits; the engine resolves the item-relative ones from its attachment poses.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum At {
    /// A world point.
    Point([f32; 3]),
    /// The hand item's joint list `k`'s point (`FUN_002645a8(hand, k)`).
    Hand(u8),
    /// The hand item's rows · v + its position.
    HandLocal([f32; 3]),
    /// The head item's joint list `k`'s point.
    Head(u8),
    /// The antenna glow 1204's position (0x1410d0) + `dz` up.
    Antenna(f32),
}

/// One glow quad `0x2781d0(size, pull, point, rgba)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sprite {
    pub size: f32,
    pub pull: f32,
    pub at: At,
    pub rgba: u32,
}

/// The glow state (module doc).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HeroGlow {
    /// 0x14040c: the hand item's glow phase; the hand item's glow word +0x90 (None until a glowing item is in hand) and
    /// the item it belongs to.
    pub hand_phase: f32,
    pub hand_word: Option<u32>,
    pub hand_id: i32,
    /// Ratchet's glow word +0x90 (`0x2297b0`).
    pub ratchet: u32,
    /// The list 0x141120 of this tick.
    pub list: Vec<Sprite>,
    /// The drawer's own quad this tick (the antenna dot of bodies 0 / 1 / 2).
    pub dot: Option<Sprite>,
    /// 0x14097c: Clank's dot size.
    pub dot_size: f32,
}

const O2_MASK: i32 = 5;

/// `fast_add_rotations`.
fn add_rot(a: f32, b: f32) -> f32 { crate::moby_update::creature::add_rot(a, b) }

/// The hand item's glow word as the tween starts from it (its moby's +0x90 at creation: 0).
fn word(g: &HeroGlow, id: i32) -> u32 { if g.hand_id == id { g.hand_word.unwrap_or(0) } else { 0 } }

/// `0x2297b0` in `HeroItemsUpdate` (module doc): the list starts empty each tick (0x228870 clears it).
pub fn items_glow(h: &mut Hero, head_shown: bool, hand_shown: bool) {
    let dt = super::physics::DT.to_f32();
    h.glow.list.clear();
    if h.head_slot.id == O2_MASK && head_shown {
        for k in 0..2 { h.glow.list.push(Sprite { size: f32::from_bits(0x3d69_78d5), pull: f32::from_bits(0x3e19_999a), at: At::Head(k), rgba: 0x301e_aa1e }); }
    }
    h.bodies.glow_phase = add_rot(h.bodies.glow_phase, dt * 2.268_928);
    h.glow.ratchet = tween_color(h.bodies.glow_phase.sin() * 0.5 + 0.5, 0x806e_8c6e, 0x805a_3232);
    if !hand_shown || h.items.slot.item.is_none() { return; }
    let id = h.items.slot.id;
    let flag = h.f13f8 != 0;
    let g = &mut h.glow;
    let cur = word(g, id);
    let set = |g: &mut HeroGlow, w: u32| {
        g.hand_word = Some(w);
        g.hand_id = id;
    };
    match id {
        0xd => {
            g.hand_phase = add_rot(g.hand_phase, dt * 4.712_389);
            let to = if 0.0 < g.hand_phase && g.hand_phase < 2.094_395_2 { 0x2814_14d2 } else { 0x2814_d214 };
            let w = tween_color(0.05, cur, to);
            set(g, w);
            g.list.push(Sprite { size: f32::from_bits(0x3dcc_cccd), pull: f32::from_bits(0x3cf5_c28f), at: At::Hand(0), rgba: w });
        }
        0x10 => {
            let ready = h.items.slot.ticks_ready;
            let t50 = super::physics::ticks(0x32);
            if t50 < ready {
                let k = if flag { 4.712_389 } else { std::f32::consts::PI };
                g.hand_phase = add_rot(g.hand_phase, dt * k);
                let s = g.hand_phase.sin();
                let (a, b, c) = ((s * 20.0) as i32, (s * 90.0) as i32, (s * 20.0) as i32);
                let (mut gg, mut rr) = ((b + 0x96) as u32, (a + 0x1e) as u32);
                if flag { (gg, rr) = ((a + 0x1e) as u32, (b + 0x96) as u32); }
                let bb = ((c + 0x1e) as u32) << 16;
                let w = tween_color(f32::from_bits(0x3e05_1eb8), cur, bb | 0x2000_0000 | gg << 8 | rr);
                set(g, w);
                let alpha = (ready - t50).min(0x20) as u32;
                g.list.push(Sprite { size: f32::from_bits(0x3e19_999a), pull: f32::from_bits(0x3d23_d70a), at: At::Hand(1), rgba: alpha << 24 | bb | gg << 8 | rr });
            }
        }
        0x11 | 0x14 => {
            let mine = id == 0x11;
            g.hand_phase = add_rot(g.hand_phase, dt * 4.363_323);
            let edge = if mine { 2.268_928 } else { 2.443_461 };
            let on = 0.0 < g.hand_phase && g.hand_phase < edge;
            let to = match (mine, on) { (true, true) => 0x280a_0a0a, (true, false) => 0x28c8_c8c8, (false, true) => 0x000a_0a0a, (false, false) => 0x00dc_dcdc };
            let w = tween_color(f32::from_bits(0x3d8f_5c29), cur, to);
            set(g, w);
            let local = if mine { [0x3c8e_17a0, 0xbd20_785e, 0x3dc5_1698] } else { [0x3c8c_1e21, 0xbdc0_58d3, 0x3e10_5647] }.map(f32::from_bits);
            let (b0, b1, b2) = (w & 0xff, (w >> 8) & 0xff, (w >> 16) & 0xff);
            let rgba = ((b1 << 5) / 200) << 24 | ((b2 as f32 * 0.2) as u32) << 16 | b1 << 8 | (b0 as f32 * 0.2) as u32;
            let size = f32::from_bits(if mine { 0x3e19_999a } else { 0x3e2e_147b });
            g.list.push(Sprite { size, pull: f32::from_bits(0x3dcc_cccd), at: At::HandLocal(local), rgba });
        }
        0x13 => {
            g.hand_phase = add_rot(g.hand_phase, dt * if flag { 7.330_383 } else { 2.268_928 });
            let s = g.hand_phase.sin();
            let (r, gg, b) = if flag {
                ((s * 30.0) as i32 + 0x32, (s * 30.0) as i32 + 0x32, (s * 90.0) as i32 + 0xaa)
            } else {
                ((s * 70.0) as i32 + 100, (s * 40.0) as i32 + 0x46, (s * 40.0) as i32 + 0x46)
            };
            let w = tween_color(f32::from_bits(0x3e19_999a), cur, (b as u32) << 16 | 0x8000_0000 | (gg as u32) << 8 | r as u32);
            set(g, w);
        }
        _ => {}
    }
}

/// `0x229440`'s dot on foot (body 0): the Heli- (2) or Thruster-Pack (3) on the back and Clank shown (0x141628 = 0);
/// drawn at the antenna glow's point when `HeroItemsAttach` placed it this frame (the engine knows).
pub fn ratchet_dot(h: &mut Hero) {
    if h.mode != 0 { return; }
    let back = h.back_slot.slot.id;
    if matches!(back, 2 | 3) && h.back_slot.clank_hidden == 0 {
        h.glow.dot = Some(Sprite { size: f32::from_bits(0x3d69_78d5), pull: f32::from_bits(0x3da3_d70a), at: At::Antenna(0.015), rgba: 0x2800_00c0 });
    }
}

/// `0x229440`'s dot for Clank (body 1) and Giant Clank (body 2) at `point` (joint list 7 / 6's, `HeroUpdateAlt` +14 /
/// +21), the size sprung for Clank by 0.003 a frame.
pub fn body_dot(h: &mut Hero, point: [f32; 4]) {
    match h.mode {
        1 => {
            let big = h.cheats.on(crate::cheats::slot::CLANK);
            let mut target = if h.bodies.flash != 0 { 0.101_5 } else { 0.07 };
            let dz = if big { 0.01 } else { 0.02 };
            if big { target *= 1.7; }
            let step = 0.003;
            let d = (target - h.glow.dot_size).clamp(-step, step);
            h.glow.dot_size += d;
            h.glow.dot = Some(Sprite { size: h.glow.dot_size, pull: f32::from_bits(0x3da3_d70a), at: At::Point([point[0], point[1], point[2] - dz]), rgba: h.bodies.glow_colour });
        }
        2 => {
            h.glow.dot = Some(Sprite { size: 0.25, pull: f32::from_bits(0x3da3_d70a), at: At::Point([point[0], point[1], point[2] - 0.07]), rgba: 0x3000_00c0 });
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mine_glove_colour_from_word_bytes() {
        let mut h = Hero::new();
        h.items.slot.id = 0x11;
        let anim = rc_formats::moby_anim::AnimState { seq_a: 1, frame_a: 0, seq_b: 1, frame_b: 0, t: 0.0, speed: 1.0, rate: 1.0, flags: 0, trigger_count: 0, skip_advance: false };
        h.items.slot.item = Some(super::super::items::HandItem { o_class: 0, mstate: 0, anim, snapshot: None, scale: 1.0, position: [0.0; 3], rows: [[0; 4]; 3], hit_timer: 0, flight: Default::default() });
        items_glow(&mut h, false, true);
        let s = h.glow.list.last().copied().unwrap();
        let w = h.glow.hand_word.unwrap();
        let (b0, b1, b2) = (w & 0xff, (w >> 8) & 0xff, (w >> 16) & 0xff);
        assert_eq!(s.rgba >> 24, (b1 << 5) / 200);
        assert_eq!((s.rgba >> 8) & 0xff, b1);
        assert_eq!(s.rgba & 0xff, (b0 as f32 * 0.2) as u32);
        assert_eq!((s.rgba >> 16) & 0xff, (b2 as f32 * 0.2) as u32);
        assert!(matches!(s.at, At::HandLocal(_)));
    }

    #[test]
    fn red_dot_needs_a_pack_and_clank_shown() {
        let mut h = Hero::new();
        h.back_slot.slot.id = 2;
        ratchet_dot(&mut h);
        assert!(h.glow.dot.is_some());
        h.glow.dot = None;
        h.back_slot.clank_hidden = 1;
        ratchet_dot(&mut h);
        assert!(h.glow.dot.is_none());
        h.back_slot.clank_hidden = 0;
        h.back_slot.slot.id = 4;
        ratchet_dot(&mut h);
        assert!(h.glow.dot.is_none());
    }
}
