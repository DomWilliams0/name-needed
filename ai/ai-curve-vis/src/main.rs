//! Helper to render
//!
use std::error::Error;
use std::io::BufRead;
use std::path::Path;

use plotters::prelude::*;
use plotters::style::WHITE;

use ai::Curve;
use common::NormalizedFloat;

fn main() {
    let stdin = std::io::stdin();
    let stdin = stdin.lock();

    let outpath = std::env::temp_dir().join("ai-curve.png");

    println!("reading lines from stdin");
    for line in stdin.lines() {
        let line = line.expect("bad input");

        let curve: Curve = match ron::from_str(&line) {
            Ok(c) => c,
            Err(err) => {
                println!("bad curve: {}", err);
                continue;
            }
        };
        draw(curve, &outpath).expect("plotting failed");
        println!("wrote to {}", outpath.display());
    }
}

fn draw(curve: Curve, outpath: &Path) -> Result<(), Box<dyn Error>> {
    let root = BitMapBackend::new(outpath, (800, 800)).into_drawing_area();
    root.fill(&WHITE)?;

    let range = 0.0..1.0f32;
    let mut chart = ChartBuilder::on(&root)
        .caption(format!("{:?}", curve), ("sans-serif", 50).into_font())
        .x_label_area_size(30)
        .y_label_area_size(30)
        .build_cartesian_2d(range.clone(), range)?;

    chart
        .configure_mesh()
        .x_desc("Input value")
        .y_desc("Curve output")
        .draw()?;

    let mut x = 0.0;
    let xs = std::iter::from_fn(|| {
        let ret = if x <= 1.0 {
            Some(NormalizedFloat::new(x))
        } else {
            None
        };
        x += 0.005;
        ret
    });
    chart.draw_series(LineSeries::new(
        xs.map(|x| (x.value(), curve.evaluate(x).value())),
        &RED,
    ))?;
    Ok(())
}
