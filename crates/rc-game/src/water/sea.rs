//! The sea and the other liquid surfaces the level code draws from a draw callback (docs/plan/world_animation.md §3.3):
//! the class updates that own them, ported per level and found by code identity (`LevelPorts`, `ClassUpdate::Sea(i)`,
//! the index into [`PORTS`]), their level data ([`SeaPortData`], read once per level through `Relocation` from each
//! port's reference level) and their state ([`SeaState`], in `Services::water`). rc-engine's `sea_render` draws the
//! registered callbacks. Three mechanisms:
//!
//! | ports | classes (level) | reference update | draws |
//! |---|---|---|---|
//! | 0–5 | 994 (03), 879 (05), 460 (07) and 317 (09, the same code), 1018 (07), 327 (08), 1260 (14) | per level (below) | the liquid grid module: level05 `0x2ce098` (init) … `0x2ce830` (draw), one [`rc_formats::sea::LiquidGrid`] each |
//! | 6 | 1111 (11, 16) | level11 `0x30b358` (the same code on 16) | the camera-following ocean `0x30a908` |
//! | 7 | 1901 (12) | level12 `0x30bff0` | the static liquid strips `0x30be68` |
//! | 8 | 854 (02) | level02 `0x2ea198` | seven small liquid grids with one image and fog `0x2ea048` ([`aridia_ref`]) |
//! | 9 | 293 (12) | level12 `0x2e7208` | 47 two-texture strips `0x2e71b0` → `0x2bc210` |
//! | 10 | 1418 (14) | level14 `0x307a80` | 19 two-texture strips `0x307a28` → `0x2ab3e8` (the same module) |
//! | 11 | 1848 (01) | level01 `0x30f208` | the reflective overlay `0x30f0e0` ([`env_overlay_ref`]) |
//! | 14–18 | 1943, 1947, 1948, 1952, 1953 (16) | level16 `0x2e8e80`, `0x2e93b0`, `0x2e98e0`, `0x2e9df0`, `0x2ea128` | Kalebo's reflections ([`reflect_ref`]) |
//! | 21–26 | 1907, 1908, 1910, 1911, 1913, 1918 (17) | level17 `0x2f57a0`, `0x2f5ab8`, `0x2f5dd0`, `0x2f60e8`, `0x2f6400`, `0x2f6718` | Veldin Orbit's window glass (1952's code, their own draws: [`fleet_glass_ref`]) |
//! | 12, 13 | 1903, 1919 (11) | level11 `0x31e2f0`, `0x31f150` | Pokitaru's pool overlays `0x31db50`, `0x31e930` (two copies of one module: [`pool_ref`]) |
//! | 19, 20 | 600, 1649 (08) | level08 `0x2f1548`, `0x3097b0` | Batalia's copies of the pool module `0x2f0dc8`, `0x309030` ([`pool_ref::C`], [`pool_ref::D`]) |
//!
//! **The liquid meshes (G-REN-026, 2026-10-01).** The callbacks that draw static strip meshes share two engine emitters
//! (level01 `0x21fda8`: one pass; `0x21fa98`: the strip twice, the second in GS context 2 with a second ST set), each
//! fed positions, colours and ST by the class's own code. One data model ([`MeshSet`]: meshes, a colour rule, one or
//! two [`MeshPass`]es: texture, ST rule, FIX) and one draw (rc-engine `sea_render::mesh_prims`) cover every user; a new
//! one is a [`SeaPort`] row and a loader of its tables:
//!
//! | address | what | port |
//! |---|---|---|
//! | level09 `0x2ef750` | `t = c/20 − ⌊c/20⌋`, `0x21e8c0(t, 0x1f3080, 108, 0x2c + (q & 15), 0x2c + ((q + 1) & 15))`, the grid `0x2c1920`, `0x2c1978(grid, 0x1f63c0, 10)` | [`GridData::extras`] ([`gaspar_ref`]) |
//! | `0x21e8c0` → `0x21e560` | the two frames blended texel by texel by t (`0x26d240`, 64×64, PSMCT24) into TEX0_1 = TEX0_2 (the same frame twice: TEX0 = that FX) | [`MeshTex::Anim`] (the grid's image code) |
//! | `0x21e8c0` | the packet `0x16eaa0` (TEST 0x50000 both contexts, TEX1 bilinear, CLAMP repeat, ALPHA_1 FIX 0x80, ALPHA_2 FIX 0x60: [`fs::strip_state_fix`]); 256 colours [`flow_colour`]; f = [`flow_offset`] | [`MeshColour::Flow`], [`MeshSt::Flow`] |
//! | | per record (0x40 B): `FastBSphereCheck(256, record)`, each strip `0x21d250` (= `0x21fa98`) with ST1 = (s, t + f), ST2 = (s, t + 2f + 0.5) | `fs::parse_flow_meshes`, two passes |
//! | `0x2c1978` | the grid's set-up (its fog from the record; with 0x15f458 = 1, the Visibomb's view, the level fog and FOGCOL stay: `rc-engine` sea_render; its image, ALPHA FIX = record +0x3f, TEST by FIX), colours 0x00757c8e, per record (0x20 B) `FastBSphereCheck(256)`, ST · 0.5, `0x21d560` (= `0x21fda8`); `0x2c1810` the fog restore | [`MeshTex::Grid`], [`MeshSt::Stored`] |
//! | level02 `0x2ea048` | `0x2a4818(0, 260080, 255, 0, 0x0f, 0x0f, 0x19, 30, 0x28, 0x40, 0x1f3e80)`: the module's image / fog / GS set-up from a stack record (FIX 0x80), the strip order DMA | [`aridia_ref`], [`SeaData::GridSet`] |
//! | | 0x15f608 ≠ 0, or pvar 0 ≠ −1 and the camera 0x1673c0 in it (`0x261928`) → record 0x1f3c00; likewise pvar 1 → 0x1f3cd0, 0x1f3d30, 0x1f3d90, 0x1f3dd0, 0x1f3e10; pvar 2 → 0x1f3c60: each `0x24f748` (block cull) + `0x24fa80` (blocks) | [`grid_set_drawn`]; the draw is the grid's |
//! | | `0x2a4880` the fog restore | n/a (per-draw fog) |
//! | level12 `0x2e71b0` | `0x2bc210(0x1f5480, 47, GetEffectTex(0x2c), GetEffectTex(0x2d))` | [`SeaKind::TwoTex`] |
//! | `0x2bc210` | TEX0_1 / TEX0_2, the packet `0x1cb7a0` (ALPHA_1 FIX 0x80, ALPHA_2 FIX 0x40), 180 colours 0x80808080; per record (0x20 B) `FastBSphereCheck(256)`, ST2 = `0x2667fc` (sphere map: [`sphere_map_st`]), `0x220a58` (= `0x21fa98`) | `fs::parse_mesh_records`, [`MeshSt::SphereMap`] |
//! | level14 `0x307a28` | `0x2ab3e8(0x1f5f80, 19, FX 0x2e, FX 0x2f)`, packet `0x1cba20` (the same values) | port 10 |
//! | level01 `0x30f0e0` | TEX0 FX 40, ALPHA 0x2000000064, CLAMP 0, TEX1 bilinear, `DrawSpriteHelper_A`; 5 meshes: `0x30ef18` (ST: [`env_map_st`]), `0x21fda8` (stored colours, clip on) | [`SeaKind::EnvOverlay`] |
//! | level15 `0x29642c` | class 28's callback `0x297808`: an FX 0xb glow quad (`FastDrawQuadReal`), not a liquid | n/a (U442) |
//!
//! **The liquid grid classes** are each level's own wrapper around the shared module: init once (`0x2ce098(scale, state,
//! FIX)`: `state+0x3f = FIX`, the module's ST table · scale), then register the level's draw callback (which calls the
//! module's `0x2ce830(state)`) every tick. Their extras: 879 (05) writes the grid height every tick, `59.5 + 0.25·sin`
//! (61.5 while Ratchet is on the Hoverboard, state group 0x16), and its callback skips the draw while the camera is in
//! one of the moby's 8 cuboids (pvar words 0..7, −1 = none); 1018 (07) registers only while the camera is outside its
//! cuboid (pvar 0); 327 (08) inits once per level (a `$gp` flag, not the moby state) and registers every tick without
//! setting its update distance, then makes its splash when Ratchet falls in (`0x2da0f0` second half: `batalia_splash`).
//! Which list: `RegisterDrawCallback` (list 1, after the mobys) on 03, 08, 14; the list drained after the ties and before
//! the shrubs (`0x16e100`, count `0x15f42c`) on 05, 07, 09.
//!
//! **The ocean 1111** (pvar: 0..2 colours of the far band and the wall, 3 / 4 the surface colour near / at two tiles,
//! 5 sea z, 6 tile size S, 7 FX base, 8 lowest camera z, 9 the wall's drop, 10 the band's reach, 11 the fade height):
//! state 0 keeps the camera and the sea z; state 1 scrolls each layer's ST by 2·scale/(5·S) of the camera's motion
//! (fraction only) and by its speed · dt, wraps each lane into [−1, 1], bobs the sea `z + 0.25·sin` and registers the
//! draw (list 1) while the camera is above pvar 8.
//!
//! **The Hoven liquid 1901**: its first init rescales the colours of both strip groups (`0x30bba0`: RGB = constant,
//! A = A/2; done here at load), state 1 scrolls the two layers and group 1 by their speeds · dt (wrapped) and registers
//! the draw on the after-ties list.
//!
//! Native `f32` throughout (the game's literal 3.14 and 0.017444445 in the bob). dt = 1/60 (`0x15ed7c`, NTSC).

use super::world::WaterWorld;
use crate::moby_runtime::MobyId;
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::services::{fv, pvar, World};
use rc_formats::level_overlay::{LevelOverlay, Relocation};
use rc_formats::sea::{self as fs, LiquidGrid, LiquidGridModule, OceanTables, StripMesh};
use rc_formats::water::Overlay;

/// Which draw list a port's callback goes on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawList {
    /// `RegisterDrawCallback` 0x21afe0: after the mobys, before the particles.
    AfterMobys,
    /// The list `0x16e100` (count `0x15f42c`), drained by `DrawWorld` after the ties and before the shrubs.
    AfterTies,
}

/// A liquid grid class's own behaviour around the module (module doc).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridVariant {
    Plain,
    /// 879 (level05 `0x316110`, callback `0x316070`).
    Rilgar,
    /// 1018 (level07 `0x30d440`).
    Umbris,
    /// 327 (level08 `0x2da0f0`).
    Batalia,
}

