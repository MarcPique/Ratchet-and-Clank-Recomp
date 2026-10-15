// GS primitives the game draws straight from a draw callback (crate::fx_draw): `FastDrawQuadReal` quads and
// `DrawEnvOverlayMesh` strips, built on the CPU every tick (the fire / smoke fields 760, the nanotech glow 806, …).
// Per vertex: position (Bevy space), ST, GS vertex RGBA bytes / 128. GS: PRIM 0x7c (Gouraud, TME, FGE, ABE), TEX0
// MODULATE with TCC (C = Ct·Cv >> 7, A = At·Av >> 7), then fog, then ALPHA 0x48 (Cs·As + Cd, params.misc.x = 1) or
// 0x44 ((Cs − Cd)·As + Cd), on the frame's display bytes like the GS (crate::display_blend `gs_add` / `gs_mix`;
// blend One, OneMinusSrcAlpha). The texture holds the raw GS bytes (texel alpha 0..0x80). FX_OPAQUE (FxPrimParams::opaque):
// an opaque world surface in the main pass, no blend, Z written.

#import bevy_pbr::view_transformations::{position_world_to_clip, position_world_to_view}
#import ratchet_clank::display_blend::{gs_add, gs_mix}

struct FxFog {
    color: vec4<f32>,
    params: vec4<f32>,
}

struct FxParams {
    misc: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var tex: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var tex_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> fog: FxFog;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<uniform> params: FxParams;

struct FxVertex {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct FxVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    // RGBA bytes / 128 and GS F / 255, screen-space linear like the GS.
    @location(1) @interpolate(linear) color: vec4<f32>,
    @location(2) @interpolate(linear) fog: f32,
}

@vertex
fn vertex(v: FxVertex) -> FxVertexOutput {
    var out: FxVertexOutput;
    out.position = position_world_to_clip(v.position);
    out.uv = v.uv;
    out.color = v.color;
    let depth = -position_world_to_view(v.position).z * 1024.0;
    let f = max(min(depth * fog.params.x + fog.params.y, fog.params.w), fog.params.z);
    out.fog = trunc(f) / 255.0;
    return out;
}

@fragment
fn fragment(in: FxVertexOutput) -> @location(0) vec4<f32> {
    let t = textureSample(tex, tex_sampler, in.uv);
    let ct = round(t * 255.0);
    // MODULATE on the bytes: floor(Ct·Cv / 128) (colour clamped to 255), A likewise.
    var rgb = min(floor(ct.rgb * in.color.rgb + vec3<f32>(1e-3)), vec3<f32>(255.0)) / 255.0;
    let a = min(floor(ct.a * in.color.a + 1e-3), 255.0);
    if (fog.color.w > 0.5) {
        rgb = mix(fog.color.rgb, rgb, in.fog);
    }
#ifdef FX_OPAQUE
    // An opaque world surface (FxPrimParams::opaque): the display bytes as the world shaders write them.
    return vec4<f32>(srgb_to_linear(rgb), 1.0);
#else
    let cs = round(rgb * 255.0);
    // 0x48 adds Cs·As; the subtractive pass (misc.y, a reverse-subtract blend) takes the same term off.
    if (params.misc.x > 0.5 || params.misc.y > 0.5) {
        return gs_add(cs, a);
    }
    return gs_mix(cs, a);
#endif
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}
