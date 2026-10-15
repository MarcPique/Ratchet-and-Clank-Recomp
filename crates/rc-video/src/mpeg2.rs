//! MPEG-2 video decoding (ISO/IEC 13818-2), scoped to what the game's movies use (docs/formats/pss.md §3, from a
//! header survey of all 84 files): Main Profile @ Main Level, 4:2:0, **frame pictures** only, I / P / B pictures,
//! `intra_dc_precision` 8 to 10 bits, both `q_scale_type`s, both intra VLC tables, loaded or default quantiser
//! matrices. 82 files are progressive (`frame_pred_frame_dct` = 1: frame DCT and frame prediction, zig-zag scan);
//! the two long extras (`mpegs[73]` / `[78]`) are interlaced and also use field prediction in frame pictures,
//! field DCT and the alternate scan. Anything else (field pictures, dual-prime prediction, 4:2:2, concealment
//! vectors, D pictures) is reported as [`DecodeError`] rather than decoded.
//!
//! [`Decoder::next_frame`] returns the frames in display order (I / P frames are held back until the next
//! reference picture, B frames come out at once). The output is bit-exact given the IDCT ([`crate::idct`], IEEE
//! 1180 accurate), and deterministic.

use crate::bits::{next_start_code, BitReader};
use crate::idct::idct;
use crate::vlc::{self, tables, DCT_EOB, DCT_ESCAPE, MBA_ESCAPE, MBA_STUFFING, MB_BACKWARD, MB_FORWARD, MB_INTRA, MB_PATTERN, MB_QUANT};
use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;

/// Zig-zag scan: coefficient `i` in transmission order is at raster position `ZIGZAG[i]`.
pub const ZIGZAG: [u8; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20, 13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57,
    50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// The default intra quantiser matrix, raster order.
pub const DEFAULT_INTRA: [u8; 64] = [
    8, 16, 19, 22, 26, 27, 29, 34, 16, 16, 22, 24, 27, 29, 34, 37, 19, 22, 26, 27, 29, 34, 34, 38, 22, 22, 26, 27, 29, 34, 37, 40, 22, 26, 27,
    29, 32, 35, 40, 48, 26, 27, 29, 32, 35, 40, 48, 58, 26, 27, 29, 34, 38, 46, 56, 69, 27, 29, 35, 38, 46, 56, 69, 83,
];

/// Alternate (vertical) scan.
pub const ALTERNATE: [u8; 64] = [
    0, 8, 16, 24, 1, 9, 2, 10, 17, 25, 32, 40, 48, 56, 57, 49, 41, 33, 26, 18, 3, 11, 4, 12, 19, 27, 34, 42, 50, 58, 35, 43, 51, 59, 20, 28,
    5, 13, 6, 14, 21, 29, 36, 44, 52, 60, 37, 45, 53, 61, 22, 30, 7, 15, 23, 31, 38, 46, 54, 62, 39, 47, 55, 63,
];

/// `quantiser_scale` for `q_scale_type` = 1 (Table 7-6); type 0 is `2·code`.
const NONLINEAR_Q: [u8; 32] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 12, 14, 16, 18, 20, 22, 24, 28, 32, 36, 40, 44, 48, 52, 56, 64, 72, 80, 88, 96, 104, 112];

/// An error with the byte offset in the elementary stream where it was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodeError {
    pub offset: usize,
    pub msg: String,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "mpeg2 at {:#x}: {}", self.offset, self.msg) }
}

impl std::error::Error for DecodeError {}

pub type Result<T> = std::result::Result<T, DecodeError>;

fn err<T>(offset: usize, msg: impl Into<String>) -> Result<T> { Err(DecodeError { offset, msg: msg.into() }) }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PictureType {
    I,
    P,
    B,
}

/// The sequence header and extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sequence {
    pub width: u32,
    pub height: u32,
    pub aspect_ratio: u8,
    pub frame_rate_code: u8,
    /// Frames per second as a fraction (`frame_rate_code` × the extension's factor).
    pub fps_num: u32,
    pub fps_den: u32,
    pub bit_rate: u32,
    pub profile_level: u8,
    pub progressive: bool,
    pub chroma_format: u8,
    pub load_intra: bool,
    pub load_non_intra: bool,
}

/// A decoded picture: 8-bit Y, Cb, Cr planes (4:2:0; strides = width, width / 2).
#[derive(Clone, Debug)]
pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub y: Vec<u8>,
    pub cb: Vec<u8>,
    pub cr: Vec<u8>,
    pub kind: PictureType,
    pub temporal_reference: u16,
    /// Decode order index.
    pub decode_index: usize,
}

impl Frame {
    fn new(width: usize, height: usize, kind: PictureType, temporal_reference: u16, decode_index: usize) -> Self {
        Frame { width, height, y: vec![0; width * height], cb: vec![128; width * height / 4], cr: vec![128; width * height / 4], kind, temporal_reference, decode_index }
    }
}

/// What the stream used (reports and the header survey).
#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub sequences: usize,
    pub gops: usize,
    pub closed_gops: usize,
    /// Pictures decoded by type (I, P, B).
    pub pictures: [usize; 3],
    /// B pictures without both reference frames (leading B pictures of an open GOP at the stream start): skipped.
    pub skipped_b: usize,
    pub frames_out: usize,
    /// Pictures by `intra_dc_precision` (0..=3), `q_scale_type`, `intra_vlc_format`, `top_field_first`,
    /// `frame_pred_frame_dct`, `alternate_scan`, `progressive_frame`.
    pub dc_precision: [usize; 4],
    pub q_scale_type: [usize; 2],
    pub intra_vlc_format: [usize; 2],
    pub top_field_first: [usize; 2],
    pub frame_pred_frame_dct: [usize; 2],
    pub alternate_scan: [usize; 2],
    pub progressive_frame: [usize; 2],
    /// Macroblocks with field prediction / field DCT (interlaced pictures).
    pub field_motion_mbs: usize,
    pub field_dct_mbs: usize,
    pub macroblocks: usize,
    pub skipped_macroblocks: usize,
    pub coded_blocks: usize,
}

