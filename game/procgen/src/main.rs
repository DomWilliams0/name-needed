use common::*;
use procgen::*;
use std::io::Write;

// TODO actually log the time
fn log_time(out: &mut dyn Write) -> std::io::Result<()> {
    write!(out, "the time")
}

#[cfg(feature = "bin")]
fn main() -> Result<(), ()> {
    let _logging = logging::LoggerBuilder::with_env()
        .and_then(|builder| builder.init(log_time))
        .expect("logging failed");
    info!("initialized logging"; "level" => ?_logging.level());

    common::panic::init_panic_detection();

    // TODO actually configure from cmdline

    fn dew_it() {
        let seed = std::env::var("NN_RANDO")
            .ok()
            .map(|_| thread_rng().gen())
            .unwrap_or(12354);
        let params = PlanetParams {
            seed,
            planet_size: 128,
            max_continents: 8,
            ..PlanetParams::default()
        };

        let mut planet = Planet::new(params).expect("failed");
        planet.initial_generation();

        let image = planet.as_image();
        let filename = "procgen.png";
        image.save(filename).expect("failed to write image");
        info!("created {file}", file = filename);
    }

    common::panic::run_and_handle_panics(dew_it)
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
