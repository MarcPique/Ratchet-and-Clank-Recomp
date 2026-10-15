//! Level-code water: the strip scroll / wobble / bob state (classes 676, 678, 761, 1225) and the ripple
//! patch simulation (class 751 + the engine module at level01 0x2b7a48..0x2b96c0). Pure logic on the PS2
//! float model ([`crate::ps2v::Pf`]); the tables come from [`rc_formats::water`], the drawing is
//! rc-engine's `water_render`. Spec and every address: docs/plan/world_animation.md §1–§3.
//!
//! **Strips** (`FUN_002b96e0` per draw call, `FUN_00262618` per vertex): [`ScrollState::tick`] advances the
//! two layer offsets and the wobble phase, [`wobble`] is the per-vertex UV offset `A·(sin θ, cos θ)` with
//! `θ = (((hash·64 + phase)·8) & 0xfff)/4096·2π − π` and `hash = ((bits(x) + bits(y)) >> 16) & 0xff`
//! (only `hash & 7` survives the mask: eight phase groups), [`bob`] is the 761/1225 z blend.
//! The phase step at descriptor +0x1c is **1** on every Novalis strip (not 0 as the doc said), so the
//! wobble turns once every 512 draws.
//!
//! **Ripples**: [`RippleSim`] holds the 21 patch records with their three height buffers, reproduces the
//! init (`0x2b7a48` + 751 init `0x2fd0e8`), the per-tick 751 update (zone activation with random drops, the
//! sim clock `0x2b7fe0`, the step `0x2b7f30`/`0x261c10`/`0x262038`, the lerp `0x261d78`), the disturbance
//! `0x2b82a8`, the height query `0x2b8910`/`0x2b7d28` and the draw-side per-vertex data (`0x2b8c08` grid,
//! `0x261df8` normal / grey / sphere-map UV, `0x2b80d0` water UV).
//!
//! Height buffer layout (pinned from `0x261c10`, the SPR copy the kernels read): 16×16 cells (row stride
//! 0x40) then halos: +0x400 row −1, +0x440 column −1, +0x480 row 16, +0x4c0 column 16, +0x500 row 17,
//! +0x540 column 17, +0x580 corners (−1,−1), (−1,16), (16,−1), (16,16), (16,17), (17,16). Here a buffer is
//! the logical 19×19 grid of rows/columns −1..=17 ([`gi`]); positions the game has no slot for stay 0 and
//! are never read. Which neighbour's halo a cell feeds (`0x262038` exchange = `0x2b82a8` per-cell writes):
//! link +0x0c (first-row side): rows 0/1 → its rows 16/17; +0x0d (first-column side): columns 0/1 → its
//! columns 16/17; +0x0e: row 15 → its row −1; +0x0f: column 15 → its column −1; +0x10: (0,0), (0,1), (1,0)
//! → its (16,16), (16,17), (17,16); +0x11: (0,15) → (16,−1); +0x12: (15,0) → (−1,16); +0x13: (15,15) → (−1,−1).
//!
//! The lerp (`0x261d78`, pinned): `out = prev·(1 − f) + cur·f` with prev = buffer [0x16121c] and cur =
//! [0x161218] (`vmulaw ACC, prev, (1−f); vmaddx out, cur, f`). The step writes the new state into prev's
//! buffer and then swaps, so after a step `cur` is the newest state and the draw lags one step behind it:
//! the step tick draws `prev` (t = 0), the next **eight** ticks draw lerp t = 0.2, 0.3 … 0.9: the step fires
//! when `f ≥ 1 − 0.1`, and with the FPU's truncating adds `1 − 0.1` = 0x3f666667 while the accumulator
//! (0.1, then + 0.1 per tick) reaches only 0x3f666664 on the eighth lerp tick, so the period is **9 ticks**
//! (IEEE round-to-nearest would give 8).
//!
//! **RNG**: the game draws the random drops from its single `rand` stream in moby-update order, so
//! [`RippleSim::new`] (751's load-pass init) and [`RippleSim::tick`] (its update) take the caller's stream
//! (the engine passes the game's one stream from the moby scheduler, at 751's place in the run order).

pub mod managers;
pub mod sea;
pub mod world;

use crate::ps2v::{self, Pf, F};
use crate::rng::Rng;
use rc_formats::moby_light::vu0_sin_cos;
use rc_formats::tfrag_light::ps2;
use rc_formats::water::{Cuboid, PatchRecord, RippleTables, StripDescriptor, SUB_STRIP_LEN};

const ONE: Pf = Pf::ONE;
const NEG_ONE: Pf = Pf::b(0xbf80_0000);
/// 0x40c90fdb / 0x40490fdb: 2π and π as `lui/ori` in `0x262618`.
const TWO_PI: F = 0x40c9_0fdb;
const PI: F = 0x4049_0fdb;

fn sin(a: Pf) -> Pf { Pf(vu0_sin_cos(a.0).0) }
fn cos(a: Pf) -> Pf { Pf(vu0_sin_cos(a.0).1) }

// =================================================================================================
// Strips

/// Runtime fields of one strip descriptor (+0x40..+0x50).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ScrollState {
    /// +0x40/+0x44: layer-1 offset (u, v); +0x48/+0x4c: layer 2.
    pub s1: [Pf; 2],
    pub s2: [Pf; 2],
    /// +0x50 (24 bits).
    pub phase: u32,
}

/// One scroll lane: `x > 1 → x − 1`, else `x < −1 → x + 1` (`c.lt.s` both ways, once).
fn wrap1(x: Pf) -> Pf {
    if ONE < x { x - ONE } else if x < NEG_ONE { x + ONE } else { x }
}

impl ScrollState {
    /// The stored runtime fields (all 0 on the disc).
    pub fn new(d: &StripDescriptor) -> Self {
        ScrollState { s1: [Pf::f(d.scroll[0]), Pf::f(d.scroll[1])], s2: [Pf::f(d.scroll[2]), Pf::f(d.scroll[3])], phase: d.phase as u32 & 0xff_ffff }
    }

    /// One `FUN_002b96e0` call for this strip (every draw, before culling): `s += dir·speed` per lane
    /// (`mul.s` then `add.s`), each lane wrapped once into ±1, then `phase = (phase + step) & 0xffffff`.
    pub fn tick(&mut self, d: &StripDescriptor) {
        let (sp1, sp2) = (Pf::f(d.speed[0]), Pf::f(d.speed[1]));
        let a = [self.s1[0] + Pf::f(d.dir[0][0]) * sp1, self.s1[1] + Pf::f(d.dir[0][1]) * sp1];
        self.s1 = a.map(wrap1);
        let b = [self.s2[0] + Pf::f(d.dir[1][0]) * sp2, self.s2[1] + Pf::f(d.dir[1][1]) * sp2];
        self.s2 = b.map(wrap1);
        self.phase = self.phase.wrapping_add(d.phase_step as u32) & 0xff_ffff;
    }
}

/// The per-vertex hash of `0x262618`: `((bits(x) + bits(y)) >> 16) & 0xff` on the raw words.
pub fn wobble_hash(x: f32, y: f32) -> u32 { (x.to_bits().wrapping_add(y.to_bits()) >> 16) & 0xff }

