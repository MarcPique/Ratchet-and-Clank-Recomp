// Moby shadow resolve (docs/plan/shadows.md §3.4, §6.2; Rust side: shadow_render.rs).
//
// Where the count target is positive, the pixel is darkened by a fixed multiply blend (`dst · K`, blend state
// Zero / Src): the game's `0.75·Cd` on the frame buffer's display bytes becomes one linear factor on the sRGB
// target, within 3/255 of the GS byte (hardware_fidelity_layers.md "Result-level reproductions"). Hard edge,
// binary: overlapping shadows do not darken twice. With MSAA the count target is multisampled and the resolve
// runs per sample.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

#ifdef MULTISAMPLED
@group(0) @binding(0) var count: texture_multisampled_2d<f32>;
#else
@group(0) @binding(0) var count: texture_2d<f32>;
#endif

// The display byte ×0.75 as one linear-light factor (the Rust constant SHADOW_FACTOR; a test keeps them equal).
const K: f32 = 0.53;

fn inside(pos: vec4<f32>, sample: u32) -> f32 {
#ifdef MULTISAMPLED
    return textureLoad(count, vec2<i32>(floor(pos.xy)), i32(sample)).r;
#else
    return textureLoad(count, vec2<i32>(floor(pos.xy)), 0).r;
#endif
}

#ifdef MULTISAMPLED
@fragment
fn darken(in: FullscreenVertexOutput, @builtin(sample_index) s: u32) -> @location(0) vec4<f32> {
    if inside(in.position, s) < 0.5 { discard; }
    return vec4<f32>(K, K, K, 1.0);
}

@fragment
fn debug(in: FullscreenVertexOutput, @builtin(sample_index) s: u32) -> @location(0) vec4<f32> {
    let c = inside(in.position, s);
    if abs(c) < 0.5 { discard; }
    return select(vec4<f32>(0.0, 0.0, 1.0, 1.0), vec4<f32>(1.0, 0.0, 0.0, 1.0), c > 0.0);
}
#else
@fragment
fn darken(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    if inside(in.position, 0u) < 0.5 { discard; }
    return vec4<f32>(K, K, K, 1.0);
}

// RC_SHADOW_DEBUG=1: the count target itself (red: inside a volume, blue: a negative count).
@fragment
fn debug(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let c = inside(in.position, 0u);
    if abs(c) < 0.5 { discard; }
    return select(vec4<f32>(0.0, 0.0, 1.0, 1.0), vec4<f32>(1.0, 0.0, 0.0, 1.0), c > 0.0);
}
#endif
