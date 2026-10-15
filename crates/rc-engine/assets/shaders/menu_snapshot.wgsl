// The menu's frame-buffer snapshot (docs/plan/menus.md §7; Rust side: menu_render.rs, "Snapshot").
//
// `darken`: on the frame the menu is entered, the finished frame (the main target's output attachment, world
// + HUD) is copied into the snapshot texture with the menu's full-screen black at alpha DARKEN (0x30) already
// applied the way the GS blends it: ALPHA_1 0x44 on the frame buffer's 8-bit display values,
// `Cd + ((0 − Cd)·As >> 7)`. The source is read back to its display byte (re-encoded when its view is sRGB),
// blended in integers and written through a UNORM view of the snapshot, so the stored byte is exact.
// `copy`: passes a texture through unchanged (the window's frame, rendered offscreen on that frame, back to
// the swap chain).

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var src: texture_2d<f32>;

fn texel(pos: vec4<f32>) -> vec4<f32> {
    let size = vec2<i32>(textureDimensions(src)) - vec2<i32>(1);
    return textureLoad(src, clamp(vec2<i32>(floor(pos.xy)), vec2<i32>(0), size), 0);
}

@fragment
fn darken(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let s = texel(in.position);
#ifdef SRC_SRGB
    let c = s.rgb;
    let e = select(1.055 * pow(max(c, vec3(0.0)), vec3(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3(0.0031308));
#else
    let e = s.rgb;
#endif
    let cd = vec3<i32>(round(clamp(e, vec3(0.0), vec3(1.0)) * 255.0));
    let d = cd + ((-cd * #{DARKEN}) >> vec3<u32>(7u));
    return vec4<f32>(vec3<f32>(d) / 255.0, s.a);
}

@fragment
fn copy(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    return texel(in.position);
}
