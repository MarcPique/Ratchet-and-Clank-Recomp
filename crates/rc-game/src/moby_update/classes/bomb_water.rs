//! The Bomb Glove bomb's water effects (class 121, level01 `0x2c3300`; read from the decomp and its calls). The
//! Mine Glove's mine (class 74, `0x2bfe40`, `super::mine`) makes the same water entry, sinking bubbles, bubble burst
//! and scorch with the same calls (its own counts: [`MINE_SCORCH`], the bubbles downward), so these are written once
//! for both.
//!
//! * [`entry`] — falling onto a water face (surface 0) with vz < 0: `RippleDisturb(x, y, 0.5, −0.35, every patch,
//!   additive)` (0x2b82a8), the splash moby 775 of size 2 at (x, y, the hit z) with alpha 0x70 (`FUN_002ff768`,
//!   `splash::spawn`), then 16 type-35 drops (the loop counts 0xf down to −1: corrected 2026-09-28, was 15) from the same point: `rand_angle`, speed `randf(0, 3·dt)` out,
//!   `randf(3·dt, 6.5·dt)` up, life `rand_range(90, 120)`, kind `randi(2)` (`PartType35Spawn` 0x2845a8). The caller
//!   sets +0x68 = 1 and the sinking velocity (0, 0, −1.5·dt).
//! * [`sink_bubble`] — every tick in water, 1 in `ticks(6) − 1` (`randi`): a type-34 bubble (`PartType34Spawn`
//!   0x2840e0) at the bomb, velocity ¼ of the bomb's, size `randf(0.05, 0.1)·210000`, popping at the water level
//!   ([`water_level`]).
//! * [`deep_burst`] — the explosion after `ticks(20)` or more in water (no hits, no high fireballs, rings or
//!   flashes): 150 bubbles, direction `(randf(±1), randf(±1), randf(−0.5, 2))` at `randf(4·dt, 8·dt)·k`, popping at
//!   the water level 1 above the bomb; the class-0 (Ratchet's) sound 0x16 (`PlayClassSoundByClass(0x16, 0, m, 0)`)
//!   instead of the bomb's sound 0 is the caller's.
//! * [`shallow_scorch`] — any explosion in water (+0x68 ≠ 0): with the water surface (`GroundHeight(0.5, p, 0)`) and
//!   the ground under it (`GroundHeight(0.5, p, 0x20)`, water faces skipped) from 3.5 above the bomb, when they differ
//!   by less than 1.2 (shallow water): 100 type-64 scorches bursting off the ground from the surface (0.5 above it,
//!   within 0.35 of the bomb, `randf(±0.75·dt)` out, `randf(4·dt, 14·dt) − 8·r·dt` up, size `randf(0.25, 1)`, gravity
//!   20·dt², colours 0x167f6060 / 0x7f6060, additive) and 20 type-15 sparks from the surface (colours tweened
//!   `randf(0.25, 1)` of 0x7f000000 → 0x7f7f6060 and `randf(0.5, 1)` of 0 → 0x3f3030, speed `randf(1.5, 3)·dt`
//!   then z = `randf(3, 6)·dt`, size `randf(31500, 52500)`, life `rand_range(ticks(90), ticks(150))`, split,
//!   texture `*def[11]`).
//!
//! The water level (`SetWaterLevel` 0x26ed38) and the ripple disturbance reach the engine's ripple patches (class
//! 751, `crate::water`) through [`crate::moby_update::services::ExternalUpdates`]; without them the level is the
//! bomb's own z and the disturbance is dropped (no draws either way). Standard `f32`; the draws and their order are
//! the game's.

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::splash;
use crate::moby_update::services::World;
use crate::particles::{tween_color, type15, type34, type35, type64};
use crate::ps2v::Pf;

type V = [f32; 4];

fn dt() -> f32 { crate::moby_update::services::DT.to_f32() }
fn dt2() -> f32 { crate::moby_update::services::DT2.to_f32() }

fn setlen(a: [f32; 3], l: f32) -> [f32; 3] {
    let n = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    if n == 0.0 { [0.0; 3] } else { a.map(|x| x * l / n) }
}

/// `SetWaterLevel(pos, 0)` 0x26ed38: the level's water over `pos` (the active ripple patch, the flat plane:
/// `crate::water::world::WaterWorld::water_height`), else `pos.z`.
pub fn water_level(w: &World, pos: V) -> f32 { w.svc.water.water_height([pos[0], pos[1], pos[2]]).unwrap_or(pos[2]) }

