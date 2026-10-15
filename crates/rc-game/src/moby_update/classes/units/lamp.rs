//! The lamps, class 1060: level00 0x2df4f8, the same code on levels 00 / 02 / 05 / 18 (census U27; 334 created
//! instances, 271 of them on 18). A pulsing ambient colour and a glow: each tick the lamp's phase advances by 180°/s,
//! its colour tweens between 0x404070 and 0x202020 with `sin(phase)·½ + ½`, it becomes the lamp's ambient and its
//! glow colour, and the lamp's moby group registers one draw callback that puts a glow quad on every visible lamp
//! of the group. Read from the level00 decomp (0x2df4f8, 0x2df4a8, the callback 0x2df3d8); every constant was
//! checked on the four copies (`$gp` 0x161b90..98 on 00, 0x161ec0.. on 02, 0x161f24.. on 05, 0x162100.. on 18: the
//! same bytes). Native `f32`.
//!
//! **Pvar block**: +0x00 f32 the phase (degrees in the level data, radians after the init), +0x04 the glow RGBA,
//! +0x08 the group's registration word: the loader points it into the level's pvar shared data (gameplay 0x4c: one
//! word per moby group, `Services::pvar_shared`).
//!
//! **0x2df4f8** (the update):
//! * state 0: phase ·= π/180 (`0x3c8efa35`), → 1.
//! * state 1: phase = `fast_add_rotations(phase, 180·π/180·dt)`; `c = FastTweenColor(sin(phase)·½ + ½, 0x404070,
//!   0x202020)`; +0x04 = `a << 24 | c & 0xffffff` with `a` = 0x20 on levels 0 and 0x12, else 0x38; the registration
//!   0x2df4a8; `0x2650d0(m, c & 0xff, (c >> 16) & 0xff, c >> 24)`: the ambient bytes +0x3c..+0x3e = (red, **blue**,
//!   alpha 0) of the tween (the game passes the blue byte as the green: kept).
//!
//! **0x2df4a8** (registration): when the lamp has a group (+0x21 ≠ −1), the tick counter `0x15f5cc` ≠ 0 and the
//! group's word ≠ the counter: word = counter, `RegisterDrawCallback(0x2df3d8, m)` (level00 0x1f93e0 = list 1).
//!
//! **0x2df3d8** (the callback; draw only): size 1.2 (`0x3f99999a`; 1.0 on levels 0 and 0x12), for each member of
//! the lamp's group (`0x26e150` / `0x26e238`) that was drawn last frame (+0x31) and has the lamp's class: the glow
//! quad `0x2781d0(size, 0.5, member position, member +0x04)` ([`glow_quads`]).
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2df4f8 state 0 | phase degrees → radians, state 1 | [`update`] |
//! | 0x2df4f8 state 1 | `fast_add_rotations` / `fast_sin` / `FastTweenColor` (0x2000e8 / 0x1ff800 / 0x200298 = L01 0x221ff8 / 0x221710 / 0x2221a8) | [`update`] (`add_rot`, `sin`, `hud::tween_color`) |
//! | | level branch `0x15ed84 ∈ {0, 0x12}` → alpha 0x20, else 0x38 | [`alpha`] |
//! | | glow RGBA → +0x04 | [`update`] |
//! | 0x2df4a8 | group ≠ −1, counter ≠ 0, shared word ≠ counter → word = counter, `RegisterDrawCallback` (list 1) | [`register`] (`Callback::UnitGlow`) |
//! | 0x2502f0 (= L01 0x2650d0) | ambient bytes (r, b, 0) | [`update`] (`Moby::ambient`) |
//! | 0x2df3d8 | size 1.2 / 1.0 (levels 0, 0x12); group walk `0x26e150` / `0x26e238`; +0x31 and same class; glow quad `0x263618` (= L01 0x2781d0) | [`glow_quads`] + `rc-engine` fx_draw |
//! | | no sound, particle, light, other moby, save flag | n/a |

use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::{add_rot, DT};
use crate::moby_update::services::{pvar as p, Services, World};
use super::GlowQuad;

/// The update in the level00 class table.
pub const UPDATE_FN: u32 = 0x2d_f4f8;
pub const REFERENCE_LEVEL: u32 = 0;
pub const CLASSES: [i16; 1] = [1060];

/// The tween's end colours (`$gp` words, the same on the four levels).
pub const COLOUR_A: u32 = 0x40_4070;
pub const COLOUR_B: u32 = 0x20_2020;
/// The phase speed, degrees per second (`$gp` float 180.0).
pub const SPEED_DEG: f32 = 180.0;
/// π/180 as the code loads it.
const DEG: f32 = f32::from_bits(0x3c8e_fa35);

/// Veldin (0) and the Veldin return (0x12) draw the lamps dimmer and smaller.
fn veldin(level: u32) -> bool { level == 0 || level == 0x12 }