/// The picture coding parameters of the current picture.
#[derive(Clone, Copy, Debug)]
struct PicParams {
    kind: PictureType,
    /// `f_code[s][t]` (s: 0 forward, 1 backward; t: 0 horizontal, 1 vertical).
    f_code: [[u8; 2]; 2],
    dc_precision: u8,
    q_scale_type: bool,
    intra_vlc_format: bool,
    frame_pred_frame_dct: bool,
    /// The coefficient scan (zig-zag or alternate).
    scan: &'static [u8; 64],
}

/// A macroblock's motion: frame prediction (`mv[0][s]`) or field prediction in a frame picture (`mv[r][s]` for the
/// top (r = 0) and bottom (r = 1) field lines, from reference field `sel[r][s]`; vertical components in field
/// lines). Vectors in half samples; s = 0 forward, 1 backward.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Motion {
    field: bool,
    mv: [[[i32; 2]; 2]; 2],
    sel: [[bool; 2]; 2],
}

/// Per-slice decoding state.
struct SliceState {
    qscale: i32,
    dc_pred: [i32; 3],
    /// Motion vector predictors `PMV[r][s][t]`.
    pmv: [[[i32; 2]; 2]; 2],
    /// Motion flags of the previous macroblock (B pictures: a skipped macroblock repeats its directions).
    prev_flags: i16,
}

/// The MPEG-2 decoder over one elementary stream.
pub struct Decoder {
    data: Arc<[u8]>,
    pos: usize,
    seq: Option<Sequence>,
    intra_q: [u8; 64],
    non_intra_q: [u8; 64],
    /// [past, most recent] reference frames.
    refs: [Option<Arc<Frame>>; 2],
    /// The most recent I / P frame, output when the next reference picture arrives (or at the end).
    held: Option<Arc<Frame>>,
    out: VecDeque<Arc<Frame>>,
    ended: bool,
    pub stats: Stats,
}

impl Decoder {
    pub fn new(es: impl Into<Arc<[u8]>>) -> Self {
        Decoder { data: es.into(), pos: 0, seq: None, intra_q: DEFAULT_INTRA, non_intra_q: [16; 64], refs: [None, None], held: None, out: VecDeque::new(), ended: false, stats: Stats::default() }
    }

    /// The first sequence header (parsed on demand).
    pub fn sequence(&mut self) -> Result<Option<Sequence>> {
        while self.seq.is_none() && !self.ended {
            if !self.step()? { break; }
        }
        // The sequence extension and user data that follow the header.
        while self.seq.is_some() {
            let Some(sc) = next_start_code(&self.data, self.pos) else { break };
            if !matches!(self.data[sc + 3], 0xb5 | 0xb2) { break; }
            self.step()?;
        }
        Ok(self.seq)
    }

    /// The next frame in display order; None at the end of the stream.
    pub fn next_frame(&mut self) -> Result<Option<Arc<Frame>>> {
        loop {
            if let Some(f) = self.out.pop_front() {
                self.stats.frames_out += 1;
                return Ok(Some(f));
            }
            if self.ended { return Ok(None); }
            self.step()?;
        }
    }

    /// Handles the next start code; false at the end of the stream.
    fn step(&mut self) -> Result<bool> {
        let data = Arc::clone(&self.data);
        let Some(sc) = next_start_code(&data, self.pos) else {
            if let Some(h) = self.held.take() { self.out.push_back(h); }
            self.ended = true;
            return Ok(false);
        };
        let code = data[sc + 3];
        self.pos = sc + 4;
        match code {
            0xb3 => self.sequence_header(&data, sc)?,
            0xb5 => self.extension(&data, sc)?,
            0xb8 => {
                let mut r = BitReader::new(&data, sc + 4);
                r.skip(25);
                self.stats.gops += 1;
                if r.bit() { self.stats.closed_gops += 1; }
            }
            0x00 => self.pos = self.picture(&data, sc)?,
            0xb7 => {
                // sequence_end_code: the held reference frame is displayed.
                if let Some(h) = self.held.take() { self.out.push_back(h); }
            }
            _ => {}
        }
        Ok(true)
    }

    fn sequence_header(&mut self, data: &[u8], sc: usize) -> Result<()> {
        let mut r = BitReader::new(data, sc + 4);
        let width = r.read(12);
        let height = r.read(12);
        let aspect_ratio = r.read(4) as u8;
        let frame_rate_code = r.read(4) as u8;
        let bit_rate = r.read(18);
        r.skip(1 + 10 + 1);
        let load_intra = r.bit();
        self.intra_q = DEFAULT_INTRA;
        if load_intra { for &z in &ZIGZAG { self.intra_q[z as usize] = r.read(8) as u8; } }
        let load_non_intra = r.bit();
        self.non_intra_q = [16; 64];
        if load_non_intra { for &z in &ZIGZAG { self.non_intra_q[z as usize] = r.read(8) as u8; } }
        if r.overrun() { return err(sc, "truncated sequence header"); }
        let (fps_num, fps_den) = match frame_rate_code {
            1 => (24000, 1001),
            2 => (24, 1),
            3 => (25, 1),
            4 => (30000, 1001),
            5 => (30, 1),
            6 => (50, 1),
            7 => (60000, 1001),
            8 => (60, 1),
            c => return err(sc, format!("frame_rate_code {c}")),
        };
        if width == 0 || height == 0 || !width.is_multiple_of(16) || !height.is_multiple_of(16) || width > 1920 || height > 1152 {
            return err(sc, format!("unsupported picture size {width}×{height}"));
        }
        if let Some(s) = &self.seq {
            if (s.width, s.height) != (width, height) { return err(sc, format!("picture size changes from {}×{} to {width}×{height}", s.width, s.height)); }
        }
        let prev = self.seq;
        self.seq = Some(Sequence {
            width,
            height,
            aspect_ratio,
            frame_rate_code,
            fps_num,
            fps_den,
            bit_rate,
            profile_level: prev.map_or(0, |s| s.profile_level),
            progressive: prev.is_some_and(|s| s.progressive),
            chroma_format: prev.map_or(0, |s| s.chroma_format),
            load_intra,
            load_non_intra,
        });
        self.stats.sequences += 1;
        Ok(())
    }