/// θ of a vertex: `k = ((hash·64 + phase)·8) & 0xfff`, `vitof12`, `·2π`, `−π` (VU0).
pub fn wobble_angle(hash: u32, phase: u32) -> Pf {
    let k = ((hash << 6).wrapping_add(phase) << 3) & 0xfff;
    Pf(ps2::sub(ps2::mul(ps2::itof12(k as i32), TWO_PI), PI))
}

/// The wobble `w = A·(sin θ, cos θ)` (`vcallms 0x192` / `0x190`, then `vmulz.xy` by +0x18).
pub fn wobble(hash: u32, phase: u32, amp: f32) -> [Pf; 2] {
    let (s, c) = vu0_sin_cos(wobble_angle(hash, phase).0);
    let a = Pf::f(amp);
    [Pf(s) * a, Pf(c) * a]
}

/// The eight distinct wobble offsets of a strip at `phase` (index = `hash & 7`).
pub fn wobble_table(phase: u32, amp: f32) -> [[f32; 2]; 8] {
    std::array::from_fn(|g| wobble(g as u32, phase, amp).map(Pf::to_f32))
}

/// Both layers' UV of a vertex as the VU0 computes them: `uv1 = (uv + s1) + w`, `uv2 = (uv + s2) − w`.
/// (The shader repeats this in GPU f32; this is the reference.)
pub fn strip_uv(base: [f32; 2], s: &ScrollState, w: [Pf; 2]) -> [[f32; 2]; 2] {
    let b = base.map(Pf::f);
    let uv1 = [(b[0] + s.s1[0]) + w[0], (b[1] + s.s1[1]) + w[1]];
    let uv2 = [(b[0] + s.s2[0]) - w[0], (b[1] + s.s2[1]) - w[1]];
    [uv1.map(Pf::to_f32), uv2.map(Pf::to_f32)]
}

/// The 761 / 1225 z blend for frame counter `t` (0x15f5cc): `0.5 + 0.5·fast_sin(((t & 63) − 32)·π/32)`
/// (`cvt.s.w`, `sub.s` 32.0, `mul.s` 0x3dc90fdb, VU0 sine, `mul.s` 0.5, `add.s` 0.5).
pub fn bob(t: u32) -> Pf {
    let x = (Pf::from_i32((t & 63) as i32) - Pf::b(0x4200_0000)) * Pf::b(0x3dc9_0fdb);
    let half = Pf::b(0x3f00_0000);
    sin(x) * half + half
}

/// `z = z0·w + z1·(1 − w)` (`vsubx.w`, `vmulx`, `vmulw`, `vaddy`).
pub fn blend_z(z0: f32, z1: f32, w: Pf) -> f32 { (Pf::f(z0) * w + Pf::f(z1) * (ONE - w)).to_f32() }

/// 761 init (update `0x2feb58`, first call, guarded by 0x161ce0): every vertex `z0 = z1 = z0 − 0.45`, then
/// per rung pair the two vertices get opposite ±0.05 on z0 / z1, alternating per pair and per strip.
pub fn init_761(strips: &mut [Vec<[f32; 4]>]) {
    let (d045, d005) = (Pf::b(0x3ee6_6666), Pf::b(0x3d4c_cccd));
    for s in strips.iter_mut() {
        for v in s.iter_mut() {
            let z = (Pf::f(v[2]) - d045).to_f32();
            v[2] = z;
            v[3] = z;
        }
    }
    let mut flip = true;
    for s in strips.iter_mut() {
        flip = !flip;
        let mut i = 0;
        while i < s.len() {
            let (a, b) = if flip { (d005, -d005) } else { (-d005, d005) };
            // v0: z0 += a, z1 −= a; v1: z0 −= a, z1 += a (`add.s`/`sub.s` of 0.05 as decoded).
            let v0 = s[i];
            s[i][2] = (Pf::f(v0[2]) + a).to_f32();
            s[i][3] = (Pf::f(v0[3]) + b).to_f32();
            if i + 1 < s.len() {
                let v1 = s[i + 1];
                s[i + 1][2] = (Pf::f(v1[2]) + b).to_f32();
                s[i + 1][3] = (Pf::f(v1[3]) + a).to_f32();
            }
            i += 2;
            flip = !flip;
        }
    }
}

// =================================================================================================
// Ripples

/// Logical grid side: rows / columns −1..=17.
pub const G: usize = 19;
/// Index of row `r`, column `c` (both −1..=17) in a logical height buffer.
#[inline]
pub const fn gi(r: i32, c: i32) -> usize { ((r + 1) as usize) * G + (c + 1) as usize }
pub type Heights = [Pf; G * G];

/// Vertex grid side (17×17, `0x2b8c08`).
pub const VG: usize = 17;

/// 751 init values of the module constants 0x1cad00.. (`0x2fd0e8`).
#[derive(Clone, Copy, Debug)]
pub struct RippleConsts {
    /// 0x1cad00/04: patch size (16, 16); 0x1cad08/0c: origin offset (−8, −8); 0x1cad10/14: cell (1, 1).
    pub origin: [Pf; 2],
    pub cell: [Pf; 2],
    pub size: [Pf; 2],
    /// 0x1cad18: sphere-map scale 0.9.
    pub refl: Pf,
    /// 0x1cad1c: damping threshold (0.16, not written by the init: the disc value).
    pub damp: Pf,
    /// 0x1cad20: clock step 0.1.
    pub step: Pf,
}

/// One patch at run time.
#[derive(Clone, Debug)]
pub struct Patch {
    pub rec: PatchRecord,
    pub centre: [Pf; 3],
    /// +0x1e.
    pub mask: u16,
    /// +0x20/+0x24 scroll, +0x28/+0x2c speed.
    pub scroll: [Pf; 2],
    pub speed: [Pf; 2],
    /// +0x30/+0x34 θ0/θ1, +0x38/+0x3c steps, +0x40 radius.
    pub theta: [Pf; 2],
    pub dtheta: [Pf; 2],
    pub radius: Pf,
    /// +0x44.
    pub pins: u32,
    /// +0x50: the three height buffers.
    pub buf: [Heights; 3],
    /// Bounds table 0x1fa6c0 entry (`0x2b7c68`): min x, y, z in 1/1024 units and the x/y extent.
    pub bounds: [i32; 3],
    pub extent: [i32; 2],
}

impl Patch {
    /// `0x2b7c68(patch, bounds)`: the bounds entry from the centre as it is now: `trunc((x + ox)·1024)`,
    /// `trunc((y + oy)·1024)`, `trunc((z − 1)·1024)`, extents `trunc(size·1024)` (u16).
    pub fn set_bounds(&mut self, c: &RippleConsts) {
        let k1024 = Pf::b(0x4480_0000);
        let centre = self.centre;
        self.bounds = [
            ((centre[0] + c.origin[0]) * k1024).to_i32(),
            ((centre[1] + c.origin[1]) * k1024).to_i32(),
            ((centre[2] - ONE) * k1024).to_i32(),
        ];
        let ext = (c.size[0] * k1024).to_i32() & 0xffff;
        self.extent = [ext, ext];
    }

    /// Record +0x08: the patch's water level (a patch manager writes its moby's z here every tick).
    pub fn set_z(&mut self, z: f32) {
        self.centre[2] = Pf::f(z);
        self.rec.centre[2] = z;
    }
}

