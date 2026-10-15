//! Text → JSON: every language block of each level's gameplay file (help boxes, banners, subtitles) and of the
//! global `all_text` lump, parsed with `rc_formats::strings`. `text` is `strings::display` (printable ASCII as is,
//! every other byte as `\xNN`: colour codes, pad icons, the newline 0x01 and the game's accented letters), which
//! round-trips to the stored bytes; `bytes` is the stored bytes in hex.

use super::data::{self, lvl};
use super::jsonv::{hex_bytes, Obj, J};
use super::Out;
use crate::Error;
use rc_formats::strings::{self, Message, LANGUAGE_SLOTS};
use std::path::Path;

/// Language of each slot (`0x15ed88`); slot 1 is unused on the disc, 6 and 7 unnamed.
fn language(slot: u32) -> &'static str {
    match slot { 0 => "en", 1 => "slot1", 2 => "fr", 3 => "de", 4 => "es", 5 => "it", 6 => "slot6", _ => "slot7" }
}

fn messages_json(source: &str, scope: &str, slot: u32, msgs: &[Message]) -> J {
    Obj::new()
        .set("source", source)
        .set("scope", scope)
        .set("language", language(slot))
        .set("slot", slot)
        .set("encoding", "text: printable ASCII as is, other bytes as \\xNN (strings::display); bytes: stored, hex")
        .set("messages", J::Arr(msgs.iter().map(|m| {
            Obj::new().set("id", m.id).set("help_audio", m.help_audio).set("text", strings::display(&m.text)).set("bytes", hex_bytes(&m.text)).build()
        }).collect()))
        .build()
}

pub(crate) fn export_level(data: &Path, out: &Out, id: u32) -> Result<(), Error> {
    let g = data::gameplay(data, id)?;
    let src = lvl(id, "gameplay_ntsc.bin");
    for slot in 0..LANGUAGE_SLOTS {
        match strings::parse_strings(&g, slot) {
            Ok(m) if m.is_empty() => {}
            Ok(m) => out.json(&format!("text/levels/{id:02}/{}.json", language(slot)), &messages_json(&src, &format!("level {id:02}"), slot, &m))?,
            Err(e) => out.skip(format!("{src} text slot {slot}: {e}")),
        }
    }
    Ok(())
}

/// `all_text`: eight u32 block offsets (the same language slots as the gameplay header), each a text block.
pub(crate) fn export_global(data: &Path, out: &Out) -> Result<(), Error> {
    let b = data::read(data, "global/all_text.bin")?;
    for slot in 0..LANGUAGE_SLOTS as usize {
        let Some(off) = b.get(4 * slot..4 * slot + 4).map(|w| u32::from_le_bytes(w.try_into().unwrap()) as usize) else { break };
        let Some(block) = b.get(off..) else { out.skip(format!("global/all_text.bin slot {slot}: offset {off:#x} past the end")); continue };
        match strings::parse_text_block(block) {
            Ok(m) if m.is_empty() => {}
            Ok(m) => out.json(&format!("text/global/all_text/{}.json", language(slot as u32)), &messages_json("global/all_text.bin", "global", slot as u32, &m))?,
            Err(e) => out.skip(format!("global/all_text.bin slot {slot}: {e}")),
        }
    }
    Ok(())
}
