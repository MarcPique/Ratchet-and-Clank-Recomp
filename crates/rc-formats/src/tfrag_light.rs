//! Tfrag vertex lighting: a bit-exact port of the game's `LightTfrags` (boot 0x234f98, level
//! overlays e.g. level01 0x2a8e40). Spec and derivation: docs/plan/tfrag_lighting.md.
//!
//! The EE runs this pass in VU0 macro mode on PS2 floats. Colours are carried as the float
//! `65536.0 + c / 128` (bits `0x4780_0000 + c`), so the final colour byte is simply the low byte
//! of the float's bit pattern after all accumulations; every add truncates to that 1/128 grid.
//! To reproduce the bytes we therefore emulate the PS2 FMAC arithmetic on raw bit patterns
//! ([`ps2`]), not IEEE `f32`.
//!
//! Inputs:
//! * [`LightBank`]: the level's directional light sets, gameplay-file section pointer 0x04
//!   (copied to EE 0x180340 by the level-load routine, `FUN_00255958` in level01).
//! * [`NormalTable`]: 256 `(cos, sin)` f32 pairs the pass DMAs into scratchpad 0x70003800. The values
//!   are not IEEE-rounded `cos`/`sin` (296 of 512 differ in the last bits), so they must be read
//!   from the executable: boot ELF vaddr [`BOOT_NORMAL_TABLE_VADDR`] (every level overlay carries an
//!   identical copy, at 0x166500 in level01).
//! * The tfrag's per-vertex light records ([`crate::tfrag::TfragLight`], re-read here as
//!   `u16 pos_ofs, u8 azimuth, u8 elevation, u16 rgb555a1, u16 light_select`).

use crate::buf::{invalid, Buf, Result};
use crate::tfrag::{Tfrag, TfragRgba};
use bytemuck::{Pod, Zeroable};

/// Boot-ELF virtual address of the 256-entry `(cos, sin)` table used as the normal decode LUT
/// (`lui a0,0x16; addiu a0,a0,0x5500` in the boot copy of `LightTfrags`).
pub const BOOT_NORMAL_TABLE_VADDR: u32 = 0x0016_5500;
/// Directional light sets in the EE bank (`FastMemZero16(0x180340, 0x400)`).
pub const BANK_SETS: usize = 16;
/// The level loader caps the gameplay light count at 12 (`if (0xb < n) { printf; n = 0xc; }`).
pub const MAX_LEVEL_LIGHTS: usize = 12;
/// Point-light slots at EE 0x180740 (`FastMemZero16(0x180740, 0x100)`, 0x20 bytes each).
pub const POINT_LIGHT_SLOTS: usize = 8;

/// One directional light set (0x40 bytes): two lights, each a colour and a direction.
/// `color*.w` is the back-face factor: the dot product `d` becomes `max(d, d * w)`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct DirLightSet {
    /// 0x00: light A colour (1.0 adds 128 to the colour byte), w = back-face factor.
    pub color_a: [f32; 4],
    /// 0x10: light A direction (unit vector the light travels along), w unused.
    pub dir_a: [f32; 4],
    /// 0x20: light B colour, w = back-face factor.
    pub color_b: [f32; 4],
    /// 0x30: light B direction.
    pub dir_b: [f32; 4],
}
const _: () = assert!(std::mem::size_of::<DirLightSet>() == 0x40);

/// The EE directional light bank (16 sets; sets past the level's count are zero).
#[derive(Clone, Debug, PartialEq)]
pub struct LightBank {
    pub sets: [DirLightSet; BANK_SETS],
    /// Number of sets the gameplay file provided (after the loader's cap of 12).
    pub count: usize,
}

impl Default for LightBank {
    fn default() -> Self { LightBank { sets: [DirLightSet::default(); BANK_SETS], count: 0 } }
}

