// Moby shadow volumes into the count target (docs/plan/shadows.md §6.2; Rust side: shadow_render.rs).
//
// The prisms (world space, Bevy axes) are drawn with the scene's depth as a read-only, GreaterEqual-tested
// attachment (reverse Z: the face is in front of the visible surface) into an R16Float target with additive
// blending: +1 for a front face, −1 for a back face. A visible surface inside a volume ends with a positive
// count (the z-pass rule the game builds in destination alpha). Faces are wound so the outside is front-facing.

struct ShadowView {
    clip_from_world: mat4x4<f32>,
}
@group(0) @binding(0) var<uniform> view: ShadowView;

@vertex
fn vertex(@location(0) position: vec3<f32>) -> @builtin(position) vec4<f32> {
    return view.clip_from_world * vec4<f32>(position, 1.0);
}

@fragment
fn fragment(@builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    return vec4<f32>(select(-1.0, 1.0, front), 0.0, 0.0, 0.0);
}
