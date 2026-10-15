//! The port's Novalis, run headless the way `rc-engine`'s `gameplay.rs` wires it (no Bevy): the moby table
//! from the loader rules, `Game::new` (hero init, follow camera, `srand(1234)`), the moby services, the
//! load pass on the game's stream, then gameplay ticks (pad → free-slot pass → moby scheduler with the
//! class-27 emitters as external updates and the ripple manager 751 as a port (`Services::water`) → hero → `UpdateParts` + glints →
//! camera → sound step → counter). The hero gets the back items' classes (pack 607, Clank 601: the back table
//! and Clank's fidgets draw), the level (0x15ed84) and, before each tick, the counter (`idle.counter`).
//! **Sound** (`rc_game::audio`, as the engine's crate::audio_out): the moby loop's class sounds through the
//! `SoundSink` (pitch-bend draws in the moby order), Ratchet's animation-trigger sounds and `sound_update` in
//! the sound step after the camera (occlusion origin = 3 draws per tick), the 989snd / SPU side rendered too
//! (its replies free the slots), all on the game's stream.
//!
//! Differences from the engine, on purpose or for lack of an engine-free API (each is a known source of
//! divergence from the PS2 as well):
//! * `UpdateParts` draws from the game's stream through the tick's particle hook (as the engine now does).
//! * No hand items (the engine's item data needs `rc-engine`'s gadget loader): the wrench / bomb-glove mobys
//!   are not created.
//! * The emitter cull view (`FastBSphereCheck`, 0x16d140) is built from the tick's camera, not from the
//!   last rendered frame's Bevy transform.
//! * +0x31 ("drawn last frame") is a stand-in: not hidden (`mode & 0x81`), not deleted and within the draw
//!   distance of the camera; the real MobyProc also tests the frustum and the occlusion.

use anyhow::{Context, Result};
use rc_formats::moby_anim::{parse_sequence, parse_sequences, MobyAnimClass, MobySequence};
use rc_formats::{collision, gameplay, level, wad};
use rc_game::audio::class_sounds::{self, ClassSoundSink, HeroClassSounds};
use rc_game::audio::{AudioSystem, LevelAudio};
use rc_game::follow_camera::CameraView;
use rc_game::hero::anim::RatchetAnim;
use rc_game::hero::Hero;
use rc_game::moby_runtime::{MobyId, MobyTable, Seq0Info};
use rc_formats::moby_spawn::{self, SpawnSave, SpawnTest};
use rc_game::moby_update::scheduler::{self, class_info, load_level_mobys, Scheduler};
use rc_game::moby_update::services::{ExternalUpdates, LevelMissions, World};
use rc_game::moby_update::{ClassTable, Services};
use rc_game::pad::PadInput;
use rc_game::particles::{type06, BSphereView, Owner, Particles};
use rc_game::ps2v::Pf;
use rc_game::rng::Rng;
use rc_game::tick::{Game, GameOptions, TickHooks};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Level-table addresses of the external updates (rc-engine gameplay.rs).
pub const EMITTER_UPDATE: u32 = 0x2bd100;
pub const RIPPLE_UPDATE: u32 = 0x2fd0e8;
pub const RIPPLE_CLASS: i16 = 751;
/// `0x160548[0]`: the ship class on a new game.
pub const SHIP_CLASS: i16 = 531;
/// tan(hfov/2) (0x16cf70) and the NTSC y ratio the emitter cull uses (rc-engine particle_render.rs).
pub const TAN_HALF_FOV_X: f32 = 0.63;
pub const NTSC_Y_RATIO: f32 = 0.775;

pub struct LevelData {
    pub mesh: collision::Collision,
    pub instances: Vec<gameplay::MobyInstance>,
    /// The loader's spawn test per instance, with [`LevelData::save`].
    pub spawn: Vec<SpawnTest>,
    /// The spawn test's save bytes: a first visit on a new game (all zero, as `port_game_state` leaves Novalis).
    pub save: SpawnSave,
    /// Pvar blocks with the moby links through the instance → moby map of `spawn`.
    pub pvars: Vec<Option<Vec<u8>>>,
    pub gp: Vec<u8>,
    pub classes: ClassTable,
    pub spawnable: usize,
    pub death_z: f32,
    pub coll_blobs: Vec<(i32, rc_formats::moby_collision::MobyCollision)>,
    pub splines: Vec<Vec<[f32; 4]>>,
    pub ratchet: MobyAnimClass,
    pub owners: Vec<Owner>,
    pub part_defs: Option<rc_formats::particle_tex::PartDefs>,
    /// The level's water data (the moby system's `Services::water`: 751 and the ripple module).
    pub water: rc_game::water::world::LevelWaterData,
    pub ship: Option<([f32; 3], f32)>,
    pub fog_zones: gameplay::FogZones,
    pub level_settings: Vec<u8>,
    /// The joint lists of the classes whose update reads joint points (`Services::joint_lists`).
    pub joint_lists: HashMap<i16, Vec<Vec<u8>>>,
    /// The back items' anim classes (pack 607, Clank 601).
    pub back: Option<(MobyAnimClass, MobyAnimClass)>,
    /// The level's sound data (None: no sound bank extracted).
    pub audio: Option<LevelAudio>,
}