/// Per-vertex draw data of one patch (`0x261df8` + `0x2b8c08` + the pin overrides).
#[derive(Clone, Debug)]
pub struct PatchVerts {
    /// Game-space position, row-major 17×17.
    pub pos: Vec<[f32; 3]>,
    /// Vertex RGBA (grey, A = 0).
    pub rgba: Vec<u32>,
    /// Sphere-map UV (pass 2).
    pub env_uv: Vec<[f32; 2]>,
}

/// What a tick of the 751 update did (for the renderer / stats).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RippleTickInfo {
    /// Highest zone index the camera is in (`iStack_10c`), −1 for none: picks the underwater colours.
    pub zone: i32,
    /// This tick ran a step (else a lerp).
    pub stepped: bool,
    pub drops: u32,
    /// Drips (class 787) spawned this tick (or, without a moby system, the spawn's draws made).
    pub drips: u32,
    /// Zone-5 mist puffs (type 56) spawned this tick.
    pub mist: u32,
}

/// The whole ripple module state.
#[derive(Clone, Debug)]
pub struct RippleSim {
    pub patches: Vec<Patch>,
    pub consts: RippleConsts,
    /// 0x161218 (newest), 0x16121c (previous), 0x161220 (drawn) buffer indices, 0x161224 the clock.
    pub cur: usize,
    pub prev: usize,
    pub render: usize,
    pub clock: Pf,
    /// 0x1cad30: the light direction `(cos a, sin a, −1)·0.57735`.
    pub light: [Pf; 3],
    pub zones: Vec<rc_formats::water::RippleZone>,
    pub zone_masks: Vec<u16>,
    /// Cuboid index per zone (751 pvar; −1 = none).
    pub zone_cuboids: Vec<i32>,
    pub uv_base: Vec<[Pf; 2]>,
    pub uv_select: Vec<u32>,
    pub strip_order: Vec<u16>,
    /// gp−0x4f7c: ticks until the next drip.
    pub drip_timer: i32,
    /// 0x1fa650: the five drip start points (x, y, z, w).
    pub drip_points: Vec<[f32; 4]>,
    /// 0x1fa6a0: the zone-5 foam ring's row per tick (`counter % 20`), 20 bytes.
    pub mist_rows: Vec<u8>,
    /// What the last [`RippleSim::tick`] did.
    pub last: RippleTickInfo,
}

/// Cells a written cell is mirrored to: (link slot, source (row, col) predicate → destination).
fn mirrors(r: i32, c: i32, mut f: impl FnMut(usize, i32, i32)) {
    if r == 0 { f(0, 16, c); }
    if r == 1 { f(0, 17, c); }
    if c == 0 { f(1, r, 16); }
    if c == 1 { f(1, r, 17); }
    if r == 15 { f(2, -1, c); }
    if c == 15 { f(3, r, -1); }
    match (r, c) {
        (0, 0) => f(4, 16, 16),
        (0, 1) => f(4, 16, 17),
        (1, 0) => f(4, 17, 16),
        _ => {}
    }
    if (r, c) == (0, 15) { f(5, 16, -1); }
    if (r, c) == (15, 0) { f(6, -1, 16); }
    if (r, c) == (15, 15) { f(7, -1, -1); }
}

/// The `0x274820` cuboid test: `l = inv·(p − pos)` (VU0, `0x2211b8` + `0x2215e0`), inside when every lane
/// satisfies `−1 ≤ l ≤ 1` (`c.le.s`).
pub fn cuboid_contains(c: &Cuboid, p: [f32; 3]) -> bool {
    let d = [0, 1, 2].map(|k| ps2::sub(Pf::f(p[k]).0, Pf::f(c.matrix[3][k]).0));
    let row = |i: usize| c.inverse[i].map(|v| Pf::f(v).0);
    let (r0, r1, r2) = (row(0), row(1), row(2));
    let l: [Pf; 3] = std::array::from_fn(|k| {
        let acc = ps2::mul(r0[k], d[0]);
        let acc = ps2::add(acc, ps2::mul(r1[k], d[1]));
        Pf(ps2::add(acc, ps2::mul(r2[k], d[2])))
    });
    l.iter().all(|&v| NEG_ONE <= v && v <= ONE)
}

impl RippleSim {
    /// Level init: `0x2b7a48(0x1e34c0, 21)` then the rest of 751 init `0x2fd0e8` (constants, light,
    /// bounds, patch 13–16 speeds, patch 15 pin, two random drops per patch, drawn from `rng`). `light_dir_xy`
    /// = directional light set 0, light A direction x/y (0x180350/0x180354).
    pub fn new(t: &RippleTables, zone_cuboids: Vec<i32>, light_dir_xy: [f32; 2], rng: &mut Rng) -> Self {
        let mut sim = RippleSim::module_init(&t.patches, &t.module(), light_dir_xy);
        sim.zones = t.zones.clone();
        sim.zone_masks = t.zone_masks.clone();
        sim.zone_cuboids = zone_cuboids;
        sim.mist_rows = t.mist_rows.clone();
        sim.drip_points = t.drips.clone();
        // 751 init overrides (Novalis): patches 13–16 scroll along u only, patch 15 pins its first column.
        if sim.patches.len() > 16 {
            for p in 13..=16 { sim.patches[p].speed = [Pf::b(0xbad1_b717), Pf::ZERO]; }
            sim.patches[15].pins = 8;
        }
        sim.init_drops(rng);
        sim
    }

    /// The part every ripple manager's init shares (751 `0x2fd0e8`, and the patch managers of levels 05 / 07 / 11 /
    /// 12 / 13): `RipplePatchesInit(table, n)` 0x2b7a48 (runtime fields; the heights zeroed), the module constants
    /// 0x1cad00.. = (16, 16, −8, −8, 1, 1, 0.9, ·, 0.1) (the disc's +0x1c damping threshold kept), the light
    /// `(cos a, sin a, −1)·0.57735` with `a = FastArcTan(light set 0 A.x, A.y)`, and every patch's bounds
    /// (`0x2b7c68`). No zones and no drops (the managers that make them add them).
    pub fn module_init(records: &[PatchRecord], m: &rc_formats::water::RippleModule, light_dir_xy: [f32; 2]) -> Self {
        let consts = RippleConsts {
            size: [Pf::b(0x4180_0000); 2],
            origin: [Pf::b(0xc100_0000); 2],
            cell: [ONE; 2],
            refl: Pf::b(0x3f66_6666),
            damp: Pf::f(m.consts[7]),
            step: Pf::b(0x3dcc_cccd),
        };
        let patches = records
            .iter()
            .map(|rec| {
                let mut p = Patch {
                    rec: rec.clone(),
                    centre: rec.centre.map(Pf::f),
                    mask: rec.mask,
                    scroll: [Pf::ZERO; 2],
                    speed: [Pf::b(0xbad1_b717); 2],
                    theta: [Pf::b(0x3fc9_0fdb), Pf::b(0xbfc9_0fdb)],
                    dtheta: [Pf::b(0x3ca3_d70a), Pf::b(0xbcfd_f3b6)],
                    radius: Pf::b(0x3cf5_c28f),
                    pins: 0,
                    buf: [[Pf::ZERO; G * G]; 3],
                    bounds: [0; 3],
                    extent: [0; 2],
                };
                p.set_bounds(&consts);
                p
            })
            .collect::<Vec<_>>();
        let a = crate::pad::fast_arctan(Pf::f(light_dir_xy[0]), Pf::f(light_dir_xy[1]));
        let k = Pf::b(0x3f13_cd36);
        let light = [cos(a) * k, sin(a) * k, Pf::b(0xbf13_cd36)];
        RippleSim {
            patches,
            consts,
            cur: 0,
            prev: 1,
            render: 2,
            clock: Pf::b(0x3f8c_cccd),
            light,
            zones: Vec::new(),
            zone_masks: Vec::new(),
            zone_cuboids: Vec::new(),
            uv_base: m.uv_base.iter().map(|v| v.map(Pf::f)).collect(),
            uv_select: m.uv_select.clone(),
            strip_order: m.strip_order.clone(),
            drip_timer: 0,
            drip_points: Vec::new(),
            mist_rows: Vec::new(),
            last: RippleTickInfo { zone: -1, ..Default::default() },
        }
    }

