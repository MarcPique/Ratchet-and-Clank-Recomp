//! Reverb zones and level sounds on every level (docs/plan/audio.md "Reverb zones and level sounds"). Skipped when
//! `extracted/` is absent.
//!
//! * **Reverb boxes** (sound instance class 3): every one parses (a libsd type the effect knows, a depth in range, a box
//!   whose inverse rows undo its matrix) and behaves as `SndInstReverbBoxUpdate` 0x319f18: walked along its x axis the
//!   depth ramps up from the −x face, the full depth stays after leaving through +x, and leaving through −x switches the
//!   reverb off.
//! * **The effect in the mix**: with the hero in a box, a level-bank sound gets a wet signal (the output differs from the
//!   same run with the effect disabled); away from every zone the output is identical; two runs are identical.
//! * **Env sample points**: a teleport onto an enabled point sends its reverb; a disabled point sends nothing.
//! * **Level sounds** (`PlayLevelSoundAtMoby` 0x2a1770): defs 0 and 1 play on every level (a slot and a live 989snd
//!   handle), draw nothing from the stream, and indices ≥ 0x15f574 are refused. Every call site in the 19 overlays passes
//!   `(0, 1, 0)` (the help box) or `(1, 0, 0)` (a skill point).

use rc_formats::sound_bank as sb;
use rc_game::audio::class_sounds::{level_sound, MOBY_LEVEL_DEFS};
use rc_game::audio::reverb::{self, mode, ReverbRequest};
use rc_game::audio::voices::{state, Listener, SndCommand};
use rc_game::audio::{AudioSystem, FrameInput, LevelAudio};
use rc_game::rng::Rng;

fn level_audio(level: u32) -> Option<LevelAudio> {
    let root = rc_formats::test_data::level_dir(level);
    let bank = std::fs::read(root.join("sound_bank.bin")).ok()?;
    let c = rc_formats::test_data::core(level)?;
    let gp = rc_formats::test_data::gameplay(level)?;
    let header = std::fs::read(root.join("level_header.bin")).ok()?;
    Some(LevelAudio::from_parts(&bank, &c.index, &c.core, &c.data, &gp, &header, &Default::default(), None).unwrap())
}

/// Box-local → world through the instance's forward matrix (rows 0..2 scaled axes, row 3 the position).
fn world_of(s: &sb::SoundInstance, l: [f32; 3]) -> [f32; 3] {
    let m = &s.matrix;
    std::array::from_fn(|k| m[3][k] + l[0] * m[0][k] + l[1] * m[1][k] + l[2] * m[2][k])
}

fn listener_at(p: [f32; 3]) -> Listener { Listener { pos: p, rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], ..Default::default() } }

