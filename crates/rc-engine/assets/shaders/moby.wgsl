// Moby base pass: skinning with the animated joint palette and the VU0 104691 vertex lighting (single-
// matrix path, emulated in f32), then GS TFX = MODULATE and GS fog (PRIM FGE = 1). Rust side:
// moby_render.rs (layout), moby_anim.rs (palette), moby_light.rs (light block, f32 replica of this code).
// Spec: docs/plan/moby_skinning_lighting.md §4-5, docs/plan/moby_animation.md.
//
// Skinning (VU0 104691 entry 0): M = F[j0] (1 joint) or w0·F[j0] + w1·F[j1] (+ w2·F[j2]) with w/256;
// pos = trunc(M0·x + M1·y + M2·z + M3) (ftoi0) wrapped to s16 (ppach keeps the low halfword); then the
// per-moby VU1 matrix, here the instance record's model matrix (packed units → Bevy world) and the view.
// Lighting: n = (cos a·cos e, sin a·cos e, sin e) from the boot-ELF table, n' = M0·n.x + M1·n.y + M2·n.z
// (not normalised), d_k = L_k·n', f_k = max(d_k, −|K_k|·d_k), c = A + ⌊128·(Σ C_k f_k)·rsqrt(|n'|²)⌋
// (the 65536 + c/128 float's 1/128 grid truncation), then the EE pack: s16(c)·m saturated to s16, >> 7,
// low byte. Colours travel as bytes / 128 like the tfrag pass.
//
// MODULATE: Cv = (Ct * Cf) >> 7, clamped. Gouraud RGBA and F are interpolated linearly in screen space
// (@interpolate(linear)), ST perspective-correctly. Alpha: ALPHA_1 = 0x8000000044, TEST_1 = 0x5360b per
// moby (MobyProc GS state at 0x1dedc0), as for tfrags; the fragment returns As / 128 and the GS_ATEST_*
// defs select the half of the alpha-test split this draw is (gs_state.rs). Fog: the moby VU1 program (13859) clamps the F lane between the same
// view-context fog terms as the tfrag program after adding the fog offset, so the tfrag fog uniform is reused.

#import bevy_pbr::{
    mesh_functions,
    view_transformations::{position_world_to_clip, position_world_to_view},
}
#ifdef DISPLAY_BLEND_MIX
#import ratchet_clank::display_blend::gs_mix
#endif
#ifdef DISPLAY_BLEND_ADD
#import ratchet_clank::display_blend::gs_add
#endif

struct MobyFog {
    // rgb = FOGCOL (display-encoded 0..1), w = 1 when fog is enabled.
    color: vec4<f32>,
    // (slope per integer unit of depth, offset qw661.w, lower clamp qw656.y, upper clamp qw656.z).
    params: vec4<f32>,
}

