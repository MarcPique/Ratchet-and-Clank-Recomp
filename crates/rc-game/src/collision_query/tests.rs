//! Unit tests on hand-built meshes. Geometry convention: a face is front-facing for a query
//! coming from the side `N = (v2 - v0) x (v1 - v0)` points to, i.e. the vertices run clockwise
//! seen from the front (a floor seen from above: v0, then +y, then +x).

use super::*;
use rc_formats::collision::{CollisionFace, CollisionLeafHeader, PackedCollisionVertex};

/// A cell at cell coordinates `c` holding world-space vertices `verts` (exactly representable at
/// 1/16 in x/y and 1/64 in z relative to the cell centre), triangles `tris` and quads `quads`
/// (`(indices, type byte)`).
fn cell(c: [i16; 3], verts: &[[f32; 3]], tris: &[([u8; 3], u8)], quads: &[([u8; 4], u8)]) -> CollisionCell {
    let mut cell = CollisionCell { x: c[0], y: c[1], z: c[2], ..Default::default() };
    let centre = cell.centre();
    for v in verts {
        let f = [(v[0] - centre[0]) * 16.0, (v[1] - centre[1]) * 16.0, (v[2] - centre[2]) * 64.0];
        assert!(f.iter().all(|x| x.fract() == 0.0), "vertex {v:?} not representable in cell {c:?}");
        let p = PackedCollisionVertex::pack(f[0] as i32, f[1] as i32, f[2] as i32);
        assert_eq!(p.world(centre), *v);
        cell.packed.push(p);
        cell.vertices.push(*v);
    }
    for (q, ty) in quads {
        cell.faces.push(CollisionFace { v: [q[0], q[1], q[2]], surface: *ty });
        cell.quad_v3.push(q[3]);
    }
    for (t, ty) in tris { cell.faces.push(CollisionFace { v: *t, surface: *ty }); }
    cell.header = CollisionLeafHeader { face_count: cell.faces.len() as u16, vertex_count: verts.len() as u8, quad_count: quads.len() as u8 };
    cell
}

fn mesh(mut cells: Vec<CollisionCell>) -> Collision {
    cells.sort_by_key(|c| (c.z, c.y, c.x));
    Collision { cells, ..Default::default() }
}

/// Floor triangle at height z over [8,12]^2 in cell (2,2,2): v0 (8,8), v1 (8,12), v2 (12,8).
fn floor(z: f32, ty: u8) -> Collision {
    mesh(vec![cell([2, 2, 2], &[[8.0, 8.0, z], [8.0, 12.0, z], [12.0, 8.0, z]], &[([0, 1, 2], ty)], &[])])
}

fn close(a: [f32; 3], b: [f32; 3], eps: f32) -> bool { (0..3).all(|k| (a[k] - b[k]).abs() <= eps) }

#[test]
fn line_hit_and_miss() {
    let m = floor(10.0, 0x21);
    let h = coll_line(&m, [9.0, 9.0, 11.0], [9.0, 9.0, 9.0], QueryFlags::NONE).expect("hit");
    assert_eq!(h.point, [9.0, 9.0, 10.0]);
    assert_eq!(h.kind, 0x1021);
    assert_eq!((h.surface_id(), h.sound_class()), (1, 1));
    assert_eq!(h.best, 0.5);
    assert_eq!(h.moby, None);
    assert_eq!(h.pushed_centre, None);
    // (v2-v0) x (v1-v0) in x1024 units: (4096,0,0) x (0,4096,0) = (0,0,4096^2).
    assert_eq!(h.normal, [0.0, 0.0, 16_777_216.0]);
    assert_eq!(h.tri, [[8.0, 8.0, 10.0], [8.0, 12.0, 10.0], [12.0, 8.0, 10.0]]);
    // Stops short of the floor / passes beside the triangle / leaves the world.
    assert!(coll_line(&m, [9.0, 9.0, 11.0], [9.0, 9.0, 10.5], QueryFlags::NONE).is_none());
    assert!(coll_line(&m, [11.5, 11.5, 11.0], [11.5, 11.5, 9.0], QueryFlags::NONE).is_none());
    assert!(coll_line(&m, [9.0, 9.0, 11.0], [9.0, 9.0, -1.0], QueryFlags::NONE).is_none());
    assert!(coll_line(&m, [9.0, 9.0, 11.0], [9.0, 9.0, 1024.0], QueryFlags::NONE).is_none());
    // SKIP_WORLD leaves only the (unported) moby pass.
    assert!(coll_line(&m, [9.0, 9.0, 11.0], [9.0, 9.0, 9.0], QueryFlags::SKIP_WORLD).is_none());
    // Zero length (same x1024 integer point) never hits.
    assert!(coll_line(&m, [9.0, 9.0, 10.0], [9.0, 9.0, 10.0], QueryFlags::TWO_SIDED).is_none());
    assert!(cells_for_line([9.0, 9.0, 10.0], [9.0, 9.0, 10.0002]).is_none());
}