#[test]
fn reverb_boxes_on_every_level() {
    let (mut levels, mut boxes, mut kinds) = (0, 0, std::collections::BTreeMap::new());
    for level in 0..19u32 {
        let Some(la) = level_audio(level) else { continue };
        levels += 1;
        let class3: Vec<usize> = la.instances.iter().enumerate().filter(|(_, s)| s.o_class == 3).map(|(i, _)| i).collect();
        let sys = AudioSystem::new(la.clone());
        assert_eq!(sys.reverb_boxes.iter().map(|b| b.instance).collect::<Vec<_>>(), class3, "level {level}: every class-3 instance is a box");
        assert!(sys.emitters.unported.is_empty(), "level {level}: unported sound instances {:?}", sys.emitters.unported);
        for b0 in &sys.reverb_boxes {
            let s = &la.instances[b0.instance];
            let what = format!("level {level} box {}", b0.instance);
            assert!(reverb::preset(b0.kind).is_some() || b0.kind == mode::ECHO || b0.kind == mode::DELAY, "{what}: type {}", b0.kind);
            assert!((1..=0x7fff).contains(&b0.depth), "{what}: depth {}", b0.depth);
            assert!(!b0.inside, "{what}: starts inside");
            // The inverse rows undo the matrix.
            for l in [[0.3, -0.4, 0.5], [-0.9, 0.2, 0.7]] {
                let back = b0.local(world_of(s, l));
                assert!((0..3).all(|k| (back[k] - l[k]).abs() < 1e-3), "{what}: {l:?} → {back:?}");
            }
            *kinds.entry(mode::NAMES[b0.kind as usize]).or_insert(0) += 1;
            boxes += 1;

            // Walk in along +x through the middle, out through +x, back in and out through −x.
            let mut b = *b0;
            let mut r = ReverbRequest::default();
            let mut sent = Vec::new();
            let step = |b: &mut reverb::ReverbBox, r: &mut ReverbRequest, x: f32, sent: &mut Vec<SndCommand>| {
                b.update(world_of(s, [x, 0.0, 0.0]), r);
                sent.extend(r.command());
            };
            for i in 0..=60 { step(&mut b, &mut r, -1.5 + 0.05 * i as f32, &mut sent); }
            assert!(matches!(sent.first(), Some(SndCommand::SetReverb { kind, depth, .. }) if *kind == b0.kind && *depth < b0.depth / 10), "{what}: entry {sent:?}");
            let depths: Vec<i32> = sent.iter().filter_map(|c| match c { SndCommand::AutoReverb { depth, .. } => Some(*depth), _ => None }).collect();
            assert!(depths.windows(2).all(|w| w[0] <= w[1]), "{what}: the depth ramps up {depths:?}");
            assert_eq!((r.kind, r.depth), (b0.kind, b0.depth), "{what}: out through +x keeps the full depth");
            assert!(!b.inside);
            sent.clear();
            for i in 0..=60 { step(&mut b, &mut r, 1.5 - 0.05 * i as f32, &mut sent); }
            assert_eq!(sent.last(), Some(&SndCommand::SetReverb { kind: 0, depth: 0, delay: 0, feedback: 0 }), "{what}: out through −x");
            assert_eq!((r.kind, r.depth), (mode::OFF, 0));
        }
    }
    eprintln!("{levels} levels, {boxes} reverb boxes by type {kinds:?}");
}

/// 150 frames of `level`'s audio with the hero (and the camera) at `hero`, the skill point jingle (a level-bank sound)
/// at frame 2; music off.
fn render(la: &LevelAudio, hero: [f32; 3], wet: bool) -> Vec<[i16; 2]> {
    let mut sys = AudioSystem::new(la.clone());
    sys.music_option = 0;
    sys.spu.reverb.enabled = wet;
    let input = FrameInput { listener: listener_at(hero), hero_pos: hero };
    let mut out = Vec::new();
    for f in 0..150 {
        if f == 2 {
            let mut rng = Rng::new();
            assert!(sys.play_level_sound_at_moby(level_sound::SKILL_POINT, 0, None, None, &input.listener, &mut rng, f) >= 0);
        }
        sys.tick(&input, &mut out);
    }
    out
}

#[test]
fn reverb_in_the_mix_on_every_level() {
    let mut checked = 0;
    for level in 0..19u32 {
        let Some(la) = level_audio(level) else { continue };
        let sys = AudioSystem::new(la.clone());
        let Some(b) = sys.reverb_boxes.first() else { continue };
        // Inside near the +x face (nearly the full depth).
        let inside = world_of(&la.instances[b.instance], [0.9, 0.0, 0.0]);
        let (dry, wet) = (render(&la, inside, false), render(&la, inside, true));
        let diff: f64 = dry.iter().zip(&wet).map(|(a, b)| (a[0] as f64 - b[0] as f64).powi(2) + (a[1] as f64 - b[1] as f64).powi(2)).sum();
        assert!(diff > 0.0, "level {level}: no wet signal in box {}", b.instance);
        assert_eq!(wet, render(&la, inside, true), "level {level}: two runs differ");
        // Far from every zone: the reverb stays off and the mix is bit-identical.
        let far = [inside[0], inside[1], inside[2] + 5000.0];
        assert_eq!(render(&la, far, false), render(&la, far, true), "level {level}: reverb away from the zones");
        let rms = |v: &[[i16; 2]]| (v.iter().map(|s| (s[0] as f64).powi(2)).sum::<f64>() / v.len() as f64).sqrt();
        eprintln!("level {level} box {} ({}): dry RMS {:.0}, wet RMS {:.0}", b.instance, mode::NAMES[b.kind as usize], rms(&dry), rms(&wet));
        checked += 1;
    }
    eprintln!("{checked} levels checked");
}