impl LevelData {
    pub fn load(extracted: &Path, lvl: u32) -> Result<LevelData> {
        let dir = extracted.join(format!("levels/{lvl:02}"));
        let rd = |n: &str| std::fs::read(dir.join(n)).with_context(|| format!("reading {}/{n}", dir.display()));
        let data = wad::decompress(&rd("core_data.bin")?)?;
        let idx = rd("core_index.bin")?;
        let gp = wad::decompress(&rd("gameplay_ntsc.bin")?)?;
        let level_settings = gameplay::section(&gp, "level_settings")?.context("no level settings section")?.to_vec();
        let core = level::parse_level_core(&idx, data.len())?;
        let mesh = collision::parse_collision(&core, &data)?;
        let instances = gameplay::parse_moby_instances(&gp)?;
        let save = SpawnSave::default();
        let spawn = moby_spawn::loader_spawns(&instances, &mut save.clone());
        let pvars = gameplay::parse_pvars_spawned(&gp, &spawn.iter().map(|t| t.spawn).collect::<Vec<_>>())?;
        let splines = gameplay::parse_splines(&gp)?;
        let rdi = |o: usize| i32::from_le_bytes(gp[o..o + 4].try_into().unwrap());
        let spawnable = rdi(rdi(gameplay::MOBY_INSTANCES_POINTER) as usize + 4).max(1) as usize;
        let death_z = f32::from_le_bytes(level_settings[0x28..0x2c].try_into().unwrap());
        let water = {
            use rc_formats::level_overlay::LevelOverlay;
            let ov_bytes = rd("overlay.bin")?;
            let target = LevelOverlay::parse(&ov_bytes)?;
            let reference = |l: u32| -> Option<Arc<LevelOverlay>> {
                let b = std::fs::read(extracted.join(format!("levels/{l:02}/overlay.bin"))).ok()?;
                Some(Arc::new(LevelOverlay::parse(&b).ok()?))
            };
            rc_game::water::world::LevelWaterData::load(&ov_bytes, &target, &reference, &gp)?
        };
        // Class table: slot = index in the core's class list; update = the Rust port or an external update.
        let ext = |oc: i16| match oc { 27 if lvl == 1 => Some(EMITTER_UPDATE), _ => None };
        let mut classes = ClassTable::default();
        let mut joint_lists = HashMap::new();
        for (slot, e) in core.moby_classes.iter().enumerate() {
            let oc = e.o_class as i16;
            let update = scheduler::port_update_fn(oc).or_else(|| ext(oc));
            let blob = core.block(&data, &format!("moby_class/{:04}", e.o_class));
            let parsed = blob.as_ref().and_then(|b| rc_formats::moby::parse_moby_class(b).ok().map(|c| (b, c)));
            match parsed {
                Some((blob, c)) => {
                    if rc_game::moby_update::classes::needs_joint_lists(oc) {
                        let lists: Vec<Vec<u8>> = (0..16).map_while(|l| rc_formats::gadget::joint_list(blob, &c.header, l).ok().map(|(a, _)| a)).collect();
                        joint_lists.insert(oc, lists);
                    }
                    let seqs = parse_sequences(blob, &c).unwrap_or_default();
                    let anim = MobyAnimClass::new(&c, seqs);
                    let mut info = class_info(&c, slot as u8, update);
                    info.seq0 = anim.sequence(0).map(|q| Seq0Info { frame_count: q.header.frame_count, loop_sound_bit7: q.header.loop_sound & 0x80 != 0 });
                    classes.classes.entry(oc).or_insert((info, Some(anim)));
                }
                None => {
                    let info = rc_game::moby_runtime::ClassInfo { slot: slot as u8, no_header: true, update_fn: update, ..Default::default() };
                    classes.classes.entry(oc).or_insert((info, None));
                }
            }
        }
        let coll_blobs = rc_formats::moby_collision::parse_level(&core, &data)?;
        // Ratchet with his own sequence table (core blocks ratchet_seq/NNN), as tests/hero/hero_novalis.rs.
        let rblob = core.block(&data, "moby_class/0000").context("Ratchet's class")?;
        let rclass = rc_formats::moby::parse_moby_class(rblob)?;
        let seqs: Vec<Option<MobySequence>> = (0..256)
            .map(|i| core.block(&data, &format!("ratchet_seq/{i:03}")).and_then(|b| parse_sequence(b, 0).ok()))
            .collect();
        let ratchet = MobyAnimClass::new(&rclass, seqs);
        // Class-27 emitters (rc-engine particle_render::load) and the part definitions.
        let mut owners = Vec::new();
        if lvl == 1 {
            for (i, m) in instances.iter().enumerate().filter(|(_, m)| m.o_class == 27) {
                let Some(p) = m.pvar(&pvars).filter(|p| p.len() >= 0xe0) else { continue };
                owners.push(Owner { instance: i, pos: [m.position[0], m.position[1], m.position[2], 0.0], rot: m.rotation, pvars: p.to_vec() });
            }
        }
        let part_defs = rc_formats::particle_tex::parse_particle_textures(&core, &idx, &data).ok().map(|t| t.defs);
        let ship = gameplay::ship_placement(&gp)?;
        let fog_zones = gameplay::parse_fog_zones(&gp)?;
        let back_anim = |o: i16| classes.classes.get(&o).and_then(|c| c.1.clone());
        let back = back_anim(607).zip(back_anim(601));
        // The sound data as the engine loads it (crate::audio_out::load); the collision for the occlusion rays.
        let audio = match rd("sound_bank.bin") {
            Ok(bank) => {
                let header = rd("level_header.bin")?;
                let table = rc_formats::sound_bank::music_table(&header)?;
                let music: [Option<Vec<u8>>; 15] = std::array::from_fn(|k| (table[k] != 0).then(|| std::fs::read(dir.join(format!("music/{k:03}.bin"))).ok()).flatten());
                Some(LevelAudio::from_parts(&bank, &idx, &core, &data, &gp, &header, &music, Some(mesh.clone()))?)
            }
            Err(_) => None,
        };
        Ok(LevelData { mesh, instances, spawn, save, pvars, gp, classes, spawnable, death_z, coll_blobs, splines, ratchet, owners, part_defs, water, ship, fog_zones, level_settings, joint_lists, back, audio })
    }
}