#[test]
fn one_sided_and_two_sided() {
    let m = floor(10.0, 0x1f);
    assert!(coll_line(&m, [9.0, 9.0, 9.0], [9.0, 9.0, 11.0], QueryFlags::NONE).is_none());
    let h = coll_line(&m, [9.0, 9.0, 9.0], [9.0, 9.0, 11.0], QueryFlags::TWO_SIDED).expect("two-sided hit");
    assert_eq!(h.point, [9.0, 9.0, 10.0]);
    assert_eq!(h.surface_id(), -1);
    // Sphere below the floor: back side.
    assert!(coll_sphere(&m, [9.0, 9.0, 9.6], 0.5, QueryFlags::NONE).is_none());
    assert!(coll_sphere(&m, [9.0, 9.0, 9.6], 0.5, QueryFlags::TWO_SIDED).is_some());
}

#[test]
fn exclusion_flags() {
    let down = |m: &Collision, f| coll_line(m, [9.0, 9.0, 11.0], [9.0, 9.0, 9.0], f);
    let m = floor(10.0, 0x4c);
    assert!(down(&m, QueryFlags::exclude_surface(0xc)).is_none());
    assert!(down(&m, QueryFlags::exclude_surface(0xd)).is_some());
    assert!(down(&m, QueryFlags(0xd24)).is_some()); // the hero's "exclude 0xd" flags
    assert!(down(&m, QueryFlags::EXCLUDE_HIGH_BIT).is_some());
    let m = floor(10.0, 0x80);
    assert!(down(&m, QueryFlags::EXCLUDE_HIGH_BIT).is_none());
    assert!(down(&m, QueryFlags(0x24)).is_none()); // surface id 0 excluded by the hero flags
    assert!(down(&m, QueryFlags::NONE).is_some());
    assert!(coll_sphere(&m, [9.0, 9.0, 10.3], 0.5, QueryFlags(0x24)).is_none());
    assert!(coll_sphere(&m, [9.0, 9.0, 10.3], 0.5, QueryFlags::NONE).is_some());
}

#[test]
fn quad_split_is_v0v1v2_then_v0v2v3() {
    // Quad over [8,12]^2 at z 10: v0 (8,8), v1 (8,12), v2 (12,12), v3 (12,8).
    let m = mesh(vec![cell(
        [2, 2, 2],
        &[[8.0, 8.0, 10.0], [8.0, 12.0, 10.0], [12.0, 12.0, 10.0], [12.0, 8.0, 10.0]],
        &[],
        &[([0, 1, 2, 3], 0x05)],
    )]);
    // Upper-left half (x < y) is (v0, v1, v2).
    let h = coll_line(&m, [9.0, 11.0, 11.0], [9.0, 11.0, 9.0], QueryFlags::NONE).unwrap();
    assert_eq!(h.tri, [[8.0, 8.0, 10.0], [8.0, 12.0, 10.0], [12.0, 12.0, 10.0]]);
    // Lower-right half is (v0, v2, v3), same type byte.
    let h = coll_line(&m, [11.0, 9.0, 11.0], [11.0, 9.0, 9.0], QueryFlags::NONE).unwrap();
    assert_eq!(h.tri, [[8.0, 8.0, 10.0], [12.0, 12.0, 10.0], [12.0, 8.0, 10.0]]);
    assert_eq!(h.kind, 0x1005);
}

#[test]
fn edge_tests_are_inclusive() {
    let m = floor(10.0, 1);
    // On the hypotenuse v1-v2 (x + y = 20), on the edge v0-v1 (x = 8), and on vertex v0.
    for p in [[10.0, 10.0], [8.0, 9.0], [8.0, 8.0]] {
        let h = coll_line(&m, [p[0], p[1], 11.0], [p[0], p[1], 9.0], QueryFlags::NONE);
        assert_eq!(h.map(|h| h.point), Some([p[0], p[1], 10.0]), "{p:?}");
    }
    // Just outside the hypotenuse.
    assert!(coll_line(&m, [10.0, 10.001, 11.0], [10.0, 10.001, 9.0], QueryFlags::NONE).is_none());
}