    /// Two drops per patch: centre ± randf(−6, 6) (x first), radius 2, amplitude 0.2, over all patches (751, and the
    /// 07 / 13 managers' inits).
    pub fn init_drops(&mut self, rng: &mut Rng) {
        let n = self.patches.len();
        for i in 0..n {
            for _ in 0..2 {
                let rx = Pf(rng.randf_bits(0xc0c0_0000, 0x40c0_0000));
                let px = self.patches[i].centre[0];
                let ry = Pf(rng.randf_bits(0xc0c0_0000, 0x40c0_0000));
                let py = self.patches[i].centre[1];
                self.disturb(px + rx, py + ry, Pf::b(0x4000_0000), Pf::b(0x3e4c_cccd), 0..n, false);
            }
        }
    }

    /// Writes `v` to cell (r, c) of buffer `b` of patch `p` and to the neighbours' halo slots it feeds.
    fn set_mirrored(&mut self, p: usize, b: usize, r: i32, c: i32, v: Pf) {
        self.patches[p].buf[b][gi(r, c)] = v;
        let links = self.patches[p].rec.links;
        let mut writes = [(usize::MAX, 0usize); 4];
        let mut n = 0;
        mirrors(r, c, |slot, dr, dc| {
            let l = links[slot];
            if l != 0xff && n < 4 {
                writes[n] = (l as usize, gi(dr, dc));
                n += 1;
            }
        });
        for &(q, i) in &writes[..n] {
            if let Some(pq) = self.patches.get_mut(q) { pq.buf[b][i] = v; }
        }
    }

    /// `0x2b82a8(x, y, r, amp, patches, n, additive)`: for every patch in `range`, every cell within
    /// ±r (clamped to 0..15) of (x, y) gets `v = amp` if d² < 1 else `amp / sqrt(d²)` (VU `vsqrt`, FPU
    /// `div.s`). Additive: `h = v + h_drawn`; else `h = v` unless `|h_cur| > |v|`. Writes the newest buffer
    /// and its halo copies in the neighbours.
    pub fn disturb(&mut self, x: Pf, y: Pf, r: Pf, amp: Pf, range: std::ops::Range<usize>, additive: bool) {
        let c = self.consts;
        let (b, rb) = (self.cur, self.render);
        for p in range {
            if p >= self.patches.len() { break; }
            let (px, py) = (self.patches[p].centre[0], self.patches[p].centre[1]);
            let oy = py + c.origin[1];
            let fy = (y - oy) / c.cell[1];
            let ox = px + c.origin[0];
            let fx = (x - ox) / c.cell[0];
            let r0 = (fy - r).to_i32().max(0);
            let r1 = (fy + r).to_i32().min(15);
            let c1 = (fx + r).to_i32().min(15);
            let c0 = (fx - r).to_i32().max(0);
            if r1 < r0 { continue; }
            let mut yc = oy + c.cell[1] * Pf::from_i32(r0);
            for row in r0..=r1 {
                let mut xc = (px + c.origin[0]) + c.cell[0] * Pf::from_i32(c0);
                let dy = y - yc;
                for col in c0..=c1 {
                    let dx = x - xc;
                    let d2 = dx * dx + dy * dy;
                    let mut v = amp;
                    if d2 >= ONE {
                        let d = Pf(ps2::add(0, ps2::sqrt(d2.0)));
                        v = amp / d;
                    }
                    let h = self.patches[p].buf[b][gi(row, col)];
                    if additive {
                        v += self.patches[p].buf[rb][gi(row, col)];
                    } else if v.abs() < h.abs() {
                        v = h;
                    }
                    self.set_mirrored(p, b, row, col, v);
                    xc += c.cell[0];
                }
                yc += c.cell[1];
            }
        }
    }

    /// `0x2b7f30`: every patch with a mask: SPR ← cur (`0x261c10`), `0x262038` writes the new state over
    /// prev (`n = 0.25·Σ8 − prev`, `|n| ≥ threshold → n − n·0.0625`) and copies its edges into the
    /// neighbours' prev halos; then cur ↔ prev.
    pub fn step(&mut self) {
        let (cb, pb) = (self.cur, self.prev);
        let quarter = Pf::b(0x3e80_0000);
        let sixteenth = Pf::b(0x3d80_0000);
        let thr = self.consts.damp.0 as i32;
        for p in 0..self.patches.len() {
            if self.patches[p].mask == 0 { continue; }
            let spr = self.patches[p].buf[cb];
            for r in 0..16 {
                for c in 0..16 {
                    let h = |dr: i32, dc: i32| spr[gi(r + dr, c + dc)];
                    let mut acc = h(-1, -1) * quarter;
                    for (dr, dc) in [(-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)] { acc += h(dr, dc) * quarter; }
                    let old = self.patches[p].buf[pb][gi(r, c)];
                    let n = acc - old * ONE;
                    let v = if ((n.0 & 0x7fff_ffff) as i32).wrapping_sub(thr) < 0 { n } else { n - n * sixteenth };
                    self.patches[p].buf[pb][gi(r, c)] = v;
                }
            }
            // Halo exchange of the new state (same cells and targets as the per-cell mirror).
            let links = self.patches[p].rec.links;
            let src = self.patches[p].buf[pb];
            for (slot, &l) in links.iter().enumerate() {
                if l == 0xff || l as usize >= self.patches.len() { continue; }
                let dst = &mut self.patches[l as usize].buf[pb];
                for r in 0..16 {
                    for c in 0..16 {
                        mirrors(r, c, |s, dr, dc| if s == slot { dst[gi(dr, dc)] = src[gi(r, c)]; });
                    }
                }
            }
        }
        std::mem::swap(&mut self.cur, &mut self.prev);
    }

    /// `0x2b7fe0`: step when `f ≥ 1 − 0.1` (then f = 0.1 and the drawn buffer is prev), else draw buffer 2 =
    /// `lerp(prev, cur, f)` for every patch with a mask; then `f += 0.1`. Returns whether it stepped.
    pub fn clock(&mut self) -> bool {
        let step = self.consts.step;
        let stepped = ONE - step <= self.clock;
        if stepped {
            self.clock = step;
            self.step();
            self.render = self.prev;
        } else {
            self.render = 2;
            let f = self.clock;
            let w = ONE - f;
            let (pb, cb) = (self.prev, self.cur);
            for p in self.patches.iter_mut().filter(|p| p.mask != 0) {
                for i in 0..G * G {
                    p.buf[2][i] = Pf(ps2::add(ps2::mul(p.buf[pb][i].0, w.0), ps2::mul(p.buf[cb][i].0, f.0)));
                }
            }
        }
        self.clock += step;
        stepped
    }

