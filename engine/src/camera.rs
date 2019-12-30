use enum_map::{Enum, EnumMap};
use num_traits::clamp;
use sdl2::keyboard::Keycode;

use common::*;

const MOVE_SPEED: f32 = 0.2;

#[derive(Enum)]
enum Direction {
    Forward,
    Backward,
    Left,
    Right,
}

impl Direction {
    fn from_key(key: Keycode) -> Option<Self> {
        match key {
            Keycode::W => Some(Direction::Forward),
            Keycode::A => Some(Direction::Left),
            Keycode::S => Some(Direction::Backward),
            Keycode::D => Some(Direction::Right),
            _ => None,
        }
    }
}

/// Only for debugging
pub struct FreeRangeCamera {
    pos: Point3,
    dir: Vector3,
    up: Vector3,

    pitch: Deg<f32>,
    yaw: Deg<f32>,

    key_state: EnumMap<Direction, bool>,
    lookaround: bool,
}

impl FreeRangeCamera {
    pub fn new(pos: Point3) -> Self {
        let mut cam = Self {
            pos,
            dir: -Vector3::unit_z(),
            up: Vector3::unit_y(),
            pitch: Deg(0.0),
            yaw: Deg(0.0),
            key_state: EnumMap::new(),
            lookaround: false,
        };
        cam.update_yaw_n_pitch();
        cam
    }

    fn update_yaw_n_pitch(&mut self) {
        self.pitch = Angle::asin(self.dir.y);
        self.yaw = Angle::atan2(self.dir.z, self.dir.x);
    }

    pub fn handle_click(&mut self, down: bool) {
        self.lookaround = down
    }

    pub fn handle_cursor(&mut self, dx: i32, dy: i32) {
        if !self.lookaround {
            return;
        }

        let turn_speed = config::get().display.camera_turn_multiplier;
        let dx = (dx as f32) * turn_speed;
        let dy = (dy as f32) * turn_speed;

        self.yaw += Deg(dx);
        self.pitch = clamp(self.pitch - Deg(dy), Deg(-89.0), Deg(89.0));

        self.dir.x = Deg::cos(self.yaw) * Deg::cos(self.pitch);
        self.dir.y = Deg::sin(self.pitch);
        self.dir.z = Deg::sin(self.yaw) * Deg::cos(self.pitch);
    }

    pub fn handle_key(&mut self, key: Keycode, pressed: bool) {
        if let Some(dir) = Direction::from_key(key) {
            self.key_state[dir] = pressed;
        }
    }

    pub fn world_to_view(&mut self) -> Matrix4<f32> {
        for dir in self.key_state
            .iter()
            .filter_map(|(d, on)| if *on { Some(d) } else { None })
        {
            let diff = match dir {
                Direction::Forward => self.dir * MOVE_SPEED,
                Direction::Backward => self.dir * -MOVE_SPEED,
                Direction::Left => self.dir.cross(self.up).normalize_to(-MOVE_SPEED),
                Direction::Right => self.dir.cross(self.up).normalize_to(MOVE_SPEED),
            };
            self.pos += diff;
        }
        Matrix4::look_at(self.pos, self.pos + self.dir, self.up)
    }
}