#[test]
fn nearest_hit_is_strictly_closer_across_cells() {
    // The same slope z = 15.25 - x/2 over x 8..16 stored in cells (2,2,2) type 1 and (3,2,2)
    // type 2. The line enters (2,2,2) first and finds the hit there although it lies at x 13
    // (t 0.75, the slope overlaps the cell's sub-segment box); (3,2,2), entered at t 0.5, is still
    // visited, but its copy is at the same t, so it is not strictly closer and loses.
    let tri = [[8.0, 6.0, 11.25], [8.0, 14.0, 11.25], [16.0, 6.0, 7.25]];
    let m = mesh(vec![cell([2, 2, 2], &tri, &[([0, 1, 2], 1)], &[]), cell([3, 2, 2], &tri, &[([0, 1, 2], 2)], &[])]);
    let (a, b) = ([10.0, 9.0, 11.0], [14.0, 9.0, 8.0]);
    let cells = cells_for_line(a, b).unwrap();
    assert_eq!(cells.iter().map(|c| c.cell).collect::<Vec<_>>(), vec![[2, 2, 2], [3, 2, 2]]);
    let h = coll_line(&m, a, b, QueryFlags::NONE).unwrap();
    assert_eq!((h.kind, h.best), (0x1001, 0.75));
    assert!(close(h.point, [13.0, 9.0, 8.75], 1e-4), "{:?}", h.point);
    // Raise the second cell's copy by 1/64: now strictly closer, it wins.
    let tri2 = tri.map(|v| [v[0], v[1], v[2] + 1.0 / 64.0]);
    let m = mesh(vec![cell([2, 2, 2], &tri, &[([0, 1, 2], 1)], &[]), cell([3, 2, 2], &tri2, &[([0, 1, 2], 2)], &[])]);
    let h = coll_line(&m, a, b, QueryFlags::NONE).unwrap();
    assert_eq!(h.kind, 0x1002);
    assert!(h.best < 0.75);
}

#[test]
fn walk_stops_once_the_next_cell_starts_behind_the_hit() {
    // Line (9,9,11.5) -> (15,9,8.5) enters cell (3,2,2) at t 0.5. Cell (2,2,2) has a floor at
    // z 10.75 hit at t 0.25. Cell (3,2,2) stores a slope z = 21 - x reaching back over x 8..16,
    // which overlaps that cell's sub-segment box (so it is not culled) and which the line crosses
    // earlier, at x 10 (t 1/6); but the walk ends before visiting (3,2,2).
    let (a, b) = ([9.0, 9.0, 11.5], [15.0, 9.0, 8.5]);
    let low = [[8.0, 8.0, 10.75], [8.0, 12.0, 10.75], [12.0, 8.0, 10.75]];
    let slope = [[8.0, 6.0, 13.0], [8.0, 14.0, 13.0], [16.0, 6.0, 5.0]];
    let f = QueryFlags::TWO_SIDED;
    let m = mesh(vec![cell([2, 2, 2], &low, &[([0, 1, 2], 1)], &[]), cell([3, 2, 2], &slope, &[([0, 1, 2], 2)], &[])]);
    let h = coll_line(&m, a, b, f).unwrap();
    assert_eq!((h.kind, h.best), (0x1001, 0.25));
    // Without the low floor the slope is found from cell (3,2,2), behind that cell's entry point.
    let m = mesh(vec![cell([3, 2, 2], &slope, &[([0, 1, 2], 2)], &[])]);
    let h = coll_line(&m, a, b, f).unwrap();
    assert_eq!(h.kind, 0x1002);
    assert!((h.best - 1.0 / 6.0).abs() < 1e-6 && close(h.point, [10.0, 9.0, 11.0], 1e-4), "{h:?}");
    // A face whose box misses the cell's sub-segment box is culled by the outcodes even though
    // the infinite segment would hit it: the flat floor z 11 over x 8..16 stored in (3,2,2).
    let high = [[8.0, 6.0, 11.0], [8.0, 14.0, 11.0], [16.0, 6.0, 11.0]];
    let m = mesh(vec![cell([3, 2, 2], &high, &[([0, 1, 2], 2)], &[])]);
    assert!(coll_line(&m, a, b, f).is_none());
    let m = mesh(vec![cell([2, 2, 2], &high, &[([0, 1, 2], 2)], &[])]);
    assert!(coll_line(&m, a, b, f).is_some());
}

#[test]
fn cell_walk_order_on_a_diagonal() {
    let cells = |a, b| cells_for_line(a, b).unwrap().into_iter().map(|c| (c.t_enter, c.cell)).collect::<Vec<_>>();
    // x and y cross their planes at the same t: x first, then y.
    assert_eq!(
        cells([1.0, 1.0, 1.0], [9.0, 9.0, 1.0]),
        vec![(0.0, [0, 0, 0]), (0.375, [1, 0, 0]), (0.375, [1, 1, 0]), (0.875, [2, 1, 0]), (0.875, [2, 2, 0])]
    );
    // Three-way tie through the corner (4,4,4), and the ties go x, y, z.
    assert_eq!(
        cells([3.0, 3.0, 3.0], [5.0, 5.0, 5.0]),
        vec![(0.0, [0, 0, 0]), (0.5, [1, 0, 0]), (0.5, [1, 1, 0]), (0.5, [1, 1, 1])]
    );
    // Negative direction: planes 8 then 4 on x, 8 on y.
    assert_eq!(
        cells([9.0, 9.0, 1.0], [1.0, 7.0, 1.0]),
        vec![(0.0, [2, 2, 0]), (0.125, [1, 2, 0]), (0.5, [1, 1, 0]), (0.625, [0, 1, 0])]
    );
    // Ending exactly on a plane: that crossing is t = 1.0, which ends the walk.
    assert_eq!(cells([2.0, 1.0, 1.0], [4.0, 1.0, 1.0]), vec![(0.0, [0, 0, 0])]);
    // ... unless the truncating reciprocal lands just below 1.0: 1/3072 rounds toward zero, so
    // (4096 - 1024) * (1/3072) = 0.99999994 and the end cell is visited.
    assert_eq!(cells([1.0, 1.0, 1.0], [4.0, 1.0, 1.0]), vec![(0.0, [0, 0, 0]), (0.99999994, [1, 0, 0])]);
    // Outside the world.
    assert!(cells_for_line([1.0, 1.0, 1.0], [-1.0, 1.0, 1.0]).is_none());
}