    /// One tick of the 751 update, state 1 (`0x2fd0e8`), with the camera position 0x167240 and the level
    /// cuboids: zone activation and random drops, the drip / mist bookkeeping, then the clock. Draws from `rng`.
    pub fn tick(&mut self, cam: [f32; 3], cuboids: &[Cuboid], rng: &mut Rng) -> RippleTickInfo { self.tick_with(cam, cuboids, rng, 0, None) }

    /// [`tick`](Self::tick) with the tick counter 0x15f5cc (the zone-5 foam row) and the particle system the zone-5
    /// waterfall foam spawns into (types 57 and 56; None: the spawners' draws are made without records). Without a
    /// moby system the drip's `CreateMoby(787)` fails: its point and timer draws only ([`RippleSim::drip_due`]); the
    /// moby loop's 751 (`crate::water::managers`) runs the phases itself and spawns the drip moby.
    pub fn tick_with(&mut self, cam: [f32; 3], cuboids: &[Cuboid], rng: &mut Rng, counter: u64, parts: Option<&mut crate::particles::Particles>) -> RippleTickInfo {
        let mut info = self.tick_zones(cam, cuboids, rng);
        if self.drip_due(info.zone, rng).is_some() {
            self.drip_timer = rng.rand_range(300, 0x4b0);
            info.drips += 1;
        }
        self.tick_mist(&mut info, rng, counter, parts);
        self.last = info;
        info
    }

    /// The first part of the 751 tick: the zone activation with the random drops, then the clock `0x2b7fe0`.
    pub fn tick_zones(&mut self, cam: [f32; 3], cuboids: &[Cuboid], rng: &mut Rng) -> RippleTickInfo {
        let mut info = RippleTickInfo { zone: -1, ..Default::default() };
        let quarter_range = (0xc080_0000, 0x4080_0000); // randf(−4, 4)
        for z in 0..self.zones.len() {
            let zone = self.zones[z];
            let idx = self.zone_cuboids.get(z).copied().unwrap_or(-1);
            let inside = idx != -1 && cuboids.get(idx as usize).is_some_and(|c| cuboid_contains(c, cam));
            let first = zone.first_patch.max(0) as usize;
            if !inside {
                for j in 0..zone.patch_count.max(0) as usize {
                    if let Some(p) = self.patches.get_mut(first + j) { p.mask = 0; }
                }
                continue;
            }
            info.zone = z as i32;
            for j in 0..zone.patch_count.max(0) as usize {
                let p = first + j;
                if p >= self.patches.len() { break; }
                self.patches[p].mask = self.zone_masks.get(p).copied().unwrap_or(0);
                if rng.randi(zone.drop_odds) == 0 {
                    let rx = Pf(rng.randf_bits(quarter_range.0, quarter_range.1));
                    let px = self.patches[p].centre[0];
                    let ry = Pf(rng.randf_bits(quarter_range.0, quarter_range.1));
                    let py = self.patches[p].centre[1];
                    self.disturb(px + rx, py + ry, ONE, Pf::b(0xbd4c_cccd), p..p + 1, true);
                    info.drops += 1;
                }
            }
        }
        info.stepped = self.clock();
        info
    }

    /// The drip's turn of the 751 tick (zone 0 or 6): `--gp−0x4f7c < 1` → the spawn point: `randi(5)` of the drip
    /// table 0x1fa650 (x, y, z, w), x + `randf(±0.15)`, then y + `randf(±0.15)`. The caller then spawns the drip 787
    /// (`0x2ffcd0(0.2, point)`: `crate::moby_update::classes::units::drip::spawn`) and re-arms the timer with
    /// `rand_range(300, 0x4b0)` into [`RippleSim::drip_timer`].
    pub fn drip_due(&mut self, zone: i32, rng: &mut Rng) -> Option<[f32; 4]> {
        if zone != 0 && zone != 6 { return None; }
        self.drip_timer -= 1;
        if self.drip_timer >= 1 { return None; }
        let i = rng.randi(5);
        let mut at = usize::try_from(i).ok().and_then(|i| self.drip_points.get(i)).copied().unwrap_or_default();
        at[0] += f32::from_bits(rng.randf_bits(0xbe19_999a, 0x3e19_999a));
        at[1] += f32::from_bits(rng.randf_bits(0xbe19_999a, 0x3e19_999a));
        Some(at)
    }

    /// The last part of the 751 tick (zone 5's waterfall foam), after the drip.
    pub fn tick_mist(&mut self, info: &mut RippleTickInfo, rng: &mut Rng, counter: u64, mut parts: Option<&mut crate::particles::Particles>) {
        // Zone 5: the waterfall foam at the foot of the fall (0x2fd750..0x2fd944): one flat foam ring (type 57) on the
        // row 0x1fa6a0[counter % 20], then up to 20 mist puffs (type 56) down the fall, each with odds 1/32. Standard
        // f32 for the positions; the row loop's 0.05 steps keep the PS2 sum (the loop count).
        if info.zone == 5 {
            use crate::particles::{type56, type57};
            let row = self.mist_rows.get((counter % 20) as usize).copied().unwrap_or(0) as f32 / 20.0;
            let x = rng.randf(-0.6, 0.6) + 177.0;
            let y = (row * -11.0 + 176.0) + rng.randf(-0.1, 0.1);
            let spin = rng.randf(0.4, 0.5);
            let pos = [x, y, 39.0, 0.0];
            match parts.as_deref_mut() {
                Some(p) => { type57::spawn(p, rng, 1.0, spin, pos, [0.02, 0.0, 0.0, 0.0]); }
                None => { rng.randi(0x10); rng.randf(0.0, 256.0); }
            }
            let (mut t, dt) = (Pf::ZERO, Pf::b(0x3d4c_cccd));
            while t < ONE {
                if rng.randi(0x20) == 0 {
                    let x = rng.randf(-0.6, 0.6) + 178.0;
                    let y = (t.to_f32() * -11.0 + 176.0) + rng.randf(-0.1, 0.1);
                    let z = rng.randf(-0.2, 0.0) + 39.0;
                    let size = rng.randf(4.0, 10.0);
                    match parts.as_deref_mut() {
                        Some(p) => { type56::spawn(p, rng, size, [x, y, z, 0.0], [0.0; 4]); }
                        None => { rng.randi(0x10); rng.randi(0x100); }
                    }
                    info.mist += 1;
                }
                t += dt;
            }
        }
        self.last = *info;
    }

    /// `0x2b7d28`: the first patch whose bounds hold (x, y, z) (1/1024 units, z within [z−1, z+1)) and whose
    /// mask has the point's 4×4 sub-block bit.
    pub fn find_patch(&self, x: Pf, y: Pf, z: Pf) -> Option<usize> {
        let k = Pf::b(0x4480_0000);
        let (xi, yi, zi) = ((x * k).to_i32(), (y * k).to_i32(), (z * k).to_i32());
        let c = self.consts;
        self.patches.iter().position(|p| {
            let [bx, by, bz] = p.bounds;
            if !(bx <= xi && by <= yi && bz <= zi && xi < bx + p.extent[0] && yi < by + p.extent[1] && zi < bz + 0x800) { return false; }
            let col = ((x - (p.centre[0] + c.origin[0])) / c.cell[0]).to_i32();
            let row = ((y - (p.centre[1] + c.origin[1])) / c.cell[1]).to_i32();
            p.mask as u32 & 1 << (((col >> 2) & 3) as u32 | (row as u32 & 0xc)) != 0
        })
    }

