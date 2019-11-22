use glium_sdl2::DisplayBuild;
use log::*;
use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::Keycode;
use sdl2::Sdl;

use gameloop::{FrameAction, GameLoop};
use simulation::{self, Simulation};
use world::{WorldRef, WorldViewer};

use crate::render::{self, GliumRenderer, SimulationRenderer};

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
        debug!(
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

        let simulation = Simulation::new(world);

        Self {
            sdl,
            renderer,
            simulation,
        }
    }

    /// Game loop
    pub fn run(mut self) {
        let mut event_pump = self.sdl.event_pump().expect("Failed to create event pump");

        // TODO separate faster rate for physics?
        let game_loop = GameLoop::new(simulation::TICKS_PER_SECOND, 5);

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

                    Event::MouseButtonDown { .. } => self.renderer.camera().handle_click(true),
                    Event::MouseButtonUp { .. } => self.renderer.camera().handle_click(false),
                    Event::MouseMotion { xrel, yrel, .. } => {
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
        trace!("tick");
        self.simulation.tick();
        self.renderer.tick();
    }

    fn render(&mut self, interpolation: f64) {
        trace!("render (interpolation={})", interpolation);
        self.renderer.render(&mut self.simulation, interpolation);
    }

    fn handle_key(&mut self, event: KeyEvent) {
        match event {
            KeyEvent::Down(Keycode::Up) => self.renderer.world_viewer().move_by(1),
            KeyEvent::Down(Keycode::Down) => self.renderer.world_viewer().move_by(-1),
            KeyEvent::Down(Keycode::Y) => {
                let wireframe = unsafe { render::wireframe_world_toggle() };
                debug!(
                    "world is {} wireframe",
                    if wireframe { "now" } else { "no longer" }
                )
            }
            KeyEvent::Down(Keycode::U) => {
                let rendering = self.simulation.toggle_physics_debug_rendering();
                debug!(
                    "{} physics debug rendering",
                    if rendering { "enabled" } else { "disabled" }
                )
            }
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
