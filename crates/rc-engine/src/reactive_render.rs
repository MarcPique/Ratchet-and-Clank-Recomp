//! The Suck Cannon's vortex, the Taunter's sound-wave rings and the Morph-o-Ray's beam: their draw callbacks
//! (`0x306158`, draw list 2; `0x2cd1d0`, list 1; `0x2d38b8`, list 2 (`RegisterDrawCallback2`)) as `FastDrawQuadReal`
//! quads through the shared draw-callback material of crate::fx_draw (additive, ALPHA 0x48). The quads come from rc-game
//! (`rc_game::hero::suck_vortex::Vortex::quads`, `rc_game::hero::taunter::Rings::quads`,
//! `rc_game::hero::morph_ray::Beam::quads`: one group per FX texture, in the callback's order); they are drawn on the
//! ticks their updates registered the callback.

use crate::fx_draw::{FxAssets, FxGroup, FxPrimMaterial, FxSlots, PrimBuf, LIST1_BIAS, LIST2_BIAS};
use crate::game_camera::{GameFog, TfragFog};
use bevy::prelude::*;

pub struct ReactivePlugin;

impl Plugin for ReactivePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ReactiveDraw>().add_systems(crate::level_switch::LevelUnload, crate::level_switch::reset::<ReactiveDraw>).add_systems(PostUpdate, draw.before(bevy::asset::AssetEventSystems));
    }
}

#[derive(Resource, Default)]
struct ReactiveDraw {
    vortex: FxSlots,
    rings: FxSlots,
    morph: FxSlots,
    drawn: Option<u64>,
}

#[allow(clippy::too_many_arguments)]
fn draw(
    mut commands: Commands,
    mut state: ResMut<ReactiveDraw>,
    play: Option<Res<crate::gameplay::Play>>,
    fog: Option<Res<GameFog>>,
    level: Res<crate::Level>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<FxPrimMaterial>>,
    mut vis: Query<&mut Visibility>,
) {
    let counter = play.as_deref().map(|p| p.game.counter);
    if counter == state.drawn && counter.is_some() { return; }
    state.drawn = counter;
    let mut vortex: Vec<FxGroup> = Vec::new();
    let mut rings: Vec<FxGroup> = Vec::new();
    let mut morph: Vec<FxGroup> = Vec::new();
    if let Some(p) = play.as_deref().filter(|p| p.svc.game_mode != 2) {
        let last = p.game.counter.wrapping_sub(1);
        let r = &p.game.hero.weapons.reactive;
        let id = p.game.hero.items.slot.id;
        if id == rc_game::hero::suck_cannon::SUCK_CANNON && r.suck.vortex.drawn == Some(last) {
            let mut g = FxGroup { fx: rc_game::hero::suck_vortex::FX, additive: true, subtract: false, prims: PrimBuf::default() };
            for q in r.suck.vortex.quads(p.game.camera.out.pos_f32()) { g.prims.quad(q.corners, q.st, q.rgba); }
            vortex.push(g);
        }
        if id == rc_game::hero::taunter::TAUNTER && r.taunter.rings.drawn == Some(last) {
            let mut g = FxGroup { fx: rc_game::hero::taunter::RING_FX, additive: true, subtract: false, prims: PrimBuf::default() };
            for (c, st, rgba) in r.taunter.rings.quads() { g.prims.quad(c, st, rgba); }
            rings.push(g);
        }
        if id == rc_game::hero::morph_ray::MORPH && r.morph.beam.drawn == Some(last) {
            for (fx, c, st, rgba) in r.morph.beam.quads(p.game.camera.out.pos_f32()) {
                if morph.last().is_none_or(|g| g.fx != fx) { morph.push(FxGroup { fx, additive: true, subtract: false, prims: PrimBuf::default() }); }
                morph.last_mut().unwrap().prims.quad(c, st, rgba);
            }
        }
    }
    let fog = fog.map(|f| f.uniform).unwrap_or_else(|| TfragFog::new(&level.0.fog));
    let tex = level.0.particles.textures.as_ref().map(|t| t.fx_textures.as_slice());
    let mut a = FxAssets { meshes: &mut meshes, images: &mut images, materials: &mut materials, fx: tex, fog };
    state.vortex.show(&mut commands, &mut vis, &mut a, vortex, LIST2_BIAS + 64.0, "suck vortex");
    state.rings.show(&mut commands, &mut vis, &mut a, rings, LIST1_BIAS + 128.0, "taunter rings");
    state.morph.show(&mut commands, &mut vis, &mut a, morph, LIST2_BIAS + 96.0, "morph beam");
}