/// A liquid grid port: its state record (a label of the reference level), the init's scale and FIX, its list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridPort {
    pub state: u32,
    pub scale: f32,
    pub fix: u8,
    pub list: DrawList,
    pub variant: GridVariant,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SeaKind {
    Grid(GridPort),
    Ocean,
    Hoven,
    /// Several small liquid grids drawn with one shared image and fog, each gated by a cuboid (02's 854).
    GridSet,
    /// Strip meshes drawn twice through the two-texture strip module (level12 `0x2bc210`, level14 `0x2ab3e8`): the
    /// stored ST with one FX texture, then a sphere-mapped ST with another (12's 293, 14's 1418).
    TwoTex(TwoTexPort),
    /// Class 1848's reflective overlay (level01 `0x30f208` / `0x30f0e0`): five static meshes with a sphere-mapped,
    /// scrolled FX 40 at FIX 0x20.
    EnvOverlay,
    /// The pool overlays: Pokitaru's 1903 / 1919 (level11) and Batalia's 600 / 1649 (level08), copies of one module ([`pool_ref`]).
    Pool(&'static pool_ref::Module),
    /// Kalebo's reflections 1943, 1947, 1948, 1952, 1953 (level16: [`reflect_ref`]).
    Reflect(&'static reflect_ref::Port),
}

/// A two-texture strip class: its mesh table (reference level labels), its two FX textures, the GS state packet the
/// module sends (both contexts' ALPHA), and its gate (pvar offset of a camera cuboid, −1 = none; None: no gate).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TwoTexPort {
    pub table: u32,
    pub count: usize,
    pub fx: [u16; 2],
    pub packet: u32,
    pub gate: Option<usize>,
}

/// One sea class port.
#[derive(Clone, Copy, Debug)]
pub struct SeaPort {
    pub name: &'static str,
    /// The level whose overlay holds [`SeaPort::func`] (and the port's data labels).
    pub level: u32,
    pub func: u32,
    /// The classes the reference level's class table runs it for.
    pub classes: &'static [i16],
    pub kind: SeaKind,
}

/// `0x3f2aaaab`: the init scale 2/3 of 03, 07 (1018), 08, 14.
const TWO_THIRDS: f32 = f32::from_bits(0x3f2a_aaab);

const fn grid(state: u32, scale: f32, fix: u8, list: DrawList, variant: GridVariant) -> SeaKind {
    SeaKind::Grid(GridPort { state, scale, fix, list, variant })
}

pub const OCEAN: usize = 6;
pub const HOVEN: usize = 7;

pub const PORTS: [SeaPort; 27] = [
    // level03 0x2dc9b0: 0x291198(2/3, 0x1dc1a0, 0x20); callback 0x2dc990 via 0x1f2d48 (list 1).
    SeaPort { name: "994 liquid", level: 3, func: 0x2d_c9b0, classes: &[994], kind: grid(0x1d_c1a0, TWO_THIRDS, 0x20, DrawList::AfterMobys, GridVariant::Plain) },
    // level05 0x316110: 0x2ce110(1.0, 0x211b20) (FIX 0x80); callback 0x316070 via 0x228110 (after the ties).
    SeaPort { name: "879 sea", level: 5, func: 0x31_6110, classes: &[879], kind: grid(0x21_1b20, 1.0, 0x80, DrawList::AfterTies, GridVariant::Rilgar) },
    // level07 0x2f8058: 0x2cc608(1.0, 0x1d3400, 0x80); callback 0x2f8038 via 0x221290 (after the ties). The same code
    // (relinked) is level09's 0x2ef7f8 for 317, the lava: 0x2c11a0(1.0, 0x1fc740, 0x80), callback 0x2ef750 via 0x218810.
    SeaPort { name: "460 / 317 liquid", level: 7, func: 0x2f_8058, classes: &[460, 317], kind: grid(0x1d_3400, 1.0, 0x80, DrawList::AfterTies, GridVariant::Plain) },
    // level07 0x30d440: 0x2cc680(2/3, 0x208560) (FIX 0x80); callback 0x30d420 via 0x221290 (after the ties).
    SeaPort { name: "1018 sea", level: 7, func: 0x30_d440, classes: &[1018], kind: grid(0x20_8560, TWO_THIRDS, 0x80, DrawList::AfterTies, GridVariant::Umbris) },
    // level08 0x2da0f0: 0x2acce8(2/3, 0x1d7060) (FIX 0x80); callback 0x2da0d0 via 0x20bdf0 (list 1).
    SeaPort { name: "327 liquid", level: 8, func: 0x2d_a0f0, classes: &[327], kind: grid(0x1d_7060, TWO_THIRDS, 0x80, DrawList::AfterMobys, GridVariant::Batalia) },
    // level14 0x303370: 0x2aabf8(2/3, 0x1e9c60, 0x40); callback 0x303350 via 0x20c5d0 (list 1).
    SeaPort { name: "1260 liquid", level: 14, func: 0x30_3370, classes: &[1260], kind: grid(0x1e_9c60, TWO_THIRDS, 0x40, DrawList::AfterMobys, GridVariant::Plain) },
    SeaPort { name: "1111 ocean", level: fs::ocean_ref::LEVEL, func: fs::ocean_ref::UPDATE_FN, classes: &[1111], kind: SeaKind::Ocean },
    SeaPort { name: "1901 Hoven liquid", level: fs::hoven_ref::LEVEL, func: fs::hoven_ref::UPDATE_FN, classes: &[1901], kind: SeaKind::Hoven },
    // level02 0x2ea198: state 0 the module init 0x2a47f8(1/6); state 1 registers 0x2ea048 via 0x20a390 (after the ties).
    SeaPort { name: "854 Aridia liquids", level: 2, func: 0x2e_a198, classes: &[854], kind: SeaKind::GridSet },
    // level12 0x2e7208 / callback 0x2e71b0: 0x2bc210(0x1f5480, 47, FX 0x2c, FX 0x2d), packet 0x1cb7a0; registered via
    // 0x21bf18 (after the ties) while the camera (0x167240) is in the cuboid of pvar +0xc (−1: always).
    SeaPort { name: "293 Hoven strips", level: 12, func: 0x2e_7208, classes: &[293], kind: SeaKind::TwoTex(TwoTexPort { table: 0x1f_5480, count: 0x2f, fx: [0x2c, 0x2d], packet: 0x1c_b7a0, gate: Some(0xc) }) },
    // level14 0x307a80 / callback 0x307a28: 0x2ab3e8(0x1f5f80, 19, FX 0x2e, FX 0x2f), packet 0x1cba20; registered via
    // 0x20c698 (the list counted by 0x15f42c: after the ties [L: by its count global]) with no gate.
    SeaPort { name: "1418 Oltanis strips", level: 14, func: 0x30_7a80, classes: &[1418], kind: SeaKind::TwoTex(TwoTexPort { table: 0x1f_5f80, count: 0x13, fx: [0x2e, 0x2f], packet: 0x1c_ba20, gate: None }) },
    // level01 0x30f208: the UV scroll 0x162110 / 0x162114 and RegisterDrawCallback(0x30f0e0) (list 1).
    SeaPort { name: "1848 env overlay", level: env_overlay_ref::LEVEL, func: env_overlay_ref::UPDATE_FN, classes: &[1848], kind: SeaKind::EnvOverlay },
    SeaPort { name: "1903 pool overlay", level: pool_ref::LEVEL, func: pool_ref::A.update, classes: &[1903], kind: SeaKind::Pool(&pool_ref::A) },
    SeaPort { name: "1919 pool overlay", level: pool_ref::LEVEL, func: pool_ref::B.update, classes: &[1919], kind: SeaKind::Pool(&pool_ref::B) },
    SeaPort { name: "1943 reflection", level: reflect_ref::LEVEL, func: reflect_ref::R1943.update, classes: &[1943], kind: SeaKind::Reflect(&reflect_ref::R1943) },
    SeaPort { name: "1947 reflection", level: reflect_ref::LEVEL, func: reflect_ref::R1947.update, classes: &[1947], kind: SeaKind::Reflect(&reflect_ref::R1947) },
    SeaPort { name: "1948 reflection", level: reflect_ref::LEVEL, func: reflect_ref::R1948.update, classes: &[1948], kind: SeaKind::Reflect(&reflect_ref::R1948) },
    SeaPort { name: "1952 reflection", level: reflect_ref::LEVEL, func: reflect_ref::R1952.update, classes: &[1952], kind: SeaKind::Reflect(&reflect_ref::R1952) },
    SeaPort { name: "1953 reflection", level: reflect_ref::LEVEL, func: reflect_ref::R1953.update, classes: &[1953], kind: SeaKind::Reflect(&reflect_ref::R1953) },
    SeaPort { name: "600 Batalia pool overlay", level: pool_ref::C_LEVEL, func: pool_ref::C.update, classes: &[600], kind: SeaKind::Pool(&pool_ref::C) },
    SeaPort { name: "1649 Batalia pool overlay", level: pool_ref::C_LEVEL, func: pool_ref::D.update, classes: &[1649], kind: SeaKind::Pool(&pool_ref::D) },
    SeaPort { name: "1907 fleet glass", level: fleet_glass_ref::LEVEL, func: fleet_glass_ref::G1907.update, classes: &[1907], kind: SeaKind::Reflect(&fleet_glass_ref::G1907) },
    SeaPort { name: "1908 fleet glass", level: fleet_glass_ref::LEVEL, func: fleet_glass_ref::G1908.update, classes: &[1908], kind: SeaKind::Reflect(&fleet_glass_ref::G1908) },
    SeaPort { name: "1910 fleet glass", level: fleet_glass_ref::LEVEL, func: fleet_glass_ref::G1910.update, classes: &[1910], kind: SeaKind::Reflect(&fleet_glass_ref::G1910) },
    SeaPort { name: "1911 fleet glass", level: fleet_glass_ref::LEVEL, func: fleet_glass_ref::G1911.update, classes: &[1911], kind: SeaKind::Reflect(&fleet_glass_ref::G1911) },
    SeaPort { name: "1913 fleet glass", level: fleet_glass_ref::LEVEL, func: fleet_glass_ref::G1913.update, classes: &[1913], kind: SeaKind::Reflect(&fleet_glass_ref::G1913) },
    SeaPort { name: "1918 fleet glass", level: fleet_glass_ref::LEVEL, func: fleet_glass_ref::G1918.update, classes: &[1918], kind: SeaKind::Reflect(&fleet_glass_ref::G1918) },
];

pub const ARIDIA: usize = 8;
pub const ENV_OVERLAY: usize = 11;

/// Class 1848's tables (level01 labels; `0x30f0e0`): five meshes, the vertex counts, the position, normal and colour
/// pointer tables (`0x208120`'s fourth table is passed to the vertex function, which does not read it); FX 40 (0x28),
/// `ALPHA_1 = 0x2000000064` (FIX 0x20), CLAMP 0 (repeat), TEX1 0xff9000000260 (bilinear).
pub mod env_overlay_ref {
    pub const LEVEL: u32 = 1;
    pub const UPDATE_FN: u32 = 0x30_f208;
    pub const COUNTS: u32 = 0x20_2ea0;
    pub const POSITIONS: u32 = 0x20_8108;
    pub const NORMALS: u32 = 0x20_8138;
    pub const COLOURS: u32 = 0x20_8150;
    pub const MESHES: usize = 5;
    pub const FX: u16 = 0x28;
    pub const FIX: u8 = 0x20;
}

/// Kalebo's reflections (level16; the name is descriptive [L]): five classes, each a set of static meshes drawn like
/// Novalis' 1848 ([`env_overlay_ref`]) through `DrawEnvOverlayMesh` (level16 `0x1fc700`, clip on) with FX 0x29 at FIX
/// 0x40 (ALPHA 0x4000000064, CLAMP repeat, TEX1 bilinear) and the reflection map of 1848's `0x30ef18` without its scroll
/// (level16 `0x2e8b98` and its four byte-identical copies: the normal's sign cancels in the reflection, so
/// [`env_map_st`] with no scroll is the same map).
///
/// | address | what | port |
/// |---|---|---|
/// | update state 0 | the culled sets: each mesh's sphere ([`bound_sphere`]) into the level's table; 1947, 1948, 1953 also update and draw distance 0xff; → 1 | [`reflect_update`] (the spheres at load) |
/// | update state 1 | `RegisterDrawCallback(draw, m)` (list 1) | [`reflect_update`] |
/// | draw | the GS state, `DrawSpriteHelper_A`; per mesh (culled sets: `FastBSphereCheck(512, sphere)` not −1) the ST and the draw | rc-engine `sea_render` (`mesh_set_groups`) |
pub mod reflect_ref {
    pub const LEVEL: u32 = 16;
    pub const FX: u16 = 0x29;
    pub const FIX: u8 = 0x40;
    pub const FAR: f32 = 512.0;
    const ST_SCALE: f32 = 2.0;
    /// One class: its update and draw, the count / position / normal / colour pointer tables, the mesh count, the
    /// sphere check, the distances at init, the draw's FX and ALPHA FIX, the reflection map's factor (`0x2e9b38`: 2; Veldin
    /// Orbit's `0x2f54d8`: 4).
    #[derive(Debug, PartialEq)]
    pub struct Port {
        pub update: u32,
        pub draw: u32,
        pub counts: u32,
        pub positions: u32,
        pub normals: u32,
        pub colours: u32,
        pub meshes: usize,
        pub culled: bool,
        pub far_dists: bool,
        pub fx: u16,
        pub fix: u8,
        pub st_scale: f32,
    }
    pub const R1943: Port = Port { update: 0x2e_8e80, draw: 0x2e_8d40, counts: 0x16_1ef0, positions: 0x16_1f00, normals: 0x16_1f20, colours: 0x16_1f30, meshes: 3, culled: true, far_dists: false, fx: FX, fix: FIX, st_scale: ST_SCALE };
    pub const R1947: Port = Port { update: 0x2e_93b0, draw: 0x2e_9270, counts: 0x16_1f40, positions: 0x16_1f60, normals: 0x16_1f70, colours: 0x16_1f78, meshes: 1, culled: true, far_dists: true, fx: FX, fix: FIX, st_scale: ST_SCALE };
    pub const R1948: Port = Port { update: 0x2e_98e0, draw: 0x2e_97a0, counts: 0x16_1f90, positions: 0x16_1f98, normals: 0x16_1fa8, colours: 0x16_1fb0, meshes: 2, culled: true, far_dists: true, fx: FX, fix: FIX, st_scale: ST_SCALE };
    pub const R1952: Port = Port { update: 0x2e_9df0, draw: 0x2e_9ce0, counts: 0x16_1fb8, positions: 0x16_1fc8, normals: 0x16_1fe8, colours: 0x16_1ff8, meshes: 3, culled: false, far_dists: false, fx: FX, fix: FIX, st_scale: ST_SCALE };
    pub const R1953: Port = Port { update: 0x2e_a128, draw: 0x2e_9fe8, counts: 0x16_2008, positions: 0x16_2018, normals: 0x16_2038, colours: 0x16_2048, meshes: 3, culled: true, far_dists: true, fx: FX, fix: FIX, st_scale: ST_SCALE };
}

/// Veldin Orbit's window glass (level17; the name is descriptive [L]): six classes whose updates are the code of Kalebo's
/// 1952 ([`reflect_ref::R1952`]: state 0 → 1, then `RegisterDrawCallback(draw, m)` each tick; no sphere check, no
/// distances), each registering its own draw: FX 0x30 (the teal cloud map), `ALPHA_1 = 0x2000000064` (FIX 0x20), CLAMP
/// repeat, TEX1 bilinear, `DrawSpriteHelper_A`, then per mesh the reflection ST (`0x2f54d8` and its copies: 1952's
/// `0x2e9b38` with the factor 4 for 2) and `DrawEnvOverlayMesh` (level17 `0x1faf00`, clip on). The tables are `$gp`-relative (gp 0x166c00:
/// `0x2f3848` writes the death height both as 0x15f638 and as gp−0x75c8).
///
/// | class | update / draw | counts, positions, normals, colours (gp−) | meshes |
/// |---|---|---|---|
/// | 1907 | `0x2f57a0` / `0x2f5690` | 0x4810, 0x4808, 0x47f8, 0x47f0 | 2 |
/// | 1908 | `0x2f5ab8` / `0x2f59a8` | 0x47e0, 0x47c0, 0x47b0, 0x47a8 | 1 |
/// | 1910 | `0x2f5dd0` / `0x2f5cc0` | 0x47a0, 0x4798, 0x4788, 0x4780 | 2 |
/// | 1911 | `0x2f60e8` / `0x2f5fd8` | 0x4770, 0x4750, 0x4740, 0x4738 | 1 |
/// | 1913 | `0x2f6400` / `0x2f62f0` | 0x4730, 0x4728, 0x4718, 0x4710 | 2 |
/// | 1918 | `0x2f6718` / `0x2f6608` | 0x4700, 0x46e0, 0x46d0, 0x46c8 | 1 |
pub mod fleet_glass_ref {
    use super::reflect_ref::Port;
    pub const LEVEL: u32 = 17;
    const FX: u16 = 0x30;
    const FIX: u8 = 0x20;
    const ST_SCALE: f32 = 4.0;
    pub const G1907: Port = Port { update: 0x2f_57a0, draw: 0x2f_5690, counts: 0x16_23f0, positions: 0x16_23f8, normals: 0x16_2408, colours: 0x16_2410, meshes: 2, culled: false, far_dists: false, fx: FX, fix: FIX, st_scale: ST_SCALE };
    pub const G1908: Port = Port { update: 0x2f_5ab8, draw: 0x2f_59a8, counts: 0x16_2420, positions: 0x16_2440, normals: 0x16_2450, colours: 0x16_2458, meshes: 1, culled: false, far_dists: false, fx: FX, fix: FIX, st_scale: ST_SCALE };
    pub const G1910: Port = Port { update: 0x2f_5dd0, draw: 0x2f_5cc0, counts: 0x16_2460, positions: 0x16_2468, normals: 0x16_2478, colours: 0x16_2480, meshes: 2, culled: false, far_dists: false, fx: FX, fix: FIX, st_scale: ST_SCALE };
    pub const G1911: Port = Port { update: 0x2f_60e8, draw: 0x2f_5fd8, counts: 0x16_2490, positions: 0x16_24b0, normals: 0x16_24c0, colours: 0x16_24c8, meshes: 1, culled: false, far_dists: false, fx: FX, fix: FIX, st_scale: ST_SCALE };
    pub const G1913: Port = Port { update: 0x2f_6400, draw: 0x2f_62f0, counts: 0x16_24d0, positions: 0x16_24d8, normals: 0x16_24e8, colours: 0x16_24f0, meshes: 2, culled: false, far_dists: false, fx: FX, fix: FIX, st_scale: ST_SCALE };
    pub const G1918: Port = Port { update: 0x2f_6718, draw: 0x2f_6608, counts: 0x16_2500, positions: 0x16_2520, normals: 0x16_2530, colours: 0x16_2538, meshes: 1, culled: false, far_dists: false, fx: FX, fix: FIX, st_scale: ST_SCALE };
}

/// Pokitaru's pool overlays (level11; the names are descriptive [L]): two copies of one module, 1903 (`0x31e2f0`, draw
/// `0x31db50`) by the pool at (454, 560) and 1919 (`0x31f150`, draw `0x31e930`) by the one at (421, 557). Each draws
/// three layers of static strips through `DrawEnvOverlayMesh` (`0x21fda8`, clip on) with the stored ST plus a scroll
/// (`0x2739c0`, per lane) and the stored colours, as the module's first init rescales them (`0x31dd10` / `0x31eb70`, the
/// code of level12's `0x30bba0`: [`hoven_rescale`]): L0 by (0.7, 1, 0.9, 0.5), L1 by (0.25, 0.25, 0.25, 0.7), L2 by
/// (0.5, 0.5, 0.5, 1).
///
/// | address (A / B) | what | port |
/// |---|---|---|
/// | update state 0 | the init once per level (the `$gp` flag): the rescale; → 1; L0's three scrolls and the wobble's phases 0; L1's scrolls (0, 0.4·k); L2's (0, 0) | [`pool_update`] (the rescale at load) |
/// | update state 1 | camera z (0x1677c0) ≥ 170, `FastBSphereCheck(1000, centre)` in view and camera z > 180: d = \|centre − camera\|; d ≤ far → L0 scroll 0 and 2 by their speeds · dt (`0x31e048`, wrapped by 1 past ±1), scroll 1 the wobble (`0x31df00`: phases += (0.52, 0.62)·dt wrapped by 2 past ±1, scroll = 0.06·sin(π·phase)); the shimmer's alpha: 255 nearer than `near`, `trunc((1 − (d − near)/20)·255)` before `far`, else 0; then L2's scroll and L1's two (`0x31e208`, `0x31e120`); `RegisterDrawCallback(draw)` | [`pool_update`] |
/// | draw | TEX1 bilinear, CLAMP repeat, TEST 0x513f1, `DrawSpriteHelper_A`; B only: the camera below z 255 and in none of the cuboids pvar 0..2 → L0 and L2 not drawn; L0 with FX 0x2c, ALPHA 0x44 (scroll 0); alpha ≠ 0: FX 0x29, FIX = (base·alpha) >> 8, ALPHA 0x68 (scroll 1) and 0x62 (scroll 2); L1 with FX 0x2a, ALPHA 0x48, scrolls 0 and 1; L2 with FX 0x2b, ALPHA 0x48 | rc-engine `sea_render` ([`pool_drawn`]) |
pub mod pool_ref {
    pub const LEVEL: u32 = 11;
    /// One layer's tables: ST, position and colour pointer tables, the vertex counts, the strip count.
    pub type Layer = (u32, u32, u32, u32, usize);
    /// One copy of the module (level11 labels).
    #[derive(Debug, PartialEq)]
    pub struct Module {
        pub update: u32,
        pub draw: u32,
        pub layers: [Layer; 3],
        /// The centre (x, y, z, sphere radius).
        pub centre: u32,
        /// The shimmer's two passes over L0 (scrolls 1 and 2): additive (`0x68`, else subtractive `0x62`) and the word
        /// holding the FIX base each scales by the fade.
        pub shimmer: [(bool, u32); 2],
        /// The scroll speeds (s, t): L0's three, L1's two, L2's one.
        pub l0_speed: u32,
        pub l1_speed: u32,
        pub l2_speed: u32,
        /// The shimmer's full and zero distances, and the divisor of the fade between them (`(d − near) / span`; 08's
        /// copy multiplies by 1/64, the same value).
        pub near: f32,
        pub far: f32,
        pub span: f32,
        /// The update's camera heights (the update's minimum, the draw's): None on 08's copy.
        pub cam_z: Option<(f32, f32)>,
        /// The colour rescales of the first init, by layer.
        pub rescale: [[f32; 4]; 3],
        /// The layers' FX textures: L0, the shimmer, L1, L2.
        pub fx: [u16; 4],
        /// L0's pass: None `ALPHA 0x44` (`(Cs − Cd)·As + Cd`); Some(word) `FIX << 32 | 0x64` with the FIX in that word.
        pub l0_fix: Option<u32>,
        /// The draw's camera gate on L0 / L2 (B).
        pub gated: bool,
    }
    pub const A: Module = Module {
        update: 0x31_e2f0,
        draw: 0x31_db50,
        layers: [(0x16_2550, 0x16_2540, 0x16_2570, 0x16_2530, 4), (0x1f_f6f0, 0x1f_f6d0, 0x1f_f730, 0x1f_9a78, 8), (0x1f_99e8, 0x1f_99b8, 0x1f_9a48, 0x1f_4978, 11)],
        centre: 0x16_24e8,
        shimmer: [(true, 0x16_24fc), (false, 0x16_24fc)],
        l0_speed: 0x1f_14d0,
        l1_speed: 0x16_2508,
        l2_speed: 0x16_2520,
        near: 15.0,
        far: 35.0,
        span: 20.0,
        cam_z: Some((MIN_CAM_Z, DRAW_CAM_Z)),
        rescale: RESCALE,
        fx: FX,
        l0_fix: None,
        gated: false,
    };
    pub const B: Module = Module {
        update: 0x31_f150,
        draw: 0x31_e930,
        layers: [(0x21_5190, 0x21_5148, 0x21_5220, 0x20_7900, 17), (0x20_7888, 0x20_7860, 0x20_78d8, 0x20_0728, 9), (0x21_7710, 0x21_76f8, 0x21_7740, 0x21_5268, 5)],
        centre: 0x16_25b8,
        shimmer: [(true, 0x16_25cc), (false, 0x16_25cc)],
        l0_speed: 0x20_0710,
        l1_speed: 0x16_25d8,
        l2_speed: 0x16_25f0,
        near: 80.0,
        far: 100.0,
        span: 20.0,
        cam_z: Some((MIN_CAM_Z, DRAW_CAM_Z)),
        rescale: RESCALE,
        fx: FX,
        l0_fix: None,
        gated: true,
    };
    /// Batalia's copy (class 600, level08 `0x2f1548`, draw `0x2f0dc8`; its labels are level08's): the shimmer
    /// subtractive (FIX base 0x161dd4) then additive (0x161dd8), drawn at any alpha (FIX 0 changes nothing: the same
    /// picture as the pools' alpha gate), no camera heights, the fade over 20..84.
    pub const C: Module = Module {
        update: 0x2f_1548,
        draw: 0x2f_0dc8,
        layers: [(0x1e_82a0, 0x1e_8268, 0x1e_8310, 0x1e_2858, 13), (0x1e_27c8, 0x1e_2798, 0x1e_2828, 0x1d_df40, 11), (0x1d_dee0, 0x1d_dec0, 0x1d_df20, 0x1d_b518, 8)],
        centre: 0x16_1dc0,
        shimmer: [(false, 0x16_1dd4), (true, 0x16_1dd8)],
        l0_speed: 0x1d_b500,
        l1_speed: 0x16_1de0,
        l2_speed: 0x16_1df8,
        near: 20.0,
        far: 84.0,
        span: 64.0,
        cam_z: None,
        rescale: [[0.5, 0.5, f32::from_bits(0x3f26_6666), 0.5], [0.25, 0.25, 0.25, 1.0], [0.5, 0.5, 0.5, 1.0]],
        fx: [0x3a, 0x3b, 0x3c, 0x3d],
        l0_fix: None,
        gated: false,
    };
    /// Batalia's second copy (class 1649, level08 `0x3097b0`, draw `0x309030`): as [`C`] but L0 drawn with `ALPHA
    /// 0x40 << 32 | 0x64` (the word 0x162390), its L2 tables in the small data.
    pub const D: Module = Module {
        update: 0x30_97b0,
        draw: 0x30_9030,
        layers: [(0x1f_84e8, 0x1f_84c8, 0x1f_8528, 0x1f_4c60, 8), (0x1f_4c18, 0x1f_4c00, 0x1f_4c48, 0x1f_01c8, 6), (0x16_23d8, 0x16_23d0, 0x16_23e8, 0x16_23c8, 2)],
        centre: 0x16_2380,
        shimmer: [(false, 0x16_2394), (true, 0x16_2398)],
        l0_speed: 0x1e_f790,
        l1_speed: 0x16_23a0,
        l2_speed: 0x16_23b8,
        l0_fix: Some(0x16_2390),
        ..C
    };
    pub const C_LEVEL: u32 = 8;
    /// The pools' colour rescales of the first init, by layer.
    pub const RESCALE: [[f32; 4]; 3] = [[0.7, 1.0, 0.9, 0.5], [0.25, 0.25, 0.25, 0.7], [0.5, 0.5, 0.5, 1.0]];
    /// The pools' layers' FX textures: L0, the shimmer, L1, L2.
    pub const FX: [u16; 4] = [0x2c, 0x29, 0x2a, 0x2b];
    /// The update's camera heights.
    pub const MIN_CAM_Z: f32 = 170.0;
    pub const DRAW_CAM_Z: f32 = 180.0;
    /// B's gate height.
    pub const GATE_Z: f32 = 255.0;
    /// `FastBSphereCheck(1000, centre)`.
    pub const VIEW_FAR: f32 = 1000.0;
    /// The wobble (`0x31df00(0.52, 0.62, 0.06)`).
    pub const WOBBLE: [f32; 3] = [f32::from_bits(0x3f05_1eb8), f32::from_bits(0x3f1e_b852), f32::from_bits(0x3d75_c28f)];
}

/// A pool overlay on the loaded level: the three layers (colours rescaled) and the module's words.
#[derive(Clone, Debug, PartialEq)]
pub struct PoolData {
    pub layers: [Vec<StripMesh>; 3],
    pub centre: [f32; 4],
    /// The shimmer passes' FIX bases ([`pool_ref::Module::shimmer`]).
    pub fix: [u8; 2],
    /// L0's FIX ([`pool_ref::Module::l0_fix`]).
    pub l0_fix: Option<u8>,
    pub l0_speed: [[f32; 2]; 3],
    pub l1_speed: [[f32; 2]; 2],
    pub l2_speed: [f32; 2],
}

/// A pool overlay's run-time globals (A: 0x1ff750, 0x162598, 0x1625a8, 0x1625b0, 0x162590; B: 0x217758, 0x162618,
/// 0x162628, 0x162630, 0x162610).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PoolRun {
    pub l0: [[f32; 2]; 3],
    pub l1: [[f32; 2]; 2],
    pub l2: [f32; 2],
    pub phase: [f32; 2],
    pub alpha: u8,
}

