#[derive(Copy, Clone)]
#[repr(u8)]
pub enum CameraDirection {
    Up,
    Left,
    Right,
    Down,
}

impl CameraDirection {
    pub const fn values() -> [CameraDirection; 4] {
        [
            CameraDirection::Up,
            CameraDirection::Left,
            CameraDirection::Right,
            CameraDirection::Down,
        ]
    }

    pub fn delta(self) -> (i8, i8) {
        match self {
            CameraDirection::Up => (0, 1),
            CameraDirection::Left => (-1, 0),
            CameraDirection::Right => (1, 0),
            CameraDirection::Down => (0, -1),
        }
    }
}

#[derive(Copy, Clone)]
pub enum Key {
    Exit,
    Restart,
    SliceUp,
    SliceDown,
    ToggleWireframe,
    Camera(CameraDirection),
}

#[derive(Copy, Clone)]
pub enum KeyEvent {
    Down(Key),
    Up(Key),
}

impl KeyEvent {
    pub fn is_down(self) -> bool {
        match self {
            KeyEvent::Down(_) => true,
            KeyEvent::Up(_) => false,
        }
    }

    pub fn key(self) -> Key {
        match self {
            KeyEvent::Down(k) => k,
            KeyEvent::Up(k) => k,
        }
    }

    pub fn parse_camera_event(self) -> Option<(CameraDirection, bool)> {
        match self.key() {
            Key::Camera(dir) => Some((dir, self.is_down())),
            _ => None,
        }
    }
}

pub enum EventHandled {
    Handled,
    NotHandled,
}
