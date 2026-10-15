//! Trigger volumes and moving platforms: the shared helpers every class calls (docs/plan/triggers.md). Level01
//! addresses; the functions are engine code, identical in all 19 level overlays (tools/ghidra/names/clusters.tsv).
//!
//! **Volume tests.** A class keeps volume *indices* in its pvars (s32, −1 = none) and passes a test point:
//! Ratchet's feet `0x13f3d0` (almost every caller), the camera `0x167240` (water 751, gunship 688) or the hero
//! body point `0x13f420` (amoeboid). The test maps `p − centre` through the record's stored inverse rows
//! (`VecSub` 0x2211b8, `MatrixMulVec3` 0x2215e0: `l = d.x·inv0 + d.y·inv1 + d.z·inv2`) and bounds `l`:
//!
//! | fn | test | index guard |
//! |---|---|---|
//! | [`point_in_cuboid`] 0x274820 | `−1 ≤ l.x, l.y, l.z ≤ 1` (`c.le.s` both ends) | `== −1` |
//! | [`point_in_cylinder`] 0x2748f8 | `|l.xy| < 1` (VU `vsqrt`, 0x221318) and `−1 ≤ l.z ≤ 1` | `< 0` |
//! | [`point_in_sphere`] 0x2749b0 | `|l| < 1` (0x2212e8) | `< 0` |
//! | [`point_in_path_polygon`] 0x26e6c0 | even-odd crossing test in XY over a path's points, closed | count ≤ 0 |
//! | [`sphere_touches_shape`] 0x20ff18 | camera-grid primitive test (sphere radius only for spheres / pill caps) | — |
//! | [`box_local`] + [`in_unit_box`] | the cuboid test for records with their own box (sound instances 0x31a128 …) | — |
//!
//! The tests are stateless: no helper keeps "was inside" state. Every edge (entered / left) is kept by the
//! class that asks, in its own pvars (checkpoint 805's rising edge, the path lift's one-shot latch that clears
//! its pvar index and sets the spawn id's death bits, …): triggers.md §4 lists them.
//!
//! **Moving platforms.** A carrier is a moby with mode `0x20` whose pvar+0x08 holds the offset of its
//! *platform block* (a relative pointer fixed up by the loader; the port keeps it block-relative):
//! [`platform_block`] = `FUN_00275290`. Each tick the carrier's update calls `CarryRiders` 0x2755f8 with its
//! displacement and rotation change ([`carry_riders`]): block +0x00 = Euler of `E(rot_new)·E(rot_old)ᵀ`, +0x10 = the
//! displacement. `HeroPlatformUpdate` 0x249618 (hero code) reads the block of the moby he stands on and moves
//! him with it ([`carry_point`] is its point transform `FUN_002752c0`). [`World::hero_on_moby`] = `HeroOnMoby`
//! 0x277fb8.
//!
//! Standard `f32` throughout (the PS2 VU/FPU rounding is not modelled; boundary cases can differ by an ulp).

use crate::moby_runtime::{Moby, MobyId};
use crate::moby_update::services::{fl, Services, World};
use rc_formats::volumes::{ShapeKind, Volumes};
use std::sync::Arc;

// ---------------------------------------------------------------------------------------------------------
// Volume tests

/// The local point of `p` in a box given by its centre and world → local rows: `l = (p − c)·inv` (0x2211b8 +
/// 0x2215e0). The same computation as [`Shape::local`](rc_formats::volumes::Shape::local), for records that
/// carry their own box instead of a cuboid index: the sound-instance boxes (centre +0x40, rows +0x50;
/// `SndInstMusicBoxUpdate` 0x31a128 and the volume / one-shot / reverb boxes, audio.md §3.3).
pub fn box_local(centre: [f32; 3], inverse: &[[f32; 4]; 3], p: [f32; 3]) -> [f32; 3] {
    let d = [p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]];
    std::array::from_fn(|k| inverse[0][k] * d[0] + inverse[1][k] * d[1] + inverse[2][k] * d[2])
}

/// The unit-box bound every box test applies to a local point: `−1 ≤ l ≤ 1` on all three axes (0x274820's
/// `c.le.s` pairs; the sound boxes' `fabs(l) ≤ 1` is the same set).
pub fn in_unit_box(l: [f32; 3]) -> bool { l.iter().all(|&x| (-1.0..=1.0).contains(&x)) }

/// `PointInCuboid(p, index)` 0x274820 (table 0x1600ec). Only −1 is rejected by the game; the port also
/// rejects other negative / out-of-range indices (the game would read outside the table).
pub fn point_in_cuboid(v: &Volumes, p: [f32; 3], index: i32) -> bool {
    if index == -1 { return false; }
    let Some(s) = v.shape(ShapeKind::Cuboid, index) else { return false };
    in_unit_box(s.local(p))
}

/// `PointInCylinder(p, index)` 0x2748f8 (table 0x1600fc): the unit cylinder along local z.
pub fn point_in_cylinder(v: &Volumes, p: [f32; 3], index: i32) -> bool {
    let Some(s) = v.shape(ShapeKind::Cylinder, index) else { return false };
    let l = s.local(p);
    (l[0] * l[0] + l[1] * l[1]).sqrt() < 1.0 && (-1.0..=1.0).contains(&l[2])
}

