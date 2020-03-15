#[derive(Copy, Clone)]
pub enum Key {
    Exit,
    Restart,
    SliceUp,
    SliceDown,
    ToggleWireframe,
    CameraForward,
    CameraLeft,
    CameraBack,
    CameraRight,
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

    pub fn is_camera_key(self) -> bool {
        match self.key() {
            Key::CameraForward | Key::CameraLeft | Key::CameraBack | Key::CameraRight => true,
            _ => false,
        }
    }
}
