use color::ColorRgb;
use unit::space::view::ViewPoint;

pub enum DebugShape {
    Line {
        points: [ViewPoint; 2],
        color: ColorRgb,
    },
    #[allow(dead_code)]
    Tri {
        points: [ViewPoint; 3],
        color: ColorRgb,
    },
}

impl DebugShape {
    pub fn color(&self) -> ColorRgb {
        *match self {
            DebugShape::Line { color, .. } => color,
            DebugShape::Tri { color, .. } => color,
        }
    }

    pub fn points(&self) -> &[ViewPoint] {
        match self {
            DebugShape::Line { points, .. } => points,
            DebugShape::Tri { points, .. } => points,
        }
    }
}
