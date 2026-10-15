//! Render-to-texture for in-world screens and 3D previews: a 3D view of some moby entities rendered into an offscreen
//! image and composed onto the display over a rectangle of game pixels, under or over the HUD ([`Composite`]). The
//! vendor's item panel and salesman screens use it (crate::vendor_render), under the HUD; the page menu's 3D widgets
//! (crate::menu_models) over it.
//!
//! **What it reproduces.** The game draws such a screen into a VRAM render target with the frame's camera but its own
//! projection (`SetRenderToTextureView`: a zoom and a centre in the target), clears it, draws `DrawMobyList(m, 1)`,
//! and copies a texel rectangle of the target onto a screen rectangle (a sprite: texels scaled by the rectangle's size).
//! Composed, that is one pinhole projection with its own focal lengths and principal point on the display. The canvas
//! renders exactly that projection at display resolution: the camera carries the main camera's transform (the
//! game's), a `GameProjection` whose tangents give the focal lengths, and a `SubCameraView` that moves the view axis to
//! the principal point; the image is cleared to the canvas colour; a UI node shows the rectangle.
//!
//! **API** ([`Canvases`], a resource):
//! * [`Canvases::create`]`(commands, images, name) → CanvasId` once (a window-sized image, a UI node, a render layer of
//!   its own), composed under the HUD; [`Canvases::create_over_hud`] for one composed over it (below);
//! * [`Canvases::layer`]`(id)`: the `RenderLayers` the caller puts on the moby entities this canvas shows (e.g.
//!   `moby_render::ExtraMobys::spawn` entities; they must not be on layer 0);
//! * [`Canvases::show`]`(id, Some(CanvasView { rect, focal, centre, clear }))` every frame it is visible, `None` to
//!   hide it (the camera is despawned: other systems take "the" `Camera3d`, and this one carries the main transform).
//!   `rect` = x, y, w, h in game pixels of the 512×416 display; `focal` = pixels per unit of x/z and y/z; `centre` = the
//!   game pixel of the view axis. [`CanvasView::from_target`] builds it from a game render target (size, zoom tangents,
//!   the texel rectangle shown and the display rectangle it is drawn on). [`Canvases::show_exact`] also takes the
//!   projection tangents themselves (a caller using the game camera's own projection: bit-identical to it).
//!
//! **Composition order** ([`Composite`], chosen at creation): where the canvas lands relative to the HUD's 2D pass
//! (crate::hud_render's composite, which also carries the menus' 2D draws):
//! * [`Composite::UnderHud`] ([`Canvases::create`]): the vendor's item panel and salesman screens. The game draws them
//!   in the frame's 3D pass and the HUD (its bolt counter and ammo slot, `HudUpdate(1)` in mode 5) after them
//!   (docs/plan/interaction.md §9) — the node sits under the HUD composite (`GlobalZIndex(i32::MAX − 2)`; the menu
//!   layer is `MAX − 1`, the HUD `MAX`).
//! * [`Composite::OverHud`] ([`Canvases::create_over_hud`]): a page menu widget's 3D view (the Gadgets / Weapons pages'
//!   3D Ratchet and item preview, crate::menu_models). `PageMenuDraw` 0x28d080 draws each panel's navy rect first
//!   (step 5), then the widget's render target copied onto it (step 6; docs/plan/menus.md §3) [H]; in the port the navy
//!   rects and every other widget are HUD 2D primitives, so the view must land above the HUD composite: the node is a
//!   child of it (UI children draw over their parent). No HUD primitive shares a 3D widget's rectangle.
//!
//! The canvas camera renders before the main camera (order −4) into its own image either way. The HUD's static layer
//! (crate::hud_render, `Hud2dHook::statics`: the monitors' and panels' noise and scan lines) is composed over every
//! canvas.

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, RenderTarget, SubCameraView};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::Msaa;
use bevy::transform::TransformSystems;

use crate::game_camera::{GameProjection, SCREEN_H, SCREEN_W};

