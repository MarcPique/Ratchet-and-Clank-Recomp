//! Level tables → JSON, from the gameplay file through the golden-tested parsers: moby instances (with their pvar
//! blocks), tie and shrub instances, volumes (cuboids, spheres, cylinders, pills), paths, grind paths, sound
//! instances, env sample points, fog zones and the ship placement. Every stored field is written; matrices as
//! stored (row or column form as each record documents it in `rc_formats`), floats bit-exact (shortest f32 text).

use super::data::{self, lvl};
use super::jsonv::{hex, hex_bytes, Obj, J};
use super::Out;
use crate::Error;
use rc_formats::volumes::{self, Shape};
use rc_formats::{gameplay, shrub, sound_bank, tie};
use std::path::Path;

fn m4(m: &[[f32; 4]; 4]) -> J { J::Arr(m.iter().map(|r| J::from(*r)).collect()) }
fn m34(m: &[[f32; 4]; 3]) -> J { J::Arr(m.iter().map(|r| J::from(*r)).collect()) }

fn shape(s: &Shape) -> J { Obj::new().set("matrix", m4(&s.matrix)).set("inverse", m34(&s.inverse)).set("euler", s.euler).set("unused_7c", s.unused_7c).build() }

fn table(source: &str, what: &str, note: &str, items: Vec<J>) -> J {
    Obj::new().set("source", source).set("table", what).set("note", note).set("count", items.len()).set("items", J::Arr(items)).build()
}