struct Externals<'a> {
    emitters: &'a HashMap<MobyId, usize>,
    view: Option<&'a BSphereView>,
}

impl ExternalUpdates for Externals<'_> {
    fn update_fn(&self, o_class: i16) -> Option<u32> {
        match o_class { 27 => Some(EMITTER_UPDATE), _ => None }
    }
    fn update(&mut self, addr: u32, id: MobyId, table: &mut MobyTable, rng: &mut Rng, _camera: [Pf; 4], _counter: u64, particles: Option<&mut Particles>) {
        if addr == EMITTER_UPDATE {
            // The owner's live moby: the Blarg flyers (660) move their emitters every tick.
            if let (Some(p), Some(&o)) = (particles, self.emitters.get(&id)) { type06::emitter_update_live(p, rng, o, self.view, 1, &table.mobys[id]); }
        }
    }
}

/// Diagnostic knobs for [`PortSim::with_options`]. The defaults are the port's rules; the knobs re-create
/// the pre-fix behaviour for comparison.
#[derive(Clone, Debug, Default)]
pub struct SimOptions {
    /// Extra `rand()` draws taken right before the load pass (after `Game::new`'s `srand(1234)` and
    /// `HeroInit`'s fidget-timer draw). Diagnostic: shifts the load pass along the stream.
    pub load_pre_draws: u32,
    /// The load pass's class-27 emitters see the camera's view (visible: one particle, 8 draws, each) instead
    /// of the game's unbuilt view (`None`: culled, `type06::emitter_update`). Diagnostic (the pre-fix port).
    pub load_emitters_visible: bool,
    /// Ticks at the start that run with the game mode 2 (in-engine cutscene: bolts hide, `BoltUpdate`).
    pub cutscene_ticks: u64,
    /// No sound layer (diagnostic: the stream before the sound draws moved onto it).
    pub no_audio: bool,
}

