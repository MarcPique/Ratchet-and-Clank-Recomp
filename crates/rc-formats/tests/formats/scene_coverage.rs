//! Scene conformance: every scene on the disc (19 levels × 15 records, NTSC and PAL, plus the global space / item
//! scene lumps) parses, and **every byte and every field value of every chunk is one the port understands**
//! (docs/plan/cutscenes.md "Scene data coverage"). The chunk format has no command or event stream: a chunk is a
//! header, one camera record per tick, the actor records (class, streamed sequence, position track) and the subtitle
//! table. So "understood" means: each field holds a value the game's parser `FUN_00259288` / player
//! `CutsceneModeUpdate` / `FUN_002ac8d8` handles (and the port with it), or a value listed below as deliberately
//! ignored with the reason, and no byte of the chunk lies outside a known structure except zero padding and the
//! authoring tool's trailing filler. A new kind of data on the disc (a new flag value, a gap, an actor class the
//! scene player cannot draw) fails the test.
//!
//! Deliberately ignored (never read by the game's parser or player):
//! * header +0x02 (always 0), +0x0a (always −1), +0x0e (always 0);
//! * actor +0x04 (one value per scene: its chunk count as authored, the NTSC count also in most PAL copies) and +0x08
//!   (= the chunk index): authoring-tool fields;
//! * the sequence header's loop-sound byte (0), trigger count 0xff (no trigger words follow) and byte +0x13 (0xff):
//!   the actors never run `MobyAnimAdvance` (mode bit 2: not in the moby loop), the player sets their frames;
//! * the trailing filler ([`scene::TRAILING_FILLER`]) after the last structure;
//! * the scenes whose actor classes are not in the level ([`UNPLAYABLE`]): no code starts them and `CreateMoby`
//!   cannot create a class that is not loaded.
//!
//! Tests skip without `extracted/` (`rc_formats::test_data::root`).

use rc_formats::scene::{self, Region, Scene, SceneChunk, SceneEntry};
use rc_formats::{level, moby, toc, wad};
use std::collections::{BTreeMap, BTreeSet};

/// (level, scene) whose actor classes are not all in the level (core, or the spaceship classes 530..=533): none has
/// speech and no class starts them; the port reports them and draws the other actors.
const UNPLAYABLE: [(u32, usize); 11] = [(0, 6), (4, 3), (6, 4), (7, 5), (9, 1), (12, 5), (12, 8), (14, 7), (15, 6), (16, 3), (17, 3)];

#[derive(Default)]
struct Counts {
    regions: usize,
    chunks: usize,
    ticks: i64,
    camera_records: usize,
    cuts: usize,
    fov_changes: usize,
    actor_records: usize,
    frames: usize,
    subtitle_entries: usize,
    trailing_filler: usize,
    unknown: Vec<String>,
}

impl Counts {
    fn bad(&mut self, what: String) {
        if self.unknown.len() < 40 { self.unknown.push(what); } else if self.unknown.len() == 40 { self.unknown.push("…".into()); }
    }
}

