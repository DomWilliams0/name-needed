use crate::climate::ClimateIteration;

pub trait ProgressTracker {
    fn update(&mut self, planet: Planet, climate: &ClimateIteration);
    fn fini(&mut self);
}

#[cfg(feature = "bin")]
mod gif {
    use crate::climate::ClimateIteration;
    use crate::progress::ProgressTracker;
    use crate::{Planet, Render};
    use image::gif::GifEncoder;
    use image::{Delay, Frame};
    use std::fs::File;
    use std::path::Path;
    use std::sync::mpsc::{sync_channel, SyncSender};
    use std::thread;
    use std::thread::JoinHandle;
    use std::time::Duration;

    pub struct GifProgressTracker {
        thread: Option<JoinHandle<GifEncoder<File>>>,
        frames_tx: SyncSender<Message>,
    }

    enum Message {
        Frame(Frame),
        Stop,
    }

    impl GifProgressTracker {
        pub fn new(out_path: impl AsRef<Path>) -> std::io::Result<Self> {
            let out_path = out_path.as_ref();
            let file = File::create(out_path)?;
            let mut encoder = GifEncoder::new(file);

            common::info!("writing out gif to {path}", path = out_path.display());

            let (send, recv) = sync_channel(32);
            let thread = thread::spawn(move || {
                let mut idx = 0;
                while let Ok(Message::Frame(frame)) = recv.recv() {
                    common::trace!("processing gif frame {frame}", frame = idx);
                    idx += 1;
                    encoder
                        .encode_frame(frame)
                        .expect("failed to encode progress gif frame");
                }

                encoder
            });

            Ok(GifProgressTracker {
                thread: Some(thread),
                frames_tx: send,
            })
        }
    }

    impl ProgressTracker for GifProgressTracker {
        fn update(&mut self, planet: Planet, climate: &ClimateIteration) {
            let (fps, layer) = {
                let params = &planet.inner().params;

                let fps = 1.0 / params.render.gif_fps as f32;
                let layer = params.render.climate_gif_layer;
                (fps, layer)
            };

            let mut render = Render::with_planet(planet);
            render.draw_continents();
            render.draw_climate_overlay(climate, layer);

            let frame = Frame::from_parts(
                render.into_image(),
                0,
                0,
                Delay::from_saturating_duration(Duration::from_secs_f32(fps)),
            );

            self.frames_tx
                .send(Message::Frame(frame))
                .expect("failed to send");
        }

        fn fini(&mut self) {
            let _ = self.frames_tx.send(Message::Stop);

            let thread = self.thread.take().expect("no thread");
            common::debug!("waiting on gif thread to finish");
            let _encoder = thread.join().expect("failed to join");
        }
    }
}

use crate::Planet;
#[cfg(feature = "bin")]
pub use gif::GifProgressTracker;

pub struct NopProgressTracker;

impl ProgressTracker for NopProgressTracker {
    fn update(&mut self, _: Planet, _: &ClimateIteration) {}

    fn fini(&mut self) {}
}
