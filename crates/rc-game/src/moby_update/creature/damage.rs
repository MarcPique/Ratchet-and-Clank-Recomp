//! The hit resolver `0x26f378(moby, hit, D, flags, &out5, &damage, &class, col)`: what a hit message does to a
//! creature before its class code reads the damage (level01; tables read from the level01 overlay).
//!
//! The damage record `D` (the creature header's +0x00, e.g. critter pvar+0x20): +0x00 f32 health, +0x06 s16 (the
//! classes' own hit cooldown), +0x08 byte column of the damage tables, +0x14 / +0x16 s16 the attacker class of the
//! last hit, +0x1c s16 the per-kind cooldown timer.
//!
//! * Attackers of class 0x131 / 0xa8 deal `gp−0x6b80[col]` = (1.0, 0.667, 0.5, 0.5).
//! * No hit: the cooldown ticks down, +0x16 = −1, out5 = 1, result 0xb.
//! * An exact push vector (w = 5627.925, the wrench and the Ratchet-made hits) is scaled by `0x1b0648[col·8 + type]`
//!   (by column: 1, .95, 1.2, 1.4, 1.4, 1.15, 1, 2 / 1, .85, 1.1, 1.2, 1.2, 1, 1, 1.5 / …). The attacker class is the
//!   record's +0x2a; for the wrench (0x47) the push is redirected: `0.3·push + 0.7·|push|·(cos a, sin a, 1)` with `a`
//!   the heading from Ratchet (0x13f3d0) to the creature, then scaled to unit xy length. (The w lane comes out as a
//!   meaningless value, so the class code no longer sees the exact marker: reproduced as w = 0.) Otherwise the class is
//!   the attacker moby's (0x47 without one) and the type byte (+0x28) is forced to 8.
//! * The attack kind from the type (+0x28), subtype (+0x29) and class (+0x2a): type 0 → kind 0 (class 1 / 2) or 1;
//!   1 → 0xb / 5 / 9; 2 → 8 / 0xd; 3 → 0xa / 0xf / 0x10 (the last also clears +0x14 / +0x1c); 4 → 0xe; 5 → 4 / 7;
//!   7 → 0xc / 0x11; else 1. `out5` is 2 (3 / 4 for type 5).
//! * The per-kind cooldown: a hit from a different attacker class than +0x14 restarts it; while it runs, kinds
//!   0, 1, 4, 7 and 2, 3, 5, 6, 8–10 lose their damage (the first group sets out5 = 1 unless the caller's flags let
//!   that kind through), kind 11 keeps it, kinds ≥ 12 lose it; when it is not running it is restarted with
//!   `ticks(37, 15, 30, 30, 60, 30, 30, 45|60, 30, 30, 30, 60)[kind]` (30 for kinds ≥ 12; kind 7: 45 when gold
//!   weapon 0x13 is owned, `0x13e533`).
//! * Result: `0x1b0600[col + kind·4]`; the knockback record's burn marker (+0x2e) = (kind == 4).

use crate::moby_runtime::MobyId;
use crate::moby_update::services::{HitRecord, World};
use crate::ps2v::Pf;

/// `gp−0x6b80` (0x160080): damage by column for attacker classes 0x131 / 0xa8.
pub const CLASS_DAMAGE: [f32; 4] = [1.0, 0.666_666_7, 0.5, 0.5];
/// `0x1b0648`: the push scale by column (8 attack types each).
pub const PUSH_SCALE: [[f32; 8]; 4] = [
    [1.0, 0.95, 1.2, 1.4, 1.4, 1.15, 1.0, 2.0],
    [1.0, 0.85, 1.1, 1.2, 1.2, 1.0, 1.0, 1.5],
    [1.0, 0.75, 1.0, 1.0, 1.1, 0.85, 1.0, 1.25],
    [1.0, 0.65, 0.9, 1.0, 1.0, 0.75, 1.0, 1.0],
];
/// `0x1b0600`: the reaction byte by `col + kind·4` (18 kinds).
pub const REACTION: [u8; 72] = [
    1, 3, 7, 9, 1, 3, 7, 9, 1, 3, 7, 9, 1, 3, 7, 9, 1, 4, 8, 9, 1, 5, 3, 9, 1, 1, 5, 9, 1, 1, 4, 9, 1, 1, 5, 8, 1, 1, 5, 8,
    1, 1, 1, 5, 2, 6, 6, 10, 1, 1, 1, 5, 1, 1, 1, 5, 1, 1, 1, 5, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
];
/// The cooldown restarts (`ticks(n)`) by kind for kinds < 12 (kind 7 is 60, or 45 with gold weapon 0x13).
pub const COOLDOWN: [i32; 12] = [37, 15, 30, 30, 60, 30, 30, 60, 30, 30, 30, 60];

/// The resolver's outputs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resolved {
    /// The return value (the reaction byte, 0xb without a hit, 0xc / 0xd for suppressed kinds).
    pub reaction: u8,
    /// `*param_5`.
    pub out5: i32,
    /// `*param_6`: the damage after the resolver (0 when suppressed or without a hit).
    pub damage: f32,
    /// `*param_7`: the attacker class (−1 without a hit).
    pub class: i32,
    /// The attack kind (0..17), when there was a hit.
    pub kind: Option<u32>,
    /// The (possibly rewritten) record, written back to the hit log slot.
    pub hit: Option<HitRecord>,
}

fn s16(w: &World, id: MobyId, o: usize) -> i16 { super::pi16(w, id, o) }