/// `PointInSphere(p, index)` 0x2749b0 (table 0x1600f4): the unit ball.
pub fn point_in_sphere(v: &Volumes, p: [f32; 3], index: i32) -> bool {
    let Some(s) = v.shape(ShapeKind::Sphere, index) else { return false };
    let l = s.local(p);
    (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt() < 1.0
}

/// `PointInPathPolygon(p, points, count)` 0x26e6c0: the path's points as a closed polygon in XY (z ignored),
/// even-odd rule. Edge k runs from point k to point k+1 (the last back to 0); it counts when `p.y` lies in
/// `(min, max]` of its y and the crossing x is left of `p.x`.
pub fn point_in_path_polygon(p: [f32; 3], points: &[[f32; 4]]) -> bool {
    let n = points.len();
    let mut inside = false;
    for k in 0..n {
        let (a, b) = (points[k], points[(k + 1) % n]);
        let spans = (a[1] < p[1] && p[1] <= b[1]) || (b[1] < p[1] && p[1] <= a[1]);
        if spans && a[0] + ((p[1] - a[1]) / (b[1] - a[1])) * (b[0] - a[0]) < p[0] { inside = !inside; }
    }
    inside
}

/// [`point_in_path_polygon`] over path `index` (`0x1b0930[index]`); false for a negative / missing index.
pub fn point_in_path(v: &Volumes, p: [f32; 3], index: i32) -> bool {
    usize::try_from(index).ok().and_then(|i| v.paths.get(i)).is_some_and(|pts| point_in_path_polygon(p, pts))
}

/// `FUN_0020ff18(r, centre, prim)`: does a sphere touch shape `index` of `kind` (the camera-collision grid's
/// primitive test, called from its lookup `FUN_0020fdb0`). Only the sphere and the pill caps use `r`: a
/// cuboid or cylinder tests the centre alone.
/// * sphere: `|centre − c| < |row0| + r` (the radius is the length of matrix row 0);
/// * cuboid / cylinder: as [`point_in_cuboid`] / [`point_in_cylinder`];
/// * pill: the cylinder, else the caps `c ± row2` with the radius at runtime +0x80
///   ([`Volumes::pill_cap_radius`]), `|centre − cap| < radius` (r not added).
pub fn sphere_touches_shape(v: &Volumes, kind: ShapeKind, index: i32, centre: [f32; 3], r: f32) -> bool {
    let Some(s) = v.shape(kind, index) else { return false };
    let len = |a: [f32; 3]| (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    let sub = |a: [f32; 3], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    match kind {
        ShapeKind::Sphere => {
            let rad = len([s.matrix[0][0], s.matrix[0][1], s.matrix[0][2]]);
            len(sub(centre, s.centre())) < rad + r
        }
        ShapeKind::Cuboid => point_in_cuboid(v, centre, index),
        ShapeKind::Cylinder => point_in_cylinder(v, centre, index),
        ShapeKind::Pill => {
            let l = s.local(centre);
            if (l[0] * l[0] + l[1] * l[1]).sqrt() < 1.0 && (-1.0..=1.0).contains(&l[2]) { return true; }
            let cap_r = v.pill_cap_radius(index).unwrap_or(0.0);
            [1.0f32, -1.0].iter().any(|&z| len(sub(centre, s.world([0.0, 0.0, z]))) < cap_r)
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// The level's volumes in the moby services, and the World-side calls

impl Services {
    /// The level's volume sections (`rc_formats::volumes::parse_volumes`), for [`World`]'s trigger tests.
    pub fn set_volumes(&mut self, v: Volumes) { self.volumes = Arc::new(v); }
}

impl World<'_> {
    /// `0x13f3d0`: Ratchet's position (feet), the point nearly every class tests.
    pub fn hero_point(&self) -> [f32; 3] { [fl(self.hero.pos[0]), fl(self.hero.pos[1]), fl(self.hero.pos[2])] }

    /// `0x167240`: the camera position.
    pub fn camera_point(&self) -> [f32; 3] { [fl(self.camera[0]), fl(self.camera[1]), fl(self.camera[2])] }

    /// `0x13f420`: the hero body point (`pos + R·(0, 0, 0.7)`).
    pub fn hero_body_point(&self) -> [f32; 3] { [fl(self.hero.body_point[0]), fl(self.hero.body_point[1]), fl(self.hero.body_point[2])] }

    /// `PointInCuboid(p, index)` 0x274820 on this level's cuboids.
    pub fn in_cuboid(&self, p: [f32; 3], index: i32) -> bool { point_in_cuboid(&self.svc.volumes, p, index) }

    /// `PointInCylinder(p, index)` 0x2748f8.
    pub fn in_cylinder(&self, p: [f32; 3], index: i32) -> bool { point_in_cylinder(&self.svc.volumes, p, index) }

    /// `PointInSphere(p, index)` 0x2749b0.
    pub fn in_sphere(&self, p: [f32; 3], index: i32) -> bool { point_in_sphere(&self.svc.volumes, p, index) }

    /// `PointInPathPolygon(p, 0x1b0930[index])` 0x26e6c0.
    pub fn in_path(&self, p: [f32; 3], index: i32) -> bool { point_in_path(&self.svc.volumes, p, index) }

    /// `HeroOnMoby(m)` 0x277fb8: in movement group 3 (0x1413dc, the ledge states) or state 0x1c (0x1413d4, the
    /// climb) he is on `m` when it is the ledge moby `0x13f848` (`Hero::ledge_blk.moby`, set by probe B for a ledge
    /// on a moby top); otherwise he stands on `m`: air ticks `0x13f65e` = 0 and ground moby `0x13f64c` = m.
    pub fn hero_on_moby(&self, id: MobyId) -> bool {
        let h = self.hero;
        if h.group == 3 || h.state == 0x1c { return h.ledge_blk.moby == Some(id); }
        h.air_ticks == 0 && h.ground_moby == Some(id)
    }
}

// ---------------------------------------------------------------------------------------------------------
// Moving platforms

/// `FUN_00275290(m)`: the platform block of a carrier: mode `0x20` and a non-zero pvar+0x08 (the block's
/// offset in the pvar block; a heap pointer in the game). None: not a carrier.
pub fn platform_block(m: &Moby) -> Option<usize> {
    if m.mode & 0x20 == 0 || m.pvars.len() < 0x0c { return None; }
    let o = i32::from_le_bytes(m.pvars[8..12].try_into().unwrap());
    usize::try_from(o).ok().filter(|&o| o != 0 && o + 0x40 <= m.pvars.len())
}

/// `FUN_002711f8(m)`: the offset of a mode-`0x20` moby's pvar record (`**(m+0x78)`: pvar+0x00, a pointer the
/// loader fixes up; the port keeps it block-relative). None: no mode 0x20, or the record lies outside the block.
pub fn pvar_record(m: &Moby) -> Option<usize> {
    if m.mode & 0x20 == 0 || m.pvars.len() < 4 { return None; }
    let o = i32::from_le_bytes(m.pvars[0..4].try_into().unwrap());
    usize::try_from(o).ok().filter(|&o| o + 0x20 <= m.pvars.len())
}

/// Bit 0 of the pvar record's u16 at +0x1e ([`pvar_record`]): a ledge on this moby's top can be grabbed
/// (`HeroWallLedgeCheckB` 0x22d090, the moby branch).
pub fn record_ledge_flag(m: &Moby) -> bool { record_flags(m).is_some_and(|f| f & 1 != 0) }

/// The u16 at +0x1e of the moby's pvar record (`FUN_002711f8`, pvar+0x00), None without a record.
pub fn record_flags(m: &Moby) -> Option<u16> { pvar_record(m).map(|o| u16::from_le_bytes([m.pvars[o + 0x1e], m.pvars[o + 0x1f]])) }

/// The platform block as the hero code reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlatformDelta {
    /// +0x00: Euler angles of this tick's rotation change `E(rot_new)·E(rot_old)ᵀ`.
    pub rotation: [f32; 3],
    /// +0x10: this tick's displacement (w copied from the caller's vector).
    pub displacement: [f32; 4],
    /// +0x3c: flags (bit 1: the hero keeps the carried velocity after leaving; bit 2: re-attach in local space
    /// every tick; `HeroPlatformUpdate`).
    pub flags: u32,
}

/// Reads a carrier's platform block (None: not a carrier).
pub fn platform_delta(m: &Moby) -> Option<PlatformDelta> {
    let o = platform_block(m)?;
    let f = |k: usize| f32::from_le_bytes(m.pvars[o + k..o + k + 4].try_into().unwrap());
    Some(PlatformDelta {
        rotation: [f(0), f(4), f(8)],
        displacement: [f(0x10), f(0x14), f(0x18), f(0x1c)],
        flags: u32::from_le_bytes(m.pvars[o + 0x3c..o + 0x40].try_into().unwrap()),
    })
}

/// `CarryRiders(block, delta, rot_old, rot_new)` 0x2755f8, writing into `pvars[block..]`: +0x00 = the Euler angles
/// ([`matrix_euler`], `FUN_002721f0`) of the rotation change `D = E(rot_new)·E(rot_old)ᵀ` (`EulerToMatrix`
/// 0x221980 twice, the transpose 0x221c08, the product 0x221ce8(out, A, B) = B·A), +0x10 = `delta` (all four
/// words). `D·E(rot_old) = E(rot_new)`; the riders apply it as `p·D` and `E(rot)·D` ([`carried`]), which is the
/// carrier's turn exactly for a turn about z (the turntables 707 / 734), and the game's product otherwise; a yaw-only turn
/// of θ gives (0, 0, θ).
///
/// The lifts, elevators and belts pass the same rotation twice: `D = I` and the angles are exactly 0 (the port
/// skips the extraction then, so their riders move by the displacement alone); the turntables 707 / 734
/// (level02), the joint-carried platform 1210 (level03), the carrier 1584 (level18) and the attached platforms
/// 812 (level05) turn.
pub fn carry_riders(pvars: &mut [u8], block: usize, delta: [f32; 4], rot_old: [f32; 4], rot_new: [f32; 4]) {
    let r = carry_rotation([rot_old[0], rot_old[1], rot_old[2]], [rot_new[0], rot_new[1], rot_new[2]]);
    for (k, a) in r.iter().enumerate() { pvars[block + 4 * k..block + 4 * k + 4].copy_from_slice(&a.to_le_bytes()); }
    for (k, d) in delta.iter().enumerate() {
        pvars[block + 0x10 + 4 * k..block + 0x14 + 4 * k].copy_from_slice(&d.to_le_bytes());
    }
}

/// The Euler angles `CarryRiders` writes for a turn from `old` to `new` (see [`carry_riders`]): (0, 0, 0) exactly
/// when they are equal.
pub fn carry_rotation(old: [f32; 3], new: [f32; 3]) -> [f32; 3] {
    if old == new { return [0.0; 3]; }
    matrix_euler(&mul(&euler_matrix(new), &transpose(&euler_matrix(old))))
}

/// `FUN_002752c0`'s point transform: where a point riding the carrier goes this tick,
/// `((p + Δ) − c)·D + c` with `c` the carrier's position after its move ([`carried`]). None: not a carrier.
/// (The hero side of the carry, `HeroPlatformUpdate`, is hero code: triggers.md §5.)
pub fn carry_point(m: &Moby, p: [f32; 3]) -> Option<[f32; 3]> { carrier(m).map(|c| carried(&c, p, [0.0; 3]).0) }

// ---------------------------------------------------------------------------------------------------------
// Mobys riding mobys: the moby side of the platform functions (docs/plan/triggers.md §5, G-CLS-024)
//
// The same four engine functions carry Ratchet (`HeroPlatformUpdate`, hero/platform.rs) and every moby that rides
// another: bolts resting on a carrier (`BoltUpdate` 0x2bb758 / 0x2bc768), crates stacked on one (0x2eac18 /
// 0x2ec388), the Bomb Glove's bombs, mines, decoys and Doom bots, the infobot's ride, and the classes that pin
// themselves to a carrier (level05 812, …). Their argument order is `(rider, carrier, p_in, rot_in, p_out,
// rot_out)`; the rider is read only for its class slot (+0x22, [`from_local`]).
//
// Rotations: `EulerToMatrix` 0x221980 (R = X·Y·Z in the row-vector convention: rows are the images of the axes)
// and `MatrixToEuler` 0x2721f0 (z = atan2(m01, m00), then y, then x of what is left), composed in `f64` (the VU
// rounding is not modelled: docs/plan/hardware_fidelity_layers.md). hero/platform.rs keeps its own copy of the
// same algebra (lane 1's file at the time of writing; the two are the same functions and can be merged).

/// A carrier as the platform functions read it: `FUN_00275290`'s block and the moby's position, rotation rows
/// and class slot (the same record the hero reads, [`crate::hero::platform::Carrier`]).
pub use crate::hero::platform::Carrier;

/// The carrier view of moby `m` (None: not a carrier, `FUN_00275290(m) == 0`).
pub fn carrier(m: &Moby) -> Option<Carrier> {
    let delta = platform_delta(m)?;
    Some(Carrier {
        position: [m.position[0], m.position[1], m.position[2]],
        rows: [0, 1, 2].map(|k| [m.rows[k][0], m.rows[k][1], m.rows[k][2]]),
        class_slot: m.class_slot,
        delta,
    })
}

type M3 = [[f64; 3]; 3];

/// `EulerToMatrix` 0x221980 (rows; R = X·Y·Z).
pub fn euler_matrix(e: [f32; 3]) -> M3 {
    let (sx, cx) = (e[0] as f64).sin_cos();
    let (sy, cy) = (e[1] as f64).sin_cos();
    let (sz, cz) = (e[2] as f64).sin_cos();
    let x = [[1.0, 0.0, 0.0], [0.0, cx, sx], [0.0, -sx, cx]];
    let y = [[cy, 0.0, -sy], [0.0, 1.0, 0.0], [sy, 0.0, cy]];
    let z = [[cz, sz, 0.0], [-sz, cz, 0.0], [0.0, 0.0, 1.0]];
    mul(&mul(&x, &y), &z)
}

/// `a·b` (row i of the result = row i of `a` through `b`).
fn mul(a: &M3, b: &M3) -> M3 { std::array::from_fn(|i| std::array::from_fn(|k| (0..3).map(|j| a[i][j] * b[j][k]).sum())) }

/// 0x221c08: the transpose of the rotation rows.
fn transpose(a: &M3) -> M3 { std::array::from_fn(|i| std::array::from_fn(|k| a[k][i])) }

/// `MatrixToEuler` 0x2721f0.
pub fn matrix_euler(m: &M3) -> [f32; 3] {
    let z = m[0][1].atan2(m[0][0]);
    let m1 = mul(m, &euler_matrix([0.0, 0.0, -z as f32]));
    let y = (-m1[0][2]).atan2(m1[0][0]);
    let m2 = mul(&m1, &euler_matrix([0.0, -y as f32, 0.0]));
    let x = m2[1][2].atan2(m2[1][1]);
    [x as f32, y as f32, z as f32]
}

fn rows64(c: &Carrier) -> M3 { c.rows.map(|r| r.map(|x| x as f64)) }
fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }

/// `FUN_002752c0(rider, carrier, p, rot, &p_out, &rot_out)` (true when the carrier has a block): where a point and a
/// rotation riding the carrier go this tick. `p_out = ((p + Δ) − c)·D + c` (`VecAdd`, `VecSub`, `MatrixMulVec3`
/// 0x2215e0 with `D = E(block+0)`, `VecAdd`); `rot_out = Euler(E(rot)·D)`. Without a turn (`D = I`) the point
/// moves by Δ and the rotation is returned as it is.
pub fn carried(c: &Carrier, p: [f32; 3], rot: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let d = &c.delta;
    let q = add3(p, [d.displacement[0], d.displacement[1], d.displacement[2]]);
    if d.rotation == [0.0; 3] { return (q, rot); }
    let md = euler_matrix(d.rotation);
    let v = sub3(q, c.position).map(|x| x as f64);
    let r: [f32; 3] = std::array::from_fn(|k| (v[0] * md[0][k] + v[1] * md[1][k] + v[2] * md[2][k]) as f32);
    (add3(r, c.position), matrix_euler(&mul(&euler_matrix(rot), &md)))
}

/// `FUN_00275528(rider, carrier, p, rot, &l_out, &lrot_out)`: the point and rotation in the carrier's local space,
/// `l = (p − c)·rowsᵀ` and `lrot = Euler(E(rot)·rowsᵀ)`.
pub fn to_local(c: &Carrier, p: [f32; 3], rot: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let d = sub3(p, c.position);
    let l = c.rows.map(|r| d[0] * r[0] + d[1] * r[1] + d[2] * r[2]);
    (l, matrix_euler(&mul(&euler_matrix(rot), &transpose(&rows64(c)))))
}

/// `FUN_002753b0(rider, carrier, l, lrot, &p_out, &rot_out)`: a local point and rotation back in the world,
/// `p = l·rows + c` (`MatrixMulVec3`, `VecAdd`), `rot = Euler(E(lrot)·rows)` (0x221bc8, 0x221ce8, 0x2721f0); then
/// with block flag bit 2 `p += Δ` and the local pair re-recorded from the result ([`to_local`], 0x275528); without
/// it, a rider whose class slot is non-zero and below the carrier's (it updated before the carrier this tick) gets
/// `ClampLen(Δ, 1)` (0x2745f0) added.
pub fn from_local(c: &Carrier, rider_slot: u8, l: &mut [f32; 3], lrot: &mut [f32; 3]) -> ([f32; 3], [f32; 3]) {
    let r = &c.rows;
    let mut p: [f32; 3] = std::array::from_fn(|k| l[0] * r[0][k] + l[1] * r[1][k] + l[2] * r[2][k]);
    p = add3(p, c.position);
    let rot = matrix_euler(&mul(&euler_matrix(*lrot), &rows64(c)));
    let d = [c.delta.displacement[0], c.delta.displacement[1], c.delta.displacement[2]];
    if c.delta.flags & 4 != 0 {
        p = add3(p, d);
        (*l, *lrot) = to_local(c, p, rot);
    } else if rider_slot != 0 && rider_slot < c.class_slot {
        let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        let d = if 1.0 < n { d.map(|x| x * (1.0 / n)) } else { d };
        p = add3(p, d);
    }
    (p, rot)
}

/// One child record of [`record_children`] / [`place_children`] (0x30 bytes in the parent's pvars): +0x00 the child's
/// offset in the parent's frame, +0x10 its Euler angles in it, +0x20 the child's moby index (−1: none).
pub const CHILD_RECORD: usize = 0x30;

/// Level18 `0x265250(parent, records, n)` (the same code on level12, `0x27b268`; cluster 6e3b7a963b84): each child's
/// pose in the parent's frame, `offset = (child.pos − parent.pos)·Rᵀ` (0x221608 with the transpose 0x221c08 of
/// `R = E(parent rot)`, 0x221960) and `euler = Euler(E(child rot)·Rᵀ)` (0x221ce8, 0x2721f0). Records whose index is
/// −1 (or outside the table) are skipped. The offset's w is `child.w − parent.w` (row 3 of the transpose is
/// (0, 0, 0, 1)); the Euler record's w is left as it is.
pub fn record_children(w: &mut World, parent: MobyId, at: usize, n: usize) {
    use crate::moby_update::services::pvar as p;
    let (ppos, prot) = { let m = w.m(parent); (m.position, m.rotation) };
    let rt = transpose(&euler_matrix([prot[0], prot[1], prot[2]]));
    for k in 0..n {
        let o = at + k * CHILD_RECORD;
        if o + CHILD_RECORD > w.m(parent).pvars.len() { break; }
        let idx = p::i32(&w.m(parent).pvars, o + 0x20);
        let Some(c) = usize::try_from(idx).ok().filter(|&i| i < w.table.mobys.len()) else { continue };
        let (cpos, crot) = { let m = w.m(c); (m.position, m.rotation) };
        let d = [(cpos[0] - ppos[0]) as f64, (cpos[1] - ppos[1]) as f64, (cpos[2] - ppos[2]) as f64];
        let l: [f32; 3] = std::array::from_fn(|j| (d[0] * rt[0][j] + d[1] * rt[1][j] + d[2] * rt[2][j]) as f32);
        let e = matrix_euler(&mul(&euler_matrix([crot[0], crot[1], crot[2]]), &rt));
        let pv = &mut w.mm(parent).pvars;
        let ew = p::ff(pv, o + 0x1c);
        p::set_v4f(pv, o, [l[0], l[1], l[2], cpos[3] - ppos[3]]);
        p::set_v4f(pv, o + 0x10, [e[0], e[1], e[2], ew]);
    }
}

/// Level18 `0x265358(parent, records, n)` (level12 `0x27b370`; cluster 584a926e540b): each recorded child placed by
/// the parent's matrix now, `child.pos = parent.pos + offset·R` (0x221608: `offset.xyz·R` plus `(0, 0, 0, offset.w)`
/// from R's row 3), `child.rot = Euler(E(euler)·R)` (0x221ce8, 0x2721f0), then `MobyBuildMatrix(child)`.
pub fn place_children(w: &mut World, parent: MobyId, at: usize, n: usize) {
    use crate::moby_update::services::pvar as p;
    let (ppos, prot) = { let m = w.m(parent); (m.position, m.rotation) };
    let r = euler_matrix([prot[0], prot[1], prot[2]]);
    for k in 0..n {
        let o = at + k * CHILD_RECORD;
        let pv = &w.m(parent).pvars;
        if o + CHILD_RECORD > pv.len() { break; }
        let (l, e, idx) = (p::v4f(pv, o), p::v4f(pv, o + 0x10), p::i32(pv, o + 0x20));
        let Some(c) = usize::try_from(idx).ok().filter(|&i| i < w.table.mobys.len()) else { continue };
        let q: [f32; 3] = std::array::from_fn(|j| (l[0] as f64 * r[0][j] + l[1] as f64 * r[1][j] + l[2] as f64 * r[2][j]) as f32);
        let rot = matrix_euler(&mul(&euler_matrix([e[0], e[1], e[2]]), &r));
        let m = w.mm(c);
        m.position = [ppos[0] + q[0], ppos[1] + q[1], ppos[2] + q[2], ppos[3] + l[3]];
        m.rotation[..3].copy_from_slice(&rot);
        w.build_matrix(c);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rc_formats::volumes::Shape;

    /// A shape centred at `c` with local axes `axes` (rows; half extents = their lengths) and the exact inverse
    /// rows for orthogonal axes.
    fn shape(c: [f32; 3], axes: [[f32; 3]; 3]) -> Shape {
        let mut s = Shape::default();
        for (k, &a) in axes.iter().enumerate() {
            let l2 = a[0] * a[0] + a[1] * a[1] + a[2] * a[2];
            s.matrix[k] = [a[0], a[1], a[2], 0.0];
            // inverse: column k of the inverse = a_k / |a_k|² → rows j get a_k[j] / l2 in lane k.
            for (row, &x) in s.inverse.iter_mut().zip(&a) { row[k] = x / l2; }
        }
        s.matrix[3] = [c[0], c[1], c[2], 1.0];
        s
    }

    fn volumes() -> Volumes {
        // Cuboid 0: centre (10, 20, 5), half sizes 4 (along world +y), 2 (along world −x), 3 (z).
        let cub = shape([10.0, 20.0, 5.0], [[0.0, 4.0, 0.0], [-2.0, 0.0, 0.0], [0.0, 0.0, 3.0]]);
        // Cylinder 0: radius 2 in xy, half height 1, at (0, 0, 10).
        let cyl = shape([0.0, 0.0, 10.0], [[2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 1.0]]);
        // Sphere 0: radius 5 at (100, 0, 0).
        let sph = shape([100.0, 0.0, 0.0], [[5.0, 0.0, 0.0], [0.0, 5.0, 0.0], [0.0, 0.0, 5.0]]);
        // Path 0: the square (0,0)-(10,0)-(10,10)-(0,10).
        let sq = vec![[0.0, 0.0, 0.0, 0.0], [10.0, 0.0, 0.0, 0.0], [10.0, 10.0, 0.0, 0.0], [0.0, 10.0, 0.0, 0.0]];
        Volumes { cuboids: vec![cub], cylinders: vec![cyl], spheres: vec![sph], paths: vec![sq], ..Default::default() }
    }

    #[test]
    fn cuboid_faces_are_inclusive() {
        let v = volumes();
        assert!(point_in_cuboid(&v, [10.0, 20.0, 5.0], 0));
        // Faces: y ± 4, x ± 2, z ± 3 are inside (c.le.s), just past them outside.
        for p in [[10.0, 24.0, 5.0], [10.0, 16.0, 5.0], [8.0, 20.0, 5.0], [12.0, 20.0, 5.0], [10.0, 20.0, 8.0], [10.0, 20.0, 2.0]] {
            assert!(point_in_cuboid(&v, p, 0), "{p:?}");
        }
        for p in [[10.0, 24.01, 5.0], [7.99, 20.0, 5.0], [10.0, 20.0, 8.01], [14.0, 20.0, 5.0]] {
            assert!(!point_in_cuboid(&v, p, 0), "{p:?}");
        }
        assert!(!point_in_cuboid(&v, [10.0, 20.0, 5.0], -1));
        let cub = &v.cuboids[0];
        assert_eq!(box_local(cub.centre(), &cub.inverse, [12.0, 24.0, 8.0]), cub.local([12.0, 24.0, 8.0]));
        assert!(in_unit_box([1.0, -1.0, 0.0]) && !in_unit_box([1.0, -1.0, 1.0001]));
        assert!(!point_in_cuboid(&v, [10.0, 20.0, 5.0], 1));
    }

    #[test]
    fn cylinder_sphere_and_path() {
        let v = volumes();
        assert!(point_in_cylinder(&v, [1.9, 0.0, 10.0], 0));
        assert!(!point_in_cylinder(&v, [2.0, 0.0, 10.0], 0), "|l.xy| < 1 is strict");
        assert!(point_in_cylinder(&v, [0.0, 0.0, 11.0], 0), "−1 ≤ l.z ≤ 1 is inclusive");
        assert!(!point_in_cylinder(&v, [0.0, 0.0, 11.01], 0));
        assert!(!point_in_cylinder(&v, [0.0, 0.0, 10.0], -2));
        assert!(point_in_sphere(&v, [104.9, 0.0, 0.0], 0));
        assert!(!point_in_sphere(&v, [105.0, 0.0, 0.0], 0), "strict");
        assert!(!point_in_sphere(&v, [104.0, 3.0, 0.0], 0));
        assert!(point_in_path(&v, [5.0, 5.0, 99.0], 0), "z ignored");
        assert!(!point_in_path(&v, [15.0, 5.0, 0.0], 0));
        assert!(!point_in_path(&v, [5.0, 5.0, 0.0], 1));
        // An edge's y range is (min, max]: y = 0 (the bottom edge's level) is outside, y = 10 inside.
        assert!(!point_in_path_polygon([5.0, 0.0, 0.0], &v.paths[0]));
        assert!(point_in_path_polygon([5.0, 10.0, 0.0], &v.paths[0]));
        assert!(!point_in_path_polygon([5.0, 5.0, 0.0], &[]));
        // A concave polygon (U shape): the notch is outside.
        let u = [[0.0, 0.0], [9.0, 0.0], [9.0, 9.0], [6.0, 9.0], [6.0, 3.0], [3.0, 3.0], [3.0, 9.0], [0.0, 9.0]].map(|p| [p[0], p[1], 0.0, 0.0]);
        assert!(!point_in_path_polygon([4.5, 6.0, 0.0], &u));
        assert!(point_in_path_polygon([1.5, 6.0, 0.0], &u));
        assert!(point_in_path_polygon([4.5, 1.5, 0.0], &u));
    }

    #[test]
    fn camera_grid_sphere_test() {
        let mut v = volumes();
        assert!(sphere_touches_shape(&v, ShapeKind::Sphere, 0, [106.0, 0.0, 0.0], 1.5));
        assert!(!sphere_touches_shape(&v, ShapeKind::Sphere, 0, [106.5, 0.0, 0.0], 1.5));
        // Cuboid / cylinder: the radius is not used.
        assert!(!sphere_touches_shape(&v, ShapeKind::Cuboid, 0, [10.0, 24.5, 5.0], 10.0));
        assert!(sphere_touches_shape(&v, ShapeKind::Cylinder, 0, [1.0, 1.0, 10.5], 0.0));
        // A pill: the cylinder plus caps at centre ± row2 with the +0x80 radius.
        v.pills = v.cylinders.clone();
        v.pill_tail = vec![0.5];
        assert!(sphere_touches_shape(&v, ShapeKind::Pill, 0, [0.0, 0.3, 11.3], 0.0));
        assert!(!sphere_touches_shape(&v, ShapeKind::Pill, 0, [0.0, 0.3, 11.6], 0.0));
        assert!(sphere_touches_shape(&v, ShapeKind::Pill, 0, [0.0, 0.0, 8.6], 0.0));
    }

    #[test]
    fn carry_block_round_trip() {
        let mut m = Moby::zeroed();
        m.pvars = vec![0; 0xd0];
        m.pvars[8..12].copy_from_slice(&0x60i32.to_le_bytes());
        assert!(platform_block(&m).is_none(), "mode 0x20 required");
        m.mode |= 0x20;
        assert_eq!(platform_block(&m), Some(0x60));
        m.position = [5.0, 5.0, 10.0, 0.0];
        let rot = [0.0, 0.0, 0.7, 0.0];
        carry_riders(&mut m.pvars, 0x60, [0.25, -0.5, 1.0, -1.0], rot, rot);
        let d = platform_delta(&m).unwrap();
        assert_eq!(d.rotation, [0.0; 3]);
        assert_eq!(d.displacement, [0.25, -0.5, 1.0, -1.0]);
        assert_eq!(carry_point(&m, [1.0, 2.0, 3.0]), Some([1.25, 1.5, 4.0]));
    }

    fn close(a: [f32; 3], b: [f32; 3]) -> bool { (0..3).all(|k| (a[k] - b[k]).abs() < 1e-5) }

    fn carrier_at(pos: [f32; 3], rot: [f32; 3], slot: u8, delta: PlatformDelta) -> Carrier {
        let r = euler_matrix(rot);
        Carrier { position: pos, rows: r.map(|row| row.map(|x| x as f32)), class_slot: slot, delta }
    }

    /// `CarryRiders` with a turn: D = E(new)·E(old)ᵀ, so E(old) carried by D is E(new); a yaw-only turn is (0, 0, θ).
    #[test]
    fn carry_rotation_is_the_turn_between_the_frames() {
        assert_eq!(carry_rotation([0.1, 0.2, 0.3], [0.1, 0.2, 0.3]), [0.0; 3]);
        let d = carry_rotation([0.0, 0.0, 0.5], [0.0, 0.0, 0.75]);
        assert!(close(d, [0.0, 0.0, 0.25]), "{d:?}");
        let (old, new) = ([0.2, -0.3, 1.0], [0.25, 0.1, 1.4]);
        let d = carry_rotation(old, new);
        // D·E(old) = E(new): D is the turn in the order the game multiplies (0x221ce8(out, A, B) = B·A).
        assert!(close(matrix_euler(&mul(&euler_matrix(d), &euler_matrix(old))), new));
        // A rider's rotation is carried as E(rot)·D (`FUN_002752c0`): for a yaw-only turn that is the carrier's turn.
        let c = carrier_at([0.0; 3], [0.0, 0.0, 0.75], 0, PlatformDelta { rotation: carry_rotation([0.0, 0.0, 0.5], [0.0, 0.0, 0.75]), ..Default::default() });
        assert!(close(carried(&c, [0.0; 3], [0.0, 0.0, 0.5]).1, [0.0, 0.0, 0.75]));
        let mut pv = vec![0u8; 0x40];
        carry_riders(&mut pv, 0, [1.0, 2.0, 3.0, 4.0], [old[0], old[1], old[2], 0.0], [new[0], new[1], new[2], 0.0]);
        assert_eq!(f32::from_le_bytes(pv[8..12].try_into().unwrap()), d[2]);
        assert_eq!(f32::from_le_bytes(pv[0x1c..0x20].try_into().unwrap()), 4.0);
    }

    /// `FUN_00275528` then `FUN_002753b0` on an unmoved carrier gives the pose back; with block flag bit 2 the move
    /// is added and the local pose re-recorded; a rider that updated before the carrier (slot below it) gets
    /// `ClampLen(Δ, 1)`.
    #[test]
    fn local_pose_round_trip_and_the_corrections() {
        let delta = PlatformDelta { displacement: [3.0, 0.0, 0.0, 0.0], ..Default::default() };
        let c = carrier_at([10.0, 5.0, 1.0], [0.0, 0.0, 0.6], 7, delta);
        let (p, r) = ([12.0, 4.0, 2.5], [0.0, 0.1, -0.4]);
        let (mut l, mut lr) = to_local(&c, p, r);
        let (q, qr) = from_local(&c, 9, &mut l, &mut lr);
        assert!(close(q, p) && close(qr, r), "{q:?} {qr:?}");
        // Slot 3 < 7: + Δ clamped to length 1.
        let (q, _) = from_local(&c, 3, &mut l, &mut lr);
        assert!(close(q, [13.0, 4.0, 2.5]), "{q:?}");
        // Slot 0: no correction.
        assert!(close(from_local(&c, 0, &mut l, &mut lr).0, p));
        // Flag bit 2: + Δ in full, and the local point now holds the moved pose.
        let c4 = Carrier { delta: PlatformDelta { flags: 4, ..delta }, ..c };
        let (q, _) = from_local(&c4, 3, &mut l, &mut lr);
        assert!(close(q, [15.0, 4.0, 2.5]), "{q:?}");
        assert!(close(to_local(&c4, q, r).0, l));
    }

    /// Novalis (from `extracted/`): the checkpoint / mission cuboids the classes name, tested at Ratchet's
    /// spawn; the native cuboid test agrees with the water port's PS2-float one on a grid of points.
    #[test]
    fn novalis_cuboids() {
        let Some(g) = rc_formats::test_data::gameplay(1) else { eprintln!("skipped: no extracted/levels/01"); return };
        let v = rc_formats::volumes::parse_volumes(&g).unwrap();
        let spawn = [162.53032, 136.39348, 60.5];
        // Checkpoint 805 (instance 906) pvar+0 = 54 contains the spawn; the mission cuboid 42 (730 / 790 / 737 /
        // 805) at (159.5, 208.4, 40) does not, but its centre is inside.
        assert!(point_in_cuboid(&v, spawn, 54));
        assert!(!point_in_cuboid(&v, spawn, 42));
        assert!(point_in_cuboid(&v, [159.51, 208.44, 40.0], 42));
        let water: Vec<rc_formats::water::Cuboid> = rc_formats::water::parse_cuboids(&g).unwrap();
        let mut disagree = 0;
        for (i, c) in water.iter().enumerate() {
            let s = &v.cuboids[i];
            assert_eq!(c.matrix, s.matrix);
            for dx in -12..=12 {
                for dz in -3..=3 {
                    let p = s.world([dx as f32 / 10.0 + 0.005, 0.37, dz as f32 / 2.5 + 0.005]);
                    if point_in_cuboid(&v, p, i as i32) != crate::water::cuboid_contains(c, p) { disagree += 1; }
                }
            }
        }
        // Off the faces the two float models agree (on a face they can round either way).
        assert_eq!(disagree, 0);
    }
}