pub(crate) fn export_level(data: &Path, out: &Out, id: u32) -> Result<(), Error> {
    let g = data::gameplay(data, id)?;
    let src = lvl(id, "gameplay_ntsc.bin");
    let dir = format!("levels/{id:02}");
    let skip = |what: &str, e: rc_formats::FormatError| out.skip(format!("{src} {what}: {e}"));

    match gameplay::parse_moby_instances(&g) {
        Ok(v) => {
            let pvars = gameplay::parse_pvars(&g).unwrap_or_default();
            let items = v.iter().enumerate().map(|(i, m)| {
                Obj::new().set("index", i).set("o_class", m.o_class).set("position", m.position).set("rotation", m.rotation).set("scale", m.scale)
                    .set("size", m.size).set("unknown_4", m.unknown_4).set("spawn_flags", m.spawn_flags).set("spawn_id", m.spawn_id)
                    .set("unknown_10", m.unknown_10).set("unknown_14", m.unknown_14).set("draw_distance", m.draw_distance).set("update_distance", m.update_distance)
                    .set("unused_28", m.unused_28).set("unused_2c", m.unused_2c).set("group", m.group).set("is_rooted", m.is_rooted)
                    .set("rooted_distance", m.rooted_distance).set("unknown_54", m.unknown_54).set("pvar_index", m.pvar_index).set("occlusion", m.occlusion)
                    .set("mode_bits", hex(m.mode_bits as u32 as u64)).set("color", m.color).set("light", hex(m.light as u32 as u64)).set("unknown_74", m.unknown_74)
                    .set("pvar", usize::try_from(m.pvar_index).ok().and_then(|p| pvars.get(p)).and_then(|p| p.as_deref()).map(hex_bytes))
                    .build()
            }).collect();
            out.json(&format!("{dir}/mobys.json"), &table(&src, "moby instances (gameplay +0x44)", "rotation: Euler radians, R = Rz·Ry·Rx; pvar: the instance's pvar block (hex)", items))?;
        }
        Err(e) => skip("moby instances", e),
    }
    match tie::parse_tie_instances(&g) {
        Ok(v) => {
            let items = v.iter().enumerate().map(|(i, t)| {
                Obj::new().set("index", i).set("o_class", t.o_class).set("uid", t.uid).set("draw_distance", t.draw_distance).set("occlusion_index", t.occlusion_index)
                    .set("matrix", m4(&t.matrix)).set("directional_lights", t.directional_lights).set("ambient_rgba5551", &t.ambient_rgbas[..]).build()
            }).collect();
            out.json(&format!("{dir}/ties.json"), &table(&src, "tie instances (gameplay +0x34)", "matrix: column-major matrix[col][row], translation in column 3; [3][3] as stored", items))?;
        }
        Err(e) => skip("tie instances", e),
    }
    match shrub::parse_shrub_instances(&g) {
        Ok(v) => {
            let items = v.iter().enumerate().map(|(i, s)| {
                Obj::new().set("index", i).set("o_class", s.o_class).set("draw_distance", s.draw_distance).set("matrix", m4(&s.matrix))
                    .set("colour", s.colour).set("dir_lights", hex(s.dir_lights as u32 as u64)).build()
            }).collect();
            out.json(&format!("{dir}/shrubs.json"), &table(&src, "shrub instances (gameplay +0x3c)", "matrix: column-major matrix[col][row], translation in column 3; [3][3] as stored", items))?;
        }
        Err(e) => skip("shrub instances", e),
    }
    match volumes::parse_volumes(&g) {
        Ok(v) => {
            let doc = Obj::new().set("source", src.as_str())
                .set("note", "matrix: local → world, row-vector form (world = l.x·row0 + l.y·row1 + l.z·row2 + row3); inverse: world → local rows applied to p − centre")
                .set("cuboids", J::Arr(v.cuboids.iter().map(shape).collect()))
                .set("spheres", J::Arr(v.spheres.iter().map(shape).collect()))
                .set("cylinders", J::Arr(v.cylinders.iter().map(shape).collect()))
                .set("pills", J::Arr(v.pills.iter().map(shape).collect()))
                .set("pill_tail", &v.pill_tail[..])
                .build();
            out.json(&format!("{dir}/volumes.json"), &doc)?;
            let paths = v.paths.iter().enumerate().map(|(i, p)| Obj::new().set("index", i).set("points", J::Arr(p.iter().map(|q| J::from(*q)).collect())).build()).collect();
            out.json(&format!("{dir}/paths.json"), &table(&src, "paths (gameplay +0x70)", "points: x, y, z, w", paths))?;
            let grind = v.grind_paths.iter().enumerate().map(|(i, p)| {
                Obj::new().set("index", i).set("bsphere", p.bsphere).set("flag", p.flag).set("points", J::Arr(p.points.iter().map(|q| J::from(*q)).collect())).build()
            }).collect();
            out.json(&format!("{dir}/grind_paths.json"), &table(&src, "grind paths (gameplay +0x74)", "points: x, y, z, w", grind))?;
        }
        Err(e) => skip("volumes", e),
    }
    match sound_bank::parse_sound_instances(&g) {
        Ok(v) => {
            let items = v.iter().enumerate().map(|(i, s)| {
                Obj::new().set("index", i).set("o_class", s.o_class).set("m_class", s.m_class).set("pvar_index", s.pvar_index).set("range", s.range)
                    .set("matrix", m4(&s.matrix)).set("inverse", m34(&s.inverse)).set("rotation", s.rotation)
                    .set("pvar", sound_bank::pvar_block(&g, s.pvar_index).ok().flatten().map(|p| hex_bytes(&p))).build()
            }).collect();
            out.json(&format!("{dir}/sound_instances.json"), &table(&src, "sound instances (gameplay +0x0c)", "o_class: 0 sphere, 1 box, 2 box one-shot, 3 reverb box, 5 underwater loop, 6 music box", items))?;
        }
        Err(e) => skip("sound instances", e),
    }
    match sound_bank::parse_env_sample_points(&g) {
        Ok(v) => {
            let items = v.iter().enumerate().map(|(i, p)| {
                Obj::new().set("index", i).set("position", p.position).set("reverb_depth", p.reverb_depth).set("reverb_type", p.reverb_type)
                    .set("reverb_delay", p.reverb_delay).set("reverb_feedback", p.reverb_feedback).set("reverb_enable", p.reverb_enable).set("music_track", p.music_track).build()
            }).collect();
            out.json(&format!("{dir}/env_sample_points.json"), &table(&src, "env sample points (gameplay +0x88)", "", items))?;
        }
        Err(e) => skip("env sample points", e),
    }
    match gameplay::parse_fog_zones(&g) {
        Ok(f) => {
            let items = f.zones.iter().zip(&f.circles).enumerate().map(|(i, (z, c))| {
                Obj::new().set("index", i).set("circle", *c).set("inverse", m4(&z.inverse)).set("hero_color", J::Arr(z.hero_color.iter().map(|&v| hex(v as u64)).collect()))
                    .set("hero_light", z.hero_light).set("flags", z.flags).set("fog_color", J::Arr(z.fog_color.iter().map(|&v| hex(v as u64)).collect()))
                    .set("side", J::Arr(z.side.iter().map(|s| Obj::new().set("near_dist", s.near_dist).set("near_density", s.near_density).set("far_dist", s.far_dist).set("far_density", s.far_density).build()).collect()))
                    .set("unused_7c", z.unused_7c).build()
            }).collect();
            out.json(&format!("{dir}/fog_zones.json"), &table(&src, "environment transitions (gameplay +0x80)", "circle: x, y, z, r²; hero_light −1 already mapped to 0xb as the loader does", items))?;
        }
        Err(e) => skip("fog zones", e),
    }
    let ship = gameplay::ship_placement(&g).ok().flatten();
    out.json(&format!("{dir}/level.json"), &Obj::new().set("source", src.as_str()).set("level", id)
        .set("ship", ship.map(|(p, yaw)| Obj::new().set("position", p).set("yaw", yaw).build())).build())
}