#[test]
fn sphere_hit_and_push_out() {
    let m = floor(10.0, 3);
    let h = coll_sphere(&m, [9.0, 9.0, 10.5], 1.0, QueryFlags::NONE).expect("hit");
    assert_eq!(h.point, [9.0, 9.0, 10.0]);
    assert_eq!(h.kind, 0x1003);
    // best = 0.99951 * (0.5*1024)^2, pushed = hit + (c - hit) * r / sqrt(best).
    let k = f32::from_bits(BEST_SHRINK);
    assert!((h.best - k * 512.0 * 512.0).abs() < 0.1, "{}", h.best);
    let pushed = h.pushed_centre.unwrap();
    assert!(close(pushed, [9.0, 9.0, 10.0 + 1.0 / k.sqrt()], 1e-5), "{pushed:?}");
    assert!(pushed[2] > 11.0); // slightly past touching: sqrt(0.99951) skin
    // Out of reach.
    assert!(coll_sphere(&m, [9.0, 9.0, 10.5], 0.49, QueryFlags::NONE).is_none());
    // Beside the hypotenuse (x + y = 20): closest point is on the edge, push-out is diagonal.
    let h = coll_sphere(&m, [10.5, 10.5, 10.0], 1.0, QueryFlags::TWO_SIDED).expect("edge hit");
    assert_eq!(h.point, [10.0, 10.0, 10.0]);
    let p = h.pushed_centre.unwrap();
    let s = 1.0 / (2.0f32).sqrt() / k.sqrt();
    assert!(close(p, [10.0 + s, 10.0 + s, 10.0], 1e-4), "{p:?}");
    // Invalid radius / outside the world.
    assert!(coll_sphere(&m, [9.0, 9.0, 10.5], 0.0, QueryFlags::NONE).is_none());
    assert!(coll_sphere(&m, [0.5, 9.0, 10.5], 1.0, QueryFlags::NONE).is_none());
}

#[test]
fn sphere_takes_the_closest_face() {
    // Two floors in one cell: z 10 (type 1) and z 10.25 (type 2); the sphere at 10.5 touches both.
    let m = mesh(vec![cell(
        [2, 2, 2],
        &[[8.0, 8.0, 10.0], [8.0, 12.0, 10.0], [12.0, 8.0, 10.0], [8.0, 8.0, 10.25], [8.0, 12.0, 10.25], [12.0, 8.0, 10.25]],
        &[([0, 1, 2], 1), ([3, 4, 5], 2)],
        &[],
    )]);
    let h = coll_sphere(&m, [9.0, 9.0, 10.5], 1.0, QueryFlags::NONE).unwrap();
    assert_eq!((h.kind, h.point), (0x1002, [9.0, 9.0, 10.25]));
    let cells = cells_for_sphere([9.0, 9.0, 10.5], 1.0).unwrap();
    assert_eq!(cells, vec![[2, 2, 2]]);
    // A sphere straddling a cell corner gathers the cells in z, y, x order.
    assert_eq!(cells_for_sphere([8.0, 8.0, 8.0], 0.5).unwrap().len(), 8);
    assert_eq!(cells_for_sphere([8.0, 8.0, 8.0], 0.5).unwrap()[1], [2, 1, 1]);
}

