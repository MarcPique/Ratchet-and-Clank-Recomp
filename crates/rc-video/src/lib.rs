//! The game's movies (FMV, `global/mpegs/NNN.bin`) played natively from the original files: an MPEG-2 video
//! decoder ([`mpeg2`], scoped to what the 84 files use), the IEEE 1180 inverse DCT ([`idct`]), and [`movie`], which
//! ties the PSS demuxer and the SPU-ADPCM audio of `rc_formats::pss` to the decoder and converts frames to RGBA.
//! Pure Rust, no external dependencies, no Bevy. Spec: docs/formats/pss.md; playback: docs/plan/cutscenes.md §5.

pub mod bits;
pub mod idct;
pub mod movie;
pub mod mpeg2;
pub mod vlc;

pub use movie::Movie;
pub use mpeg2::{DecodeError, Decoder, Frame, PictureType, Sequence};
