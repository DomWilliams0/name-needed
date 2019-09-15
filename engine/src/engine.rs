use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use std::time::Instant;
use sdl2::render::{CanvasBuilder, WindowCanvas};
use sdl2::Sdl;
use sdl2::pixels::Color;

const TICKS_PER_SECOND: usize = 20;
const SKIP_TICKS: usize = 1000 / TICKS_PER_SECOND;
const MAX_FRAMESKIP: u32 = 5;

pub struct Engine {
    sdl: Sdl,
    canvas: WindowCanvas,
}

impl Engine {
    /// Panics if SDL fails
    pub fn new() -> Self {
        let sdl = sdl2::init().expect("Failed to init SDL");

        let video = sdl.video().expect("Failed to init SDL video");

        let window = video
            .window("Name Needed", 800, 600)
            .position_centered()
            .build()
            .expect("Failed to create window");

        let canvas = CanvasBuilder::new(window)
            .accelerated()
            .present_vsync()
            .build()
            .expect("Failed to create canvas");

        Self {
            sdl,
            canvas,
        }
    }

    /// Game loop
    pub fn run(mut self) {
        let mut event_pump = self.sdl.event_pump().expect("Failed to create event pump");

        // deWITTERS game loop
        let start_time = Instant::now();
        let mut next_game_tick: usize = start_time.elapsed().as_millis() as usize;

        'running: loop {
            // process events
            for event in event_pump.poll_iter() {
                match event {
                    Event::Quit { .. }
                    | Event::KeyDown {
                        keycode: Some(Keycode::Escape),
                        ..
                    } => {
                        break 'running;
                    }
                    _ => {}
                }
            }

            let mut loops = 0;
            let now = start_time.elapsed().as_millis() as usize;
            while now > next_game_tick && loops < MAX_FRAMESKIP {
                self.tick();

                next_game_tick += SKIP_TICKS;
                loops += 1;
            }

            let now = start_time.elapsed().as_millis() as usize;
            let interpolation: f64 = ((now + SKIP_TICKS - next_game_tick) as f64) / (SKIP_TICKS as f64);

            self.render(interpolation);
        }
    }

    fn tick(&mut self) {
        println!("tick");
    }

    fn render(&mut self, interpolation: f64) {
        // clear
        let bg = Color::RGB(17, 17, 19);
        self.canvas.set_draw_color(bg);
        self.canvas.clear();

        println!("render ({})", interpolation);

        // render
        self.canvas.present();
    }
}
