//! The hero trace: one row per gameplay tick of Ratchet's state, as read from PCSX2 (`rc-trace record`) or
//! produced by the port (`rc-trace replay-hero`). Doc: docs/plan/hero_feel_pass.md.
//!
//! **Format** (UTF-8 text, tab separated, one line per tick):
//! ```text
//! # rc-trace hero trace v1
//! # meta <key>=<value>             (source, slot, game, level, camera and moby addresses, start time …)
//! # field <name>\t<address>\t<meaning>   (one per column, in column order)
//! <name>\t<name>\t…                 (the header: column names)
//! <value>\t<value>\t…               (one row per tick)
//! ```
//! Floats are written with Rust's shortest round-trip form (`{:?}`: `0.1`, `-0.0`, `1e-7`), so the parsed value is
//! bit-for-bit the recorded f32. Integers are decimal; byte blocks are lower-case hex (`-` = not available). The
//! reader matches columns by name: unknown columns are ignored and missing ones read as 0 / empty, so the
//! format can grow.
//!
//! **Tick alignment.** A row with `tick = N` is the state right after the gameplay tick that incremented the
//! counter 0x15f5cc to N, together with the pad input that tick consumed (the PAD record 0x13c940 is rebuilt by
//! `UpdatePad` at the start of the next main-loop iteration, after the recorder has read it).

use anyhow::{bail, Context, Result};
use std::fmt::Write as _;
use std::path::Path;

pub const MAGIC: &str = "# rc-trace hero trace v1";

/// The PAD record (level01 `0x13c940`, boot .bss: the same on every level).
pub const PAD: u32 = 0x13c940;

/// One column: name, where it comes from, what it is.
#[derive(Clone, Copy, Debug)]
pub struct Field {
    pub name: &'static str,
    pub addr: &'static str,
    pub desc: &'static str,
}

/// A cell value.
pub trait Cell: Sized {
    fn put(&self, out: &mut String);
    fn get(s: &str) -> Result<Self>;
    /// The value as a number for the diff (None: not numeric).
    fn num(&self) -> Option<f64>;
}

impl Cell for f32 {
    fn put(&self, out: &mut String) { let _ = write!(out, "{self:?}"); }
    fn get(s: &str) -> Result<Self> { Ok(s.parse()?) }
    fn num(&self) -> Option<f64> { Some(*self as f64) }
}

macro_rules! int_cell {
    ($($t:ty),*) => {$(
        impl Cell for $t {
            fn put(&self, out: &mut String) { let _ = write!(out, "{self}"); }
            fn get(s: &str) -> Result<Self> { Ok(s.parse()?) }
            fn num(&self) -> Option<f64> { Some(*self as f64) }
        }
    )*};
}
int_cell!(i32, u32, u64, i16, u8);

/// A byte block (hex; `-` when empty).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Hex(pub Vec<u8>);

impl Cell for Hex {
    fn put(&self, out: &mut String) {
        if self.0.is_empty() { out.push('-'); return; }
        for b in &self.0 { let _ = write!(out, "{b:02x}"); }
    }
    fn get(s: &str) -> Result<Self> {
        if s == "-" || s.is_empty() { return Ok(Hex(Vec::new())); }
        if !s.len().is_multiple_of(2) { bail!("odd hex length"); }
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).context("bad hex")).collect::<Result<Vec<u8>>>().map(Hex)
    }
    fn num(&self) -> Option<f64> { None }
}

macro_rules! sample {
    ($($name:ident : $ty:ty = $addr:literal, $desc:literal;)*) => {
        /// One tick of the hero (see the module doc; [`FIELDS`] documents each column).
        #[derive(Clone, Debug, Default, PartialEq)]
        pub struct Sample { $(pub $name: $ty,)* }

        /// The columns, in order.
        pub const FIELDS: &[Field] = &[$(Field { name: stringify!($name), addr: $addr, desc: $desc },)*];

        impl Sample {
            /// The row (tab separated, no newline).
            pub fn row(&self) -> String {
                let mut s = String::new();
                let mut first = true;
                $(
                    if !first { s.push('\t'); }
                    first = false;
                    Cell::put(&self.$name, &mut s);
                )*
                let _ = first;
                s
            }
            /// Sets column `name` from its text; false for an unknown column.
            pub fn set(&mut self, name: &str, v: &str) -> Result<bool> {
                match name {
                    $(stringify!($name) => { self.$name = <$ty as Cell>::get(v).with_context(|| format!("column {name}: {v:?}"))?; Ok(true) })*
                    _ => Ok(false),
                }
            }
            /// Column `name` as a number (None: unknown or not numeric).
            pub fn num(&self, name: &str) -> Option<f64> {
                match name {
                    $(stringify!($name) => Cell::num(&self.$name),)*
                    _ => None,
                }
            }
        }
    };
}

