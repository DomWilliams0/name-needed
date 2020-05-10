use sfml::graphics::{
    CircleShape, Color, Drawable, PrimitiveType, RenderStates, RenderTarget, RenderWindow, Shape,
    Transformable, Vertex, VertexArray,
};

use crate::render::debug::DebugShape;
use color::ColorRgb;
use common::Vector2;
use simulation::{PhysicalComponent, Renderer, TransformComponent};
use unit::view::ViewPoint;

pub struct SfmlRenderer {
    frame_target: Option<FrameTarget>,
    debug_shapes: Vec<DebugShape>,
    debug_lines: VertexArray,
}

impl Default for SfmlRenderer {
    fn default() -> Self {
        Self {
            frame_target: None,
            debug_shapes: Vec::new(),
            debug_lines: VertexArray::new(PrimitiveType::Lines, 0),
        }
    }
}

pub struct FrameTarget {
    pub target: RenderWindow, // TODO generic?
}

impl Renderer for SfmlRenderer {
    type Target = FrameTarget;

    fn init(&mut self, target: Self::Target) {
        self.frame_target = Some(target);
    }

    fn sim_start(&mut self) {}

    fn sim_entity(&mut self, transform: &TransformComponent, physical: &PhysicalComponent) {
        // TODO instancing
        let frame = self.frame_target.as_mut().unwrap();
        let radius = physical.radius();
        let mut shape = CircleShape::new(radius * unit::world::SCALE, 40);

        let fill_color = Color::from(u32::from(physical.color()));
        let outline_color = fill_color + Color::rgb(40, 50, 60);
        shape.set_fill_color(fill_color);
        shape.set_outline_color(outline_color);
        shape.set_outline_thickness(-0.05);

        let pos = {
            let mut pos = transform.position;
            // position is centre of entity but sfml uses bottom corner
            pos.0 -= radius;
            pos.1 -= radius;
            pos
        };
        let ViewPoint(x, y, _) = pos.into();
        shape.set_position((x, y)); // TODO use z with manual opengl

        shape.draw(&mut frame.target, RenderStates::default());
    }

    fn sim_finish(&mut self) {}

    fn debug_start(&mut self) {
        self.debug_shapes.clear();
    }

    fn debug_add_line(&mut self, from: ViewPoint, to: ViewPoint, color: ColorRgb) {
        self.debug_shapes.push(DebugShape::Line {
            points: [from, to],
            color,
        });
    }

    fn debug_add_tri(&mut self, _points: [ViewPoint; 3], _color: ColorRgb) {
        unimplemented!()
    }

    fn debug_finish(&mut self) {
        self.debug_lines.resize(self.debug_shapes.len() * 2);

        let mut idx = 0;
        for shape in self.debug_shapes.drain(..) {
            match shape {
                DebugShape::Line { points, color } => unsafe {
                    let color = Color::from(Into::<u32>::into(color));
                    for p in &points {
                        let pos = sfml::system::Vector2::new(p.0, p.1); // TODO z ignored
                        self.debug_lines
                            .set_vertex_unchecked(idx, &Vertex::with_pos_color(pos, color));
                        idx += 1;
                    }
                },
                _ => unimplemented!(),
            };
        }

        let frame = self.frame_target.as_mut().unwrap();
        frame.target.draw(&self.debug_lines);
    }

    fn deinit(&mut self) -> Self::Target {
        self.frame_target.take().unwrap()
    }
}
