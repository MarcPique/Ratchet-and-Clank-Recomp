//! Integration tests: the levels as a whole: the all-levels smoke, the per-level class tables, collision, water, the sea, shadows and cutscenes (docs/workflows/testing.md §3).
//! One module per former test file; run one with `cargo xtask test-job --test world --filter <module>::`.

#[path = "../common/mod.rs"]
mod common;

pub mod all_levels_smoke; // pub: its `Outcome` fields are public API, as when the file was its own crate root
mod camera_levels;
mod cutscene_novalis;
mod level_ports;
mod moby_collision_novalis;
mod nebula_petal_doors;
mod novalis_collision;
mod novalis_world;
mod sea_levels;
mod shadow_volume_novalis;
mod water_levels;
