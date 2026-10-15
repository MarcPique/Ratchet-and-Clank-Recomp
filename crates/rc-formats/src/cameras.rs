//! The level's camera records (gameplay section 0x08) and their pvar blocks: the data of the level camera system
//! (docs/plan/player_controller.md §15, "The level camera system").
//!
//! **File layout** (docs/formats/wad_layouts_rac1.md §3.13): `s32 count, pad[3]`, then `count` × 0x20-byte records
//! `{i32 class, f32 pos[3], f32 rot[3], i32 pvar}`. **Loader** (`InitLevelRenderGlobals` 0x255958, level01): the table
//! `0x15ef50` (count `0x15ef54`) gets the records **reordered** to `{pos[3], class @+0x0c, rot[3], pvar @+0x1c}`, and
//! after the moby pvars each record's `+0x1c` becomes the address of its pvar block's copy (0 for index −1; the
//! pointer fixups apply to it as to a moby's). Every camera's update and activation reads its record through
//! `0x15ef50 + slot·0x20` (UpdateCam +0x84 = the record index).
//!
//! **The pvar header** (every class, [`CameraHeader`]): the region test's shapes (+0x08 sphere, +0x0c cuboid, +0x10
//! cylinder, +0x14 path), +0x04 = the slot's UpdateCam (written by the slot init 0x20ef58), and the four bytes the slot
//! init and the switch read: priority +0x1c (UpdateCam +0x7c), blend kind +0x1d (`FUN_0020d110`), +0x1e (not read by
//! the system [L]) and the activation kind +0x1f (UpdateCam +0x74, `Camera_ActivationCheckPriority` 0x20d410).
//! **Class 17** (the follow-camera tweak regions, 58 records on 14 levels) has a 0x60-byte block, [`RegionTweak`].
//!
//! **The classes a level has** are its overlay's `lvl.camvtbl` (`crate::level_overlay::LevelOverlay::camvtbl`):
//! `{class, activate, init, update, pre}` per class, `-1` ending it.

use crate::buf::{invalid, Buf, Result};

/// Gameplay header pointer of the camera section.
pub const CAMERAS_POINTER: usize = 0x08;
/// Size of one record (file and runtime).
pub const RECORD_SIZE: usize = 0x20;

/// One camera record, as the file has it (the loader's reordering is not kept: the fields are named).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraRecord {
    /// File +0x00 (runtime +0x0c, read as s16): the camera class, UpdateCam +0x86.
    pub class: i32,
    /// File +0x04 (runtime +0x00).
    pub pos: [f32; 3],
    /// File +0x10 (runtime +0x10); class 17 reads `rot.z` as the region's facing (`0x318de0`).
    pub rot: [f32; 3],
    /// File +0x1c: the pvar index (−1: none).
    pub pvar_index: i32,
}

/// The camera records of a decompressed gameplay file (empty when the section is absent).
pub fn parse_cameras(gameplay: &[u8]) -> Result<Vec<CameraRecord>> {
    let g = Buf(gameplay);
    let p = g.u32(CAMERAS_POINTER)? as usize;
    if p == 0 { return Ok(Vec::new()); }
    let n = g.i32(p)?;
    if !(0..=48).contains(&n) { return invalid(format!("camera count {n} (the runtime has 48 slots)")); }
    (0..n as usize)
        .map(|i| {
            let o = p + 0x10 + RECORD_SIZE * i;
            Ok(CameraRecord {
                class: g.i32(o)?,
                pos: [g.f32(o + 4)?, g.f32(o + 8)?, g.f32(o + 0xc)?],
                rot: [g.f32(o + 0x10)?, g.f32(o + 0x14)?, g.f32(o + 0x18)?],
                pvar_index: g.i32(o + 0x1c)?,
            })
        })
        .collect()
}

/// A record with its pvar block (the loader's copy; None for index −1).
#[derive(Clone, Debug, PartialEq)]
pub struct LevelCamera {
    pub record: CameraRecord,
    pub pvar: Option<Vec<u8>>,
}

/// The camera records of a gameplay file with their pvar blocks.
pub fn parse_level_cameras(gameplay: &[u8]) -> Result<Vec<LevelCamera>> {
    parse_cameras(gameplay)?
        .into_iter()
        .map(|record| Ok(LevelCamera { record, pvar: crate::sound_bank::pvar_block(gameplay, record.pvar_index)? }))
        .collect()
}

