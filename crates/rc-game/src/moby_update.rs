//! Per-class moby updates: the scheduler of the level's moby loop, the services the updates call and the
//! ported classes (bolts 13–16, crates 500/501/502/505/511, grass 724/725, Blarg flyers 660, teleporter pads 1135,
//! gold-weapon offers 304/1456–1465). Spec: `docs/plan/moby_update_catalogue.md` §1, §4, §7 and the "In the port"
//! sections.
pub mod anim_sound;
pub mod class_swap;
pub mod classes;
pub mod creature;
pub mod interact;
pub mod manip;
pub mod scheduler;
pub mod services;
pub mod story;
pub mod triggers;
pub mod visit;

pub use scheduler::{Groups, Scheduler};
pub use services::{ClassData, ClassTable, Services, World};

#[cfg(test)]
#[allow(clippy::too_many_arguments, clippy::needless_range_loop)]
mod tests {
    use super::classes::{bolt, crate_, grass};
    use super::scheduler::{self, build_active_list, Groups, Scheduler};
    use super::services::{HitTemplate, World};
    use super::{ClassTable, Services};
    use crate::hero::physics::{self as ph, V4};
    use crate::hero::Hero;
    use crate::moby_runtime::{mode, ClassInfo, Moby, MobyTable, Seq0Info};
    use crate::ps2v::Pf;
    use crate::rng::{Rng, LEVEL_SEED};
    use rc_formats::moby_anim::{self, MobyAnimClass, MobyFrame, MobyFrameHeader, MobySequence, MobySequenceHeader};

    fn seeded() -> Rng { let mut r = Rng::new(); r.srand(LEVEL_SEED); r }

    /// Number of `rand()` calls between two states of the stream.
    fn draws(from: &Rng, to: &Rng) -> usize {
        let mut r = *from;
        for n in 0..100_000 {
            if r.state == to.state { return n; }
            r.rand();
        }
        panic!("state not reached");
    }

    fn info(slot: u8, o_class: i16, collision: bool) -> ClassInfo {
        ClassInfo {
            slot,
            update_fn: scheduler::port_update_fn(o_class),
            has_collision: collision,
            scale: 1.0,
            seq0: Some(Seq0Info { frame_count: 1, loop_sound_bit7: false }),
            ..Default::default()
        }
    }

    fn moby(index: u32, o_class: i16, class: &ClassInfo, pos: [f32; 3], pvars: usize) -> Moby {
        let mut m = Moby::init_instance(index, o_class, Some(class));
        m.position = [pos[0], pos[1], pos[2], 0.0];
        m.rows = moby_anim::IDENTITY;
        m.pvars = vec![0; pvars];
        m.update_dist = 0x40;
        m
    }

    fn hero_at(p: [f32; 3]) -> Hero {
        let mut h = Hero::new();
        h.pos = ph::v4(p[0], p[1], p[2]);
        h.ground_point = h.pos;
        h.body_point = ph::v4(p[0], p[1], p[2] + 1.0);
        h.health = 3;
        h
    }

    /// A hero moby (class 0, mode 2) at index 0.
    fn hero_moby() -> Moby {
        let mut m = Moby::init_instance(0, 0, Some(&ClassInfo { scale: 1.0, ..Default::default() }));
        m.mode |= mode::NO_UPDATE;
        m
    }