    /// `0x2b8910` (the water-height query `0x26ed38` uses): bilinear height of the drawn buffer at (x, y),
    /// plus the patch z. `None` when no active patch holds the point (the flat-plane fallback is off on
    /// Novalis).
    pub fn patch_height(&self, x: f32, y: f32, z: f32) -> Option<f32> {
        let (x, y) = (Pf::f(x), Pf::f(y));
        let i = self.find_patch(x, y, Pf::f(z))?;
        let p = &self.patches[i];
        let c = self.consts;
        let dx = x - (p.centre[0] + c.origin[0]);
        let dy = y - (p.centre[1] + c.origin[1]);
        let col = (dx / c.cell[0]).to_i32();
        let row = (dy / c.cell[1]).to_i32();
        let fx = (dx - Pf::from_i32(col) * c.cell[0]) / c.cell[0];
        let h = &p.buf[self.render];
        let (h00, h01, h10, h11) = (h[gi(row, col)], h[gi(row, col + 1)], h[gi(row + 1, col)], h[gi(row + 1, col + 1)]);
        let a = h00 + (h01 - h00) * fx;
        let b = h10 + (h11 - h10) * fx;
        let fy = (dy - Pf::from_i32(row) * c.cell[1]) / c.cell[1];
        Some((a + (b - a) * fy + p.centre[2]).to_f32())
    }

    /// `0x2b80d0` for patch `p` (once per draw of a visible patch): advance the scroll (wrapped to [0, 1)) and
    /// the two wobble angles, then the 46 water UVs of a sub-block strip.
    pub fn advance_uv(&mut self, p: usize) -> [[f32; 2]; SUB_STRIP_LEN] {
        let pa = &mut self.patches[p];
        for k in 0..2 {
            let mut s = pa.scroll[k] + pa.speed[k];
            if ONE < s { s -= ONE; }
            if s < Pf::ZERO { s += ONE; }
            pa.scroll[k] = s;
        }
        pa.theta[0] = Pf(crate::particles::type06::fast_add_rotations(pa.theta[0].0, pa.dtheta[0].0));
        pa.theta[1] = Pf(crate::particles::type06::fast_add_rotations(pa.theta[1].0, pa.dtheta[1].0));
        let (r, su, sv) = (pa.radius, pa.scroll[0], pa.scroll[1]);
        let (c0, s0, c1, s1) = (cos(pa.theta[0]), sin(pa.theta[0]), cos(pa.theta[1]), sin(pa.theta[1]));
        let v0 = sv + s0 * r;
        std::array::from_fn(|k| {
            let b = self.uv_base[k];
            if self.uv_select[k] == 0 {
                [(b[0] + (su + c0 * r)).to_f32(), (b[1] + v0).to_f32()]
            } else {
                [(b[0] + (su + c1 * r)).to_f32(), ((b[1] + sv) + s1 * r).to_f32()]
            }
        })
    }

    /// The 17×17 draw vertices of patch `p` for a camera at `cam` (`0x2b8c08` positions from the drawn
    /// buffer, the pin overrides, `0x261df8` normal → grey and sphere-map UV).
    pub fn patch_verts(&self, p: usize, cam: [f32; 3]) -> PatchVerts {
        let pa = &self.patches[p];
        let c = self.consts;
        let h = &pa.buf[self.render];
        let [px, py, pz] = pa.centre;
        let mut pos = Vec::with_capacity(VG * VG);
        let mut y = py + c.origin[1];
        for r in 0..VG as i32 {
            let mut x = px + c.origin[0];
            for col in 0..VG as i32 {
                pos.push([x.to_f32(), y.to_f32(), (h[gi(r, col)] + pz).to_f32()]);
                x += c.cell[0];
            }
            y += c.cell[1];
        }
        let zf = pz.to_f32();
        for k in 0..VG {
            if pa.pins & 1 != 0 { pos[k][2] = zf; }
            if pa.pins & 2 != 0 { pos[k * VG + 16][2] = zf; }
            if pa.pins & 4 != 0 { pos[16 * VG + k][2] = zf; }
            if pa.pins & 8 != 0 { pos[k * VG][2] = zf; }
        }

        // 0x261df8 (VU0 macro code), lanes as decoded in the module docs.
        let (k0573, k325, k65536_5, half) = (0x3f12_b021u32, 0x4050_0000u32, 0x4780_0040u32, 0x3f00_0000u32);
        let cam = cam.map(|v| Pf::f(v).0);
        let light = self.light.map(|v| v.0);
        let (cx, cy) = (c.cell[0].0, c.cell[1].0);
        let mut rgba = Vec::with_capacity(VG * VG);
        let mut env_uv = Vec::with_capacity(VG * VG);
        let mut vy = ps2::add(py.0, c.origin[1].0);
        for r in 0..VG as i32 {
            let mut vx = ps2::add(px.0, c.origin[0].0);
            for col in 0..VG as i32 {
                let hh = |dr: i32, dc: i32| h[gi(r + dr, col + dc)].0;
                let hc = ps2::add(0, hh(0, 0));
                let (hd, hr, hu, hl) = (hh(1, 0), hh(0, 1), hh(-1, 0), hh(0, -1));
                let e10 = [0, ps2::sub(0, cy), ps2::sub(hc, hd)];
                let e11 = [ps2::sub(0, cx), cy, ps2::sub(hd, hr)];
                let e12 = [0, cy, ps2::sub(hc, hu)];
                let e13 = [ps2::add(0, cx), ps2::sub(0, cy), ps2::sub(hu, hl)];
                let n = ps2v::vadd(ps2v::cross(e11, e10), ps2v::cross(e13, e12));
                let q = ps2::rsqrt(ps2::ONE, ps2v::dot_x(n, n));
                let n = ps2v::vsub([0; 3], ps2v::vmuls(n, q));
                let p3 = [vx, vy, ps2::add(hc, pz.0)];
                let e = ps2v::vsub(cam, p3);
                let qe = ps2::rsqrt(ps2::ONE, ps2v::dot_x(e, e));
                let ln = ps2v::dot_x(light, n);
                let t = ps2::add(ps2::mul(ps2::sub(ln, k0573), k325), k0573);
                let t = ps2::min(ps2::max(t, 0), ps2::ONE);
                let e = ps2v::vmuls(e, qe);
                let g = ps2::add(t, k65536_5) & 0xff;
                rgba.push(g | g << 8 | g << 16);
                let v5 = ps2v::vsub([0; 3], e);
                let d = ps2v::dot_x(v5, n);
                let t1 = ps2v::vsub(ps2v::vmuls(n, d), v5);
                let refl = if ps2v::neg(d) { e } else { ps2v::vadd(v5, ps2v::vadd(t1, t1)) };
                let uv = [0, 1].map(|k| ps2::add(ps2::mul(ps2::mul(refl[k], c.refl.0), half), half));
                env_uv.push(uv.map(ps2v::to_f32));
                vx = ps2::add(vx, cx);
            }
            vy = ps2::add(vy, cy);
        }
        PatchVerts { pos, rgba, env_uv }
    }
}

// =================================================================================================
// Waterfall foam scroll (class 809 global, 760 elements)