    /// Extensions outside a picture: the sequence extension (others are ignored).
    fn extension(&mut self, data: &[u8], sc: usize) -> Result<()> {
        let mut r = BitReader::new(data, sc + 4);
        if r.read(4) != 1 { return Ok(()); }
        let Some(seq) = self.seq.as_mut() else { return err(sc, "sequence extension before a sequence header") };
        seq.profile_level = r.read(8) as u8;
        seq.progressive = r.bit();
        seq.chroma_format = r.read(2) as u8;
        let (hx, vx) = (r.read(2), r.read(2));
        r.skip(12 + 1 + 8 + 1);
        let (n, d) = (r.read(2), r.read(5));
        if hx != 0 || vx != 0 { return err(sc, "size extension bits are set"); }
        if seq.chroma_format != 1 { return err(sc, format!("chroma_format {} (only 4:2:0 is supported)", seq.chroma_format)); }
        seq.fps_num *= n + 1;
        seq.fps_den *= d + 1;
        Ok(())
    }

    /// Decodes the picture whose header starts at `sc`; returns the offset of the start code after its slices.
    fn picture(&mut self, data: &[u8], sc: usize) -> Result<usize> {
        let Some(seq) = self.seq else { return err(sc, "picture before a sequence header") };
        let mut r = BitReader::new(data, sc + 4);
        let temporal_reference = r.read(10) as u16;
        let kind = match r.read(3) {
            1 => PictureType::I,
            2 => PictureType::P,
            3 => PictureType::B,
            t => return err(sc, format!("picture_coding_type {t}")),
        };
        // The MPEG-1 fields (vbv_delay, full_pel_*, f_codes) are superseded by the picture coding extension.
        let mut params = PicParams { kind, f_code: [[15; 2]; 2], dc_precision: 0, q_scale_type: false, intra_vlc_format: false, frame_pred_frame_dct: true, scan: &ZIGZAG };
        let mut have_ext = false;
        let mut pos = sc + 4;
        // Extensions and user data up to the first slice.
        let first_slice = loop {
            let Some(s) = next_start_code(data, pos) else { return err(sc, "picture without slices") };
            let code = data[s + 3];
            pos = s + 4;
            match code {
                0xb5 => {
                    let mut e = BitReader::new(data, s + 4);
                    match e.read(4) {
                        8 => {
                            for fs in &mut params.f_code { for ft in fs.iter_mut() { *ft = e.read(4) as u8; } }
                            params.dc_precision = e.read(2) as u8;
                            let structure = e.read(2);
                            let tff = e.bit();
                            let fpfd = e.bit();
                            let concealment = e.bit();
                            params.q_scale_type = e.bit();
                            params.intra_vlc_format = e.bit();
                            let alternate_scan = e.bit();
                            let _repeat_first_field = e.bit();
                            let _chroma_420_type = e.bit();
                            let progressive_frame = e.bit();
                            if structure != 3 { return err(s, format!("picture_structure {structure} (only frame pictures are supported)")); }
                            if concealment { return err(s, "concealment motion vectors not supported"); }
                            if params.dc_precision == 3 { return err(s, "intra_dc_precision 11 bits not allowed at Main Profile"); }
                            params.frame_pred_frame_dct = fpfd;
                            params.scan = if alternate_scan { &ALTERNATE } else { &ZIGZAG };
                            self.stats.dc_precision[params.dc_precision as usize] += 1;
                            self.stats.q_scale_type[params.q_scale_type as usize] += 1;
                            self.stats.intra_vlc_format[params.intra_vlc_format as usize] += 1;
                            self.stats.top_field_first[tff as usize] += 1;
                            self.stats.frame_pred_frame_dct[fpfd as usize] += 1;
                            self.stats.alternate_scan[alternate_scan as usize] += 1;
                            self.stats.progressive_frame[progressive_frame as usize] += 1;
                            have_ext = true;
                        }
                        3 => {
                            // quant_matrix_extension (luma matrices; the chroma ones are for 4:2:2 / 4:4:4).
                            if e.bit() { for &z in &ZIGZAG { self.intra_q[z as usize] = e.read(8) as u8; } }
                            if e.bit() { for &z in &ZIGZAG { self.non_intra_q[z as usize] = e.read(8) as u8; } }
                        }
                        _ => {}
                    }
                }
                0x01..=0xaf => break s,
                0xb2 => {}
                c => return err(s, format!("start code {c:#x} before the picture's first slice")),
            }
        };
        if !have_ext { return err(sc, "MPEG-1 picture (no picture coding extension)"); }
        let decode_index = self.stats.pictures.iter().sum::<usize>() + self.stats.skipped_b;
        let (w, h) = (seq.width as usize, seq.height as usize);
        let fwd = match kind {
            PictureType::I => None,
            PictureType::P => match &self.refs[1] {
                Some(f) => Some(Arc::clone(f)),
                None => return err(sc, "P picture without a reference frame"),
            },
            PictureType::B => self.refs[0].clone(),
        };
        let bwd = if kind == PictureType::B { self.refs[1].clone() } else { None };
        // Find the end of the slices either way (a skipped B picture still has to be stepped over).
        let mut frame = Frame::new(w, h, kind, temporal_reference, decode_index);
        let mut s = first_slice;
        let skip = kind == PictureType::B && (fwd.is_none() || bwd.is_none());
        let end = loop {
            let code = data[s + 3];
            if !(0x01..=0xaf).contains(&code) { break s; }
            let next = next_start_code(data, s + 4).unwrap_or(data.len());
            if !skip {
                let row = code as usize - 1;
                if row >= h / 16 { return err(s, format!("slice row {row} outside the picture")); }
                let slice = &data[..next];
                decode_slice(&mut frame, &params, &self.intra_q, &self.non_intra_q, fwd.as_deref(), bwd.as_deref(), slice, s, row, &mut self.stats)?;
            }
            if next >= data.len() { break data.len(); }
            s = next;
        };
        if skip {
            self.stats.skipped_b += 1;
            return Ok(end);
        }
        self.stats.pictures[kind as usize] += 1;
        let frame = Arc::new(frame);
        match kind {
            PictureType::B => self.out.push_back(frame),
            _ => {
                if let Some(h) = self.held.replace(Arc::clone(&frame)) { self.out.push_back(h); }
                self.refs[0] = self.refs[1].take();
                self.refs[1] = Some(frame);
            }
        }
        Ok(end)
    }
}