    #[test]
    fn active_rule_camera_distance_visible_groups_and_slot_order() {
        let a = info(5, 500, true);
        let b = info(2, 724, false);
        let mut ms = vec![hero_moby()];
        // 1: slot 5, 10 units from the camera, update distance 16 → active.
        ms.push(moby(1, 500, &a, [10.0, 0.0, 0.0], 0x100));
        ms[1].update_dist = 16;
        // 2: slot 2, 20 units, distance 16 → inactive; 3: same but visible → active.
        ms.push(moby(2, 724, &b, [20.0, 0.0, 0.0], 0));
        ms[2].update_dist = 16;
        ms.push(moby(3, 724, &b, [20.0, 0.0, 0.0], 0));
        ms[3].update_dist = 16;
        ms[3].visible = 1;
        // 4: exactly on the boundary (d² − |p|² = 0: sign clear) → active; 5: 0xff far away → active.
        ms.push(moby(4, 724, &b, [16.0, 0.0, 0.0], 0));
        ms[4].update_dist = 16;
        ms.push(moby(5, 500, &a, [900.0, 0.0, 0.0], 0x100));
        ms[5].update_dist = 0xff;
        ms[5].mode |= mode::TARGETABLE;
        // 6/7: group 3; 6 is in range, 7 is far away but joins through the group (even with mode 2).
        ms.push(moby(6, 500, &a, [1.0, 0.0, 0.0], 0x100));
        ms[6].group = 3;
        ms.push(moby(7, 724, &b, [500.0, 0.0, 0.0], 0));
        ms[7].group = 3;
        ms[7].mode |= mode::NO_UPDATE;
        // 8: deleted.
        ms.push(moby(8, 500, &a, [0.0, 0.0, 0.0], 0x100));
        ms[8].state = 0xfd;
        let table = MobyTable::new(ms, 4);
        let groups = Groups { lists: vec![None, None, None, Some(vec![7, 6])] };
        let cam: V4 = [Pf::ZERO; 4];
        let (list, targets) = build_active_list(&table, cam, &groups);
        // Slot 2: direct 3, 4 then group member 7; slot 5: direct 1, 5 then group member 6.
        assert_eq!(list, vec![3, 4, 7, 1, 5, 6]);
        assert_eq!(targets, vec![5]);
        for (id, m) in table.mobys.iter().enumerate().take(9) {
            let expect = list.contains(&id);
            let direct = m.state < 0x80 && m.mode & 2 == 0 && m.group < 0 && scheduler::is_active(m, cam);
            if m.group < 0 { assert_eq!(expect, direct, "moby {id}"); }
        }
    }

    /// The scheduler loop for tests: `n` ticks from `counter`, returns (ticks run).
    fn run(sched: &mut Scheduler, table: &mut MobyTable, hero: &Hero, rng: &mut Rng, classes: &ClassTable, svc: &mut Services, counter: &mut u64, n: usize) {
        for _ in 0..n {
            table.free_slot_pass(*counter);
            let mut w = World::new(table, hero, rng, classes, svc, *counter);
            sched.tick(&mut w);
            *counter += 1;
        }
    }

    #[test]
    fn bolt_flies_to_the_hero_and_is_collected() {
        let bi = info(1, 13, false);
        let mut classes = ClassTable::default();
        classes.classes.insert(13, (bi, None));
        let mut b = moby(1, 13, &bi, [2.0, 0.0, 0.0], 0x80);
        b.pvars[0x54] = 1;
        b.spawn_id = 7;
        b.mission = 0xff;
        let mut table = MobyTable::new(vec![hero_moby(), b], 8);
        let hero = hero_at([0.0, 0.0, 0.0]);
        let mut rng = seeded();
        let start = rng;
        let mut svc = Services::new();
        let mut sched = Scheduler::new();
        {
            let mut w = World::new(&mut table, &hero, &mut rng, &classes, &mut svc, 0);
            sched.load_pass(&mut w);
        }
        assert_eq!(table.mobys[1].state, 3);
        assert_eq!(draws(&start, &rng), 4, "init: randi(2), 2 × rand_angle, rand_range");
        // Start at tick 10 so the pickup sound plays (no random bend: a straight path).
        let mut counter = 10u64;
        let mut ticks = 0;
        while table.mobys[1].state < 0x80 && ticks < 1000 {
            run(&mut sched, &mut table, &hero, &mut rng, &classes, &mut svc, &mut counter, 1);
            ticks += 1;
        }
        assert_eq!(svc.counters.bolts, 1);
        assert_eq!(svc.save.collected.get(&7), Some(&1));
        assert_eq!(svc.sounds.len(), 1);
        assert_eq!(draws(&start, &rng), 4 + 2 + 5, "fly start: randf(3,7), randf(240,720); pickup: rand_vec (3), randi, randf");

        // Independent f64 model of the flight (straight path): vel up at s·dt, drag 10·dt², speed ramps by
        // 16·dt² to 48·dt, collected once the distance to the body point is within the speed.
        let mut r = start;
        for _ in 0..4 { r.rand(); }
        let s = r.randf(3.0, 7.0) as f64;
        let dt = 1.0 / 60.0f64;
        let (mut p, mut v, mut speed) = ([2.0f64, 0.0, 0.0], [0.0f64, 0.0, s * dt], 0.0f64);
        let body = [0.0f64, 0.0, 1.0];
        let mut expect = 1; // the tick that starts the flight
        loop {
            expect += 1;
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            let drag = 10.0 * dt * dt;
            if drag < l { for k in 0..3 { v[k] -= v[k] / l * drag; } }
            speed = (speed + 16.0 * dt * dt).min(48.0 * dt);
            for k in 0..3 { p[k] += v[k]; }
            let to = [body[0] - p[0], body[1] - p[1], body[2] - p[2]];
            let d = (to[0] * to[0] + to[1] * to[1] + to[2] * to[2]).sqrt();
            if speed < d { for k in 0..3 { p[k] += to[k] / d * speed; } } else { break; }
        }
        assert!((ticks as i64 - expect as i64).abs() <= 1, "collected after {ticks} ticks, model {expect}");
    }