sample! {
    t_us: u64 = "host", "microseconds since the recording started (host clock, when the row was read)";
    tick: u32 = "0x15f5cc", "gameplay tick counter (after this tick's increment)";
    mode: i32 = "0x15f5c4", "game mode (0 gameplay, 2 in-engine cutscene)";
    level: i32 = "0x15ed84", "level (planet) number";
    mirror: u8 = "0x15edb4", "mirrored-controls option";
    pad: Hex = "PAD 0x13c940 +0x140/+0x1b0/+0x1c0", "the 18 libpad2 bytes this tick consumed, rebuilt from the PAD record (buttons active-low, rx ry lx ly, 12 pressures); decodes exactly as the game decoded them";
    lib_pad: Hex = "PAD +0x1c / +0x9c", "the newer of libpad2's two DMA buffers when read (diagnostic: may already hold the next frame's read)";
    pad_held: u32 = "0x13cae0", "PAD +0x1a0 held mask (after the input lock)";
    pad_pressed: u32 = "0x13cae4", "PAD +0x1a4 pressed mask (+0x10000 flick)";
    pad_held_u: u32 = "0x13cb00", "PAD +0x1c0 held mask before the lock / mirror";
    state: i32 = "0x1413d4", "hero state";
    substate: i32 = "0x1413d8", "substate";
    group: i32 = "0x1413dc", "movement group (0 stand, 1 walk, 2 fall, 4 jump, 5 glide, 0xc crouch …)";
    prev_state: i32 = "0x1413e0", "previous state";
    st_timer: i32 = "0x13f4e8", "ticks in the current state";
    pos_x: f32 = "0x13f3d0", "position x (feet)";
    pos_y: f32 = "0x13f3d4", "position y";
    pos_z: f32 = "0x13f3d8", "position z";
    yaw: f32 = "0x13f3e8", "yaw (rad)";
    vel_x: f32 = "0x13f430", "velocity x (u/tick)";
    vel_y: f32 = "0x13f434", "velocity y";
    vel_z: f32 = "0x13f438", "velocity z";
    disp_x: f32 = "0x13f450", "displacement this tick x (pos - old)";
    disp_y: f32 = "0x13f454", "displacement y";
    disp_z: f32 = "0x13f458", "displacement z";
    mom_x: f32 = "0x13f4a0", "carried momentum x";
    mom_y: f32 = "0x13f4a4", "carried momentum y";
    mom_z: f32 = "0x13f4a8", "carried momentum z";
    eff_len_xy: f32 = "0x13f4b4", "|effective velocity xy| (u/tick)";
    fwd_speed: f32 = "0x13f4b8", "forward speed (u/tick)";
    target_yaw: f32 = "0x13f4d0", "target yaw";
    target_speed: f32 = "0x13f4e0", "target speed (u/tick)";
    speed: f32 = "0x13f4e4", "current speed (u/tick)";
    ground_z: f32 = "0x13f628", "ground z under the hero";
    height: f32 = "0x13f62c", "height above ground";
    air_ticks: i16 = "0x13f65e", "air ticks (0 = grounded)";
    t_508: i32 = "0x13f508", "Thruster long-jump lockout";
    t_510: i32 = "0x13f510", "hit invulnerability";
    t_lockout: i32 = "0x13f514", "landing lockout (blocks jump / crouch / walk)";
    t_524: i32 = "0x13f524", "jump-buffer override after the glide (12 - T on landing)";
    t_542: i16 = "0x13f542", "jump lockout";
    t_70c: i16 = "0x13f70c", "landing-into-run blend";
    j_f744: f32 = "0x13f744", "pack-jump target speed";
    j_g_down: f32 = "0x13f748", "gravity once descending";
    j_takeoff_x: f32 = "0x13f750", "takeoff position x";
    j_takeoff_y: f32 = "0x13f754", "takeoff position y";
    j_takeoff_z: f32 = "0x13f758", "takeoff position z";
    j_ref_z: f32 = "0x13f760", "jump reference height";
    j_landed: i32 = "0x13f768", "landed counter (0 in the air)";
    j_curve_on: i16 = "0x13f76c", "scripted curve active";
    j_descending: i16 = "0x13f76e", "descending";
    j_takeoff: i32 = "0x13f770", "takeoff tick (windup)";
    j_air_speed: f32 = "0x13f774", "air stick speed scale";
    j_pending: f32 = "0x13f778", "pending vz";
    j_applied: f32 = "0x13f77c", "applied vz";
    j_h: f32 = "0x13f780", "height target h";
    j_land_eta: i32 = "0x13f79c", "landing ETA (ticks)";
    j_acc: f32 = "0x13f7d0", "windup accel";
    j_dec: f32 = "0x13f7d4", "windup decel";
    j_hmin: f32 = "0x13f7d8", "min jump height";
    j_hmax: f32 = "0x13f7dc", "max jump height";
    j_ramp: i16 = "0x13f7e8", "ticks to ramp min -> max";
    j_max_air: i16 = "0x13f7ea", "ticks before forcing the fall";
    j_g: f32 = "0x13f7f0", "gravity (u/tick^2)";
    jump_raw: Hex = "0x13f720..0x13f800", "the whole jump block (PCSX2 only)";
    anim_seq: u8 = "[0x1413d0]+0x53", "Ratchet's anim sequence (key B)";
    anim_frame: u8 = "[0x1413d0]+0x51", "anim frame index (key B)";
    anim_t: f32 = "[0x1413d0]+0x54", "anim blend / key fraction";
    anim_key: f32 = "0x13fdf8", "anim key time readout";
    anim_speed: f32 = "0x13fde0", "anim playback speed";
    stick_x: f32 = "0x141070", "hero stick x (d-pad fallback applied)";
    stick_y: f32 = "0x141074", "hero stick y";
    stick_mag: f32 = "0x1415ec", "stick magnitude (one tick late)";
    health: i32 = "0x1415f8", "health";
    owned: Hex = "0x13d4c0..+37", "item-owned table (id 2 = Heli-Pack, 3 = Thruster-Pack)";
    back_state: i32 = "0x1404f4", "back item slot state (2 = ready)";
    back_id: i32 = "0x1404f8", "back item id (2 Heli, 3 Thruster, 4 Hydro)";
    cam_x: f32 = "camera+0x00", "camera position x (level01 0x167240, level03 0x166ec0; see meta camera)";
    cam_y: f32 = "camera+0x04", "camera position y";
    cam_z: f32 = "camera+0x08", "camera position z";
    cam_roll: f32 = "camera+0x10", "camera roll";
    cam_pitch: f32 = "camera+0x14", "camera pitch";
    cam_yaw: f32 = "camera+0x18", "camera yaw (the hero reads it)";
    cam_fx: f32 = "camera+0x210", "camera forward row x (the hero reads the rows)";
    cam_fy: f32 = "camera+0x214", "camera forward row y";
    cam_fz: f32 = "camera+0x218", "camera forward row z";
    cam_lx: f32 = "camera+0x220", "camera left row x";
    cam_ly: f32 = "camera+0x224", "camera left row y";
    cam_lz: f32 = "camera+0x228", "camera left row z";
    cam_ux: f32 = "camera+0x230", "camera up row x";
    cam_uy: f32 = "camera+0x234", "camera up row y";
    cam_uz: f32 = "camera+0x238", "camera up row z";
}

