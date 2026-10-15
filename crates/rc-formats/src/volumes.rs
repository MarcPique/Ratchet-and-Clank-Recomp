//! Gameplay volumes: the shape sections the trigger tests read (cuboids 0x60, spheres 0x64, cylinders 0x68,
//! pills 0x6c), plus the two spline sections the area tests and the grind code read (paths 0x70, grind paths
//! 0x74). Spec and the tests that use them: docs/plan/triggers.md; section layout: docs/formats/wad_layouts_rac1.md
//! §3.2, docs/formats/collision_rac1.md §6.2.
//!
//! **Loader** (`InitLevelRenderGlobals` 0x255958, level01, after the grind paths):
//!
//! | gameplay ptr | runtime table (count / base) | stride | test |
//! |---|---|---|---|
//! | 0x60 cuboids | 0x1600f0 / 0x1600ec | 0x80 | `PointInCuboid` 0x274820 |
//! | 0x64 spheres | 0x1600f8 / 0x1600f4 | 0x80 | `PointInSphere` 0x2749b0 |
//! | 0x68 cylinders | 0x160100 / 0x1600fc | 0x80 | `PointInCylinder` 0x2748f8 |
//! | 0x6c pills | 0x1600e8 / 0x1600e4 | **0x90** | camera-grid only (`FUN_0020ff18` type 7) |
//! | 0x70 paths | 0x160104 / 0x1b0930[i] | — | `PointInPathPolygon` 0x26e6c0, spline sample / project |
//! | 0x74 grind paths | 0x15f710 / 0x15f70c | 0x20 | grind rails (0x314e98 / 0x315358) |
//!
//! Each shape section is `s32 count, pad[3]` then `count` × 0x80-byte [`Shape`] records; the loader zeroes the
//! table and `MemCopy`s every record. Pills are the exception: the runtime stride is 0x90 and the loader copies
//! **0x90 bytes from each 0x80-byte source record**, so the runtime +0x80 word (the pill cap radius the
//! camera-grid test reads) is the first word of whatever follows the record in the file. No RAC1 level has a
//! pill (every count is 0), so this never matters on disc; [`Volumes::pill_cap_radius`] reproduces it.

use crate::buf::{invalid, Buf, Result};
use bytemuck::{Pod, Zeroable};

/// Gameplay header pointers of the four shape sections.
pub const CUBOIDS_POINTER: usize = 0x60;
pub const SPHERES_POINTER: usize = 0x64;
pub const CYLINDERS_POINTER: usize = 0x68;
pub const PILLS_POINTER: usize = 0x6c;
/// Gameplay header pointer of the grind paths.
pub const GRIND_PATHS_POINTER: usize = 0x74;
/// Size of one shape record in the file (and the runtime stride of cuboids, spheres and cylinders).
pub const SHAPE_SIZE: usize = 0x80;
/// Runtime stride of the pill table (0x1600e4).
pub const PILL_RUNTIME_SIZE: usize = 0x90;
/// Size of one grind-path header record in the file.
pub const GRIND_PATH_SIZE: usize = 0x20;

/// The four shape kinds, in the camera-collision grid's type numbering (primitive +0x10,
/// docs/formats/collision_rac1.md §6.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShapeKind {
    Cuboid,
    Sphere,
    Cylinder,
    Pill,
}

impl ShapeKind {
    pub const ALL: [ShapeKind; 4] = [ShapeKind::Cuboid, ShapeKind::Sphere, ShapeKind::Cylinder, ShapeKind::Pill];

    /// Gameplay header pointer of this kind's section.
    pub const fn pointer(self) -> usize {
        match self {
            ShapeKind::Cuboid => CUBOIDS_POINTER,
            ShapeKind::Sphere => SPHERES_POINTER,
            ShapeKind::Cylinder => CYLINDERS_POINTER,
            ShapeKind::Pill => PILLS_POINTER,
        }
    }

    /// Camera-collision grid type tag (`FUN_0020ff18`'s switch): 3 cuboid, 5 sphere, 6 cylinder, 7 pill.
    pub const fn grid_type(self) -> i32 {
        match self {
            ShapeKind::Cuboid => 3,
            ShapeKind::Sphere => 5,
            ShapeKind::Cylinder => 6,
            ShapeKind::Pill => 7,
        }
    }

    /// The kind of a camera-grid type tag.
    pub fn from_grid_type(t: i32) -> Option<ShapeKind> { ShapeKind::ALL.into_iter().find(|k| k.grid_type() == t) }
}