/// `RippleDisturb(x, y, r, amp, every patch, n, additive)` 0x2b82a8.
pub fn ripple(w: &mut World, x: f32, y: f32, r: f32, amp: f32, additive: bool) { w.svc.water.disturb(x, y, r, amp, additive); }

/// `PartType34Spawn(size, level, pos, vel)` 0x2840e0 (its six draws, with or without a particle system).
pub fn part34(w: &mut World, size: f32, level: f32, pos: V, vel: [f32; 3]) {
    *w.svc.fx.part_spawns.entry(type34::TYPE).or_default() += 1;
    let d = type34::Draws::draw(w.rng);
    if let Some(p) = w.particles.as_deref_mut() {
        if type34::spawn(p, size, level, pos, vel, &d).is_none() { w.svc.fx.part_failed += 1; }
    }
}

/// `PartType35Spawn(pos, vel, kind, life)` 0x2845a8 (two draws with a record).
pub fn part35(w: &mut World, pos: V, vel: V, kind: i32, life: i32) {
    *w.svc.fx.part_spawns.entry(type35::TYPE).or_default() += 1;
    match w.particles.as_deref_mut() {
        Some(p) => {
            if type35::spawn(p, w.rng, pos, vel, kind, life).is_none() { w.svc.fx.part_failed += 1; }
        }
        None => {
            w.rng.randf(10500.0, 16800.0);
            w.rng.randf(0.0, 1.0);
        }
    }
}

/// `PartType64Spawn` 0x288d90 (one draw with a record).
pub fn part64(w: &mut World, a: &type64::Spawn) {
    *w.svc.fx.part_spawns.entry(type64::TYPE).or_default() += 1;
    match w.particles.as_deref_mut() {
        Some(p) => {
            if type64::spawn(p, w.rng, a).is_none() { w.svc.fx.part_failed += 1; }
        }
        None => {
            w.rng.randf(0.0, 255.0);
        }
    }
}

/// The water entry (module doc) at the hit height `surface_z`.
pub fn entry(w: &mut World, id: MobyId, surface_z: f32) {
    let p = w.m(id).position;
    ripple(w, p[0], p[1], 0.5, f32::from_bits(0xbeb3_3333), true);
    splash_and_drops(w, 2.0, [p[0], p[1], surface_z, p[3]]);
}

/// The second half of the water entry: the splash moby 775 of size `size` (2 here) at `at` with alpha 0x70 and the 16
/// type-35 drops (the same calls in the drip 787's splash, level01 `0x2ffdc0`: `units::drip`, size 2; and level 08's
/// liquid 327 when Ratchet falls in, `0x2da0f0`: `crate::water::sea`, size 3).
pub fn splash_and_drops(w: &mut World, size: f32, at: V) {
    if let Some(s) = splash::spawn(w, size, at) { w.mm(s).alpha = 0x70; }
    for _ in 0..16 {
        let a = w.rng.rand_angle();
        let s = w.rng.randf(dt() * 0.0, dt() * 3.0);
        let vz = w.rng.randf(dt() * 3.0, dt() * 6.5);
        let life = w.rng.rand_range(0x5a, 0x78);
        let kind = w.rng.randi(2);
        part35(w, at, [a.cos() * s, a.sin() * s, vz, 0.0], kind, life);
    }
}

/// The sinking bomb's bubble chance (module doc); `vel` = its velocity after this tick's gravity.
pub fn sink_bubble(w: &mut World, id: MobyId, vel: V) {
    let n = w.ticks(6);
    if w.rng.randi(n - 1) != 0 { return; }
    let pos = w.m(id).position;
    let level = water_level(w, pos);
    let v = [vel[0] * 0.25, vel[1] * 0.25, vel[2] * 0.25];
    let size = w.rng.randf(f32::from_bits(0x3d4c_cccd), f32::from_bits(0x3dcc_cccd)) * 210000.0;
    part34(w, size, level, pos, v);
}

/// The deep-water explosion's 150 bubbles (module doc); `k` = the bomb's size factor (1, 2 with the gold glove).
pub fn deep_burst(w: &mut World, id: MobyId, k: f32) {
    let pos = w.m(id).position;
    let level = water_level(w, [pos[0], pos[1], pos[2] + 1.0, pos[3]]);
    bubbles(w, pos, level, k, 2.0);
}