/// Columns whose values are angles (the diff wraps their difference to ±π).
pub const ANGLES: &[&str] = &["yaw", "target_yaw", "cam_yaw", "cam_pitch", "cam_roll"];

/// A trace: metadata and rows.
#[derive(Clone, Debug, Default)]
pub struct Trace {
    pub meta: Vec<(String, String)>,
    pub samples: Vec<Sample>,
}

impl Trace {
    pub fn meta(&self, key: &str) -> Option<&str> { self.meta.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str()) }
    pub fn set_meta(&mut self, key: &str, v: impl Into<String>) {
        let v = v.into();
        match self.meta.iter_mut().find(|(k, _)| k == key) {
            Some(e) => e.1 = v,
            None => self.meta.push((key.into(), v)),
        }
    }

    /// The file header (magic, meta, field docs, column names), with a trailing newline.
    pub fn header(&self) -> String {
        let mut s = format!("{MAGIC}\n");
        for (k, v) in &self.meta { let _ = writeln!(s, "# meta {k}={v}"); }
        for f in FIELDS { let _ = writeln!(s, "# field {}\t{}\t{}", f.name, f.addr, f.desc); }
        s += &FIELDS.iter().map(|f| f.name).collect::<Vec<_>>().join("\t");
        s.push('\n');
        s
    }

    pub fn to_text(&self) -> String {
        let mut s = self.header();
        for r in &self.samples { s += &r.row(); s.push('\n'); }
        s
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        crate::write_output(path, self.to_text()).with_context(|| format!("writing {}", path.display()))
    }

    pub fn parse(text: &str) -> Result<Trace> {
        let mut lines = text.lines().enumerate();
        let mut t = Trace::default();
        let mut cols: Option<Vec<String>> = None;
        let mut saw_magic = false;
        for (n, line) in &mut lines {
            if line.is_empty() { continue; }
            if let Some(rest) = line.strip_prefix('#') {
                if line.starts_with(MAGIC) { saw_magic = true; }
                if let Some(kv) = rest.trim_start().strip_prefix("meta ") {
                    let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
                    t.meta.push((k.trim().into(), v.into()));
                }
                continue;
            }
            match &cols {
                None => {
                    if !saw_magic { bail!("not a hero trace (no {MAGIC:?} line before the header)"); }
                    cols = Some(line.split('\t').map(String::from).collect());
                }
                Some(c) => {
                    let mut s = Sample::default();
                    for (name, v) in c.iter().zip(line.split('\t')) {
                        s.set(name, v).with_context(|| format!("line {}", n + 1))?;
                    }
                    t.samples.push(s);
                }
            }
        }
        if cols.is_none() { bail!("no header line"); }
        Ok(t)
    }

    pub fn read(path: &Path) -> Result<Trace> {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        Trace::parse(&text).with_context(|| format!("parsing {}", path.display()))
    }
}

