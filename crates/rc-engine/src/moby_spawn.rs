//! Moby state at the end of level load (docs/plan/moby_update_catalogue.md, "In the port"): the level
//! loader's ship and the result of the load-time update pass (`FUN_002792d0`), from the pure rules in
//! `rc_formats::moby_spawn`, applied without porting the update functions.
//!
//! * [`apply_load_pass`] (before the app starts, on the loaded level): runs the loader's spawn test
//!   (`rc_formats::moby_spawn::loader_spawns` with the level's save bits, `crate::gameplay::level_spawn_save`, the
//!   ones the game's moby table reads; a rejected instance is hidden),
//!   computes every created instance's `SpawnState` from its pvars (`rc_formats::gameplay::parse_pvars_spawned`:
//!   moby links through the instance → moby map), the level's splines and a ground
//!   probe; writes the moved positions into the instance records (the renderer builds its model matrices
//!   from them); appends the ship the loader creates (`CreateMoby(SHIP_CLASSES[ship])`, ship = the level's
//!   first-visit index or `RC_SHIP`, its class from the level core or the `spaceships` file; at level
//!   settings +0x2c, yaw +0x38,
//!   the `InitMobyInstance` render defaults: light word 0, ambient 0x40, occlusion always visible) as one
//!   more instance with its light block; hidden while the level's mission NPC has its mission open
//!   (`moby_spawn::ship_hidden_on_arrival`: the first arrival on Novalis).
//! * `init` (PostStartup, once the moby renderer has built `MobyAnim`): sets each animated instance's state
//!   to its after-load state, queues the first-update changes and tags the entities of hidden instances
//!   with [`SpawnHidden`].
//! * `force_hidden` (PostUpdate, after `moby_render::update_moby_occlusion`, before visibility
//!   propagation): keeps `Visibility::Hidden` on [`SpawnHidden`] entities. The occlusion system writes
//!   visibility only when its own result changes, so re-hiding right after it makes the hide win.
//!
//! The ground probe (`FUN_0026e618`, used by enemy 459) is a plain f32 segment test against the collision
//! triangles, one-sided like `CollLine_Fix` with flags 2 (face front = +N, N = (v2 − v0) × (v1 − v0), world
//! mesh only); not the bit-exact kernel of `rc-game`, which the engine does not link. Its margins on
//! Novalis are printed at load.
//!
//! With the game tick (crate::gameplay) these states are used for the classes without a ported update only:
//! the moby scheduler's table drives the ported ones (bolts, crates, grass) after its load pass
//! (`MobyAnim::drive`, `MobyOcclusion::drive`); `initial_state` leaves those classes plain.
//!
//! `RC_SPAWN_RULES=0` restores the old behaviour: every instance plays sequence 0 from the spawn state,
//! nothing hidden or moved, no ship.

use crate::level_load::LoadedLevel;
use crate::moby_anim::{MobyAnim, PendingChange};
use crate::moby_render::{MobyMaterial, Placed};
use anyhow::{Context, Result};
use bevy::camera::visibility::VisibilitySystems;
use bevy::mesh::MeshTag;
use bevy::prelude::*;
use rc_formats::collision::{self, CollisionTriangle};
use rc_formats::gameplay::{self, MobyInstance, MOBY_INSTANCE_SIZE};
use rc_formats::level::{ClassEntry, TextureEntry};
use rc_formats::moby::{self, LevelMobyClass};
use rc_formats::texture::{LevelTexture, TextureSource, TextureTable};
use rc_formats::moby_anim::{self, MobyAnimClass};
use rc_formats::moby_spawn::{self, ChangeAt, SpawnEnv, SpawnState};
use rc_formats::occlusion::OcclBits;
use rc_formats::tfrag_light::ps2;
use rc_formats::level;
use std::cell::OnceCell;
use std::collections::BTreeMap;
use std::path::Path;

