//! The Rilgar laser fences, class 838 (census U183, 26 instances; level05 update 0x30dc68, draw callback 0x30de90).
//! Read from the level05 decomp and the overlay's data (Ghidra `level05.elf`); native `f32`.
//!
//! The fences of a moby group share **one looping voice** (G-AUD-010): the group's slot word lives in the pvar shared
//! data, and each tick the fence nearest the camera takes the playing slot over by writing itself and its position into
//! the slot's record (`0x13e5c0 + slot·0x70` +0x18 / +0x20; [`World::hand_over_sound`]). The fence itself is hidden;
//! it is drawn as five red laser bars, one callback per tick for the group of the first fence to update.
//!
//! **Pvars** (P): +0x00 s32 the switch moby (−1 none): when its command byte +0xbc is set the fence goes off; +0x04 the
//! group's voice-slot word (an offset into the pvar shared data, [`crate::moby_update::Services::shared_i32`]).
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x30dc68 state 0 | → 1, +0x31 = 0, mode \|= 1 (hidden), slot word = −1 | [`update`] |
//! | state 1 | the level word gp−0x4ec8 ≠ tick → = tick, `RegisterDrawCallback` 0x228048 (= L01 0x21afe0, list 1) (0x30de90, self) | [`update`] (`Callback::UnitQuads`) |
//! | | `VecDistance(pos, camera 0x1671c0)` < 64: slot < 0, or its owner +0x18 null or not class 838 → `PlayClassSound(0, 4 (loop), self)` into the slot word | [`update`] (`World::play_sound`) |
//! | | owner = self: nothing; owner in state 1 and no farther from the camera: nothing; else the slot's owner := self, position := self +0x10 | [`update`] ([`World::hand_over_sound`], G-AUD-010) |
//! | | switch P+0 ≥ 0 with +0xbc ≠ 0 → `PlayClassSound(1, 0, self)` (the power-down), +0x94 = 0 (collision off), → 2 | [`update`] |
//! | state 2 | +0x94 ≠ 0 → 0 | [`update`] |
//! | 0x30de90 | phase = (tick & 0x3f)/63·2π; per bar i = 0..4: t = (sin phase + 1)/2, wide colour `FastTweenColor(t, 0x200000ff, 0x20000080)`, narrow `FastTweenColor(t, 0x300000ff, 0x30000080)` (gp−0x4ebc..−0x4eb8), phase += 2π/5 (`0x3fa0d97c`, `fast_add_rotations`) | [`fx_quads`] |
//! | | quads: the wide table 0x20b7a0 (y ±5, z ±0.1) and the narrow 0x20b760 (y ±5, z ±0.05), z + i + 1, ST (0.5, 0 / 1) from 0x20b740, FX 0x13 (gp−0x4e68), ALPHA 0x48 (gp−0x4e90..−0x4e84 = 0, 2, 0, 1: additive), TEX1 0xff9000000260 | [`fx_quads`] + `rc-engine` fx_draw |
//! | | for each member of the moby's group list 0x1abcc0 with class 838 in state 1: `FastDrawQuadReal(q, member rows +0xc0 / position +0x10, 0)`, wide then narrow per bar | [`fx_quads`] |
//! | | no particle, light, help, stat, save flag; the only other moby touched is the voice slot's owner | n/a |

