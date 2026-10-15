// Underwater full-screen tint (docs/plan/world_animation.md §6; Rust side: fog_state.rs).
//
// `DrawDebugProfiler` (level01 0x21a1b8, `0x15f3f4 & 0x40` pass) sets ALPHA_1 = 0x8000000044 and draws
// untextured full-screen sprites (DIRECT 0x13cc90) with RGBAQ = 0x161200..03 while the underwater flag is
// set: A = Cs, B = Cd, C = As, D = Cd, i.e. Cv = ((Cs − Cd)·As >> 7) + Cd on the frame buffer's 8-bit,
// display-encoded values (COLCLAMP clamps to 0..255). The frame buffer here is sRGB-encoded storage, so
// the pass reads the pixel, re-encodes it to the byte the GS would hold, blends in integers like the GS
// and writes the result back (the sRGB target stores that byte exactly). Destination alpha is kept.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;

struct UnderwaterTint {
    // GS RGBA bytes (A: 0x80 = 1.0).
    rgba: vec4<u32>,
}
@group(0) @binding(2) var<uniform> tint: UnderwaterTint;

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
    let cs = vec3<i32>(tint.rgba.rgb);
    let a = i32(tint.rgba.a);
    let cv = clamp((((cs - cd) * a) >> vec3<u32>(7u)) + cd, vec3(0), vec3(255));
    return vec4(to_linear(vec3<f32>(cv) / 255.0), d.a);
}
