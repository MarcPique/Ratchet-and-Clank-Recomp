//! Distilled Novalis spawn facts: the numbers `compare-novalis-spawn` reads from one savestate, written once
//! (`rc-trace distill-spawn`) as a small, human-readable TSV, so a test can check the port against the game
//! without the user's savestate or its 32 MiB EE dump. Numbers only, no raw memory: counters, the fog and
//! underwater look, the RAM light bank, per-chunk and per-palette FNV-1a hashes, and the moby table's
//! slot / class / spawn id / state columns. The committed fixture is `tools/trace/tests/fixtures/novalis_spawn.tsv`;
//! `tests/trace/novalis_spawn.rs` compares the port (needing only `extracted/`) with it.
//!
//! Every check here mirrors one in [`crate::novalis_spawn`] and produces the same tally name; `distill-spawn`
//! runs both on the dump it distils and refuses to write a fixture whose tallies disagree. Tie and shrub
//! palettes are compared per instance by hash (the EE check also counts entries and prints the differing ones).

use crate::ee::EeImage;
use crate::novalis_spawn::{self as ns, Out, Timeline};
use crate::port_sim::{LevelData, PortSim};
use crate::tie_shrub_cmp as tsc;
use anyhow::{bail, ensure, Context, Result};
use rc_formats::shrub_light::{self, ShrubPointLights, PALETTE};
use rc_formats::tfrag_light::{DirLightSet, LightBank, PointLight, BANK_SETS, POINT_LIGHT_SLOTS};
use rc_formats::tie_light::{self, TiePointLights};
use rc_game::game_state::Scope;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::path::Path;

/// FNV-1a, 64 bit.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// One save chunk at its RAM address.
#[derive(Clone, Debug, PartialEq)]
pub struct Chunk {
    /// None = global, Some(L) = level L.
    pub level: Option<usize>,
    pub id: i32,
    pub addr: u32,
    pub size: u32,
    pub hash: u64,
    /// Human-readable summary (`novalis_spawn::describe`); informational only.
    pub value: String,
}

/// One RAM moby slot (0x100-byte record) before the 0xff end marker.
#[derive(Clone, Debug, PartialEq)]
pub struct MobySlot {
    pub slot: u32,
    /// +0xa6
    pub class: i16,
    /// +0xb2 spawn id (uid)
    pub uid: i16,
    /// +0x20
    pub state: u8,
    /// +0x34
    pub mode: u16,
    /// +0x21
    pub group: i8,
    /// +0x30
    pub update_dist: u8,
    /// +0x31
    pub visible: u8,
    /// +0x32
    pub draw_dist: i16,
    /// Class 13 (bolt): pvar +0x55, the load pass's spin-direction draw.
    pub spin: Option<u8>,
    /// Class 500 (crate): rotation z (+0x48), the load pass's π/2 turn.
    pub turn: Option<f32>,
}