// ------------------------------------------------------------------------------------------------
// The pad bytes from the PAD record.

/// The libpad2 axis byte whose decode (`rc_game::pad::axis`: `|b − 127| < 48 → 0`, else `(d − 48) / 76` clamped
/// to 1, negative below 127) is `v`. Exact for every value the decode produces.
pub fn axis_byte(v: f32) -> u8 {
    if v == 0.0 { return 0x7f; }
    let d = 48 + (v.abs().min(1.0) * 76.0).round() as i32;
    (if v < 0.0 { 127 - d } else { 127 + d }).clamp(0, 255) as u8
}

/// `ProcessPadInput`'s LEFT / RIGHT swap of the mirror option.
fn mirror_lr(m: u32) -> u32 {
    if m & 0x8000 != 0 { m & !0x8000 | 0x2000 } else if m & 0x2000 != 0 { m & !0x2000 | 0x8000 } else { m }
}

/// The 18 libpad2 bytes that decode to the PAD record's values: `analog` = PAD +0x140..+0x17c (rx, ry, lx, ly and
/// the 12 pressures / 255, stored before the input lock and with the mirror undone), `held_u` = +0x1c0 (buttons |
/// stick-direction bits, unmirrored, before the lock), `raw` = +0x1b0 (raw buttons, mirrored, masked by lock 2).
/// Buttons other than the d-pad come from `held_u` (the stick bits only touch 0xf000); the d-pad from `raw` and,
/// where the lock cleared it, from its pressure bytes.
pub fn pad_bytes(analog: &[f32; 16], held_u: u32, raw: u32, mirror: bool) -> [u8; 18] {
    let raw = if mirror { mirror_lr(raw) } else { raw };
    let mut buttons = (held_u & 0x0fff) | (raw & 0xf000);
    for (k, bit) in [(0usize, 0x2000u32), (1, 0x8000), (2, 0x1000), (3, 0x4000)] {
        if analog[4 + k] > 0.0 { buttons |= bit; }
    }
    let b = !(buttons as u16);
    let mut o = [0u8; 18];
    o[0] = (b >> 8) as u8;
    o[1] = b as u8;
    for k in 0..4 { o[2 + k] = axis_byte(analog[k]); }
    for k in 0..12 { o[6 + k] = (analog[4 + k] * 255.0).round().clamp(0.0, 255.0) as u8; }
    o
}

