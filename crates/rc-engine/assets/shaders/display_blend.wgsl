// PS2-style blending of the effects in display bytes (crate::display_blend; docs/plan/hardware_fidelity_layers.md,
// "Result-level reproductions": additive and glowing effects).
//
// Effect draws render into a display-encoded target (Rgba8Unorm holding the frame's display bytes, the GS frame
// buffer's) with the blend `One, OneMinusSrcAlpha`, so the hardware computes the GS equations on the bytes:
// ALPHA 0x48 `Cd + (Cs·As >> 7)` and ALPHA 0x44 `Cd + ((Cs − Cd)·As >> 7)`, clamped to 0..255, stacking and
// saturating in draw order as on the PS2. These helpers give the premultiplied fragment output in display units:
// `cs` is the fragment colour in bytes (after MODULATE: it may reach 255 from a 0x80 texel and a 0xff vertex colour),
// `as_` its GS alpha (0x80 = 1.0, up to 0xff).

#define_import_path ratchet_clank::display_blend

// ALPHA 0x48 (`Cs·As + Cd`).
fn gs_add(cs: vec3<f32>, as_: f32) -> vec4<f32> {
    return vec4<f32>(min(floor(cs * as_ / 128.0), vec3<f32>(255.0)) / 255.0, 0.0);
}

// ALPHA 0x44 (`(Cs − Cd)·As + Cd`); As above 0x80 clamps to 1 in the destination factor.
fn gs_mix(cs: vec3<f32>, as_: f32) -> vec4<f32> {
    let a = min(as_ / 128.0, 1.0);
    return vec4<f32>(cs / 255.0 * a, a);
}