/// `0x26f378` for moby `id`, damage record at pvar `d`, the hit `hit` (the record `MobyGetHitMessage` returned),
/// caller flags `flags4`, column `col` (4 = the record's own +0x08).
pub fn resolve(w: &mut World, id: MobyId, hit: Option<HitRecord>, d: usize, flags4: u32, col: i32) -> Resolved {
    let mut r = Resolved { reaction: 0, out5: 0, damage: 0.0, class: -1, kind: None, hit };
    let mut rec = match hit {
        Some(h) => h,
        None => {
            super::dec_timer_pvar_s16(w, id, d + 0x1c);
            super::set_pi16(w, id, d + 0x16, -1);
            r.out5 = 1;
            r.reaction = 0xb;
            return r;
        }
    };
    let dcol = super::pu8(w, id, d + 8) as usize;
    if let Some(a) = rec.attacker {
        let c = w.m(a).o_class;
        if c == 0x131 || c == 0xa8 { rec.damage = Pf::f(CLASS_DAMAGE[dcol.min(3)]); }
    }
    let col = if col == 4 { dcol } else { col as usize } & 3;
    let exact = f32::from_bits(rec.dir[3].0) == crate::hero::damage::EXACT_PUSH_W;
    let cls: i32;
    if exact {
        let s = PUSH_SCALE[col][(rec.b28 as usize).min(7)];
        for c in &mut rec.dir[..3] { *c = Pf::f(f32::from_bits(c.0) * s); }
        cls = rec.h2a as i32;
        if cls == 0x47 {
            let me = super::pos(w, id);
            let h = w.hero.pos.map(|x| f32::from_bits(x.0));
            let a = super::atan(me[0] - h[0], me[1] - h[1]);
            let (c, s) = super::cs(a);
            let dir: super::V = rec.dir.map(|x| f32::from_bits(x.0));
            let l = super::len3([dir[0], dir[1], dir[2], 0.0]);
            let u = super::set_len3([c, s, 1.0, 0.0], l * 0.7);
            let v = super::add(super::scale([dir[0], dir[1], dir[2], 0.0], 0.3), u);
            let v = super::set_len2(v, 1.0);
            rec.dir = [Pf::f(v[0]), Pf::f(v[1]), Pf::f(v[2]), Pf::ZERO];
        }
    } else {
        cls = rec.attacker.map_or(0x47, |a| w.m(a).o_class as i32);
        rec.b28 = 8;
    }
    let b29 = rec.b29;
    let mut out5 = 2;
    let kind: u32 = match rec.b28 {
        0 => if (rec.h2a.wrapping_sub(1)) < 2 { 0 } else { 1 },
        1 => if b29 < 2 { 0xb } else if b29 == 2 { 5 } else { 9 },
        2 => if b29 < 2 { 8 } else { 0xd },
        3 => {
            if b29 < 2 { 0xa } else if b29 < 3 { 0xf } else {
                super::set_pi16(w, id, d + 0x14, -1);
                super::set_pi16(w, id, d + 0x1c, 0);
                0x10
            }
        }
        4 => 0xe,
        5 => {
            if b29 < 2 { out5 = 3; 4 } else { out5 = 4; 7 }
        }
        7 => if b29 < 2 { 0xc } else { 0x11 },
        _ => 1,
    };
    r.class = cls;
    let mut reaction = REACTION.get(col + kind as usize * 4).copied().unwrap_or(1);
    if s16(w, id, d + 0x14) as i32 != cls { super::set_pi16(w, id, d + 0x1c, 0); }
    let t = super::dec_timer_pvar_s16(w, id, d + 0x1c);
    if t != 0 {
        let n = if kind < 12 {
            if kind == 7 {
                let gold = w.svc.counters.gold_weapons.get(0x13).copied().unwrap_or(0);
                if gold != 0 { 45 } else { 60 }
            } else {
                COOLDOWN[kind as usize]
            }
        } else {
            30
        };
        let v = super::ticks(w, n);
        super::set_pi16(w, id, d + 0x1c, v as i16);
    } else if kind >= 12 {
        rec.damage = Pf::ZERO;
        reaction = 0xd;
    } else {
        let gate = match kind {
            0 | 1 => Some((0x10, 0x20)),
            4 => Some((1, 2)),
            7 => Some((4, 8)),
            11 => None,
            _ => {
                rec.damage = Pf::ZERO;
                reaction = 0xd;
                None
            }
        };
        if kind == 11 {
            reaction = 0xc;
        } else if let Some((keep, silent)) = gate {
            if flags4 & keep == 0 { reaction = 0xb; }
            if flags4 & silent == 0 { out5 = 1; }
            rec.damage = Pf::ZERO;
        }
    }
    super::set_pi16(w, id, d + 0x16, cls as i16);
    super::set_pi16(w, id, d + 0x14, cls as i16);
    r.damage = f32::from_bits(rec.damage.0);
    if let Some(kr) = super::header(w, id).knock {
        super::set_pu8(w, id, kr + super::knock::k::BURN, (kind == 4) as u8);
    }
    r.out5 = out5;
    r.reaction = reaction;
    r.kind = Some(kind);
    // The game rewrites the record in the hit log in place.
    let slot = w.m(id).hit_slot;
    if slot != 0xff {
        if let Some(e) = w.svc.hits.records.get_mut(slot as usize) {
            if e.target == id { *e = rec; }
        }
    }
    r.hit = Some(rec);
    r
}
