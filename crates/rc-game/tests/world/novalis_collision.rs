//! Disc-data checks of the collision queries on Novalis (level 1). Skipped when `extracted/`
//! (the extracted game data, never shipped with the repo) is absent.

use rc_formats::{collision, gameplay, level, tfrag};
use rc_game::collision_query::{cells_for_line, coll_capsule, coll_line, coll_sphere, CollOutput, QueryFlags};
use std::collections::BTreeMap;

struct Novalis {
    mesh: collision::Collision,
    mobys: Vec<gameplay::MobyInstance>,
    /// Tfrag vertex bounding box (world units).
    lo: [f32; 3],
    hi: [f32; 3],
}

fn novalis() -> Option<Novalis> {
    let dir = rc_formats::test_data::root().join("levels/01");
    let data = rc_formats::test_data::core_data(1)?;
    let idx = std::fs::read(dir.join("core_index.bin")).ok()?;
    let gp = rc_formats::test_data::gameplay(1)?;
    let core = level::parse_level_core(&idx, data.len()).unwrap();
    let mesh = collision::parse_collision(&core, &data).unwrap();
    let tfrags = tfrag::parse_level_tfrags(&core, &data).unwrap();
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for t in &tfrags {
        for i in 0..t.lod_vertex_info_count(0) as usize {
            let p = t.world_position(i);
            for k in 0..3 { lo[k] = lo[k].min(p[k]); hi[k] = hi[k].max(p[k]); }
        }
    }
    Some(Novalis { mesh, mobys: gameplay::parse_moby_instances(&gp).unwrap(), lo, hi })
}

/// Deterministic xorshift32 in [0, 1).
struct Rng(u32);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }
}

fn clamp_world(z: f32) -> f32 { z.clamp(0.001, 1023.99) }

fn histogram(hits: &[CollOutput]) -> BTreeMap<u8, usize> {
    let mut h = BTreeMap::new();
    for o in hits { *h.entry(o.kind as u8).or_insert(0) += 1; }
    h
}

#[test]
fn novalis_vertical_rays_find_ground() {
    let Some(n) = novalis() else { eprintln!("skipped: no extracted/"); return; };
    let long_leaves = n.mesh.cells.iter().filter(|c| c.header.face_count > 255).count();
    eprintln!("tfrag bbox {:?} .. {:?}; {} cells ({} with > 255 faces)", n.lo, n.hi, n.mesh.cells.len(), long_leaves);

    // 1000 seeded points over the tfrag box, cast from above its top to below its bottom.
    let mut rng = Rng(0x2002_1104);
    let (top, bottom) = (clamp_world(n.hi[2] + 1.0), clamp_world(n.lo[2] - 1.0));
    let mut hits = Vec::new();
    for _ in 0..1000 {
        let x = n.lo[0] + rng.next() * (n.hi[0] - n.lo[0]);
        let y = n.lo[1] + rng.next() * (n.hi[1] - n.lo[1]);
        if let Some(h) = coll_line(&n.mesh, [x, y, top], [x, y, bottom], QueryFlags::NONE) {
            assert!(h.point[2] <= top && h.point[2] >= bottom);
            assert!(h.normal[2] > 0.0, "one-sided downward ray must hit an upward-facing face");
            hits.push(h);
        }
    }
    eprintln!("random rays: {}/1000 hit ({:.1} %); type histogram {:x?}", hits.len(), hits.len() as f32 / 10.0, histogram(&hits));
    assert!(!hits.is_empty());

    // Every moby instance: from 1 unit above its position down 64 units.
    let mut moby_hits = Vec::new();
    let mut misses = Vec::new();
    let mut drops = Vec::new();
    for (i, m) in n.mobys.iter().enumerate() {
        let p = m.position;
        let a = [p[0], p[1], clamp_world(p[2] + 1.0)];
        let b = [p[0], p[1], clamp_world(p[2] - 64.0)];
        match coll_line(&n.mesh, a, b, QueryFlags::NONE) {
            Some(h) => { drops.push(p[2] - h.point[2]); moby_hits.push(h); }
            None => misses.push((i, m.o_class, p)),
        }
    }
    let rate = moby_hits.len() as f32 / n.mobys.len() as f32;
    drops.sort_by(f32::total_cmp);
    eprintln!(
        "moby rays: {}/{} hit ({:.1} %); drop below position: median {:.3}, 90th pct {:.3}; type histogram {:x?}",
        moby_hits.len(), n.mobys.len(), rate * 100.0, drops[drops.len() / 2], drops[drops.len() * 9 / 10], histogram(&moby_hits)
    );
    eprintln!("misses (index, class, position): {misses:?}");
    assert!(rate > 0.95, "moby ground hit rate {rate}");

    // Ratchet: the instance of moby class 0 (the hero class); report the others for reference.
    let class0: Vec<usize> = (0..n.mobys.len()).filter(|&i| n.mobys[i].o_class == 0).collect();
    eprintln!("class-0 instances: {:?}", class0.iter().map(|&i| (i, n.mobys[i].position)).collect::<Vec<_>>());
    let ratchet = class0.first().copied().unwrap_or(0);
    let p = n.mobys[ratchet].position;
    let h = coll_line(&n.mesh, [p[0], p[1], clamp_world(p[2] + 1.0)], [p[0], p[1], clamp_world(p[2] - 64.0)], QueryFlags::NONE)
        .expect("Ratchet stands on ground");
    let drop = p[2] - h.point[2];
    eprintln!("Ratchet (instance {ratchet}, class {}) at {p:?}: ground {:?} ({drop:.4} below), type {:#x}", n.mobys[ratchet].o_class, h.point, h.kind);
    assert!((-0.01..=2.0).contains(&drop), "ground {drop} below Ratchet's spawn");

    // The same ground from the sphere and the hero capsule (flags 0x24) resting just above it.
    let s = coll_sphere(&n.mesh, [h.point[0], h.point[1], h.point[2] + 0.4], 0.5, QueryFlags::NONE).expect("sphere");
    assert!(s.pushed_centre.unwrap()[2] > h.point[2] + 0.45);
    let c = coll_capsule(&n.mesh, [h.point[0], h.point[1], h.point[2] + 0.4], 1.2, 0.5, QueryFlags(0x24)).expect("capsule");
    eprintln!("sphere: {:?} -> {:?}; capsule: {:?} -> {:?}", s.point, s.pushed_centre, c.point, c.pushed_centre);
}

/// Cell walks of long rays stay consistent: consecutive cells differ by one step on one axis and
/// their entry parameters never decrease.
#[test]
fn novalis_line_cell_walks_are_connected() {
    let Some(n) = novalis() else { eprintln!("skipped: no extracted/"); return; };
    let mut rng = Rng(7);
    let pick = |r: &mut Rng| [0, 1, 2].map(|k| n.lo[k] + r.next() * (n.hi[k] - n.lo[k])).map(clamp_world);
    for _ in 0..200 {
        let (a, b) = (pick(&mut rng), pick(&mut rng));
        let Some(cells) = cells_for_line(a, b) else { continue };
        for w in cells.windows(2) {
            let steps: i32 = (0..3).map(|k| (w[1].cell[k] - w[0].cell[k]).abs()).sum();
            assert_eq!(steps, 1, "{a:?} -> {b:?}: {:?} -> {:?}", w[0], w[1]);
            assert!(w[1].t_enter >= w[0].t_enter);
        }
    }
}

