//! Particle type 62, the nanotech orb and its trail puffs (`moby_update::classes::pickup`'s nanotech cluster, class
//! 806), read from the level01 code:
//!
//! * spawner [`spawn`] = `PartType62Spawn(pos, life, kind)` 0x288bb0: kind-0 sprite (byte1 0), additive (0x48),
//!   byte9 `trunc(4.0) + 0x40`, RGBA 0x7f7f4040, size 30000, rotation `randi(255)` (one draw), frame
//!   `def[62][kind]` (`kind > 1` reads frame 0), life `life`, the alpha step `trunc(127 / life)` at +0x20 and the
//!   kind at +0x22. The caller then sets the colour / size it wants (the cluster's 0x7f7f4040, 60000 or 10000);
//! * [`update`] = 0x288ca8: kind 1 (the orbs themselves) does nothing (the cluster moves and kills them); kind 2
//!   follows its anchor moby (+0x24) at the kept offset +0x30; kinds 0 and 2 lose the step of alpha a tick and die
//!   when it is gone.
//!
//! **Anchors.** The record's +0x24 holds a moby pointer in the game; here the moby index + 1, and the moby's
//! position is read from [`Particles::anchors`], which the owning class writes during the moby loop (the cluster
//! writes its own and its crate's position). A record whose anchor is not listed keeps its position.

use super::{rec, Particles, Record};
use crate::rng::Rng;

pub const TYPE: u8 = 62;

/// `PartType62Spawn(pos, life, kind)` 0x288bb0. One `randi(255)` draw (the rotation). None when the pool is full.
pub fn spawn(sys: &mut Particles, rng: &mut Rng, pos: [f32; 4], life: i32, kind: i16) -> Option<usize> {
    let i = sys.create_part(TYPE)?;
    let frame = frame(sys, kind);
    let r = &mut sys.pool.recs[i];
    rec::set_v4(r, 0x10, pos);
    rec::set_u32(r, 4, 0x7f7f_4040);
    r[1] = 0;
    r[3] = 0x48;
    rec::set_ff(r, 0xc, 30000.0);
    r[9] = 4 + 0x40;
    r[8] = rng.randi(0xff) as u8;
    rec::set_i16(r, 0x22, kind);
    r[2] = frame;
    let step = if life != 0 { (127.0 / life as f32) as i32 } else { 0 };
    rec::set_i16(r, 0x20, step as i16);
    rec::set_i16(r, 0xa, life as i16);
    Some(i)
}

/// `*(def[62] + kind)`, kind > 1 → 0.
fn frame(sys: &Particles, kind: i16) -> u8 {
    let k = if kind > 1 { 0 } else { kind.max(0) as usize };
    sys.defs.as_ref().and_then(|d| d.start(TYPE as usize).and_then(|s| d.blob.get(s + k).copied())).unwrap_or(0)
}

/// Attaches record `i` to moby `anchor` (kind 2): +0x24 = the moby, +0x30 = the offset from `anchor_pos`.
pub fn attach(r: &mut Record, anchor: usize, anchor_pos: [f32; 3]) {
    rec::set_u32(r, 0x24, anchor as u32 + 1);
    let p = rec::pos(r);
    rec::set_v3(r, 0x30, [p[0] - anchor_pos[0], p[1] - anchor_pos[1], p[2] - anchor_pos[2]]);
}

/// Update 0x288ca8 (no RNG).
pub fn update(sys: &mut Particles, i: usize, _rng: &mut Rng) {
    let kind = rec::i16(&sys.pool.recs[i], 0x22);
    if kind == 1 { return; }
    if kind == 2 {
        let a = rec::u32(&sys.pool.recs[i], 0x24);
        if let Some(p) = a.checked_sub(1).and_then(|m| sys.anchors.get(&(m as usize)).copied()) {
            let r = &mut sys.pool.recs[i];
            let o = rec::v3(r, 0x30);
            rec::set_v3(r, 0x10, [p[0] + o[0], p[1] + o[1], p[2] + o[2]]);
        }
    }
    let r = &mut sys.pool.recs[i];
    let c = rec::u32(r, 4);
    let a = ((c as i32) >> 24) - rec::i16(r, 0x20) as i32;
    if a < 1 {
        sys.kill_part(i);
    } else {
        rec::set_u32(r, 4, (c & 0xff_ffff) | ((a as u32) << 24));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A trail puff (kind 0, 20 ticks): alpha 0x7f loses trunc(127/20) = 6 a tick and dies on the 22nd update; an
    /// orb (kind 1) never changes; a kind-2 puff follows its anchor.
    #[test]
    fn puffs_fade_orbs_stay_anchored_follow() {
        let mut s = Particles::new(None, Vec::new());
        let mut rng = Rng::new();
        let p = spawn(&mut s, &mut rng, [10.0, 10.0, 10.0, 0.0], 20, 0).unwrap();
        let o = spawn(&mut s, &mut rng, [10.0, 10.0, 10.0, 0.0], 0, 1).unwrap();
        let q = spawn(&mut s, &mut rng, [11.0, 10.0, 10.0, 0.0], 40, 2).unwrap();
        attach(&mut s.pool.recs[q], 7, [10.0, 10.0, 10.0]);
        s.anchors.insert(7, [20.0, 10.0, 10.0]);
        let mut died = None;
        for t in 1..=30 {
            s.update_parts(&mut rng);
            if died.is_none() && s.pool.recs[p][1] & 0x80 != 0 { died = Some(t); }
        }
        assert_eq!(died, Some(22));
        assert_eq!(s.pool.recs[o][1] & 0x80, 0, "the orb lives");
        assert_eq!(rec::u32(&s.pool.recs[o], 4), 0x7f7f_4040);
        assert_eq!(rec::pos(&s.pool.recs[q]), [21.0, 10.0, 10.0]);
    }
}
