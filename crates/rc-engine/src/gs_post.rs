//! Full-screen GS-blend passes on the main camera (a read of the frame, integer blending on the display bytes, the
//! result written back): the port's own copy of Bevy's `FullscreenMaterialPlugin`, keyed by the pass type.
//!
//! **Why a copy.** Bevy 0.19's plugin keeps each view's pipeline in one component shared by every material type
//! (`FullscreenMaterialPipelineId`, not generic): the prepare system of a type the camera does not carry removes it,
//! so with two types registered only the one whose prepare runs last draws. The scene fade (`SceneFade`) stays on
//! Bevy's plugin (its only user); the passes here each keep their own pipeline ([`PassPipeline<T>`]). The pass types
//! still implement Bevy's [`FullscreenMaterial`] (the shader and the ordering).
//!
//! Users: the Visibomb's missile view overlay (crate::visibomb_view), the underwater tint (crate::fog_state).

use bevy::core_pipeline::fullscreen_material::FullscreenMaterial;
use bevy::core_pipeline::FullscreenShader;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::camera::ExtractedCamera;
use bevy::render::extract_component::{ComponentUniforms, DynamicUniformIndex, ExtractComponentPlugin, UniformComponentPlugin};
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer};
use bevy::render::render_resource::{
    BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, CachedRenderPipelineId, ColorTargetState,
    ColorWrites, FragmentState, Operations, PipelineCache, RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor,
    Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages, TextureFormat, TextureSampleType, TextureViewId,
};
use bevy::render::renderer::{RenderContext, RenderDevice, ViewQuery};
use bevy::render::view::{ExtractedView, ViewTarget};
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::ShaderRef;
use std::marker::PhantomData;

/// Registers pass `T` (Bevy's `FullscreenMaterialPlugin` with a per-type pipeline component).
pub struct GsPostPlugin<T>(PhantomData<T>);

impl<T> Default for GsPostPlugin<T> {
    fn default() -> Self { GsPostPlugin(PhantomData) }
}

impl<T: FullscreenMaterial> Plugin for GsPostPlugin<T> {
    fn build(&self, app: &mut App) {
        app.add_plugins((ExtractComponentPlugin::<T>::default(), UniformComponentPlugin::<T>::default()));
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app.add_systems(RenderStartup, init::<T>).add_systems(
            Render,
            (prepare_pipelines::<T>.in_set(RenderSystems::Prepare), prepare_bind_groups::<T>.in_set(RenderSystems::PrepareBindGroups)),
        );
        let system = T::schedule_configs(pass::<T>.into_configs());
        render_app.add_systems(T::schedule(), system);
    }
}

/// The pass's layout, sampler, base descriptor and its pipelines by target format.
#[derive(Resource)]
struct Pipelines<T> {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    desc: RenderPipelineDescriptor,
    by_format: HashMap<TextureFormat, CachedRenderPipelineId>,
    _marker: PhantomData<T>,
}

/// The view's pipeline for pass `T`.
#[derive(Component)]
pub struct PassPipeline<T>(CachedRenderPipelineId, PhantomData<T>);

/// Bind groups for both main textures (the source can be either).
#[derive(Component)]
pub struct PassBindGroups<T> {
    a: (TextureViewId, BindGroup),
    b: (TextureViewId, BindGroup),
    _marker: PhantomData<T>,
}