    fn crate_table(o_class: i16, b4: i16, dynamic: usize) -> (MobyTable, ClassTable) {
        let ci = info(3, o_class, true);
        let mut classes = ClassTable::default();
        classes.classes.insert(o_class, (ci, None));
        for c in [13, 14, 15, 16] { classes.classes.insert(c, (info(1, c, false), None)); }
        let mut c = moby(1, o_class, &ci, [5.0, 0.0, 0.0], 0x100);
        c.b4 = b4;
        c.spawn_flag = 0xff;
        c.spawn_id = 9;
        c.mission = 0xff;
        c.pvars[0xc0..0xc4].copy_from_slice(&(-1i32).to_le_bytes());
        (MobyTable::new(vec![hero_moby(), c], dynamic), classes)
    }

    /// The 3-coin approximation of `BoltBurst` (flags 8), written independently: A = the best value ≥ N
    /// reachable from 50+50+50 by breaking the biggest coin down, B = the best ≤ N from 1+1+1 building up.
    fn three_coins(n: i32) -> Vec<i32> {
        let mut a = vec![50, 50, 50];
        let down = |c: i32| match c { 50 => 20, 20 => 5, _ => 1 };
        if n < 150 {
            loop {
                if let Some(i) = a.iter().position(|&c| c > 1) { a[i] = down(a[i]); }
                let s: i32 = a.iter().sum();
                if !(n < s) || s < 4 { break; }
                a.sort_unstable_by(|x, y| y.cmp(x));
            }
        }
        let mut b = vec![1, 1, 1];
        let up = |c: i32| match c { 1 => 5, 5 => 20, _ => 50 };
        if 3 < n {
            loop {
                if let Some(i) = b.iter().position(|&c| c < 50) {
                    let lowest = *b.iter().filter(|&&c| c < 50).min().unwrap();
                    let i = b.iter().position(|&c| c == lowest).unwrap_or(i);
                    b[i] = up(b[i]);
                }
                let s: i32 = b.iter().sum();
                if !(s < n) || !(s < 150) { break; }
            }
        }
        let (sa, sb): (i32, i32) = (a.iter().sum(), b.iter().sum());
        let mut r = if sa - n < n - sb { a } else { b };
        r.sort_unstable_by(|x, y| y.cmp(x));
        r
    }