/// One lit tie or shrub instance.
#[derive(Clone, Debug, PartialEq)]
pub struct Lit {
    pub index: u32,
    /// The RAM record passed the content check against the disc instance.
    pub located: bool,
    /// Run-time record +0x1c light selector, +0x1e point-light nibbles.
    pub sel: u16,
    pub plist: u16,
    /// FNV-1a of the lit RGBA words in RAM.
    pub hash: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpawnFacts {
    pub level: u32,
    pub tick_counter: u32,
    pub mode: i32,
    pub mode_frames: i32,
    pub idle: i32,
    pub rand_state: u32,
    pub hp: i32,
    pub fog_color: [u8; 3],
    /// near, far, near intensity, far intensity.
    pub fog: [f32; 4],
    pub camera: [f32; 3],
    pub underwater_tint: [u8; 4],
    pub underwater_color: [u8; 3],
    pub underwater_fog: [f32; 4],
    pub chunks: Vec<Chunk>,
    pub mobys: Vec<MobySlot>,
    /// The 16 directional sets as in RAM: colour a, dir a, colour b, dir b.
    pub light_sets: Vec<[f32; 16]>,
    /// Point-light slots: colour, pos.
    pub point_lights: Vec<[f32; 8]>,
    pub tie_count: u32,
    pub ties: Vec<Lit>,
    pub shrub_count: u32,
    pub shrubs: Vec<Lit>,
}

fn f(ee: &EeImage, a: u32) -> Result<f32> { Ok(f32::from_bits(ee.u32(a)?)) }

impl SpawnFacts {
    /// Reads the facts from an EE image; `extracted` supplies the chunk table and the disc tie/shrub instances
    /// the RAM records are located by.
    pub fn from_ee(ee: &EeImage, extracted: &Path) -> Result<SpawnFacts> {
        let mut s = SpawnFacts {
            level: ee.u32(ns::LEVEL)?,
            tick_counter: ee.u32(ns::TICK)?,
            mode: ee.u32(ns::MODE)? as i32,
            mode_frames: ee.u32(ns::MODE_FRAMES)? as i32,
            idle: ee.u32(ns::IDLE)? as i32,
            rand_state: ee.u32(ns::RAND_STATE)?,
            hp: ee.u32(0x1415f8)? as i32,
            ..Default::default()
        };
        let c = ee.bytes(ns::FOG_GLOBALS, 3)?;
        s.fog_color = [c[0], c[1], c[2]];
        s.fog = [f(ee, 0x15f448)?, f(ee, 0x15f44c)?, f(ee, 0x15f450)?, f(ee, 0x15f454)?];
        s.camera = [f(ee, ns::CAM_POS)?, f(ee, ns::CAM_POS + 4)?, f(ee, ns::CAM_POS + 8)?];
        let u = ee.bytes(ns::UNDERWATER_LOOK, 0x18)?;
        s.underwater_tint = [u[0], u[1], u[2], u[3]];
        s.underwater_color = [u[4], u[5], u[6]];
        s.underwater_fog = [f(ee, 0x161208)?, f(ee, 0x16120c)?, f(ee, 0x161210)?, f(ee, 0x161214)?];
        // Save chunks, in the order `check_game_state` walks them.
        let gs = ns::port_game_state(extracted)?;
        let t = &gs.state.tables;
        for scope in std::iter::once(None).chain((0..20).map(Some)) {
            let (descs, l) = match scope { None => (&t.global, 0), Some(l) => (&t.level, l as u32) };
            for d in descs {
                let addr = d.addr + l * d.size;
                let ram = ee.bytes(addr, d.size as usize)?;
                let sc = scope.map_or(Scope::Global, Scope::Level);
                s.chunks.push(Chunk { level: scope, id: d.id, addr, size: d.size, hash: fnv1a(ram), value: ns::describe(sc, d.id, ram) });
            }
        }
        // Moby table (same walk as `check_mobys`).
        let hero_ptr = ee.u32(ns::HERO_MOBY_PTR)?;
        let hero_index = ee.u32(hero_ptr + 0xac)?;
        let base = hero_ptr - 0x100 * hero_index;
        let mut j = 0u32;
        loop {
            let r = ee.bytes(base + 0x100 * j, 0x100)?;
            if r[0x20] == 0xff { break; }
            let class = i16::from_le_bytes([r[0xa6], r[0xa7]]);
            let spin = (class == 13).then(|| -> Result<u8> { let pv = ee.u32(base + 0x100 * j + 0x78)?; Ok(ee.bytes(pv + 0x55, 1)?[0]) }).and_then(Result::ok);
            let turn = (class == 500).then(|| f32::from_le_bytes(r[0x48..0x4c].try_into().unwrap()));
            s.mobys.push(MobySlot {
                slot: j, class, uid: i16::from_le_bytes([r[0xb2], r[0xb3]]), state: r[0x20], mode: u16::from_le_bytes([r[0x34], r[0x35]]), group: r[0x21] as i8,
                update_dist: r[0x30], visible: r[0x31], draw_dist: i16::from_le_bytes([r[0x32], r[0x33]]), spin, turn,
            });
            j += 1;
        }
        // Light bank and lit palettes.
        let (bank, points) = tsc::ram_banks(ee, 0)?;
        s.light_sets = bank.sets.iter().map(|x| { let v = [x.color_a, x.dir_a, x.color_b, x.dir_b].concat(); v.try_into().unwrap() }).collect();
        s.point_lights = points.iter().map(|p| { let v = [p.color, p.pos].concat(); v.try_into().unwrap() }).collect();
        let inp = tsc::Inputs::load(extracted, 1)?;
        s.tie_count = ee.u32(tsc::TIE_COUNT)?;
        let rt = ee.u32(tsc::TIE_RT_RECORDS)?;
        for (i, inst) in inp.ties.iter().enumerate().take(s.tie_count as usize) {
            let rec = ee.bytes(rt + 0x20 * i as u32, 0x20)?;
            let p = u32::from_le_bytes(rec[0x10..0x14].try_into().unwrap());
            let b = ee.bytes(p, 0x1c0)?;
            let amb_ok = (0..tie_light::SLOTS).all(|j| u16::from_le_bytes([b[0x140 + 2 * j], b[0x141 + 2 * j]]) == inst.ambient_rgbas[j]);
            let mat_ok = (0..4).all(|c| (0..3).all(|k| f32::from_le_bytes(b[16 * c + 4 * k..16 * c + 4 * k + 4].try_into().unwrap()).to_bits() == inst.matrix[c][k].to_bits()));
            s.ties.push(Lit { index: i as u32, located: amb_ok && mat_ok, sel: u16::from_le_bytes([rec[0x1c], rec[0x1d]]), plist: u16::from_le_bytes([rec[0x1e], rec[0x1f]]), hash: fnv1a(&b[0x40..0x140]) });
        }
        s.shrub_count = ee.u32(tsc::SHRUB_COUNT)?;
        let rt = ee.u32(tsc::SHRUB_RT_RECORDS)?;
        let pal = ee.u32(tsc::SHRUB_PALETTES)?;
        let mats = ee.u32(tsc::SHRUB_RT_RECORDS + 8)?;
        for (i, inst) in inp.shrubs.iter().enumerate().take(s.shrub_count as usize) {
            let rec = ee.bytes(rt + 0x20 * i as u32, 0x20)?;
            let m = ee.bytes(mats + 0x40 * i as u32, 0x40)?;
            let mat_ok = (0..4).all(|c| (0..3).all(|k| f32::from_le_bytes(m[16 * c + 4 * k..16 * c + 4 * k + 4].try_into().unwrap()).to_bits() == inst.matrix[c][k].to_bits()));
            let ram = ee.bytes(pal + 0x60 * i as u32, 4 * PALETTE)?;
            s.shrubs.push(Lit { index: i as u32, located: mat_ok, sel: u16::from_le_bytes([rec[0x1c], rec[0x1d]]), plist: u16::from_le_bytes([rec[0x1e], rec[0x1f]]), hash: fnv1a(ram) });
        }
        Ok(s)
    }

