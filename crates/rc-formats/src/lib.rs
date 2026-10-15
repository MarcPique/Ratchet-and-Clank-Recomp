//! Readers for the on-disc formats of Ratchet & Clank (2002, PS2).
//!
//! Every reader here follows the specification in `docs/formats/`. The readers were first verified byte for byte
//! against a C++ reference extractor, retired on 2026-09-27 (git history at 2230812); the golden tests now
//! check their output against the committed snapshot table `data/loader_snapshots.tsv` (`tests/formats/golden.rs`).

pub mod buf;
pub mod wad;
pub mod toc;
pub mod iso9660;
pub mod disc;
pub mod level;
pub mod texture;
pub mod vif;
pub mod tfrag;
pub mod moby;
pub mod gadget;
pub mod tie;
pub mod shrub;
pub mod shrub_light;
pub mod tfrag_light;
pub mod tie_light;
pub mod gameplay;
pub mod moby_light;
pub mod moby_anim;
pub mod moby_collision;
pub mod moby_shadow;
pub mod moby_spawn;
pub mod sky;
pub mod collision;
pub mod occlusion;
pub mod particle_tex;
pub mod hud;
pub mod pif;
pub mod font;
pub mod level_overlay;
pub mod strings;
pub mod sound_bank;
pub mod vag;
pub mod water;
pub mod sea;
pub mod save_game;
pub mod frontend;
pub mod scene;
pub mod transition;
pub mod volumes;
pub mod cameras;
pub mod pss;
pub mod sha1;
pub mod test_data;

pub use buf::{Buf, FormatError};
