#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum BlockType {
    Air,
    Dirt,
    Grass,
    Stone,
}

impl BlockType {
    pub fn color_as_u8(self) -> (u8, u8, u8) {
        match self {
            BlockType::Air => (0, 0, 0),
            BlockType::Dirt => (192, 57, 43),
            BlockType::Grass => (40, 102, 25),
            BlockType::Stone => (106, 106, 117),
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

    pub fn solid(self) -> bool {
        self != BlockType::Air
    }
}

impl Default for BlockType {
    fn default() -> Self {
        BlockType::Air
    }
}