/// The first render layer the canvases take (one each).
const FIRST_LAYER: usize = 40;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CanvasId(usize);

/// Where a canvas is composed relative to the HUD's 2D pass (module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Composite {
    UnderHud,
    OverHud,
}

/// Where and how a canvas shows this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasView {
    /// Display rectangle (game pixels): x, y, w, h.
    pub rect: [f32; 4],
    /// Pixels per unit of x/z, y/z.
    pub focal: [f32; 2],
    /// Game pixel of the view axis.
    pub centre: [f32; 2],
    pub clear: Color,
}

impl CanvasView {
    /// A game render target of `size` texels drawn with the zoom tangents `tan` and its centre at the middle, whose texels
    /// (0, 0)..`texels` are drawn on the display rectangle `rect` (`DrawBoneQuads`: u → x + u·w / texels.x).
    #[allow(dead_code)]
    pub fn from_target(size: (f32, f32), tan: (f32, f32), texels: (f32, f32), rect: [f32; 4], clear: Color) -> CanvasView {
        let (sx, sy) = (rect[2] / texels.0.max(1.0), rect[3] / texels.1.max(1.0));
        let (hw, hh) = (size.0 * 0.5, size.1 * 0.5);
        CanvasView { rect, focal: [hw / tan.0 * sx, hh / tan.1 * sy], centre: [rect[0] + hw * sx, rect[1] + hh * sy], clear }
    }

    /// The `GameProjection` tangents and the `SubCameraView` offset on the 512×416 display.
    fn camera(&self) -> (f32, f32, Vec2) {
        let tan_x = (SCREEN_W * 0.5) / self.focal[0].max(1e-3);
        let tan_y = (SCREEN_H * 0.5) / self.focal[1].max(1e-3);
        (tan_x, tan_y, Vec2::new(SCREEN_W * 0.5 - self.centre[0], SCREEN_H * 0.5 - self.centre[1]))
    }
}

struct Slot {
    image: Handle<Image>,
    cam: Option<Entity>,
    node: Entity,
    layer: usize,
    view: Option<CanvasView>,
    /// [`Canvases::show_exact`]: the projection tangents as given (else derived from the view's focal lengths).
    tan: Option<(f32, f32)>,
    composite: Composite,
}

#[derive(Resource, Default)]
pub struct Canvases {
    slots: Vec<Slot>,
}

impl Canvases {
    /// A new canvas (hidden), composed under the HUD.
    pub fn create(&mut self, commands: &mut Commands, images: &mut Assets<Image>, name: &str) -> CanvasId { self.create_composited(commands, images, name, Composite::UnderHud) }

    /// A new canvas (hidden), composed over the HUD (a page menu's 3D widget).
    pub fn create_over_hud(&mut self, commands: &mut Commands, images: &mut Assets<Image>, name: &str) -> CanvasId { self.create_composited(commands, images, name, Composite::OverHud) }

    fn create_composited(&mut self, commands: &mut Commands, images: &mut Assets<Image>, name: &str, composite: Composite) -> CanvasId {
        let mut img = Image::new_target_texture(1024, 832, TextureFormat::Rgba8UnormSrgb, None);
        img.sampler = bevy::image::ImageSampler::nearest();
        let image = images.add(img);
        let mut node = commands.spawn((Node { position_type: PositionType::Absolute, ..default() }, ImageNode::new(image.clone()), Visibility::Hidden, Name::new(format!("canvas {name}"))));
        // Under the HUD: a root node below the HUD composite; over it: a child of the composite (parented by `apply`).
        if composite == Composite::UnderHud { node.insert(GlobalZIndex(i32::MAX - 2)); }
        let node = node.id();
        let layer = FIRST_LAYER + self.slots.len();
        self.slots.push(Slot { image, cam: None, node, layer, view: None, tan: None, composite });
        CanvasId(self.slots.len() - 1)
    }