/// Per gameplay instance (plus the appended ship): the state after the load pass.
#[derive(Resource)]
pub struct MobySpawn {
    pub enabled: bool,
    pub states: Vec<SpawnState>,
    /// Instance index of the loader's ship, when one was created.
    pub ship: Option<usize>,
}

/// Marks the entities of an instance that is hidden after the load pass (mode bit 1).
#[derive(Component)]
pub struct SpawnHidden;

/// `RC_SPAWN_RULES` (default on).
pub fn enabled() -> bool { !std::env::var("RC_SPAWN_RULES").is_ok_and(|v| v.trim() == "0") }

struct Env<'a> {
    splines: Vec<Vec<[f32; 4]>>,
    positions: Vec<[f32; 3]>,
    root: &'a Path,
    index: u32,
    triangles: OnceCell<Vec<CollisionTriangle>>,
    /// (instance position, ground) of every probe, for the report.
    probes: std::cell::RefCell<Vec<([f32; 3], f32)>>,
}

impl Env<'_> {
    fn triangles(&self) -> &[CollisionTriangle] {
        self.triangles.get_or_init(|| {
            let load = || -> Result<Vec<CollisionTriangle>> {
                let read = |n: &str| crate::disc_source::level_file(self.root, self.index, n);
                let core_data = rc_data::level_core_data(self.root, self.index).context("decompressing core_data")?;
                let core = level::parse_level_core(&read("core_index.bin")?, core_data.len())?;
                Ok(collision::collision_triangles(&collision::parse_collision(&core, &core_data)?))
            };
            load().unwrap_or_else(|e| { warn!("moby spawn: no collision for the ground probe ({e:#}); every probe misses"); Vec::new() })
        })
    }
}

type P3 = [f32; 3];
fn sub(a: P3, b: P3) -> P3 { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn cross(a: P3, b: P3) -> P3 { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn dot(a: P3, b: P3) -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }

/// Nearest front-face hit of the segment a → b (`CollLine_Fix`'s rules, f32): the t in [0, 1).
fn segment_hit(tris: &[CollisionTriangle], a: P3, b: P3) -> Option<f32> {
    let d = sub(b, a);
    let mut best = 1.0f32;
    let mut hit = None;
    for tri in tris {
        let (v0, v1, v2) = (tri.a, tri.b, tri.c);
        let (e1, e2) = (sub(v1, v0), sub(v2, v0));
        let n = cross(e2, e1);
        let (s0, s1) = (dot(sub(v0, a), n), dot(sub(v0, b), n));
        if !(s0 < 0.0 && s1 > 0.0) { continue; }
        let t = s0 / dot(d, n);
        let p = sub([a[0] + d[0] * t, a[1] + d[1] * t, a[2] + d[2] * t], v0);
        if dot(cross(p, e1), n) < 0.0 || dot(cross(e2, p), n) < 0.0 || dot(cross(sub(p, e1), sub(e2, e1)), n) < 0.0 { continue; }
        if best - t > 0.0 { best = t; hit = Some(t); }
    }
    hit
}

impl SpawnEnv for Env<'_> {
    fn spline(&self, index: i32) -> Option<&[[f32; 4]]> { self.splines.get(usize::try_from(index).ok()?).map(Vec::as_slice) }
    fn ground_height(&self, pos: [f32; 3]) -> f32 {
        let a = [pos[0], pos[1], pos[2] + 0.5];
        let b = [pos[0], pos[1], 0.01];
        let in_world = |p: P3| p.iter().all(|&c| (0.0..1024.0).contains(&c));
        let g = if in_world(a) && in_world(b) { segment_hit(self.triangles(), a, b).map_or(0.0, |t| a[2] + (b[2] - a[2]) * t) } else { 0.0 };
        self.probes.borrow_mut().push((pos, g));
        g
    }
    fn moby_position(&self, index: i32) -> Option<[f32; 3]> { self.positions.get(usize::try_from(index).ok()?).copied() }
}