/// The pool draw's gate on L0 and L2 (module B): the camera above z 255, or in one of the cuboids pvar 0..2.
pub fn pool_drawn(m: &pool_ref::Module, pvars: &[u8], volumes: &rc_formats::volumes::Volumes, camera: [f32; 3]) -> bool {
    !m.gated || pool_ref::GATE_Z < camera[2] || (0..3).any(|k| pvars.len() >= 4 * k + 4 && crate::moby_update::triggers::point_in_cuboid(volumes, camera, pvar::i32(pvars, 4 * k)))
}

/// The shimmer's FIX: `(base·alpha) >> 8`.
pub fn pool_fix(base: u8, alpha: u8) -> u8 { ((base as i32 * alpha as i32) >> 8) as u8 }

fn load_pool(ov: &Overlay, rel: &Relocation, m: &pool_ref::Module) -> Result<PoolData, rc_formats::FormatError> {
    let at = |l: u32| rel.data(l).ok_or_else(|| rc_formats::FormatError::Invalid(format!("label {l:#x} not found")));
    let f = |l: u32| -> Result<f32, rc_formats::FormatError> { ov.f32(at(l)?) };
    let pair = |l: u32, k: u32| -> Result<[f32; 2], rc_formats::FormatError> { Ok([f(l + 8 * k)?, f(l + 8 * k + 4)?]) };
    let mut layers: [Vec<StripMesh>; 3] = Default::default();
    for (k, l) in m.layers.iter().enumerate() {
        layers[k] = fs::parse_strip_meshes(ov, at(l.0)?, at(l.1)?, at(l.2)?, at(l.3)?, l.4)?;
        hoven_rescale(&mut layers[k], m.rescale[k]);
    }
    Ok(PoolData {
        layers,
        centre: [f(m.centre)?, f(m.centre + 4)?, f(m.centre + 8)?, f(m.centre + 12)?],
        fix: [ov.u32(at(m.shimmer[0].1)?)? as u8, ov.u32(at(m.shimmer[1].1)?)? as u8],
        l0_fix: match m.l0_fix {
            Some(l) => Some(ov.u32(at(l)?)? as u8),
            None => None,
        },
        l0_speed: [pair(m.l0_speed, 0)?, pair(m.l0_speed, 1)?, pair(m.l0_speed, 2)?],
        l1_speed: [pair(m.l1_speed, 0)?, pair(m.l1_speed, 1)?],
        l2_speed: pair(m.l2_speed, 0)?,
    })
}