/// The running port.
pub struct PortSim<'l> {
    pub lv: &'l LevelData,
    pub game: Game,
    pub svc: Services,
    pub sched: Scheduler,
    pub particles: Particles,
    pub classes: Arc<ClassTable>,
    pub emitters: HashMap<MobyId, usize>,
    pub anim: RatchetAnim,
    /// Ratchet's gameplay instance and his moby (`0x1acc00[hero_ii]`).
    pub hero_ii: usize,
    pub hero_id: MobyId,
    pub n_static: usize,
    /// Static moby → gameplay instance, and the instances the spawn test rejected.
    pub moby_to_instance: Vec<usize>,
    pub pending: Vec<usize>,
    /// The fidget timer `HeroInit` drew (0x140360 right after the hero init).
    pub fidget_at_init: i32,
    /// The level's mission state (fresh load: the save's mission bytes, deaths cleared).
    pub missions: LevelMissions,
    /// The underwater look 0x161200 as 751 leaves it (`set_from_ripple_zone` every tick; no zone keeps it).
    pub look: rc_game::fog_zones::UnderwaterLook,
    pub ship: Option<MobyId>,
    /// Draws from `srand(1234)` to the end of the load pass (`HeroInit`'s draw included).
    pub load_draws: u64,
    /// `rand()` draws per tick (from the state distance).
    pub draws_per_tick: Vec<u64>,
    /// Of those, the sound step's (trigger sounds + `sound_update`), per tick.
    pub sound_draws: Vec<u64>,
    pub rng_after_load: u32,
    /// The sound layer (None: `SimOptions::no_audio` or no sound data).
    pub audio: Option<AudioSystem>,
    /// The last tick's 800 samples.
    pub samples: Vec<[i16; 2]>,
}

/// Number of `rand()` steps from state `from` to state `to` (the LCG has full period 2^32), by lifting
/// one bit of the step count at a time: the low k bits of the state only depend on the low k bits of n.
pub fn lcg_distance(from: u32, to: u32) -> u64 {
    fn jump(mut s: u32, mut n: u64) -> u32 {
        let (mut a, mut c) = (0x41c6_4e6du32, 0x3039u32);
        while n != 0 {
            if n & 1 != 0 { s = s.wrapping_mul(a).wrapping_add(c); }
            c = a.wrapping_mul(c).wrapping_add(c);
            a = a.wrapping_mul(a);
            n >>= 1;
        }
        s
    }
    let mut n = 0u64;
    for k in 0..32 {
        let m = if k == 31 { u32::MAX } else { (1u32 << (k + 1)) - 1 };
        if jump(from, n) & m != to & m { n |= 1 << k; }
    }
    debug_assert_eq!(jump(from, n), to);
    n
}