/// The loader's moby-link fixups (gameplay 0x50) on the camera records' pvar blocks: each listed s32 is a gameplay
/// instance index, replaced by that instance's runtime moby index (`0x1acc00[i]`, −1 when it was not created on this
/// load); negative values are kept. Class 18's moby +0x28 is one (on every record that names a moby). The blocks are
/// matched by the record's pvar index, as `crate::gameplay::parse_pvars_spawned` does for the mobys' blocks.
pub fn remap_moby_links(cams: &mut [LevelCamera], gameplay: &[u8], instance_to_moby: &dyn Fn(usize) -> Option<usize>) -> Result<()> {
    let fixups = crate::gameplay::parse_pvar_fixups(gameplay)?;
    for c in cams.iter_mut() {
        let Some(p) = c.pvar.as_mut() else { continue };
        for &(index, ofs) in fixups.moby_links.iter().filter(|f| f.0 == c.record.pvar_index) {
            let Some(o) = usize::try_from(ofs).ok().filter(|&o| o + 4 <= p.len()) else { continue };
            let v = i32::from_le_bytes(p[o..o + 4].try_into().unwrap());
            if v < 0 { continue; }
            let r = instance_to_moby(v as usize).map_or(-1, |m| m as i32);
            p[o..o + 4].copy_from_slice(&r.to_le_bytes());
            let _ = index;
        }
    }
    Ok(())
}

/// The pvar header every camera class has (the first 0x20 bytes; module doc).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CameraHeader {
    /// +0x00: class-specific (class 17: the turn in degrees).
    pub w00: f32,
    /// +0x08 sphere, +0x0c cuboid, +0x10 cylinder, +0x14 path (−1: none): the region test's shapes.
    pub sphere: i32,
    pub cuboid: i32,
    pub cylinder: i32,
    pub path: i32,
    /// +0x18: 1.5 on most records (not read by the system or class 17 [L]).
    pub f18: f32,
    /// +0x1c: priority (UpdateCam +0x7c; 0 = never).
    pub priority: u8,
    /// +0x1d: the blend kind of a switch to this camera (`FUN_0020d110`: 1 / 5 blend, 3 / 6 copy the pose, else cut).
    pub blend: u8,
    /// +0x1e.
    pub b1e: u8,
    /// +0x1f: the activation kind (UpdateCam +0x74): 0 always, 1 / 2 when entered (+0x7d), 4 hero in the cuboid,
    /// 7 the hero's camera mode 0x1415d4 = the class, else never (the class's own hook only).
    pub activation: u8,
}

impl CameraHeader {
    /// The header of a pvar block (None when shorter than 0x20).
    pub fn parse(pvar: &[u8]) -> Option<CameraHeader> {
        let b = Buf(pvar);
        Some(CameraHeader {
            w00: b.f32(0).ok()?,
            sphere: b.i32(8).ok()?,
            cuboid: b.i32(0xc).ok()?,
            cylinder: b.i32(0x10).ok()?,
            path: b.i32(0x14).ok()?,
            f18: b.f32(0x18).ok()?,
            priority: b.u8(0x1c).ok()?,
            blend: b.u8(0x1d).ok()?,
            b1e: b.u8(0x1e).ok()?,
            activation: b.u8(0x1f).ok()?,
        })
    }
}