/// Every structure of one decompressed chunk: fields in their known value sets, no byte outside a structure.
fn check_chunk(dec: &[u8], c: &SceneChunk, index: usize, tag: &str, n: &mut Counts) {
    let h = &c.header;
    if h.unknown_02 != 0 || h.unknown_0a != -1 || h.unknown_0e != 0 { n.bad(format!("{tag}: header +2/+a/+e = {} {} {}", h.unknown_02, h.unknown_0a, h.unknown_0e)); }
    // Audio start (0x16cd28): the stream continues once the tick reaches it; always before tick 0 on the disc.
    if h.audio_start >= 0 { n.bad(format!("{tag}: audio start {}", h.audio_start)); }
    if h.subtitle_offset < 0 { n.bad(format!("{tag}: subtitle offset {:#x}", h.subtitle_offset)); }
    for r in &c.camera {
        if r.cut > 1 { n.bad(format!("{tag}: camera word +0xc = {:#x}", r.cut)); }
        if !(r.tan_half_fov.is_finite() && r.tan_half_fov > 0.0) || !r.angles.iter().chain(&r.eye).all(|v| v.is_finite()) { n.bad(format!("{tag}: camera record {r:?}")); }
    }
    n.camera_records += c.camera.len();
    n.fov_changes += c.camera.windows(2).filter(|w| w[0].tan_half_fov != w[1].tan_half_fov).count();
    for (k, a) in c.actors.iter().enumerate() {
        // +0x08 = this chunk's index; +0x04 (one value per scene, checked in check_scene) = the scene's chunk count as
        // the authoring tool counted it (the NTSC count, also in most PAL copies).
        if a.unknown_04 <= 0 || a.unknown_08 != index as i32 { n.bad(format!("{tag} actor {k}: +4/+8 = {} {}", a.unknown_04, a.unknown_08)); }
        let sh = &a.sequence.header;
        if sh.loop_sound != 0 || sh.trigger_count != 0xff || sh.pad != 0xff || sh.trigger_data != 0 || sh.rate_override != 0.0 {
            n.bad(format!("{tag} actor {k}: sequence header {sh:?}"));
        }
        if a.positions.len() != a.sequence.frames.len() { n.bad(format!("{tag} actor {k}: {} positions for {} frames", a.positions.len(), a.sequence.frames.len())); }
        let joints: BTreeSet<u16> = a.sequence.frames.iter().map(|f| f.header.quat_bytes).collect();
        if joints.len() > 1 { n.bad(format!("{tag} actor {k}: joint counts change between frames {joints:?}")); }
        n.frames += a.sequence.frames.len();
    }
    n.actor_records += c.actors.len();
    for s in &c.subtitles {
        if s.pad != 0 || s.text_offsets.iter().any(|&o| o < 0) || s.end < s.start { n.bad(format!("{tag}: subtitle {} {} {:?} {}", s.start, s.end, s.text_offsets, s.pad)); }
    }
    n.subtitle_entries += c.subtitles.len();
    // Byte coverage.
    let mut cov = vec![false; dec.len()];
    let mut mark = |a: usize, len: usize| for b in cov.iter_mut().skip(a).take(len) { *b = true; };
    mark(0, 0x14 + 4 * c.actors.len());
    mark(h.camera_offset as usize, 0x20 * c.camera.len());
    for a in &c.actors {
        let at = a.offset as usize;
        mark(at, 0x2c + 4 * a.frame_offsets.len());
        for (f, &o) in a.sequence.frames.iter().zip(&a.frame_offsets) { mark(at + 0x10 + o as usize, 0x10 + f.payload.len()); }
        mark(a.track_offset as usize, 16 * a.positions.len());
    }
    if h.subtitle_offset >= scene::SUBTITLE_MIN_OFFSET {
        let base = h.subtitle_offset as usize;
        // The entries and the terminator entry (start = end = −1, a whole 16-byte entry).
        mark(base, 16 * (c.subtitles.len() + 1));
        for s in &c.subtitles {
            for (t, &o) in s.text.iter().zip(&s.text_offsets) { mark(base + o as usize, t.len() + 1); }
        }
    }
    let last = cov.iter().rposition(|&c| c).unwrap_or(0);
    if let Some(i) = (0..=last).find(|&i| !cov[i] && dec[i] != 0) { n.bad(format!("{tag}: byte {i:#x} = {:#x} outside every structure", dec[i])); }
    let tail = &dec[last + 1..];
    if let Some(i) = tail.iter().position(|&b| b != 0) {
        let t = &tail[i..];
        if t.starts_with(&scene::TRAILING_FILLER) { n.trailing_filler += 1; } else { n.bad(format!("{tag}: trailing data {:02x?} at {:#x} (sub off {:#x}, last {:#x}, len {:#x})", &t[..t.len().min(8)], last + 1 + i, h.subtitle_offset, last, dec.len())); }
    }
}

fn check_scene(s: &Scene, dec: &[Vec<u8>], tag: &str, n: &mut Counts) {
    n.regions += 1;
    n.chunks += s.chunks.len();
    n.ticks += s.end_tick() as i64;
    n.cuts += s.cut_ticks().len();
    let classes = s.actor_classes();
    let u04: BTreeSet<i32> = s.chunks.iter().flat_map(|c| &c.actors).map(|a| a.unknown_04).collect();
    if u04.len() > 1 { n.bad(format!("{tag}: actor +0x04 varies {u04:?}")); }
    for (i, (c, d)) in s.chunks.iter().zip(dec).enumerate() {
        let got: Vec<i32> = c.actors.iter().map(|a| a.class).collect();
        if got != classes { n.bad(format!("{tag} chunk {i}: actor classes {got:?} ≠ {classes:?}")); }
        check_chunk(d, c, i, &format!("{tag} chunk {i}"), n);
    }
}

