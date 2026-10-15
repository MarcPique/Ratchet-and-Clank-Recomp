// Sky star sprites as SkySpriteProc + the particle VU1 program draw them (sky_stars.rs,
// docs/plan/sky_render_notes.md "Stars in the port").
//
// Per vertex: sprite index << 2 | corner, from a persistent quad mesh. Per sprite (storage buffer, the layout
// of particle.wgsl; quads past `count` are dropped): the star position (Bevy axes; a direction, the sky's
// rotation-only view), the corner vectors a and b in 512x416 frame-buffer pixels (x right, y down; corners
// S+a, S+b, S-b, S-a with ST (0,0), (1,0), (0,1), (1,1)), the RGBA (0x80 = 1.0) and 1 for ALPHA_1 0x48.
// The view drops the camera translation (w = 0) like sky.wgsl; the pixel offsets are added in clip space
// (x / 256, -y / 208 in NDC) like particle.wgsl. Depth is 0 and unused (compare Always, no write).
//
// Fragment: MODULATE (C = Ct·Cv >> 7, A = At·Av >> 7, At in GS units), no fog, then premultiplied output for
// `One, OneMinusSrcAlpha`: rgb = Cs·As, alpha = As (0x44) or 0 (0x48, additive: every retail star).

#import bevy_pbr::mesh_view_bindings::view

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var tex: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var tex_sampler: sampler;

struct Sprite {
    // direction xyz, a.x
    p: vec4<f32>,
    // a.y, b.x, b.y, 0
    q: vec4<f32>,
    // RGBA, additive
    c: vec4<u32>,
}

struct Sprites {
    count: vec4<u32>,
    items: array<Sprite>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<storage, read> sprites: Sprites;

struct StarVertex {
    @location(0) sprite_corner: u32,
}

struct StarVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    // Flat like the GS sprite (PRIM 0x54: no Gouraud).
    @location(1) @interpolate(flat) color: vec4<u32>,
    @location(2) @interpolate(flat) additive: u32,
}

@vertex
fn vertex(v: StarVertex) -> StarVertexOutput {
    var out: StarVertexOutput;
    let i = v.sprite_corner >> 2u;
    let k = v.sprite_corner & 3u;
    if (i >= sprites.count.x) {
        // Unused quad: all four corners on one point outside the clip volume (no fragments).
        out.position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        return out;
    }
    let s = sprites.items[i];
    let a = vec2<f32>(s.p.w, s.q.x);
    let b = s.q.yz;
    var corner = a;
    var st = vec2<f32>(0.0, 0.0);
    if (k == 1u) { corner = b; st = vec2<f32>(1.0, 0.0); }
    if (k == 2u) { corner = -b; st = vec2<f32>(0.0, 1.0); }
    if (k == 3u) { corner = -a; st = vec2<f32>(1.0, 1.0); }
    let view_dir = (view.view_from_world * vec4<f32>(s.p.xyz, 0.0)).xyz;
    var clip = view.clip_from_view * vec4<f32>(view_dir, 1.0);
    // Game pixels per NDC unit across: 256, × the 16:9 widening (crate::display; as particle.wgsl).
    clip.x += corner.x * (1.0 / (256.0 * 0.775 * view.clip_from_view[1][1] / view.clip_from_view[0][0])) * clip.w;
    clip.y -= corner.y * (1.0 / 208.0) * clip.w;
    clip.z = 0.0;
    out.position = clip;
    out.uv = st;
    let c = s.c.x;
    out.color = vec4<u32>(c & 0xffu, (c >> 8u) & 0xffu, (c >> 16u) & 0xffu, c >> 24u);
    out.additive = s.c.y;
    return out;
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

@fragment
fn fragment(in: StarVertexOutput) -> @location(0) vec4<f32> {
    let t = textureSample(tex, tex_sampler, in.uv);
    // Texel in GS units: RGB 0..255, A 0..0x80 (sky_image stores the raw alpha as byte / 255).
    let ct = round(t.rgb * 255.0);
    let at = round(t.a * 255.0);
    let cv = vec3<f32>(in.color.rgb);
    let av = f32(in.color.a);
    let cs = min(floor(ct * cv / 128.0), vec3<f32>(255.0)) / 255.0;
    let a = min(floor(at * av / 128.0), 255.0);
    let as_ = a / 128.0;
    return vec4<f32>(srgb_to_linear(cs) * as_, select(min(as_, 1.0), 0.0, in.additive != 0u));
}