// ------------------------------------------------------------------------------------------------
// Reading a sample out of EE memory.

/// EE memory a sample is read from: a PINE snapshot or a savestate image.
pub trait Mem {
    fn get(&self, addr: u32, len: usize) -> Option<&[u8]>;
}

impl Mem for crate::ee::EeImage {
    fn get(&self, addr: u32, len: usize) -> Option<&[u8]> { self.bytes(addr, len).ok() }
}

/// The ranges one PINE poll reads (8-byte aligned), besides Ratchet's moby and the camera.
pub const RANGES: &[(u32, u32)] = &[
    (0x15f5c0, 0x10),
    (0x15ed80, 0x38),
    (PAD + 0x18, 0x18),
    (PAD + 0x78, 0x08),
    (PAD + 0x98, 0x18),
    (PAD + 0xf8, 0x08),
    (PAD + 0x140, 0x40),
    (PAD + 0x1a0, 0x30),
    (0x13d4c0, 0x28),
    (0x13f3d0, 0x20),
    (0x13f430, 0x30),
    (0x13f4a0, 0x50),
    (0x13f500, 0x48),
    (0x13f620, 0x40),
    (0x13f708, 0x08),
    (0x13f720, 0xe0),
    (0x13fde0, 0x20),
    (0x1404f0, 0x10),
    (0x141070, 0x08),
    (0x1413d0, 0x18),
    (0x1415e8, 0x18),
];

/// Ratchet's moby pointer and the tick counter.
pub const MOBY_PTR: u32 = 0x1413d0;
pub const TICK: u32 = 0x15f5cc;
pub const LEVEL: u32 = 0x15ed84;

/// Known camera record addresses per level (the `Camera` record is in each level overlay's data).
pub const KNOWN_CAMERAS: &[(i32, u32)] = &[(1, 0x167240), (3, 0x166ec0)];

struct R<'a>(&'a dyn Mem);
impl R<'_> {
    fn b(&self, a: u32, n: usize) -> Result<&[u8]> { self.0.get(a, n).with_context(|| format!("EE {a:#x}+{n} not read")) }
    fn u32(&self, a: u32) -> Result<u32> { Ok(u32::from_le_bytes(self.b(a, 4)?.try_into().unwrap())) }
    fn i32(&self, a: u32) -> Result<i32> { Ok(self.u32(a)? as i32) }
    fn i16(&self, a: u32) -> Result<i16> { Ok(i16::from_le_bytes(self.b(a, 2)?.try_into().unwrap())) }
    fn u16(&self, a: u32) -> Result<u16> { Ok(u16::from_le_bytes(self.b(a, 2)?.try_into().unwrap())) }
    fn f(&self, a: u32) -> Result<f32> { Ok(f32::from_bits(self.u32(a)?)) }
}

