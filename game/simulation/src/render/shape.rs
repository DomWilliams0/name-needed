use color::ColorRgb;
use serde::Deserialize;

// TODO physical shape wastes so much space
#[derive(Debug, Copy, Clone, Deserialize)]
#[serde(untagged)]
pub enum Shape2d {
    /// Ordinal 0
    Circle { radius: f32 },
    /// Ordinal 1
    Rectangle { rx: f32, ry: f32 },
}

impl Shape2d {
    /// For simple sorting
    pub fn ord(self) -> usize {
        match self {
            Shape2d::Circle { .. } => 0,
            Shape2d::Rectangle { .. } => 1,
        }
    }

    pub fn circle(radius: f32) -> Self {
        Shape2d::Circle { radius }
    }

    pub fn rect(rx: f32, ry: f32) -> Self {
        Shape2d::Rectangle { rx, ry }
    }

    pub fn square(r: f32) -> Self {
        Shape2d::Rectangle { rx: r, ry: r }
    }

    pub fn radius(&self) -> f32 {
        match self {
            Shape2d::Circle { radius } => *radius,
            Shape2d::Rectangle { rx, ry } => rx.max(*ry),
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

impl From<RenderHexColor> for ColorRgb {
    fn from(c: RenderHexColor) -> Self {
        match c {
            RenderHexColor::Hex(i) => {
                let bytes = [i[0], i[1], i[2], 255];
                let int = u32::from_be_bytes(bytes);
                int.into()
            }
            RenderHexColor::RgbInt { r, g, b } => Self::new(r, g, b),
            RenderHexColor::RgbFloat { r, g, b } => Self::new_float(r, g, b),
            RenderHexColor::Hsl { h, s, l } => Self::new_hsl(h, s, l),
        }
    }
}
