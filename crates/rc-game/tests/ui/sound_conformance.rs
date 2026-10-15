//! Sound conformance on every level (docs/plan/audio.md "Sound paths"): every sound the level's data asks the general
//! sound paths to play resolves in the port and reaches the 989snd player with a valid bank sound. Skipped when
//! `extracted/` is absent.
//!
//! * **Animation triggers and loop sounds** of every sequence of every class the level loads (the level's moby classes,
//!   Ratchet's `ratchet_seq` sequences, the gadget classes): `PlayClassSound(sound, 0 | 4, moby)` through
//!   `AudioSystem::play_class_sound`, one EE frame and one IOP frame, then the play reply must carry a live 989snd
//!   handle. A loop whose def is not flagged looped (and a trigger on a looped def) is refused by the game's
//!   `SoundSlotAlloc` (loop flag ≠ def loop), as is a def whose volumes are all below 0x20: those are listed and must
//!   be refused by the port too.
//! * **Footsteps**: every `(class, foot, variant)` of `PlayFootstepSound` on the level (`class_sounds::footstep_def`);
//!   a def that is looped or past the level's defs is refused by the game (listed; the port must refuse it too).
//! * **The code constants** the footsteps read: `g_footstep_level_base` and `0x15f574` in every level overlay.

use rc_formats::moby_anim::{parse_sequence, parse_sequences, MobyAnimClass, MobySequence};
use rc_formats::sound_bank as sb;
use rc_game::audio::class_sounds::{footstep_def, FOOTSTEP_LEVEL_BASE, MOBY_LEVEL_DEFS};
use rc_game::audio::voices::{state, Listener, SoundSlots};
use rc_game::audio::{AudioSystem, FrameInput, LevelAudio};
use rc_game::moby_update::services::SoundEvent;
use rc_game::rng::Rng;

/// A sound the data references: `(o_class, sequence, class sound, flags)`.
type Ref = (i32, usize, i32, u32);

fn classes(level: u32, c: &rc_formats::test_data::Core) -> Vec<(i32, MobyAnimClass)> {
    let mut out = Vec::new();
    for e in &c.core.moby_classes {
        let Some(blob) = c.block(&format!("moby_class/{:04}", e.o_class)) else { continue };
        let Ok(mc) = rc_formats::moby::parse_moby_class(blob) else { continue };
        let seqs = if e.o_class == 0 {
            (0..256).map(|i| c.block(&format!("ratchet_seq/{i:03}")).and_then(|b| parse_sequence(b, 0).ok())).collect::<Vec<Option<MobySequence>>>()
        } else {
            parse_sequences(blob, &mc).unwrap_or_else(|e| panic!("level {level} class {}: {e}", mc.header.joint_count))
        };
        out.push((e.o_class, MobyAnimClass::new(&mc, seqs)));
    }
    for g in rc_formats::gadget::parse_gadget_classes(&c.core, &c.data).unwrap() {
        out.push((g.moby.o_class, MobyAnimClass::new(&g.moby.class, parse_sequences(&g.blob, &g.moby.class).unwrap_or_default())));
    }
    out
}

