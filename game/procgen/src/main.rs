use common::*;
use image::{ImageBuffer, Rgb, DynamicImage, GenericImageView};
use procgen::*;
use std::io::Write;
use image::imageops::FilterType;

fn log_time(out: &mut dyn Write) -> std::io::Result<()> {
    write!(out, "the time")
}

#[cfg(feature = "bin")]
fn main() {
    let _logging = logging::LoggerBuilder::with_env()
        .and_then(|builder| builder.init(log_time))
        .expect("logging failed");
    info!("initialized logging"; "level" => ?_logging.level());

    // TODO actually configure from cmdline

    let params = PlanetParams {
        seed: 10230123,
        planet_size: 32,
        ..PlanetParams::default()
    };

    let mut planet = Planet::new(params).expect("failed");
    planet.initial_generation();

    let inner = planet.inner();

    let regions = inner.regions();
    let image = image_from_grid(
        regions.iter().map(|([x, y, _], val)| ([x, y], val.height)),
        regions.dimensions_xy(),
    );

    let dy = DynamicImage::ImageRgb8(image);
    let image = dy.resize(dy.width() * 8, dy.height() * 8, FilterType::Gaussian);
    image.save("procgen.png").expect("failed to write image");
}

fn image_from_grid(
    grid: impl Iterator<Item = ([usize; 2], f64)>,
    dims: [usize; 2],
) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    let mut image = ImageBuffer::new(dims[0] as u32, dims[1] as u32);

    for ([x, y], val) in grid {
        let pixel = (val * 220.0) as u8;
        trace!("{},{} => {:?} => {}", x, y, val, pixel);
        image.put_pixel(x as u32, y as u32, Rgb([pixel, pixel, pixel]));
    }

    image
}
