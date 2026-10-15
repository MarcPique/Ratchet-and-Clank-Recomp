//! **After-images** (the game's "speed blur"): translucent copies of a moby drawn where it was a few ticks ago, in its
//! current pose. The game has no screen-space motion blur (`DrawWorld` 0x21a1b8 draws no previous frame: its only
//! full-screen passes are the frame clear `append_gif_transfer_packet` 0x222ee8, the AA blit `PutAABlitPacket_A`
//! 0x223200 — a bilinear copy of the draw buffer, PRIM 0x16 without blending — the fogged sprite, the underwater tint
//! and the fades); what reads as a blur behind the fast moves is this mechanism. Level01, read from the decompiler C
//! and checked against the disassembly (docs/plan/hero_states.md "After-images").
//!
//! **The record** (0x140 bytes; Ratchet's at 0x1409c0, the thrown wrench's at 0x140b00): a ring of the owner's last 8
//! positions (+0x00) and Euler rotations (+0x80), up to 4 ghosts (+0x100 alpha, +0x110 ticks back, +0x120 the ghost
//! moby), +0x130 the ring head, +0x132 the history count (≤ 8), +0x134 the owner, +0x138 the ghost count, +0x13c
//! active.
//! * `0x277400(owner, rec)` [`Trail::start`]: when not active: owner, active, head / count / ghosts 0.
//! * `0x277428(rec, alpha, back)` [`Trail::add`]: with fewer than 4 ghosts and `back < 9`, `CreateMoby(owner's class)`
//!   with the owner's draw distance +0x32 and light word +0x38, +0x31 = 1, mode +0x34 = 0x80a (bit 8: MobyProc's fading
//!   group, drawn blended), +0x30 = 0, +0x94 = 0, class 0 (Ratchet): LOD distance +0x72 = 0; its alpha +0x23 = `alpha`.
//! * `0x277508(rec, fade)` [`Trail::update`] (every tick of its user): the owner gone → `0x277740`; else the owner's
//!   +0x10 / +0x40 into the ring at the head, head + 1 (mod 8), count + 1 (≤ 8); every ghost whose `back` ≤ count takes
//!   the owner's anim keys (+0x50..+0x55: frames, sequences, blend t; `update_moby_animation_state`) and the ring entry
//!   `head − back` (`back` = 1 is the entry just written), then its matrix; with `fade` ≠ 0 every ghost's alpha drops
//!   by `fade` (not below 0) and the trail ends (`0x277740`) once all are 0.
//! * `0x277740(rec)` [`Trail::kill`]: the ghosts deleted, not active.
//!
//! **Users** (every call site in level01): Ratchet's record 0x1409c0 — the Thruster-Pack long jump 0x10 (SetState
//! 0x23cf98: ghosts 0x28 / 0x14 / 0x0a at 2 / 4 / 6 ticks back) and high jump 0xd (0x28 / 0x14 at 3 / 5), faded by 2 a
//! tick after tick 12 (`HeroStatePhysics` 0x2370b8), and the gadget lunge 0x20 (the physics: 0x30 / 0x17 / 0x0c at
//! 2 / 4 / 6 from tick 8, faded by 5 after tick 18: `crate::hero::walloper`); every SetState of the on-foot
//! body (0x1413f4 = 0), `HeroTeleport` 0x2368e0 and the body switch 0x231348 end it (the port: SetState only). The
//! thrown wrench's record
//! 0x140b00 — the Comet-Strike's throw `0x236da0` (0x30 / 0x17 at 3 / 5 ticks back), updated without fade by the flight
//! `0x2be1c0` and ended at the catch.
//!
//! **Native.** The ghosts are plain values here (placement, pose keys, alpha), drawn by the engine as extra instances
//! of the owner's class (`rc-engine` afterimage_render), not mobys of the table: they take no moby slot (the game's
//! can fail to be created when the table is full) and are not drawn before their first placement (the game's sit at
//! their creation point until then). Standard `f32`.

/// Ring entries (`+0x130` wraps mod 8) and ghosts per record.
pub const RING: usize = 8;
pub const MAX_GHOSTS: usize = 4;

/// The owner's anim keys a ghost copies (moby +0x50..+0x55).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pose {
    pub seq_a: u8,
    pub seq_b: u8,
    pub frame_a: u8,
    pub frame_b: u8,
    pub t: f32,
}

/// A placement: the owner's rotation rows (from its Euler +0x40) and position +0x10.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Place {
    pub rows: [[f32; 4]; 3],
    pub position: [f32; 4],
}

/// One ghost: alpha +0x23, ticks back, and where / how it is drawn once placed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ghost {
    pub alpha: u8,
    pub back: i32,
    pub drawn: Option<(Place, Pose)>,
}