    #[test]
    fn crate_break_drops_bolts_with_the_game_draws() {
        let (mut table, classes) = crate_table(500, 7, 64);
        let hero = hero_at([0.0, 0.0, 0.0]);
        let mut rng = seeded();
        let start = rng;
        let mut svc = Services::new();
        let mut sched = Scheduler::new();
        {
            let mut w = World::new(&mut table, &hero, &mut rng, &classes, &mut svc, 0);
            sched.load_pass(&mut w);
        }
        assert_eq!(table.mobys[1].state, 1);
        assert_eq!(draws(&start, &rng), 1, "class 500 init: randi(2) quarter turn");
        let after_init = rng;
        {
            let mut w = World::new(&mut table, &hero, &mut rng, &classes, &mut svc, 20);
            w.deliver_hit(1, &HitTemplate { flags: 0x1_0000, damage: Pf::ONE, ..Default::default() });
        }
        let mut counter = 20u64;
        run(&mut sched, &mut table, &hero, &mut rng, &classes, &mut svc, &mut counter, 1);
        assert_eq!(table.mobys[1].state, 3, "broken and hidden");
        assert!(table.mobys[1].mode & mode::HIDDEN != 0);
        // Free slots 64 (≤ 100): one big debris, no chunks; SetDeathBits: 7 bolts ± 1, flags 5 | 8 (≤ 3 coins).
        let bolts: Vec<i16> = table.mobys[2..].iter().filter(|m| (13..=16).contains(&m.o_class) && !m.is_deleted()).map(|m| m.o_class).collect();
        let mut r = after_init;
        for _ in 0..9 { r.rand(); } // debris: randf, randf, randi + DebrisSpawn's 6
        let n = r.randi(3) + 6;
        let coins = three_coins(n);
        let classes_expected: Vec<i16> = coins.iter().map(|&v| match v { 5 => 14, 20 => 15, 50 => 16, _ => 13 }).collect();
        assert_eq!(bolts, classes_expected, "N = {n}");
        assert_eq!(draws(&after_init, &rng), 9 + 1 + 9 * coins.len(), "debris 9, N 1, per coin 4 + BoltSpawn 5");
        assert!(table.mobys[2..].iter().filter(|m| (13..=16).contains(&m.o_class)).all(|m| m.state == 1), "dropped bolts fall");
        assert!(svc.save.death.contains(&(1, 9)));
        assert_eq!(svc.sounds.iter().filter(|s| s.index == 0).count(), 1, "break sound");
        // The next tick the crate (no respawn flag) is deleted after its hidden state 3.
        run(&mut sched, &mut table, &hero, &mut rng, &classes, &mut svc, &mut counter, 1);
        assert_eq!(table.mobys[1].state, 0xfd);
    }

    #[test]
    fn tnt_fuse_runs_179_updates_with_four_beeps() {
        let (mut table, classes) = crate_table(505, 0, 64);
        let mut hero = hero_at([0.0, 0.0, 0.0]);
        let mut rng = seeded();
        let mut svc = Services::new();
        let mut sched = Scheduler::new();
        {
            let mut w = World::new(&mut table, &hero, &mut rng, &classes, &mut svc, 0);
            sched.load_pass(&mut w);
        }
        hero.cap_moby = Some(1);
        let mut counter = 10u64;
        run(&mut sched, &mut table, &hero, &mut rng, &classes, &mut svc, &mut counter, 1);
        assert_eq!(table.mobys[1].state, 5, "touch lights the fuse");
        hero.cap_moby = None;
        let mut updates = 0;
        let mut beeps = Vec::new();
        while table.mobys[1].state == 5 && updates < 400 {
            let before = table.mobys[1].cmd;
            let n = svc.sounds.len();
            run(&mut sched, &mut table, &hero, &mut rng, &classes, &mut svc, &mut counter, 1);
            updates += 1;
            if svc.sounds[n..].iter().any(|s| s.index == 1) { beeps.push(if before == 0 { 179 } else { before as i32 }); }
        }
        assert_eq!(updates, 179);
        assert_eq!(beeps, vec![177, 117, 57, 13]);
        assert_eq!(table.mobys[1].state, 3);
        // The blast grows for ticks(20) + 1 updates, then the crate is deleted.
        let mut n = 0;
        while table.mobys[1].state == 3 && n < 100 {
            run(&mut sched, &mut table, &hero, &mut rng, &classes, &mut svc, &mut counter, 1);
            n += 1;
        }
        assert_eq!(n, 21);
        assert_eq!(table.mobys[1].state, 0xfd);
    }