/// One shape record (0x80 bytes), identical for all four kinds. The shape is a canonical primitive in local
/// space mapped to the world by `matrix`: cuboid |l.x|, |l.y|, |l.z| ≤ 1; sphere |l| < 1; cylinder |l.xy| < 1,
/// |l.z| ≤ 1 (axis = local z); pill = the cylinder plus two cap spheres at local z = ±1.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Shape {
    /// 0x00: local → world, VU row-vector form: `world = l.x·row0 + l.y·row1 + l.z·row2 + row3`. Row 3 is the
    /// centre (the tests subtract it: `+0x30`). Wrench writes 0.01 into `[3][3]`; the tests never read it.
    pub matrix: [[f32; 4]; 4],
    /// 0x40: world → local rotation/scale rows applied to `p − centre` (VU0 0x2215e0:
    /// `l = d.x·inv0 + d.y·inv1 + d.z·inv2`). The inverse translation is not stored (the tests subtract the
    /// centre first).
    pub inverse: [[f32; 4]; 3],
    /// 0x70: Euler angles of `matrix`. No test reads them; classes that use a cuboid as a placement marker do
    /// (camera trigger 737: `HeroTeleport(centre, euler)` and `CameraScript(centre, euler)`).
    pub euler: [f32; 3],
    /// 0x7c: unused (0).
    pub unused_7c: f32,
}
const _: () = assert!(std::mem::size_of::<Shape>() == SHAPE_SIZE);

impl Shape {
    /// The centre (`+0x30`).
    pub fn centre(&self) -> [f32; 3] { [self.matrix[3][0], self.matrix[3][1], self.matrix[3][2]] }

    /// `p − centre` through the stored inverse rows: the point in the shape's local space (`0x2211b8` then
    /// `0x2215e0`, as every test computes it).
    pub fn local(&self, p: [f32; 3]) -> [f32; 3] {
        let c = self.centre();
        let d = [p[0] - c[0], p[1] - c[1], p[2] - c[2]];
        let r = &self.inverse;
        std::array::from_fn(|k| r[0][k] * d[0] + r[1][k] * d[1] + r[2][k] * d[2])
    }

    /// The world point of local point `l` (`matrix`, row-vector form).
    pub fn world(&self, l: [f32; 3]) -> [f32; 3] {
        let m = &self.matrix;
        std::array::from_fn(|k| l[0] * m[0][k] + l[1] * m[1][k] + l[2] * m[2][k] + m[3][k])
    }
}

/// One grind path (section 0x74): a header record and its spline. The loader keeps
/// `{bsphere, spline pointer (+0x10), flag (+0x14)}` at `0x15f70c + i·0x20`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GrindPath {
    /// Record +0x00: bounding sphere (x, y, z, radius).
    pub bsphere: [f32; 4],
    /// Record +0x10 (runtime +0x14): 0 or 1 on disc (read by the grind code; meaning not reversed).
    pub flag: i32,
    /// The spline, same format as a path: `s32 count, pad[3]`, then `count` × (x, y, z, w).
    pub points: Vec<[f32; 4]>,
}

/// Every volume section of one gameplay file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Volumes {
    pub cuboids: Vec<Shape>,
    pub spheres: Vec<Shape>,
    pub cylinders: Vec<Shape>,
    pub pills: Vec<Shape>,
    /// The runtime +0x80 word of each pill (0x1600e4 + i·0x90 + 0x80): the file word after the record (see the
    /// module doc). Empty on every RAC1 level.
    pub pill_tail: Vec<f32>,
    /// Section 0x70, `0x1b0930[i]` (the same data as `crate::gameplay::parse_splines`).
    pub paths: Vec<Vec<[f32; 4]>>,
    pub grind_paths: Vec<GrindPath>,
}

impl Volumes {
    /// The shapes of one kind.
    pub fn shapes(&self, kind: ShapeKind) -> &[Shape] {
        match kind {
            ShapeKind::Cuboid => &self.cuboids,
            ShapeKind::Sphere => &self.spheres,
            ShapeKind::Cylinder => &self.cylinders,
            ShapeKind::Pill => &self.pills,
        }
    }

    /// Shape `index` of `kind`, None for a negative or out-of-range index.
    pub fn shape(&self, kind: ShapeKind, index: i32) -> Option<&Shape> { self.shapes(kind).get(usize::try_from(index).ok()?) }

    /// The cap radius the camera-grid test reads at pill +0x80.
    pub fn pill_cap_radius(&self, index: i32) -> Option<f32> { self.pill_tail.get(usize::try_from(index).ok()?).copied() }
}