    pub fn layer(&self, id: CanvasId) -> RenderLayers { RenderLayers::layer(self.slots[id.0].layer) }

    /// The offscreen image (window-sized; the view shows its letterboxed rectangle).
    #[allow(dead_code)]
    pub fn image(&self, id: CanvasId) -> Handle<Image> { self.slots[id.0].image.clone() }

    /// This frame's view (None: hidden).
    pub fn show(&mut self, id: CanvasId, view: Option<CanvasView>) { (self.slots[id.0].view, self.slots[id.0].tan) = (view, None); }

    /// [`Canvases::show`] with the projection tangents given exactly (they replace the ones the view's focal lengths give):
    /// a caller drawing with the game camera's own projection passes its tangents, so the canvas projection is
    /// bit-identical to it (`half / (half / tan)` does not always give `tan` back in `f32`).
    pub fn show_exact(&mut self, id: CanvasId, view: Option<CanvasView>, tan: (f32, f32)) { (self.slots[id.0].view, self.slots[id.0].tan) = (view, Some(tan)); }
}

pub struct CanvasPlugin;

impl Plugin for CanvasPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Canvases>().add_systems(crate::level_switch::LevelUnload, crate::level_switch::reset::<Canvases>).add_systems(PostUpdate, apply.before(TransformSystems::Propagate).in_set(crate::display::ResizeTargets));
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply(
    mut commands: Commands,
    mut canvases: ResMut<Canvases>,
    frame: Res<crate::display::GameFrame>,
    main: Query<(Entity, &Transform), With<crate::fly_cam::FlyCam>>,
    mut transforms: Query<&mut Transform, Without<crate::fly_cam::FlyCam>>,
    mut cams: Query<(&mut Camera, &mut Projection)>,
    mut nodes: Query<(&mut Node, &mut ImageNode, &mut Visibility)>,
    hud: Query<Entity, With<crate::hud_render::HudBoxNode>>,
    parents: Query<(), With<ChildOf>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(main_t) = main.iter().next().map(|(_, t)| *t) else { return };
    // The game frame's 4:3 box (crate::display): the canvas image covers it, the 512×416 screen stretched onto it.
    let (_, bs) = crate::display::ui_box(frame.size);
    let size = bs.as_uvec2().max(UVec2::ONE);
    let scale = Vec2::new(size.x as f32 / SCREEN_W, size.y as f32 / SCREEN_H);
    for s in &mut canvases.slots {
        match s.composite {
            // Both are children of the HUD composite's 4:3 box (crate::display); under the composite by their global z
            // index (MAX − 2 < the composite's MAX), over it as plain children.
            Composite::UnderHud | Composite::OverHud => {
                if !parents.contains(s.node) {
                    if let Some(h) = hud.iter().next() { commands.entity(h).add_child(s.node); }
                }
            }
        }
        let Some(v) = s.view else {
            if let Some(e) = s.cam.take() { commands.entity(e).despawn(); }
            if let Ok((_, _, mut vis)) = nodes.get_mut(s.node) { vis.set_if_neq(Visibility::Hidden); }
            continue;
        };
        let same = images.get(&s.image).is_some_and(|i| i.texture_descriptor.size.width == size.x && i.texture_descriptor.size.height == size.y);
        // A fresh image of the new size (crate::display::fresh_target): the node shows it, the camera is spawned again on it.
        if !same {
            if let Some(new) = crate::display::fresh_target(&mut images, &s.image, size) {
                if let Ok((_, mut img, _)) = nodes.get_mut(s.node) { img.image = new.clone(); }
                if let Some(e) = s.cam.take() { commands.entity(e).despawn(); }
                s.image = new;
            }
        }
        let (mut tan_x, mut tan_y, offset) = v.camera();
        if let Some(t) = s.tan { (tan_x, tan_y) = t; }
        let sub = SubCameraView { full_size: UVec2::new(SCREEN_W as u32, SCREEN_H as u32), offset, size: UVec2::new(SCREEN_W as u32, SCREEN_H as u32) };
        let proj = GameProjection { tan_x, tan_y, ..GameProjection::default() };
        match s.cam {
            None => {
                s.cam = Some(
                    commands
                        .spawn((
                            Camera3d::default(),
                            Camera { order: -4, clear_color: ClearColorConfig::Custom(v.clear), sub_camera_view: Some(sub), ..default() },
                            RenderTarget::Image(s.image.clone().into()),
                            Projection::custom(proj),
                            Msaa::Off,
                            Tonemapping::None,
                            DebandDither::Disabled,
                            RenderLayers::layer(s.layer),
                            main_t,
                            Name::new(format!("canvas camera (layer {})", s.layer)),
                        ))
                        .id(),
                );
            }
            Some(e) => {
                if let Ok(mut t) = transforms.get_mut(e) {
                    if *t != main_t { *t = main_t; }
                }
                if let Ok((mut c, mut p)) = cams.get_mut(e) {
                    if c.sub_camera_view != Some(sub) { c.sub_camera_view = Some(sub); }
                    if !matches!(c.clear_color, ClearColorConfig::Custom(k) if k == v.clear) { c.clear_color = ClearColorConfig::Custom(v.clear); }
                    let cur = match &*p {
                        Projection::Custom(c) => c.get::<GameProjection>().map(|g| (g.tan_x, g.tan_y)),
                        _ => None,
                    };
                    if cur != Some((tan_x, tan_y)) { *p = Projection::custom(proj); }
                }
            }
        }
        if let Ok((mut n, mut img, mut vis)) = nodes.get_mut(s.node) {
            // Negative sizes (a rectangle projected from the far corner) flip to the positive one.
            let (x0, x1) = (v.rect[0].min(v.rect[0] + v.rect[2]), v.rect[0].max(v.rect[0] + v.rect[2]));
            let (y0, y1) = (v.rect[1].min(v.rect[1] + v.rect[3]), v.rect[1].max(v.rect[1] + v.rect[3]));
            let pct = |a: f32, full: f32| Val::Percent(a * 100.0 / full);
            let want = (pct(x0, SCREEN_W), pct(y0, SCREEN_H), pct(x1 - x0, SCREEN_W), pct(y1 - y0, SCREEN_H));
            if (n.left, n.top, n.width, n.height) != want { (n.left, n.top, n.width, n.height) = want; }
            // The camera renders the 512×416 screen onto the whole image.
            let r = Rect::new(x0 * scale.x, y0 * scale.y, x1 * scale.x, y1 * scale.y);
            if img.rect != Some(r) { img.rect = Some(r); }
            vis.set_if_neq(if x1 - x0 >= 0.5 && y1 - y0 >= 0.5 { Visibility::Inherited } else { Visibility::Hidden });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_view_maps_texels_to_the_rectangle() {
        // A 512×128 target at zoom 1.0 (tan 1.0, 0.775) whose 200×100 texels go to a 200×100 rectangle at (40, 60).
        let v = CanvasView::from_target((512.0, 128.0), (1.0, 0.775), (200.0, 100.0), [40.0, 60.0, 200.0, 100.0], Color::BLACK);
        assert_eq!(v.centre, [40.0 + 256.0, 60.0 + 64.0]);
        assert_eq!(v.focal[0], 256.0);
        assert!((v.focal[1] - 64.0 / 0.775).abs() < 1e-3);
        let (tan_x, tan_y, off) = v.camera();
        assert!((tan_x - 1.0).abs() < 1e-6 && (tan_y - 208.0 * 0.775 / 64.0).abs() < 1e-4);
        assert_eq!(off, Vec2::new(256.0 - 296.0, 208.0 - 124.0));
        // Half size: the same texels squeezed.
        let h = CanvasView::from_target((512.0, 128.0), (1.0, 0.775), (200.0, 100.0), [90.0, 85.0, 100.0, 50.0], Color::BLACK);
        assert_eq!(h.focal[0], 128.0);
    }

}