    /// The timeline the port is scripted from (as [`Timeline::read`] builds it from RAM).
    pub fn timeline(&self) -> Timeline { Timeline::from_counters(self.tick_counter, self.mode, self.mode_frames, self.idle) }

    pub fn to_tsv(&self, header: &str) -> String {
        let mut o = String::new();
        for l in header.lines() { let _ = writeln!(o, "# {l}"); }
        o += "# Numbers only (no raw memory); floats in Rust's shortest round-trip form, hashes FNV-1a 64 in hex.\n";
        o += "# Record types (first column):\n";
        o += "#   counter  name value\n";
        o += "#   fog      name values...   (fog: near far near_intensity far_intensity; camera: x y z; underwater_*)\n";
        o += "#   chunk    scope(G | level) id addr size fnv64 value\n";
        o += "#   moby     slot class uid state mode group update_dist visible draw_dist spin(class 13) turn(class 500)\n";
        o += "#   light    set colour_a[4] dir_a[4] colour_b[4] dir_b[4]\n";
        o += "#   point    slot colour[4] pos[4]\n";
        o += "#   tie / shrub  index located sel plist fnv64   (tie_count / shrub_count: counter rows)\n";
        for (n, v) in [("level", self.level as i64), ("tick_counter", self.tick_counter as i64), ("mode", self.mode as i64), ("mode_frames", self.mode_frames as i64),
            ("idle", self.idle as i64), ("rand_state", self.rand_state as i64), ("hp", self.hp as i64), ("tie_count", self.tie_count as i64), ("shrub_count", self.shrub_count as i64)] {
            let _ = writeln!(o, "counter\t{n}\t{v}");
        }
        let fl = |v: &[f32]| v.iter().map(|x| fmt_f(*x)).collect::<Vec<_>>().join("\t");
        let by = |v: &[u8]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("\t");
        let _ = writeln!(o, "fog\tcolor\t{}", by(&self.fog_color));
        let _ = writeln!(o, "fog\tfog\t{}", fl(&self.fog));
        let _ = writeln!(o, "fog\tcamera\t{}", fl(&self.camera));
        let _ = writeln!(o, "fog\tunderwater_tint\t{}", by(&self.underwater_tint));
        let _ = writeln!(o, "fog\tunderwater_color\t{}", by(&self.underwater_color));
        let _ = writeln!(o, "fog\tunderwater_fog\t{}", fl(&self.underwater_fog));
        for c in &self.chunks {
            let _ = writeln!(o, "chunk\t{}\t{}\t{:#x}\t{}\t{:016x}\t{}", c.level.map_or("G".into(), |l| l.to_string()), c.id, c.addr, c.size, c.hash, c.value);
        }
        for m in &self.mobys {
            let _ = writeln!(o, "moby\t{}\t{}\t{}\t{:#x}\t{:#06x}\t{}\t{}\t{}\t{}\t{}\t{}", m.slot, m.class, m.uid, m.state, m.mode, m.group, m.update_dist, m.visible, m.draw_dist,
                m.spin.map_or("-".into(), |v| v.to_string()), m.turn.map_or("-".into(), fmt_f));
        }
        for (i, x) in self.light_sets.iter().enumerate() { let _ = writeln!(o, "light\t{i}\t{}", fl(x)); }
        for (i, x) in self.point_lights.iter().enumerate() { let _ = writeln!(o, "point\t{i}\t{}", fl(x)); }
        for (kind, v) in [("tie", &self.ties), ("shrub", &self.shrubs)] {
            for t in v { let _ = writeln!(o, "{kind}\t{}\t{}\t{:#06x}\t{:#06x}\t{:016x}", t.index, t.located as u8, t.sel, t.plist, t.hash); }
        }
        o
    }