fn init<T: FullscreenMaterial>(mut commands: Commands, render_device: Res<RenderDevice>, asset_server: Res<AssetServer>, fullscreen: Res<FullscreenShader>) {
    let layout = BindGroupLayoutDescriptor::new(
        "gs_post_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (texture_2d(TextureSampleType::Float { filterable: true }), sampler(SamplerBindingType::Filtering), uniform_buffer::<T>(true)),
        ),
    );
    let sampler = render_device.create_sampler(&SamplerDescriptor::default());
    let shader = match T::fragment_shader() {
        ShaderRef::Handle(h) => h,
        ShaderRef::Path(p) => asset_server.load(p),
        ShaderRef::Default => panic!("GsPostPlugin: no fragment shader"),
    };
    let desc = RenderPipelineDescriptor {
        label: Some(format!("gs_post<{}>", std::any::type_name::<T>()).into()),
        layout: vec![layout.clone()],
        vertex: fullscreen.to_vertex_state(),
        fragment: Some(FragmentState { shader, targets: vec![Some(ColorTargetState { format: TextureFormat::Rgba8UnormSrgb, blend: None, write_mask: ColorWrites::ALL })], ..default() }),
        ..default()
    };
    commands.insert_resource(Pipelines::<T> { layout, sampler, desc, by_format: HashMap::default(), _marker: PhantomData });
}

fn prepare_pipelines<T: FullscreenMaterial>(
    mut commands: Commands,
    cache: Res<PipelineCache>,
    mut pipelines: ResMut<Pipelines<T>>,
    views: Query<(Entity, &ExtractedView, Option<&T>), With<ExtractedCamera>>,
) {
    for (e, view, material) in &views {
        if material.is_none() {
            commands.entity(e).remove::<PassPipeline<T>>();
            continue;
        }
        let format = view.target_format;
        let p = &mut *pipelines;
        let id = *p.by_format.entry(format).or_insert_with(|| {
            let mut d = p.desc.clone();
            if let Some(Some(t)) = d.fragment.as_mut().and_then(|f| f.targets.get_mut(0)) { t.format = format; }
            cache.queue_render_pipeline(d)
        });
        commands.entity(e).insert(PassPipeline::<T>(id, PhantomData));
    }
}

#[allow(clippy::type_complexity)]
fn prepare_bind_groups<T: FullscreenMaterial>(
    mut commands: Commands,
    mut views: Query<(Entity, &ViewTarget, Option<&mut PassBindGroups<T>>, Option<&T>)>,
    pipelines: Option<Res<Pipelines<T>>>,
    cache: Res<PipelineCache>,
    uniforms: Res<ComponentUniforms<T>>,
    render_device: Res<RenderDevice>,
) {
    let Some(pipelines) = pipelines else { return };
    let Some(binding) = uniforms.uniforms().binding() else { return };
    for (e, target, groups, material) in &mut views {
        if material.is_none() {
            commands.entity(e).remove::<PassBindGroups<T>>();
            continue;
        }
        let make = |tex: &bevy::render::render_resource::TextureView| {
            (tex.id(), render_device.create_bind_group("gs_post_bind_group", &cache.get_bind_group_layout(&pipelines.layout), &BindGroupEntries::sequential((tex, &pipelines.sampler, binding.clone()))))
        };
        let (main, other) = (target.main_texture_view(), target.main_texture_other_view());
        match groups {
            Some(mut g) => {
                if g.a.0 != main.id() { g.a = make(main); }
                if g.b.0 != other.id() { g.b = make(other); }
            }
            None => { commands.entity(e).insert(PassBindGroups::<T> { a: make(main), b: make(other), _marker: PhantomData }); }
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn pass<T: FullscreenMaterial>(
    view: ViewQuery<(&ViewTarget, &DynamicUniformIndex<T>, &PassBindGroups<T>, &PassPipeline<T>)>,
    cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let (target, index, groups, pipeline) = view.into_inner();
    let Some(pipeline) = cache.get_render_pipeline(pipeline.0) else { return };
    let post = target.post_process_write();
    let (source, destination) = (post.source, post.destination);
    let (_, group) = if groups.a.0 == source.id() { &groups.a } else { &groups.b };
    let desc = RenderPassDescriptor {
        label: Some("gs_post_pass"),
        color_attachments: &[Some(RenderPassColorAttachment { view: destination, depth_slice: None, resolve_target: None, ops: Operations::default() })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    };
    let mut rp = ctx.command_encoder().begin_render_pass(&desc);
    rp.set_pipeline(pipeline);
    rp.set_bind_group(0, group, &[index.index()]);
    rp.draw(0..3, 0..1);
}
