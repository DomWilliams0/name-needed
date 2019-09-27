#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum BlockType {
    Air,
    Dirt,
    Hi,
}

impl BlockType {
    pub fn color_as_u8(self) -> (u8, u8, u8) {
        match self {
            BlockType::Air => (0, 0, 0),
            BlockType::Dirt => (192, 57, 43),
            BlockType::Hi => (20, 100, 20),
        }
    }

    pub fn color_as_f32(self) -> (f32, f32, f32) {
        let (r, g, b) = self.color_as_u8();
        (
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
        )
    }
}

impl Default for BlockType {
    fn default() -> Self {
        BlockType::Air
    }
}