#[test]
fn capsule_floor_wall_and_height() {
    let m = floor(10.0, 1);
    // Base 0.3 above the floor, radius 0.5: pushed up to touch.
    let h = coll_capsule(&m, [9.0, 9.0, 10.3], 1.5, 0.5, QueryFlags::NONE).expect("floor");
    assert_eq!(h.point, [9.0, 9.0, 10.0]);
    let k = f32::from_bits(BEST_SHRINK);
    assert!(close(h.pushed_centre.unwrap(), [9.0, 9.0, 10.0 + 0.5 / k.sqrt()], 1e-5), "{:?}", h.pushed_centre);

    // Wall: plane x = 11 spanning y 8..12, z 8..12, facing -x (toward the capsule).
    let wall = mesh(vec![cell([2, 2, 2], &[[11.0, 8.0, 8.0], [11.0, 12.0, 8.0], [11.0, 8.0, 12.0]], &[([0, 1, 2], 4)], &[])]);
    let h = coll_capsule(&wall, [10.6, 9.0, 8.5], 2.0, 0.5, QueryFlags::NONE).expect("wall");
    assert!(close(h.point, [11.0, 9.0, 8.5], 1e-5), "{:?}", h.point);
    let p = h.pushed_centre.unwrap();
    assert!(close(p, [11.0 - 0.5 / k.sqrt(), 9.0, 8.5], 1e-4), "{p:?}");
    // From behind the wall: back side.
    assert!(coll_capsule(&wall, [11.4, 9.0, 8.5], 2.0, 0.5, QueryFlags::NONE).is_none());

    // Height reaches a ceiling stored only in cell (2,2,3), but cell gathering tests only the base
    // sphere (radius 0.5 around z 9.0), so that cell is never visited.
    let ceiling = mesh(vec![cell([2, 2, 3], &[[8.0, 8.0, 12.5], [12.0, 8.0, 12.5], [8.0, 12.0, 12.5]], &[([0, 1, 2], 1)], &[])]);
    assert!(!cells_for_capsule([9.0, 9.0, 9.0], 3.3, 0.5).unwrap().contains(&[2, 2, 3]));
    assert!(coll_capsule(&ceiling, [9.0, 9.0, 9.0], 3.3, 0.5, QueryFlags::NONE).is_none());
    // The same ceiling is found once the base sphere reaches into that cell.
    let h = coll_capsule(&ceiling, [9.0, 9.0, 11.8], 0.5, 0.5, QueryFlags::NONE).expect("ceiling");
    assert!(close(h.point, [9.0, 9.0, 12.5], 1e-5), "{:?}", h.point);
}

// ---------------------------------------------------------------------------------------------------
// Mobys

use crate::moby_runtime::{ClassInfo, Moby, MobyTable};
use rc_formats::collision::{HeroGroup, HeroGroupHeader, HeroTriangle, HeroVertex};
use rc_formats::moby_collision::{MobyCollPrim, MobyCollision};
use std::collections::HashMap;
use std::sync::Mutex;

/// Mobys from a table, with fixed joint positions per moby.
struct Fixed<'a> {
    table: &'a MobyTable,
    joints: HashMap<usize, Vec<[u32; 4]>>,
}

impl MobySource for Fixed<'_> {
    fn moby(&self, id: usize) -> Option<CollMoby> { self.table.mobys.get(id).map(CollMoby::of) }
    fn joints(&self, id: usize, count: usize) -> Vec<[u32; 4]> {
        let mut j = self.joints.get(&id).cloned().unwrap_or_default();
        j.resize(count, [0; 4]);
        j
    }
}

/// A moby of class `oc` at `pos` (world), identity rows, scale 1, bounding sphere radius `r` (world).
fn moby(index: u32, oc: i16, pos: [f32; 3], r: f32) -> Moby {
    let info = ClassInfo { has_collision: true, scale: 1.0, ..Default::default() };
    let mut m = Moby::init_instance(index, oc, Some(&info));
    m.position = [pos[0], pos[1], pos[2], 0.0];
    m.rows = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0; 4]];
    m.bsphere = [pos[0] * 1024.0, pos[1] * 1024.0, pos[2] * 1024.0, r * 1024.0];
    m
}

fn prim(kind: u8, mask: u16, q0: [f32; 4], q1: [f32; 4]) -> MobyCollPrim {
    let mut raw = [0u32; 8];
    for k in 0..4 { raw[k] = q0[k].to_bits(); raw[4 + k] = q1[k].to_bits(); }
    raw[0] = kind as u32 | 4 << 8 | (mask as u32) << 16;
    MobyCollPrim { raw }
}

/// A crate-like box: 1×1 footprint about the origin, z 0..1 (model units ×1024), top face and the +x side.
fn box_blob() -> MobyCollision {
    let (h, t) = (512i16, 1024i16);
    MobyCollision {
        joint_counts: [0, 0],
        prims: vec![],
        vertices: vec![[-h, -h, t, 0], [-h, h, t, 0], [h, -h, t, 0], [h, h, t, 0], [h, -h, 0, 0], [h, h, 0, 0], [h, -h, t, 0], [h, h, t, 0]],
        faces: vec![[0, 1, 2, 0x21], [2, 1, 3, 0x21], [4, 6, 5, 0x22], [5, 6, 7, 0x22]],
    }
}

struct World {
    table: MobyTable,
    grid: MobyGrid,
    classes: HashMap<i16, MobyCollision>,
    cache: Mutex<PoseCache>,
    joints: HashMap<usize, Vec<[u32; 4]>>,
}