/// Class 17's pvar block (0x60 bytes): a region that retunes the follow camera while Ratchet is in it (level01
/// activation `0x319688` → `0x318de0`, region test `0x318c40`; the same code on all 14 levels that have the class).
/// The run-time words (+0x2e counter, +0x40 / +0x44 the captured springs, +0x4a "left") start as the file has them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RegionTweak {
    pub header: CameraHeader,
    /// +0x00: the turn toward the region's facing, degrees per tick at full strength (0: none).
    pub turn: f32,
    /// +0x20: the turn's tolerance, degrees (`0x313888`'s first argument; 0: always).
    pub tolerance: f32,
    /// +0x24 distance, +0x28 pivot height, +0x30 look height (0: leave the follow camera's).
    pub distance: f32,
    pub pivot_height: f32,
    /// +0x2c (s16): the mode (0..11; module doc of `rc_game::follow_camera::level`).
    pub mode: i16,
    /// +0x2e (s16): ticks inside (run time).
    pub counter: i16,
    pub look_height: f32,
    /// +0x34 (s16): 0 = the leash off (`0x313740(0)`).
    pub leash: i16,
    /// +0x36 (s16): only while the camera faces within 80° of the region's facing (`rot.z`).
    pub facing: i16,
    /// +0x38 / +0x3c: the horizontal spring (k, d) eased in over 120 ticks from +0x40 / +0x44 (captured on entry).
    pub spring_k: f32,
    pub spring_d: f32,
    pub from_k: f32,
    pub from_d: f32,
    /// +0x48 (s16): once (with +0x4a: not again after leaving); mode 5: L1 / L2 held (0x13cae0 & 5) cancels.
    pub once: i16,
    /// +0x4a (s16): left once (run time).
    pub left: i16,
    /// +0x4c: a second cuboid.
    pub cuboid2: i32,
    /// +0x50: the scripted pitch, degrees (0: none).
    pub pitch: f32,
    /// +0x54 / +0x56 (s16): the stick's pitch (D+0x1b8 |= 2) / yaw (|= 1) off.
    pub no_pitch: i16,
    pub no_yaw: i16,
}

impl RegionTweak {
    pub fn parse(pvar: &[u8]) -> Option<RegionTweak> {
        let b = Buf(pvar);
        let f = |o| b.f32(o).ok();
        let h = |o| b.i16(o).ok();
        Some(RegionTweak {
            header: CameraHeader::parse(pvar)?,
            turn: f(0)?,
            tolerance: f(0x20)?,
            distance: f(0x24)?,
            pivot_height: f(0x28)?,
            mode: h(0x2c)?,
            counter: h(0x2e)?,
            look_height: f(0x30)?,
            leash: h(0x34)?,
            facing: h(0x36)?,
            spring_k: f(0x38)?,
            spring_d: f(0x3c)?,
            from_k: f(0x40)?,
            from_d: f(0x44)?,
            once: h(0x48)?,
            left: h(0x4a)?,
            cuboid2: b.i32(0x4c).ok()?,
            pitch: f(0x50)?,
            no_pitch: h(0x54)?,
            no_yaw: h(0x56)?,
        })
    }
}

/// Class 23's pvar block (0x60 bytes): a region that **places the follow camera at the record's position** while
/// Ratchet is in it (level00 activation `0x2ed8b0` → region test `0x2ed348`, update `0x2ed498`; the same code on the
/// 11 levels that have the class): the arrival spots (Ratchet starts inside it at the ship). Once Ratchet is outside
/// with the follow camera up it is done (+0x26) for good. Run-time words (+0x20, +0x26, +0x38..+0x40) start as the
/// file has them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlacedView {
    pub header: CameraHeader,
    /// +0x20: ticks inside (run time, at most 200).
    pub counter: i32,
    /// +0x24 (s16): 0 = the leash off (`0x313740(0)`).
    pub leash: i16,
    /// +0x26 (s16): done (run time).
    pub done: i16,
    /// +0x28: the scripted pitch, degrees (0: none).
    pub pitch: f32,
    /// +0x2c: the look angle (radians): look height = height − distance·tan(angle) (0 at ±90°).
    pub look_angle: f32,
    /// +0x30: not read by the class [L].
    pub f30: f32,
    /// +0x34: only while the follow camera's yaw step D+0x164 and pitch input D+0x1b0 are within ±0.01.
    pub still: i32,
    /// +0x38 distance, +0x3c height, +0x40 look height: from the record's position on the first two ticks (run time).
    pub distance: f32,
    pub height: f32,
    pub look_height: f32,
    /// +0x44 (s16): not as Clank (body 1); +0x46 (s16): only as Clank.
    pub not_clank: i16,
    pub clank_only: i16,
}