/// One slice: `data[..]` ends at the next start code, the slice's start code is at `sc`.
#[allow(clippy::too_many_arguments)]
fn decode_slice(
    frame: &mut Frame,
    p: &PicParams,
    intra_q: &[u8; 64],
    non_intra_q: &[u8; 64],
    fwd: Option<&Frame>,
    bwd: Option<&Frame>,
    data: &[u8],
    sc: usize,
    row: usize,
    stats: &mut Stats,
) -> Result<()> {
    let t = tables();
    let mb_w = frame.width / 16;
    let mut r = BitReader::new(data, sc + 4);
    let code = r.read(5) as i32;
    if code == 0 { return err(sc, "quantiser_scale_code 0"); }
    if r.bit() {
        r.skip(1 + 7);
        while r.bit() { r.skip(8); }
    }
    let dc_reset = 1 << (7 + p.dc_precision);
    let mut st = SliceState { qscale: qscale(p, code), dc_pred: [dc_reset; 3], pmv: [[[0; 2]; 2]; 2], prev_flags: 0 };
    let mut addr = (row * mb_w) as isize - 1;
    let mut first = true;
    let count = (mb_w * frame.height / 16) as isize;
    loop {
        let at = r.byte_pos();
        let mut inc = 0isize;
        loop {
            match t.mba.decode(&mut r) {
                Some(MBA_ESCAPE) => inc += 33,
                Some(MBA_STUFFING) => {}
                Some(v) => {
                    inc += v as isize;
                    break;
                }
                None => return err(at, "bad macroblock_address_increment"),
            }
        }
        if !first {
            for k in 1..inc {
                let a = addr + k;
                if a >= count { return err(at, "skipped macroblocks past the picture"); }
                skipped_macroblock(frame, p, fwd, bwd, &mut st, a as usize, at)?;
                stats.skipped_macroblocks += 1;
            }
        }
        first = false;
        addr += inc;
        if addr >= count { return err(at, format!("macroblock address {addr} past the picture")); }
        macroblock(frame, p, intra_q, non_intra_q, fwd, bwd, &mut st, &mut r, addr as usize, stats)?;
        stats.macroblocks += 1;
        if r.overrun() { return err(r.byte_pos(), "slice data runs past the next start code"); }
        if r.at_start_code() { break; }
    }
    Ok(())
}

fn qscale(p: &PicParams, code: i32) -> i32 { if p.q_scale_type { NONLINEAR_Q[code as usize] as i32 } else { 2 * code } }

