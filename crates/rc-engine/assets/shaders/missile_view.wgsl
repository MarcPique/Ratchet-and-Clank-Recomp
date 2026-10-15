// The Visibomb's missile view overlay (docs/plan/hero_gameplay.md §17; Rust side: visibomb_view.rs).
//
// `DrawWorld` (level01 0x21a1b8) draws `0x21b9f8` at its end while 0x15f30c is set: with the record 0x16cc00
// (0x16cc30 on levels 2, 5–8, 10) it sets ALPHA_1 = 0x42 (A = 0, B = Cs, C = As, D = Cd: Cv = Cd − (Cs·As >> 7), a
// subtraction, COLCLAMP to 0) and draws `DrawRectOverlay` rectangles over the frame: the whole frame in 0x80500050
// (red and blue down by 0x50: the green look), then from y = 0 bands of 31 rows in 0x17ffffff (0x0fffffff) with 3
// rows between them (every channel down by 46 / 30). As underwater_tint.wgsl: the pixel re-encoded to the GS byte,
// blended in integers, written back.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;

struct MissileView {
    // The game frame's rectangle in target pixels (x, y, w, h).
    vp: vec4<f32>,
    // x: the bands' alpha byte (0x17 / 0x0f); y: the frame height in game rows (416); z: the band and period rows.
    band: vec4<u32>,
}
@group(0) @binding(2) var<uniform> view: MissileView;

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
    let gy = (in.position.y - view.vp.y) / view.vp.w * f32(view.band.y);
    let gx = (in.position.x - view.vp.x) / view.vp.z;
    if (gy < 0.0 || gy >= f32(view.band.y) || gx < 0.0 || gx >= 1.0) {
        return d;
    }
    var cd = vec3<i32>(round(clamp(to_display(d.rgb), vec3(0.0), vec3(1.0)) * 255.0));
    // The whole frame: Cs (0x50, 0, 0x50), As 0x80.
    cd = max(cd - ((vec3<i32>(0x50, 0, 0x50) * 0x80) >> vec3<u32>(7u)), vec3(0));
    // The bands: Cs 0xff, As the band alpha (arithmetic shift of the negative product, as the GS).
    let row = u32(floor(gy));
    if (row % view.band.z) < view.band.w {
        let a = i32(view.band.x);
        cd = max(cd + ((vec3<i32>(-255) * a) >> vec3<u32>(7u)), vec3(0));
    }
    return vec4(to_linear(vec3<f32>(cd) / 255.0), d.a);
}
