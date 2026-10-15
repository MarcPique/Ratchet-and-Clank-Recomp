// Level-code water (docs/plan/world_animation.md §1, §2; Rust side and every constant: water_render.rs).
//
// One pipeline for the four draw kinds (params.misc.y):
//   0 / 1  strip layer 1 / 2 (FUN_002b96e0 + FUN_00262618): uv = base + scroll ± wobble[hash & 7], the wobble
//          table computed per frame on the CPU with the VU0 sine (rc_game::water::wobble_table);
//          z = z0·w + z1·(1 − w) with w = the class's z blend (761 / 1225 bob, else the stored 0).
//   2 / 3  ripple patch water / env pass (FUN_002b91c8): the CPU built positions, grey and both UV sets.
// Strip vertex: position = (game x, game y, z0), z1, uv = (u1, v1, u2, v2), tag = (RGBA, hash group); the
// per-frame w, scroll and wobble come from this draw's `frame` record (storage, index = params.ids.x).
// Ripple vertex: tag = (sub-block | strip index << 8, grid vertex); position, grey and UVs come from the
// patch's `ripple` record (first word params.ids.y: mask, 17x17 x (x, y, z, env u, env v, RGBA), then the 46
// water UVs at params.ids.z); a sub-block outside the mask is dropped.
//
// Fragment: GS MODULATE (C = Ct·Cv >> 7), then fog (PRIM FGE, VU1 program 57843's F like the tfrag program),
// then ALPHA = FIX<<32 | 0x64: C = Cd + (Cs − Cd)·FIX/128. FIX is baked into the output alpha and blended
// with SrcAlpha / OneMinusSrcAlpha (gs_state::GS_BLEND). The vertex alpha is 0, so As = 0 always fails the
// TEST (AREF 0x60, AFAIL RGB_ONLY): colour only, never Z (depth write off, GEQUAL test).

#import bevy_pbr::view_transformations::{position_world_to_clip, position_world_to_view}

struct WaterFog {
    color: vec4<f32>,
    params: vec4<f32>,
}

struct WaterParams {
    // x = FIX / 128, y = kind.
    misc: vec4<f32>,
    // x = frame slot, y = ripple record's first word, z = water UV offset in the record.
    ids: vec4<u32>,
}

struct FrameRecord {
    // x = z blend w, yz = this layer's scroll offset.
    misc: vec4<f32>,
    // The eight wobble offsets A·(sin θ, cos θ), two per vec4 (index = hash & 7).
    wobble: array<vec4<f32>, 4>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var tex: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var tex_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> fog: WaterFog;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<uniform> params: WaterParams;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var<storage, read> frame: array<FrameRecord>;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var<storage, read> ripple: array<u32>;

struct WaterVertex {
    @location(0) position: vec3<f32>,
    @location(1) z1: f32,
    @location(2) uv: vec4<f32>,
    @location(3) tag: vec2<u32>,
}

struct WaterVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    // Gouraud RGB bytes / 128 and GS F / 255, screen-space linear like the GS.
    @location(1) @interpolate(linear) color: vec3<f32>,
    @location(2) @interpolate(linear) fog: f32,
}

fn wobble(f: FrameRecord, g: u32) -> vec2<f32> {
    let q = f.wobble[g >> 1u];
    return select(q.xy, q.zw, (g & 1u) == 1u);
}

@vertex
fn vertex(v: WaterVertex) -> WaterVertexOutput {
    var out: WaterVertexOutput;
    let kind = u32(params.misc.y + 0.5);
    var pos = v.position;
    var z = v.position.z;
    var uv = v.uv.xy;
    var c = v.tag.x;
    var drop = false;
    if (kind < 2u) {
        let f = frame[params.ids.x];
        let w = f.misc.x;
        z = v.position.z * w + v.z1 * (1.0 - w);
        let wob = wobble(f, v.tag.y & 7u);
        uv = v.uv.xy + f.misc.yz + select(wob, -wob, kind == 1u);
    } else {
        let base = params.ids.y;
        // A sub-block outside the patch's mask is dropped (below).
        drop = ((ripple[base] >> (v.tag.x & 0xffu)) & 1u) == 0u;
        let g = base + 4u + v.tag.y * 6u;
        pos = vec3<f32>(bitcast<f32>(ripple[g]), bitcast<f32>(ripple[g + 1u]), bitcast<f32>(ripple[g + 2u]));
        z = pos.z;
        let k = base + params.ids.z + (v.tag.x >> 8u) * 2u;
        uv = vec2<f32>(bitcast<f32>(ripple[k]), bitcast<f32>(ripple[k + 1u]));
        if (kind == 3u) {
            uv = vec2<f32>(bitcast<f32>(ripple[g + 3u]), bitcast<f32>(ripple[g + 4u]));
        }
        c = ripple[g + 5u];
    }
    // Game (x, y, z) -> Bevy (x, z, -y).
    let p = vec3<f32>(pos.x, z, -pos.y);
    out.position = position_world_to_clip(p);
    out.uv = uv;
    out.color = vec3<f32>(f32(c & 0xffu), f32((c >> 8u) & 0xffu), f32((c >> 16u) & 0xffu)) / 128.0;
    let depth = -position_world_to_view(p).z * 1024.0;
    let f = max(min(depth * fog.params.x + fog.params.y, fog.params.w), fog.params.z);
    out.fog = trunc(f) / 255.0;
    if (drop) {
        // All vertices of the sub-block on one point outside the clip volume: no fragments.
        out.position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    }
    return out;
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

@fragment
fn fragment(in: WaterVertexOutput) -> @location(0) vec4<f32> {
    let t = textureSample(tex, tex_sampler, in.uv);
    // MODULATE on the display bytes: floor(Ct·Cv / 128), clamped to 255.
    let ct = round(t.rgb * 255.0);
    var rgb = min(floor(ct * in.color + vec3<f32>(1e-3)), vec3<f32>(255.0)) / 255.0;
    if (fog.color.w > 0.5) {
        rgb = mix(fog.color.rgb, rgb, in.fog);
    }
    return vec4<f32>(srgb_to_linear(rgb), params.misc.x);
}