    fn grass_class() -> MobyAnimClass {
        let key = |rate: f32| MobyFrame {
            header: MobyFrameHeader { rate, time: 0, qwc: 1, quat_bytes: 8, scale_count: 0, trans_offset: 8, trans_count: 0 },
            quats: vec![[0, 0, 0, 0x7fff]],
            scales: vec![],
            trans: vec![],
            payload: vec![0, 0, 0, 0, 0, 0, 0xff, 0x7f, 0, 0, 0, 0, 0, 0, 0, 0],
        };
        let seq = |n: u8| Some(MobySequence { header: MobySequenceHeader { frame_count: n, loop_sound: 0xff, ..Default::default() }, frames: vec![key(0.5); n as usize], triggers: vec![] });
        MobyAnimClass { joint_count: 1, skeleton: vec![moby_anim::IDENTITY], rest: vec![[0.0; 3]], parent_word: vec![0], sequences: vec![seq(2), seq(4)] }
    }

    #[test]
    fn grass_bends_when_the_hero_runs_through_and_returns() {
        let mut gi = info(2, 724, false);
        gi.seq0 = Some(Seq0Info { frame_count: 2, loop_sound_bit7: false });
        let mut classes = ClassTable::default();
        classes.classes.insert(724, (gi, Some(grass_class())));
        let g = moby(1, 724, &gi, [0.5, 0.0, 0.0], 0);
        let mut table = MobyTable::new(vec![hero_moby(), g], 4);
        let mut hero = hero_at([0.0, 0.0, 0.0]);
        let mut rng = seeded();
        let start = rng;
        let mut svc = Services::new();
        let mut sched = Scheduler::new();
        let mut counter = 1u64;
        // Standing still: stays on sequence 0.
        run(&mut sched, &mut table, &hero, &mut rng, &classes, &mut svc, &mut counter, 5);
        assert_eq!(table.mobys[1].anim.seq_b, 0);
        // Moving 0.05 u/tick (> 2·dt) within 1 unit: blend to sequence 1.
        hero.disp = ph::v4(0.05, 0.0, 0.0);
        run(&mut sched, &mut table, &hero, &mut rng, &classes, &mut svc, &mut counter, 1);
        assert_eq!(table.mobys[1].anim.seq_b, 1);
        // Just below 2·dt: no trigger (already on 1 anyway); leave: the sequence plays out and returns to 0.
        hero.disp = ph::v4(0.0, 0.0, 0.0);
        let mut t = 0;
        while table.mobys[1].anim.seq_b == 1 && t < 100 {
            run(&mut sched, &mut table, &hero, &mut rng, &classes, &mut svc, &mut counter, 1);
            t += 1;
        }
        assert_eq!(table.mobys[1].anim.seq_b, 0);
        // 6-tick blend into key 0, then keys 0→1→2→3 at 2 ticks each; the wrap flag (bit 1) is raised when
        // the advance steps into the last interval (key 3 → key 0), which is when the grass blends back.
        assert_eq!(t, 6 + 2 * 3);
        assert_eq!(draws(&start, &rng), 0, "grass draws nothing");
        // Far away (> 1 unit) and moving: nothing.
        let far = hero_at([3.0, 0.0, 0.0]);
        let mut far = far;
        far.disp = ph::v4(0.1, 0.0, 0.0);
        run(&mut sched, &mut table, &far, &mut rng, &classes, &mut svc, &mut counter, 30);
        assert_eq!(table.mobys[1].anim.seq_b, 0);
        let _ = grass::UPDATE_FN + bolt::UPDATE_FN + crate_::UPDATE_FN;
    }
}