impl World {
    /// Moby 0: box (class 500) at (9, 9, 4); moby 1: a 0.25 sphere primitive 2 above (40, 40, 20) (class 501);
    /// moby 2: a joint capsule (class 502) at (70, 70, 20) between joints 0 (0,0,1) and 1 (0,0,3), radius 0.25.
    fn new() -> World {
        let mut classes = HashMap::new();
        classes.insert(500, box_blob());
        classes.insert(501, MobyCollision { prims: vec![prim(1, 0x8003, [0.0; 4], [0.0, 0.0, 2048.0, 256.0])], ..Default::default() });
        let mut cap = prim(4, 0x8002, [0.0, 0.0, 0.0, 256.0], [0.0; 4]);
        cap.raw[1] = 1 << 16; // joints 0 and 1
        classes.insert(502, MobyCollision { joint_counts: [0, 2], prims: vec![cap], ..Default::default() });
        let mut table = MobyTable::new(vec![moby(0, 500, [9.0, 9.0, 4.0], 1.5), moby(1, 501, [40.0, 40.0, 20.0], 2.5), moby(2, 502, [70.0, 70.0, 20.0], 3.5)], 0);
        let grid = MobyGrid::build(&mut table);
        let mut joints = HashMap::new();
        joints.insert(2, vec![[0, 0, 1024.0f32.to_bits(), 0], [0, 0, 3072.0f32.to_bits(), 0]]);
        World { table, grid, classes, cache: Mutex::new(PoseCache::default()), joints }
    }
}

macro_rules! scene {
    ($w:expr, $src:ident, $sc:ident) => {
        let $src = Fixed { table: &$w.table, joints: $w.joints.clone() };
        let $sc = MobyScene { mobys: &$src, grid: &$w.grid, classes: &$w.classes, cache: &$w.cache };
    };
}

#[test]
fn moby_grid_registration() {
    let w = World::new();
    // The box's bounding sphere (9,9) r 1.5 spans x/y 7.5..10.5: cell (0, 0) only.
    assert_eq!(w.grid.cells_of(0), [(0, 0)]);
    assert_eq!(w.table.mobys[0].ba0, [0, 0, 0, 0]);
    // (40, 40) r 2.5: 37.5..42.5 → cells 2 only.
    assert_eq!(w.grid.cells_of(1), [(2, 2)]);
    // A moby straddling x = 16: two cells, appended in x order.
    let mut g = MobyGrid::new();
    let mut a = moby(5, 500, [16.0, 8.0, 0.0], 1.0);
    g.register(&mut a);
    assert_eq!(g.cells_of(5), [(0, 0), (1, 0)]);
    assert_eq!(MobyGrid::rect_for(a.bsphere.map(f32::to_bits)), 0x0001_0000);
    let mut b = moby(6, 500, [8.0, 8.0, 0.0], 1.0);
    let mut c = moby(7, 500, [9.0, 8.0, 0.0], 1.0);
    g.register(&mut b);
    g.register(&mut c);
    assert_eq!(g.cell(0, 0), [5, 6, 7]);
    // Moving a out of cell (0,0): its entry is replaced by the cell's last one; it stays in (1,0).
    a.bsphere[0] = 24.0 * 1024.0;
    g.register(&mut a);
    assert_eq!((g.cell(0, 0), g.cell(1, 0)), (&[7u16, 6][..], &[5u16][..]));
    // Off the grid (x0 < 0): the old registration stays.
    b.bsphere[0] = -1024.0;
    g.register(&mut b);
    assert_eq!(g.cells_of(6), [(0, 0)]);
    // DeleteMoby.
    g.remove(&mut c);
    assert_eq!((g.cell(0, 0), c.ba0), (&[6u16][..], GRID_NONE.to_le_bytes()));
    // No blob (+0x94 = 0): never registered.
    let mut d = moby(8, 500, [8.0, 8.0, 0.0], 1.0);
    d.has_collision = false;
    g.register(&mut d);
    assert!(g.cells_of(8).is_empty());
}

use super::mobys::GRID_NONE;

