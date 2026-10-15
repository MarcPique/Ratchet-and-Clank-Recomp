//! **The visit-state records**: the words a class wants back after a death reload (level01 `0x29b0a0` record,
//! `0x29afc8` apply, `0x29b080`; level03's copy `0x273f50`, the same code with its table at 0x1ba720). The table
//! (`0x1baaa0` on 01: a count and up to 64 records of 16 bytes) lives for the visit; the checkpoint record
//! (`0x29ac10`) copies it whole to `0x1bb700`, and the respawn after a death reload (`0x29adc8`, last) writes every
//! record of that copy back into the reloaded mobys. A record names its owner by uid (+0xb2) and class (+0xa6), the
//! place written (an address relative to the owner moby, mode 1, or to its pvars, other modes), its size (≤ 4) and
//! the bytes. Recording the same place again (same offset, size, owner, a non-zero mode) updates the bytes.
//!
//! The port keeps the place by meaning instead of by address ([`Place`]): the game's offsets are differences of RAM
//! addresses that reach other mobys (the talking NPC's record of the Infobot's state byte, Kerwan's NPC's record of
//! the course moby's pvar +0x8c) and only map back because the reload rebuilds the same layout; here the target moby
//! is named by its own uid and class. The callers: the bolt cranks 280 (their progress), the Novalis bridge halves of
//! the mission NPC 730 (+0xbc), the Water Pump Worker 774 (the Infobot's state), Kerwan's Helga 890 (the course's
//! "open" word).
//!
//! ## Coverage
//! | address | what | port |
//! |---|---|---|
//! | `0x29b0a0(addr, size, moby, mode, table)` | a record with the same offset, size, uid, class and a non-zero mode → its bytes updated; else appended (count + 1; no bound check: 64 fit) | [`record`] (the port drops a 65th [L]) |
//! | `0x29ac10` | `0x1baaa0` copied to `0x1bb700` (0x404 bytes) | `checkpoint::record` (`SaveBits::checkpoint_visit`) |
//! | `0x29adc8` tail → `0x29b080` → `0x29afc8(0x1bb700)` | each record: the owner found (`0x29af80(class, uid)`), the bytes written at its place; no owner: skipped | [`restore`] (after the reload's load pass [L]) |

use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::services::World;

/// The table's capacity (0x404 bytes: a count and 64 records).
pub const MAX: usize = 64;

/// Where a record's bytes go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// A byte field of a moby (mode 1): the state +0x20 or the command byte +0xbc.
    Field { uid: i16, class: i16, offset: u16 },
    /// A moby's pvars at `offset` (the game's other modes).
    Pvar { uid: i16, class: i16, offset: u16 },
}

/// One record (owner uid +0xc, class +0xe; the place; size +8; mode +0xa; the bytes +4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Visit {
    pub owner_uid: i16,
    pub owner_class: i16,
    pub place: Place,
    pub size: u8,
    pub mode: i16,
    pub bytes: [u8; 4],
}

/// The bytes at `place` of the table (None when the target is missing or the field is not one the port maps).
fn read(t: &MobyTable, place: Place, size: usize) -> Option<[u8; 4]> {
    let mut out = [0u8; 4];
    match place {
        Place::Field { uid, class, offset } => {
            let m = find(t, class, uid).map(|i| &t.mobys[i])?;
            out[0] = match offset { 0x20 => m.state, 0xbc => m.cmd, _ => return None };
        }
        Place::Pvar { uid, class, offset } => {
            let m = find(t, class, uid).map(|i| &t.mobys[i])?;
            let o = offset as usize;
            out[..size].copy_from_slice(m.pvars.get(o..o + size)?);
        }
    }
    Some(out)
}

/// `0x29af80(class, uid)`: the moby of class `class` with uid `uid`.
pub fn find(t: &MobyTable, class: i16, uid: i16) -> Option<MobyId> {
    t.mobys.iter().position(|m| m.o_class == class && m.spawn_id == uid && m.state != crate::moby_runtime::state::END)
}

/// `0x29b0a0(place, size, owner, mode, 0x1baaa0)` (module table): `owner` names the record; the bytes are read from
/// `place` now.
pub fn record(w: &mut World, owner: MobyId, place: Place, size: usize, mode: i16) {
    let size = size.min(4);
    let (uid, class) = { let m = w.m(owner); (m.spawn_id, m.o_class) };
    let Some(bytes) = read(w.table, place, size) else {
        w.svc.unported("visit record: a place the port does not map");
        return;
    };
    let list = &mut w.svc.save.visit;
    if let Some(v) = list.iter_mut().find(|v| v.place == place && v.size as usize == size && v.owner_uid == uid && v.owner_class == class && v.mode != 0) {
        v.bytes = bytes;
        return;
    }
    if list.len() < MAX { list.push(Visit { owner_uid: uid, owner_class: class, place, size: size as u8, mode, bytes }); }
}

/// The record of moby `id`'s own byte field `offset` (mode 1).
pub fn record_field(w: &mut World, owner: MobyId, target: MobyId, offset: u16) {
    let (uid, class) = { let m = w.m(target); (m.spawn_id, m.o_class) };
    record(w, owner, Place::Field { uid, class, offset }, 1, 1);
}

/// The record of `target`'s pvars at `offset` (`size` bytes; the game's mode 2).
pub fn record_pvar(w: &mut World, owner: MobyId, target: MobyId, offset: u16, size: usize) {
    let (uid, class) = { let m = w.m(target); (m.spawn_id, m.o_class) };
    record(w, owner, Place::Pvar { uid, class, offset }, size, 2);
}

/// `0x29afc8(0x1bb700)`: every record of the checkpoint's copy written back into `t` (module table).
pub fn restore(t: &mut MobyTable, list: &[Visit]) {
    for v in list {
        if find(t, v.owner_class, v.owner_uid).is_none() { continue; }
        match v.place {
            Place::Field { uid, class, offset } => {
                let Some(i) = find(t, class, uid) else { continue };
                let m = &mut t.mobys[i];
                match offset { 0x20 => m.state = v.bytes[0], 0xbc => m.cmd = v.bytes[0], _ => {} }
            }
            Place::Pvar { uid, class, offset } => {
                let Some(i) = find(t, class, uid) else { continue };
                let m = &mut t.mobys[i];
                let (o, n) = (offset as usize, v.size as usize);
                if m.pvars.len() < o + n { m.pvars.resize(o + n, 0); }
                m.pvars[o..o + n].copy_from_slice(&v.bytes[..n]);
            }
        }
    }
}
