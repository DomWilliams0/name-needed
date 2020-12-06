use image::{ImageBuffer, Rgb};
use procgen::*;

#[cfg(feature = "bin")]
fn main() {
    let mut args = std::env::args().skip(1);
    let radius: i32 = args
        .next()
        .and_then(|s| s.parse().ok())
        .expect("bad radius");
    let seed: u32 = args.next().and_then(|s| s.parse().ok()).expect("bad seed");
    let noise_scale: f64 = args.next().and_then(|s| s.parse().ok()).expect("bad scale");
    assert!(args.next().is_none(), "trailing args");
    let chunk_size = 8i32;

    todo!()

    // let diameter = (chunk_size * ((2 * radius) + 1)) as u32;
    // let mut image = ImageBuffer::new(diameter, diameter);
    //
    // for cy in -radius..=radius {
    //     for cx in -radius..=radius {
    //         let chunk = generate_chunk((cx, cy), chunk_size as usize, seed as u64, noise_scale);
    //
    //         for (i, height) in chunk.heightmap.iter().enumerate() {
    //             let i = i as i32;
    //             let bx = i % chunk_size;
    //             let by = i / chunk_size;
    //
    //             let px = ((cx + radius) * chunk_size) + bx;
    //             let py = ((cy + radius) * chunk_size) + by;
    //
    //             let pixel = (*height * 220.0) as u8;
    //             // println!("{},{} | {},{} => {},{} = {}", cx, cy, bx, by, px, py, pixel);
    //             image.put_pixel(px as u32, py as u32, Rgb([pixel, pixel, pixel]));
    //         }
    //     }
    // }
    //
    // image.save("procgen.png").expect("failed to write image");
}