/// The sample in `mem` (the fields of [`FIELDS`]), with Ratchet's moby at `moby` (0: skip the anim fields) and the
/// camera record at `camera` (None: camera fields 0).
pub fn sample_from(mem: &dyn Mem, moby: u32, camera: Option<u32>) -> Result<Sample> {
    let r = R(mem);
    let mut analog = [0f32; 16];
    for (k, v) in analog.iter_mut().enumerate() { *v = r.f(PAD + 0x140 + 4 * k as u32)?; }
    let mirror = r.b(0x15edb4, 1)?[0];
    let held_u = r.u32(PAD + 0x1c0)?;
    let raw = r.u32(PAD + 0x1b0)?;
    // libpad2's two DMA buffers (+0x00 / +0x80, data at +0x1c, frame counter at +0x7c): the newer one.
    let (c0, c1) = (r.u16(PAD + 0x7c)?, r.u16(PAD + 0xfc)?);
    let newer = if (c1.wrapping_sub(c0) as i16) > 0 { PAD + 0x9c } else { PAD + 0x1c };
    let mut s = Sample {
        tick: r.u32(TICK)?,
        mode: r.i32(0x15f5c4)?,
        level: r.i32(LEVEL)?,
        mirror,
        pad: Hex(pad_bytes(&analog, held_u, raw, mirror != 0).to_vec()),
        lib_pad: Hex(r.b(newer, 18)?.to_vec()),
        pad_held: r.u32(PAD + 0x1a0)?,
        pad_pressed: r.u32(PAD + 0x1a4)?,
        pad_held_u: held_u,
        state: r.i32(0x1413d4)?,
        substate: r.i32(0x1413d8)?,
        group: r.i32(0x1413dc)?,
        prev_state: r.i32(0x1413e0)?,
        st_timer: r.i32(0x13f4e8)?,
        pos_x: r.f(0x13f3d0)?,
        pos_y: r.f(0x13f3d4)?,
        pos_z: r.f(0x13f3d8)?,
        yaw: r.f(0x13f3e8)?,
        vel_x: r.f(0x13f430)?,
        vel_y: r.f(0x13f434)?,
        vel_z: r.f(0x13f438)?,
        disp_x: r.f(0x13f450)?,
        disp_y: r.f(0x13f454)?,
        disp_z: r.f(0x13f458)?,
        mom_x: r.f(0x13f4a0)?,
        mom_y: r.f(0x13f4a4)?,
        mom_z: r.f(0x13f4a8)?,
        eff_len_xy: r.f(0x13f4b4)?,
        fwd_speed: r.f(0x13f4b8)?,
        target_yaw: r.f(0x13f4d0)?,
        target_speed: r.f(0x13f4e0)?,
        speed: r.f(0x13f4e4)?,
        ground_z: r.f(0x13f628)?,
        height: r.f(0x13f62c)?,
        air_ticks: r.i16(0x13f65e)?,
        t_508: r.i32(0x13f508)?,
        t_510: r.i32(0x13f510)?,
        t_lockout: r.i32(0x13f514)?,
        t_524: r.i32(0x13f524)?,
        t_542: r.i16(0x13f542)?,
        t_70c: r.i16(0x13f70c)?,
        j_f744: r.f(0x13f744)?,
        j_g_down: r.f(0x13f748)?,
        j_takeoff_x: r.f(0x13f750)?,
        j_takeoff_y: r.f(0x13f754)?,
        j_takeoff_z: r.f(0x13f758)?,
        j_ref_z: r.f(0x13f760)?,
        j_landed: r.i32(0x13f768)?,
        j_curve_on: r.i16(0x13f76c)?,
        j_descending: r.i16(0x13f76e)?,
        j_takeoff: r.i32(0x13f770)?,
        j_air_speed: r.f(0x13f774)?,
        j_pending: r.f(0x13f778)?,
        j_applied: r.f(0x13f77c)?,
        j_h: r.f(0x13f780)?,
        j_land_eta: r.i32(0x13f79c)?,
        j_acc: r.f(0x13f7d0)?,
        j_dec: r.f(0x13f7d4)?,
        j_hmin: r.f(0x13f7d8)?,
        j_hmax: r.f(0x13f7dc)?,
        j_ramp: r.i16(0x13f7e8)?,
        j_max_air: r.i16(0x13f7ea)?,
        j_g: r.f(0x13f7f0)?,
        jump_raw: Hex(r.b(0x13f720, 0xe0)?.to_vec()),
        anim_key: r.f(0x13fdf8)?,
        anim_speed: r.f(0x13fde0)?,
        stick_x: r.f(0x141070)?,
        stick_y: r.f(0x141074)?,
        stick_mag: r.f(0x1415ec)?,
        health: r.i32(0x1415f8)?,
        owned: Hex(r.b(0x13d4c0, rc_formats::save_game::ITEM_COUNT)?.to_vec()),
        back_state: r.i32(0x1404f4)?,
        back_id: r.i32(0x1404f8)?,
        ..Default::default()
    };
    if moby != 0 {
        let a = r.b(moby + 0x50, 8)?;
        s.anim_frame = a[1];
        s.anim_seq = a[3];
        s.anim_t = f32::from_le_bytes(a[4..8].try_into().unwrap());
    }
    if let Some(c) = camera {
        s.cam_x = r.f(c)?;
        s.cam_y = r.f(c + 4)?;
        s.cam_z = r.f(c + 8)?;
        s.cam_roll = r.f(c + 0x10)?;
        s.cam_pitch = r.f(c + 0x14)?;
        s.cam_yaw = r.f(c + 0x18)?;
        let row = |o: u32| -> Result<[f32; 3]> { Ok([r.f(c + o)?, r.f(c + o + 4)?, r.f(c + o + 8)?]) };
        [s.cam_fx, s.cam_fy, s.cam_fz] = row(0x210)?;
        [s.cam_lx, s.cam_ly, s.cam_lz] = row(0x220)?;
        [s.cam_ux, s.cam_uy, s.cam_uz] = row(0x230)?;
    }
    Ok(s)
}

