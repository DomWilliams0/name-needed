use cgmath::ortho;

use common::input::{CameraDirection, EventHandled, KeyEvent};
use common::*;
use unit::dim::CHUNK_SIZE;
use unit::view::ViewPoint;
use unit::world::{ChunkPosition, WorldPoint, WorldPosition, SCALE};

pub struct Camera {
    /// Camera pos in screen space
    pos: Point2,
    velocity: Vector2,
    last_extrapolated_pos: Point2,
    input: [bool; 4],
    zoom: f32,
    window_size: Vector2,
}

const CAMERA_Z: f32 = 10.0;
const SCREEN_SCALE: f32 = 64.0;

impl Camera {
    pub fn new(width: i32, height: i32) -> Self {
        let mut cam = Self {
            input: [false; 4],
            velocity: Vector2::zero(),
            pos: Point2::new(0.0, 0.0),
            last_extrapolated_pos: Point2::new(0.0, 0.0),
            zoom: 1.0,
            window_size: Vector2::zero(), // set in on_resize
        };
        cam.on_resize(width, height);

        // centre on the first chunk initially
        let centre = WorldPoint(CHUNK_SIZE.as_f32() / 2.0, CHUNK_SIZE.as_f32() / 2.0, 0.0);
        cam.set_centre(centre.into());

        cam
    }

    fn set_centre(&mut self, centre: ViewPoint) {
        self.pos = Point2::new(centre.0, centre.1) - (self.window_size / 2.0 / SCREEN_SCALE);
    }

    pub fn on_resize(&mut self, width: i32, height: i32) {
        let w = width as f32;
        let h = height as f32;

        let new_sz = Vector2::new(w, h);
        let old_sz = std::mem::replace(&mut self.window_size, new_sz);

        // keep screen centre in the same place TODO only sometimes?
        let delta = (new_sz - old_sz) / SCREEN_SCALE;
        self.pos -= delta;
    }

    pub fn handle_key(&mut self, event: KeyEvent) -> EventHandled {
        // TODO zoom
        if let Some((dir, down)) = event.parse_camera_event() {
            self.input[dir as usize] = down;
            EventHandled::Handled
        } else {
            EventHandled::NotHandled
        }
    }

    pub fn tick(&mut self) -> (ChunkPosition, ChunkPosition) {
        let (dx, dy) = CameraDirection::values()
            .iter()
            .zip(&self.input)
            .filter(|(_, set)| **set)
            .fold((0i8, 0i8), |(x, y), (dir, _)| {
                let (dx, dy) = dir.delta();
                (x + dx, y + dy)
            });

        self.pos += self.velocity;

        if dx != 0 || dy != 0 {
            let speed = config::get().display.camera_speed;
            self.velocity.x = dx as f32 * speed;
            self.velocity.y = dy as f32 * speed;
        } else {
            self.velocity.set_zero();
            self.pos = self.last_extrapolated_pos;
        }

        // calculate visible chunk bounds
        // TODO cache
        let bottom_left = WorldPosition::from(ViewPoint::from(self.pos));
        let top_right = {
            let mul = 4.0 * self.zoom * SCALE / SCREEN_SCALE;
            let hor = (mul * self.window_size.x).ceil() as i32;
            let ver = (mul * self.window_size.y).ceil() as i32;

            bottom_left + (hor, ver, 0)
        };

        (bottom_left.into(), top_right.into())
    }

    fn position(&self, interpolation: f64) -> Point3 {
        let pos = self.pos + (self.velocity * interpolation as f32);
        Point3::new(pos.x, pos.y, CAMERA_Z)
    }

    pub fn view_matrix(&mut self, interpolation: f64) -> Matrix4 {
        let pos = self.position(interpolation);
        self.last_extrapolated_pos = Point2::new(pos.x, pos.y);
        Matrix4::look_at_dir(pos, -AXIS_UP, AXIS_FWD)
    }

    pub fn projection_matrix(&self) -> Matrix4 {
        let zoom = self.zoom / SCREEN_SCALE;

        ortho(
            0.0,
            zoom * self.window_size.x,
            0.0,
            zoom * self.window_size.y,
            0.0,
            100.0,
        )
    }
}
