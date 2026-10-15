// Sky shells as the EE-built GS packets draw them (docs/plan/sky_render_notes.md, sky_render.rs).
//
// Transform (boot 0x22bf94): p = (x, y, z, 1)·SkyM·C with the raw s16 vertex integers, C = boot 0x187040 =
// rotation-only view × the world projection, then XY = 2048 + (p.xy / p.w)·(256, 208). SkyM (the shell's
// rotation; the space skies' scaled, turned and translated shells, crate::flight_render) is the mesh transform,
// applied to (x, y, z, 1) as the game does (a level sky's entity has no translation and no scale, so that is its
// rotation alone); the view's translation is dropped by transforming the result as a direction (w = 0).
// GS Z is 0 for every sky vertex (the builders write the XYZ2 high word as 0): clip.z = 0 gives depth 0,
// which is GS Z 0 under the port's depth = Z / 2^24 mapping (game_camera.rs), the far end.
//
// Interpolation: the GS interpolates RGBA linearly in screen space, and the textured builder sends Q = 1.0,
// so S/Q and T/Q are screen-space linear too (affine texture mapping): all attributes are
// @interpolate(linear).
//
// Colour, per shell kind (PRIM 0x4b gouraud / 0x5b textured, FGE 0 = no fog for both):
// * gouraud: the GS writes the vertex RGBA as is (display-encoded bytes); As = vertex alpha.
// * textured: TFX MODULATE, TCC 1 with RGBAQ = (0x80, 0x80, 0x80, a): Cv = min(Ct·0x80 >> 7, 0xff) = Ct,
//   As = (At·a) >> 7. The texture holds the raw texel alpha (0..0x80) as byte / 255.
// Blend: ALPHA_1 = (Cs − Cd)·As >> 7 + Cd for both; the fragment returns As / 128 for SrcAlpha /
// OneMinusSrcAlpha. The textured alpha test (GEQUAL 0x80, AFAIL FB_ONLY) never suppresses the colour write.

#import bevy_pbr::{forward_io::Vertex, mesh_functions, mesh_view_bindings::view}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var tex: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var tex_sampler: sampler;

struct SkyVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(linear) uv: vec2<f32>,
    // Raw GS RGBA bytes (0..255).
    @location(1) @interpolate(linear) color: vec4<f32>,
}

@vertex
fn vertex(v: Vertex) -> SkyVertexOutput {
    var out: SkyVertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(v.instance_index);
    // (x, y, z, 1)·SkyM: the whole shell matrix (rotation, scale, translation row).
    let dir = (world_from_local * vec4<f32>(v.position, 1.0)).xyz;
    // Rotation-only view (0x186f40 without the camera translation), then the world projection.
    let view_dir = (view.view_from_world * vec4<f32>(dir, 0.0)).xyz;
    var clip = view.clip_from_view * vec4<f32>(view_dir, 1.0);
    clip.z = 0.0;
    out.position = clip;
#ifdef VERTEX_UVS_A
    out.uv = v.uv;
#endif
#ifdef VERTEX_COLORS
    out.color = v.color;
#else
    out.color = vec4<f32>(128.0);
#endif
    return out;
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

@fragment
fn fragment(in: SkyVertexOutput) -> @location(0) vec4<f32> {
    // GS integer colour: the interpolated RGBA is truncated to 8 bits per channel.
    let c = floor(in.color);
#ifdef SKY_TEXTURED
    let t = textureSample(tex, tex_sampler, in.uv);
    let rgb = min(t.rgb * c.rgb / 128.0, vec3<f32>(1.0));
    let a_s = min(floor(round(t.a * 255.0) * c.a / 128.0), 255.0);
#else
    let rgb = c.rgb / 255.0;
    let a_s = c.a;
#endif
    return vec4<f32>(srgb_to_linear(rgb), a_s / 128.0);
}
