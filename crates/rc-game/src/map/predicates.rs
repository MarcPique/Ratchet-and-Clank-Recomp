//! The per-level reveal predicates (table 0x184410: 19 levels × 8 function pointers; zone flag `0x100 << k` requires
//! predicate k of the current level). Each is a small hand-written test of the hero and the map pixel, ported one
//! by one from its level01 code (the same code in every overlay). Arguments as the fog writer passes them: the
//! hero position (x, y, z) and the brush centre pixel (px, py) = the hero's map pixel + 1.
//!
//! Recurring pieces: `compute_cross_product_sign` 0x25ec10 ([`side`]), "riding" (the movement group 0x1413dc is
//! 0x11 / 0x12 or 0x140634 == 1), the global flags 0x13d388, the map zone flags 0x184928 (i32 each), and a vector
//! distance 0x221398 on (x, y, 0).

/// What the predicates read.
#[derive(Clone, Copy, Debug)]
pub struct Env<'a> {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub px: i32,
    pub py: i32,
    /// 0x13d388: the global flags.
    pub flags: &'a [u8],
    /// 0x184928: the map zone flags.
    pub zone_flags: &'a [i32],
    /// 0x1413dc: the hero's movement group; 0x140634.
    pub group: i32,
    pub f0634: u8,
    /// gp−0x6e04 (0x15fdfc; level 3's predicate: 0 → true).
    pub g15fdfc: i32,
}

impl Env<'_> {
    fn flag(&self, n: usize) -> bool { self.flags.get(n).is_some_and(|&b| b != 0) }
    fn zf(&self, n: usize) -> bool { self.zone_flags.get(n).is_some_and(|&v| v != 0) }
    fn riding(&self) -> bool { (self.group - 0x11) as u32 <= 1 || self.f0634 == 1 }
    fn in_z(&self, lo: f32, hi: f32) -> bool { lo <= self.z && self.z <= hi }
    /// `compute_cross_product_sign(px, py, ax, ay, bx, by)`: the pixel right of A → B (y down).
    fn side(&self, ax: i32, ay: i32, bx: i32, by: i32) -> bool { side(self.px, self.py, ax, ay, bx, by) }
}

/// `compute_cross_product_sign` (0x25ec10): `(bx − ax)(py − ay) − (by − ay)(px − ax) < 0`.
pub fn side(px: i32, py: i32, ax: i32, ay: i32, bx: i32, by: i32) -> bool {
    ((bx - ax).wrapping_mul(py - ay)).wrapping_sub((by - ay).wrapping_mul(px - ax)) < 0
}

/// 0x221398 on two (x, y, 0) points.
fn dist(a: (f32, f32), b: (f32, f32)) -> f32 { ((b.0 - a.0) * (b.0 - a.0) + (b.1 - a.1) * (b.1 - a.1)).sqrt() }

/// The level01 address of predicate k of `level` (0x184410 + 0x20·level + 4k), None when the slot is empty.
pub fn address(level: usize, k: usize) -> Option<u32> {
    let row: &[u32] = match level {
        1 => &[0x25cc08, 0x25cc30, 0x25cc40, 0x25cc60, 0x25cd20, 0x25cda0],
        2 => &[0x25cc50],
        3 => &[0x25ce08],
        4 => &[0x25ce18, 0x25ce28, 0x25ce38, 0x25ce48],
        6 => &[0x25ce58],
        7 => &[0x25ce68, 0x25d0a8, 0x25d1e0, 0x25d390, 0x25d450, 0x25d4e0, 0x25d590, 0x25d648],
        8 => &[0x25d710, 0x25d740, 0x25d798, 0x25d810, 0x25d888, 0x25d8c0, 0x25d8d0, 0x25d8e0],
        9 => &[0x25d8f0, 0x25d900, 0x25d910],
        10 => &[0x25d920, 0x25d960, 0x25d970, 0x25d980, 0x25d990, 0x25d9a0],
        11 => &[0x25d9b0, 0x25da30, 0x25da90, 0x25db10, 0x25dbc0, 0x25dc58],
        12 => &[0x25dc68, 0x25dc98, 0x25dca0],
        15 => &[0x25dca8],
        16 => &[0x25dd48, 0x25dd58, 0x25dd68, 0x25dd78],
        18 => &[0x25dd88],
        _ => &[],
    };
    row.get(k).copied()
}

