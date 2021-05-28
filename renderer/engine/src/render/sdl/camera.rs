use cgmath::ortho;

use common::input::CameraDirection;
use common::*;
use std::convert::TryFrom;
use unit::space::view::ViewPoint;
use unit::world::{ChunkLocation, WorldPoint, WorldPosition};
use unit::world::{BLOCKS_SCALE, CHUNK_SIZE};

pub struct Camera {
    /// Camera pos in metres
    pos: Point2,
    velocity: Vector2,
    last_extrapolated_pos: Point2,
    input: [bool; 4],
    zoom: f32,
    window_size: Vector2,
}

const SCREEN_SCALE: f32 = 64.0;

impl Camera {
    pub fn new(width: i32, height: i32) -> Self {
        let mut cam = Self {
            input: [false; 4],
            velocity: Vector2::zero(),
            pos: Point2::new(0.0, 0.0),
            last_extrapolated_pos: Point2::new(0.0, 0.0),
            zoom: config::get().display.initial_zoom,
            window_size: Vector2::zero(), // set in on_resize
        };
        cam.on_resize(width, height);

        // centre on the first chunk initially
        let centre =
            WorldPoint::new_unchecked(CHUNK_SIZE.as_f32() / 2.0, CHUNK_SIZE.as_f32() / 2.0, 0.0);
        cam.set_centre(centre);

        cam
    }

    pub(crate) fn set_centre(&mut self, centre: impl Into<ViewPoint>) {
        let (x, y, _) = centre.into().xyz();
        self.pos = Point2::new(x, y) - (self.window_size / 2.0 / SCREEN_SCALE);
        self.last_extrapolated_pos = self.pos;
    }

    pub fn on_resize(&mut self, width: i32, height: i32) {
        let w = width as f32;
        let h = height as f32;

        let new_sz = Vector2::new(w, h);
        let old_sz = std::mem::replace(&mut self.window_size, new_sz);

        // keep screen centre in the same place
        let delta = (new_sz - old_sz) / SCREEN_SCALE / 2.0 * self.zoom;

        self.pos -= delta;
        self.last_extrapolated_pos = self.pos;
    }

    pub fn handle_move(&mut self, direction: CameraDirection, is_down: bool) {
        self.input[direction as usize] = is_down;
    }

    pub fn handle_zoom(&mut self, mut delta: i32) {
        if delta.abs() > 1 {
            warn!(
                "mouse wheel scrolled faster than expected, investigate me ({})",
                delta
            );
            delta = delta.signum();
        }

        let speed = config::get().display.camera_zoom_speed;
        self.zoom = (self.zoom - (speed * delta as f32)).clamp(0.1, 6.0);

        // TODO zoom into mouse position/screen centre
        // TODO interpolate zoom
    }

    pub fn tick(&mut self) -> (ChunkLocation, ChunkLocation) {
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
            let speed = config::get().display.camera_move_speed;
            self.velocity.x = dx as f32 * speed;
            self.velocity.y = dy as f32 * speed;
        } else {
            self.velocity.set_zero();
            self.pos = self.last_extrapolated_pos;
        }

        // calculate visible chunk bounds
        // TODO cache
        let view_point = ViewPoint::try_from(self.pos).expect("invalid camera position");
        let bottom_left = WorldPosition::from(view_point);
        let top_right = {
            let mul = self.zoom / SCREEN_SCALE / BLOCKS_SCALE;
            let hor = (mul * self.window_size.x).ceil() as i32;
            let ver = (mul * self.window_size.y).ceil() as i32;

            bottom_left + (hor, ver, 0)
        };

        (bottom_left.into(), top_right.into())
    }

    fn position(&self, interpolation: f64, z: f32) -> Point3 {
        let pos = self.pos + (self.velocity * interpolation as f32);
        Point3::new(pos.x, pos.y, z)
    }

    pub fn view_matrix(&mut self, interpolation: f64, z: f32) -> Matrix4 {
        let pos = self.position(interpolation, z);
        self.last_extrapolated_pos = Point2::new(pos.x, pos.y);
        Matrix4::look_to_rh(pos, -AXIS_UP, AXIS_FWD)
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

    /// Returns (x, y) in world scale
    pub fn screen_to_world(&self, screen_pos: (i32, i32)) -> (f32, f32) {
        let offset = {
            let xpixels = screen_pos.0 as f32;
            let ypixels = self.window_size.y - screen_pos.1 as f32;
            let to_metres = self.zoom / SCREEN_SCALE;
            Vector2::new(xpixels * to_metres, ypixels * to_metres)
        };
        ((self.pos + offset) / BLOCKS_SCALE).into()
    }
}
