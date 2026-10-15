//! The animation-driven sounds of every moby: the sequence sound triggers and the sequence loop sound, played by the
//! engine's one `MobyAnimAdvance` (level01 0x265260) for every class on every level. The class code does not play
//! these: the data does (each sequence header's loop sound +0x11 and its trigger words; docs/plan/audio.md
//! "Sound paths"). Novalis examples: the troopers' jetpack loop (459 seq 8), their reload / hit / step triggers,
//! the Blarg flyers' and the gunship's engine loops (660 / 688 seq 0), the amoeboids' loop, the critters' steps.
//!
//! * **Trigger** ([`rc_formats::moby_anim::advance_trigger`]): the first trigger word whose time the key time passed
//!   this tick plays `PlayClassSound(sound, 0, moby)`; the advance then returns (no loop refresh that tick).
//! * **Loop sound** (`FUN_002637d8`, called from the advance's tail): moby +0x7c = class sound of the sequence's loop
//!   (0xff none), +0x7d = its voice slot (0xff none). A moby with neither skips it; a moby without a slot is only
//!   refreshed on its phase, `(moby address >> 8 & 3) == (0x15f5cc & 3)`, i.e. every 4th tick (the moby array is at
//!   0x1e9a480 on Novalis, so the phase is `index & 3`). The refresh plays the loop with flags 4 when there is no
//!   slot (a refused play — out of range, no free slot — leaves 0xff: retried on the next phase tick); with a slot it
//!   drops the slot when another owner took it (the voice was freed: its owner +0x18 is zeroed) and releases it when
//!   the slot plays another class sound of this moby (the sequence changed its loop).
//! * A tick whose advance landed a blend's transition does neither (the game sets its +0x7c word to 0xffff for the
//!   rest of the call). A zero rate or speed still refreshes the loop (the flyers' one-frame sequence 0 has speed 0).
//! * Sequence changes set +0x7c: [`after_sequence_change`] (the blend and the hard cut); [`init`] is the part of
//!   `InitMobyInstance` 0x263488 that does (`update_moby_animation_state` on sequence 0).
//!
//! The draws (a play's pitch bend, `SoundSlotAlloc` 0x2a13a0) land in the moby loop at the moby's advance, before its
//! update, as in the game.

use crate::moby_runtime::{Moby, MobyId};
use crate::moby_update::services::World;
use rc_formats::moby_anim::{self, MobyAnimClass};

/// Moby +0x7c / +0x7d: no loop sound, no slot.
pub const NONE: u8 = 0xff;

/// The +0x7c / +0x7e part of `InitMobyInstance` (0x263488): `update_moby_animation_state` on sequence 0 when the
/// class has sequences (moby +0x7d stays 0xff).
pub fn init(m: &mut Moby, class: Option<&MobyAnimClass>) {
    if let Some(lp) = class.and_then(|c| init_state(&mut m.anim, c)) { m.b7c = lp; }
}

/// The animation part of [`init`] on any spawned animation state (the table mobys, and the hand item that
/// `HeroItemsCreate` makes with `CreateMoby`, which the port keeps in the hero block): +0x7e = sequence 0's trigger
/// count, so the item's sequence 0 (its take-out) plays its trigger sounds. Returns sequence 0's loop sound (+0x7c).
pub fn init_state(anim: &mut moby_anim::AnimState, class: &MobyAnimClass) -> Option<u8> {
    let q = class.sequence(0)?;
    anim.trigger_count = q.header.trigger_count;
    Some(q.header.loop_sound)
}

/// +0x7c after a sequence change (`hard_cut` 0x26c5a8 / `MobyAnimBlend` 0x26c660, both through
/// `update_moby_animation_state`; `rc_formats::moby_anim` sets +0x7e).
pub fn after_sequence_change(m: &mut Moby, class: &MobyAnimClass) { m.b7c = moby_anim::loop_sound_of(class, m.anim.seq_b); }

/// `MobyAnimAdvance(moby)` (0x265260) with its sounds (module doc). The caller has checked mode 0x40.
pub fn advance(w: &mut World, id: MobyId) {
    let classes = w.classes;
    let Some(class) = classes.anim(w.m(id).o_class) else { return };
    let before = w.m(id).anim;
    moby_anim::advance(&mut w.table.mobys[id].anim, class);
    let after = w.m(id).anim;
    if moby_anim::advance_landed(&before, &after) { return; }
    if let Some(sound) = moby_anim::advance_trigger(&before, &after, class) {
        w.play_sound(sound as i16 as i32, 0, id);
        return;
    }
    let (lp, slot) = (w.m(id).b7c, w.m(id).b7d);
    if (lp, slot) == (NONE, NONE) { return; }
    if slot != NONE || (id as u64 & 3) == (w.counter & 3) { refresh_loop(w, id); }
}

