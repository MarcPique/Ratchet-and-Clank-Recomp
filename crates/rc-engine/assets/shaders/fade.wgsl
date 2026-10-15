// The mode-2 black fade quad (docs/plan/cutscenes_transitions.md §3; Rust side: scene_render.rs).
//
// `DrawWorld` ends with a full-screen black sprite of alpha 0x15f3fc·128 (and `FadeToBlack` draws the same
// quads over a held image), blended with ALPHA_1 (Cs − Cd)·As >> 7 + Cd on the frame buffer's 8-bit,
// display-encoded values. Like the underwater tint it runs after the UI pass (HUD, subtitles) and the tint:
// read the pixel, re-encode it to the byte the GS holds, blend in integers, write it back.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;

struct SceneFade {
    // GS RGBA bytes of the quad (A: 0x80 = 1.0).
    rgba: vec4<u32>,
}
@group(0) @binding(2) var<uniform> fade: SceneFade;

fn to_display(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(max(c, vec3(0.0)), vec3(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3(0.0031308));
}

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3(2.4));
    return select(hi, lo, c <= vec3(0.04045));
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let d = textureLoad(screen_texture, vec2<i32>(floor(in.position.xy)), 0);
    let cd = vec3<i32>(round(clamp(to_display(d.rgb), vec3(0.0), vec3(1.0)) * 255.0));
    let cs = vec3<i32>(fade.rgba.rgb);
    let a = i32(fade.rgba.a);
    let cv = clamp((((cs - cd) * a) >> vec3<u32>(7u)) + cd, vec3(0), vec3(255));
    return vec4(to_linear(vec3<f32>(cv) / 255.0), d.a);
}
