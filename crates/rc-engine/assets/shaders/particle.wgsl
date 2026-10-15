// Particle sprites (render kinds 0 and 1) as PartProc + VU1 program 221571 draw them. Rust side, the cull/sort rules
// and every constant: particle_render.rs (docs/plan/particles.md §7).
//
// Per vertex: sprite index << 2 | corner, from a persistent quad mesh. Per sprite (storage buffer, rewritten
// every frame in draw order; quads past `count` are dropped): the particle centre (Bevy world), the corner
// vectors a and b in 512x416 frame-buffer pixels (x right, y down; corners S+a, S+b, S-b, S-a with ST (0,0),
// (1,0), (0,1), (1,1)), the vertex RGBA (0x80 = 1.0, alpha already faded) and the texture layer | additive << 16 |
// flat << 17 (kind 1: a and b are world offsets in the game's XY plane, projected per corner).
// The corner keeps the centre's depth: the offset is added in clip space (x / 256, -y / 208 in NDC).
//
// Fragment: MODULATE (C = Ct·Cv >> 7 clamped to 255, so vertex colours above 0x80 brighten up to 2×; A = At·Av >> 7,
// At in GS units: the texture stores 0x80 = 1.0), TEST_1 0x5380b (draw 0 keeps A >= 0x80 and writes Z, draw 1 keeps
// the rest), then the record's ALPHA_1 on the frame's display bytes like the GS: 0x44 `gs_mix`, 0x48 `gs_add`
// (crate::display_blend, `display_blend.wgsl`; blend One, OneMinusSrcAlpha).

#import bevy_pbr::view_transformations::position_world_to_clip
#import bevy_pbr::mesh_view_bindings::view
#import ratchet_clank::display_blend::{gs_add, gs_mix}

// Game pixels per NDC unit across: 256 in the game's 4:3 view; × the frame's widening in 16:9 (crate::display: the
// projection's x tangent is the game's × s, its y tangent the game's × 0.775, so s = 0.775·(1/tan_y)/(1/tan_x)).
fn half_w() -> f32 { return 256.0 * 0.775 * view.clip_from_view[1][1] / view.clip_from_view[0][0]; }

struct ParticleParams {
    // x = draw (0 = A >= AREF with Z, 1 = the rest), y = AREF.
    misc: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var tex: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var tex_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> params: ParticleParams;

struct Sprite {
    // centre xyz, a.x
    p: vec4<f32>,
    // a.y, b.x, b.y, 0
    q: vec4<f32>,
    // RGBA, tag
    c: vec4<u32>,
}

struct Sprites {
    count: vec4<u32>,
    items: array<Sprite>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(3) var<storage, read> sprites: Sprites;

struct ParticleVertex {
    @location(0) sprite_corner: u32,
}

struct ParticleVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    // Vertex RGBA bytes (0..255, 0x80 = 1.0): one value on a sprite (PRIM 0x54, flat), Gouraud on the lines and
    // ribbons (screen-linear like the GS).
    @location(1) @interpolate(linear) color: vec4<f32>,
    @location(2) @interpolate(flat) tag: u32,
}

@vertex
fn vertex(v: ParticleVertex) -> ParticleVertexOutput {
    var out: ParticleVertexOutput;
    let i = v.sprite_corner >> 2u;
    let k = v.sprite_corner & 3u;
    if (i >= sprites.count.x) {
        // Unused quad: all four corners on one point outside the clip volume (no fragments).
        out.position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        return out;
    }
    let s = sprites.items[i];
    let kind = (s.c.y >> 18u) & 3u;
    var st = vec2<f32>(f32(k & 1u), f32(k >> 1u));
    var c = s.c.x;
    if (kind >= 2u) {
        // Lines (2) and ribbons (3): p = (end 1, w1), q = (end 2, k), c = (RGBA 1, tag, RGBA 2); corners 0 / 1 at end
        // 1, 2 / 3 at end 2, on the + / − side.
        let p1 = s.p.xyz;
        let p2 = s.q.xyz;
        let e = select(p1, p2, k >= 2u);
        let side = select(1.0, -1.0, (k & 1u) == 1u);
        if (k >= 2u) { c = s.c.z; }
        if (kind == 3u) {
            let n = normalize(cross(p2 - p1, (p1 + p2) * 0.5 - view.world_position));
            let w = s.p.w * select(1.0, s.q.w, k >= 2u);
            out.position = position_world_to_clip(e + n * (w * side));
        } else {
            // One frame-buffer pixel wide: half a pixel either side, perpendicular to the line on the screen.
            let a = position_world_to_clip(p1);
            let b = position_world_to_clip(p2);
            let d = vec2<f32>((b.x / b.w - a.x / a.w) * half_w(), (a.y / a.w - b.y / b.w) * 208.0);
            let len = max(length(d), 1e-6);
            let nrm = vec2<f32>(-d.y, d.x) / len * (0.5 * side);
            var clip = select(a, b, k >= 2u);
            clip.x += nrm.x * (1.0 / half_w()) * clip.w;
            clip.y -= nrm.y * (1.0 / 208.0) * clip.w;
            out.position = clip;
        }
    } else {
        let a = vec2<f32>(s.p.w, s.q.x);
        let b = s.q.yz;
        var corner = a;
        if (k == 1u) { corner = b; }
        if (k == 2u) { corner = -b; }
        if (k == 3u) { corner = -a; }
        if (((s.c.y >> 17u) & 1u) != 0u) {
            // Kind 1, flat quad: the corner is a world offset in the game's XY plane (game (x, y) = Bevy (x, -z)).
            out.position = position_world_to_clip(s.p.xyz + vec3<f32>(corner.x, 0.0, -corner.y));
        } else {
            var clip = position_world_to_clip(s.p.xyz);
            clip.x += corner.x * (1.0 / half_w()) * clip.w;
            clip.y -= corner.y * (1.0 / 208.0) * clip.w;
            out.position = clip;
        }
    }
    out.uv = st;
    out.color = vec4<f32>(f32(c & 0xffu), f32((c >> 8u) & 0xffu), f32((c >> 16u) & 0xffu), f32(c >> 24u));
    out.tag = s.c.y;
    return out;
}

@fragment
fn fragment(in: ParticleVertexOutput) -> @location(0) vec4<f32> {
    let layer = i32(in.tag & 0xffffu);
    let additive = ((in.tag >> 16u) & 1u) != 0u;
    var t = textureSample(tex, tex_sampler, in.uv, layer);
    // A line is untextured (the GS then takes the vertex colour as is: MODULATE by a white 0x80 texel).
    if (((in.tag >> 18u) & 3u) == 2u) { t = vec4<f32>(128.0 / 255.0); }
    // Texel in GS units: RGB 0..255, A 0..0x80.
    let ct = round(t.rgb * 255.0);
    let at = round(t.a * 255.0);
    let cv = round(in.color.rgb);
    let av = round(in.color.a);
    let a = min(floor(at * av / 128.0), 255.0);
    let draw = params.misc.x;
    let aref = params.misc.y;
    if ((draw < 0.5 && a < aref) || (draw >= 0.5 && a >= aref)) { discard; }
    let cs = min(floor(ct * cv / 128.0), vec3<f32>(255.0));
    if (additive) {
        return gs_add(cs, a);
    }
    return gs_mix(cs, a);
}