use crate::moby_runtime::{mode, MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::services::{pvar as p, Services, World};
use super::{FxQuad, FxQuads};

pub const UPDATE_FN: u32 = 0x30_dc68;
pub const REFERENCE_LEVEL: u32 = 5;
pub const CLASSES: [i16; 1] = [838];

/// The level05 word gp−0x4ec8 (0x161d38): the tick of the last registration.
pub const TICK_WORD: u32 = 0x16_1d38;
/// The loop's hand-off range from the camera.
pub const RANGE: f32 = 64.0;
/// `FastTweenColor` ends: the wide bars (gp−0x4ebc / −0x4eb8) and the narrow ones (gp−0x4ec4 / −0x4ec0).
pub const WIDE: (u32, u32) = (0x2000_00ff, 0x2000_0080);
pub const NARROW: (u32, u32) = (0x3000_00ff, 0x3000_0080);
/// FX texture gp−0x4e68.
pub const FX: usize = 0x13;
/// Half widths (z) of the wide (0x20b7a0) and narrow (0x20b760) bars; both span y ±5.
pub const WIDE_HALF: f32 = f32::from_bits(0x3dcc_cccd);
pub const NARROW_HALF: f32 = f32::from_bits(0x3d4c_cccd);

fn dist(a: [f32; 4], b: [f32; 3]) -> f32 { ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt() }

/// Level05 0x30dc68 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 8 { return; }
    let (link, word) = (p::i32(&w.m(id).pvars, 0), p::i32(&w.m(id).pvars, 4));
    match w.m(id).state {
        0 => {
            let m = w.mm(id);
            m.state = 1;
            m.visible = 0;
            m.mode |= mode::HIDDEN;
            w.svc.set_shared_i32(word, -1);
        }
        1 => {
            let tick = w.counter as u32;
            if w.svc.units.word(TICK_WORD) != tick {
                w.svc.units.set_word(TICK_WORD, tick);
                if let Some(i) = super::row(REFERENCE_LEVEL, UPDATE_FN) { w.svc.draw_callbacks.register(Callback::UnitQuads(i), id); }
            }
            let cam = w.camera_point();
            let d = dist(w.m(id).position, cam);
            if d < RANGE {
                let slot = w.svc.shared_i32(word);
                let owner = w.sound_owner(slot).filter(|&o| w.table.mobys.get(o).is_some_and(|m| m.o_class == CLASSES[0]));
                match owner {
                    None => {
                        let s = w.play_sound(0, 4, id);
                        w.svc.set_shared_i32(word, s);
                    }
                    Some(o) if o == id => {}
                    Some(o) => {
                        let keep = w.m(o).state == 1 && dist(w.m(o).position, cam) <= d;
                        if !keep {
                            let pos = w.m(id).position;
                            w.hand_over_sound(slot, id, [pos[0], pos[1], pos[2]]);
                        }
                    }
                }
            }
            if link >= 0 && w.table.mobys.get(link as usize).is_some_and(|m| m.cmd != 0) {
                w.play_sound(1, 0, id);
                let m = w.mm(id);
                m.has_collision = false;
                m.state = 2;
            }
        }
        2 => {
            let m = w.mm(id);
            if m.has_collision { m.has_collision = false; }
        }
        _ => {}
    }
}

