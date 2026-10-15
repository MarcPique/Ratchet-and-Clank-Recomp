//! The level's water as the moby system keeps it ([`WaterWorld`], `Services::water`): the ripple module's state
//! (`0x1612d0` patch table, `0x1612d4` bounds, `0x1612d8` count: [`RippleSim`]), the flat water plane
//! (`0x1612dc..0x1612ec`: [`WaterPlane`]), the underwater look the managers write (`0x161200..0x161214`), and the
//! level data they read ([`LevelWaterData`], loaded once per level by code identity: no table is keyed by level
//! number). [`WaterWorld::water_height`] is `SetWaterLevel` 0x26ed38 without its last fallback (the caller's hit z).
//!
//! **Where the data comes from.** Every table is a data address of a manager's (or the module's) code on the level
//! that reversed it (the reference: level 01 for 751 and the module, 05 / 07 / 11 / 12 / 13 for the patch managers),
//! found on the loaded level through [`Relocation::data`] from that code's copy (docs/plan/level_generalisation.md
//! W2). A level whose overlay has no copy of a manager gets no data for it, and the port does nothing there.

use super::managers::PORTS;
use super::RippleSim;
use crate::fog_zones::{FogGlobals, UnderwaterLook};
use rc_formats::level_overlay::{LevelOverlay, Relocation};
use rc_formats::water::{self as wf, Cuboid, Overlay, PatchRecord, RippleModule, RippleTables};
use std::collections::HashMap;
use std::sync::Arc;

/// The flat water plane `0x1612dc` (enable), `0x1612e0` (centre, a quadword), `0x1612e8` (height), `0x1612ec`
/// (radius): `SetWaterLevel` returns the height for a point within the radius (2-D, `VecDistance2`) and within 0.5
/// of the height. Only level 05's class 982 sets it ([`managers`]); `ResetDrawGlobals` clears the enable.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WaterPlane {
    pub on: bool,
    pub centre: [f32; 3],
    pub z: f32,
    pub radius: f32,
}

/// One manager port's tables on the loaded level (the reference level's labels, relocated).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PortData {
    /// Index into [`PORTS`].
    pub port: usize,
    /// The patch records as stored (the table the manager's init hands `RipplePatchesInit`).
    pub patches: Vec<PatchRecord>,
    /// The per-patch sub-block masks the manager copies to +0x1e (u16, stride [`super::managers::Port::mask_stride`]).
    pub masks: Vec<u16>,
    /// Other data the manager reads, one word per reference address (a table's words at `label + 4·k`): the `.lit`
    /// globals (`$gp`) and tables as stored.
    pub words: HashMap<u32, u32>,
}

impl PortData {
    /// The word at reference address `label` (0 when the level has none).
    pub fn word(&self, label: u32) -> u32 { self.words.get(&label).copied().unwrap_or(0) }
}

/// The level's water data (read-only once loaded).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LevelWaterData {
    /// The ripple module's constant tables (None: the level has no copy of the module).
    pub module: Option<RippleModule>,
    /// Directional light set 0, light A direction x / y (the managers' `FastArcTan(0x180350, 0x180354)`).
    pub light_xy: [f32; 2],
    /// The gameplay cuboids (751's zones, 982's volume, level 12's camera gate).
    pub cuboids: Vec<Cuboid>,
    /// 751's tables and its instance's zone cuboid indices.
    pub z751: Option<(RippleTables, Vec<i32>)>,
    /// The patch managers present on the level.
    pub ports: Vec<PortData>,
    /// `0x161200..0x161214` as stored: the underwater tint and alternate fog before any manager writes them.
    pub look: UnderwaterLook,
    /// The classes of the level's table that run a ripple manager (751 or a patch manager), in table order.
    pub manager_classes: Vec<i32>,
    /// The sea ports present on the level and their data ([`super::sea`]).
    pub sea: Vec<super::sea::SeaPortData>,
}

/// Reads the underwater look bytes (tint RGBA, fog RGB, pad, near, far, near / far intensity) at `a`.
fn read_look(ov: &Overlay, a: u32) -> Option<UnderwaterLook> {
    let b = ov.read(a, 0x18).ok()?;
    let f = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    Some(UnderwaterLook {
        tint: [b[0], b[1], b[2], b[3]],
        fog: FogGlobals { color: [b[4], b[5], b[6]], near_dist: f(8), far_dist: f(0xc), near_intensity: f(0x10), far_intensity: f(0x14) },
    })
}