impl PlacedView {
    pub fn parse(pvar: &[u8]) -> Option<PlacedView> {
        let b = Buf(pvar);
        let f = |o| b.f32(o).ok();
        let h = |o| b.i16(o).ok();
        Some(PlacedView {
            header: CameraHeader::parse(pvar)?,
            counter: b.i32(0x20).ok()?,
            leash: h(0x24)?,
            done: h(0x26)?,
            pitch: f(0x28)?,
            look_angle: f(0x2c)?,
            f30: f(0x30)?,
            still: b.i32(0x34).ok()?,
            distance: f(0x38)?,
            height: f(0x3c)?,
            look_height: f(0x40)?,
            not_clank: h(0x44)?,
            clank_only: h(0x46)?,
        })
    }
}

/// Class 18's pvar block (0x60 bytes): a region that **turns the follow camera toward a moby** (or along the
/// record's facing) while Ratchet is in it (level02 hook `0x2fc298` → `0x2fb9c8`, region test `0x2fb648`, view test
/// `0x2fb788`; the same code on the 7 levels that have the class). Run-time words (+0x20, +0x3e, +0x40, +0x48) start
/// as the file has them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MobyFocus {
    pub header: CameraHeader,
    /// +0x00: the turn, degrees per tick at full strength.
    pub turn: f32,
    /// +0x20 (s16): ticks inside (run time; −1: the moby is gone, for good).
    pub counter: i16,
    /// +0x22: 1 = the region is the distance +0x24 from the moby (else the header's first shape).
    pub near_kind: u8,
    /// +0x24: that distance.
    pub radius: f32,
    /// +0x28: the moby (table index; used when the group +0x44 is −1).
    pub moby: i32,
    /// +0x2c: the view test's elevation limit, degrees (0: none); +0x30: its flat angle limit, degrees (0: none).
    pub max_pitch: f32,
    pub max_angle: f32,
    /// +0x34 distance, +0x38 pivot height, +0x4c look height (0: leave the follow camera's).
    pub distance: f32,
    pub pivot_height: f32,
    /// +0x3c (s16): the mode (1..7; `rc_game::follow_camera::focus`).
    pub mode: i16,
    /// +0x3e (s16): the second counter (run time).
    pub counter2: i16,
    /// +0x40: the facing the modes 4 / 5 turn to (run time).
    pub yaw: f32,
    /// +0x44: the moby group (its first live member; −1: the moby +0x28).
    pub group: i32,
    pub look_height: f32,
    /// +0x50: set → the region leaves this tick (cleared by the hook every tick; no writer found [L]).
    pub suppress: i32,
}

impl MobyFocus {
    pub fn parse(pvar: &[u8]) -> Option<MobyFocus> {
        let b = Buf(pvar);
        let f = |o| b.f32(o).ok();
        let h = |o| b.i16(o).ok();
        Some(MobyFocus {
            header: CameraHeader::parse(pvar)?,
            turn: f(0)?,
            counter: h(0x20)?,
            near_kind: b.u8(0x22).ok()?,
            radius: f(0x24)?,
            moby: b.i32(0x28).ok()?,
            max_pitch: f(0x2c)?,
            max_angle: f(0x30)?,
            distance: f(0x34)?,
            pivot_height: f(0x38)?,
            mode: h(0x3c)?,
            counter2: h(0x3e)?,
            yaw: f(0x40)?,
            group: b.i32(0x44).ok()?,
            look_height: f(0x4c)?,
            suppress: b.i32(0x50).ok()?,
        })
    }
}

/// Class 3's pvar block (0x40 bytes): the **rail / slide camera** (level01 init `0x315de0` + `0x314e98`, update
/// `0x315358`, pre hook `0x316030`; 13 records on 01, 05, 08, 16, 17). The camera rides a camera path (+0x20) beside
/// Ratchet's rail or slide and looks ahead along it. The run-time words (+0x34, +0x36) start as the file has them.
///
/// | offset | type | field |
/// |---|---|---|
/// | 0x00 | f32 | the look-ahead along the path / rail (0.5 .. 15) |
/// | 0x20 | s32 | the camera path (`0x1b0930[i]`; −1 on level 17's record: never current) |
/// | 0x24 | s32 | the grind path (`0x15f70c[i]`) Ratchet must ride (the kind-7 check, the pre hook); −1: any |
/// | 0x28 / 0x2c | s32 | mode 2's mapping paths: +0x28 over the camera path, +0x2c over the rail |
/// | 0x30 | s32 | the mode: 0 parallel to the rail, 2 mapped piecewise, else (1, 3) the path point nearest Ratchet |
/// | 0x34 | s16 | mode 2's mapping done (set by the first init; kept for the level) |
/// | 0x36 | s16 | the init found the camera ahead of Ratchet along the path (mode 3): the place offset flips |
/// | 0x38 | f32 | the place offset along the path / rail (−3, −4: behind) |
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RailCamera {
    pub header: CameraHeader,
    pub ahead: f32,
    pub path: i32,
    pub rail: i32,
    pub map_path: i32,
    pub map_rail: i32,
    pub mode: i32,
    pub mapped: i16,
    pub flipped: i16,
    pub along: f32,
}