/// The glow alpha of the level.
pub fn alpha(level: u32) -> u32 { if veldin(level) { 0x20 } else { 0x38 } }

/// The callback's glow size on the level.
pub fn glow_size(level: u32) -> f32 { if veldin(level) { 1.0 } else { f32::from_bits(0x3f99_999a) } }

/// Level00 0x2df4f8 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0xc { return; }
    match w.m(id).state {
        0 => {
            let ph = p::ff(&w.m(id).pvars, 0) * DEG;
            let m = w.mm(id);
            p::set_ff(&mut m.pvars, 0, ph);
            m.state = 1;
        }
        1 => {
            let ph = add_rot(p::ff(&w.m(id).pvars, 0), SPEED_DEG * DEG * DT);
            let c = crate::hud::tween_color(ph.sin() * 0.5 + 0.5, COLOUR_A, COLOUR_B);
            let a = alpha(w.svc.level);
            let m = w.mm(id);
            p::set_ff(&mut m.pvars, 0, ph);
            p::set_u32(&mut m.pvars, 4, a << 24 | c & 0xff_ffff);
            register(w, id);
            let m = w.mm(id);
            m.ambient[0] = c as u8;
            m.ambient[1] = (c >> 16) as u8;
            m.ambient[2] = (c >> 24) as u8;
        }
        _ => {}
    }
}

/// Level00 0x2df4a8: one callback per group and tick (module doc).
fn register(w: &mut World, id: MobyId) {
    if w.m(id).group == -1 { return; }
    let tick = w.counter as i32;
    if tick == 0 { return; }
    let word = p::i32(&w.m(id).pvars, 8);
    if w.svc.shared_i32(word) == tick { return; }
    w.svc.set_shared_i32(word, tick);
    if let Some(i) = super::row(REFERENCE_LEVEL, UPDATE_FN) { w.svc.draw_callbacks.register(Callback::UnitGlow(i), id); }
}

/// Level00 0x2df3d8 for the lamp `id` (module doc).
pub fn glow_quads(table: &MobyTable, svc: &Services, id: MobyId) -> Vec<GlowQuad> {
    let Some(lamp) = table.mobys.get(id) else { return Vec::new() };
    let size = glow_size(svc.level);
    let Some(Some(list)) = usize::try_from(lamp.group).ok().and_then(|g| svc.groups.lists.get(g)) else { return Vec::new() };
    list.iter()
        .filter_map(|&e| table.mobys.get((e & 0x7fff) as usize))
        .filter(|m| m.visible != 0 && m.o_class == lamp.o_class && m.pvars.len() >= 8)
        .map(|m| GlowQuad { size, pull: 0.5, point: [m.position[0], m.position[1], m.position[2]], rgba: p::u32(&m.pvars, 4) })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};
    use crate::moby_update::scheduler::Groups;
    use std::f32::consts::PI;

    fn lamp(phase_deg: f32, group: i8) -> Moby {
        let mut m = Moby { o_class: 1060, group, visible: 1, pvars: vec![0; 16], ..Moby::default() };
        p::set_ff(&mut m.pvars, 0, phase_deg);
        m
    }

    #[test]
    fn pulse_colour_and_one_callback_per_group() {
        let mut t = MobyTable::new(vec![lamp(90.0, 0), lamp(0.0, 0), lamp(0.0, -1)], 4);
        let hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = Services::new();
        svc.level = 18;
        svc.pvar_shared = vec![0; 8];
        svc.groups = Groups { lists: vec![Some(vec![0, 1])] };
        for c in 0..2 {
            let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, c);
            for id in 0..3 { update(&mut w, id); }
        }
        // Tick 0 inits (and would not register: counter 0); tick 1 pulses and the group registers once.
        let m = &t.mobys[0];
        let ph = PI / 2.0 + PI * DT;
        assert_eq!(p::ff(&m.pvars, 0), ph);
        let c = crate::hud::tween_color(ph.sin() * 0.5 + 0.5, COLOUR_A, COLOUR_B);
        assert_eq!(p::u32(&m.pvars, 4), 0x2000_0000 | c);
        assert_eq!(m.ambient[..3], [c as u8, (c >> 16) as u8, 0]);
        assert_eq!(svc.draw_callbacks.list1.len(), 1, "one registration per group and tick; none without a group");
        assert_eq!(svc.shared_i32(0), 1);
        let q = glow_quads(&t, &svc, 0);
        assert_eq!(q.len(), 2);
        assert_eq!((q[0].size, q[0].pull, q[1].rgba), (1.0, 0.5, p::u32(&t.mobys[1].pvars, 4)));
        // A lamp not drawn last frame gets no quad.
        t.mobys[1].visible = 0;
        assert_eq!(glow_quads(&t, &svc, 0).len(), 1);
        assert_eq!((alpha(5), glow_size(5)), (0x38, f32::from_bits(0x3f99_999a)));
    }
}