/// Predicate `k` of `level` (None: no function in the slot, which the writer treats as passing).
pub fn eval(level: usize, k: usize, e: &Env) -> Option<bool> { Some(call(address(level, k)?, e)) }

/// The predicate at level01 address `a`.
pub fn call(a: u32, e: &Env) -> bool {
    match a {
        0x25cc08 => e.side(211, 219, 297, 249),
        0x25cc30 => e.flag(12),
        0x25cc40 => e.flag(13),
        0x25cc50 => e.flag(21),
        0x25cc60 => {
            if e.py < 200 { e.in_z(39.5, 42.5) && !e.riding() } else { 87.0 <= e.z && e.group != 0x10 }
        }
        0x25cd20 => {
            if e.px < 256 { e.flag(14) } else { e.in_z(58.0, 86.0) && e.group != 0x10 }
        }
        0x25cda0 => {
            if e.px < 256 { e.riding() } else { 95.0 <= e.z }
        }
        0x25ce08 => e.g15fdfc == 0,
        0x25ce18 => e.flag(28),
        0x25ce28 => e.flag(29),
        0x25ce38 => e.flag(30),
        0x25ce48 => e.flag(31),
        0x25ce58 => e.flag(37),
        0x25ce68 => {
            if e.side(393, 367, 135, 237) {
                e.side(117, 326, 395, 460) && e.side(321, 304, 176, 304) && f32::from_bits(0x422f_999a) <= e.z
            } else if e.side(401, 205, 210, 315) {
                dist((e.x, e.y), (337.5, 250.0)) <= 7.0 || (e.side(322, 298, 371, 252) && e.zf(2))
            } else {
                e.riding() && e.side(299, 184, 314, 242) && e.side(304, 237, 357, 203) && e.side(353, 220, 315, 176) && e.side(342, 168, 281, 209)
            }
        }
        0x25d0a8 => {
            (e.side(147, 360, 386, 360) && e.py >= 309 && e.zf(0) && f32::from_bits(0x423e_cccd) <= e.z)
                || (e.riding() && e.side(197, 154, 316, 225) && e.side(214, 195, 343, 197) && e.side(263, 218, 369, 160))
        }
        0x25d1e0 => {
            if e.py < 233 {
                e.side(306, 160, 351, 216) && e.side(333, 216, 385, 154) && e.side(386, 180, 311, 147) && e.side(343, 140, 304, 170)
            } else {
                (e.side(143, 277, 328, 331) || e.side(231, 264, 295, 356))
                    && (e.side(237, 351, 340, 270) || e.side(162, 299, 367, 327))
                    && (e.side(306, 355, 321, 204) || e.side(194, 264, 416, 302))
            }
        }
        0x25d390 => {
            if e.py < 187 { !e.riding() && e.zf(5) } else { e.in_z(51.5, 54.0) && e.side(266, 229, 292, 249) }
        }
        0x25d450 => e.zf(3) && (e.side(153, 237, 352, 279) || e.side(270, 247, 317, 281)),
        0x25d4e0 => {
            if e.py < 157 { e.in_z(73.5, 80.0) && e.zf(6) } else { e.in_z(51.5, 54.0) && e.zf(4) }
        }
        0x25d590 => {
            if e.py >= 261 {
                e.zf(1) && f32::from_bits(0x423e_cccd) <= e.z
            } else if e.py < 193 {
                e.riding()
            } else {
                e.riding() && !e.side(217, 184, 342, 210)
            }
        }
        0x25d648 => !e.riding() && 71.5 <= e.z && !e.side(305, 226, 198, 147) && !e.side(400, 137, 209, 251),
        0x25d710 => {
            if e.py < 257 { e.flag(53) } else { e.group == 0xf }
        }
        0x25d740 => {
            if e.px < 224 { 47.75 <= e.z } else { e.z < 29.0 }
        }
        0x25d798 => {
            if e.px < 224 { e.in_z(44.0, 45.0) } else { 37.0 <= e.z }
        }
        0x25d810 => {
            if e.px < 224 { e.in_z(42.0, 43.0) } else { 39.0 <= e.z }
        }
        0x25d888 => e.px < 224 && e.z <= 38.0,
        0x25d8c0 => e.flag(48),
        0x25d8d0 => e.flag(49),
        0x25d8e0 => e.flag(50),
        0x25d8f0 => e.flag(67),
        0x25d900 => e.flag(68),
        0x25d910 => e.flag(69),
        0x25d920 => {
            if e.px < 190 { 58.5 <= e.z } else { e.flag(80) }
        }
        0x25d960 => e.flag(76),
        0x25d970 => e.flag(77),
        0x25d980 => e.flag(78),
        0x25d990 => e.flag(79),
        0x25d9a0 => e.flag(81),
        0x25d9b0 => {
            if e.px < 221 { e.in_z(232.0, 235.0) && e.flag(86) } else { e.z <= 180.0 }
        }
        0x25da30 => {
            if e.px < 221 { 242.0 <= e.z } else { e.z <= 180.0 && e.flag(87) }
        }
        0x25da90 => {
            if e.px < 221 { e.in_z(238.0, 241.0) } else { e.z <= 180.0 && e.flag(88) }
        }
        0x25db10 => {
            if e.px < 351 { 200.0 <= e.z && e.riding() } else { e.in_z(233.0, 235.0) && e.flag(90) }
        }
        0x25dbc0 => {
            if e.px < 351 { e.in_z(228.0, 230.0) } else { e.in_z(233.0, 235.0) && e.flag(91) }
        }
        0x25dc58 => e.flag(89),
        0x25dc68 => e.py >= 321 || 63.5 <= e.z,
        0x25dc98 | 0x25dca0 => true,
        0x25dca8 => e.zf(0) || dist((e.px as f32, e.py as f32), (161.5, 249.0)) <= 35.0,
        0x25dd48 => e.flag(114),
        0x25dd58 => e.flag(115),
        0x25dd68 => e.flag(116),
        0x25dd78 => e.flag(117),
        0x25dd88 => e.flag(127),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every predicate against its game code: 20,000 inputs from a fixed LCG (the pixel −4..516, z −20..430 in
    /// quarters, x / y 0..600, the movement group among 0 / 0xf / 0x10 / 0x11 / 0x12 / 5, 0x140634, the global flags,
    /// the zone flags, gp−0x6e04), the answers hashed (FNV-1a over 0 / 1) and counted. The expected values come from
    /// running the level01 functions themselves on the same inputs (a throwaway R5900 interpreter, not part of the
    /// port).
    #[test]
    fn predicates_match_the_game_code() {
        const GOLDEN: &[(u32, u64, u32)] = &[
        (0x25cc08, 0x06f30afe36d3cd35, 9160),
        (0x25cc30, 0xdbd4b170c1ebc359, 9894),
        (0x25cc40, 0x30927f80a6b5daa0, 10021),
        (0x25cc50, 0xcfa989b73c0f6dff, 10138),
        (0x25cc60, 0xd78123e8925d5c8c, 7685),
        (0x25cd20, 0xc87243eaa7acc762, 5499),
        (0x25cda0, 0x3ab97d77b732ea04, 12975),
        (0x25ce08, 0x59f2785034a0fef2, 10125),
        (0x25ce18, 0x4495172c93bea071, 9966),
        (0x25ce28, 0x16cbc6cebdbb215a, 10037),
        (0x25ce38, 0xe3cf456f3f6786ea, 9855),
        (0x25ce48, 0xf967b59014fc8da3, 10046),
        (0x25ce58, 0xdd1cb3c591a2c9a3, 10042),
        (0x25ce68, 0x47515b5220a2d931, 2370),
        (0x25d0a8, 0x03d20894a03b2678, 3657),
        (0x25d1e0, 0xcd0d2220990873f7, 2210),
        (0x25d390, 0x600a50c20703ab46, 1699),
        (0x25d450, 0x227679756f3bff58, 5491),
        (0x25d4e0, 0x522c622e1fd3e20f, 96),
        (0x25d590, 0xc9ef213507277605, 9390),
        (0x25d648, 0x9ec6c139fe4b40b5, 1710),
        (0x25d710, 0xf320e3ec9c02cb48, 6725),
        (0x25d740, 0x54699047ee9a9c7d, 8700),
        (0x25d798, 0xf9ed7b8fc90d8c84, 9819),
        (0x25d810, 0xbe0dce9ffdec74dd, 9780),
        (0x25d888, 0x9c600f93ca731f97, 1104),
        (0x25d8c0, 0x4c00288110cb8b22, 9899),
        (0x25d8d0, 0x988011640d038503, 10194),
        (0x25d8e0, 0x96d3b9605b5954ec, 9991),
        (0x25d8f0, 0x00971531a47626ef, 9930),
        (0x25d900, 0x49dc7a19eb8da4a7, 10038),
        (0x25d910, 0xe73025500a259764, 10133),
        (0x25d920, 0x8f00498df1654842, 12483),
        (0x25d960, 0xdbd4b170c1ebc359, 9894),
        (0x25d970, 0x30927f80a6b5daa0, 10021),
        (0x25d980, 0xddb14565309cb05d, 10014),
        (0x25d990, 0x14377d11d07512cd, 9952),
        (0x25d9a0, 0x07b2102b4e18962f, 9974),
        (0x25d9b0, 0xab309b5c11abc96d, 5148),
        (0x25da30, 0xf8e16e84df8ced0d, 6086),
        (0x25da90, 0xa102d47f4f77f812, 2667),
        (0x25db10, 0xf113608d1f54c8d8, 3933),
        (0x25dbc0, 0x3ae82ff691fe302f, 78),
        (0x25dc58, 0x2894872a1dc5fc8f, 10078),
        (0x25dc68, 0x75724f6d83d47cc7, 17724),
        (0x25dc98, 0x475ea216048cd7c5, 20000),
        (0x25dca0, 0x475ea216048cd7c5, 20000),
        (0x25dca8, 0xb201939eef7134f3, 9930),
        (0x25dd48, 0x96d3b9605b5954ec, 9991),
        (0x25dd58, 0xe90e311bb228c849, 9898),
        (0x25dd68, 0xfe1f4f12d633433a, 10055),
        (0x25dd78, 0x26dffc715133ea25, 9998),
        (0x25dd88, 0x0ed9e7ee21f20da5, 0),
        ];
        let mut all = std::collections::BTreeSet::new();
        for l in 0..19 {
            for k in 0..8 {
                if let Some(a) = address(l, k) { all.insert(a); }
            }
        }
        assert_eq!(all.len(), GOLDEN.len());
        let mut bad = Vec::new();
        for &(a, want_h, want_t) in GOLDEN {
            let mut s: u64 = 0x9e37_79b9_7f4a_7c15;
            let mut next = || {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                s >> 33
            };
            let (mut h, mut t) = (0xcbf2_9ce4_8422_2325u64, 0u32);
            for _ in 0..20_000 {
                let px = (next() % 520) as i32 - 4;
                let py = (next() % 520) as i32 - 4;
                let z = (next() % 1800) as f32 * 0.25 - 20.0;
                let x = (next() % 2400) as f32 * 0.25;
                let y = (next() % 2400) as f32 * 0.25;
                let group = [0, 0xf, 0x10, 0x11, 0x12, 5][(next() % 6) as usize];
                let f0634 = (next() % 3 == 0) as u8;
                let fr = (next() << 31) | next();
                let zr = next();
                let g = (next() % 2) as i32;
                let flags: Vec<u8> = (0..128).map(|n| (fr >> (n & 63) & 1) as u8).collect();
                let zone_flags: Vec<i32> = (0..8).map(|k| (zr >> k & 1) as i32).collect();
                let e = Env { x, y, z, px, py, flags: &flags, zone_flags: &zone_flags, group, f0634, g15fdfc: g };
                let b = call(a, &e) as u64;
                t += b as u32;
                h = (h ^ b).wrapping_mul(0x100_0000_01b3);
            }
            if (h, t) != (want_h, want_t) { bad.push(format!("{a:#x}: {t} true (game {want_t})")); }
        }
        assert!(bad.is_empty(), "{bad:#?}");
    }
}
