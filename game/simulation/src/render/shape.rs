use color::Color;
use serde::Deserialize;

#[derive(Debug, Copy, Clone, Deserialize)]
pub enum Shape2d {
    /// Ordinal 0
    Circle,
    /// Ordinal 1
    Rect,
}

impl Shape2d {
    /// For simple sorting
    pub fn ord(self) -> usize {
        match self {
            Shape2d::Circle { .. } => 0,
            Shape2d::Rect { .. } => 1,
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum RenderHexColor {
    #[serde(with = "hex_serde")]
    Hex([u8; 3]),
    RgbInt {
        r: u8,
        g: u8,
        b: u8,
    },
    RgbFloat {
        r: f32,
        g: f32,
        b: f32,
    },
    Hsl {
        h: f32,
        s: f32,
        l: f32,
    },
}

impl From<RenderHexColor> for Color {
    fn from(c: RenderHexColor) -> Self {
        match c {
            RenderHexColor::Hex(i) => {
                let bytes = [i[0], i[1], i[2], 255];
                let int = u32::from_be_bytes(bytes);
                int.into()
            }
            RenderHexColor::RgbInt { r, g, b } => Self::rgb(r, g, b),
            RenderHexColor::RgbFloat { r, g, b } => Self::rgb_f(r, g, b),
            RenderHexColor::Hsl { h, s, l } => Self::hsl(h, s, l),
        }
    }
}
