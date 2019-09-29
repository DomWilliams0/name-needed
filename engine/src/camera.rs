use cgmath::prelude::*;
use cgmath::{Deg, Matrix4, Point3, Vector3};
use enum_map::{Enum, EnumMap};
use sdl2::keyboard::Keycode;

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
    pos: Point3<f32>,
    dir: Vector3<f32>,
    up: Vector3<f32>,

    pitch: Deg<f32>,
    yaw: Deg<f32>,

    key_state: EnumMap<Direction, bool>,
}

impl FreeRangeCamera {
    pub fn new(pos: Point3<f32>) -> Self {
        let mut cam = Self {
            pos,
            dir: -Vector3::unit_z(),
            up: Vector3::unit_y(),
            pitch: Deg(0.0),
            yaw: Deg(0.0),
            key_state: EnumMap::new(),
        };
        cam.update_yaw_n_pitch();
        cam
    }

    fn update_yaw_n_pitch(&mut self) {
        self.pitch = Angle::asin(self.dir.y);
        self.yaw = Angle::atan2(self.dir.z, self.dir.x);
    }

    pub fn handle_cursor(&mut self, dx: i32, dy: i32) {
        self.yaw += Deg(dx as f32 * 0.3);
        self.pitch = {
            let Deg(mut pitch) = self.pitch;
            pitch -= dy as f32;
            Deg(if pitch < -89.0 {
                -89.0
            } else if pitch > 89.0 {
                89.0
            } else {
                pitch
            })
        };

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

    //    pub fn pos(&self) -> Point3<f32> {
    //        self.pos
    //    }
}