/// Parses the directional lights from a decompressed gameplay file: `u32` at 0x04 is the
/// section offset; the section is `s32 count`, 12 bytes padding, then `count` x [`DirLightSet`].
pub fn parse_light_bank(gameplay: &[u8]) -> Result<LightBank> {
    let g = Buf(gameplay);
    let ofs = g.u32(0x04)? as usize;
    let n = g.i32(ofs)?;
    if n < 0 { return invalid("negative directional light count"); }
    let n = (n as usize).min(MAX_LEVEL_LIGHTS);
    let mut bank = LightBank { count: n, ..Default::default() };
    let sets: Vec<DirLightSet> = g.pod_slice(ofs + 0x10, n, "directional lights")?;
    bank.sets[..n].copy_from_slice(&sets);
    Ok(bank)
}

/// One point light as stored at EE 0x180740 + 0x20 * slot (built at run time by gameplay code:
/// `FUN_002525f8`/`FUN_00252750` write colour = rgb / 128 and position/radius).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct PointLight {
    /// 0x00: colour (1.0 adds 128), w = back-face factor (also added into alpha, see spec).
    pub color: [f32; 4],
    /// 0x10: world position, w = radius.
    pub pos: [f32; 4],
}

/// Point lights applied to one tfrag: the 8-slot bank, the tfrag's nibble list (header 0x36;
/// low nibble first, 0xf terminates, at most 4) and the integer origin quadword at `light_ofs`.
pub struct PointLights<'a> {
    pub bank: &'a [PointLight; POINT_LIGHT_SLOTS],
    pub list: u16,
    pub origin: [i32; 4],
}

/// 256 `(cos, sin)` pairs for angle `i * 2pi / 256`, as raw f32 bits.
#[derive(Clone, Debug, PartialEq)]
pub struct NormalTable(pub [[u32; 2]; 256]);

impl NormalTable {
    /// Reads the table from an ELF32 image (the boot ELF) at `vaddr`.
    pub fn from_elf(elf: &[u8], vaddr: u32) -> Result<NormalTable> {
        let bytes = elf_read(elf, vaddr, 256 * 8)?;
        let words: Vec<u32> = Buf(bytes).pod_slice(0, 512, "normal table")?;
        let mut t = [[0u32; 2]; 256];
        for (i, e) in t.iter_mut().enumerate() { *e = [words[2 * i], words[2 * i + 1]]; }
        Ok(NormalTable(t))
    }
}

/// Reads `len` bytes at virtual address `vaddr` from the PT_LOAD segments of a little-endian ELF32.
pub fn elf_read(elf: &[u8], vaddr: u32, len: usize) -> Result<&[u8]> {
    let e = Buf(elf);
    if e.sub(0, 4, "ELF magic")?.bytes() != b"\x7fELF" { return invalid("not an ELF file"); }
    let (phoff, phentsize, phnum) = (e.u32(0x1c)? as usize, e.u16(0x2a)? as usize, e.u16(0x2c)? as usize);
    for i in 0..phnum {
        let ph = phoff + i * phentsize;
        let (ty, off, va, filesz) = (e.u32(ph)?, e.u32(ph + 4)? as usize, e.u32(ph + 8)?, e.u32(ph + 16)? as usize);
        if ty == 1 && vaddr >= va && (vaddr - va) as usize + len <= filesz {
            return Ok(e.sub(off + (vaddr - va) as usize, len, "ELF segment data")?.bytes());
        }
    }
    invalid(format!("vaddr {vaddr:#x}+{len:#x} is not in any ELF load segment"))
}

/// The per-vertex light record, decoded with the field meanings `LightTfrags` uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VertexLight {
    /// +0: byte offset of this vertex's position (3 x s16) from the tfrag data start (point lights only).
    pub pos_ofs: u16,
    /// +2: normal azimuth index into [`NormalTable`] (unsigned; 2pi / 256 per step).
    pub azimuth: u8,
    /// +3: normal elevation index.
    pub elevation: u8,
    /// +4: base (ambient) colour, 5:5:5:1 (r low bits, alpha bit 15), expanded by PEXT5.
    pub color: u16,
    /// +6: light set select: if bits 8..15 are 0, set `bits 0..3`; else blend set `bits 0..3`
    /// with set `bits 4..7` by `t = bits 8..15 / 256`.
    pub select: u16,
}