/// A skipped macroblock: P = forward frame prediction with a zero vector (vector predictors reset); B = the previous
/// macroblock's directions with frame prediction from the vector predictors `PMV[0][s]` (7.6.6.4: also after a
/// field-predicted macroblock). The DC predictors are reset.
fn skipped_macroblock(frame: &mut Frame, p: &PicParams, fwd: Option<&Frame>, bwd: Option<&Frame>, st: &mut SliceState, addr: usize, at: usize) -> Result<()> {
    let dc_reset = 1 << (7 + p.dc_precision);
    st.dc_pred = [dc_reset; 3];
    let mut pred = MbPred::default();
    match p.kind {
        PictureType::I => return err(at, "skipped macroblock in an I picture"),
        PictureType::P => {
            st.pmv = [[[0; 2]; 2]; 2];
            predict_mb(&mut pred, frame, fwd, None, addr, &Motion::default());
        }
        PictureType::B => {
            let f = st.prev_flags;
            if f & (MB_FORWARD | MB_BACKWARD) == 0 { return err(at, "skipped B macroblock after an intra macroblock"); }
            // Frame prediction with the vector predictors (frame units), whatever the previous macroblock used.
            let m = Motion { mv: [[st.pmv[0][0], st.pmv[0][1]], [[0; 2]; 2]], ..Motion::default() };
            predict_mb(&mut pred, frame, if f & MB_FORWARD != 0 { fwd } else { None }, if f & MB_BACKWARD != 0 { bwd } else { None }, addr, &m);
        }
    }
    store_mb(frame, addr, &pred);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn macroblock(
    frame: &mut Frame,
    p: &PicParams,
    intra_q: &[u8; 64],
    non_intra_q: &[u8; 64],
    fwd: Option<&Frame>,
    bwd: Option<&Frame>,
    st: &mut SliceState,
    r: &mut BitReader,
    addr: usize,
    stats: &mut Stats,
) -> Result<()> {
    let t = tables();
    let at = r.byte_pos();
    let flags = t.mb_type[p.kind as usize].decode(r).ok_or_else(|| DecodeError { offset: at, msg: format!("bad macroblock_type in a {:?} picture", p.kind) })?;
    // macroblock_modes(): frame_motion_type and dct_type exist only when frame_pred_frame_dct = 0.
    let mut field_motion = false;
    if flags & (MB_FORWARD | MB_BACKWARD) != 0 && !p.frame_pred_frame_dct {
        match r.read(2) {
            1 => field_motion = true,
            2 => {}
            3 => return err(at, "dual-prime prediction not supported"),
            _ => return err(at, "reserved frame_motion_type 0"),
        }
    }
    let field_dct = flags & (MB_INTRA | MB_PATTERN) != 0 && !p.frame_pred_frame_dct && r.bit();
    if field_motion { stats.field_motion_mbs += 1; }
    if field_dct { stats.field_dct_mbs += 1; }
    if flags & MB_QUANT != 0 {
        let code = r.read(5) as i32;
        if code == 0 { return err(at, "quantiser_scale_code 0"); }
        st.qscale = qscale(p, code);
    }
    let dc_reset = 1 << (7 + p.dc_precision);
    let mut pred = MbPred::default();
    if flags & MB_INTRA != 0 {
        st.pmv = [[[0; 2]; 2]; 2];
        st.prev_flags = MB_INTRA;
        for b in 0..6 {
            let mut blk = [0i32; 64];
            intra_block(r, p, intra_q, st, b, &mut blk)?;
            idct(&mut blk);
            stats.coded_blocks += 1;
            pred.put_block(b, &blk, field_dct, true);
        }
        store_mb(frame, addr, &pred);
        return Ok(());
    }
    st.dc_pred = [dc_reset; 3];
    let mut motion = Motion { field: field_motion, ..Motion::default() };
    for s in 0..2 {
        if flags & [MB_FORWARD, MB_BACKWARD][s] == 0 { continue; }
        if field_motion {
            for fr in 0..2 {
                motion.sel[fr][s] = r.bit();
                motion.mv[fr][s] = motion_vector(r, p.f_code[s], &mut st.pmv[fr][s], true, at)?;
            }
        } else {
            motion.mv[0][s] = motion_vector(r, p.f_code[s], &mut st.pmv[0][s], false, at)?;
            st.pmv[1][s] = st.pmv[0][s];
        }
    }
    // P picture, "No MC": forward frame prediction with a zero vector, predictors reset.
    let (use_f, use_b) = match p.kind {
        PictureType::P => {
            if flags & MB_FORWARD == 0 {
                st.pmv = [[[0; 2]; 2]; 2];
                motion = Motion::default();
            }
            (true, false)
        }
        _ => (flags & MB_FORWARD != 0, flags & MB_BACKWARD != 0),
    };
    st.prev_flags = flags;
    let cbp = if flags & MB_PATTERN != 0 {
        let v = t.cbp.decode(r).ok_or_else(|| DecodeError { offset: at, msg: "bad coded_block_pattern".into() })?;
        if v == 0 { return err(at, "coded_block_pattern 0 (4:2:2 code) in 4:2:0"); }
        v as u32
    } else {
        0
    };
    predict_mb(&mut pred, frame, if use_f { fwd } else { None }, if use_b { bwd } else { None }, addr, &motion);
    for b in 0..6 {
        if cbp & (32 >> b) == 0 { continue; }
        let mut blk = [0i32; 64];
        non_intra_block(r, p.scan, non_intra_q, st.qscale, &mut blk, at)?;
        idct(&mut blk);
        stats.coded_blocks += 1;
        pred.put_block(b, &blk, field_dct, false);
    }
    store_mb(frame, addr, &pred);
    Ok(())
}

/// A macroblock's samples: luma 16×16, Cb and Cr 8×8, raster order.
struct MbPred {
    y: [u8; 256],
    cb: [u8; 64],
    cr: [u8; 64],
}

impl Default for MbPred {
    fn default() -> Self { MbPred { y: [0; 256], cb: [0; 64], cr: [0; 64] } }
}

impl MbPred {
    /// Puts block `b`'s IDCT output: intra = the samples, else added to the prediction; clamped to 0..=255. Luma
    /// blocks 0..3 cover the quadrants (frame DCT) or, with field DCT, the left / right halves of the top-field
    /// lines (blocks 0, 1) and of the bottom-field lines (2, 3).
    #[inline]
    fn put_block(&mut self, b: usize, blk: &[i32; 64], field_dct: bool, intra: bool) {
        let (plane, x0, y0, step, stride): (&mut [u8], usize, usize, usize, usize) = match b {
            0..=3 if field_dct => (&mut self.y, (b & 1) * 8, b >> 1, 2, 16),
            0..=3 => (&mut self.y, (b & 1) * 8, (b >> 1) * 8, 1, 16),
            4 => (&mut self.cb, 0, 0, 1, 8),
            _ => (&mut self.cr, 0, 0, 1, 8),
        };
        for i in 0..8 {
            let o = (y0 + i * step) * stride + x0;
            let row = &mut plane[o..o + 8];
            let src = &blk[i * 8..i * 8 + 8];
            if intra {
                for (d, &v) in row.iter_mut().zip(src) { *d = v.clamp(0, 255) as u8; }
            } else {
                for (d, &v) in row.iter_mut().zip(src) { *d = (*d as i32 + v).clamp(0, 255) as u8; }
            }
        }
    }
}

/// Writes a macroblock into the frame.
fn store_mb(frame: &mut Frame, addr: usize, pred: &MbPred) {
    let mb_w = frame.width / 16;
    let (mx, my) = (addr % mb_w, addr / mb_w);
    let w = frame.width;
    for y in 0..16 {
        let d = (my * 16 + y) * w + mx * 16;
        frame.y[d..d + 16].copy_from_slice(&pred.y[y * 16..y * 16 + 16]);
    }
    let cw = w / 2;
    for (src, plane) in [(&pred.cb, &mut frame.cb), (&pred.cr, &mut frame.cr)] {
        for y in 0..8 {
            let d = (my * 8 + y) * cw + mx * 8;
            plane[d..d + 8].copy_from_slice(&src[y * 8..y * 8 + 8]);
        }
    }
}

/// The prediction of one direction `s` from `src` into `out`.
fn predict_dir(out: &mut MbPred, src: &Frame, m: &Motion, s: usize, mx: isize, my: isize) {
    let (w, h, cw, ch) = (src.width, src.height, src.width / 2, src.height / 2);
    if !m.field {
        let mv = m.mv[0][s];
        predict(&mut out.y, 16, 16, 16, &src.y, 0, w, w, h, mx * 16, my * 16, mv[0], mv[1]);
        let (cx, cy) = (mv[0] / 2, mv[1] / 2);
        predict(&mut out.cb, 8, 8, 8, &src.cb, 0, cw, cw, ch, mx * 8, my * 8, cx, cy);
        predict(&mut out.cr, 8, 8, 8, &src.cr, 0, cw, cw, ch, mx * 8, my * 8, cx, cy);
        return;
    }
    // Field prediction in a frame picture: field r's lines (r, r + 2, …) from reference field sel[r][s].
    for fr in 0..2 {
        let mv = m.mv[fr][s];
        let sel = m.sel[fr][s] as usize;
        predict(&mut out.y[fr * 16..], 32, 16, 8, &src.y, sel * w, 2 * w, w, h / 2, mx * 16, my * 8, mv[0], mv[1]);
        let (cx, cy) = (mv[0] / 2, mv[1] / 2);
        predict(&mut out.cb[fr * 8..], 16, 8, 4, &src.cb, sel * cw, 2 * cw, cw, ch / 2, mx * 8, my * 4, cx, cy);
        predict(&mut out.cr[fr * 8..], 16, 8, 4, &src.cr, sel * cw, 2 * cw, cw, ch / 2, mx * 8, my * 4, cx, cy);
    }
}

/// Forms the prediction of a macroblock: forward only, backward only, or the rounded average of both. Chroma
/// vectors are the luma vectors halved toward zero (4:2:0).
fn predict_mb(pred: &mut MbPred, frame: &Frame, fwd: Option<&Frame>, bwd: Option<&Frame>, addr: usize, m: &Motion) {
    let mb_w = frame.width / 16;
    let (mx, my) = ((addr % mb_w) as isize, (addr / mb_w) as isize);
    match (fwd, bwd) {
        (Some(f), Some(b)) => {
            let mut pb = MbPred::default();
            predict_dir(pred, f, m, 0, mx, my);
            predict_dir(&mut pb, b, m, 1, mx, my);
            let avg = |d: &mut [u8], s: &[u8]| for (x, &y) in d.iter_mut().zip(s) { *x = ((*x as u16 + y as u16 + 1) >> 1) as u8 };
            avg(&mut pred.y, &pb.y);
            avg(&mut pred.cb, &pb.cb);
            avg(&mut pred.cr, &pb.cr);
        }
        (Some(f), None) => predict_dir(pred, f, m, 0, mx, my),
        (None, Some(b)) => predict_dir(pred, b, m, 1, mx, my),
        // A reference is missing (cannot happen for a decodable picture): mid grey.
        (None, None) => *pred = MbPred { y: [128; 256], cb: [128; 64], cr: [128; 64] },
    }
}

/// Half-sample prediction of a `w`×`h` block into `dst` (row stride `dst_stride`) whose top-left sample is at
/// (x, y) of a `pw`×`ph` plane (row k at `src[off + k·stride]`), displaced by (mvx, mvy) half samples. Samples
/// outside the plane (never referenced by a valid stream) are clamped to its edge.
#[allow(clippy::too_many_arguments)]
fn predict(dst: &mut [u8], dst_stride: usize, w: usize, h: usize, src: &[u8], off: usize, stride: usize, pw: usize, ph: usize, x: isize, y: isize, mvx: i32, mvy: i32) {
    let ix = x + (mvx >> 1) as isize;
    let iy = y + (mvy >> 1) as isize;
    let (hx, hy) = ((mvx & 1) as usize, (mvy & 1) as usize);
    let inside = ix >= 0 && iy >= 0 && ix as usize + w + hx <= pw && iy as usize + h + hy <= ph;
    if inside {
        let (ix, iy) = (ix as usize, iy as usize);
        for r in 0..h {
            let o = off + (iy + r) * stride + ix;
            let row = &src[o..o + w + hx];
            let d = &mut dst[r * dst_stride..r * dst_stride + w];
            match (hx, hy) {
                (0, 0) => d.copy_from_slice(&row[..w]),
                (1, 0) => for (k, v) in d.iter_mut().enumerate() { *v = ((row[k] as u16 + row[k + 1] as u16 + 1) >> 1) as u8 },
                (0, 1) => {
                    let below = &src[o + stride..o + stride + w];
                    for (k, v) in d.iter_mut().enumerate() { *v = ((row[k] as u16 + below[k] as u16 + 1) >> 1) as u8 }
                }
                _ => {
                    let below = &src[o + stride..o + stride + w + 1];
                    for (k, v) in d.iter_mut().enumerate() {
                        *v = ((row[k] as u16 + row[k + 1] as u16 + below[k] as u16 + below[k + 1] as u16 + 2) >> 2) as u8
                    }
                }
            }
        }
        return;
    }
    let at = |xx: isize, yy: isize| -> u16 { src[off + yy.clamp(0, ph as isize - 1) as usize * stride + xx.clamp(0, pw as isize - 1) as usize] as u16 };
    for r in 0..h {
        for k in 0..w {
            let (sx, sy) = (ix + k as isize, iy + r as isize);
            dst[r * dst_stride + k] = match (hx, hy) {
                (0, 0) => at(sx, sy),
                (1, 0) => (at(sx, sy) + at(sx + 1, sy) + 1) >> 1,
                (0, 1) => (at(sx, sy) + at(sx, sy + 1) + 1) >> 1,
                _ => (at(sx, sy) + at(sx + 1, sy) + at(sx, sy + 1) + at(sx + 1, sy + 1) + 2) >> 2,
            } as u8;
        }
    }
}

/// One `motion_vector(r, s)`: horizontal then vertical component, each predicted from and stored to `pmv`
/// (7.6.3.1). A field vector in a frame picture (`field`) predicts its vertical component from `PMV >> 1` and
/// stores it back doubled (the predictors stay in frame units).
fn motion_vector(r: &mut BitReader, f_code: [u8; 2], pmv: &mut [i32; 2], field: bool, at: usize) -> Result<[i32; 2]> {
    let t = tables();
    let mut out = [0i32; 2];
    for c in 0..2 {
        let fc = f_code[c];
        if !(1..=9).contains(&fc) { return err(at, format!("f_code {fc}")); }
        let m = t.motion.decode(r).ok_or_else(|| DecodeError { offset: at, msg: "bad motion_code".into() })? as i32;
        let code = if m != 0 && r.bit() { -m } else { m };
        let r_size = fc as u32 - 1;
        let f = 1i32 << r_size;
        let delta = if f == 1 || code == 0 {
            code
        } else {
            let residual = r.read(r_size) as i32;
            let d = (code.abs() - 1) * f + residual + 1;
            if code < 0 { -d } else { d }
        };
        let halve = field && c == 1;
        let prediction = if halve { pmv[c] >> 1 } else { pmv[c] };
        let (low, high, range) = (-16 * f, 16 * f - 1, 32 * f);
        let mut v = prediction + delta;
        if v < low { v += range; }
        if v > high { v -= range; }
        pmv[c] = if halve { v * 2 } else { v };
        out[c] = v;
    }
    Ok(out)
}

/// Saturation and mismatch control (7.4.3, 7.4.4) over a dequantised block.
#[inline]
fn mismatch(blk: &mut [i32; 64], sum: i32) {
    if sum & 1 == 0 { blk[63] ^= 1; }
}

fn intra_block(r: &mut BitReader, p: &PicParams, q: &[u8; 64], st: &mut SliceState, b: usize, blk: &mut [i32; 64]) -> Result<()> {
    let t = tables();
    let at = r.byte_pos();
    let comp = if b < 4 { 0 } else { b - 3 };
    let size = (if comp == 0 { t.dc_luma.decode(r) } else { t.dc_chroma.decode(r) }).ok_or_else(|| DecodeError { offset: at, msg: "bad dct_dc_size".into() })? as u32;
    let diff = if size == 0 {
        0
    } else {
        let v = r.read(size) as i32;
        if v < 1 << (size - 1) { v - (1 << size) + 1 } else { v }
    };
    let dc = st.dc_pred[comp] + diff;
    st.dc_pred[comp] = dc;
    let f0 = (dc * (8 >> p.dc_precision)).clamp(-2048, 2047);
    blk[0] = f0;
    let mut sum = f0;
    let table = if p.intra_vlc_format { &t.dct_b15 } else { &t.dct_b14 };
    let qs = st.qscale;
    let mut i = 1usize;
    loop {
        let (run, level) = match coefficient(r, table, at)? {
            None => break,
            Some(x) => x,
        };
        i += run;
        if i >= 64 { return err(at, "intra block coefficient index past 63"); }
        let pos = p.scan[i] as usize;
        let v = ((level * q[pos] as i32 * qs) / 16).clamp(-2048, 2047);
        blk[pos] = v;
        sum += v;
        i += 1;
    }
    mismatch(blk, sum);
    Ok(())
}

fn non_intra_block(r: &mut BitReader, scan: &[u8; 64], q: &[u8; 64], qs: i32, blk: &mut [i32; 64], at: usize) -> Result<()> {
    let t = tables();
    let mut sum = 0i32;
    let mut i = 0usize;
    // The first coefficient: "1s" is (0, ±1).
    if r.peek(1) == 1 {
        r.skip(1);
        let level: i32 = if r.bit() { -1 } else { 1 };
        let v = (((2 * level + level.signum()) * q[0] as i32 * qs) / 32).clamp(-2048, 2047);
        blk[0] = v;
        sum += v;
        i = 1;
    }
    loop {
        let (run, level) = match coefficient(r, &t.dct_b14, at)? {
            None => break,
            Some(x) => x,
        };
        i += run;
        if i >= 64 { return err(at, "non-intra block coefficient index past 63"); }
        let pos = scan[i] as usize;
        let v = (((2 * level + level.signum()) * q[pos] as i32 * qs) / 32).clamp(-2048, 2047);
        blk[pos] = v;
        sum += v;
        i += 1;
    }
    mismatch(blk, sum);
    Ok(())
}

/// One (run, signed level) or None at end of block.
#[inline]
fn coefficient(r: &mut BitReader, table: &vlc::Vlc, at: usize) -> Result<Option<(usize, i32)>> {
    let v = table.decode(r).ok_or_else(|| DecodeError { offset: at, msg: format!("bad DCT coefficient code {:016b}", r.peek(16)) })?;
    match v {
        DCT_EOB => Ok(None),
        DCT_ESCAPE => {
            let run = r.read(6) as usize;
            let raw = r.read(12) as i32;
            let level = if raw >= 2048 { raw - 4096 } else { raw };
            if level == 0 { return err(at, "escape level 0"); }
            Ok(Some((run, level)))
        }
        v => {
            let (run, l) = ((v >> 8) as usize, (v & 0xff) as i32);
            Ok(Some((run, if r.bit() { -l } else { l })))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal bit writer for building synthetic streams.
    #[derive(Default)]
    struct W {
        bytes: Vec<u8>,
        acc: u64,
        n: u32,
    }

    impl W {
        fn put(&mut self, v: u32, bits: u32) {
            for k in (0..bits).rev() {
                self.acc = self.acc << 1 | ((v >> k) & 1) as u64;
                self.n += 1;
                if self.n == 8 {
                    self.bytes.push(self.acc as u8);
                    self.acc = 0;
                    self.n = 0;
                }
            }
        }
        fn code(&mut self, s: &str) { for ch in s.chars().filter(|c| !c.is_whitespace()) { self.put((ch == '1') as u32, 1); } }
        fn align(&mut self) { while self.n != 0 { self.put(0, 1); } }
        fn start(&mut self, c: u8) {
            self.align();
            self.bytes.extend_from_slice(&[0, 0, 1, c]);
        }
    }

    fn seq(w: &mut W, width: u32, height: u32) {
        w.start(0xb3);
        w.put(width, 12);
        w.put(height, 12);
        w.put(1, 4);
        w.put(5, 4);
        w.put(10000, 18);
        w.put(1, 1);
        w.put(100, 10);
        w.put(0, 1);
        w.put(0, 1);
        w.put(0, 1);
        w.start(0xb5);
        w.put(1, 4);
        w.put(0x48, 8);
        w.put(1, 1); // progressive
        w.put(1, 2); // 4:2:0
        w.put(0, 2);
        w.put(0, 2);
        w.put(0, 12);
        w.put(1, 1);
        w.put(0, 8);
        w.put(0, 1);
        w.put(0, 2);
        w.put(0, 5);
    }

    fn pic(w: &mut W, tr: u32, kind: u32, f_code: u32) {
        w.start(0x00);
        w.put(tr, 10);
        w.put(kind, 3);
        w.put(0xffff, 16);
        if kind >= 2 { w.put(0, 1); w.put(7, 3); }
        if kind == 3 { w.put(0, 1); w.put(7, 3); }
        w.put(0, 1);
        w.start(0xb5);
        w.put(8, 4);
        for _ in 0..4 { w.put(f_code, 4); }
        w.put(0, 2); // dc precision 8
        w.put(3, 2); // frame
        w.put(0, 1);
        w.put(1, 1); // frame_pred_frame_dct
        w.put(0, 1);
        w.put(0, 1); // q_scale_type 0
        w.put(0, 1); // intra_vlc_format 0
        w.put(0, 1);
        w.put(0, 1);
        w.put(0, 1);
        w.put(1, 1); // progressive_frame
        w.put(0, 1);
    }

    /// Luma DC differential code of `diff` (sizes ≤ 4).
    fn dc_luma(w: &mut W, diff: i32) {
        let size = if diff == 0 { 0 } else { 32 - diff.unsigned_abs().leading_zeros() };
        w.code(["100", "00", "01", "101", "110"][size as usize]);
        if size > 0 { w.put(if diff > 0 { diff as u32 } else { (diff + (1 << size) - 1) as u32 }, size); }
    }

    /// An intra macroblock whose blocks carry only DC (luma differentials `d`, chroma 0).
    fn intra_mb(w: &mut W, d: [i32; 4]) {
        w.code("1"); // macroblock_type intra (I picture)
        for v in d {
            dc_luma(w, v);
            w.code("10"); // EOB
        }
        for _ in 0..2 {
            w.code("00"); // chroma size 0
            w.code("10");
        }
    }

    /// A synthetic 32×16 stream (2 macroblocks, one slice per picture): an I picture with DC-only blocks, a P picture
    /// predicted at a half-sample vector, and a B picture predicted backward with one coded block. Checks the display
    /// order I(0), B(1), P(2) and the samples computed by hand: DC/8 (the mismatch toggle on F[7][7] adds less than
    /// ±0.25), half-sample averaging, and the non-intra dequantisation.
    #[test]
    fn synthetic_display_order_and_samples() {
        let mut w = W::default();
        seq(&mut w, 32, 16);
        pic(&mut w, 0, 1, 15);
        w.start(0x01);
        w.put(8, 5);
        w.put(0, 1);
        w.code("1");
        intra_mb(&mut w, [0, 0, 0, 0]);
        w.code("1");
        intra_mb(&mut w, [8, 0, 0, 0]);
        pic(&mut w, 2, 2, 1);
        w.start(0x01);
        w.put(8, 5);
        w.put(0, 1);
        w.code("1");
        w.code("001");
        w.code("010");
        w.code("1");
        w.code("1");
        w.code("001");
        w.code("1");
        w.code("1");
        pic(&mut w, 1, 3, 1);
        w.start(0x01);
        w.put(8, 5);
        w.put(0, 1);
        w.code("1");
        w.code("011"); // backward, coded
        w.code("1");
        w.code("1");
        w.code("1101"); // cbp 4 (block 3)
        w.code("0100"); // (0, 2): the first-coefficient "1s" rule does not apply to a code starting with 0
        w.put(0, 1);
        w.code("10"); // EOB
        w.code("1"); // increment 1 → MB 1
        w.code("010"); // backward, not coded
        w.code("1");
        w.code("1");
        w.start(0xb7);
        let mut d = Decoder::new(w.bytes);
        let mut frames = Vec::new();
        while let Some(f) = d.next_frame().unwrap() { frames.push(f); }
        let order: Vec<(u16, PictureType)> = frames.iter().map(|f| (f.temporal_reference, f.kind)).collect();
        assert_eq!(order, [(0, PictureType::I), (1, PictureType::B), (2, PictureType::P)]);
        let (i, b, p) = (&frames[0], &frames[1], &frames[2]);
        // I: MB 0 block 0 DC 128 → F0 = 1024, sum even → F77 = 1: samples 128 ± a fraction → 128.
        assert_eq!(i.y[0], 128);
        // MB 1 block 0: DC 136 (predictor 128 + 8); its blocks 1..3 continue from 136.
        assert_eq!(i.y[16], 136);
        assert_eq!(i.y[31], 136);
        assert!(i.cb.iter().all(|&v| v == 128));
        // P: MB 0 predicted at +½ sample: columns 0..14 = 128, column 15 = avg(128, 136) = 132.
        assert_eq!(&p.y[0..16], &[128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 128, 132]);
        assert_eq!(p.y[16], 136);
        // B: backward from P; MB 0 block 3 has a residual (0, 2) non-intra at qscale 16: F0 = (5·16·16)/32 = 40 →
        // +5 on every sample (plus the mismatch toggle's fraction).
        assert_eq!(b.y[0], 128);
        assert_eq!(b.y[8 * 32 + 8], 133);
        assert_eq!(b.y[15 * 32 + 15], 132 + 5);
        assert_eq!(d.stats.pictures, [1, 1, 1]);
        assert_eq!(d.stats.frames_out, 3);
    }
}