#[test]
fn env_points_on_every_level() {
    let (mut enabled, mut disabled) = (0, 0);
    for level in 0..19u32 {
        let Some(la) = level_audio(level) else { continue };
        for p in la.env_points.clone() {
            let mut sys = AudioSystem::new(la.clone());
            sys.reverb_log = Some(Vec::new());
            // The first frame is the level start's HeroTeleport at the hero position.
            let input = FrameInput { listener: listener_at(p.position), hero_pos: p.position };
            sys.game_frame(&input);
            let log = sys.reverb_log.take().unwrap();
            let first = log.first().map(|e| e.1);
            // A point inside a reverb box: the box's update of the same frame decides.
            if sys.reverb_boxes.iter().any(|b| b.local(p.position).iter().all(|x| x.abs() <= 1.0)) { continue; }
            if p.reverb_enable != 0 {
                enabled += 1;
                assert_eq!(first, Some(SndCommand::SetReverb { kind: p.reverb_type, depth: p.reverb_depth, delay: p.reverb_delay, feedback: p.reverb_feedback }), "level {level}");
                assert!(reverb::preset(p.reverb_type).is_some() || p.reverb_type == mode::OFF, "level {level}: type {}", p.reverb_type);
            } else {
                disabled += 1;
                assert!(!matches!(first, Some(SndCommand::SetReverb { kind, .. }) if kind == p.reverb_type && kind != 0), "level {level}: {first:?}");
            }
        }
    }
    eprintln!("env points: {enabled} with reverb, {disabled} without");
}

#[test]
fn level_sounds_on_every_level() {
    let mut played = 0;
    for level in 0..19u32 {
        let Some(la) = level_audio(level) else { continue };
        for (index, flags) in [(level_sound::HELP_OPEN, 1u32), (level_sound::SKILL_POINT, 0)] {
            let mut sys = AudioSystem::new(la.clone());
            let l = listener_at([100.0, 100.0, 50.0]);
            let mut rng = Rng::new();
            let before = rng;
            let k = sys.play_level_sound_at_moby(index, flags, None, None, &l, &mut rng, 0);
            assert!(k >= 0, "level {level}: level sound {index} refused");
            assert_eq!(rng, before, "level {level}: level sound {index} drew from the stream");
            let s = sys.slots.slots[k as usize];
            // No owner, no position: 2-D at fixed volume at the listener; the slot remembers the def index.
            assert_eq!((s.flags & 0x11, s.class_index, s.owner), (0x11, index as u16, None));
            let input = FrameInput { listener: l, hero_pos: l.pos };
            let mut out = Vec::new();
            sys.tick(&input, &mut out);
            sys.game_frame(&input);
            let s = sys.slots.slots[k as usize];
            // The play callback confirmed a live 989snd handle (the next send then waits for a new reply: handle −1).
            assert_eq!(s.state, state::CONFIRMED, "level {level}: level sound {index} has no live handle ({} / {})", s.state, s.handle);
            played += 1;
            for bad in [MOBY_LEVEL_DEFS as i32, -1] { assert_eq!(sys.play_level_sound_at_moby(bad, 0, None, None, &l, &mut rng, 0), -1); }
        }
    }
    eprintln!("{played} level sounds played");
}