impl VertexLight {
    pub fn from_record(l: &crate::tfrag::TfragLight) -> VertexLight {
        let b = bytemuck::bytes_of(l);
        VertexLight {
            pos_ofs: u16::from_le_bytes([b[0], b[1]]),
            azimuth: b[2],
            elevation: b[3],
            color: u16::from_le_bytes([b[4], b[5]]),
            select: u16::from_le_bytes([b[6], b[7]]),
        }
    }
}

/// PS2 FPU/VU float arithmetic on raw bit patterns. Behaviour modelled (see the spec for sources):
/// no NaN/Inf/denormals (exponent 0 = zero, exponent 255 is an ordinary value, overflow clamps to
/// ±0x7fffffff, underflow flushes to ±0); every result rounds toward zero; the adder first
/// discards the bits of the smaller operand that lie more than one bit below the larger operand's
/// LSB, and returns the larger operand unchanged when the exponents differ by 25 or more.
/// Not modelled: the multiplier's rare last-bit deviations from a truncated exact product.
pub mod ps2 {
    pub const SIGN: u32 = 0x8000_0000;
    pub const MAX: u32 = 0x7fff_ffff;
    pub const ONE: u32 = 0x3f80_0000;

    fn exp(x: u32) -> i32 { ((x >> 23) & 0xff) as i32 }
    fn man(x: u32) -> u64 { ((x & 0x7f_ffff) | 0x80_0000) as u64 }

    /// Packs `sign * mag * 2^(lsb_exp)` (mag > 0, lsb_exp = biased exponent of mag's bit 23),
    /// truncating to 24 significant bits.
    fn pack(sign: u32, mag: u64, lsb_exp: i32) -> u32 {
        let k = 63 - mag.leading_zeros() as i32;
        let e = lsb_exp + (k - 23);
        let m = if k >= 23 { mag >> (k - 23) } else { mag << (23 - k) };
        if e > 255 { sign | MAX } else if e < 1 { sign } else { sign | (e as u32) << 23 | (m as u32 & 0x7f_ffff) }
    }

    pub fn mul(a: u32, b: u32) -> u32 {
        let s = (a ^ b) & SIGN;
        if exp(a) == 0 || exp(b) == 0 { return s; }
        // man(a) * man(b) has its binary point at bit 46; bit 23 of (p >> 23) is worth 2^(ea+eb-254).
        pack(s, (man(a) * man(b)) >> 23, exp(a) + exp(b) - 127)
    }

    pub fn add(a: u32, b: u32) -> u32 {
        let (ea, eb) = (exp(a), exp(b));
        if ea == 0 || eb == 0 {
            if ea != 0 { return a; }
            if eb != 0 { return b; }
            return if a & b & SIGN != 0 { SIGN } else { 0 };
        }
        let (hi, lo) = if ea >= eb { (a, b) } else { (b, a) };
        let d = exp(hi) - exp(lo);
        if d >= 25 { return hi; }
        let lo = if d >= 1 { lo & (u32::MAX << (d - 1)) } else { lo };
        let sv = |x: u32, m: u64| if x & SIGN != 0 { -(m as i64) } else { m as i64 };
        let sum = sv(hi, man(hi) << d) + sv(lo, man(lo));
        if sum == 0 { return 0; }
        pack(if sum < 0 { SIGN } else { 0 }, sum.unsigned_abs(), exp(lo))
    }

    pub fn sub(a: u32, b: u32) -> u32 { add(a, b ^ SIGN) }

    /// DIV: truncated quotient; division by zero gives ±MAX.
    pub fn div(a: u32, b: u32) -> u32 {
        let s = (a ^ b) & SIGN;
        if exp(b) == 0 { return s | MAX; }
        if exp(a) == 0 { return s; }
        let q = (man(a) << 24) / man(b); // in [2^23, 2^25)
        pack(s, q, exp(a) - exp(b) + 127 - 1)
    }

    /// SQRT of |x|, truncated.
    pub fn sqrt(x: u32) -> u32 {
        if exp(x) == 0 { return 0; }
        let e = exp(x) - 127;
        let odd = e.rem_euclid(2);
        let r = isqrt(man(x) << (23 + odd)); // in [2^23, 2^24)
        pack(0, r, (e - odd) / 2 + 127)
    }

