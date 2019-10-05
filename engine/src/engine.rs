use glium_sdl2::DisplayBuild;
use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::Keycode;
use sdl2::Sdl;

use gameloop::{FrameAction, GameLoop};
use simulation::Simulation;
use tweaker::Tweak;
use world::{WorldRef, WorldViewer};

use crate::render::{GliumRenderer, SimulationRenderer};

pub struct Engine<'a> {
    sdl: Sdl,
    renderer: GliumRenderer,
    simulation: Simulation<'a, SimulationRenderer>,
}

#[derive(Debug)]
enum KeyEvent {
    Down(Keycode),
    Up(Keycode),
}

impl<'a> Engine<'a> {
    /// Panics if SDL or glium initialisation fails
    pub fn new(world: WorldRef) -> Self {
        let sdl = sdl2::init().expect("Failed to init SDL");

        let video = sdl.video().expect("Failed to init SDL video");
        video.gl_attr().set_context_version(3, 3);
        video.gl_attr().set_context_minor_version(3);
        println!(
            "opengl {}.{}",
            video.gl_attr().context_major_version(),
            video.gl_attr().context_minor_version(),
        );

        let display = video
            .window("Name Needed", 800, 600)
            .position_centered()
            .build_glium()
            .expect("Failed to create glium window");

        video.gl_attr().set_depth_size(24);

        let renderer = GliumRenderer::new(display, WorldViewer::from_world(world.clone()));

        let simulation = Simulation::new(world.clone());

        Self {
            sdl,
            renderer,
            simulation,
        }
    }

    /// Game loop
    pub fn run(mut self) {
        let mut event_pump = self.sdl.event_pump().expect("Failed to create event pump");

        let game_loop = GameLoop::new(20, 5);

        'running: loop {
            let frame = game_loop.start_frame();

            // process events
            for event in event_pump.poll_iter() {
                match event {
                    Event::Quit { .. }
                    | Event::KeyDown {
                        keycode: Some(Keycode::Escape),
                        ..
                    } => break 'running,

                    Event::KeyDown {
                        keycode: Some(key), ..
                    } => self.handle_key(KeyEvent::Down(key)),
                    Event::KeyUp {
                        keycode: Some(key), ..
                    } => self.handle_key(KeyEvent::Up(key)),
                    Event::Window {
                        win_event: WindowEvent::Resized(w, h),
                        ..
                    } => self.renderer.on_resize(w, h),

                    Event::MouseMotion { xrel, yrel, .. }
                        if Tweak::<i64>::lookup("lookaround") == 1 =>
                    {
                        self.renderer.camera().handle_cursor(xrel, yrel)
                    }
                    _ => {}
                }
            }

            for action in frame.actions() {
                match action {
                    FrameAction::Tick => self.tick(),
                    FrameAction::Render { interpolation } => self.render(interpolation),
                }
            }
        }
    }

    fn tick(&mut self) {
        // println!("tick");
        self.simulation.tick();
        self.renderer.tick();
    }

    fn render(&mut self, interpolation: f64) {
        // println!("render ({})", interpolation);
        self.renderer.render(&mut self.simulation, interpolation);
    }

    fn handle_key(&mut self, event: KeyEvent) {
        match event {
            KeyEvent::Down(Keycode::Up) => self.renderer.world_viewer().move_up(),
            KeyEvent::Down(Keycode::Down) => self.renderer.world_viewer().move_down(),
            _ => {}
        };

        // this is silly
        let pressed = if let KeyEvent::Down(_) = event {
            true
        } else {
            false
        };
        let key = match event {
            KeyEvent::Down(k) => k,
            KeyEvent::Up(k) => k,
        };

        self.renderer.camera().handle_key(key, pressed);
    }
}