/// Computes the load-pass result, moves instances, appends the ship. Prints per-class counts.
pub fn apply_load_pass(root: &Path, index: u32, level: &mut LoadedLevel, save: &moby_spawn::SpawnSave) -> Result<MobySpawn> {
    let m = &mut level.mobys;
    let empty = MobyAnimClass { joint_count: 0, skeleton: vec![], rest: vec![], parent_word: vec![], sequences: vec![] };
    let by_class: BTreeMap<i32, usize> = m.classes.iter().enumerate().map(|(i, c)| (c.o_class, i)).collect();
    let anim_of = |o: i32| by_class.get(&o).map_or(&empty, |&c| &m.anim[c]);
    if !enabled() {
        println!("moby spawn: RC_SPAWN_RULES=0: every instance plays sequence 0 from its spawn state, nothing hidden or moved, no ship");
        let states = m.instances.iter().map(|i| { let mut s = SpawnState::plain(i.o_class, anim_of(i.o_class)); s.runs_load_pass = false; s }).collect();
        return Ok(MobySpawn { enabled: false, states, ship: None });
    }
    let gameplay = rc_data::level_gameplay(root, index).context("decompressing gameplay_ntsc")?;
    // The loader's spawn test with the level's save bits (`crate::gameplay::level_spawn_save`: the same ones the game's
    // moby table is built with). Records it rejects are not created: hidden here; the pvar moby links and moby
    // positions use the runtime indices.
    let tests = moby_spawn::loader_spawns(&m.instances, &mut save.clone());
    let spawned: Vec<bool> = tests.iter().map(|t| t.spawn).collect();
    let pvars = gameplay::parse_pvars_spawned(&gameplay, &spawned).context("parsing pvars")?;
    let env = Env {
        splines: gameplay::parse_splines(&gameplay).context("parsing paths")?,
        positions: m.instances.iter().zip(&tests).filter(|(_, t)| t.spawn).map(|(i, _)| i.position).collect(),
        root,
        index,
        triangles: OnceCell::new(),
        probes: Default::default(),
    };
    // With the game tick (crate::gameplay) a class with a ported update runs its own init in the scheduler's load pass:
    // its state stays plain here, or the init's moves / hides would be applied twice (577's lift, 865/866's drop).
    let play = std::env::var("RC_PLAY").map_or(true, |v| v.trim() != "0");
    let mut states: Vec<SpawnState> = m.instances.iter().zip(&tests).map(|(i, t)| {
        if t.spawn && play && crate::gameplay::level_ports().get(i.o_class as i16).is_some() { return SpawnState::plain(i.o_class, anim_of(i.o_class)); }
        if t.spawn { return moby_spawn::initial_state(i.o_class, i, i.pvar(&pvars), anim_of(i.o_class), &env); }
        let mut s = SpawnState::plain(i.o_class, anim_of(i.o_class));
        (s.hidden, s.runs_load_pass) = (true, false);
        s
    }).collect();
    let not_created = tests.iter().filter(|t| !t.spawn).count();
    for (i, s) in m.instances.iter_mut().zip(&states) {
        if let Some(p) = s.position { i.position = p; }
    }

    // The loader's ship: SHIP_CLASSES[ship] (first-visit index of this level, RC_SHIP overrides), from
    // the level core when it has the class, else from the spaceships file (FUN_00253a18 registers it).
    let mut ship = None;
    let ship_index = ship_index(index);
    let ship_class = moby_spawn::SHIP_CLASSES[ship_index];
    // `InitLevelRenderGlobals`: on level 13 with ship 2 and 0x160540 set (Gemlik's placed 533 adopted after the escape) the
    // placed ship is adopted again (crate::gameplay) and none is created.
    let adopts = index == 13 && ship_index == 2 && crate::level_load::ship_adopted();
    if adopts { println!("moby spawn: level 13 with ship 2 after the escape (0x160540): the placed ship 533 is the ship, none created"); }
    match if adopts { None } else { gameplay::ship_placement(&gameplay)? } {
        Some((pos, yaw)) => {
            let (ci, from) = match by_class.get(&ship_class) {
                Some(&ci) => (Some(ci), "level core"),
                None => (load_spaceship(root, ship_index, level).map_err(|e| warn!("moby spawn: ship class {ship_class} not loaded from the spaceships file: {e:#}")).ok(), "spaceships file"),
            };
            let m = &mut level.mobys;
            if let Some(ci) = ci {
                let inst = MobyInstance {
                    size: MOBY_INSTANCE_SIZE as i32,
                    unknown_4: -1,
                    spawn_id: -1,
                    o_class: ship_class,
                    scale: 1.0,
                    draw_distance: 0xff,
                    update_distance: 0x10,
                    position: pos,
                    rotation: [0.0, 0.0, yaw],
                    group: -1,
                    pvar_index: -1,
                    occlusion: 1,
                    color: [0x40; 3],
                    unknown_74: -1,
                    ..Default::default()
                };
                let class = &m.classes[ci];
                let rows = crate::moby_light::instance_rows(&inst);
                let lights = m.lighting.as_ref().map(|l| crate::moby_light::instance_lights(l, &rows, &inst));
                let scale = f32::from_bits(ps2::mul(class.class.header.scale.to_bits(), inst.scale.to_bits()));
                m.placed.push(Some(Placed { class: ci, rows: rows.map(|r| [0, 1, 2].map(|k| f32::from_bits(r[k]))), scale, lights, colors: None }));
                m.instances.push(inst);
                level.occlusion.objects.moby.push(OcclBits::ALWAYS);
                let n = tests.len();
                let hidden = moby_spawn::ship_hidden_on_arrival(&m.instances[..n], &tests, &save.missions);
                states.push(moby_spawn::ship_state(ship_class, &m.anim[ci], hidden));
                ship = Some(m.instances.len() - 1);
                println!(
                    "moby spawn: ship {ship_index} (class {ship_class}, from the {from}) created at ({:.2}, {:.2}, {:.2}), yaw {yaw:.4} (level settings +0x2c), sequence 1 by hard cut{}",
                    pos[0], pos[1], pos[2], if hidden { "; hidden: the mission NPC's mission is open (first arrival)" } else { "" }
                );
            }
        }
        None => println!("moby spawn: no ship in this level (level settings +0x2c <= 0)"),
    }

    println!("moby spawn: spawn test: {} of {} instances created, {not_created} not (hidden)", tests.len() - not_created, tests.len());
    report(&level.mobys.instances, &states, &env);
    Ok(MobySpawn { enabled: true, states, ship })
}