/// Level 9's extras in 317's draw callback `0x2ef750` (the reference level's labels; module doc).
pub mod gaspar_ref {
    pub const LEVEL: u32 = 9;
    /// The callback: its copy on the loaded level makes the extras present.
    pub const CALLBACK: u32 = 0x2e_f750;
    /// `0x21e8c0(t, 0x1f3080, 0x6c, 0x2c + (q & 15), 0x2c + ((q + 1) & 15))`, q = counter / 20.
    pub const FLOWS: u32 = 0x1f_3080;
    pub const FLOW_COUNT: usize = 0x6c;
    pub const FLOW_FX: u16 = 0x2c;
    pub const FLOW_FRAMES: u8 = 16;
    pub const FLOW_PERIOD: u8 = 20;
    /// The strip state packet `0x21e8c0` sends (TEST, TEX1, CLAMP, ALPHA of both contexts).
    pub const FLOW_PACKET: u32 = 0x16_eaa0;
    /// `0x2c1978(0x1fc740, 0x1f63c0, 10)`: meshes textured with the grid's image.
    pub const GRID_MESHES: u32 = 0x1f_63c0;
    pub const GRID_MESH_COUNT: usize = 10;
}

/// Level 2's 854 (callback `0x2ea048`): the seven grid records in draw order and the pvar word of the cuboid that gates
/// each (0: the first, 1: the next five, 2: the last), and the shared image / fog record the callback builds on its
/// stack for `0x2a4818` (near 0, far 260080 (0x487dfc00), intensities 255 → 0, FOGCOL (0x0f, 0x0f, 0x19), 30 ticks a
/// frame, FX 0x28 + 64 frames, FIX 0x80).
pub mod aridia_ref {
    pub const LEVEL: u32 = 2;
    pub const CALLBACK: u32 = 0x2e_a048;
    pub const RECORDS: [(u32, usize); 7] = [(0x1f_3c00, 0), (0x1f_3cd0, 1), (0x1f_3d30, 1), (0x1f_3d90, 1), (0x1f_3dd0, 1), (0x1f_3e10, 1), (0x1f_3c60, 2)];
    /// The init `0x2a47f8` → `0x2a40d0(1/6)` (`0x3e2aaaab`).
    pub const SCALE: f32 = f32::from_bits(0x3e2a_aaab);
    pub const FOG: [f32; 4] = [0.0, 260080.0, 255.0, 0.0];
    pub const FOG_RGB: [u8; 3] = [0x0f, 0x0f, 0x19];
    pub const ANIM: super::GridAnim = super::GridAnim { tex: 0x28, frames: 0x40, period: 0x1e };
    pub const FIX: u8 = 0x80;
}