impl<'l> PortSim<'l> {
    /// Loader (spawn test, instance → moby map) + `Game::new` (`srand(1234)`, hero init) + services + the load
    /// pass at counter 0 + the load's counter increment.
    pub fn new(lv: &'l LevelData) -> Result<PortSim<'l>> { Self::with_options(lv, &SimOptions::default()) }

    /// [`PortSim::new`] with diagnostic knobs.
    pub fn with_options(lv: &'l LevelData, opt: &SimOptions) -> Result<PortSim<'l>> {
        let mut classes = ClassTable { classes: lv.classes.classes.clone() };
        let hero_ii = lv.instances.iter().position(|m| m.o_class == 0).context("no Ratchet instance")?;
        let statics = load_level_mobys(&lv.instances, &mut classes, &lv.pvars, &lv.spawn);
        let hero_id = statics.instance_to_moby[hero_ii].context("Ratchet not created by the spawn test")?;
        let n_static = statics.mobys.len();
        let mut table = MobyTable::new(statics.mobys.clone(), lv.spawnable);
        // The ship (loader, after the static mobys): CreateMoby, update distance 0x10, draw distance 0xff;
        // hidden (mode |= 3, no collision) while the mission NPC's mission is open.
        let mut ship = None;
        if let Some((p, yaw)) = lv.ship {
            let info = classes.classes.get(&SHIP_CLASS).map(|c| c.0);
            if let Some(id) = table.create(SHIP_CLASS, info.as_ref(), 0) {
                let m = &mut table.mobys[id];
                m.position = [p[0], p[1], p[2], 0.0];
                m.rotation = [0.0, 0.0, yaw, 0.0];
                m.draw_dist = 0xff;
                m.update_dist = 0x10;
                if moby_spawn::ship_hidden_on_arrival(&lv.instances, &lv.spawn, &lv.save.missions) {
                    m.mode |= 3;
                    m.has_collision = false;
                }
                ship = Some(id);
            }
        }
        let mut game = Game::new(&lv.mesh, table, hero_id, GameOptions::default(), lv.death_z);
        let fidget_at_init = game.hero.fidget_timer;
        // The back items (created on the first hero update) and the level for the idle code.
        if let Some((pack, clank)) = &lv.back { game.hero.set_back_classes(pack.clone(), clank.clone()); }
        game.hero.idle.level = 1;
        let mut svc = Services::new();
        svc.level = 1;
        svc.set_splines(&lv.splines);
        svc.joint_lists = lv.joint_lists.clone();
        svc.groups = statics.groups(&lv.gp);
        svc.set_moby_collision(lv.coll_blobs.clone());
        // The level's volumes (cuboids 0x1600ec …), as the engine sets them: the triggers and the foam fields 760 read them.
        svc.set_volumes(rc_formats::volumes::parse_volumes(&lv.gp)?);
        svc.build_grid(&mut game.mobys);
        svc.water = rc_game::water::world::WaterWorld::new(lv.water.clone());
        let mut particles = Particles::new(lv.part_defs.clone(), lv.owners.clone());
        particles.pool.level_init();
        // Owner k belongs to gameplay instance `o.instance`, i.e. to moby 0x1acc00[o.instance].
        let emitters: HashMap<MobyId, usize> =
            lv.owners.iter().enumerate().filter_map(|(k, o)| Some((statics.instance_to_moby.get(o.instance).copied().flatten()?, k))).collect();
        let mut sim = PortSim {
            lv,
            game,
            svc,
            sched: Scheduler::new(),
            particles,
            classes: Arc::new(classes),
            emitters,
            anim: RatchetAnim::new(&lv.ratchet),
            hero_ii,
            hero_id,
            n_static,
            moby_to_instance: statics.moby_to_instance.clone(),
            pending: statics.pending.clone(),
            fidget_at_init,
            missions: LevelMissions::fresh_load(1, lv.save.missions),
            look: Default::default(),
            ship,
            load_draws: 0,
            draws_per_tick: Vec::new(),
            sound_draws: Vec::new(),
            rng_after_load: 0,
            audio: if opt.no_audio { None } else { lv.audio.clone().map(AudioSystem::new) },
            samples: Vec::new(),
        };
        // Diagnostic: extra draws before the load pass (see SimOptions).
        for _ in 0..opt.load_pre_draws { sim.game.rng.rand(); }
        {
            let hero = sim.game.hero.clone();
            // The game's view before the first render (None: culls the Novalis emitters), or the diagnostic.
            let cam_view = view_of(&sim.game.camera.out);
            let view = opt.load_emitters_visible.then_some(&cam_view);
            let mut ext = Externals { emitters: &sim.emitters, view };
            let mut w = World::new(&mut sim.game.mobys, &hero, &mut sim.game.rng, &*sim.classes, &mut sim.svc, sim.game.counter);
            w.camera = sim.game.camera.out.pos;
            w.coll = Some(&lv.mesh);
            w.particles = Some(&mut sim.particles);
            w.external = Some(&mut ext);
            w.missions = &sim.missions;
            sim.sched.load_pass(&mut w);
        }
        // LoadLevelCoreData: 0x15f5cc++ after the load pass.
        sim.game.finish_load();
        sim.load_draws = lcg_distance(rc_game::rng::LEVEL_SEED, sim.game.rng.state);
        sim.rng_after_load = sim.game.rng.state;
        Ok(sim)
    }

    /// One gameplay tick with `pad`, as rc-engine's `gameplay::tick`.
    pub fn tick(&mut self, pad: PadInput) {
        let lv = self.lv;
        let r0 = self.game.rng.state;
        let cam = self.game.camera.out;
        let view = view_of(&cam);
        standin_visible(&mut self.game.mobys, cam.pos_f32());
        let svc = RefCell::new(&mut self.svc);
        let parts = RefCell::new(&mut self.particles);
        let (sched, classes, emitters, missions) = (&mut self.sched, &self.classes, &self.emitters, &self.missions);
        let audio = RefCell::new(self.audio.as_mut());
        let hero_id = self.hero_id;
        let mut mobys = |table: &mut MobyTable, hero: &Hero, rng: &mut Rng, cam: &CameraView, coll: &collision::Collision, counter: u64| {
            let mut s = svc.borrow_mut();
            let mut p = parts.borrow_mut();
            let mut a = audio.borrow_mut();
            let mut sink = a.as_deref_mut().map(|a| ClassSoundSink { audio: a, listener: class_sounds::listener_of(cam), hero: Some(hero_id) });
            let mut ext = Externals { emitters, view: Some(&view) };
            let mut w = World::new(table, hero, rng, &**classes, &mut s, counter);
            w.sound = sink.as_mut().map(|s| s as &mut dyn rc_game::moby_update::services::SoundSink);
            w.camera = cam.pos;
            w.coll = Some(coll);
            w.particles = Some(&mut **p);
            w.view = Some(&view);
            w.external = Some(&mut ext);
            w.missions = missions;
            sched.tick(&mut w);
        };
        // UpdateParts + glints (FUN_00220928) on the game's stream, between the hero and the camera.
        let mut partsf = |_: &Hero, _: &CameraView, rng: &mut Rng, _: u64| {
            parts.borrow_mut().update_parts(rng);
            svc.borrow_mut().glints.update();
        };
        let classes_dyn: Arc<dyn rc_game::moby_update::ClassData + Send + Sync> = self.classes.clone();
        let mut world = rc_game::moby_update::services::SharedServices { svc: &svc, classes: classes_dyn };
        let mut hooks = TickHooks { mobys: &mut mobys, particles: &mut partsf, world: Some(&mut world) };
        // The sound step (after the camera, before the counter increment).
        let samples = &mut self.samples;
        samples.clear();
        let mut sound_n = 0;
        let mut sound = |table: &MobyTable, hero: &Hero, cam: &CameraView, rng: &mut Rng, counter: u64| {
            let mut a = audio.borrow_mut();
            let Some(a) = a.as_deref_mut() else { return };
            let r = rng.state;
            class_sounds::sound_step(a, table, hero, cam, rng, counter, samples);
            sound_n = lcg_distance(r, rng.state);
        };
        let has_audio = audio.borrow().is_some();
        self.game.hero.idle.counter = self.game.counter as i32;
        let mut anim = self.anim.ctl(&lv.ratchet);
        let mut hits = rc_game::hero::items::NoHits;
        // Ratchet's own sounds (his animation triggers, his voices) inside the hero update, with the listener the
        // hero sees (the previous tick's camera).
        let mut hero_sounds = HeroClassSounds {
            audio: || std::cell::RefMut::filter_map(audio.borrow_mut(), |a| a.as_deref_mut()).ok(),
            class: &lv.ratchet,
            body_classes: None,
            listener: class_sounds::listener_of(&cam),
            hero: hero_id,
            counter: self.game.counter,
        };
        let sound = if has_audio { Some(&mut sound as &mut rc_game::tick::SoundHook) } else { None };
        self.game.tick_with_hero_sounds(Some(&pad.bytes()), &lv.mesh, &mut anim, &mut hooks, &mut hits, sound, &mut hero_sounds);
        self.sound_draws.push(sound_n);
        self.draws_per_tick.push(lcg_distance(r0, self.game.rng.state));
        // 751's per-tick colours (the moby system's water: `Services::water::look`).
        self.look = self.svc.water.look;
    }
}

/// The emitter cull view (0x16d140) from a camera.
pub fn view_of(cam: &CameraView) -> BSphereView {
    let r = cam.rows_f32();
    BSphereView::from_camera(cam.pos_f32(), r[0], r[1], r[2], TAN_HALF_FOV_X, TAN_HALF_FOV_X * NTSC_Y_RATIO)
}

/// +0x31 stand-in (see the module doc).
pub fn standin_visible(t: &mut MobyTable, cam: [f32; 3]) {
    for m in t.mobys.iter_mut() {
        if m.state == 0xff { break; }
        let d = ((m.position[0] - cam[0]).powi(2) + (m.position[1] - cam[1]).powi(2) + (m.position[2] - cam[2]).powi(2)).sqrt();
        m.visible = (m.state < 0x80 && m.mode & 0x81 == 0 && d <= m.draw_dist as f32) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::lcg_distance;

    #[test]
    fn lcg_distance_counts_steps() {
        let mut r = rc_game::rng::Rng::new();
        r.srand(1234);
        for n in [0u64, 1, 2, 3, 1000, 254_936] {
            let mut s = rc_game::rng::Rng::new();
            s.srand(1234);
            for _ in 0..n { s.rand(); }
            assert_eq!(lcg_distance(1234, s.state), n);
        }
        assert_eq!(lcg_distance(r.state, r.state), 0);
    }
}