/// `RC_SHIP` (0, 1 or 2), else 0x13e056 of the runtime level change, else the level's first-visit ship index
/// (`moby_spawn::first_visit_ship`).
pub(crate) fn ship_index_for(level: u32) -> usize { ship_index(level) }

fn ship_index(level: u32) -> usize {
    match std::env::var("RC_SHIP").ok().and_then(|v| v.trim().parse::<usize>().ok()) {
        Some(i) if i < moby_spawn::SHIP_CLASSES.len() => i,
        // 0x13e056 as `DoSpaceTransition` set it for this load (crate::level_load::set_ship), else the level's first-visit ship.
        _ => crate::level_load::ship().unwrap_or_else(|| moby_spawn::first_visit_ship(level)),
    }
}

/// Registers the ship class of `spaceships` entry `ship + 1` like `FUN_00253a18`: the class with its
/// sequences, and its one texture appended to the moby texture table (slot 0 → the new index, the
/// descriptor 0x15fc00 the loader fills). Returns the class index. The file is read through
/// `disc_source::read` as the extractor names it (`global/spaceships/NNN.bin`); the disc source serves
/// only level and boot files, so this comes from `extracted/`.
fn load_spaceship(root: &Path, ship: usize, level: &mut LoadedLevel) -> Result<usize> {
    let entry = moby_spawn::spaceships_entry(ship);
    let file = crate::disc_source::read(root, &format!("global/spaceships/{entry:03}.bin"))?;
    let s = moby_spawn::parse_spaceship(&file)?;
    let class = moby::parse_moby_class(s.class).context("parsing the ship class")?;
    let seqs = moby_anim::parse_sequences(s.class, &class).context("parsing the ship sequences")?;
    let tex = level.textures.iter().filter(|t| t.table == TextureTable::Moby).map(|t| t.index + 1).max().unwrap_or(0);
    let slot = u8::try_from(tex).ok().filter(|&t| t != 0xff).context("moby texture table full")?;
    let mut textures = [0xff; 16];
    textures[0] = slot;
    let o_class = moby_spawn::SHIP_CLASSES[ship];
    let desc = TextureEntry { data_offset: 0, width: 256, height: 256, ty: 4, palette: 0, mipmap: 0, pad: 0 };
    level.textures.push(LevelTexture { table: TextureTable::Moby, index: tex, source: TextureSource::Entry(desc), texture: s.texture });
    let m = &mut level.mobys;
    m.anim.push(MobyAnimClass::new(&class, seqs));
    m.classes.push(LevelMobyClass { o_class, entry: ClassEntry { offset_in_asset_wad: 0, o_class, unknown_8: 0, unknown_c: 0, textures }, class });
    Ok(m.classes.len() - 1)
}