/// Whether the 0x240 bytes at `rec` look like the `Camera` record near `hero`: position and Euler with w = 0, unit
/// forward / left / up rows at +0x210 / +0x220 / +0x230, the Euler yaw = atan2(forward.y, forward.x), within 60 units.
pub fn is_camera_record(rec: &[u8], hero: [f32; 3]) -> bool {
    if rec.len() < 0x240 { return false; }
    let f = |o: usize| f32::from_le_bytes(rec[o..o + 4].try_into().unwrap());
    if f(0xc) != 0.0 || f(0x1c) != 0.0 { return false; }
    let len = |o: usize| (f(o).powi(2) + f(o + 4).powi(2) + f(o + 8).powi(2)).sqrt();
    if [0x210, 0x220, 0x230].iter().any(|&o| (len(o) - 1.0).abs().partial_cmp(&1e-3) != Some(std::cmp::Ordering::Less)) { return false; }
    let yaw = f(0x214).atan2(f(0x210));
    let mut d = (yaw - f(0x18)).abs();
    if d > std::f32::consts::PI { d = std::f32::consts::TAU - d; }
    if d.partial_cmp(&2e-3) != Some(std::cmp::Ordering::Less) { return false; }
    let dist = ((f(0) - hero[0]).powi(2) + (f(4) - hero[1]).powi(2) + (f(8) - hero[2]).powi(2)).sqrt();
    dist.is_finite() && dist < 60.0 && dist > 0.1
}

/// The camera record in `block` (EE bytes from `base`), 16-byte aligned: the first address that passes
/// [`is_camera_record`].
pub fn find_camera(block: &[u8], base: u32, hero: [f32; 3]) -> Option<u32> {
    (0..block.len().saturating_sub(0x240)).step_by(16).find(|&o| is_camera_record(&block[o..o + 0x240], hero)).map(|o| base + o as u32)
}

/// Where to scan for the camera record (the level overlays' data).
pub const CAMERA_SCAN: (u32, usize) = (0x160000, 0x10300);

/// The camera record in `mem` for `level`: the known address when it passes the check, else a scan.
pub fn locate_camera(mem: &dyn Mem, level: i32, hero: [f32; 3]) -> Option<u32> {
    for &(l, a) in KNOWN_CAMERAS {
        if l == level && mem.get(a, 0x240).is_some_and(|r| is_camera_record(r, hero)) { return Some(a); }
    }
    let (base, len) = CAMERA_SCAN;
    find_camera(mem.get(base, len)?, base, hero)
}

