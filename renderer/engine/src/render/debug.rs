use color::Color;
use unit::space::view::ViewPoint;

pub enum DebugShape {
    Line {
        points: [ViewPoint; 2],
        color: Color,
    },
    #[allow(dead_code)]
    Tri {
        points: [ViewPoint; 3],
        color: Color,
    },
}

impl DebugShape {
    pub fn color(&self) -> Color {
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