impl RailCamera {
    /// The block (None when shorter than 0x3c).
    pub fn parse(pvar: &[u8]) -> Option<RailCamera> {
        let b = Buf(pvar);
        Some(RailCamera {
            header: CameraHeader::parse(pvar)?,
            ahead: b.f32(0).ok()?,
            path: b.i32(0x20).ok()?,
            rail: b.i32(0x24).ok()?,
            map_path: b.i32(0x28).ok()?,
            map_rail: b.i32(0x2c).ok()?,
            mode: b.i32(0x30).ok()?,
            mapped: b.i16(0x34).ok()?,
            flipped: b.i16(0x36).ok()?,
            along: b.f32(0x38).ok()?,
        })
    }
}

/// Class 14's pvar block (0x40 bytes): the **side view** (level03 hook `0x2ebd70`, init `0x2eb978`, update
/// `0x2ebdb0`, pre hook `0x2ebde8`; Kerwan's 4 records): while Ratchet's feet are in the header's cuboid the camera
/// keeps the record's facing (`rot` as Euler rows, row 0) at +0x20 behind and +0x24 above a smoothed Ratchet. Class 1
/// (the fixed view, level03 `0x2e8870`.., 03 / 04) reads only the header (its look height is the header's +0x18).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SideView {
    pub header: CameraHeader,
    /// +0x20: the distance behind along the record's facing (8).
    pub distance: f32,
    /// +0x24: the height above Ratchet along −gravity (4).
    pub height: f32,
}

