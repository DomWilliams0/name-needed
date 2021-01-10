use crate::climate::ClimateIteration;
use async_trait::async_trait;

#[async_trait]
pub trait ProgressTracker {
    fn update(&mut self, step: u32, planet: Planet, climate: &ClimateIteration);
    fn fini(&mut self);
}

#[cfg(feature = "bin")]
mod gif {
    use crate::climate::ClimateIteration;
    use crate::params::{AirLayer, RenderProgressParams};
    use crate::progress::ProgressTracker;
    use crate::{Planet, Render};
    use common::*;
    use crossbeam::channel::{unbounded, Sender};
    use std::io::ErrorKind;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::thread;
    use std::thread::JoinHandle;
    use strum::IntoEnumIterator;

    pub struct GifProgressTracker {
        threads: Vec<JoinHandle<PathBuf>>,
        frames_tx: Sender<Message>,
        /// Has already rendered continents that don't change across frames
        base: Option<Render>,
        fps_str: Option<String>,
    }

    enum Message {
        Frame(u32, Render, String),
        Stop,
    }

    impl GifProgressTracker {
        /// Path is deleted then created!!
        pub fn new(out_dir: impl AsRef<Path>, threads: usize) -> std::io::Result<Self> {
            if threads < 1 {
                return Err(std::io::Error::new(ErrorKind::Other, "bad thread count"));
            }

            let out_dir = out_dir.as_ref().to_owned();
            let _ = std::fs::remove_dir_all(&out_dir);
            std::fs::create_dir_all(&out_dir)?;

            let (send, recv) = unbounded();
            let threads = (0..threads)
                .map(|_| {
                    let recv = recv.clone();
                    let out_dir = out_dir.clone();
                    thread::spawn(move || {
                        while let Ok(Message::Frame(step, render, wat)) = recv.recv() {
                            let mut path = out_dir.join(&wat);
                            std::fs::create_dir_all(&path).expect("failed to create dir for layer");

                            path.push(format!("{}-{:04}.png", wat, step));

                            let image = render.into_image();
                            image.save(path).expect("failed to save image to file");
                        }

                        out_dir
                    })
                })
                .collect();

            Ok(GifProgressTracker {
                threads,
                frames_tx: send,
                base: None,    // populated on first image
                fps_str: None, // populated on first image
            })
        }
    }

    impl ProgressTracker for GifProgressTracker {
        async fn update(&mut self, step: u32, planet: Planet, climate: &ClimateIteration) {
            let (fps, to_do) = {
                let params = &planet.inner().params.await;

                let fps = params.render.gif_fps;
                let to_do = if params.render.gif_all {
                    None
                } else {
                    Some((params.render.gif_layer, params.render.gif_progress))
                };
                (fps, to_do)
            };

            self.fps_str = Some(fps.to_string());

            let to_render = AirLayer::iter()
                .cartesian_product(RenderProgressParams::iter())
                .filter(|(layer, wat)| to_do.is_none() || to_do == Some((*layer, *wat)));

            for (layer, wat) in to_render {
                let mut render = match &self.base {
                    None => {
                        let mut render = Render::with_planet(planet.clone());
                        render.draw_continents();
                        self.base = Some(render.clone());
                        render
                    }
                    Some(base) => base.clone(),
                };

                render.draw_climate_overlay(climate, layer, wat);
                let mut gif_name = format!("{:?}-{:?}", wat, layer);
                gif_name.make_ascii_lowercase();

                self.frames_tx
                    .send(Message::Frame(step, render, gif_name))
                    .expect("failed to send");
            }
        }

        fn fini(&mut self) {
            for _ in 0..self.threads.len() {
                let _ = self.frames_tx.send(Message::Stop);
            }

            debug!("waiting on gif processing threads");

            // TODO every thread returns the same pathbuf
            let mut out_dir = None;
            self.threads
                .drain(..)
                .for_each(|t| out_dir = Some(t.join().expect("failed to join")));

            let out_dir = out_dir.expect("no threads?");
            debug!("creating progress gifs");

            let dew_it = || -> std::io::Result<()> {
                for dir in std::fs::read_dir(&out_dir)? {
                    let dir = dir?;
                    if dir.file_type()?.is_dir() {
                        let layer = dir.file_name().into_string().unwrap();
                        let input_files = dir
                            .path()
                            .join(format!("{}-%004d.png", layer))
                            .display()
                            .to_string();
                        let output_gif =
                            out_dir.join(format!("{}.gif", layer)).display().to_string();
                        let palette = out_dir.join(".palette.png").display().to_string();
                        common::debug!("writing gif to {file}", file = &output_gif);

                        // need to make palette first for smooth gif
                        if !Command::new("ffmpeg")
                            .args(&[
                                "-f",
                                "image2",
                                "-y", // overwrite
                                "-i",
                                &input_files,
                                "-vf",
                                "palettegen",
                                &palette,
                            ])
                            .spawn()?
                            .wait()?
                            .success()
                        {
                            return Err(std::io::Error::new(
                                ErrorKind::Other,
                                "ffpmeg palettegen failure",
                            ));
                        }

                        if !Command::new("ffmpeg")
                            .args(&[
                                "-f",
                                "image2",
                                "-framerate",
                                self.fps_str.as_ref().expect("fps not set"),
                                "-y", // overwrite
                                "-i",
                                &input_files,
                                "-i",
                                &palette,
                                "-lavfi",
                                "paletteuse",
                                &output_gif,
                            ])
                            .spawn()?
                            .wait()?
                            .success()
                        {
                            return Err(std::io::Error::new(ErrorKind::Other, "ffpmeg failure"));
                        }
                    }
                }

                Ok(())
            };

            dew_it().expect("failed to export gifs")
        }
    }
}

use crate::Planet;
#[cfg(feature = "bin")]
pub use gif::GifProgressTracker;

pub struct NopProgressTracker;

impl ProgressTracker for NopProgressTracker {
    fn update(&mut self, _: u32, _: Planet, _: &ClimateIteration) {}

    fn fini(&mut self) {}
}