/// An animated liquid image: frames `tex + (c/p mod frames)` and the next one blended by the tick's fraction
/// (`0x277b20` / `0x26d240`; the renderer's `grid_image`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GridAnim {
    pub tex: u16,
    pub frames: u8,
    pub period: u8,
}

/// Where a liquid mesh pass's texture comes from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeshTex {
    /// FX texture n (`GetEffectTex`).
    Fx(u16),
    /// An animated blend of FX frames ([`GridAnim`]): the lava flows' `0x21e560` + `0x26d240`.
    Anim(GridAnim),
    /// The port's grid image, its fog and its FIX (the grid-textured meshes of `0x2c1978`).
    Grid,
}

/// How a liquid mesh pass gets its ST.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeshSt {
    /// The stored ST · scale.
    Stored { scale: f32 },
    /// The lava flows' scroll: `(s, t + k·f + add)` with `f = 1 − (counter & 0x7ff)/2048`.
    Flow { k: f32, add: f32 },
    /// The two-texture module's sphere map `0x2667fc` from the position, the stored normal and the camera.
    SphereMap,
    /// Class 1848's reflection map `0x30ef18` plus the port's scroll ([`env_map_st`]); `scale` the map's factor (2; Veldin
    /// Orbit's glass 4).
    EnvMap { scale: f32 },
}

/// The vertex colours of a mesh set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeshColour {
    /// One RGBA for every vertex (the two-texture module 0x80808080, the grid meshes 0x00757c8e).
    Const(u32),
    /// The lava flows' 256-entry pattern `0x80787070 + ((i·0x89) & 15)·0x20200` by vertex index.
    Flow,
    /// The strips' stored colours.
    Stored,
}

/// One pass of a liquid mesh set: ALPHA `FIX << 32 | 0x64` (`(Cs − Cd)·FIX + Cd`), Z test GEQUAL (TEST 0x50000).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshPass {
    pub tex: MeshTex,
    pub st: MeshSt,
    /// The pass's FIX (ignored for [`MeshTex::Grid`], which uses the grid's).
    pub fix: u8,
}

/// Liquid meshes drawn through the generic strip emitters (level01 `0x21fda8` one pass, `0x21fa98` two passes: the
/// second in GS context 2), each mesh culled by `FastBSphereCheck(256, sphere)`.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshSet {
    pub meshes: Vec<fs::LiquidMesh>,
    pub colour: MeshColour,
    pub passes: Vec<MeshPass>,
    /// In a grid port's extras: drawn before the grid's blocks (the lava flows: `0x21e8c0` precedes `0x2c1920`).
    pub before_grid: bool,
    /// The `FastBSphereCheck` distance of the meshes' spheres (256; Kalebo's reflections 512).
    pub far: f32,
}

/// Class 1848's per-vertex ST (`0x30ef18`): `e = unit(p − cam)` (0x167240), `n' = unit(−n)`, `r = unit(e − 2(n'·e)n')`,
/// `r.z += 1`, `l = |r|`; ST = `2·(r.x/(2l) + 0.5) + scroll.s`, `2·(r.y/(2l) + 0.5) + scroll.t`.
pub fn env_map_st(p: [f32; 3], n: [f32; 3], cam: [f32; 3], scroll: [f32; 2], scale: f32) -> [f32; 2] {
    let unit = |v: [f32; 3]| {
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        v.map(|x| x / l)
    };
    let e = unit([p[0] - cam[0], p[1] - cam[1], p[2] - cam[2]]);
    let m = unit([-n[0], -n[1], -n[2]]);
    let d = m[0] * e[0] + m[1] * e[1] + m[2] * e[2];
    let mut r = unit(std::array::from_fn(|k| e[k] - m[k] * (d + d)));
    r[2] += 1.0;
    let l = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
    let (u, v) = (r[0] / (l + l) + 0.5, r[1] / (l + l) + 0.5);
    [u * scale + scroll[0], v * scale + scroll[1]]
}

/// The lava flows' vertex colour `i` (`0x21e8c0`'s 256 words at the scratchpad).
pub fn flow_colour(i: usize) -> u32 { 0x8078_7070u32.wrapping_add(((i as u32).wrapping_mul(0x89) & 0xf) * 0x2_0200) }

/// The lava flows' ST lane offset for tick `counter`: `1 − (counter & 0x7ff)·(1/2048)`.
pub fn flow_offset(counter: u64) -> f32 { 1.0 - (counter & 0x7ff) as f32 * 0.000_488_281_25 }

/// The two-texture module's sphere-map ST (`0x2667fc`, VU0): `e = 0.45·unit(cam − p)`, `v = −e`, `d = v·n` (n as
/// stored); `d > 0` (as an integer: positive and not ±0) → `e`, else `v + 2(n·d − v)`; ST = that's (x, y) + 0.5.
pub fn sphere_map_st(p: [f32; 3], n: [f32; 3], cam: [f32; 3]) -> [f32; 2] {
    let d = [cam[0] - p[0], cam[1] - p[1], cam[2] - p[2]];
    let q = 0.45 / (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let e = d.map(|x| x * q);
    let v = e.map(|x| 0.0 - x);
    let dot = v[0] * n[0] + v[1] * n[1] + v[2] * n[2];
    let r = if dot.to_bits() as i32 > 0 {
        e
    } else {
        let m: [f32; 3] = std::array::from_fn(|k| n[k] * dot - v[k]);
        std::array::from_fn(|k| v[k] + (m[k] + m[k]))
    };
    [r[0] + 0.5, r[1] + 0.5]
}

/// Port indices as the `ClassUpdate::Sea` payload.
pub fn ids() -> impl Iterator<Item = u8> { 0..PORTS.len() as u8 }

/// A liquid grid on the loaded level: the state record and the module's tables.
#[derive(Clone, Debug, PartialEq)]
pub struct GridData {
    pub grid: LiquidGrid,
    pub module: LiquidGridModule,
    /// The level callback's own meshes after the grid (level 9's 317: [`gaspar_ref`]): the lava flows, then the
    /// grid-textured meshes.
    pub extras: Vec<MeshSet>,
}

/// 854's data: the grid records with their gate (pvar word), the module, and the shared image and fog.
#[derive(Clone, Debug, PartialEq)]
pub struct GridSetData {
    pub grids: Vec<(LiquidGrid, usize)>,
    pub module: LiquidGridModule,
    pub anim: GridAnim,
    pub fog: [f32; 4],
    pub fog_rgb: [u8; 3],
    pub fix: u8,
}

/// The animation of a grid record (+0x3c first FX, +0x3e frames, +0x33 ticks a frame).
pub fn grid_anim(g: &LiquidGrid) -> GridAnim { GridAnim { tex: g.tex, frames: g.frames, period: g.period } }

/// Class 1901's strips and globals on the loaded level.
#[derive(Clone, Debug, PartialEq)]
pub struct HovenData {
    /// Group 1 (the camera-cuboid group) and group 2 (the two-layer group), colours as the init leaves them.
    pub groups: [Vec<StripMesh>; 2],
    /// The second layer's ALPHA FIX (`0x1620d4`).
    pub fix2: u8,
    /// Group 1's scroll speed (s, t) and the layers' (s, t) per second.
    pub g1_speed: [f32; 2],
    pub layer_speed: [[f32; 2]; 2],
}

/// One sea port's level data.
#[derive(Clone, Debug, PartialEq)]
pub enum SeaData {
    Grid(Box<GridData>),
    Ocean(OceanTables),
    Hoven(Box<HovenData>),
    GridSet(Box<GridSetData>),
    Meshes(Box<MeshSet>),
    Pool(Box<PoolData>),
}

/// A port present on the loaded level (its update has a copy in the level's overlay) and its data.
#[derive(Clone, Debug, PartialEq)]
pub struct SeaPortData {
    pub port: usize,
    pub data: SeaData,
}

/// `0x30bba0(r, g, b, a, table, n, counts, 1)`: every vertex colour of the group becomes `(trunc(r·255), trunc(g·255),
/// trunc(b·255), trunc(a·A))`.
pub fn hoven_rescale(strips: &mut [StripMesh], k: [f32; 4]) {
    let c = [k[0] * 255.0, k[1] * 255.0, k[2] * 255.0].map(|x| x as u32 & 0xff);
    for s in strips {
        for v in &mut s.rgba { *v = (((k[3] * (*v >> 24) as f32) as u32 & 0xff) << 24) | c[2] << 16 | c[1] << 8 | c[0]; }
    }
}

/// The sea ports' data on the level `target` (`ov` its raw overlay), each read through the relocation from its reference
/// level (`rel_of(level)`). A port whose update has no copy on the level is absent; one whose tables do not parse is
/// reported and skipped.
pub fn load<'t>(ov: &Overlay, target: &'t LevelOverlay, rel_of: &dyn Fn(u32) -> Option<Relocation<'t>>) -> Vec<SeaPortData> {
    let _ = target;
    let mut out = Vec::new();
    let module = rel_of(fs::grid_ref::LEVEL).and_then(|r| r.data(fs::grid_ref::ST_BLOCK)).map(|a| LiquidGridModule::parse(ov, a).map_err(|e| e.to_string()));
    for (i, p) in PORTS.iter().enumerate() {
        let Some(rel) = rel_of(p.level) else { continue };
        if rel.func(p.func).is_none() { continue; }
        let data = match p.kind {
            SeaKind::Grid(g) => {
                let Some(Ok(m)) = module.clone() else {
                    eprintln!("sea: {}: the liquid grid module's tables are not on this level", p.name);
                    continue;
                };
                let Some(a) = rel.data(g.state) else {
                    eprintln!("sea: {}: state record {:#x} not found", p.name, g.state);
                    continue;
                };
                match LiquidGrid::parse(ov, a) {
                    Ok(grid) => {
                        let extras = rel_of(gaspar_ref::LEVEL).filter(|r| r.func(gaspar_ref::CALLBACK).is_some()).map_or_else(Vec::new, |r| {
                            gaspar_extras(ov, &r).unwrap_or_else(|e| {
                                eprintln!("sea: {}: the lava meshes: {e}", p.name);
                                Vec::new()
                            })
                        });
                        SeaData::Grid(Box::new(GridData { grid, module: m, extras }))
                    }
                    Err(e) => {
                        eprintln!("sea: {}: {e}", p.name);
                        continue;
                    }
                }
            }
            SeaKind::GridSet => {
                let Some(Ok(m)) = module.clone() else {
                    eprintln!("sea: {}: the liquid grid module's tables are not on this level", p.name);
                    continue;
                };
                match aridia_grids(ov, &rel) {
                    Ok(grids) => SeaData::GridSet(Box::new(GridSetData {
                        grids,
                        module: m,
                        anim: aridia_ref::ANIM,
                        fog: aridia_ref::FOG,
                        fog_rgb: aridia_ref::FOG_RGB,
                        fix: aridia_ref::FIX,
                    })),
                    Err(e) => {
                        eprintln!("sea: {}: {e}", p.name);
                        continue;
                    }
                }
            }
            SeaKind::EnvOverlay => match env_overlay_set(ov, &rel) {
                Ok(set) => SeaData::Meshes(Box::new(set)),
                Err(e) => {
                    eprintln!("sea: {}: {e}", p.name);
                    continue;
                }
            },
            SeaKind::TwoTex(t) => match two_tex_set(ov, &rel, &t) {
                Ok(set) => SeaData::Meshes(Box::new(set)),
                Err(e) => {
                    eprintln!("sea: {}: {e}", p.name);
                    continue;
                }
            },
            SeaKind::Ocean => {
                let (Some(s), Some(v)) = (rel.data(fs::ocean_ref::SCALE), rel.data(fs::ocean_ref::SPEED)) else { continue };
                match OceanTables::parse(ov, s, v) {
                    Ok(t) => SeaData::Ocean(t),
                    Err(e) => {
                        eprintln!("sea: {}: {e}", p.name);
                        continue;
                    }
                }
            }
            SeaKind::Reflect(r) => match reflect_set(ov, &rel, r) {
                Ok(set) => SeaData::Meshes(Box::new(set)),
                Err(e) => {
                    eprintln!("sea: {}: {e}", p.name);
                    continue;
                }
            },
            SeaKind::Pool(m) => match load_pool(ov, &rel, m) {
                Ok(d) => SeaData::Pool(Box::new(d)),
                Err(e) => {
                    eprintln!("sea: {}: {e}", p.name);
                    continue;
                }
            },
            SeaKind::Hoven => match load_hoven(ov, &rel) {
                Ok(h) => SeaData::Hoven(Box::new(h)),
                Err(e) => {
                    eprintln!("sea: {}: {e}", p.name);
                    continue;
                }
            },
        };
        out.push(SeaPortData { port: i, data });
    }
    out
}

