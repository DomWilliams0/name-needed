#![cfg(feature = "bin")]

use common::*;
use procgen::*;
use std::io::Write;

// TODO actually log the time
fn log_time(out: &mut dyn Write) -> std::io::Result<()> {
    write!(out, "the time")
}

fn main() {
    // parse config and args first
    let params = PlanetParams::load_with_args("procgen.txt");

    let _logging = logging::LoggerBuilder::with_env()
        .and_then(|builder| builder.init(log_time))
        .expect("logging failed");
    info!("initialized logging"; "level" => ?_logging.level());
    debug!("config: {:#?}", params);

    common::panic::init_panic_detection();

    let dew_it = || {
        let mut planet = Planet::new(params).expect("failed");
        planet.initial_generation();

        let mut render = Render::with_planet(planet);
        render.draw_continents();
        render.save("procgen.png").expect("failed to write image");
    };

    let exit = match common::panic::run_and_handle_panics(dew_it) {
        Ok(_) => 0,
        Err(_) => 1,
    };

    // let logging end gracefully
    drop(_logging);
    std::thread::sleep(std::time::Duration::from_secs(1));

    std::process::exit(exit);
}

// fn image_from_grid(
//     grid: impl Iterator<Item = ([usize; 2], f64)>,
//     dims: [usize; 2],
// ) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
//     let mut image = ImageBuffer::new(dims[0] as u32, dims[1] as u32);
//
//     for ([x, y], val) in grid {
//         let pixel = (val * 220.0) as u8;
//         trace!("{},{} => {:?} => {}", x, y, val, pixel);
//         image.put_pixel(x as u32, y as u32, Rgb([pixel, pixel, pixel]));
//     }
//
//     image
// }