/// One after-image record (module doc).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Trail {
    pub active: bool,
    /// The owner's class (`CreateMoby(owner+0xa6)`).
    pub owner_class: i16,
    ring: [Place; RING],
    head: usize,
    count: i32,
    pub ghosts: Vec<Ghost>,
}

impl Trail {
    /// `0x277400(owner, rec)`.
    pub fn start(&mut self, owner_class: i16) {
        if self.active { return; }
        *self = Trail { active: true, owner_class, ..Trail::default() };
    }

    /// `0x277428(rec, alpha, back)`.
    pub fn add(&mut self, alpha: u8, back: i32) {
        if self.ghosts.len() != MAX_GHOSTS && back < 9 { self.ghosts.push(Ghost { alpha, back, drawn: None }); }
    }

    /// `0x277508(rec, fade)`: `owner` = the owner's placement and anim keys now, None when it is gone.
    pub fn update(&mut self, owner: Option<(Place, Pose)>, fade: u8) {
        if !self.active { return; }
        let Some((place, pose)) = owner else {
            self.kill();
            return;
        };
        self.ring[self.head] = place;
        self.head = (self.head + 1) % RING;
        self.count = (self.count + 1).min(RING as i32);
        for g in &mut self.ghosts {
            if self.count < g.back { continue; }
            let k = (self.head as i32 - g.back).rem_euclid(RING as i32) as usize;
            g.drawn = Some((self.ring[k], pose));
        }
        if fade == 0 { return; }
        let mut all_out = true;
        for g in &mut self.ghosts {
            g.alpha -= g.alpha.min(fade);
            if g.alpha != 0 { all_out = false; }
        }
        if all_out { self.kill(); }
    }

    /// `0x277740(rec)`.
    pub fn kill(&mut self) {
        if !self.active { return; }
        self.ghosts.clear();
        self.active = false;
    }

    /// The ghosts to draw: placed and not fully faded.
    pub fn shown(&self) -> impl Iterator<Item = (&Ghost, Place, Pose)> {
        self.ghosts.iter().filter(|g| self.active && g.alpha != 0).filter_map(|g| g.drawn.map(|(p, q)| (g, p, q)))
    }
}

/// The two records of the hero block: Ratchet's (0x1409c0) and the thrown wrench's (0x140b00).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Trails {
    pub hero: Trail,
    pub wrench: Trail,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32) -> Place { Place { position: [x, 0.0, 0.0, 1.0], ..Default::default() } }

    /// The Thruster long jump's trail: ghosts 2 / 4 / 6 ticks back, placed once the history is that long, on the ring
    /// entry `back − 1` ticks before the newest, in the owner's current pose; the fade of 2 empties the brightest
    /// (0x28) after 20 ticks and ends the trail.
    #[test]
    fn thruster_trail_places_and_fades() {
        let mut t = Trail::default();
        t.start(0);
        for (a, b) in [(0x28, 2), (0x14, 4), (0x0a, 6)] { t.add(a, b); }
        t.add(1, 9);
        assert_eq!(t.ghosts.len(), 3, "back 9 is refused");
        let pose = |k: u8| Pose { seq_b: k, ..Default::default() };
        for k in 0..3 { t.update(Some((at(k as f32), pose(k))), 0); }
        let placed: Vec<_> = t.ghosts.iter().map(|g| g.drawn.map(|(p, q)| (p.position[0], q.seq_b))).collect();
        assert_eq!(placed, vec![Some((1.0, 2)), None, None]);
        for k in 3..6 { t.update(Some((at(k as f32), pose(k))), 0); }
        let placed: Vec<_> = t.ghosts.iter().map(|g| g.drawn.map(|(p, _)| p.position[0])).collect();
        assert_eq!(placed, vec![Some(4.0), Some(2.0), Some(0.0)]);
        // Eight entries wrap: back 6 at tick 20 is tick 15's.
        for k in 6..21 { t.update(Some((at(k as f32), pose(0))), 0); }
        assert_eq!(t.ghosts[2].drawn.unwrap().0.position[0], 15.0);
        let mut n = 0;
        while t.active {
            t.update(Some((at(0.0), pose(0))), 2);
            n += 1;
        }
        assert_eq!(n, 20);
        assert!(t.ghosts.is_empty());
    }

    /// `0x277400` on an active record keeps it; `0x277740` ends it; the owner gone ends it.
    #[test]
    fn start_is_once_and_kill_ends() {
        let mut t = Trail::default();
        t.start(0);
        t.add(0x30, 3);
        t.start(71);
        assert_eq!((t.owner_class, t.ghosts.len()), (0, 1));
        t.update(None, 0);
        assert!(!t.active && t.ghosts.is_empty());
        t.start(71);
        t.add(0x30, 3);
        t.kill();
        assert!(!t.active && t.shown().count() == 0);
    }
}