impl SideView {
    /// The block (None when shorter than 0x28).
    pub fn parse(pvar: &[u8]) -> Option<SideView> {
        let b = Buf(pvar);
        Some(SideView { header: CameraHeader::parse(pvar)?, distance: b.f32(0x20).ok()?, height: b.f32(0x24).ok()? })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(records: &[(i32, i32)]) -> Vec<u8> {
        let mut g = vec![0u8; 0x100];
        g[8..12].copy_from_slice(&0x20u32.to_le_bytes());
        g[0x20..0x24].copy_from_slice(&(records.len() as i32).to_le_bytes());
        for (i, &(class, pvar)) in records.iter().enumerate() {
            let o = 0x30 + 0x20 * i;
            g[o..o + 4].copy_from_slice(&class.to_le_bytes());
            g[o + 4..o + 8].copy_from_slice(&1.5f32.to_le_bytes());
            g[o + 0x18..o + 0x1c].copy_from_slice(&2.0f32.to_le_bytes());
            g[o + 0x1c..o + 0x20].copy_from_slice(&pvar.to_le_bytes());
        }
        g
    }

    #[test]
    fn records_in_file_order() {
        let c = parse_cameras(&file(&[(17, 3), (0, -1)])).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0], CameraRecord { class: 17, pos: [1.5, 0.0, 0.0], rot: [0.0, 0.0, 2.0], pvar_index: 3 });
        assert_eq!(c[1].class, 0);
        assert_eq!(c[1].pvar_index, -1);
    }

    /// The moby-link fixups reach the camera blocks by pvar index: an instance index becomes the runtime index, a
    /// missing instance −1, a negative value stays.
    #[test]
    fn moby_links_remapped() {
        let mut g = vec![0u8; 0x200];
        g[0x50..0x54].copy_from_slice(&0x100u32.to_le_bytes());
        for (k, (idx, ofs)) in [(3i32, 0x28i32), (4, 0x28), (5, 0x28), (-1, 0)].iter().enumerate() {
            g[0x100 + 8 * k..0x104 + 8 * k].copy_from_slice(&idx.to_le_bytes());
            g[0x104 + 8 * k..0x108 + 8 * k].copy_from_slice(&ofs.to_le_bytes());
        }
        let cam = |pvar_index, v: i32| {
            let mut p = vec![0u8; 0x60];
            p[0x28..0x2c].copy_from_slice(&v.to_le_bytes());
            LevelCamera { record: CameraRecord { class: 18, pos: [0.0; 3], rot: [0.0; 3], pvar_index }, pvar: Some(p) }
        };
        let mut cams = vec![cam(3, 7), cam(4, 9), cam(5, -1), cam(6, 7)];
        remap_moby_links(&mut cams, &g, &|i| if i == 7 { Some(2) } else { None }).unwrap();
        let v: Vec<i32> = cams.iter().map(|c| MobyFocus::parse(c.pvar.as_deref().unwrap()).map_or(0, |f| f.moby)).collect();
        assert_eq!(v, [2, -1, -1, 7]);
    }

    #[test]
    fn region_tweak_fields() {
        let mut p = vec![0u8; 0x60];
        p[0..4].copy_from_slice(&8.0f32.to_le_bytes());
        p[0xc..0x10].copy_from_slice(&34i32.to_le_bytes());
        p[0x1c..0x20].copy_from_slice(&[4, 3, 3, 3]);
        p[0x2c..0x2e].copy_from_slice(&2i16.to_le_bytes());
        p[0x4c..0x50].copy_from_slice(&(-1i32).to_le_bytes());
        let t = RegionTweak::parse(&p).unwrap();
        assert_eq!((t.turn, t.header.cuboid, t.header.priority, t.header.activation, t.mode, t.cuboid2), (8.0, 34, 4, 3, 2, -1));
        assert!(RegionTweak::parse(&p[..0x40]).is_none());
    }

    /// Survey (ignored): every record that is not class 0 / 4 / 5 / 6 / 17, with its header and pvar words, for the
    /// camera-class ports (docs/plan/player_controller.md §15).
    #[test]
    #[ignore]
    fn survey_other_classes() {
        for level in 0..19u32 {
            let Some(g) = crate::test_data::gameplay(level) else { return };
            if let Ok(b) = std::fs::read(crate::test_data::level_dir(level).join("overlay.bin")) {
                let ov = crate::level_overlay::LevelOverlay::parse(&b).unwrap();
                for e in ov.camvtbl() { eprintln!("L{level:02} camvtbl class {} activate {:#x} init {:#x} update {:#x} pre {:#x}", e.class, e.activate, e.init, e.update, e.pre); }
            }
            for (i, c) in parse_level_cameras(&g).unwrap().iter().enumerate() {
                let p = c.pvar.as_deref().unwrap_or(&[]);
                let h = CameraHeader::parse(p).unwrap_or_default();
                if c.record.class == 17 { continue; }
                if c.record.class == 18 {
                    let inst = crate::gameplay::parse_moby_instances(&g).unwrap();
                    let f = MobyFocus::parse(p).unwrap();
                    let m = usize::try_from(f.moby).ok().and_then(|i| inst.get(i));
                    let fx = crate::gameplay::parse_pvar_fixups(&g).unwrap();
                    let links: Vec<i32> = fx.moby_links.iter().filter(|l| l.0 == c.record.pvar_index).map(|l| l.1).collect();
                    eprintln!("L{level:02} #{i} class 18 mode {} moby {} -> {:?} links {links:x?}", f.mode, f.moby, m.map(|m| (m.o_class, m.position, m.spawn_id)));
                }
                let words: Vec<String> = p.chunks(4).skip(8).map(|w| { let v = u32::from_le_bytes(w.try_into().unwrap_or([0; 4])); format!("{v:08x}({})", f32::from_bits(v)) }).collect();
                eprintln!("L{level:02} #{i} class {} pos {:?} rot {:?} len {:#x} shapes s{} c{} y{} p{} prio {} blend {} b1e {} act {} | {}", c.record.class, c.record.pos, c.record.rot, p.len(), h.sphere, h.cuboid, h.cylinder, h.path, h.priority, h.blend, h.b1e, h.activation, words.join(" "));
            }
        }
    }

    /// Every level's records from the extracted data: the counts, the class census (the system cameras 0, 4, 5, 6, 7
    /// once per level; 58 class-17 regions on 14 levels), every class in the level's `lvl.camvtbl`, the class-17
    /// blocks (0x60 bytes, activation kind 3 = the class's own hook, modes 0..11), and the empty camera-collision grid.
    #[test]
    fn every_level_disc() {
        const COUNTS: [usize; 19] = [7, 7, 7, 34, 8, 8, 8, 7, 13, 10, 15, 6, 11, 7, 15, 13, 12, 9, 7];
        const CENSUS: [(i32, usize); 16] =
            [(0, 19), (1, 2), (3, 13), (4, 19), (5, 19), (6, 19), (7, 19), (8, 2), (14, 4), (17, 58), (18, 8), (19, 6), (20, 1), (21, 1), (22, 2), (23, 12)];
        let mut census = std::collections::BTreeMap::new();
        let mut modes = std::collections::BTreeSet::new();
        for level in 0..19u32 {
            let Some(g) = crate::test_data::gameplay(level) else { eprintln!("skipped: no extracted/"); return };
            let cams = parse_level_cameras(&g).unwrap();
            assert_eq!(cams.len(), COUNTS[level as usize], "level {level}");
            // The camera-collision grid (section 0x84, `0x20fdb0`'s table 0x15ef80): a 0x10-byte header and 64 × 64
            // cell words, every one 0 (no primitive anywhere: the grid's users never find one).
            let grid = crate::gameplay::section(&g, "camera_collision_grid").unwrap().expect("the grid section");
            assert_eq!(grid.len(), 0x4010, "level {level}");
            assert!(grid[0x10..].iter().all(|&b| b == 0), "level {level}: an empty grid");
            let ov = std::fs::read(crate::test_data::level_dir(level).join("overlay.bin")).ok().map(|b| crate::level_overlay::LevelOverlay::parse(&b).unwrap());
            let classes: Vec<i32> = ov.map(|o| o.camvtbl().iter().map(|e| e.class).collect()).unwrap_or_default();
            if !classes.is_empty() { assert_eq!(classes[0], 0, "level {level}: class 0 first"); }
            for (i, c) in cams.iter().enumerate() {
                *census.entry(c.record.class).or_insert(0usize) += 1;
                if !classes.is_empty() { assert!(classes.contains(&c.record.class), "level {level} record {i}: class {} not in lvl.camvtbl", c.record.class); }
                let p = c.pvar.as_deref().expect("every record has a pvar block");
                let h = CameraHeader::parse(p).unwrap();
                match c.record.class {
                    0 => assert_eq!((p.len(), h.priority, h.activation), (0x20, 5, 0), "level {level}"),
                    17 => {
                        assert_eq!(p.len(), 0x60, "level {level}");
                        let t = RegionTweak::parse(p).unwrap();
                        assert_eq!(t.counter, 0, "level {level} record {i}: no run time on disc");
                        assert!((0..=11).contains(&t.mode), "level {level} record {i}: mode {}", t.mode);
                        modes.insert(t.mode);
                        // Only level 00's record 1 has the cuboid kind (4, priority 4 < the follow camera's 5: never).
                        assert_eq!(h.activation, if (level, i) == (0, 1) { 4 } else { 3 }, "level {level} record {i}");
                    }
                    18 => {
                        assert_eq!(p.len(), 0x60, "level {level}");
                        let v = MobyFocus::parse(p).unwrap();
                        assert_eq!((v.counter, v.counter2, v.suppress, h.activation), (0, 0, 0, 3), "level {level} record {i}: no run time on disc");
                        assert!((1..=7).contains(&v.mode), "level {level} record {i}: mode {}", v.mode);
                    }
                    23 => {
                        assert_eq!(p.len(), 0x60, "level {level}");
                        let v = PlacedView::parse(p).unwrap();
                        assert_eq!((v.counter, v.done, h.activation), (0, 0, 3), "level {level} record {i}: no run time on disc");
                        assert!(v.look_angle.abs() < 1.0 && (0 <= h.cuboid || 0 <= h.cylinder || 0 <= h.sphere), "level {level} record {i}");
                    }
                    _ => {}
                }
            }
        }
        assert_eq!(census.into_iter().collect::<Vec<_>>(), CENSUS);
        assert_eq!(modes.into_iter().collect::<Vec<_>>(), (0..=11).collect::<Vec<i16>>());
    }
}
