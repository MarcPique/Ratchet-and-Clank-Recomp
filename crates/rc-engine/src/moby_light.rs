//! Moby vertex lighting: the MobyProc light block (bit-exact, `rc_formats::moby_light::moby_lights`)
//! per instance at load, evaluated per vertex on the GPU (`assets/shaders/moby.wgsl`) after skinning,
//! with an f32 replica here ([`shader_light_f32`]) to measure how far the GPU's f32 math drifts from the
//! game's PS2-float VU0 104691 pass. Spec: docs/plan/moby_skinning_lighting.md §2, §4-5.
//!
//! `RC_MOBY_CPU_LIGHT=1` keeps the bit-exact CPU path (`rc_formats::moby_light::light_lod`, identity
//! palette = bind pose only): colours are computed once per distinct (class, rotation, light word,
//! ambient) and read from a storage buffer. `RC_MOBY_LIGHT_CHECK=1` compares, for the identity palette,
//! the CPU exact colours with [`shader_light_f32`] and prints the drift.
//!
//! Inputs: the level's directional light bank (gameplay pointer 0x04, the same bank the tfrag pass
//! uses) and the boot ELF's `(cos, sin)` normal table (`extracted/boot/SCUS_971.99`), which the shader
//! also receives so the normal decode matches the game's table, not `sin()`.
//! `RC_NO_LIGHT=1` skips lighting: every vertex gets 0x80 (= texture colour unchanged).

use anyhow::{Context, Result};
use rc_formats::gameplay::MobyInstance;
use rc_formats::moby::LevelMobyClass;
use rc_formats::moby_anim::Rows;
use rc_formats::moby_light::{self, MobyLights, V4};
use rc_formats::tfrag_light::{self, LightBank, NormalTable, BOOT_NORMAL_TABLE_VADDR};
use std::path::Path;

pub struct MobyLighting {
    pub bank: LightBank,
    pub table: NormalTable,
}

/// `None` when `RC_NO_LIGHT` is set.
pub fn load(root: &Path, gameplay: &[u8]) -> Result<Option<MobyLighting>> {
    if std::env::var("RC_NO_LIGHT").is_ok_and(|v| v != "0") { return Ok(None); }
    let bank = tfrag_light::parse_light_bank(gameplay).context("parsing directional lights")?;
    let elf_path = root.join("boot/SCUS_971.99");
    let elf = crate::disc_source::read_path(root, &elf_path)?;
    let table = NormalTable::from_elf(&elf, BOOT_NORMAL_TABLE_VADDR).context("reading the normal table from the boot ELF")?;
    Ok(Some(MobyLighting { bank, table }))
}

/// Rotation rows of an instance as `fun_0020def8` stores them (moby+0xc0..0xef).
pub fn instance_rows(m: &MobyInstance) -> [V4; 3] { moby_light::instance_rows(m.rotation, m.mode()) }

/// The job light block of an instance (alpha 0x80: moby+0x23 = 0x80 and the MobyProc distance fade is
/// full inside the draw distance).
pub fn instance_lights(l: &MobyLighting, rows: &[V4; 3], m: &MobyInstance) -> MobyLights {
    moby_light::moby_lights(rows, &l.bank, m.light_word(), m.ambient_rgb(), 0x80)
}

/// Per-vertex RGBA of the class's high-LOD packets for one instance in the bind pose, bit-exact.
pub fn light_instance(l: Option<&MobyLighting>, class: &LevelMobyClass, lights: Option<&MobyLights>) -> Vec<Vec<[u8; 4]>> {
    match (l, lights) {
        (Some(l), Some(k)) => moby_light::light_lod(&class.class.high_lod, k, &l.table),
        _ => class.class.high_lod.iter().map(|s| vec![[0x80; 4]; s.vertices.len()]).collect(),
    }
}

/// The light block as the shader's `MobyInst` holds it (f32; ambient as colour counts 0..255).
#[derive(Clone, Copy, Debug, Default)]
pub struct GpuLights {
    /// Row j = (L_0[j], L_1[j], L_2[j], 0).
    pub rows: [[f32; 4]; 3],
    pub colors: [[f32; 4]; 3],
    /// (−|K_0|, −|K_1|, −|K_2|, 1).
    pub neg_k: [f32; 4],
    /// r, g, b, alpha byte (the low byte of `0x47800000 | byte`).
    pub ambient: [f32; 4],
}