/// The 809 update `0x2ba658`: every tick `0x161374 −= 0.0025·[0x15ed60]` (= 1.0), `+= 8` once it is ≤ −8.
/// 760's second quad set adds it to U.
pub fn foam_global_tick(u: Pf) -> Pf {
    let v = u - Pf::b(0x3b23_d70a) * ONE;
    if v <= Pf::b(0xc100_0000) { v + Pf::b(0x4100_0000) } else { v }
}

/// 760 per element (`0x2fe080`): `v −= speed·[0x15ed60]` per draw, `+= 8` once it is ≤ −8.
pub fn foam_element_tick(v: Pf, speed: Pf) -> Pf {
    let v = v - speed * ONE;
    if v <= Pf::b(0xc100_0000) { v + Pf::b(0x4100_0000) } else { v }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rc_formats::water::RippleZone;

    fn strip(n: usize) -> StripDescriptor {
        StripDescriptor {
            addr: 0,
            vertices: vec![[0.0; 4]; n],
            uvs: vec![[0.0; 2]; n],
            bsphere: [0.0; 4],
            speed: [0.25, 0.5],
            wobble_amp: 0.025,
            phase_step: 1,
            dir: [[-0.1, 0.9], [0.1, 0.9]],
            rgba: 0x808080,
            fx: [44, 44],
            fix: [0x28, 0x28],
            scroll: [0.0; 4],
            phase: 0,
            z_blend: 0.0,
        }
    }

    #[test]
    fn scroll_wraps_each_lane_once() {
        let mut d = strip(3);
        d.speed = [1.0, 1.0];
        d.dir = [[0.75, -0.75], [0.5, 0.0]];
        let mut s = ScrollState::default();
        s.tick(&d);
        assert_eq!(s.s1.map(Pf::to_f32), [0.75, -0.75]);
        s.tick(&d);
        // 1.5 > 1 → 0.5; −1.5 < −1 → −0.5.
        assert_eq!(s.s1.map(Pf::to_f32), [0.5, -0.5]);
        s.tick(&d);
        s.tick(&d);
        assert_eq!(s.s2.map(Pf::to_f32), [1.0, 0.0], "exactly 1.0 is not > 1");
        s.tick(&d);
        assert_eq!(s.s2[0].to_f32(), 0.5);
        assert_eq!(s.phase, 5);
        s.phase = 0xff_ffff;
        s.tick(&d);
        assert_eq!(s.phase, 0);
    }

    #[test]
    fn wobble_groups_and_values() {
        // Only hash & 7 matters.
        assert_eq!(wobble_angle(3, 17), wobble_angle(11, 17));
        // hash 0, phase 0: k = 0 → θ = −π: sin ≈ 0, cos ≈ −1.
        let w = wobble(0, 0, 1.0).map(Pf::to_f32);
        assert!(w[0].abs() < 1e-5 && (w[1] + 1.0).abs() < 1e-5, "{w:?}");
        // hash 4 → k = 2048 → θ = 0: (0, A).
        let w = wobble(4, 0, 0.025).map(Pf::to_f32);
        assert!(w[0].abs() < 1e-6 && (w[1] - 0.025).abs() < 1e-6);
        // One turn every 512 phase steps.
        assert_eq!(wobble_angle(2, 5), wobble_angle(2, 5 + 512));
        assert_eq!(wobble_hash(1.0, 2.0), ((1f32.to_bits() + 2f32.to_bits()) >> 16) & 0xff);
    }

    #[test]
    fn bob_period_and_range() {
        // t & 63 = 32 → sin(0) → 0.5; t = 48 → sin(π/2) ≈ 1; 64-tick period.
        assert_eq!(bob(32).to_f32(), 0.5);
        assert!((bob(48).to_f32() - 1.0).abs() < 1e-5);
        assert!((bob(16).to_f32()).abs() < 1e-5);
        assert_eq!(bob(5), bob(69));
        assert!((blend_z(10.0, 20.0, Pf::f(0.25)) - 17.5).abs() < 1e-5);
    }

    #[test]
    fn init_761_rungs_alternate() {
        let mut s = vec![vec![[0.0, 0.0, 1.0, 0.0]; 4], vec![[0.0, 0.0, 1.0, 0.0]; 2]];
        init_761(&mut s);
        let z = |i: usize, j: usize| [s[i][j][2], s[i][j][3]];
        let (lo, hi) = (0.55f32 - 0.05, 0.55f32 + 0.05);
        // Strip 0, pair 0: flip false → v0 z0 −, z1 +.
        assert!((z(0, 0)[0] - lo).abs() < 1e-6 && (z(0, 0)[1] - hi).abs() < 1e-6);
        assert!((z(0, 1)[0] - hi).abs() < 1e-6 && (z(0, 1)[1] - lo).abs() < 1e-6);
        // Pair 1 alternates.
        assert!((z(0, 2)[0] - hi).abs() < 1e-6);
        // Strip 1 starts opposite to strip 0 (even pair count per strip).
        assert!((z(1, 0)[0] - hi).abs() < 1e-6);
    }

    /// A single patch with no links, zero tables.
    fn one_patch() -> RippleSim {
        let rec = PatchRecord { centre: [100.0, 200.0, 50.0], links: [0xff; 8], fx_env: 40, fx_water: 41, fix_env: 0x1c, fix_water: 0x44, mask: 0, pins: 0 };
        let t = RippleTables {
            base: 0,
            patches: vec![rec],
            zones: vec![RippleZone { first_patch: 0, patch_count: 1, drop_odds: 200 }],
            zone_masks: vec![0xffff],
            drips: vec![],
            mist_rows: vec![],
            consts: [32.0, 32.0, -16.0, -16.0, 2.0, 2.0, 0.95, 0.16, 0.1],
            uv_base: vec![[0.0; 2]; SUB_STRIP_LEN],
            uv_select: vec![0; SUB_STRIP_LEN],
            strip_order: (0..SUB_STRIP_LEN as u16).collect(),
        };
        let mut s = RippleSim::new(&t, vec![-1], [1.0, 0.0], &mut Rng::new());
        s.patches[0].buf.fill([Pf::ZERO; G * G]);
        s.patches[0].mask = 0xffff;
        s
    }

    #[test]
    fn wave_step_on_a_3x3_impulse() {
        let mut s = one_patch();
        // Impulse of 0.1 (below the damping threshold) at (5, 5) in cur, prev zero.
        s.patches[0].buf[s.cur][gi(5, 5)] = Pf::f(0.1);
        let (cur, prev) = (s.cur, s.prev);
        s.step();
        // The new state went into the old prev buffer, which is now cur.
        assert_eq!((s.cur, s.prev), (prev, cur));
        let n = &s.patches[0].buf[s.cur];
        for (r, c) in [(4, 4), (4, 5), (4, 6), (5, 4), (5, 6), (6, 4), (6, 5), (6, 6)] {
            assert_eq!(n[gi(r, c)].to_f32(), 0.025, "neighbour ({r},{c})");
        }
        assert_eq!(n[gi(5, 5)].to_f32(), 0.0, "centre: 0.25·Σ(0) − 0");
        assert_eq!(n[gi(3, 5)].to_f32(), 0.0);
        // Second step: the centre = 0.25·8·0.025 − 0.1 = −0.05.
        s.step();
        let n = &s.patches[0].buf[s.cur];
        assert!((n[gi(5, 5)].to_f32() + 0.05).abs() < 1e-6, "{:?}", n[gi(5, 5)]);
    }

    #[test]
    fn damping_threshold() {
        let mut s = one_patch();
        // prev = −0.2 at a cell with flat neighbours: n = 0.2 ≥ 0.16 → 0.2 − 0.2·0.0625 = 0.1875.
        s.patches[0].buf[s.prev][gi(8, 8)] = Pf::f(-0.2);
        // prev = −0.15: n = 0.15 < 0.16 → kept.
        s.patches[0].buf[s.prev][gi(2, 2)] = Pf::f(-0.15);
        s.step();
        let n = &s.patches[0].buf[s.cur];
        assert_eq!(n[gi(8, 8)].to_f32(), (Pf::f(0.2) - Pf::f(0.2) * Pf::f(0.0625)).to_f32());
        assert!((n[gi(8, 8)].to_f32() - 0.1875).abs() < 1e-6);
        assert_eq!(n[gi(2, 2)].to_f32(), 0.15);
    }

    #[test]
    fn interpolation_ticks_0_to_8() {
        let mut s = one_patch();
        s.patches[0].buf[s.cur][gi(5, 5)] = Pf::f(1.0);
        // Tick 0: clock 1.1 ≥ 0.9 → step; draw prev (the pre-step state, which holds the impulse).
        assert!(s.clock());
        assert_eq!(s.render, s.prev);
        assert_eq!(s.patches[0].buf[s.render][gi(5, 5)].to_f32(), 1.0);
        let (prev, cur) = (s.prev, s.cur);
        let mut fs = Vec::new();
        for _ in 1..9 {
            let f = s.clock.to_f32();
            assert!(!s.clock(), "lerp tick");
            assert_eq!(s.render, 2);
            let v = s.patches[0].buf[2][gi(5, 5)].to_f32();
            let expect = 1.0 * (1.0 - f) + s.patches[0].buf[cur][gi(5, 5)].to_f32() * f;
            assert!((v - expect).abs() < 1e-6);
            // The neighbour's new value 0.25 is above the threshold: 0.25 − 0.25/16.
            let nb = s.patches[0].buf[2][gi(4, 5)].to_f32();
            assert_eq!(s.patches[0].buf[cur][gi(4, 5)].to_f32(), 0.234375);
            assert!((nb - 0.234375 * f).abs() < 1e-6, "neighbour lerps toward the new state");
            fs.push(f);
        }
        assert_eq!(s.prev, prev);
        let r: Vec<f32> = fs.iter().map(|f| (f * 10.0).round() / 10.0).collect();
        // Eight lerp ticks: on the PS2 FPU the accumulator reaches only 0x3f666664 after 0.1 + 7·0.1, below
        // 1 − 0.1 = 0x3f666667, so t = 0.9 is drawn too and the step comes every 9 ticks.
        assert_eq!(r, [0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9]);
        assert_eq!(fs[7].to_bits(), 0x3f66_6664);
        assert!(s.clock(), "tick 9 steps");
    }

    #[test]
    fn disturb_and_height_query() {
        let mut s = one_patch();
        // Drop at the cell (row 4, col 6) position: x = 100 − 8 + 6, y = 200 − 8 + 4.
        s.disturb(Pf::f(98.0), Pf::f(196.0), Pf::f(1.0), Pf::f(0.5), 0..1, false);
        let b = &s.patches[0].buf[s.cur];
        assert_eq!(b[gi(4, 6)].to_f32(), 0.5, "d² < 1: amp");
        // Diagonal neighbour: d² = 2 → amp / √2.
        assert!((b[gi(5, 7)].to_f32() - 0.5 / 2f32.sqrt()).abs() < 1e-5);
        assert_eq!(b[gi(4, 8)].to_f32(), 0.0, "outside ±r");
        // Non-additive keeps the larger magnitude.
        s.disturb(Pf::f(98.0), Pf::f(196.0), Pf::f(0.5), Pf::f(0.1), 0..1, false);
        assert_eq!(s.patches[0].buf[s.cur][gi(4, 6)].to_f32(), 0.5);
        // Height query on the drawn buffer: make it the one we wrote.
        s.render = s.cur;
        assert_eq!(s.patch_height(98.0, 196.0, 50.0), Some(50.5));
        // Half way to (4, 7) (d² = 1 exactly: amp / 1 = 0.5 as well) and a quarter toward row 5.
        assert_eq!(s.patch_height(98.5, 196.0, 50.0), Some(50.5));
        let h = s.patch_height(98.0, 196.25, 50.0).unwrap();
        assert!((h - (50.5 + (0.5 / 2f32.sqrt() - 0.5) * 0.0 + (0.5 - 0.5) * 0.25)).abs() < 1e-5, "{h}");
        assert_eq!(s.patch_height(98.0, 196.0, 52.0), None, "z outside [z−1, z+1)");
        s.patches[0].mask = 0;
        assert_eq!(s.patch_height(98.0, 196.0, 50.0), None, "inactive patch");
    }

    #[test]
    fn flat_water_is_grey_139() {
        let s = one_patch();
        let v = s.patch_verts(0, [100.0, 200.0, 80.0]);
        assert_eq!(v.rgba[8 * VG + 8] & 0xff, 139);
        assert_eq!(v.rgba[8 * VG + 8] >> 24, 0);
        // Camera straight above the vertex: R = (0, 0, …) → UV (0.5, 0.5).
        let uv = v.env_uv[8 * VG + 8];
        assert!((uv[0] - 0.5).abs() < 1e-6 && (uv[1] - 0.5).abs() < 1e-6, "{uv:?}");
        assert_eq!(v.pos[0], [92.0, 192.0, 50.0]);
        assert_eq!(v.pos[VG * VG - 1], [108.0, 208.0, 50.0]);
    }

    #[test]
    fn halo_mirror_between_linked_patches() {
        let mut s = one_patch();
        let mut q = s.patches[0].clone();
        q.rec.links = [0xff; 8];
        q.centre[0] = Pf::f(116.0);
        s.patches.push(q);
        // Patch 1 lies past patch 0's last column: link +0x0f of 0 = 1, +0x0d of 1 = 0.
        s.patches[0].rec.links[3] = 1;
        s.patches[1].rec.links[1] = 0;
        s.disturb(Pf::f(107.0), Pf::f(196.0), Pf::f(0.0), Pf::f(0.3), 0..2, false);
        assert_eq!(s.patches[0].buf[s.cur][gi(4, 15)].to_f32(), 0.3);
        assert_eq!(s.patches[1].buf[s.cur][gi(4, -1)].to_f32(), 0.3, "column 15 feeds the neighbour's column −1");
        s.disturb(Pf::f(108.0), Pf::f(197.0), Pf::f(0.0), Pf::f(0.2), 0..2, false);
        assert_eq!(s.patches[1].buf[s.cur][gi(5, 0)].to_f32(), 0.2);
        assert_eq!(s.patches[0].buf[s.cur][gi(5, 16)].to_f32(), 0.2, "column 0 feeds the neighbour's column 16");
    }

    #[test]
    fn foam_scroll_wraps_at_minus_8() {
        let mut u = Pf::ZERO;
        for _ in 0..3200 { u = foam_global_tick(u); }
        assert!(u.to_f32() > -8.0 && u.to_f32() <= 0.0);
        assert_eq!(foam_element_tick(Pf::f(-7.5), Pf::f(0.5)).to_f32(), 0.0);
    }
}
