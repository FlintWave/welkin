// Generator for the first nine terrains (Rust, uses welkin-scene at 854b115). Writes <name>.bin and <name>.f32.
use welkin_scene::terrain::TerrainChannel;
use welkin_scene::terrain_layer::encode_land_layer;
fn rng(s: &mut u64) -> f32 { *s ^= *s << 13; *s ^= *s >> 7; *s ^= *s << 17; ((*s >> 11) as f64 / (1u64 << 53) as f64) as f32 }
fn main() {
    let out = std::env::args().nth(1).unwrap();
    let mut seed = 0x1234_5678_9abc_def0u64;
    let cases: Vec<(&str, Box<dyn Fn(u32,u32,&mut u64)->f32>)> = vec![
        ("flat21", Box::new(|_,_,_| 21.0)),
        ("hills", Box::new(|x,y,_| 21.0 + 8.0*((x as f32)*0.05).sin()*((y as f32)*0.04).cos() + 3.0*((x as f32)*0.2+(y as f32)*0.1).sin())),
        ("noise2m", Box::new(|_,_,s| 30.0 + 2.0*rng(s))),
        ("noise60m", Box::new(|_,_,s| 40.0 + 60.0*rng(s))),
        ("ramp", Box::new(|x,y,_| 0.25*(x as f32) + 0.1*(y as f32))),
        ("step", Box::new(|x,_,_| if x < 100 { 10.0 } else { 120.0 })),
        ("negative", Box::new(|x,y,_| -5.0 + 0.02*(x as f32) - 0.01*(y as f32))),
        ("subtle", Box::new(|x,_,_| 21.0 + 0.3*((x as f32)/255.0))),
        ("mountain", Box::new(|x,y,_| { let dx=x as f32-128.0; let dy=y as f32-128.0; 20.0 + 180.0*(-(dx*dx+dy*dy)/4000.0).exp() })),
    ];
    for (name, f) in cases {
        let mut t = TerrainChannel::new_default();
        for y in 0..256u32 { for x in 0..256u32 { t.set(x, y, f(x, y, &mut seed)); } }
        let patches: Vec<(u32,u32)> = (0..16).flat_map(|py| (0..16).map(move |px| (px, py))).collect();
        std::fs::write(format!("{out}/{name}.bin"), encode_land_layer(&t, &patches)).unwrap();
        let mut raw = Vec::with_capacity(65536*4);
        for y in 0..256u32 { for x in 0..256u32 { raw.extend_from_slice(&t.get(x, y).to_le_bytes()); } }
        std::fs::write(format!("{out}/{name}.f32"), raw).unwrap();
    }
}
