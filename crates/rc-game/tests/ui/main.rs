//! Integration tests: menus, HUD, map, help, the vendor, game state, and the audio conformance tests (docs/workflows/testing.md §3).
//! One module per former test file; run one with `cargo xtask test-job --test ui --filter <module>::`.

mod gadgets_novalis;
mod game_state_novalis;
mod gemlik_generalisation;
mod help_novalis;
mod interaction_vendor;
mod map_levels;
mod pause_pages_novalis;
mod reverb_conformance;
mod sound_conformance;
