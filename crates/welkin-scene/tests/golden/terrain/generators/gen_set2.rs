use welkin_scene::terrain::TerrainChannel;
use welkin_scene::terrain_layer::encode_land_layer;
fn rng(s: &mut u64) -> f32 { *s ^= *s << 13; *s ^= *s >> 7; *s ^= *s << 17; ((*s >> 11) as f64 / (1u64 << 53) as f64) as f32 }
fn main() {
    let out = std::env::args().nth(1).unwrap();
    let mut seed = 0xfeed_beef_0bad_f00du64;
    let cases: Vec<(&str, Box<dyn Fn(u32,u32,&mut u64)->f32>)> = vec![
        ("checker4096", Box::new(|x,y,_| if (x+y)%2==0 { 0.0 } else { 4096.0 })),
        ("spike", Box::new(|x,y,_| if x==128 && y==128 { 4000.0 } else { 10.0 })),
        ("spread004", Box::new(|x,y,_| 21.0 + 0.004*(((x+y)%2) as f32))),
        ("spread006", Box::new(|x,y,_| 21.0 + 0.006*(((x+y)%2) as f32))),
        ("spread100", Box::new(|x,y,_| 21.0 + 0.0 + 0.5*(((x+y)%2) as f32))),
        ("lowzero", Box::new(|x,_,s| if x < 64 { 0.0 } else { 3.0*rng(s) })),
        ("sinus", Box::new(|x,y,_| 500.0 + 400.0*((x as f32)*0.3).sin()*((y as f32)*0.25).cos())),
        ("rampy", Box::new(|x,y,_| 5.0 + 0.5*(y as f32) - 0.01*(x as f32))),
        ("noise1000", Box::new(|_,_,s| 1000.0*rng(s))),
        ("terraced", Box::new(|x,y,_| 20.0 + 8.0*(((x/16 + y/16) % 3) as f32))),
    ];
    for (name, f) in cases {
        let mut t = TerrainChannel::new_default();
        for y in 0..256u32 { for x in 0..256u32 { t.set(x, y, f(x, y, &mut seed)); } }
        let patches: Vec<(u32,u32)> = (0..16).flat_map(|py| (0..16).map(move |px| (px, py))).collect();
        let bytes = encode_land_layer(&t, &patches);
        std::fs::write(format!("{out}/{name}.bin"), &bytes).unwrap();
        let mut raw = Vec::with_capacity(65536*4);
        for y in 0..256u32 { for x in 0..256u32 { raw.extend_from_slice(&t.get(x, y).to_le_bytes()); } }
        std::fs::write(format!("{out}/{name}.f32"), raw).unwrap();
    }
}
