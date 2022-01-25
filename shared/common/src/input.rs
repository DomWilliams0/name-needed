#[derive(Copy, Clone, Debug)]
pub enum CameraDirection {
    Up,
    Left,
    Right,
    Down,
}

#[derive(Copy, Clone)]
pub enum ChangeSliceDirection {
    Up,
    Down,
}

#[derive(Copy, Clone, Debug)]
pub enum RendererKey {
    SliceUp,
    SliceDown,
    Camera(CameraDirection),
}

#[derive(Copy, Clone, Debug)]
pub enum EngineKey {
    Exit,
    Restart,
}

#[derive(Copy, Clone, Debug)]
pub enum GameKey {
    CancelSelection,
}

pub enum KeyAction {
    Renderer(RendererKey),
    Engine(EngineKey),
    Game(GameKey),
}

impl CameraDirection {
    pub const fn values() -> [CameraDirection; 4] {
        use CameraDirection::*;
        [Up, Left, Right, Down]
    }

    pub fn delta(self) -> (i8, i8) {
        use CameraDirection::*;
        match self {
            Up => (0, 1),
            Left => (-1, 0),
            Right => (1, 0),
            Down => (0, -1),
        }
    }
}