/// Section header `s32 count` at `s` (the other three words are padding); None for an absent section.
fn section_count(g: Buf, pointer: usize, what: &str) -> Result<Option<(usize, usize)>> {
    let s = g.u32(pointer)? as usize;
    if s == 0 { return Ok(None); }
    let n = g.i32(s)?;
    if !(0..=0x10000).contains(&n) { return invalid(format!("implausible {what} count {n}")); }
    Ok(Some((s, n as usize)))
}

/// One shape section.
pub fn parse_shapes(gameplay: &[u8], kind: ShapeKind) -> Result<Vec<Shape>> {
    let g = Buf(gameplay);
    let Some((s, n)) = section_count(g, kind.pointer(), "shape")? else { return Ok(Vec::new()) };
    g.pod_slice::<Shape>(s + 0x10, n, "shape records")
}

/// The grind paths: header `{s32 count, s32 data_offset, s32 data_size, pad}` (offsets from the section
/// start), `count` × 0x20 records `{f32 bsphere[4], s32 flag, pad[3]}`, then `count` s32 spline offsets into the
/// data (the loader's `0x15f70c[i]+0x10 = data copy + offset[i]`).
pub fn parse_grind_paths(gameplay: &[u8]) -> Result<Vec<GrindPath>> {
    let g = Buf(gameplay);
    let Some((s, n)) = section_count(g, GRIND_PATHS_POINTER, "grind path")? else { return Ok(Vec::new()) };
    let (data_ofs, data_size) = (g.i32(s + 4)?, g.i32(s + 8)?);
    if data_ofs < 0 || data_size < 0 { return invalid("negative grind path data field"); }
    let data = g.sub(s + data_ofs as usize, data_size as usize, "grind path data")?;
    let offsets = s + 0x10 + n * GRIND_PATH_SIZE;
    (0..n)
        .map(|i| {
            let r = s + 0x10 + i * GRIND_PATH_SIZE;
            let bsphere = [g.f32(r)?, g.f32(r + 4)?, g.f32(r + 8)?, g.f32(r + 12)?];
            let flag = g.i32(r + 0x10)?;
            let o = g.i32(offsets + 4 * i)?;
            let o = usize::try_from(o).map_err(|_| crate::FormatError::Invalid(format!("grind path {i}: negative offset")))?;
            let k = data.i32(o)?;
            if k < 0 { return invalid(format!("grind path {i}: negative point count")); }
            let points = data.pod_slice::<[f32; 4]>(o + 0x10, k as usize, "grind path points")?;
            Ok(GrindPath { bsphere, flag, points })
        })
        .collect()
}

