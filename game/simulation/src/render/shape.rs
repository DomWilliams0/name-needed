#[derive(Debug, Copy, Clone)]
pub enum PhysicalShape {
    /// Ordinal 0
    Circle { radius: f32 },
    /// Ordinal 1
    Rectangle { rx: f32, ry: f32 },
}

impl PhysicalShape {
    /// For simple sorting
    pub fn ord(self) -> usize {
        match self {
            PhysicalShape::Circle { .. } => 0,
            PhysicalShape::Rectangle { .. } => 1,
        }
    }

    pub fn circle(radius: f32) -> Self {
        PhysicalShape::Circle { radius }
    }

    pub fn rect(rx: f32, ry: f32) -> Self {
        PhysicalShape::Rectangle { rx, ry }
    }

    pub fn square(r: f32) -> Self {
        PhysicalShape::Rectangle { rx: r, ry: r }
    }

    pub fn radius(&self) -> f32 {
        match self {
            PhysicalShape::Circle { radius } => *radius,
            PhysicalShape::Rectangle { rx, ry } => rx.max(*ry),
        }
    }
}