    /// RSQRT a / sqrt(b): the square root is truncated first, then divided.
    pub fn rsqrt(a: u32, b: u32) -> u32 { div(a, sqrt(b)) }

    /// ITOF12: signed integer / 4096 (truncating integers wider than 24 bits).
    pub fn itof12(i: i32) -> u32 {
        if i == 0 { return 0; }
        pack(if i < 0 { SIGN } else { 0 }, i.unsigned_abs() as u64, 127 + 23 - 12)
    }

    fn key(x: u32) -> i64 { if x & SIGN != 0 { -((x & MAX) as i64) } else { x as i64 } }
    pub fn max(a: u32, b: u32) -> u32 { if key(b) > key(a) { b } else { a } }
    pub fn min(a: u32, b: u32) -> u32 { if key(b) < key(a) { b } else { a } }

    fn isqrt(n: u64) -> u64 {
        let mut r = (n as f64).sqrt() as u64;
        while r * r > n { r -= 1; }
        while (r + 1) * (r + 1) <= n { r += 1; }
        r
    }
}

type V4 = [u32; 4];

fn bits(v: [f32; 4]) -> V4 { v.map(f32::to_bits) }
/// `vmul`: lane-wise product.
fn vmul(a: V4, b: V4) -> V4 { [0, 1, 2, 3].map(|k| ps2::mul(a[k], b[k])) }
/// `vmulbc`: every lane times one scalar.
fn vmuls(a: V4, s: u32) -> V4 { a.map(|x| ps2::mul(x, s)) }
/// `vadda / vmadd` step: `acc + a * s` per lane.
fn vmadds(acc: V4, a: V4, s: u32) -> V4 { [0, 1, 2, 3].map(|k| ps2::add(acc[k], ps2::mul(a[k], s))) }
/// `vmul.xyz t, a, b; vadday.x ACC, t, t; vmaddz.x r, vf21(1.0), t`: `(ax*bx + ay*by) + 1.0*(az*bz)`.
fn dot3(a: V4, b: V4) -> u32 {
    let p = vmul(a, b);
    ps2::add(ps2::add(p[0], p[1]), ps2::mul(ps2::ONE, p[2]))
}
/// Colour byte -> float `65536 + c / 128` (`pextlb/pextlh` to words, `padduw` with 0x47800000).
fn color_float(c: [u8; 4]) -> V4 { c.map(|b| 0x4780_0000 + b as u32) }
/// `vminibcx.xyz` with 0x478000ff, then `ppach/ppacb`: the low byte of each lane.
fn pack_color(v: V4) -> [u8; 4] {
    let clamp = 0x4780_00ff;
    [ps2::min(v[0], clamp) as u8, ps2::min(v[1], clamp) as u8, ps2::min(v[2], clamp) as u8, v[3] as u8]
}

/// PEXT5 of the 5:5:5:1 base colour: r = bits 0..4 << 3, g = bits 5..9 << 3, b = bits 10..14 << 3, a = bit 15 << 7.
pub fn pext5(c: u16) -> [u8; 4] {
    [((c & 0x1f) << 3) as u8, ((c >> 5 & 0x1f) << 3) as u8, ((c >> 10 & 0x1f) << 3) as u8, ((c >> 15) << 7) as u8]
}

/// The normal the pass stores in scratchpad: `-(cos az * cos el, sin az * cos el, sin el)`, w = sin el.
/// (`ld` of table[az] ‖ table[el]; `vmulz.xy`; `vaddw.z`; `vsub.xyz vf0 - v`.)
pub fn decode_normal(table: &NormalTable, azimuth: u8, elevation: u8) -> V4 {
    let [ca, sa] = table.0[azimuth as usize];
    let [ce, se] = table.0[elevation as usize];
    let (x, y, z) = (ps2::mul(ca, ce), ps2::mul(sa, ce), ps2::add(0, se));
    [ps2::sub(0, x), ps2::sub(0, y), ps2::sub(0, z), se]
}

/// Light set for one vertex as the VU registers hold it: (colour A, dir A, colour B, dir B), with
/// the back-face factors extracted and colour w lanes cleared.
struct ActiveSet { ca: V4, da: V4, cb: V4, db: V4, wa: u32, wb: u32 }