/// 150 type-34 bubbles from `pos` popping at `level`: direction `(randf(±1), randf(±1), randf(−0.5, z_hi))` at
/// `randf(4·dt, 8·dt)·k` (the bomb's deep burst: `z_hi` 2; the mine's explosion in water `0x2bfe40`: −2, `k` 1).
pub fn bubbles(w: &mut World, pos: V, level: f32, k: f32, z_hi: f32) {
    for _ in 0..150 {
        let x = w.rng.randf(-1.0, 1.0);
        let y = w.rng.randf(-1.0, 1.0);
        let z = w.rng.randf(-0.5, z_hi);
        let s = w.rng.randf(dt() * 4.0, dt() * 8.0);
        let v = setlen([x, y, z], k * s);
        let size = w.rng.randf(f32::from_bits(0x3d4c_cccd), f32::from_bits(0x3dcc_cccd)) * 210000.0;
        part34(w, size, level, pos, v);
    }
}

/// The shallow-water scorch and sparks of an explosion in water (module doc).
pub fn shallow_scorch(w: &mut World, id: MobyId) {
    let pos = w.m(id).position;
    let probe = [Pf::f(pos[0]), Pf::f(pos[1]), Pf::f(pos[2] + 3.5), Pf::f(pos[3])];
    let surf = w.ground_height(Pf::f(0.5), probe, 0).to_f32();
    let ground = w.ground_height(Pf::f(0.5), probe, 0x20).to_f32();
    if surf == ground || surf - ground >= 1.2 { return; }
    scorch(w, pos, surf, &BOMB_SCORCH);
}

/// The scorch's constants: the count, the top of the upward speed (×dt), the size range.
pub struct Scorch {
    pub n: usize,
    pub vz_hi: f32,
    pub size: (f32, f32),
}

/// The bomb's (`0x2c3300`): 100, 14, 0.25..1.
pub const BOMB_SCORCH: Scorch = Scorch { n: 100, vz_hi: 14.0, size: (0.25, 1.0) };
/// The mine's in water (`0x2bfe40`, always, from the surface over it): 150, 16, 0.35..1.25.
pub const MINE_SCORCH: Scorch = Scorch { n: 150, vz_hi: 16.0, size: (0.35, 1.25) };