/// The body of `PlayLevelSoundAtMoby` (36 words, the same in every overlay except the `jal SoundSlotAlloc`, word 18).
const BODY: [u32; 36] = [
    0x27bdffd0, 0x3c020016, 0x8c42f574, 0x7fb00000, 0x7fb10010, 0x0080802d, 0x7fbf0020, 0x0202102a, 0x14400003, 0x00c0882d, 0x10000013, 0x2402ffff,
    0x3c020016, 0x8c42f5f4, 0x00102140, 0x0220302d, 0x0000382d, 0x00442021, 0, 0x24080400, 0x0040202d, 0x04800007, 0x24030070, 0x3c020014,
    0x00831818, 0x2442e550, 0x00621821, 0xa470007e, 0xac710088, 0x0080102d, 0x7bbf0020, 0x7bb10010, 0x7bb00000, 0x03e00008, 0x27bd0030, 0x00000000,
];

/// The last write of `reg` before `at` (up to 64 instructions back) or in the delay slot: `addiu/ori reg, zero, n` or
/// a move from zero.
fn arg(words: &[(u32, u32)], at: usize, reg: u32) -> Option<u32> {
    let decode = |w: u32| -> Option<Option<u32>> {
        let (op, rs, rt, rd, f) = (w >> 26, (w >> 21) & 31, (w >> 16) & 31, (w >> 11) & 31, w & 63);
        if (op == 9 || op == 0xd) && rt == reg { return Some((rs == 0).then_some(w & 0xffff)); }
        if op == 0 && rd == reg && matches!(f, 0x21 | 0x25 | 0x2d) { return Some((rs == 0 && rt == 0).then_some(0)); }
        None
    };
    if let Some(v) = words.get(at + 1).and_then(|&(_, w)| decode(w)) { return v; }
    (1..=64).filter_map(|k| at.checked_sub(k)).find_map(|i| decode(words[i].1)).flatten()
}

#[test]
fn level_sound_call_sites_in_every_overlay() {
    let mut total = std::collections::BTreeMap::new();
    let mut seen = 0;
    for level in 0..19u32 {
        let Ok(raw) = std::fs::read(rc_formats::test_data::level_dir(level).join("overlay.bin")) else { continue };
        let secs = rc_formats::font::parse_overlay_sections(&raw).unwrap();
        let words: Vec<(u32, u32)> = secs
            .iter()
            .filter(|s| s.kind != 8)
            .flat_map(|s| s.data.as_chunks::<4>().0.iter().enumerate().map(move |(i, c)| (s.dest + 4 * i as u32, u32::from_le_bytes(*c))))
            .collect();
        let fns: Vec<u32> = (0..words.len().saturating_sub(BODY.len()))
            .filter(|&i| BODY.iter().enumerate().all(|(k, &w)| if k == 18 { words[i + k].1 >> 26 == 3 } else { words[i + k].1 == w }))
            .map(|i| words[i].0)
            .collect();
        assert_eq!(fns.len(), 1, "level {level}: PlayLevelSoundAtMoby found {} times", fns.len());
        let jal = 0x0c00_0000 | (fns[0] >> 2);
        let mut sites = 0;
        for (i, &(addr, w)) in words.iter().enumerate() {
            if w != jal { continue; }
            let a = (arg(&words, i, 4), arg(&words, i, 5), arg(&words, i, 6));
            assert!(matches!(a, (Some(0), Some(1), Some(0)) | (Some(1), Some(0), Some(0))), "level {level} site {addr:#x}: {a:?}");
            *total.entry(a).or_insert(0) += 1;
            sites += 1;
        }
        assert!(sites >= 3, "level {level}: {sites} call sites");
        seen += 1;
    }
    eprintln!("{seen} overlays, call sites by (index, flags, moby): {total:?}");
}