    pub fn parse_tsv(text: &str) -> Result<SpawnFacts> {
        let mut s = SpawnFacts::default();
        for (n, line) in text.lines().enumerate() {
            if line.is_empty() || line.starts_with('#') { continue; }
            let c: Vec<&str> = line.split('\t').collect();
            let ctx = || format!("line {}: {line:?}", n + 1);
            let get = |k: usize| c.get(k).copied().with_context(ctx);
            match c[0] {
                "counter" => {
                    let v: i64 = get(2)?.parse().with_context(ctx)?;
                    match get(1)? {
                        "level" => s.level = v as u32, "tick_counter" => s.tick_counter = v as u32, "mode" => s.mode = v as i32,
                        "mode_frames" => s.mode_frames = v as i32, "idle" => s.idle = v as i32, "rand_state" => s.rand_state = v as u32,
                        "hp" => s.hp = v as i32, "tie_count" => s.tie_count = v as u32, "shrub_count" => s.shrub_count = v as u32,
                        k => bail!("unknown counter {k:?} ({})", ctx()),
                    }
                }
                "fog" => {
                    let fs = || -> Result<Vec<f32>> { c[2..].iter().map(|x| parse_f(x)).collect() };
                    let bs = || -> Result<Vec<u8>> { c[2..].iter().map(|x| x.parse::<u8>().with_context(ctx)).collect() };
                    match get(1)? {
                        "color" => s.fog_color = arr(bs()?, ctx)?, "fog" => s.fog = arr(fs()?, ctx)?, "camera" => s.camera = arr(fs()?, ctx)?,
                        "underwater_tint" => s.underwater_tint = arr(bs()?, ctx)?, "underwater_color" => s.underwater_color = arr(bs()?, ctx)?,
                        "underwater_fog" => s.underwater_fog = arr(fs()?, ctx)?,
                        k => bail!("unknown fog row {k:?} ({})", ctx()),
                    }
                }
                "chunk" => s.chunks.push(Chunk {
                    level: match get(1)? { "G" => None, l => Some(l.parse().with_context(ctx)?) },
                    id: get(2)?.parse().with_context(ctx)?,
                    addr: parse_hex(get(3)?).with_context(ctx)?,
                    size: get(4)?.parse().with_context(ctx)?,
                    hash: u64::from_str_radix(get(5)?, 16).with_context(ctx)?,
                    value: c.get(6).copied().unwrap_or("").to_string(),
                }),
                "moby" => s.mobys.push(MobySlot {
                    slot: get(1)?.parse().with_context(ctx)?, class: get(2)?.parse().with_context(ctx)?, uid: get(3)?.parse().with_context(ctx)?,
                    state: parse_hex(get(4)?).with_context(ctx)? as u8, mode: parse_hex(get(5)?).with_context(ctx)? as u16, group: get(6)?.parse().with_context(ctx)?,
                    update_dist: get(7)?.parse().with_context(ctx)?, visible: get(8)?.parse().with_context(ctx)?, draw_dist: get(9)?.parse().with_context(ctx)?,
                    spin: match get(10)? { "-" => None, v => Some(v.parse().with_context(ctx)?) },
                    turn: match get(11)? { "-" => None, v => Some(parse_f(v)?) },
                }),
                "light" => s.light_sets.push(arr(c[2..].iter().map(|x| parse_f(x)).collect::<Result<Vec<_>>>()?, ctx)?),
                "point" => s.point_lights.push(arr(c[2..].iter().map(|x| parse_f(x)).collect::<Result<Vec<_>>>()?, ctx)?),
                kind @ ("tie" | "shrub") => {
                    let l = Lit {
                        index: get(1)?.parse().with_context(ctx)?, located: get(2)? == "1", sel: parse_hex(get(3)?).with_context(ctx)? as u16,
                        plist: parse_hex(get(4)?).with_context(ctx)? as u16, hash: u64::from_str_radix(get(5)?, 16).with_context(ctx)?,
                    };
                    if kind == "tie" { s.ties.push(l) } else { s.shrubs.push(l) }
                }
                k => bail!("unknown record {k:?} ({})", ctx()),
            }
        }
        ensure!(s.light_sets.len() == BANK_SETS, "{} light sets, expected {BANK_SETS}", s.light_sets.len());
        ensure!(s.point_lights.len() == POINT_LIGHT_SLOTS, "{} point lights, expected {POINT_LIGHT_SLOTS}", s.point_lights.len());
        Ok(s)
    }