#[test]
fn moby_mesh_line_hits_top_face_after_the_world() {
    let w = World::new();
    scene!(w, src, sc);
    let none = Collision::default();
    let h = coll_line_m(&none, Some(&sc), [9.2, 9.1, 7.0], [9.2, 9.1, 3.0], QueryFlags::NONE, None).expect("box top");
    assert_eq!((h.moby, h.primitive, h.kind), (Some(0), None, 0x1021));
    assert_eq!(h.point, [9.2, 9.1, 5.0]);
    assert_eq!(h.tri[0], [9.5, 8.5, 5.0]); // the second top triangle (2, 1, 3)
    assert!(coll_line_m(&none, None, [9.2, 9.1, 7.0], [9.2, 9.1, 3.0], QueryFlags::NONE, None).is_none());
    assert!(coll_line_m(&none, Some(&sc), [9.2, 9.1, 7.0], [9.2, 9.1, 3.0], QueryFlags::NONE, Some(0)).is_none(), "ignored");
    // Flag 0x1 (and 0x8) restrict the line's moby pass to mode 0x4000.
    assert!(coll_line_m(&none, Some(&sc), [9.2, 9.1, 7.0], [9.2, 9.1, 3.0], QueryFlags(0x9), None).is_none());
    // The world floor at z 10 over [8,12]^2 is nearer from above; from below it the box wins.
    let m = floor(10.0, 3);
    let h = coll_line_m(&m, Some(&sc), [9.2, 9.1, 11.0], [9.2, 9.1, 3.0], QueryFlags::NONE, None).unwrap();
    assert_eq!((h.moby, h.kind, h.point[2]), (None, 0x1003, 10.0));
    let h = coll_line_m(&m, Some(&sc), [9.2, 9.1, 9.0], [9.2, 9.1, 3.0], QueryFlags::NONE, None).unwrap();
    assert_eq!((h.moby, h.point[2]), (Some(0), 5.0));
    // One-sided: from inside the box upward nothing.
    assert!(coll_line_m(&none, Some(&sc), [9.2, 9.1, 4.5], [9.2, 9.1, 6.0], QueryFlags::NONE, None).is_none());
}

#[test]
fn moby_mesh_blocks_sphere_and_capsule() {
    let w = World::new();
    scene!(w, src, sc);
    let none = Collision::default();
    let k = f32::from_bits(BEST_SHRINK);
    // Capsule beside the +x side (x = 9.5): pushed out along +x.
    let h = coll_capsule_m(&none, Some(&sc), [9.7, 9.0, 4.2], 0.5, 0.3, QueryFlags(0x24), None).expect("side");
    assert_eq!((h.moby, h.kind), (Some(0), 0x1022));
    assert!(close(h.point, [9.5, 9.0, 4.2], 1e-5), "{:?}", h.point);
    assert!(close(h.pushed_centre.unwrap(), [9.5 + 0.3 / k.sqrt(), 9.0, 4.2], 1e-4), "{:?}", h.pushed_centre);
    // Standing on the top: a sphere 0.2 above it.
    let h = coll_sphere_m(&none, Some(&sc), [9.0, 9.0, 5.2], 0.5, QueryFlags::NONE, None).expect("top");
    assert!(close(h.point, [9.0, 9.0, 5.0], 1e-5) && h.moby == Some(0));
    assert!(coll_sphere_m(&none, Some(&sc), [9.0, 9.0, 5.2], 0.5, QueryFlags::NONE, Some(0)).is_none());
}

#[test]
fn moby_sphere_primitive_line_sphere_capsule() {
    let w = World::new();
    scene!(w, src, sc);
    let none = Collision::default();
    // Line down through the primitive (centre (40,40,22), r 0.25): enters at z 22.25.
    let h = coll_line_m(&none, Some(&sc), [40.0, 40.0, 25.0], [40.0, 40.0, 19.0], QueryFlags::NONE, None).expect("prim");
    assert_eq!((h.moby, h.primitive, h.kind, h.surface_id()), (Some(1), Some(0), -0x10, -1));
    assert!(close(h.point, [40.0, 40.0, 22.25], 1e-5), "{:?}", h.point);
    assert!(close(h.normal, [0.0, 0.0, 256.0], 1e-2), "{:?}", h.normal);
    assert!((h.best - 2.75 / 6.0).abs() < 1e-6, "{}", h.best);
    // Flag 0x2 skips primitives; a line passing beside misses.
    assert!(coll_line_m(&none, Some(&sc), [40.0, 40.0, 25.0], [40.0, 40.0, 19.0], QueryFlags(0x2), None).is_none());
    assert!(coll_line_m(&none, Some(&sc), [40.3, 40.0, 25.0], [40.3, 40.0, 19.0], QueryFlags::NONE, None).is_none());
    // Sphere 0.7 above the centre, r 0.5: closest point of the primitive (40,40,22.25), best = d^2 unshrunk.
    let h = coll_sphere_m(&none, Some(&sc), [40.0, 40.0, 22.7], 0.5, QueryFlags::NONE, None).expect("sphere");
    assert_eq!((h.moby, h.kind), (Some(1), -0x10));
    assert!(close(h.point, [40.0, 40.0, 22.25], 1e-5), "{:?}", h.point);
    assert!((h.best - (0.45f32 * 1024.0).powi(2)).abs() < 2.0, "{}", h.best);
    assert!(close(h.pushed_centre.unwrap(), [40.0, 40.0, 22.75], 1e-4), "{:?}", h.pushed_centre);
    assert!(coll_sphere_m(&none, Some(&sc), [40.0, 40.0, 22.7], 0.4, QueryFlags::NONE, None).is_none());
    // Capsule base 0.4 above the centre, radius 0.2: the axis point is the base.
    let h = coll_capsule_m(&none, Some(&sc), [40.0, 40.0, 22.4], 1.0, 0.2, QueryFlags(0x24), None).expect("capsule");
    assert!(close(h.pushed_centre.unwrap(), [40.0, 40.0, 22.45], 1e-4), "{:?}", h.pushed_centre);
    // Mask: a flag-0x4 query reads bit 0 (set on this primitive); the joint capsule has bit 1 only.
    assert!(coll_line_m(&none, Some(&sc), [70.0, 70.0, 25.0], [70.0, 70.0, 19.0], QueryFlags(0x4), None).is_none());
    // coll_sphere_mobys: every moby touched, once.
    assert_eq!(coll_sphere_mobys(&sc, [40.0, 40.0, 22.7], 0.5, QueryFlags::NONE, None), [1]);
    assert!(coll_sphere_mobys(&sc, [40.0, 40.0, 22.7], 0.4, QueryFlags::NONE, None).is_empty());
    assert_eq!(coll_sphere_mobys(&sc, [9.0, 9.0, 5.2], 0.5, QueryFlags(0x2), None), [0]);
}