impl LevelWaterData {
    /// The level's water data: `overlay` (the raw overlay lump) parsed as `target`, `reference(level)` the reference
    /// overlays (level 01 and the patch managers' levels), `gameplay` the decompressed gameplay file.
    pub fn load(overlay: &[u8], target: &LevelOverlay, reference: &dyn Fn(u32) -> Option<Arc<LevelOverlay>>, gameplay: &[u8]) -> Result<LevelWaterData, rc_formats::FormatError> {
        let ov = Overlay::parse(overlay)?;
        let mut data = LevelWaterData { cuboids: wf::parse_cuboids(gameplay)?, ..Default::default() };
        let bank = rc_formats::tfrag_light::parse_light_bank(gameplay)?;
        data.light_xy = [bank.sets[0].dir_a[0], bank.sets[0].dir_a[1]];
        let instances = rc_formats::gameplay::parse_moby_instances(gameplay)?;
        let pvars = rc_formats::gameplay::parse_pvars(gameplay)?;
        let vtbl = target.vtbl();
        let mut refs: HashMap<u32, Arc<LevelOverlay>> = HashMap::new();
        let extra_refs = [super::sea::gaspar_ref::LEVEL];
        for l in std::iter::once(1).chain(PORTS.iter().map(|p| p.level)).chain(super::sea::PORTS.iter().map(|p| p.level)).chain(extra_refs) {
            if refs.contains_key(&l) { continue; }
            if let Some(o) = reference(l) { refs.insert(l, o); }
        }
        let rel_of = |l: u32| refs.get(&l).map(|o| Relocation::new(o, target));
        if let Some(r01) = rel_of(1) {
            // The underwater tint the frame draw reads (`DrawDebugProfiler` forms 0x161200) and the module tables.
            data.look = r01.data(0x161200).and_then(|a| read_look(&ov, a)).unwrap_or_default();
            if let Some(m) = wf::RippleModuleAddrs::locate(&r01) { data.module = Some(wf::parse_ripple_module(&ov, &m)?); }
            if let (Some(a), Some(m)) = (wf::Ripple751Addrs::locate(&r01), wf::RippleModuleAddrs::locate(&r01)) {
                let t = wf::parse_ripple_tables(&ov, &a, &m)?;
                // The zone cuboids: the pvar of the (first) instance whose class runs 751.
                let f751 = r01.func(wf::RIPPLE_751_FN);
                let classes: Vec<i32> = vtbl.iter().filter(|e| Some(e.update) == f751).map(|e| e.o_class).collect();
                let zones = instances
                    .iter()
                    .find(|m| classes.contains(&m.o_class))
                    .and_then(|m| m.pvar(&pvars))
                    .map(|p| wf::ripple_zone_cuboids(p, t.zones.len()))
                    .transpose()?
                    .unwrap_or_default();
                data.z751 = Some((t, zones));
                data.manager_classes.extend(classes);
            }
        }
        for (i, p) in PORTS.iter().enumerate().skip(1) {
            let Some(rel) = rel_of(p.level) else { continue };
            if rel.func(p.func).is_none() { continue; }
            let f = rel.func(p.func);
            if p.patches.is_some() { data.manager_classes.extend(vtbl.iter().filter(|e| Some(e.update) == f).map(|e| e.o_class)); }
            let mut pd = PortData { port: i, ..Default::default() };
            let at = |label: u32| rel.data(label);
            if let Some((label, n)) = p.patches {
                let base = at(label).ok_or_else(|| rc_formats::FormatError::Invalid(format!("water port {i}: patch table {label:#x} not found")))?;
                pd.patches = wf::parse_patches(&ov, base, n)?;
            }
            if let (Some(label), Some((_, n))) = (p.masks, p.patches) {
                let base = at(label).ok_or_else(|| rc_formats::FormatError::Invalid(format!("water port {i}: masks {label:#x} not found")))?;
                pd.masks = (0..n as u32).map(|k| ov.buf(base + p.mask_stride * k, 2).and_then(|b| b.u16(0))).collect::<Result<_, _>>()?;
            }
            for &(label, n) in p.words {
                let Some(a) = at(label) else { continue };
                for k in 0..n as u32 { pd.words.insert(label + 4 * k, ov.u32(a + 4 * k)?); }
            }
            data.ports.push(pd);
        }
        data.sea = super::sea::load(&ov, target, &rel_of);
        Ok(data)
    }

    /// The patch records of the level's ripple module as its manager's init hands them over (751's table, else the
    /// first patch manager's), None without a module user.
    pub fn patch_records(&self) -> Option<&[PatchRecord]> {
        if let Some((t, _)) = &self.z751 { return Some(&t.patches); }
        self.ports.iter().find(|p| !p.patches.is_empty()).map(|p| p.patches.as_slice())
    }

    /// The data of port `port` (None: not on this level).
    pub fn port(&self, port: usize) -> Option<&PortData> { self.ports.iter().find(|p| p.port == port) }
}

/// A store into the underwater flag 0x167494 made by the game's code during a tick, outside the camera update's test
/// `0x20e9f0` (which runs after it on the stored value: a test with no water hit within ±0.75 keeps it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnderwaterStore {
    /// `0x167494 = 0`: the hero's surface jump out of the water (`0x2406b0`, state 6), the level start (`0x20ef58`).
    Off,
    /// `HeroTeleport` 0x2368e0: `0x167494 = (0x1413dc == 0x11)`, Ratchet's state group after the teleport's `SetState`
    /// (0x11: under water).
    HeroGroup,
}

