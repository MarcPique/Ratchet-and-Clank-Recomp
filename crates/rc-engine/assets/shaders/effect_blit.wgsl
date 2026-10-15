// The effect pass's frame conversions (crate::display_blend): the view target (sRGB: textureLoad returns linear
// light) into the display-encoded effect target, whose bytes are the GS frame buffer's, and back. Both round-trip
// every byte exactly. Per pixel (the resolved frame); with MSAA every sample gets the pixel's value.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var src: texture_2d<f32>;

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

@fragment
fn to_display(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let c = textureLoad(src, vec2<i32>(floor(in.position.xy)), 0);
    return vec4<f32>(linear_to_srgb(clamp(c.rgb, vec3<f32>(0.0), vec3<f32>(1.0))), c.a);
}

@fragment
fn to_linear(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let c = textureLoad(src, vec2<i32>(floor(in.position.xy)), 0);
    return vec4<f32>(srgb_to_linear(c.rgb), c.a);
}