#[test]
fn moby_joint_capsule_and_pose_cache() {
    let w = World::new();
    scene!(w, src, sc);
    let none = Collision::default();
    // Capsule between (70,70,21) and (70,70,23), r 0.25: a horizontal line at z 22 enters at x 69.75.
    let h = coll_line_m(&none, Some(&sc), [68.0, 70.0, 22.0], [72.0, 70.0, 22.0], QueryFlags::NONE, None).expect("capsule");
    assert_eq!((h.moby, h.primitive), (Some(2), Some(0)));
    assert!(close(h.point, [69.75, 70.0, 22.0], 1e-4), "{:?}", h.point);
    // Down the axis: enters the top cap at 23.25.
    let h = coll_line_m(&none, Some(&sc), [70.0, 70.0, 25.0], [70.0, 70.0, 19.0], QueryFlags::NONE, None).expect("cap");
    assert!(close(h.point, [70.0, 70.0, 23.25], 1e-4), "{:?}", h.point);
    let h = coll_sphere_m(&none, Some(&sc), [70.6, 70.0, 22.0], 0.5, QueryFlags::NONE, None).expect("side");
    assert!(close(h.point, [70.25, 70.0, 22.0], 1e-4), "{:?}", h.point);
    // The pose came from the cache's first entry (key = moby+0xa8), filled once.
    let c = w.cache.lock().unwrap();
    assert_eq!((c.keys[0], c.next), (w.table.mobys[2].uid_hi, 1));
    drop(c);
    let mut pc = PoseCache::default();
    for k in 1..=9u32 { pc.get(k, 1, |_| vec![[k, 0, 0, 0]]); }
    assert_eq!((pc.keys, pc.next), ([9, 2, 3, 4, 5, 6, 7, 8], 1));
    assert_eq!(pc.get(5, 1, |_| unreachable!())[0], [5, 0, 0, 0]);
}

#[test]
fn hero_groups_sphere() {
    // One group: a floor triangle at z 10 over (8,8)-(8,12)-(12,8), bounding sphere (10,10,10) r 3.
    let v = |x: f32, y: f32, z: f32| HeroVertex { xyz: [(x * 64.0) as u16, (y * 64.0) as u16, (z * 64.0) as u16], pad: 0 };
    let g = HeroGroup {
        header: HeroGroupHeader { sphere: [640, 640, 640, 192], triangle_count: 1, vertex_count: 3, data: 0 },
        vertices: vec![v(8.0, 8.0, 10.0), v(8.0, 12.0, 10.0), v(12.0, 8.0, 10.0)],
        triangles: vec![HeroTriangle { v: [0, 1, 2], pad: 0 }],
    };
    let m = Collision { hero_groups: vec![g], hero_group_count: 1, ..Default::default() };
    let h = coll_sphere_hero_groups(&m, [9.0, 9.0, 10.3], 0.5).expect("hit");
    assert_eq!((h.moby, h.kind), (None, 0x1000));
    assert!(close(h.point, [9.0, 9.0, 10.0], 1e-5), "{:?}", h.point);
    let k = f32::from_bits(BEST_SHRINK);
    assert!(close(h.pushed_centre.unwrap(), [9.0, 9.0, 10.0 + 0.5 / k.sqrt()], 1e-4), "{:?}", h.pushed_centre);
    assert!(coll_sphere_hero_groups(&m, [9.0, 9.0, 10.6], 0.5).is_none(), "out of reach");
    assert!(coll_sphere_hero_groups(&m, [9.0, 9.0, 9.7], 0.5).is_none(), "back side");
    assert!(coll_sphere_hero_groups(&m, [30.0, 30.0, 10.3], 0.5).is_none(), "culled by the bounding sphere");
}