fn report(instances: &[MobyInstance], states: &[SpawnState], env: &Env) {
    #[derive(Default)]
    struct C { n: usize, hidden: usize, deleted: usize, moved: usize, load: usize, first: usize, create: usize }
    let mut per: BTreeMap<i32, C> = BTreeMap::new();
    for (i, s) in instances.iter().zip(states) {
        let c = per.entry(i.o_class).or_default();
        c.n += 1;
        c.hidden += s.hidden as usize;
        c.deleted += s.deleted as usize;
        c.moved += s.position.is_some() as usize;
        match s.change_at {
            Some(ChangeAt::LoadPass) => c.load += 1,
            Some(ChangeAt::FirstUpdate) => c.first += 1,
            Some(ChangeAt::Create) => c.create += 1,
            None => {}
        }
    }
    let tot = |f: fn(&C) -> usize| per.values().map(f).sum::<usize>();
    println!(
        "moby spawn: {} instances: {} hidden ({} deleted), {} moved, sequence changes: {} at creation, {} in the load pass, {} at the first update",
        states.len(), tot(|c| c.hidden), tot(|c| c.deleted), tot(|c| c.moved), tot(|c| c.create), tot(|c| c.load), tot(|c| c.first)
    );
    for (o, c) in per.iter().filter(|(_, c)| c.hidden + c.moved + c.load + c.first + c.create > 0) {
        let s = states.iter().zip(instances).find(|(s, i)| i.o_class == *o && s.change_at.is_some()).map(|(s, _)| s);
        let seq = s.map(|s| format!(", seq {} {}", s.sequence, s.blend_ticks.map_or("cut".to_string(), |b| format!("blend {b}")))).unwrap_or_default();
        println!(
            "  class {o:4}: {:3} placed, {:3} hidden, {:3} moved, {} load-pass / {} first-update / {} creation sequence changes{seq}",
            c.n, c.hidden, c.moved, c.load, c.first, c.create
        );
    }
    let probes = env.probes.borrow();
    if !probes.is_empty() {
        let margins: Vec<String> = probes.iter().map(|(p, g)| format!("{:+.2}", p[2] - g - 0.5)).collect();
        println!("  ground probes (z - ground - 0.5; > 0 = hidden): {}", margins.join(" "));
    }
}

pub struct MobySpawnPlugin;

impl Plugin for MobySpawnPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(crate::level_switch::LevelPostStartup, init).add_systems(
            PostUpdate,
            force_hidden.after(crate::moby_render::update_moby_occlusion).before(VisibilitySystems::VisibilityPropagate),
        );
    }
}

