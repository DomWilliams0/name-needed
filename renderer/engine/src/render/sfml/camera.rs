use common::input::{CameraDirection, EventHandled, KeyEvent};
use sfml::graphics::View;
use sfml::system::{SfBox, Vector2f};
use unit::dim::CHUNK_SIZE;
use unit::view::ViewPoint;
use unit::world::WorldPoint;

pub struct Camera {
    view: SfBox<View>,
    input: [bool; 4],
    velocity: Vector2f,
    centre: Vector2f,
}

impl Camera {
    pub fn new(width: u32, height: u32) -> Self {
        let mut cam = Self {
            view: View::new(Vector2f::default(), Vector2f::default()),
            input: [false; 4],
            velocity: Vector2f::default(),
            centre: Vector2f::default(),
        };
        cam.on_resize(width, height);
        cam
    }
    pub fn on_resize(&mut self, width: u32, height: u32) {
        // TODO what is this value??
        const SCALE: u32 = 64;

        // negative height to invert y axis
        self.view
            .set_size(((width / SCALE) as f32, -((height / SCALE) as f32)));

        // centre on the first chunk
        let centre = WorldPoint(CHUNK_SIZE.as_f32() / 2.0, CHUNK_SIZE.as_f32() / 2.0, 0.0);
        let centre = ViewPoint::from(centre);
        self.view.set_center((centre.0, centre.1));
        self.centre = self.view.center();
    }

    pub fn handle_key(&mut self, event: KeyEvent) -> EventHandled {
        if let Some((dir, down)) = event.parse_camera_event() {
            self.input[dir as usize] = down;
            EventHandled::Handled
        } else {
            EventHandled::NotHandled
        }
    }

    pub fn tick(&mut self) {
        let (dx, dy) = CameraDirection::values()
            .iter()
            .zip(&self.input)
            .filter(|(_, set)| **set)
            .fold((0i8, 0i8), |(x, y), (dir, _)| {
                let (dx, dy) = dir.delta();
                (x + dx, y + dy)
            });

        self.centre += self.velocity;

        if dx != 0 || dy != 0 {
            let speed = config::get().display.camera_speed;
            self.velocity.x = dx as f32 * speed;
            self.velocity.y = dy as f32 * speed;
        } else {
            self.velocity = Vector2f::default();
            self.centre = self.view.center(); // stay at the extrapolated pos
        }
    }

    pub fn view(&mut self, interpolation: f64) -> &View {
        let extrapolated = self.centre + (self.velocity * interpolation as f32);
        self.view.set_center(extrapolated);

        &self.view
    }
}