/// Every volume section.
pub fn parse_volumes(gameplay: &[u8]) -> Result<Volumes> {
    let g = Buf(gameplay);
    let pills = parse_shapes(gameplay, ShapeKind::Pill)?;
    let pill_tail = match section_count(g, PILLS_POINTER, "pill")? {
        // The loader's 0x90-byte copy: +0x80 of record i is the file word at record + 0x80.
        Some((s, n)) => (0..n).map(|i| g.f32(s + 0x10 + i * SHAPE_SIZE + SHAPE_SIZE)).collect::<Result<_>>()?,
        None => Vec::new(),
    };
    Ok(Volumes {
        cuboids: parse_shapes(gameplay, ShapeKind::Cuboid)?,
        spheres: parse_shapes(gameplay, ShapeKind::Sphere)?,
        cylinders: parse_shapes(gameplay, ShapeKind::Cylinder)?,
        pills,
        pill_tail,
        paths: crate::gameplay::parse_splines(gameplay)?,
        grind_paths: parse_grind_paths(gameplay)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(b: &mut [u8], o: usize, v: &[u8]) { b[o..o + v.len()].copy_from_slice(v); }

    /// A gameplay file with one cuboid (centre (10, 20, 5), half sizes (4, 2, 3)), no spheres, two cylinders and
    /// one pill whose record is followed by the marker 7.5.
    #[test]
    fn synthetic_sections() {
        let mut g = vec![0u8; 0x800];
        let shape = |c: [f32; 3], h: [f32; 3]| {
            let mut s = Shape::default();
            for (k, &x) in h.iter().enumerate() { s.matrix[k][k] = x; s.inverse[k][k] = 1.0 / x; }
            s.matrix[3] = [c[0], c[1], c[2], 1.0];
            s
        };
        let cub = shape([10.0, 20.0, 5.0], [4.0, 2.0, 3.0]);
        put(&mut g, 0x60, &0x100u32.to_le_bytes());
        put(&mut g, 0x100, &1i32.to_le_bytes());
        put(&mut g, 0x110, bytemuck::bytes_of(&cub));
        put(&mut g, 0x64, &0x200u32.to_le_bytes()); // count 0
        put(&mut g, 0x68, &0x300u32.to_le_bytes());
        put(&mut g, 0x300, &2i32.to_le_bytes());
        put(&mut g, 0x310, bytemuck::bytes_of(&cub));
        put(&mut g, 0x390, bytemuck::bytes_of(&shape([0.0; 3], [1.0; 3])));
        put(&mut g, 0x6c, &0x500u32.to_le_bytes());
        put(&mut g, 0x500, &1i32.to_le_bytes());
        put(&mut g, 0x510, bytemuck::bytes_of(&cub));
        put(&mut g, 0x590, &7.5f32.to_le_bytes());
        let v = parse_volumes(&g).unwrap();
        assert_eq!((v.cuboids.len(), v.spheres.len(), v.cylinders.len(), v.pills.len()), (1, 0, 2, 1));
        assert_eq!(v.cuboids[0], cub);
        assert_eq!(v.pill_cap_radius(0), Some(7.5));
        assert_eq!(v.cuboids[0].local([14.0, 20.0, 8.0]), [1.0, 0.0, 1.0]);
        assert_eq!(v.cuboids[0].world([1.0, -1.0, 0.0]), [14.0, 18.0, 5.0]);
        assert!(v.shape(ShapeKind::Cuboid, -1).is_none() && v.shape(ShapeKind::Cylinder, 1).is_some());
        assert!(v.paths.is_empty() && v.grind_paths.is_empty());
        assert_eq!(ShapeKind::from_grid_type(6), Some(ShapeKind::Cylinder));
    }

    /// Every level of the disc (from `extracted/`): the section counts, no parse error, the stored inverse is
    /// the inverse of the stored matrix (row-vector convention), and the grind splines are well formed.
    #[test]
    fn all_levels_disc() {
        // (cuboids, spheres, cylinders, pills, paths, grind paths) per level 0..18.
        let expect: [(usize, usize, usize, usize, usize, usize); 19] = [
            (14, 0, 0, 0, 42, 0), (83, 0, 0, 0, 82, 0), (86, 0, 3, 0, 32, 0), (74, 1, 5, 0, 112, 3),
            (63, 0, 0, 0, 84, 0), (76, 0, 0, 0, 103, 0), (109, 0, 2, 0, 108, 1), (47, 0, 0, 0, 66, 0),
            (29, 0, 0, 0, 121, 7), (51, 0, 0, 0, 87, 0), (74, 0, 0, 0, 171, 0), (50, 0, 0, 0, 65, 0),
            (88, 0, 0, 0, 137, 0), (69, 0, 0, 0, 56, 0), (177, 0, 0, 0, 141, 5), (91, 0, 2, 0, 110, 1),
            (72, 0, 2, 0, 159, 11), (147, 0, 0, 0, 43, 0), (78, 0, 0, 0, 125, 9),
        ];
        let mut seen = 0;
        for (lvl, want) in expect.iter().enumerate() {
            let Some(g) = crate::test_data::gameplay(lvl as u32) else { continue };
            seen += 1;
            let v = parse_volumes(&g).unwrap_or_else(|e| panic!("level {lvl}: {e}"));
            let got = (v.cuboids.len(), v.spheres.len(), v.cylinders.len(), v.pills.len(), v.paths.len(), v.grind_paths.len());
            assert_eq!(got, *want, "level {lvl} counts");
            for (kind, shapes) in [("cuboid", &v.cuboids), ("sphere", &v.spheres), ("cylinder", &v.cylinders)] {
                for (i, s) in shapes.iter().enumerate() {
                    // The local point of centre + row k is e_k.
                    for k in 0..3 {
                        let r = s.matrix[k];
                        let c = s.centre();
                        let l = s.local([c[0] + r[0], c[1] + r[1], c[2] + r[2]]);
                        for (j, &x) in l.iter().enumerate() {
                            let e = if j == k { 1.0 } else { 0.0 };
                            assert!((x - e).abs() < 2e-3, "level {lvl} {kind} {i}: local(row {k}) = {l:?}");
                        }
                    }
                }
            }
            for (i, gp) in v.grind_paths.iter().enumerate() {
                assert!(gp.points.len() >= 2, "level {lvl} grind path {i}: {} points", gp.points.len());
                assert!(gp.flag == 0 || gp.flag == 1, "level {lvl} grind path {i}: flag {}", gp.flag);
                assert!(gp.bsphere[3] > 0.0);
            }
            assert_eq!(v.paths, crate::gameplay::parse_splines(&g).unwrap());
        }
        if seen == 0 { eprintln!("skipped: no extracted/levels"); }
    }
}