fn label(rel: &Relocation, l: u32) -> Result<u32, rc_formats::FormatError> { rel.data(l).ok_or_else(|| rc_formats::FormatError::Invalid(format!("label {l:#x} not found"))) }

/// Level 9's 317 extras ([`gaspar_ref`]): the lava flows (two passes of the animated FX blend, FIX of the packet's
/// contexts 1 / 2, ST scrolled by [`flow_offset`] once and twice + 0.5) and the ten grid-textured meshes (ST · 0.5,
/// colour 0x00757c8e).
fn gaspar_extras(ov: &Overlay, rel: &Relocation) -> Result<Vec<MeshSet>, rc_formats::FormatError> {
    use gaspar_ref as g;
    let fix = fs::strip_state_fix(ov, label(rel, g::FLOW_PACKET)?)?;
    let anim = MeshTex::Anim(GridAnim { tex: g::FLOW_FX, frames: g::FLOW_FRAMES, period: g::FLOW_PERIOD });
    let flows = MeshSet {
        meshes: fs::parse_flow_meshes(ov, label(rel, g::FLOWS)?, g::FLOW_COUNT)?,
        colour: MeshColour::Flow,
        passes: vec![MeshPass { tex: anim, st: MeshSt::Flow { k: 1.0, add: 0.0 }, fix: fix[0] }, MeshPass { tex: anim, st: MeshSt::Flow { k: 2.0, add: 0.5 }, fix: fix[1] }],
        before_grid: true,
        far: 256.0,
    };
    let grid = MeshSet {
        meshes: fs::parse_mesh_records(ov, label(rel, g::GRID_MESHES)?, g::GRID_MESH_COUNT, false)?,
        colour: MeshColour::Const(0x0075_7c8e),
        passes: vec![MeshPass { tex: MeshTex::Grid, st: MeshSt::Stored { scale: 0.5 }, fix: 0 }],
        before_grid: false,
        far: 256.0,
    };
    Ok(vec![flows, grid])
}

/// 854's grid records ([`aridia_ref`]), each a full liquid grid record (only its geometry, cull distance, colours and
/// flags are read by the draw), with its gate.
fn aridia_grids(ov: &Overlay, rel: &Relocation) -> Result<Vec<(LiquidGrid, usize)>, rc_formats::FormatError> {
    aridia_ref::RECORDS.iter().map(|&(l, gate)| Ok((LiquidGrid::parse(ov, label(rel, l)?)?, gate))).collect()
}

/// A two-texture strip class's meshes and passes: FX a with the stored ST (FIX of context 1), FX b with the sphere map
/// (FIX of context 2), colour 0x80808080.
fn two_tex_set(ov: &Overlay, rel: &Relocation, t: &TwoTexPort) -> Result<MeshSet, rc_formats::FormatError> {
    let fix = fs::strip_state_fix(ov, label(rel, t.packet)?)?;
    Ok(MeshSet {
        meshes: fs::parse_mesh_records(ov, label(rel, t.table)?, t.count, true)?,
        colour: MeshColour::Const(0x8080_8080),
        passes: vec![
            MeshPass { tex: MeshTex::Fx(t.fx[0]), st: MeshSt::Stored { scale: 1.0 }, fix: fix[0] },
            MeshPass { tex: MeshTex::Fx(t.fx[1]), st: MeshSt::SphereMap, fix: fix[1] },
        ],
        before_grid: false,
        far: 256.0,
    })
}

/// Class 1848's meshes ([`env_overlay_ref`]): one pass of FX 40 with the reflection map, the stored colours, FIX 0x20.
fn env_overlay_set(ov: &Overlay, rel: &Relocation) -> Result<MeshSet, rc_formats::FormatError> {
    use env_overlay_ref as e;
    Ok(MeshSet {
        meshes: fs::parse_pointer_meshes(ov, label(rel, e::COUNTS)?, label(rel, e::POSITIONS)?, label(rel, e::NORMALS)?, label(rel, e::COLOURS)?, e::MESHES)?,
        colour: MeshColour::Stored,
        passes: vec![MeshPass { tex: MeshTex::Fx(e::FX), st: MeshSt::EnvMap { scale: 2.0 }, fix: e::FIX }],
        before_grid: false,
        far: 256.0,
    })
}

/// A reflection class's meshes ([`reflect_ref`]): FX 0x29 with the reflection map (no scroll), the stored colours, FIX
/// 0x40; with `culled`, each mesh's sphere as the first update computes it ([`bound_sphere`]), checked at 512.
fn reflect_set(ov: &Overlay, rel: &Relocation, r: &reflect_ref::Port) -> Result<MeshSet, rc_formats::FormatError> {
    let mut meshes = fs::parse_pointer_meshes(ov, label(rel, r.counts)?, label(rel, r.positions)?, label(rel, r.normals)?, label(rel, r.colours)?, r.meshes)?;
    if r.culled {
        for m in &mut meshes { m.sphere = m.strips.first().map(|s| bound_sphere(&s.pos)); }
    }
    Ok(MeshSet {
        meshes,
        colour: MeshColour::Stored,
        passes: vec![MeshPass { tex: MeshTex::Fx(r.fx), st: MeshSt::EnvMap { scale: r.st_scale }, fix: r.fix }],
        before_grid: false,
        far: reflect_ref::FAR,
    })
}

/// The reflections' sphere of a mesh (their first update, e.g. level16 `0x2e8e80`): the centre of the box of the
/// positions (from ±1024), the radius the farthest position (`vec_distance`).
pub fn bound_sphere(pos: &[[f32; 3]]) -> [f32; 4] {
    let (mut lo, mut hi) = ([1024.0f32; 3], [-1024.0f32; 3]);
    for p in pos {
        for k in 0..3 {
            if hi[k] < p[k] { hi[k] = p[k]; }
            if p[k] < lo[k] { lo[k] = p[k]; }
        }
    }
    let c: [f32; 3] = std::array::from_fn(|k| (hi[k] + lo[k]) * 0.5);
    let mut r = 0.0f32;
    for p in pos {
        let d = ((p[0] - c[0]) * (p[0] - c[0]) + (p[1] - c[1]) * (p[1] - c[1]) + (p[2] - c[2]) * (p[2] - c[2])).sqrt();
        if r < d { r = d; }
    }
    [c[0], c[1], c[2], r]
}

fn load_hoven(ov: &Overlay, rel: &Relocation) -> Result<HovenData, rc_formats::FormatError> {
    use fs::hoven_ref as h;
    let at = |l: u32| rel.data(l).ok_or_else(|| rc_formats::FormatError::Invalid(format!("label {l:#x} not found")));
    let group = |g: (u32, u32, u32, u32, usize)| -> Result<Vec<StripMesh>, rc_formats::FormatError> {
        fs::parse_strip_meshes(ov, at(g.0)?, at(g.1)?, at(g.2)?, at(g.3)?, g.4)
    };
    let mut groups = [group(h::G1)?, group(h::G2)?];
    // The first init's 0x30bba0 calls (level12 0x30c030 / 0x30c060).
    hoven_rescale(&mut groups[1], [f32::from_bits(0x3dcc_cccd), 0.5, f32::from_bits(0x3ecc_cccd), 0.5]);
    hoven_rescale(&mut groups[0], [f32::from_bits(0x3dcc_cccd), f32::from_bits(0x3f19_999a), f32::from_bits(0x3ecc_cccd), 0.5]);
    let f = |l: u32| -> Result<f32, rc_formats::FormatError> { ov.f32(at(l)?) };
    Ok(HovenData {
        groups,
        fix2: ov.u32(at(h::FIX2)?)? as u8,
        g1_speed: [f(h::G1_SPEED)?, f(h::G1_SPEED + 4)?],
        layer_speed: [[f(h::LAYER_SPEED)?, f(h::LAYER_SPEED + 4)?], [f(h::LAYER_SPEED + 8)?, f(h::LAYER_SPEED + 12)?]],
    })
}