#[test]
fn every_scene_on_the_disc_is_understood() {
    let root = rc_formats::test_data::root();
    if !root.join("levels/01/level_header.bin").exists() { eprintln!("skipped: no extracted/"); return; }
    let mut n = Counts::default();
    let mut unplayable = BTreeSet::new();
    let mut per_scene = Vec::new();
    let mut class_modes: BTreeMap<i32, i16> = BTreeMap::new();
    let mut scenes = 0;
    for lvl in 0..19u32 {
        let dir = root.join(format!("levels/{lvl:02}"));
        let h = toc::parse_level_header(&std::fs::read(dir.join("level_header.bin")).unwrap()).unwrap();
        let idx = std::fs::read(dir.join("core_index.bin")).unwrap();
        let data = wad::decompress(&std::fs::read(dir.join("core_data.bin")).unwrap()).unwrap();
        let core = level::parse_level_core(&idx, data.len()).unwrap();
        let drawn: BTreeMap<i32, i16> = moby::parse_level_mobys(&core, &data).unwrap().iter().map(|c| (c.o_class, c.class.header.mode_bits)).collect();
        for (k, rec) in h.scenes.iter().enumerate() {
            let e = SceneEntry::new(rec).unwrap();
            if e.ntsc.is_some() || e.pal.is_some() { scenes += 1; }
            for region in [Region::Ntsc, Region::Pal] {
                let Some(rt) = e.region(region).filter(|r| !r.chunks.is_empty()) else { continue };
                let file = std::fs::read(dir.join(format!("scene/{k:02}_{}.bin", region.name()))).unwrap();
                let s = Scene::load(&h, &file, k, region).unwrap();
                let dec: Vec<Vec<u8>> = (0..rt.chunks.len()).map(|i| wad::decompress(&file[rt.chunk_in_file(i).unwrap()]).unwrap()).collect();
                let tag = format!("L{lvl:02} S{k:02} {}", region.name());
                check_scene(&s, &dec, &tag, &mut n);
                let missing: Vec<i32> = s.actor_classes().into_iter().filter(|c| !drawn.contains_key(c) && !(530..=533).contains(c)).collect();
                if !missing.is_empty() {
                    unplayable.insert((lvl, k));
                    if e.speech.iter().any(Option::is_some) { n.bad(format!("{tag}: classes {missing:?} not loaded but the scene has speech")); }
                }
                for c in s.actor_classes() { if let Some(&m) = drawn.get(&c) { class_modes.insert(c, m); } }
                if region == Region::Ntsc {
                    let tans: BTreeSet<u32> = s.chunks.iter().flat_map(|c| &c.camera).map(|c| c.tan_half_fov.to_bits()).collect();
                    per_scene.push(format!(
                        "L{lvl:02} S{k:02}: {} ticks, {} chunks, actors {:?}, cuts {}, subtitle lines {}, tan(hfov/2) {}, speech {}{}",
                        s.end_tick(),
                        s.chunks.len(),
                        s.actor_classes(),
                        s.cut_ticks().len(),
                        s.chunks.iter().flat_map(|c| &c.subtitles).map(|t| (t.start, t.end)).collect::<BTreeSet<_>>().len(),
                        if tans.len() == 1 { format!("{:.3}", f32::from_bits(*tans.iter().next().unwrap())) } else { format!("{} values (animated FOV)", tans.len()) },
                        e.speech.iter().filter(|s| s.is_some()).count(),
                        if missing.is_empty() { String::new() } else { format!(", classes not loaded {missing:?}") }
                    ));
                }
            }
        }
    }
    // Global scene lumps (mode 6: space take-off / landing, item scenes): the same chunk format.
    let mut lumps = 0;
    for (dir, half) in [("global/unknown_12e8", 10usize), ("global/unknown_1530", 14)] {
        for i in 0..2 * half {
            let Ok(f) = std::fs::read(root.join(format!("{dir}/{i:03}.bin"))) else { continue };
            if scene::lump_chunks(&f).map(|c| c.is_empty()).unwrap_or(true) { continue; }
            let region = if i < half { Region::Ntsc } else { Region::Pal };
            let s = Scene::from_lump(&f, i, region).unwrap_or_else(|e| panic!("{dir}/{i:03}: {e}"));
            let dec: Vec<Vec<u8>> = scene::lump_chunks(&f).unwrap().into_iter().map(|c| wad::decompress(c).unwrap()).collect();
            check_scene(&s, &dec, &format!("{dir}/{i:03}"), &mut n);
            if i < half { per_scene.push(format!("{dir}/{i:03}: {} ticks, {} chunks, actors {:?}, cuts {}", s.end_tick(), s.chunks.len(), s.actor_classes(), s.cut_ticks().len())); }
            lumps += 1;
        }
    }
    for l in &per_scene { eprintln!("{l}"); }
    // Every drawn actor class takes the regular moby draw (no additive 0x200, no fading 8 mode bit).
    let blended: Vec<(i32, i16)> = class_modes.iter().filter(|(_, &m)| m as u16 & 0x208 != 0).map(|(&c, &m)| (c, m)).collect();
    eprintln!(
        "scenes: {scenes} level scenes, {lumps} global lumps; {} regions, {} chunks, {} ticks, {} camera records ({} cuts, {} FOV steps), {} actor records, \
         {} animation frames, {} subtitle entries, {} chunks with trailing filler; unplayable {:?}",
        n.regions, n.chunks, n.ticks, n.camera_records, n.cuts, n.fov_changes, n.actor_records, n.frames, n.subtitle_entries, n.trailing_filler, unplayable
    );
    assert!(n.unknown.is_empty(), "scene data the port does not understand:\n{}", n.unknown.join("\n"));
    assert_eq!(unplayable, UNPLAYABLE.into_iter().collect::<BTreeSet<_>>(), "scenes with actor classes outside their level");
    assert!(blended.is_empty(), "scene actor classes with blend mode bits: {blended:?}");
    assert_eq!(scenes, 138);
}