/// Anim instance k belongs to the k-th instance with geometry (`moby_render::spawn_mobys` pushes one
/// `AnimInstance` per placed instance, in instance order).
fn anim_indices(level: &LoadedLevel) -> Vec<Option<usize>> {
    let mut k = 0;
    level.mobys.placed.iter().map(|p| p.as_ref().map(|_| { k += 1; k - 1 })).collect()
}

fn init(
    mut commands: Commands,
    spawn: Option<Res<MobySpawn>>,
    level: Res<crate::Level>,
    anim: Option<ResMut<MobyAnim>>,
    entities: Query<(Entity, &MeshTag), With<MeshMaterial3d<MobyMaterial>>>,
) {
    let Some(spawn) = spawn else { return };
    if !spawn.enabled { return; }
    let lv = &level.0;
    let mut hidden_entities = 0;
    for (e, tag) in &entities {
        if spawn.states.get(tag.0 as usize).is_some_and(|s| s.hidden) {
            commands.entity(e).insert((SpawnHidden, Visibility::Hidden));
            hidden_entities += 1;
        }
    }
    let Some(mut anim) = anim else { return };
    let idx = anim_indices(lv);
    if anim.instances.len() != idx.iter().flatten().count() {
        warn!("moby spawn: {} animation instances for {} placed instances; spawn states not applied", anim.instances.len(), idx.iter().flatten().count());
        return;
    }
    let (mut pending, mut changed) = (0, 0);
    for (ii, (st, k)) in spawn.states.iter().zip(&idx).enumerate() {
        let Some(k) = *k else { continue };
        let class = &lv.mobys.anim[anim.instances[k].class];
        let (s, snap) = st.anim_after_load(class);
        changed += (s.seq_b != 0) as usize;
        anim.instances[k].state = s;
        anim.snapshots[k] = snap;
        anim.hidden[k] = st.hidden;
        if st.change_at == Some(ChangeAt::FirstUpdate) && !st.hidden {
            let inst = &lv.mobys.instances[ii];
            anim.pending[k] = Some(PendingChange { spawn: st.clone(), position: Vec3::from(inst.position), update_distance: inst.update_distance as u8 });
            pending += 1;
        }
    }
    anim.pose_dirty = true;
    if let Some(k) = spawn.ship.and_then(|i| idx.get(i).copied().flatten()) {
        let s = &anim.instances[k].state;
        println!("moby spawn: ship after the load pass: seq A {} key {} -> seq B {} key {}, t {}", s.seq_a, s.frame_a, s.seq_b, s.frame_b, s.t);
    }
    println!(
        "moby spawn: {} animation states set from the load pass ({} not on sequence 0), {} first-update changes queued, {} entities hidden",
        idx.iter().flatten().count(), changed, pending, hidden_entities
    );
}

/// Hidden wins over the occlusion system's un-hide (see the module doc).
fn force_hidden(mut q: Query<&mut Visibility, With<SpawnHidden>>) {
    for mut v in &mut q {
        if *v != Visibility::Hidden { *v = Visibility::Hidden; }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_hits_the_front_face_only() {
        // A floor at z = 10 whose front (+N, N = (v2-v0) x (v1-v0)) faces up.
        let tri = CollisionTriangle { a: [0.0, 0.0, 10.0], b: [0.0, 10.0, 10.0], c: [10.0, 0.0, 10.0], ..Default::default() };
        let n = cross(sub(tri.c, tri.a), sub(tri.b, tri.a));
        assert!(n[2] > 0.0);
        let t = segment_hit(&[tri], [2.0, 2.0, 20.0], [2.0, 2.0, 0.0]).unwrap();
        assert!((20.0 - 20.0 * t - 10.0).abs() < 1e-4);
        // From below: back face, no hit; outside the triangle: no hit.
        assert!(segment_hit(&[tri], [2.0, 2.0, 0.0], [2.0, 2.0, 20.0]).is_none());
        assert!(segment_hit(&[tri], [9.0, 9.0, 20.0], [9.0, 9.0, 0.0]).is_none());
    }
}