/// Every trigger and loop sound of the classes' sequences. A single-sequence class with one frame and loop bit 7
/// is never advanced (`InitMobyInstance` sets mode 0x40), so its byte is not a sound.
fn references(classes: &[(i32, MobyAnimClass)]) -> Vec<Ref> {
    let mut out = Vec::new();
    for (o, a) in classes {
        let still = a.sequences.len() == 1 && a.sequence(0).is_some_and(|q| q.header.frame_count <= 1 && q.header.loop_sound & 0x80 != 0);
        for (si, q) in a.sequences.iter().enumerate() {
            let Some(q) = q else { continue };
            for &w in q.triggers.iter().take(q.header.trigger_count as usize) { out.push((*o, si, (w & 0xffff) as i16 as i32, 0)); }
            if q.header.loop_sound != 0xff && !still { out.push((*o, si, q.header.loop_sound as i8 as i32, 4)); }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn level_audio(level: u32) -> Option<LevelAudio> {
    let root = rc_formats::test_data::level_dir(level);
    let bank = std::fs::read(root.join("sound_bank.bin")).ok()?;
    let c = rc_formats::test_data::core(level)?;
    let gp = rc_formats::test_data::gameplay(level)?;
    let header = std::fs::read(root.join("level_header.bin")).ok()?;
    Some(LevelAudio::from_parts(&bank, &c.index, &c.core, &c.data, &gp, &header, &Default::default(), None).unwrap())
}

/// Plays `ev` (or the footstep `fs`) on a fresh slot table at the listener, runs the EE frame, the IOP frame and the
/// next EE frame; returns the slot (−1: refused) and whether the play reply carried a live 989snd handle (a zero
/// handle frees the slot when the reply is applied; the next send then waits for a new reply, so the slot's state is
/// the evidence).
fn play(sys: &mut AudioSystem, ev: Option<&SoundEvent>, fs: Option<(u8, u8, u8, i32)>) -> (i32, bool) {
    sys.movie_stop();
    sys.slots = SoundSlots::new();
    let pos = [100.0f32, 100.0, 50.0];
    let listener = Listener { pos: [pos[0], pos[1], pos[2] + 1.0], rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], ..Default::default() };
    let mut rng = Rng::new();
    let k = match (ev, fs) {
        (Some(ev), _) => sys.play_class_sound(ev, None, &listener, &mut rng),
        (_, Some((class, foot, variant, level))) => sys.play_footstep(level, class, foot, variant, 0, 1, pos, &listener, &mut rng, 0),
        _ => unreachable!(),
    };
    if k < 0 { return (k, false); }
    let input = FrameInput { listener, hero_pos: pos };
    let mut out = Vec::new();
    sys.tick_with(&input, 1, &mut rng, &|_| Some(pos), &mut out);
    sys.game_frame_with(&input, 2, &mut rng, &|_| Some(pos));
    (k, sys.slots.slots[k as usize].state != state::FREE)
}

#[test]
fn every_animation_sound_and_footstep_resolves_and_plays() {
    let mut seen = 0;
    let mut totals = (0usize, 0usize, 0usize, 0usize);
    for level in 0..19u32 {
        let (Some(audio), Some(c)) = (level_audio(level), rc_formats::test_data::core(level)) else { continue };
        let n_bank = audio.bank.sounds.len();
        let cls = classes(level, &c);
        let refs = references(&cls);
        let mut sys = AudioSystem::new(audio.clone());
        let (mut ok, mut refused, mut unknown) = (Vec::new(), Vec::new(), Vec::new());
        for &(o, seq, index, flags) in &refs {
            let def = usize::try_from(index).ok().and_then(|i| audio.sounds.def(sb::SoundOwner::Class(o), i)).copied();
            let Some(def) = def.filter(|d| (d.index as usize) < n_bank) else {
                unknown.push(format!("class {o} seq {seq} sound {index} flags {flags}"));
                continue;
            };
            let ev = SoundEvent { index, flags, moby: 1, o_class: o as i16, sound_class: o as i16, pos: [100.0, 100.0, 50.0], tick: 0 };
            let (k, handle) = play(&mut sys, Some(&ev), None);
            // SoundSlotAlloc refuses a loop flag that differs from the def's, and a start volume below 0x20 (a def
            // whose near and far volumes are both below it is never heard): the game never plays these.
            if (flags & 4 != 0) != (def.looped != 0) || def.vol_near.max(def.vol_far) < 0x20 {
                assert_eq!(k, -1, "level {level}: class {o} seq {seq} sound {index}: the port played a play the game refuses");
                refused.push(format!("class {o} seq {seq} sound {index} flags {flags} (looped {}, volume {}..{})", def.looped, def.vol_far, def.vol_near));
                continue;
            }
            assert!(k >= 0, "level {level}: class {o} seq {seq} sound {index} flags {flags}: no slot at the listener ({def:?})");
            assert!(handle, "level {level}: class {o} seq {seq} sound {index} (bank {}): no 989snd handle", def.index);
            ok.push((o, index));
        }
        assert!(unknown.is_empty(), "level {level}: unresolved sound references {unknown:?}");
        // Footsteps: every class / foot / variant. A def past the level's count (0x15f5f0) or a looped def (the play
        // has flags 0) is refused by the game as by the port: the level has no such footstep (e.g. no water steps).
        let (mut steps, mut no_step) = (0, Vec::new());
        for class in 0..4u8 {
            for foot in 0..2u8 {
                for variant in 0..2u8 {
                    let idx = footstep_def(level as i32, class, foot, variant).unwrap();
                    let (k, handle) = play(&mut sys, None, Some((class, foot, variant, level as i32)));
                    match audio.sounds.level_defs.get(idx) {
                        Some(d) if d.looped == 0 => {
                            assert!((d.index as usize) < n_bank, "level {level}: footstep def {idx} bank id {}", d.index);
                            assert!(k >= 0 && handle, "level {level}: footstep {class}/{foot}/{variant} (def {idx} {d:?}): slot {k}, live {handle}");
                            steps += 1;
                        }
                        _ => {
                            assert_eq!(k, -1, "level {level}: footstep {class}/{foot}/{variant} (def {idx}) played, the game refuses it");
                            no_step.push((class, foot, variant));
                        }
                    }
                }
            }
        }
        let n_classes = ok.iter().map(|r| r.0).collect::<std::collections::BTreeSet<_>>().len();
        let mut inst = std::collections::BTreeMap::new();
        for i in &audio.instances { *inst.entry(i.o_class).or_insert(0) += 1; }
        eprintln!("level {level:2}: sound instances by class {inst:?}");
        eprintln!("level {level:2}: {} references ({} classes played), {} refused as in the game {refused:?}, {steps} footsteps (refused {no_step:?})", refs.len(), n_classes, refused.len());
        totals = (totals.0 + refs.len(), totals.1 + ok.len(), totals.2 + refused.len(), totals.3 + steps);
        seen += 1;
    }
    eprintln!("{seen} levels: {} animation sound references, {} played, {} refused as in the game, {} footsteps", totals.0, totals.1, totals.2, totals.3);
}

/// The footsteps' code constants: the 19-byte level table and `0x15f574` (= 2) are the same in every level overlay.
#[test]
fn footstep_constants_in_every_overlay() {
    let mut seen = 0;
    for level in 0..19u32 {
        let Ok(raw) = std::fs::read(rc_formats::test_data::level_dir(level).join("overlay.bin")) else { continue };
        let secs = rc_formats::font::parse_overlay_sections(&raw).unwrap();
        let found = secs.iter().filter(|s| s.kind != 8).any(|s| s.data.windows(FOOTSTEP_LEVEL_BASE.len()).any(|w| w == FOOTSTEP_LEVEL_BASE));
        assert!(found, "level {level}: footstep table not in the overlay");
        let w = rc_formats::font::read_overlay(&secs, 0x15f574, 4).expect("0x15f574");
        assert_eq!(u32::from_le_bytes(w.try_into().unwrap()) as usize, MOBY_LEVEL_DEFS, "level {level}: 0x15f574");
        seen += 1;
    }
    eprintln!("{seen} overlays checked");
}