/// `FUN_002637d8(moby)`: the loop-sound voice refresh (module doc).
pub fn refresh_loop(w: &mut World, id: MobyId) {
    let (lp, slot) = (w.m(id).b7c, w.m(id).b7d);
    if slot == NONE {
        // PlayClassSound((char)+0x7c, 4, moby); a negative index is refused (no class has one).
        if lp != NONE {
            let k = w.play_sound(lp as i8 as i32, 4, id);
            w.mm(id).b7d = if k < 0 { NONE } else { k as u8 };
        }
        return;
    }
    match w.sound.as_deref().and_then(|s| s.slot_owner(slot as i32)) {
        Some((owner, index)) if owner == id => {
            if index != lp as u16 {
                w.release_sound(slot as i32, id);
                w.mm(id).b7d = NONE;
            }
        }
        _ => w.mm(id).b7d = NONE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rc_formats::moby_anim::{AnimState, MobyFrame, MobyFrameHeader, MobySequence, MobySequenceHeader};

    fn key() -> MobyFrame {
        MobyFrame { header: MobyFrameHeader { rate: 0.25, ..Default::default() }, quats: vec![], scales: vec![], trans: vec![], payload: vec![] }
    }

    fn class(seqs: Vec<(u8, u8, Vec<u32>)>) -> MobyAnimClass {
        let sequences = seqs
            .into_iter()
            .map(|(fc, lp, triggers)| {
                let header = MobySequenceHeader { frame_count: fc, loop_sound: lp, trigger_count: triggers.len() as u8, ..Default::default() };
                Some(MobySequence { header, frames: vec![key(); fc as usize], triggers })
            })
            .collect();
        MobyAnimClass { joint_count: 0, skeleton: vec![], rest: vec![], parent_word: vec![], sequences }
    }

    fn st(seq_a: u8, seq_b: u8, frame_a: u8, t: f32, tc: u8) -> AnimState {
        AnimState { seq_a, frame_a, seq_b, frame_b: frame_a + 1, t, speed: 1.0, rate: 0.25, flags: 0, trigger_count: tc, skip_advance: false }
    }

    #[test]
    fn trigger_window_and_transitions() {
        // Trooper-like: class sound 7 at time 128 (frame 8), 2 at 24 (frame 1.5).
        let c = class(vec![(15, 0xff, vec![128 << 16 | 7, 24 << 16 | 2])]);
        assert_eq!(moby_anim::advance_trigger(&st(0, 0, 7, 0.75, 2), &st(0, 0, 8, 0.0, 2), &c), Some(7));
        assert_eq!(moby_anim::advance_trigger(&st(0, 0, 8, 0.0, 2), &st(0, 0, 8, 0.25, 2), &c), None, "exclusive below");
        assert_eq!(moby_anim::advance_trigger(&st(0, 0, 1, 0.25, 2), &st(0, 0, 1, 0.5, 2), &c), Some(2));
        // No trigger count (a snapshot blend left 0), a running blend, a wrap back: nothing.
        assert_eq!(moby_anim::advance_trigger(&st(0, 0, 7, 0.75, 0), &st(0, 0, 8, 0.0, 0), &c), None);
        assert_eq!(moby_anim::advance_trigger(&st(0xff, 0, 7, 0.75, 2), &st(0, 0, 8, 0.0, 2), &c), None);
        assert_eq!(moby_anim::advance_trigger(&st(0, 0, 14, 0.75, 2), &st(0, 0, 0, 0.0, 2), &c), None);
        assert!(moby_anim::advance_landed(&st(0xff, 0, 0, 0.9, 0), &st(0, 0, 0, 0.1, 2)));
    }

    #[test]
    fn init_and_sequence_change_set_the_loop_byte() {
        let c = class(vec![(1, 0, vec![]), (8, 6, vec![1 << 16 | 3])]);
        let mut m = Moby::zeroed();
        m.b7c = NONE;
        init(&mut m, Some(&c));
        assert_eq!((m.b7c, m.anim.trigger_count), (0, 0));
        m.anim.t = 0.0;
        let mut snap = None;
        assert!(moby_anim::set_sequence(&mut m.anim, &c, 1, 0, 8, &mut snap));
        after_sequence_change(&mut m, &c);
        // t ≤ 0.025: key A stays sequence 0 (no triggers), +0x7c = key B's loop.
        assert_eq!((m.b7c, m.anim.trigger_count), (6, 0));
        assert!(moby_anim::hard_cut(&mut m.anim, &c, 1, 0));
        after_sequence_change(&mut m, &c);
        assert_eq!((m.b7c, m.anim.trigger_count), (6, 1));
    }
}