/// One port's run-time state (the globals and record fields its update and callback share).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SeaRun {
    /// The module init has run (grid; the level-8 `$gp` flag; 1901's colour rescale flag).
    pub inited: bool,
    /// Grid: the state record's z (`+0x08`) as the class leaves it, and its FIX (`+0x3f`).
    pub z: f32,
    pub fix: u8,
    /// Ocean / Hoven: the layers' ST scroll (s, t) (1111: `0x161e08 + 8·layer`; 1901: `0x1620f8 + 8·layer`).
    pub scroll: [[f32; 2]; 2],
    /// 1901 group 1's scroll (`0x162108`, `0x16210c`).
    pub g1_scroll: [f32; 2],
    /// 1111: the camera at the last update (`$gp − 0x43c0`), the sea z it draws (`0x161e18`) and its base (`0x161e1c`).
    pub prev_cam: [f32; 3],
    pub sea_z: f32,
    pub base_z: f32,
    /// The pool overlays' scrolls and shimmer ([`PoolRun`]).
    pub pool: PoolRun,
}

/// The sea ports' state (in `WaterWorld`), by port index.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SeaState {
    pub run: [SeaRun; PORTS.len()],
}

/// The port's level data (None: not on this level, or the water data is not loaded).
pub fn data(water: &WaterWorld, port: usize) -> Option<&SeaData> {
    water.data.as_ref()?.sea.iter().find(|d| d.port == port).map(|d| &d.data)
}

/// `(*moby+0x74)(moby)` of sea port `port`. Without the level's data for it nothing runs.
pub fn update(w: &mut World, id: MobyId, port: u8) {
    let port = port as usize;
    let Some(p) = PORTS.get(port) else { return };
    if data(&w.svc.water, port).is_none() { return; }
    match p.kind {
        SeaKind::Grid(g) => grid_update(w, id, port, g),
        SeaKind::Ocean => ocean_update(w, id, port),
        SeaKind::Hoven => hoven_update(w, id, port),
        SeaKind::GridSet => grid_set_update(w, id, port),
        SeaKind::TwoTex(t) => two_tex_update(w, id, port, &t),
        SeaKind::EnvOverlay => env_overlay_update(w, id, port),
        SeaKind::Pool(m) => pool_update(w, id, port, m),
        SeaKind::Reflect(r) => reflect_update(w, id, port, r),
    }
}

/// Kalebo's reflections ([`reflect_ref`]) and Veldin Orbit's glass ([`fleet_glass_ref`]).
fn reflect_update(w: &mut World, id: MobyId, port: usize, r: &reflect_ref::Port) {
    match w.m(id).state {
        0 => {
            w.svc.water.sea.run[port].inited = true;
            let m = w.mm(id);
            if r.far_dists {
                m.update_dist = 0xff;
                m.draw_dist = 0xff;
            }
            m.state = 1;
        }
        1 => register(w, id, port, DrawList::AfterMobys),
        _ => {}
    }
}

/// 1903 / 1919 (level11 `0x31e2f0` / `0x31f150`; [`pool_ref`]).
fn pool_update(w: &mut World, id: MobyId, port: usize, m: &pool_ref::Module) {
    let Some(SeaData::Pool(d)) = data(&w.svc.water, port) else { return };
    let (centre, l0s, l1s, l2s) = (d.centre, d.l0_speed, d.l1_speed, d.l2_speed);
    match w.m(id).state {
        0 => {
            w.mm(id).state = 1;
            let r = &mut w.svc.water.sea.run[port];
            r.inited = true;
            r.pool = PoolRun { l1: [[0.0, 0.0], [0.0, 0.4]], ..PoolRun::default() };
        }
        1 => {
            let c = fv(w.camera);
            let cam = [c[0], c[1], c[2]];
            let (min_z, draw_z) = m.cam_z.unwrap_or((f32::NEG_INFINITY, f32::NEG_INFINITY));
            if cam[2] < min_z { return; }
            if !crate::moby_update::creature::fx::in_view(w, pool_ref::VIEW_FAR, centre, centre[3]) || cam[2] <= draw_z { return; }
            let v = [centre[0] - cam[0], centre[1] - cam[1], centre[2] - cam[2]];
            let dist = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            let r = &mut w.svc.water.sea.run[port].pool;
            let step = |x: &mut [f32; 2], sp: [f32; 2]| for k in 0..2 { x[k] = wrap(x[k] + sp[k] * DT); };
            if dist <= m.far {
                step(&mut r.l0[0], l0s[0]);
                let [a, b, amp] = pool_ref::WOBBLE;
                for (k, rate) in [a, b].into_iter().enumerate() {
                    let mut x = r.phase[k] + rate * DT;
                    if 1.0 < x { x -= 2.0; }
                    if x < -1.0 { x += 2.0; }
                    r.phase[k] = x;
                }
                r.l0[1] = r.phase.map(|x| (x * std::f32::consts::PI).sin() * amp);
                step(&mut r.l0[2], l0s[2]);
                r.alpha = if dist < m.near {
                    0xff
                } else if dist < m.far {
                    ((1.0 - (dist - m.near) / m.span) * 255.0) as i32 as u8
                } else {
                    0
                };
            }
            step(&mut r.l2, l2s);
            step(&mut r.l1[0], l1s[0]);
            step(&mut r.l1[1], l1s[1]);
            register(w, id, port, DrawList::AfterMobys);
        }
        _ => {}
    }
}

/// 1848 (level01 `0x30f208`): state 0 zeroes the scroll (0x162110, 0x162114), → 1; state 1 adds `dt·0.025` (dt =
/// 0x15ed7c) to each lane, wraps it (above 1 → −1, below −1 → +1) and registers the draw `0x30f0e0` on list 1.
fn env_overlay_update(w: &mut World, id: MobyId, port: usize) {
    match w.m(id).state {
        0 => {
            let r = &mut w.svc.water.sea.run[port];
            (r.inited, r.scroll[0]) = (true, [0.0; 2]);
            w.mm(id).state = 1;
        }
        1 => {
            let s = &mut w.svc.water.sea.run[port].scroll[0];
            for x in s.iter_mut() { *x = wrap(*x + DT * 0.025); }
            register(w, id, port, DrawList::AfterMobys);
        }
        _ => {}
    }
}

/// 854 (level02 `0x2ea198`): state 0 runs the module init `0x2a47f8` → `0x2a40d0(1/6)` (the module's ST · 1/6; its
/// `sb 0x80, 0x3f(a0)` lands in the moby, whose +0x3f no one reads: n/a), update distance 0xff, → 1; state 1 registers
/// the callback `0x2ea048` on the after-ties list (`0x20a390`).
fn grid_set_update(w: &mut World, id: MobyId, port: usize) {
    match w.m(id).state {
        0 => {
            let r = &mut w.svc.water.sea.run[port];
            (r.inited, r.fix) = (true, aridia_ref::FIX);
            let m = w.mm(id);
            m.update_dist = 0xff;
            m.state = 1;
        }
        1 => register(w, id, port, DrawList::AfterTies),
        _ => {}
    }
}

/// 293 (level12 `0x2e7208`) / 1418 (level14 `0x307a80`): state 0 → update distance 0xff, state 1; state 1 registers the
/// callback on the after-ties list, 293 only while its pvar +0xc cuboid is −1 or holds the camera (0x167240,
/// `0x278850`).
fn two_tex_update(w: &mut World, id: MobyId, port: usize, t: &TwoTexPort) {
    match w.m(id).state {
        0 => {
            w.svc.water.sea.run[port].inited = true;
            let m = w.mm(id);
            m.update_dist = 0xff;
            m.state = 1;
        }
        1 => {
            if let Some(o) = t.gate {
                if w.m(id).pvars.len() < o + 4 { return; }
                let cub = pvar::i32(&w.m(id).pvars, o);
                let c = fv(w.camera);
                if cub != -1 && !crate::moby_update::triggers::point_in_cuboid(&w.svc.volumes, [c[0], c[1], c[2]], cub) { return; }
            }
            register(w, id, port, DrawList::AfterTies);
        }
        _ => {}
    }
}

/// 854's callback `0x2ea048`: record k is drawn when the occlusion fallback 0x15f608 is set (`all_visible`: the
/// Visibomb's view) or its gate word (pvar 0 / 1 / 2) is not −1 and the camera (0x1673c0) is in that cuboid
/// (`0x261928`).
pub fn grid_set_drawn(gate: usize, pvars: &[u8], volumes: &rc_formats::volumes::Volumes, camera: [f32; 3], all_visible: bool) -> bool {
    if all_visible { return true; }
    let o = 4 * gate;
    if pvars.len() < o + 4 { return false; }
    let cub = pvar::i32(pvars, o);
    cub != -1 && crate::moby_update::triggers::point_in_cuboid(volumes, camera, cub)
}

/// dt `0x15ed7c` (NTSC).
const DT: f32 = 1.0 / 60.0;

/// The bob every sea uses: `sin((counter % 360)·0.017444445 − 3.14)` (the game's literals).
#[allow(clippy::approx_constant)]
pub fn bob(counter: u64) -> f32 { (((counter % 360) as f32) * 0.017_444_445 - 3.14).sin() }

/// A scroll lane after `+= d`: above 1 → −1, below −1 → +1 (each test once, in that order).
fn wrap(x: f32) -> f32 {
    let x = if x > 1.0 { x - 1.0 } else { x };
    if x < -1.0 { x + 1.0 } else { x }
}

fn register(w: &mut World, id: MobyId, port: usize, list: DrawList) {
    let cb = Callback::Sea(port as u8);
    match list {
        DrawList::AfterMobys => w.svc.draw_callbacks.register(cb, id),
        DrawList::AfterTies => w.svc.draw_callbacks.register_ties(cb, id),
    }
}

/// The module init `0x2ce098(scale, state, FIX)` as the port keeps it: the FIX and the record's z.
fn grid_init(w: &mut World, port: usize, g: &GridPort) {
    let z = match data(&w.svc.water, port) {
        Some(SeaData::Grid(d)) => d.grid.origin[2],
        _ => return,
    };
    let r = &mut w.svc.water.sea.run[port];
    if r.inited { return; }
    *r = SeaRun { inited: true, z, fix: g.fix, ..*r };
}

