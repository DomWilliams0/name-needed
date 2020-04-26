use sfml::graphics::{
    CircleShape, Color, Drawable, RenderStates, RenderWindow, Shape, Transformable,
};

use simulation::{PhysicalComponent, Renderer, TransformComponent};
use unit::view::ViewPoint;

pub struct SfmlRenderer {
    frame_target: Option<FrameTarget>,
}

impl Default for SfmlRenderer {
    fn default() -> Self {
        Self { frame_target: None }
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

        let mut shape = CircleShape::new(physical.radius() * unit::scale::HUMAN * 0.5, 20);

        let fill_color = Color::from(u32::from(physical.color()));
        let outline_color = fill_color + Color::rgb(40, 50, 60);
        shape.set_fill_color(fill_color);
        shape.set_outline_color(outline_color);
        shape.set_outline_thickness(0.05);

        let ViewPoint(x, y, _) = transform.position.into();
        shape.set_position((x, y)); // TODO use z with manual opengl

        shape.draw(&mut frame.target, RenderStates::default());
    }

    fn sim_finish(&mut self) {}

    fn deinit(&mut self) -> Self::Target {
        self.frame_target.take().unwrap()
    }
}
