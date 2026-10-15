//! Integration tests: rc-formats' loaders against the disc data: the committed loader snapshots and the per-format disc checks (docs/workflows/testing.md §3).
//! One module per former test file; run one with `cargo xtask test-job --test formats --filter <module>::`.

mod golden;
mod level_overlay_disc;
mod moby_anim_golden;
mod moby_collision_disc;
mod moby_shadow_disc;
mod occlusion_frames;
mod scene_coverage;
mod tfrag_light_golden;

/// The committed loader-snapshot table the golden tests check against (used by `golden`).
mod snapshot;