/// Level05 0x30de90 for the registering fence `id` (module doc): the bars of every lit fence of its group.
pub fn fx_quads(table: &MobyTable, svc: &Services, id: MobyId) -> Option<FxQuads> {
    let fence = table.mobys.get(id)?;
    let tick = svc.units.word(TICK_WORD);
    // 6.28318 as the code loads it (not τ).
    let mut ph = (tick & 0x3f) as f32 / 63.0 * f32::from_bits(0x40c9_0fd0);
    let mut colours = [(0u32, 0u32); 5];
    for c in &mut colours {
        let t = (ph.sin() + 1.0) * 0.5;
        *c = (crate::hud::tween_color(t, WIDE.0, WIDE.1), crate::hud::tween_color(t, NARROW.0, NARROW.1));
        ph = crate::moby_update::creature::add_rot(ph, f32::from_bits(0x3fa0_d97c));
    }
    let list = svc.groups.lists.get(fence.group as u8 as usize)?.as_ref()?;
    let st = [[0.5, 0.0], [0.5, 1.0], [0.5, 0.0], [0.5, 1.0]];
    let mut quads = Vec::new();
    for &e in list {
        let Some(m) = table.mobys.get((e & 0x7fff) as usize) else { continue };
        if m.o_class != fence.o_class || m.state != 1 { continue; }
        let r = m.rows;
        let world = |v: [f32; 3]| -> [f32; 3] { std::array::from_fn(|k| v[0] * r[0][k] + v[1] * r[1][k] + v[2] * r[2][k] + m.position[k]) };
        for (i, &(wide, narrow)) in colours.iter().enumerate() {
            let z = (i + 1) as f32;
            for (half, rgba) in [(WIDE_HALF, wide), (NARROW_HALF, narrow)] {
                let c = [[0.0, -5.0, z - half], [0.0, -5.0, z + half], [0.0, 5.0, z - half], [0.0, 5.0, z + half]].map(world);
                quads.push(FxQuad { corners: c, st, rgba: [rgba; 4] });
            }
        }
    }
    Some(FxQuads { fx: FX, additive: true, subtract: false, quads })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};
    use crate::moby_update::scheduler::Groups;
    use crate::moby_update::services::{SoundEvent, SoundSink};
    use crate::rng::Rng;

    /// One slot, as `ClassSoundSink` keeps it: owner and position.
    #[derive(Default)]
    struct Sink {
        owner: Option<MobyId>,
        pos: [f32; 3],
        plays: Vec<(i32, u32, MobyId)>,
    }
    impl SoundSink for Sink {
        fn play_class_sound(&mut self, ev: &SoundEvent, _rng: &mut Rng) -> i32 {
            self.plays.push((ev.index, ev.flags, ev.moby));
            if ev.flags & 4 != 0 { self.owner = Some(ev.moby); self.pos = ev.pos; }
            0
        }
        fn slot_owner(&self, slot: i32) -> Option<(MobyId, u16)> { (slot == 0).then_some(self.owner?).map(|o| (o, 0)) }
        fn hand_over(&mut self, slot: i32, moby: MobyId, pos: [f32; 3]) {
            assert_eq!(slot, 0);
            self.owner = Some(moby);
            self.pos = pos;
        }
    }

    fn fence(x: f32, link: i32) -> Moby {
        let mut m = Moby { o_class: 838, group: 0, has_collision: true, pvars: vec![0; 8], ..Moby::default() };
        m.position = [x, 0.0, 0.0, 1.0];
        m.rows = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0; 4]];
        p::set_i32(&mut m.pvars, 0, link);
        m
    }

    #[test]
    fn the_nearest_fence_takes_the_loop_and_the_switch_turns_it_off() {
        let switch = Moby { o_class: 1, ..Moby::default() };
        let mut t = MobyTable::new(vec![fence(0.0, 2), fence(10.0, 2), switch], 4);
        let mut hero = crate::hero::Hero::new();
        let mut rng = Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = Services::new();
        svc.pvar_shared = vec![0; 4];
        svc.groups = Groups { lists: vec![Some(vec![0, 0x8001])] };
        let mut sink = Sink::default();
        let run = |t: &mut MobyTable, hero: &crate::hero::Hero, rng: &mut Rng, svc: &mut Services, sink: &mut Sink, tick: u64| {
            let mut w = World::new(t, hero, rng, &classes, svc, tick);
            w.camera = hero.pos;
            w.sound = Some(sink);
            update(&mut w, 0);
            update(&mut w, 1);
        };
        // Init: slot word −1, hidden, state 1.
        run(&mut t, &hero, &mut rng, &mut svc, &mut sink, 1);
        assert_eq!((svc.shared_i32(0), t.mobys[0].state, t.mobys[0].mode & mode::HIDDEN), (-1, 1, mode::HIDDEN));
        // Camera at fence 0: fence 0 starts the loop (sound 0, flags 4); fence 1 is farther and keeps off.
        hero.pos = crate::hero::physics::v4(1.0, 0.0, 0.0);
        run(&mut t, &hero, &mut rng, &mut svc, &mut sink, 2);
        assert_eq!(sink.plays, vec![(0, 4, 0)]);
        assert_eq!((svc.shared_i32(0), sink.owner), (0, Some(0)));
        // One callback per tick, from the first fence.
        assert_eq!(svc.draw_callbacks.list1.len(), 1, "tick 2 (tick 1 arms)");
        // The camera moves to fence 1: fence 1 takes the slot over (owner and position), no new play.
        hero.pos = crate::hero::physics::v4(9.0, 0.0, 0.0);
        run(&mut t, &hero, &mut rng, &mut svc, &mut sink, 3);
        assert_eq!((sink.owner, sink.pos, sink.plays.len()), (Some(1), [10.0, 0.0, 0.0], 1));
        // The quads: 2 lit fences × 5 bars × 2 widths, additive FX 0x13, bar 1 at z 1.
        let q = fx_quads(&t, &svc, 0).unwrap();
        assert_eq!((q.quads.len(), q.fx, q.additive), (20, FX, true));
        assert_eq!(q.quads[0].corners[0], [0.0, -5.0, 1.0 - WIDE_HALF]);
        assert_eq!(q.quads[0].rgba[0] >> 24, 0x20);
        assert_eq!(q.quads[1].rgba[0] >> 24, 0x30);
        // The switch: power-down sound 1, collision off, state 2; its bars go out.
        t.mobys[2].cmd = 1;
        run(&mut t, &hero, &mut rng, &mut svc, &mut sink, 4);
        assert_eq!(&sink.plays[1..], &[(1, 0, 0), (1, 0, 1)]);
        assert!(t.mobys[..2].iter().all(|m| m.state == 2 && !m.has_collision));
        assert!(fx_quads(&t, &svc, 0).unwrap().quads.is_empty());
    }
}