    pub fn read(path: &Path) -> Result<SpawnFacts> {
        SpawnFacts::parse_tsv(&std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?)
    }
}

fn fmt_f(x: f32) -> String { if x.is_nan() { format!("nan:{:#010x}", x.to_bits()) } else { format!("{x:?}") } }
fn parse_f(s: &str) -> Result<f32> {
    match s.strip_prefix("nan:") {
        Some(h) => Ok(f32::from_bits(parse_hex(h)?)),
        None => s.parse().with_context(|| format!("bad float {s:?}")),
    }
}
fn parse_hex(s: &str) -> Result<u32> { u32::from_str_radix(s.trim_start_matches("0x"), 16).with_context(|| format!("bad hex {s:?}")) }
fn arr<T: std::fmt::Debug, const N: usize>(v: Vec<T>, ctx: impl Fn() -> String) -> Result<[T; N]> {
    let n = v.len();
    v.try_into().map_err(|_| anyhow::anyhow!("expected {N} values, got {n} ({})", ctx()))
}

// ------------------------------------------------------------------------------------------------
// The checks, on the facts (same tally names as `novalis_spawn`).

/// a. Every save chunk: the port's bytes (hashed) vs RAM's hash; play-dependent chunks that differ are not counted.
pub fn check_game_state(facts: &SpawnFacts, gs: &ns::GameStateInputs, out: &mut Out) {
    let s = &gs.state;
    let by_key: HashMap<(Option<usize>, i32), &Chunk> = facts.chunks.iter().map(|c| ((c.level, c.id), c)).collect();
    let (mut eq, mut total) = (0u64, 0u64);
    let mut unexpected = Vec::new();
    for scope in std::iter::once(None).chain((0..20).map(Some)) {
        let (descs, l) = match scope { None => (&s.tables.global, 0), Some(l) => (&s.tables.level, l as u32) };
        let sc = scope.map_or(Scope::Global, Scope::Level);
        for d in descs {
            let port = s.chunk_bytes(sc, d);
            let same = by_key.get(&(scope, d.id)).is_some_and(|c| c.addr == d.addr + l * d.size && c.size == d.size && c.hash == fnv1a(&port));
            if same { eq += 1; total += 1; } else if !ns::play_dependent(sc, d.id) { total += 1; unexpected.push(format!("{scope:?} {}", d.id)); }
        }
    }
    total += 1;
    if facts.hp == gs.session.hp { eq += 1 } else { unexpected.push("HP".into()) }
    out.line(format!("  game state: {eq}/{total} chunks equal (hashes); unexpected: {unexpected:?}"));
    out.tally("a. game state (chunks, excl. play-dependent)", eq, total);
}

/// e. rand state and tick counter.
pub fn check_rng(facts: &SpawnFacts, sim: &PortSim) -> [(String, u64, u64); 2] {
    [("e. rand state".into(), (facts.rand_state == sim.game.rng.state) as u64, 1), ("e. tick counter".into(), (sim.game.counter == facts.tick_counter as u64) as u64, 1)]
}

/// d. Fog globals vs the level settings (+ the zone at RAM's camera), and the underwater look.
pub fn check_fog(facts: &SpawnFacts, lv: &LevelData, sim: &PortSim, out: &mut Out) {
    use rc_game::fog_zones::{self, FogGlobals};
    let s = &lv.level_settings;
    let w = |o: usize| u32::from_le_bytes(s[o..o + 4].try_into().unwrap());
    let settings = FogGlobals { color: [w(0x0c) as u8, w(0x10) as u8, w(0x14) as u8], near_dist: f32::from_bits(w(0x18)), far_dist: f32::from_bits(w(0x1c)), near_intensity: f32::from_bits(w(0x20)), far_intensity: f32::from_bits(w(0x24)) };
    let ram = FogGlobals { color: facts.fog_color, near_dist: facts.fog[0], far_dist: facts.fog[1], near_intensity: facts.fog[2], far_intensity: facts.fog[3] };
    let mut expect = settings;
    if let Some((i, t)) = fog_zones::lookup(&lv.fog_zones, facts.camera) { if lv.fog_zones.zones[i].lerps_fog() { fog_zones::apply(&mut expect, &lv.fog_zones.zones[i], t); } }
    let bits = |a: &FogGlobals| [a.near_dist, a.far_dist, a.near_intensity, a.far_intensity].map(f32::to_bits);
    let ok = ram.color == expect.color && bits(&ram) == bits(&expect);
    let d = sim.look;
    let ok_u = facts.underwater_tint == d.tint && facts.underwater_color == d.fog.color && facts.underwater_fog.map(f32::to_bits) == bits(&d.fog);
    out.line(format!("  fog globals {}; underwater look {}", if ok { "MATCH" } else { "MISMATCH" }, if ok_u { "equal" } else { "DIFFERS" }));
    out.tally("d. fog globals", ok as u64, 1);
    out.tally("d. underwater look", ok_u as u64, 1);
}

/// f. The static moby table: slot j holds the same (class, spawn id); no rejected instance in RAM; the load
/// pass's draws (bolt spin bits, crate turns) on the paired slots.
pub fn check_mobys(facts: &SpawnFacts, sim: &PortSim, out: &mut Out) {
    let port = &sim.game.mobys.mobys;
    let ram = &facts.mobys;
    let pairs: Vec<usize> = (0..sim.n_static).filter(|&i| ram.get(i).is_some_and(|r| (r.class, r.uid) == (port[i].o_class, port[i].spawn_id))).collect();
    let rejected: BTreeSet<(i16, i16)> = sim.pending.iter().map(|&ii| (sim.lv.instances[ii].o_class as i16, sim.lv.instances[ii].spawn_id as i16)).collect();
    let in_ram = ram.iter().filter(|r| rejected.contains(&(r.class, r.uid))).count();
    let (mut spin_ok, mut spin_n, mut turn_ok, mut turn_n) = (0u64, 0u64, 0u64, 0u64);
    for &i in &pairs {
        let p = &port[i];
        match p.o_class {
            13 => { spin_n += 1; spin_ok += (ram[i].spin.is_some() && ram[i].spin.as_ref() == p.pvars.get(0x55)) as u64; }
            500 => { turn_n += 1; turn_ok += ram[i].turn.is_some_and(|t| t.to_bits() == p.rotation[2].to_bits()) as u64; }
            _ => {}
        }
    }
    out.line(format!("  moby slots paired {}/{}; rejected absent {}/{}; spin bits {spin_ok}/{spin_n}; crate turns {turn_ok}/{turn_n}",
        pairs.len(), sim.n_static, sim.pending.len() - in_ram, sim.pending.len()));
    out.tally("f. moby slots paired by index", pairs.len() as u64, sim.n_static as u64);
    out.tally("f. rejected instances absent from RAM", (sim.pending.len() - in_ram) as u64, sim.pending.len() as u64);
    out.tally("e. load pass: bolt spin bits", spin_ok, spin_n);
    out.tally("e. load pass: crate 500 turns", turn_ok, turn_n);
}

/// g. Tie and shrub lit colours, lit with RAM's bank and each record's selector / point list; equal per
/// instance when the palette hash matches.
pub fn check_tie_shrub(facts: &SpawnFacts, extracted: &Path, out: &mut Out) -> Result<()> {
    let inp = tsc::Inputs::load(extracted, 1)?;
    ensure!(facts.tie_count as usize == inp.ties.len(), "tie count in RAM {} != disc {}", facts.tie_count, inp.ties.len());
    ensure!(facts.shrub_count as usize == inp.shrubs.len(), "shrub count in RAM {} != disc {}", facts.shrub_count, inp.shrubs.len());
    let mut bank = LightBank { count: inp.disc_bank.count, ..Default::default() };
    for (s, v) in bank.sets.iter_mut().zip(&facts.light_sets) {
        let q = |k: usize| -> [f32; 4] { v[4 * k..4 * k + 4].try_into().unwrap() };
        *s = DirLightSet { color_a: q(0), dir_a: q(1), color_b: q(2), dir_b: q(3) };
    }
    let points: [PointLight; POINT_LIGHT_SLOTS] = std::array::from_fn(|k| { let v = &facts.point_lights[k]; PointLight { color: v[..4].try_into().unwrap(), pos: v[4..].try_into().unwrap() } });
    let flat = |c: &[[u8; 4]]| c.iter().flatten().copied().collect::<Vec<u8>>();
    let tie_classes: HashMap<i32, _> = inp.tie_classes.iter().map(|c| (c.o_class, c)).collect();
    let (mut located, mut equal, mut differ) = (0u64, 0u64, Vec::new());
    for t in &facts.ties {
        let inst = &inp.ties[t.index as usize];
        if !t.located { continue }
        located += 1;
        let Some(class) = tie_classes.get(&inst.o_class) else { differ.push(t.index); continue };
        let mut live = *inst;
        live.directional_lights = t.sel as i32;
        let tp = TiePointLights { bank: &points, list: t.plist };
        let ours = tie_light::light_tie_instance(&class.class, &live, &bank, (t.plist != 0xffff).then_some(&tp));
        if fnv1a(&flat(&ours)) == t.hash { equal += 1 } else { differ.push(t.index) }
    }
    out.line(format!("  ties: {equal}/{located} instances equal (hash); differing: {differ:?}"));
    out.tally("g. tie instances", equal, located);
    let shrub_classes: HashMap<i32, _> = inp.shrub_classes.iter().map(|c| (c.o_class, c)).collect();
    let (mut located, mut equal, mut differ) = (0u64, 0u64, Vec::new());
    for t in &facts.shrubs {
        let inst = &inp.shrubs[t.index as usize];
        if !t.located { continue }
        located += 1;
        let Some(class) = shrub_classes.get(&inst.o_class) else { differ.push(t.index); continue };
        let mut live = *inst;
        live.dir_lights = t.sel as i32;
        let sp = ShrubPointLights { bank: &points, list: t.plist };
        let ours = shrub_light::light_shrub_instance_with_points(&class.class, &live, &bank, (t.plist != 0xffff).then_some(&sp));
        if fnv1a(&flat(&ours)) == t.hash { equal += 1 } else { differ.push(t.index) }
    }
    out.line(format!("  shrubs: {equal}/{located} instances equal (hash); differing: {differ:?}"));
    out.tally("g. shrub instances", equal, located);
    Ok(())
}

/// Runs every fact check: the game state, then the port on the facts' timeline (✕ held 2 ticks, the
/// cutscene ticks from the mode counters, as the old savestate test and `compare-novalis-spawn` default to).
pub fn check_all(facts: &SpawnFacts, extracted: &Path) -> Result<Out> {
    ensure!(facts.level == 1, "the facts are for level {}, not Novalis", facts.level);
    let mut out = Out::default();
    check_tie_shrub(facts, extracted, &mut out)?;
    let gs = ns::port_game_state(extracted)?;
    check_game_state(facts, &gs, &mut out);
    let tl = facts.timeline();
    let lv = LevelData::load(extracted, 1)?;
    let opt = crate::port_sim::SimOptions { cutscene_ticks: tl.ticks.saturating_sub(tl.mode_frames.max(0) as u64), ..Default::default() };
    let sim = ns::run_port_hold(&lv, &tl, 2, &opt)?;
    for (n, ok, total) in check_rng(facts, &sim) { out.tally(&n, ok, total); }
    check_fog(facts, &lv, &sim, &mut out);
    check_mobys(facts, &sim, &mut out);
    Ok(out)
}

/// One disagreeing tally: (name, (matching, total) in `a`, the same in `b`).
pub type Disagreement = (String, (u64, u64), (u64, u64));

/// The tallies of `a` whose name also appears in `b`, where they disagree.
pub fn disagreements(a: &Out, b: &Out) -> Vec<Disagreement> {
    let bm: BTreeMap<&str, (u64, u64)> = b.tallies.iter().map(|t| (t.0.as_str(), (t.1, t.2))).collect();
    a.tallies.iter().filter_map(|t| bm.get(t.0.as_str()).filter(|&&v| v != (t.1, t.2)).map(|&v| (t.0.clone(), (t.1, t.2), v))).collect()
}