/// The scorch (type 64) and the 20 sparks (type 15) from the water surface `surf` at `pos` (module doc).
pub fn scorch(w: &mut World, pos: V, surf: f32, c: &Scorch) {
    for _ in 0..c.n {
        let vx = w.rng.randf(dt() * -0.75, dt() * 0.75);
        let vy = w.rng.randf(dt() * -0.75, dt() * 0.75);
        let vz = w.rng.randf(dt() * 4.0, dt() * c.vz_hi);
        let r = w.rng.randf(0.0, f32::from_bits(0x3eb3_3333));
        let a = w.rng.rand_angle();
        let at = [a.cos() * r + pos[0], a.sin() * r + pos[1], surf + 0.5, pos[3]];
        let vel = [vx, vy, vz - r * dt() * 8.0, 0.0];
        let size = w.rng.randf(c.size.0, c.size.1);
        part64(w, &type64::Spawn { size, floor: surf, g: dt2() * 20.0, pos: at, vel, life: 0, rgba: 0x167f_6060, rgba2: 0x7f_6060, additive: true });
    }
    let def = w.particles.as_deref().map_or(-1, |p| p.def_first(11) as i32);
    for _ in 0..20 {
        let f = w.rng.randf(0.25, 1.0);
        let c1 = tween_color(f.to_bits(), 0x7f00_0000, 0x7f7f_6060);
        let f = w.rng.randf(0.5, 1.0);
        let c2 = tween_color(f.to_bits(), 0, 0x3f_3030);
        let x = w.rng.randf(-3.0, 3.0);
        let y = w.rng.randf(-3.0, 3.0);
        let z = w.rng.randf(dt() * 4.0, dt() * 14.0);
        let s = w.rng.randf(1.5, 3.0);
        let mut v = setlen([x, y, z], s * dt());
        v[2] = w.rng.randf(3.0, 6.0) * dt();
        let size = w.rng.randf(f32::from_bits(0x46f6_1801), f32::from_bits(0x474d_1400));
        let (t90, t150) = (w.ticks(0x5a), w.ticks(0x96));
        let life = w.rng.rand_range(t90, t150);
        let a = type15::Spawn { size, pos: [pos[0], pos[1], surf, pos[3]], vel: [v[0], v[1], v[2], 0.0], c1, c2, life, split: 1, def, blend: -1 };
        crate::moby_update::creature::fx::part15(w, &a);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    //! The bomb-in-water rand ledger: a Bomb Glove bomb dropped into a hand-built pool, tick by tick, against the
    //! draws the game's code makes (the dry ledger is `bomb::tests::dry_explosion_rand_stream_matches_the_game_ledger`;
    //! the per-call counts are the module doc's and `bomb.rs`'s).
    use crate::hero::testkit::{cell, mesh};
    use crate::hero::Hero;
    use crate::moby_runtime::{ClassInfo, Moby, MobyTable};
    use crate::moby_update::classes::bomb::{init_held, pv, release, BOMB_CLASS, FIREBALL_CLASS, FLASH_CLASS, FLYING};
    use crate::moby_update::scheduler::{self, Scheduler};
    use crate::moby_update::services::{pvar as p, ClassTable, Services, World};
    use crate::ps2v::Pf;
    use crate::rng::Rng;
    use rc_formats::collision::Collision;

    /// Floor quads (surface 1) at `floor` and water quads (surface 0) at `water` over x, y in 0..48.
    pub(crate) fn pool(floor: f32, water: f32) -> Collision {
        let mut cells = Vec::new();
        for cx in 0..12i16 {
            for cy in 0..12i16 {
                let (x, y) = (cx as f32 * 4.0, cy as f32 * 4.0);
                let q = |z: f32| [[x, y, z], [x, y + 4.0, z], [x + 4.0, y + 4.0, z], [x + 4.0, y, z]];
                let (cf, cw) = (((floor - 2.0) / 4.0).round() as i16, ((water - 2.0) / 4.0).round() as i16);
                if cf == cw {
                    let mut v = q(floor).to_vec();
                    v.extend_from_slice(&q(water));
                    cells.push(cell([cx, cy, cf], &v, &[([0, 1, 2, 3], 0x21), ([4, 5, 6, 7], 0x00)]));
                } else {
                    cells.push(cell([cx, cy, cf], &q(floor), &[([0, 1, 2, 3], 0x21)]));
                    cells.push(cell([cx, cy, cw], &q(water), &[([0, 1, 2, 3], 0x00)]));
                }
            }
        }
        mesh(cells)
    }

    fn classes() -> ClassTable { classes_with(&[]) }

    /// The bomb's classes (bomb, fireball, flash, explosion light, splash) plus `extra`, each with its port's update
    /// (the other gloves' objects' tests use it too).
    pub(crate) fn classes_with(extra: &[i16]) -> ClassTable {
        let mut t = ClassTable::default();
        let base = [BOMB_CLASS, FIREBALL_CLASS, FLASH_CLASS, crate::moby_update::creature::fx::LIGHT_CLASS, crate::moby_update::classes::splash::CLASS];
        for (slot, &oc) in base.iter().chain(extra).enumerate() {
            let info = ClassInfo { slot: slot as u8 + 1, update_fn: scheduler::port_update_fn(oc), scale: 1.0, ..Default::default() };
            t.classes.insert(oc, (info, None));
        }
        t
    }

    /// A small moby world for the glove objects' tests: Ratchet's moby 0 (no update), a class table, the collision,
    /// the particles and the scheduler; `tick` runs one moby loop (with an optional camera view).
    pub(crate) struct Bench {
        pub table: MobyTable,
        pub hero: Hero,
        pub rng: Rng,
        pub svc: Services,
        pub sched: Scheduler,
        pub parts: crate::particles::Particles,
        pub ct: ClassTable,
        pub coll: Collision,
        pub camera: [Pf; 4],
        pub counter: u64,
    }

    impl Bench {
        pub(crate) fn new(coll: Collision, extra: &[i16], hero_at: [f32; 3]) -> Bench {
            let mut h = Moby::init_instance(0, 0, Some(&ClassInfo { scale: 1.0, has_collision: false, ..Default::default() }));
            h.mode |= crate::moby_runtime::mode::NO_UPDATE;
            h.position = [hero_at[0], hero_at[1], hero_at[2], 1.0];
            let mut hero = Hero::new();
            hero.pos = crate::hero::physics::v4(hero_at[0], hero_at[1], hero_at[2]);
            let mut rng = Rng::new();
            rng.srand(crate::rng::LEVEL_SEED);
            Bench {
                table: MobyTable::new(vec![h], 96),
                hero,
                rng,
                svc: Services::new(),
                sched: Scheduler::new(),
                parts: crate::particles::Particles::new(None, Vec::new()),
                ct: classes_with(extra),
                coll,
                camera: [Pf::f(50.0), Pf::f(20.0), Pf::f(13.0), Pf::ONE],
                counter: 0,
            }
        }
        /// A moby of `o_class` (the class table's init).
        pub(crate) fn create(&mut self, o_class: i16) -> usize {
            let info = self.ct.classes.get(&o_class).map(|c| &c.0);
            self.table.create(o_class, info, self.counter).unwrap()
        }
        pub(crate) fn tick(&mut self, view: Option<&crate::particles::BSphereView>) {
            self.counter += 1;
            self.table.free_slot_pass(self.counter);
            let mut w = World::new(&mut self.table, &self.hero, &mut self.rng, &self.ct, &mut self.svc, self.counter);
            w.camera = self.camera;
            w.coll = Some(&self.coll);
            w.particles = Some(&mut self.parts);
            w.view = view;
            self.sched.tick(&mut w);
        }
        pub(crate) fn parts(&self, ty: u8) -> u64 { self.svc.fx.part_spawns.get(&ty).copied().unwrap_or(0) }
        pub(crate) fn alive(&self, o_class: i16) -> usize { self.table.mobys.iter().filter(|m| m.o_class == o_class && m.state < 0xfd).count() }
    }

    fn draws(from: u32, to: u32) -> usize {
        let mut r = Rng { state: from };
        for n in 0..1_000_000 {
            if r.state == to { return n; }
            r.rand();
        }
        panic!("state not reached");
    }

    struct Outcome {
        entry: bool,
        exploded_after: i16,
        deep: bool,
        scorch: bool,
        bubbles: u64,
        stream: u32,
    }

    /// Drops a bomb from (20, 20, 13) into the pool, fuse `life`; checks every tick's draws against the ledger.
    fn run(floor: f32, water: f32, life: i16) -> Outcome {
        let coll = pool(floor, water);
        let mut h = Moby::init_instance(0, 0, Some(&ClassInfo { scale: 1.0, ..Default::default() }));
        h.mode |= crate::moby_runtime::mode::NO_UPDATE;
        let mut table = MobyTable::new(vec![h], 96);
        let ct = classes();
        let mut hero = Hero::new();
        hero.pos = crate::hero::physics::v4(40.0, 40.0, 20.0);
        let mut rng = Rng::new();
        rng.srand(crate::rng::LEVEL_SEED);
        let mut part_rng = Rng::new();
        let mut svc = Services::new();
        let mut sched = Scheduler::new();
        let mut parts = crate::particles::Particles::new(None, Vec::new());
        let b = table.create(BOMB_CLASS, ct.classes.get(&BOMB_CLASS).map(|c| &c.0), 0).unwrap();
        init_held(&mut table.mobys[b], &mut rng, [20.0, 20.0, 13.0], 0.0);
        p::set_v4f(&mut table.mobys[b].pvars, pv::VEL, [0.02, 0.0, 0.0, 0.0]);
        release(&mut table.mobys[b], [20.0, 20.0, 13.0], None);
        p::set_i16(&mut table.mobys[b].pvars, pv::LIFE, life);
        let camera = [Pf::f(50.0), Pf::f(20.0), Pf::f(13.0), Pf::ONE];
        let mut out = Outcome { entry: false, exploded_after: -1, deep: false, scorch: false, bubbles: 0, stream: 0 };
        for counter in 1..260u64 {
            table.free_slot_pass(counter);
            let timers = |t: &MobyTable| -> Vec<(usize, i16, u8, i32)> {
                t.mobys.iter().enumerate().filter(|(_, m)| m.state < 0xfd).map(|(i, m)| match m.o_class {
                    FIREBALL_CLASS => (i, p::i16(&m.pvars, 0x18), m.cmd, 0),
                    crate::moby_update::creature::fx::LIGHT_CLASS if m.pvars.len() >= 0x80 => (i, 0, 0xff, p::i32(&m.pvars, 0x48)),
                    _ => (i, 0, 0xfe, 0),
                }).collect()
            };
            let before = timers(&table);
            let flying = table.mobys[b].state == FLYING && table.mobys[b].o_class == BOMB_CLASS;
            let wet0 = if flying { p::i16(&table.mobys[b].pvars, pv::WATER) } else { 0 };
            let b34 = svc.fx.part_spawns.get(&34).copied().unwrap_or(0);
            let (b64, b15) = (svc.fx.part_spawns.get(&64).copied().unwrap_or(0), svc.fx.part_spawns.get(&15).copied().unwrap_or(0));
            let s0 = rng.state;
            {
                let mut w = World::new(&mut table, &hero, &mut rng, &ct, &mut svc, counter);
                w.camera = camera;
                w.coll = Some(&coll);
                w.particles = Some(&mut parts);
                sched.tick(&mut w);
            }
            parts.counter = counter;
            parts.update_parts(&mut part_rng);
            let made = draws(s0, rng.state);
            let mut want = 0usize;
            let d34 = svc.fx.part_spawns.get(&34).copied().unwrap_or(0) - b34;
            if flying {
                let wet1 = p::i16(&table.mobys[b].pvars, pv::WATER);
                let exploded = table.mobys[b].state != FLYING || table.mobys[b].o_class != BOMB_CLASS;
                if wet0 == 0 && counter & 7 == 0 { want += 11; }
                if wet0 == 0 && wet1 == 1 && !exploded {
                    // The entry: the splash's rand_angle, 16 drops × (5 + PartType35Spawn's 2).
                    want += 1 + 16 * 7;
                    out.entry = true;
                }
                let mut deep_n = 0;
                if exploded {
                    let dry = (wet1 as i32) < 20;
                    want += 80;
                    if dry { want += 28 + 6 + 4 * (5 + 5) + 5 * 3; } else { want += 150 * (4 + 1 + 6); deep_n = 150; out.deep = true; }
                    let d64 = svc.fx.part_spawns.get(&64).copied().unwrap_or(0) - b64;
                    let d15 = svc.fx.part_spawns.get(&15).copied().unwrap_or(0) - b15;
                    if d64 > 0 {
                        assert_eq!((d64, d15), (100, 20), "tick {counter}: the scorch");
                        want += 100 * 7 + 20 * 10;
                        out.scorch = true;
                    }
                    out.exploded_after = wet1;
                }
                if wet0 != 0 {
                    let sink = d34 - deep_n;
                    assert!(sink <= 1);
                    want += 1 + 7 * sink as usize;
                    out.bubbles += sink;
                }
            }
            for (i, t0, cmd, life0) in &before {
                let m = &table.mobys[*i];
                if *cmd == 1 && m.o_class == FIREBALL_CLASS && (m.state >= 0xfd || p::i16(&m.pvars, 0x18) != *t0) { want += 5; }
                if *cmd == 0xff && m.state < 0xfd && p::i32(&m.pvars, 0x48) != *life0 { want += 1; }
            }
            assert_eq!(made, want, "tick {counter}: {made} draws, the game's code makes {want}");
        }
        assert!(svc.fx.unported.is_empty(), "unported: {:?}", svc.fx.unported);
        assert_eq!(svc.fx.part_failed, 0, "the particle pool overflowed");
        out.stream = rng.state;
        out
    }

    /// Shallow water (floor 0.75 under the surface): the bomb enters, sinks 30 ticks onto the floor and bursts deep
    /// (≥ 20 ticks in water: 150 bubbles, no hits) with the scorch (the floor within 1.2 of the surface).
    #[test]
    fn bomb_sinks_onto_a_shallow_floor() {
        let o = run(10.0, 10.75, 300);
        assert!(o.entry && o.deep && o.scorch, "entry {} deep {} scorch {}", o.entry, o.deep, o.scorch);
        assert!(o.exploded_after >= 20);
        assert!(o.bubbles > 0, "sinking bubbles");
    }

    /// The fuse runs out 10 ticks after the entry: the dry explosion (hits, rings, flashes) plus the scorch.
    #[test]
    fn bomb_fuse_ends_just_under_the_surface() {
        let o = run(10.0, 10.75, 48);
        assert!(o.entry && !o.deep && o.scorch, "entry {} deep {} scorch {}", o.entry, o.deep, o.scorch);
        assert!((1..20).contains(&o.exploded_after), "{}", o.exploded_after);
    }

    /// Deep water (floor 6.75 under the surface): the fuse ends while sinking; bubbles, no scorch. Two runs agree.
    #[test]
    fn bomb_bursts_in_deep_water() {
        let o = run(4.0, 10.75, 90);
        assert!(o.entry && o.deep && !o.scorch, "entry {} deep {} scorch {}", o.entry, o.deep, o.scorch);
        assert_eq!(run(4.0, 10.75, 90).stream, o.stream);
    }
}