impl GpuLights {
    pub fn new(k: &MobyLights) -> Self {
        let f = |v: V4| v.map(f32::from_bits);
        GpuLights { rows: k.rows.map(f), colors: k.colors.map(f), neg_k: f(k.neg_k), ambient: k.ambient.map(|b| (b & 0xffff) as f32) }
    }
}

/// f32 replica of the WGSL lighting (same operation order; the GPU may still fuse multiply-adds and
/// uses its own `inverseSqrt`): n from the table, n′ = M·n, d = L·n′, f = max(d, −|K|·d),
/// c = A + ⌊128·(Σ C_k f_k)·rsqrt(|n′|²)⌋, then the EE pack `(s16(c)·m clamped to s16) >> 7`, low byte.
pub fn shader_light_f32(g: &GpuLights, table: &NormalTable, m: &Rows, az: u8, el: u8, mult: [u8; 4]) -> [u8; 4] {
    let t = |i: u8| table.0[i as usize].map(f32::from_bits);
    let ([ca, sa], [ce, se]) = (t(az), t(el));
    let n = [ca * ce, sa * ce, se];
    let np: [f32; 3] = std::array::from_fn(|k| m[0][k] * n[0] + m[1][k] * n[1] + m[2][k] * n[2]);
    let d: [f32; 4] = std::array::from_fn(|k| g.rows[0][k] * np[0] + g.rows[1][k] * np[1] + g.rows[2][k] * np[2]);
    let f: [f32; 4] = std::array::from_fn(|k| d[k].max(d[k] * g.neg_k[k]));
    let s: [f32; 4] = std::array::from_fn(|k| g.colors[0][k] * f[0] + g.colors[1][k] * f[1] + g.colors[2][k] * f[2]);
    let q = 1.0 / (np[0] * np[0] + np[1] * np[1] + np[2] * np[2]).sqrt();
    std::array::from_fn(|k| {
        let c = g.ambient[k] + (s[k] * q * 128.0).floor();
        let h = (c as i32) as i16 as i32;
        (((h * mult[k] as i32).clamp(-32768, 32767)) >> 7) as u8
    })
}

/// The multiplier each vertex's colour is packed with. Duplicates are not re-lit by the game: they copy
/// the colour cached for their 9-bit id (carried across the packets of the list), i.e. the source's
/// multiplier, so they get the source's bytes here and the GPU relights them to the same colour (their
/// normal, skin and position are the source's too).
pub fn vertex_multipliers(lod: &[rc_formats::moby::MobySubmesh]) -> Vec<Vec<[u8; 4]>> {
    let mut cache = [[0x80u8; 4]; 512];
    lod.iter()
        .map(|sub| {
            let mult = sub.rgba_multiplier_records();
            let n_in = sub.vertices.iter().take_while(|v| !v.duplicate).count();
            let mut out: Vec<[u8; 4]> = (0..n_in).map(|i| mult.get(i).copied().unwrap_or([0x80; 4])).collect();
            for (v, m) in sub.vertices[..n_in].iter().zip(&out) { cache[(v.id & 0x1ff) as usize] = *m; }
            out.extend(sub.vertices[n_in..].iter().map(|v| cache[(v.id & 0x1ff) as usize]));
            out
        })
        .collect()
}

/// `RC_MOBY_LIGHT_CHECK=1`: CPU exact vs [`shader_light_f32`] for the identity palette on every vertex of
/// every distinct colour set. Returns (vertices, max per-channel difference, vertices differing by > 1,
/// vertices differing at all).
pub fn light_check(l: &MobyLighting, class: &LevelMobyClass, k: &MobyLights, exact: &[Vec<[u8; 4]>]) -> (usize, i32, usize, usize) {
    let g = GpuLights::new(k);
    let (mut n, mut max, mut over1, mut any) = (0, 0, 0, 0);
    let mults = vertex_multipliers(&class.class.high_lod);
    for ((sub, ex), mult) in class.class.high_lod.iter().zip(exact).zip(&mults) {
        for ((v, e), &m) in sub.vertices.iter().zip(ex).zip(mult) {
            let c = shader_light_f32(&g, &l.table, &rc_formats::moby_anim::IDENTITY, v.normal_azimuth, v.normal_elevation, m);
            let dmax = (0..4).map(|k| (c[k] as i32 - e[k] as i32).abs()).max().unwrap();
            n += 1;
            max = max.max(dmax);
            if dmax > 1 { over1 += 1; }
            if dmax > 0 { any += 1; }
        }
    }
    (n, max, over1, any)
}