fn grid_update(w: &mut World, id: MobyId, port: usize, g: GridPort) {
    if g.variant == GridVariant::Batalia {
        // 0x2da0f0: `if (!gp flag) { flag = 1; init }`, then register every tick.
        grid_init(w, port, &g);
        register(w, id, port, g.list);
        batalia_splash(w, port);
        return;
    }
    match w.m(id).state {
        0 => {
            grid_init(w, port, &g);
            let m = w.mm(id);
            m.update_dist = 0xff;
            m.state = 1;
        }
        1 => {
            match g.variant {
                GridVariant::Rilgar => {
                    // 0x316110: 61.5 while Ratchet's state group (0x1413dc) is 0x16 (the Hoverboard), else 59.5.
                    let base = if w.hero.group == 0x16 { 61.5 } else { 59.5 };
                    w.svc.water.sea.run[port].z = bob(w.counter) * 0.25 + base;
                }
                GridVariant::Umbris => {
                    // 0x30d440: no registration while the camera (0x166e40) is in the cuboid of pvar 0.
                    let c = fv(w.camera);
                    let cub = pvar::i32(&w.m(id).pvars, 0);
                    if crate::moby_update::triggers::point_in_cuboid(&w.svc.volumes, [c[0], c[1], c[2]], cub) { return; }
                }
                _ => {}
            }
            register(w, id, port, g.list);
        }
        _ => {}
    }
}

/// Level 08's splash when Ratchet falls into its liquid (the second half of `0x2da0f0`): on level 8 (`0x15ed84`), with
/// his z below the record's z (+0x08) and his z less this tick's displacement (0x13f458) at or above it: the voice of
/// his death fall 0x77 (0x141602) released when alive (`SoundIsAlive`, `release_voice_slot`) and forgotten (0xffff);
/// at (his x, y, the liquid z): the splash 775 of size 3 with alpha 0x70 and 16 type-35 drops (`0x2f9718`,
/// `PartType35Spawn`: [`bomb_water::splash_and_drops`]); then 16 type-46 rings, each at `rand_vec(1, 1)` from that
/// point at z = liquid z + 0.05, size `randf(0.7, 1)`, spin 2 (the first) or −2, velocity 0 (0x15f580), on the liquid
/// z (`PartType46Spawn` 0x27bdb0), and, when the ring was made, its timer +0x0a = `trunc(scale(randf(30, 60)))`.
fn batalia_splash(w: &mut World, port: usize) {
    use crate::moby_update::classes::bomb_water;
    if w.svc.level != 8 { return; }
    let z = w.svc.water.sea.run[port].z;
    let (hz, dz) = (f32::from_bits(w.hero.pos[2].0), f32::from_bits(w.hero.disp[2].0));
    if !(hz < z && z <= hz - dz) { return; }
    if let Some(h) = w.hero_moby {
        let slot = w.hero.damage.voice_slot;
        if w.sound_alive(slot, h) {
            w.release_sound(slot, h);
            w.hero_fields_mut().fall_voice_clear = true;
        }
    }
    let hp = w.hero.pos.map(|x| f32::from_bits(x.0));
    let at = [hp[0], hp[1], z, hp[3]];
    bomb_water::splash_and_drops(w, 3.0, at);
    for i in 0..16 {
        let v = w.rng.rand_vec(1.0, 1.0);
        let p = [v[0] + at[0], v[1] + at[1], z + 0.05, at[3]];
        let size = w.rng.randf(f32::from_bits(0x3f33_3333), 1.0);
        let spin = if i == 0 { 2.0 } else { -2.0 };
        *w.svc.fx.part_spawns.entry(crate::particles::type46::TYPE).or_default() += 1;
        let Some(sys) = w.particles.as_deref_mut() else {
            // `PartType46Spawn`'s two draws and the timer's (a record assumed, as the other callers without a particle
            // system).
            w.rng.randi(10);
            w.rng.randf(0.0, 256.0);
            w.rng.randf(30.0, 60.0);
            continue;
        };
        match crate::particles::type46::spawn(sys, w.rng, size, spin, p, [0.0; 4], z) {
            Some(r) => {
                let t = w.rng.randf(30.0, 60.0);
                let t = w.svc.timing.scale(crate::ps2v::Pf::f(t)).to_f32() as i32;
                if let Some(sys) = w.particles.as_deref_mut() { crate::particles::rec::set_i16(&mut sys.pool.recs[r], 0xa, t as i16); }
            }
            None => w.svc.fx.part_failed += 1,
        }
    }
}

/// 879's callback `0x316070`: no draw while the camera (0x1671c0) is in one of the moby's 8 cuboids (pvar words 0..7,
/// −1 = none). The other grid callbacks always draw.
pub fn grid_draws(port: usize, pvars: &[u8], volumes: &rc_formats::volumes::Volumes, camera: [f32; 3]) -> bool {
    let Some(SeaKind::Grid(g)) = PORTS.get(port).map(|p| p.kind) else { return false };
    if g.variant != GridVariant::Rilgar { return true; }
    !(0..8).any(|k| pvars.len() >= 4 * k + 4 && crate::moby_update::triggers::point_in_cuboid(volumes, camera, pvar::i32(pvars, 4 * k)))
}

/// 1901's callback: group 1 is drawn while the camera (0x167240) is in the moby's cuboid (pvar 0).
pub fn hoven_group1_drawn(pvars: &[u8], volumes: &rc_formats::volumes::Volumes, camera: [f32; 3]) -> bool {
    pvars.len() >= 4 && crate::moby_update::triggers::point_in_cuboid(volumes, camera, pvar::i32(pvars, 0))
}

/// Class 1111's pvar fields (module doc).
pub mod ocean_pvar {
    pub const SEA_Z: usize = 0x14;
    pub const TILE: usize = 0x18;
    pub const FX: usize = 0x1c;
    pub const MIN_CAM_Z: usize = 0x20;
    pub const WALL_DROP: usize = 0x24;
    pub const REACH: usize = 0x28;
    pub const FADE: usize = 0x2c;
}

/// The ocean 1111's sea z (`0x161e18` on level 11) as its last update left it: the level's classes (the convoys'
/// sludge 1524) read it through a pointer; 0 without the ocean (the data's initial word).
pub fn ocean_z(water: &WaterWorld) -> f32 {
    PORTS.iter().position(|p| matches!(p.kind, SeaKind::Ocean)).and_then(|i| water.sea.run.get(i)).map_or(0.0, |r| r.sea_z)
}

fn ocean_update(w: &mut World, id: MobyId, port: usize) {
    let Some(SeaData::Ocean(t)) = data(&w.svc.water, port).cloned() else { return };
    let cam = fv(w.camera);
    let cam = [cam[0], cam[1], cam[2]];
    let pf = |w: &World, o: usize| pvar::ff(&w.m(id).pvars, o);
    if w.m(id).pvars.len() < 0x30 { return; }
    if w.m(id).state == 0 {
        let base_z = pf(w, ocean_pvar::SEA_Z);
        let m = w.mm(id);
        m.update_dist = 0xff;
        m.state = 1;
        let r = &mut w.svc.water.sea.run[port];
        r.scroll = [[0.0; 2]; 2];
        r.prev_cam = cam;
        r.base_z = base_z;
        return;
    }
    let (tile, min_z) = (pf(w, ocean_pvar::TILE), pf(w, ocean_pvar::MIN_CAM_Z));
    let counter = w.counter;
    let r = &mut w.svc.water.sea.run[port];
    let d = [cam[0] - r.prev_cam[0], cam[1] - r.prev_cam[1]];
    r.prev_cam = cam;
    // `c.lt.s pvar8, cam z` then `bc1f`: no draw unless strictly above (and not on a NaN).
    if min_z.partial_cmp(&cam[2]) != Some(std::cmp::Ordering::Less) { return; }
    for (layer, s) in r.scroll.iter_mut().enumerate() {
        // `vec_scale((scale + scale) / (S·5), delta)`, then each lane less its integer part unless it is exactly 1.
        let k = (t.scale[layer] + t.scale[layer]) / (tile * 5.0);
        for (lane, dv) in d.iter().enumerate() {
            let mut x = dv * k;
            if x != 1.0 { x -= x.trunc(); }
            s[lane] = wrap(s[lane] + x);
        }
    }
    r.sea_z = r.base_z + bob(counter) * 0.25;
    for (layer, s) in r.scroll.iter_mut().enumerate() {
        for (lane, x) in s.iter_mut().enumerate() { *x = wrap(*x + t.speed[layer][lane] * DT); }
    }
    register(w, id, port, DrawList::AfterMobys);
}

fn hoven_update(w: &mut World, id: MobyId, port: usize) {
    let Some(SeaData::Hoven(h)) = data(&w.svc.water, port) else { return };
    let (g1_speed, layer_speed) = (h.g1_speed, h.layer_speed);
    match w.m(id).state {
        0 => {
            let m = w.mm(id);
            m.state = 1;
            m.draw_dist = 0xff;
            m.update_dist = 0xff;
            let r = &mut w.svc.water.sea.run[port];
            r.inited = true;
            r.scroll = [[0.0; 2]; 2];
        }
        1 => {
            let r = &mut w.svc.water.sea.run[port];
            for (layer, s) in r.scroll.iter_mut().enumerate() {
                for (lane, x) in s.iter_mut().enumerate() { *x = wrap(*x + layer_speed[layer][lane] * DT); }
            }
            for (lane, x) in r.g1_scroll.iter_mut().enumerate() { *x = wrap(*x + g1_speed[lane] * DT); }
            register(w, id, port, DrawList::AfterTies);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_tests_each_bound_once() {
        assert_eq!(wrap(1.5), 0.5);
        assert_eq!(wrap(-1.5), -0.5);
        assert_eq!(wrap(0.25), 0.25);
        assert_eq!(wrap(1.0), 1.0);
    }

    #[test]
    fn bob_is_the_games_sine() {
        // counter 180: 180·0.017444445 − 3.14 = 0.00000... ≈ 0.0000001
        assert!(bob(180).abs() < 1e-3);
        assert!((bob(270) - 1.0).abs() < 1e-3);
        assert_eq!(bob(0), bob(360));
    }

    #[test]
    fn hoven_rescale_sets_rgb_and_halves_alpha() {
        let mut s = vec![StripMesh { pos: vec![[0.0; 3]], rgba: vec![0x7f12_3456], st: vec![[0.0; 2]] }];
        hoven_rescale(&mut s, [f32::from_bits(0x3dcc_cccd), 0.5, f32::from_bits(0x3ecc_cccd), 0.5]);
        assert_eq!(s[0].rgba[0], 0x3f_66_7f_19);
    }

    #[test]
    fn ports_are_unique() {
        for (i, a) in PORTS.iter().enumerate() {
            for b in &PORTS[i + 1..] { assert!(a.level != b.level || a.func != b.func); }
        }
    }
}