/// The water state of the moby system (see the module docs).
#[derive(Clone, Debug, Default)]
pub struct WaterWorld {
    pub data: Option<Arc<LevelWaterData>>,
    /// The ripple module (None until a manager's init ran: `0x1612d8` = 0).
    pub sim: Option<RippleSim>,
    pub plane: WaterPlane,
    /// `0x161200..0x161214` as the managers leave it (the underwater tint and alternate fog).
    pub look: UnderwaterLook,
    /// The module's FIX globals `0x1cad3c` / `0x1cad3d` (env, water) the 07 / 13 managers copy into every patch.
    pub module_fix: [u8; 2],
    /// The level's patch table as stored (`LevelWaterData::patch_records`), which the managers' mobys write before
    /// the module's init builds [`WaterWorld::sim`] from it.
    pub records: Vec<PatchRecord>,
    /// The patch managers' `.lit` globals (counters, flood stage, drown count, …) by reference label, from the stored
    /// values ([`PortData::words`]) on the first use.
    pub globals: HashMap<u32, u32>,
    /// The sea ports' state ([`super::sea`]).
    pub sea: super::sea::SeaState,
    /// The last store into the underwater flag 0x167494 outside the camera's test, with the tick counter it was made
    /// on ([`UnderwaterStore`]); the engine's camera-side flag (`rc-engine` `fog_state`) applies each store once,
    /// before its test.
    pub underwater_store: Option<(u64, UnderwaterStore)>,
    /// The level fog globals 0x15f444..0x15f454 as the moby code reads them: the engine's copy (`rc-engine` `fog_state`
    /// mirrors it every frame after its fog zones; set at the level load from the level settings), with this tick's
    /// class stores on top. None: no engine (tests).
    pub fog: Option<FogGlobals>,
    /// The last class store into 0x15f444..0x15f454 ([`WaterWorld::store_fog`]), with the tick counter it was made on;
    /// the engine applies each store once, before its fog zones.
    pub fog_store: Option<(u64, FogGlobals)>,
}

impl WaterWorld {
    /// A class's store into the level fog globals 0x15f444..0x15f454 on tick `tick` (Rilgar's 841).
    pub fn store_fog(&mut self, tick: u64, g: FogGlobals) {
        self.fog = Some(g);
        self.fog_store = Some((tick, g));
    }

    /// The water of a level with `data`.
    pub fn new(data: LevelWaterData) -> WaterWorld {
        let (look, records) = (data.look, data.patch_records().map(<[_]>::to_vec).unwrap_or_default());
        WaterWorld { data: Some(Arc::new(data)), look, records, ..Default::default() }
    }

    /// `SetWaterLevel` 0x26ed38 without the caller's fallback: with a ripple module (`0x1612d8 ≠ 0`) the active patch
    /// under `p` (`RippleHeightQuery` 0x2b8910), else the flat plane when `p` is within its radius (2-D) and within 0.5
    /// of its height, else None (the caller uses its hit's z).
    pub fn water_height(&self, p: [f32; 3]) -> Option<f32> {
        if let Some(h) = self.sim.as_ref().and_then(|s| s.patch_height(p[0], p[1], p[2])) { return Some(h); }
        let pl = &self.plane;
        if pl.on && (p[2] - pl.z).abs() < 0.5 {
            let (dx, dy) = (p[0] - pl.centre[0], p[1] - pl.centre[1]);
            if (dx * dx + dy * dy).sqrt() < pl.radius { return Some(pl.z); }
        }
        None
    }

    /// `RippleDisturb(x, y, r, amp, 0x1612d0, 0x1612d8, additive)` 0x2b82a8 over every patch of the module (the hero's
    /// splashes, the bomb's water entry); nothing without a module.
    pub fn disturb(&mut self, x: f32, y: f32, r: f32, amp: f32, additive: bool) {
        use crate::ps2v::Pf;
        if let Some(sim) = self.sim.as_mut() {
            let n = sim.patches.len();
            sim.disturb(Pf::f(x), Pf::f(y), Pf::f(r), Pf::f(amp), 0..n, additive);
        }
    }

    /// `*(*0x1612d0 + i·0x1190 + 8)`: record +0x08 of patch `i`, its water level (the module's patch after the init,
    /// else the stored record the managers' mobys write before it); None without a patch `i`. The amoeboids' fall-out
    /// rule on 05 / 11 reads it (`moby_update::classes::amoeboid`, pvar +0x258).
    pub fn patch_level(&self, i: usize) -> Option<f32> {
        match &self.sim {
            Some(s) => s.patches.get(i).map(|p| p.rec.centre[2]),
            None => self.records.get(i).map(|r| r.centre[2]),
        }
    }

    /// A manager's `.lit` global at reference label `label` (its stored value until written).
    pub fn global(&self, port: usize, label: u32) -> u32 {
        if let Some(&v) = self.globals.get(&label) { return v; }
        self.data.as_ref().and_then(|d| d.port(port)).map_or(0, |p| p.word(label))
    }

    pub fn set_global(&mut self, label: u32, v: u32) { self.globals.insert(label, v); }
}