struct MobyInst {
    // Packed game model units -> Bevy world: game_to_bevy · [s/1024 · R | p].
    model: mat4x4<f32>,
    // Row j = (L_0[j], L_1[j], L_2[j], 0): model-space "to light" vectors of the 3 lights.
    light_rows: array<vec4<f32>, 3>,
    // C_0, C_1, C_2 (1.0 adds 128 to a colour byte), w = 0.
    light_colors: array<vec4<f32>, 3>,
    // (−|K_0|, −|K_1|, −|K_2|, 1).
    neg_k: vec4<f32>,
    // Ambient r, g, b and alpha as colour counts (0..255).
    ambient: vec4<f32>,
    // x = first palette slot, y = colour mode (0 GPU light, 1 CPU table, 2 unlit), z = CPU colour base.
    misc: vec4<u32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var tex: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var tex_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<storage, read> fog: MobyFog;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<storage, read> insts: array<MobyInst>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var<storage, read> palette: array<mat4x4<f32>>;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var<storage, read> normal_table: array<vec2<f32>>;
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var<storage, read> cpu_colors: array<u32>;

// Per instance, rewritten per frame by MobyProc's replay (crate::moby_lod): x = vertex alpha (the ambient α
// lane: distance fade × moby+0x23 >> 7), y = flags (1 = tint red: low LOD with RC_MOBY_LOD_TINT=1),
// z = shine alpha, w = glow word (bit 24: on the glow list, mode 0x10; bits 0..23: moby+0x90 RGB); e = the metal
// pass's sphere-map basis (read by moby_metal.wgsl).
struct MobyLod {
    misc: vec4<u32>,
    e: array<vec4<f32>, 3>,
}
@group(#{MATERIAL_BIND_GROUP}) @binding(7) var<storage, read> lods: array<MobyLod>;

struct MobyVertex {
    @builtin(instance_index) instance_index: u32,
    // Packed model position (s16 as f32), game axes.
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    // x = azimuth | elevation << 8 | joint count << 16 | glow packet << 24; y = joints; z = weights (10 bits each);
    // w = multiplier RGBA.
    @location(2) skin: vec4<u32>,
    // Index of the vertex in the class's high-LOD list (CPU colour table).
    @location(3) vid: u32,
}

struct MobyVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    // Gouraud RGBA / 128, screen-space linear like the GS.
    @location(1) @interpolate(linear) color: vec4<f32>,
    // GS F / 255, screen-space linear like the GS.
    @location(2) @interpolate(linear) fog: f32,
}

fn s16(x: i32) -> i32 { return (x << 16u) >> 16u; }

@vertex
fn vertex(v: MobyVertex) -> MobyVertexOutput {
    var out: MobyVertexOutput;
    let tag = mesh_functions::get_tag(v.instance_index);
    let inst = &insts[tag];
    let lod = &lods[tag];
    let base = (*inst).misc.x;
    let alpha = f32((*lod).misc.x);

    // Blended joint matrix (weights are /256; every blend on the disc sums to 256). Count 0: a low-LOD list
    // drawn with job joint count 0, for which MobyAnimEval only writes the identity (moby_render build_parts).
    let count = (v.skin.x >> 16u) & 0xffu;
    let j = vec3<u32>(v.skin.y & 0xffu, (v.skin.y >> 8u) & 0xffu, (v.skin.y >> 16u) & 0xffu);
    var m = palette[base + j.x];
    if (count == 0u) {
        m = mat4x4<f32>(vec4<f32>(1.0, 0.0, 0.0, 0.0), vec4<f32>(0.0, 1.0, 0.0, 0.0), vec4<f32>(0.0, 0.0, 1.0, 0.0), vec4<f32>(0.0, 0.0, 0.0, 1.0));
    }
    if (count >= 2u) {
        let w = vec3<f32>(f32(v.skin.z & 0x3ffu), f32((v.skin.z >> 10u) & 0x3ffu), f32((v.skin.z >> 20u) & 0x3ffu)) / 256.0;
        m = palette[base + j.x] * w.x + palette[base + j.y] * w.y;
        if (count >= 3u) {
            m = m + palette[base + j.z] * w.z;
        }
    }

    // ftoi0 (truncate; out-of-range saturates) then the s16 wrap of ppach.
    let sp = clamp(trunc((m * vec4<f32>(v.position, 1.0)).xyz), vec3<f32>(-2147483520.0), vec3<f32>(2147483520.0));
    let pi = vec3<i32>(s16(i32(sp.x)), s16(i32(sp.y)), s16(i32(sp.z)));
    let world = ((*inst).model * vec4<f32>(vec3<f32>(pi), 1.0)).xyz;
    out.position = position_world_to_clip(world);
    // A class texture scroll (misc.w: s | t << 16, s16 in 1/4096: `FUN_00263d90`'s offset on the class's STs).
    let scroll = (*inst).misc.w;
    out.uv = v.uv + vec2<f32>(f32(s16(i32(scroll & 0xffffu))), f32(s16(i32(scroll >> 16u)))) / 4096.0;

    let mode = (*inst).misc.y;
    if (mode == 1u) {
        out.color = unpack4x8unorm(cpu_colors[(*inst).misc.z + v.vid]) * (255.0 / 128.0);
        out.color.a = alpha / 128.0;
    } else if (mode == 2u) {
        out.color = vec4<f32>(1.0, 1.0, 1.0, alpha / 128.0);
    } else {
        let ta = normal_table[v.skin.x & 0xffu];
        let te = normal_table[(v.skin.x >> 8u) & 0xffu];
        let n = vec3<f32>(ta.x * te.x, ta.y * te.x, te.y);
        let np = (m[0] * n.x + m[1] * n.y + m[2] * n.z).xyz;
        let d = (*inst).light_rows[0] * np.x + (*inst).light_rows[1] * np.y + (*inst).light_rows[2] * np.z;
        let f = max(d, d * (*inst).neg_k);
        let s = (*inst).light_colors[0] * f.x + (*inst).light_colors[1] * f.y + (*inst).light_colors[2] * f.z;
        let q = inverseSqrt(np.x * np.x + np.y * np.y + np.z * np.z);
        let c = vec4<f32>((*inst).ambient.xyz, alpha) + floor(s * q * 128.0);
        let mult = vec4<i32>(unpack4x8unorm(v.skin.w) * 255.0 + 0.5);
        let h = vec4<i32>(s16(i32(c.x)), s16(i32(c.y)), s16(i32(c.z)), s16(i32(c.w)));
        let p = clamp(h * mult, vec4<i32>(-32768), vec4<i32>(32767)) >> vec4<u32>(7u);
        out.color = vec4<f32>(p & vec4<i32>(0xff)) / 128.0;
    }
    // The glow list (MobyProc level01 0x26b890, fun_002116b8 0x26a650; crate::moby_lod): after the skin/light pass
    // every vertex colour of a glow packet is replaced by the moby's glow word, its byte 3 by the vertex alpha.
    let glow = (*lod).misc.w;
    if ((v.skin.x & 0x1000000u) != 0u && (glow & 0x1000000u) != 0u) {
        out.color = vec4<f32>(f32(glow & 0xffu), f32((glow >> 8u) & 0xffu), f32((glow >> 16u) & 0xffu), alpha) / 128.0;
    }
    if (((*lod).misc.y & 1u) != 0u) {
        out.color = vec4<f32>(out.color.r * 0.5 + 1.0, out.color.gb * 0.3, out.color.a);
    }

    // Camera-space depth in the game's integer units (Bevy view space looks down -z, game units).
    let depth = -position_world_to_view(world).z * 1024.0;
    // VU: w = depth * slope (the w row of the matrix), += qw661.w, min qw656.z, max qw656.y, ftoi4.
    let fg = max(min(depth * fog.params.x + fog.params.y, fog.params.w), fog.params.z);
    out.fog = trunc(fg) / 255.0;
    return out;
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

@fragment
fn fragment(in: MobyVertexOutput) -> @location(0) vec4<f32> {
    let t = textureSample(tex, tex_sampler, in.uv);
    var rgb = min(t.rgb * in.color.rgb, vec3<f32>(1.0));
    if (fog.color.w > 0.5) {
        rgb = mix(fog.color.rgb, rgb, in.fog);
    }
    // As = (At * Af) >> 7 in GS units (At raw 0..0x80, Af 0x80 = 1.0), clamped to 0xff; FGE leaves it alone.
    let a_s = min(floor(round(t.a * 255.0) * round(in.color.a * 128.0) / 128.0), 255.0);
    // GS TEST_1 alpha test (ATST GEQUAL AREF, AFAIL RGB_ONLY): which half of the split this draw is (gs_state.rs).
#ifdef GS_ATEST_PASS
    if (a_s < f32(#{GS_AREF})) { discard; }
#endif
#ifdef GS_ATEST_FAIL
    if (a_s >= f32(#{GS_AREF})) { discard; }
#endif
    // Effect mobys (gs_state `EffectMix` / `AdditiveNoZ`): the GS blend on the frame's display bytes
    // (crate::display_blend).
#ifdef DISPLAY_BLEND_MIX
    return gs_mix(round(rgb * 255.0), a_s);
#else ifdef DISPLAY_BLEND_ADD
    return gs_add(round(rgb * 255.0), a_s);
#else
    return vec4<f32>(srgb_to_linear(rgb), a_s / 128.0);
#endif
}