/// The hero position in `mem`.
pub fn hero_pos(mem: &dyn Mem) -> Option<[f32; 3]> {
    let b = mem.get(0x13f3d0, 12)?;
    Some([0, 4, 8].map(|o| f32::from_le_bytes(b[o..o + 4].try_into().unwrap())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rc_game::pad::{PadInput, PadState};

    fn sample(tick: u32) -> Sample {
        Sample { tick, state: 10, pos_x: 0.1, pos_y: -0.0, pos_z: 1e-7, yaw: f32::MIN_POSITIVE, pad: Hex(vec![0xff, 0xbf, 0x7f]), owned: Hex(vec![0, 1]), anim_seq: 0x12, air_ticks: -3, t_us: u64::MAX, ..Default::default() }
    }

    #[test]
    fn format_round_trip_is_bit_exact() {
        let mut t = Trace::default();
        t.set_meta("source", "test");
        t.set_meta("level", "3");
        t.samples = vec![sample(7), sample(8)];
        t.samples[1].pos_x = f32::from_bits(0x3dcc_cccd);
        t.samples[1].vel_z = f32::NAN;
        let text = t.to_text();
        let back = Trace::parse(&text).unwrap();
        assert_eq!(back.meta("level"), Some("3"));
        assert_eq!(back.samples.len(), 2);
        assert_eq!(back.samples[0], t.samples[0]);
        assert_eq!(back.samples[1].pos_x.to_bits(), 0x3dcc_cccd);
        assert!(back.samples[1].vel_z.is_nan());
        assert_eq!(back.samples[0].pos_y.to_bits(), (-0.0f32).to_bits());
        // Every field is documented, and the header names every column.
        let header = text.lines().find(|l| !l.starts_with('#')).unwrap();
        assert_eq!(header.split('\t').count(), FIELDS.len());
        assert_eq!(text.lines().filter(|l| l.starts_with("# field ")).count(), FIELDS.len());
    }

    #[test]
    fn format_tolerates_reordered_unknown_and_missing_columns() {
        let text = format!("{MAGIC}\n# meta a=b=c\nstate\tfuture_column\ttick\n3\tx\t10\n\n4\ty\t11\n");
        let t = Trace::parse(&text).unwrap();
        assert_eq!(t.meta("a"), Some("b=c"));
        assert_eq!(t.samples.iter().map(|s| (s.tick, s.state)).collect::<Vec<_>>(), vec![(10, 3), (11, 4)]);
        assert_eq!(t.samples[0].pos_x, 0.0);
        assert!(Trace::parse("tick\n1\n").is_err(), "no magic");
        assert!(Trace::parse(&format!("{MAGIC}\ntick\nzz\n")).is_err(), "bad number");
    }

    /// The PAD record written by the port's `ProcessPadInput` for a pad read → the rebuilt bytes decode the same.
    #[test]
    fn pad_bytes_rebuild_the_decode() {
        let inputs = [
            PadInput::neutral(),
            PadInput::neutral().stick(0.0, -1.0).press(rc_game::pad::button::CROSS | rc_game::pad::button::R1),
            PadInput::neutral().stick(0.37, 0.8).rstick(-0.5, 0.2).press(rc_game::pad::button::UP | rc_game::pad::button::START),
            PadInput { lx: 0, ly: 255, rx: 100, ry: 150, ..PadInput::neutral() }.press(rc_game::pad::button::LEFT | rc_game::pad::button::SQUARE),
        ];
        for mirror in [false, true] {
            for (lock, inp) in inputs.iter().enumerate().flat_map(|(i, p)| [(0, (i, p)), (2, (i, p))]) {
                let bytes = inp.1.bytes();
                let mut p = PadState { lock, ..Default::default() };
                p.update(Some(&bytes), mirror);
                let analog: [f32; 16] = std::array::from_fn(|k| p.analog_copy[k].to_f32());
                let rebuilt = pad_bytes(&analog, p.held_unmirrored, p.raw, mirror);
                let mut q = PadState::default();
                q.update(Some(&rebuilt), mirror);
                let mut r = PadState::default();
                r.update(Some(&bytes), mirror);
                assert_eq!((q.held, q.raw), (r.held, r.raw), "input {} lock {lock} mirror {mirror}", inp.0);
                assert_eq!((q.lx.to_f32(), q.ly.to_f32(), q.rx.to_f32(), q.ry.to_f32()), (r.lx.to_f32(), r.ly.to_f32(), r.rx.to_f32(), r.ry.to_f32()));
                assert_eq!(q.pressure.map(|v| v.to_f32()), r.pressure.map(|v| v.to_f32()));
            }
        }
        for b in 0..=255u8 {
            let v = rc_game::pad::axis(b).to_f32();
            assert_eq!(rc_game::pad::axis(axis_byte(v)).to_f32(), v, "byte {b}");
        }
    }

    #[test]
    fn camera_record_check() {
        let mut rec = vec![0u8; 0x240];
        let mut put = |o: usize, v: f32| rec[o..o + 4].copy_from_slice(&v.to_le_bytes());
        let yaw = -0.3f32;
        for (o, v) in [(0, 10.0), (4, 20.0), (8, 5.0), (0x18, yaw), (0x210, yaw.cos()), (0x214, yaw.sin()), (0x220, -yaw.sin()), (0x224, yaw.cos()), (0x238, 1.0)] { put(o, v); }
        assert!(is_camera_record(&rec, [14.0, 19.0, 4.0]));
        assert!(!is_camera_record(&rec, [140.0, 19.0, 4.0]), "too far");
        let mut block = vec![0u8; 0x1000];
        block[0x400..0x640].copy_from_slice(&rec);
        assert_eq!(find_camera(&block, 0x160000, [14.0, 19.0, 4.0]), Some(0x160400));
        rec[0x18..0x1c].copy_from_slice(&0.5f32.to_le_bytes());
        assert!(!is_camera_record(&rec, [14.0, 19.0, 4.0]), "yaw disagrees with the rows");
    }
}
