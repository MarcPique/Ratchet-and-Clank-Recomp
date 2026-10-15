// Point lights on the world geometry (crate::world_lights; docs/plan/tfrag_lighting.md "Point lights on world
// geometry"): the point-light halves of the game's `LightTfrags` (0x2a8e40), `LightTies` (0x2ab218) and
// `LightShrubs` (0x29e7e8), added to the baked (directional) colours the static buffers hold.
//
// Colours are bytes (0x80 = 1.0); a light colour of 1.0 times a dot product of 1.0 adds 128. The game adds each
// term on the float grid 65536 + c/128, i.e. `c + floor(128 · colour · d)`, and clamps the rgb lanes (255 for
// tfrags, 243 for ties and shrubs). A nibble list names up to four bank slots, low nibble first, 0xf ends it.

#define_import_path ratchet_clank::world_lights

// pos.xyz = position (Bevy world), pos.w = radius; col.rgb = colour, col.w = intensity (the back factor w:
// d -> max(d, d·w)).
struct WorldLight {
    pos: vec4<f32>,
    col: vec4<f32>,
}

const NO_LIGHTS: u32 = 0xffffu;

// `LightTfrags`, point pass (after the directional one, on its packed result): per listed light whose radius
// reaches the vertex (r² − |P − L|² not negative), d = (−N)·(P − L)·(1 − dist/r)/dist, d = max(d, d·w), then
// rgba += floor(128 · (colour, w) · d) (all four lanes: the intensity also feeds alpha), rgb clamped at 255.
// `c0` = the vertex's baked bytes, `p` = its position, `n` = (N, 1 when the vertex has a light record).
fn tfrag_lit(list: u32, lights: array<WorldLight, 8>, c0: vec4<f32>, p: vec3<f32>, n: vec4<f32>) -> vec4<f32> {
    if (n.w == 0.0) { return c0; }
    var ls = lights;
    var c = round(c0);
    var l = list | 0xf0000u;
    loop {
        let i = l & 0xfu;
        if (i == 0xfu) { break; }
        l = l >> 4u;
        let light = ls[i & 7u];
        let v = p - light.pos.xyz;
        let d2 = dot(v, v);
        let r = light.pos.w;
        if (r * r - d2 < 0.0 || d2 <= 0.0) { continue; }
        let dist = sqrt(d2);
        var d = -dot(n.xyz, v) * ((1.0 - dist / r) / dist);
        d = max(d, d * light.col.w);
        let rgb = clamp(c.rgb + floor(128.0 * light.col.rgb * d), vec3<f32>(0.0), vec3<f32>(255.0));
        let a = c.a + floor(128.0 * light.col.w * d);
        c = vec4<f32>(rgb, a - 256.0 * floor(a / 256.0));
    }
    return c;
}

// The third light `LightTies` / `LightShrubs` merge from the listed lights that reach the instance's
// bounding-sphere centre: direction = sum of the unit vectors (centre − L) (renormalised only when two or more
// lights reach it), colour = sum of colour · (1 − dist/r), back factor = sum of w · (1 − dist/r).
struct InstanceLight {
    dir: vec3<f32>,
    col: vec3<f32>,
    back: f32,
    hit: bool,
}

fn instance_light(list: u32, lights: array<WorldLight, 8>, centre: vec3<f32>) -> InstanceLight {
    var ls = lights;
    var out = InstanceLight(vec3<f32>(0.0), vec3<f32>(0.0), 0.0, false);
    var n = 0u;
    var l = list | 0xf0000u;
    loop {
        let i = l & 0xfu;
        if (i == 0xfu) { break; }
        l = l >> 4u;
        let light = ls[i & 7u];
        let v = centre - light.pos.xyz;
        let d2 = dot(v, v);
        let r = light.pos.w;
        if (r * r - d2 < 0.0) { continue; }
        let dist = sqrt(d2);
        let a = 1.0 - dist / r;
        if (dist > 0.0) { out.dir += v / dist; }
        out.col += light.col.rgb * a;
        out.back += light.col.w * a;
        n += 1u;
    }
    let len2 = dot(out.dir, out.dir);
    if (n >= 2u && len2 > 0.0) { out.dir *= inverseSqrt(len2); }
    out.hit = n > 0u;
    return out;
}

// One lit colour of the instance (bytes): the merged light on the class normal `nw` (in world axes: the class
// normal through the instance's unit axis columns), d = −dir·nw, d = max(d, d·back), rgb += floor(128 · col · d),
// clamped at `top` (243).
fn instance_lit(c: vec3<f32>, il: InstanceLight, nw: vec3<f32>, top: f32) -> vec3<f32> {
    var d = -dot(il.dir, nw);
    d = max(d, d * il.back);
    return clamp(c + floor(128.0 * il.col * d), vec3<f32>(0.0), vec3<f32>(top));
}

// A packed RGBA8 colour with the merged light on class normal `n` (s16 / 32768 units) of an instance whose
// class -> world matrix is `m` (the unit axis columns remove scale, as the game's `c · rsqrt(|c|²)` / `1/|c|`).
fn instance_lit_packed(rgba: u32, il: InstanceLight, n: vec3<f32>, m: mat4x4<f32>, top: f32) -> u32 {
    let nw = normalize(m[0].xyz) * n.x + normalize(m[1].xyz) * n.y + normalize(m[2].xyz) * n.z;
    let c = vec3<f32>(f32(rgba & 0xffu), f32((rgba >> 8u) & 0xffu), f32((rgba >> 16u) & 0xffu));
    let lit = vec3<u32>(instance_lit(c, il, nw, top));
    return lit.x | (lit.y << 8u) | (lit.z << 16u) | (rgba & 0xff000000u);
}
