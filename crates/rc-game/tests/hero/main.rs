//! Integration tests: Ratchet's movement core on real levels: walk, jump, ledges, packs, boots, the swingshot, surfaces, damage and the NO_IDLE hero digest (docs/workflows/testing.md §3).
//! One module per former test file; run one with `cargo xtask test-job --test hero --filter <module>::`.

mod hero_boots_grind;
mod hero_boots_magnet;
mod hero_cable_kerwan;
mod hero_damage;
mod hero_followups;
mod hero_ledge_novalis;
mod hero_novalis;
mod hero_pack_swap_novalis;
mod hero_packs_novalis;
mod hero_platform_novalis;
mod hero_pose;
mod hero_scripted_novalis;
mod hero_surfaces;
mod hero_swingshot_levels;