fn active_set(bank: &LightBank, select: u16) -> ActiveSet {
    let set = |i: u16| bank.sets[(i & 0xf) as usize];
    let (mut ca, da, mut cb, db);
    if select & 0xff00 == 0 {
        let s = set(select);
        (ca, da, cb, db) = (bits(s.color_a), bits(s.dir_a), bits(s.color_b), bits(s.dir_b));
    } else {
        // t = ((sel >> 4) & 0xff0) itof12 = hi / 256; w = 1 - t.
        let t = ps2::itof12(((select as u32 >> 4) & 0xff0) as i32);
        let w = ps2::sub(ps2::ONE, t);
        let (a, b) = (set(select), set(select >> 4));
        let blend = |x: [f32; 4], y: [f32; 4]| {
            let (p, q) = (vmuls(bits(x), w), vmuls(bits(y), t));
            [0, 1, 2, 3].map(|k| ps2::add(p[k], q[k]))
        };
        ca = blend(a.color_a, b.color_a);
        cb = blend(a.color_b, b.color_b);
        let norm = |v: V4| {
            let q = ps2::rsqrt(ps2::ONE, dot3(v, v));
            [ps2::mul(v[0], q), ps2::mul(v[1], q), ps2::mul(v[2], q), v[3]]
        };
        da = norm(blend(a.dir_a, b.dir_a));
        db = norm(blend(a.dir_b, b.dir_b));
    }
    // vaddw.x/.y vf31 = 0 + colour.w; vsubw.w colour = 1 - 1.
    let (wa, wb) = (ps2::add(0, ca[3]), ps2::add(0, cb[3]));
    ca[3] = 0;
    cb[3] = 0;
    ActiveSet { ca, da, cb, db, wa, wb }
}

/// Directional pass for one vertex: `base + A.rgb * max(dA, dA*wA) + B.rgb * max(dB, dB*wB)`.
pub fn light_vertex(bank: &LightBank, table: &NormalTable, l: &VertexLight) -> [u8; 4] {
    light_vertex_n(bank, &decode_normal(table, l.azimuth, l.elevation), l)
}

fn light_vertex_n(bank: &LightBank, n: &V4, l: &VertexLight) -> [u8; 4] {
    let s = active_set(bank, l.select);
    let da = dot3(*n, s.da);
    let db = dot3(*n, s.db);
    let da = ps2::max(da, ps2::mul(da, s.wa));
    let db = ps2::max(db, ps2::mul(db, s.wb));
    let acc = vmuls(color_float(pext5(l.color)), ps2::ONE);
    let acc = vmadds(acc, s.ca, da);
    pack_color(vmadds(acc, s.cb, db))
}

/// Point-light pass for one vertex (only when `r^2 - |p - L|^2` is not negative).
fn point_light_vertex(pl: &PointLight, base: [u32; 3], pos: [i16; 3], n: &V4, rgba: [u8; 4]) -> Option<[u8; 4]> {
    let (c, lp) = (bits(pl.color), bits(pl.pos));
    let p = [0, 1, 2].map(|k| ps2::add(ps2::itof12((pos[k] as i32) << 2), base[k]));
    let inv_r = ps2::div(ps2::ONE, lp[3]);
    let dv: V4 = [ps2::sub(p[0], lp[0]), ps2::sub(p[1], lp[1]), ps2::sub(p[2], lp[2]), 0];
    let dist2 = dot3(dv, dv);
    // `vsubx.w vf0, r2, dist2` only sets flags; `cfc2` MAC flag 0x10 (sign of w) skips the vertex.
    if ps2::sub(ps2::mul(lp[3], lp[3]), dist2) & ps2::SIGN != 0 { return None; }
    let dist = ps2::sqrt(dist2);
    let falloff = ps2::sub(ps2::ONE, ps2::mul(inv_r, dist));
    let scale = ps2::mul(falloff, ps2::div(ps2::ONE, dist));
    let v = [ps2::mul(dv[0], scale), ps2::mul(dv[1], scale), ps2::mul(dv[2], scale), 0];
    let d = dot3(*n, v);
    let d = ps2::max(d, ps2::mul(d, c[3]));
    let acc = vmuls(color_float(rgba), ps2::ONE);
    Some(pack_color(vmadds(acc, c, d)))
}

/// Runs `LightTfrags` for one tfrag and returns its new RGBA array (the game writes it back over
/// the tfrag's RGBA block, `rgba_ofs`, which VU1 then reads verbatim). Entries `0..vert_count`
/// are computed; the padding entries up to `rgba_size * 4` keep the stored values (the game fills
/// them with scratchpad leftovers that no vertex references).
pub fn light_tfrag(t: &Tfrag, bank: &LightBank, table: &NormalTable, points: Option<&PointLights>) -> Vec<TfragRgba> {
    let mut out = t.rgba.clone();
    let n = (t.header.vert_count as usize).min(t.lights.len()).min(out.len());
    let recs: Vec<VertexLight> = t.lights[..n].iter().map(VertexLight::from_record).collect();
    let normals: Vec<V4> = recs.iter().map(|l| decode_normal(table, l.azimuth, l.elevation)).collect();
    // Header 0x34 >= 0 selects one set for the whole tfrag (unused by every RAC1 NTSC level: all are -1).
    let single = t.header.dir_lights_one as i8;
    let mut rgba: Vec<[u8; 4]> = recs
        .iter()
        .zip(&normals)
        .map(|(l, nv)| {
            let l = if single >= 0 { VertexLight { select: (single as u16) & 0xf, ..*l } } else { *l };
            light_vertex_n(bank, nv, &l)
        })
        .collect();
    if let Some(p) = points {
        let base = [0, 1, 2].map(|k| ps2::itof12(p.origin[k].wrapping_shl(2)));
        let mut list = p.list as u32 | 0xf_0000;
        while list & 0xf != 0xf {
            let pl = &p.bank[(list & 0xf) as usize % POINT_LIGHT_SLOTS];
            list >>= 4;
            for (i, c) in rgba.iter_mut().enumerate() {
                let q = t.positions.get(i).map_or([0; 3], |q| [q.x, q.y, q.z]);
                if let Some(v) = point_light_vertex(pl, base, q, &normals[i], *c) { *c = v; }
            }
        }
    }
    for (o, c) in out.iter_mut().zip(rgba) { *o = TfragRgba { r: c[0], g: c[1], b: c[2], a: c[3] }; }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(x: f32) -> u32 { x.to_bits() }
    fn table() -> NormalTable {
        // Synthetic table: exact values at the quarter angles, IEEE cos/sin elsewhere.
        let mut t = [[0u32; 2]; 256];
        for (i, e) in t.iter_mut().enumerate() {
            let a = i as f64 * std::f64::consts::TAU / 256.0;
            *e = [f(a.cos() as f32), f(a.sin() as f32)];
        }
        t[0] = [f(1.0), 0];
        t[64] = [0, f(1.0)];
        t[192] = [0, f(-1.0)];
        NormalTable(t)
    }

    #[test]
    fn ps2_arithmetic_truncates() {
        assert_eq!(ps2::add(f(65536.0), f(0.5 / 128.0 * 1.999)), f(65536.0)); // below the 1/128 grid
        assert_eq!(ps2::add(f(65536.0), f(1.0)), f(65537.0));
        assert_eq!(ps2::mul(f(1.0), f(0.3)), f(0.3));
        assert_eq!(ps2::mul(0x3f80_0003, 0x3fc0_0001), 0x3fc0_0005); // exact 1.5 + 5.5 ulp: truncated (IEEE nearest: ..06)
        assert_eq!(ps2::add(f(1.0), f(-1.0)), 0);
        assert_eq!(ps2::add(f(1.0), f(1e-10)), f(1.0)); // exponent gap >= 25
        // Adder drops the smaller operand's bits below half an ULP of the larger one first:
        assert_eq!(ps2::add(f(1.0), f(-1.5 * (2.0f32).powi(-24))), 0x3f7f_ffff); // IEEE round-toward-zero: 0x3f7ffffe
        assert_eq!(ps2::sqrt(f(4.0)), f(2.0));
        assert_eq!(ps2::sqrt(f(2.0)), 0x3fb5_04f3); // sqrt 2 = 1.41421353.., truncated
        assert_eq!(ps2::div(f(1.0), f(4.0)), f(0.25));
        assert_eq!(ps2::div(f(1.0), f(3.0)), 0x3eaa_aaaa); // IEEE nearest is ..ab
        assert_eq!(ps2::itof12(0x800), f(0.5));
        assert_eq!(ps2::itof12(-4096 * 3), f(-3.0));
        assert_eq!(ps2::mul(f(1e30), f(1e30)), ps2::MAX);
    }

    #[test]
    fn normal_decode() {
        let t = table();
        // Elevation 64 = straight up: stored normal is the negation, (0, 0, -1).
        assert_eq!(decode_normal(&t, 0, 64)[..3], [0, 0, f(-1.0)]);
        // Azimuth 0, elevation 0: -(1, 0, 0).
        assert_eq!(decode_normal(&t, 0, 0)[..3], [f(-1.0), 0, 0]);
        // Azimuth 64 (+Y), elevation 192 (down): -(0*0, 1*0, -1) = (0, 0, 1).
        assert_eq!(decode_normal(&t, 64, 192)[..3], [0, 0, f(1.0)]);
        assert_eq!(pext5(0x8000 | 31 | 16 << 5 | 1 << 10), [248, 128, 8, 128]);
    }

    #[test]
    fn one_light_evaluation() {
        let t = table();
        let mut bank = LightBank::default();
        // Sun travelling straight down (-Z), colour (1.0, 0.5, 0.25); back light along +X with w = -0.5.
        bank.sets[2] = DirLightSet {
            color_a: [1.0, 0.5, 0.25, 0.0],
            dir_a: [0.0, 0.0, -1.0, 0.0],
            color_b: [0.25, 0.25, 0.25, -0.5],
            dir_b: [1.0, 0.0, 0.0, 0.0],
        };
        // Up-facing vertex, base colour (40, 48, 56), alpha bit set.
        let l = VertexLight { pos_ofs: 0, azimuth: 0, elevation: 64, color: 0x8000 | 5 | 6 << 5 | 7 << 10, select: 2 };
        // dA = (0,0,-1).(0,0,-1) = 1 -> +128, +64, +32. dB = 0.
        assert_eq!(light_vertex(&bank, &t, &l), [168, 112, 88, 128]);
        // Facing -X (azimuth 0, elevation 0 -> stored normal (-1, 0, 0)): dA = 0, dB = -1 -> max(-1, 0.5) = 0.5 -> +16.
        let l2 = VertexLight { elevation: 0, ..l };
        assert_eq!(light_vertex(&bank, &t, &l2), [56, 64, 72, 128]);
        // Saturation: 248 + 128 clamps to 255.
        let l3 = VertexLight { color: 0x8000 | 31, ..l };
        assert_eq!(light_vertex(&bank, &t, &l3)[0], 255);
        // Blend 50 % between set 2 and an empty set 3: colour halves, direction renormalises -> +64 red.
        let l4 = VertexLight { select: 0x80 << 8 | 3 << 4 | 2, ..l };
        assert_eq!(light_vertex(&bank, &t, &l4), [104, 80, 72, 128]);
    }

    #[test]
    fn point_light_falloff() {
        let pl = PointLight { color: [1.0, 1.0, 1.0, 0.0], pos: [0.0, 0.0, 8.0, 16.0] };
        let n = [0, 0, f(-1.0), 0]; // up-facing
        // Vertex at the origin, 8 below the light, r = 16: (1 - 8/16) / 8 * (0,0,-8) -> d = 0.5 -> +64.
        assert_eq!(point_light_vertex(&pl, [0; 3], [0, 0, 0], &n, [10, 20, 30, 128]), Some([74, 84, 94, 128]));
        // Out of range: skipped.
        let far = PointLight { pos: [0.0, 0.0, 30.0, 16.0], ..pl };
        assert_eq!(point_light_vertex(&far, [0; 3], [0, 0, 0], &n, [10, 20, 30, 128]), None);
    }
}
