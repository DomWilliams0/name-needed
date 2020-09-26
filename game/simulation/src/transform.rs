use common::*;
use unit::world::{WorldPoint, WorldPosition};

use crate::ecs::*;
use crate::physics::Bounds;
use crate::PhysicalShape;
use common::cgmath::Rotation;
use specs::{Builder, EntityBuilder};

/// Position and rotation component
#[derive(Debug, Clone, Component, EcsComponent)]
#[storage(VecStorage)]
#[name("transform")]
pub struct TransformComponent {
    /// Position in world, center of entity in x/y and bottom of entity in z
    pub position: WorldPoint,

    /// Height in z axis
    pub height: f32,

    /// Physical shape in x,y axes
    pub shape: PhysicalShape,

    /// Used for render interpolation
    pub last_position: WorldPoint,

    /// Last known-accessible block position
    pub accessible_position: Option<WorldPosition>,

    /// 1d rotation around z axis
    pub rotation: Basis2,

    /// Current velocity
    pub velocity: Vector2,

    /// Number of blocks fallen in current fall
    pub fallen: u32,
}

impl TransformComponent {
    pub fn new(position: WorldPoint, shape: PhysicalShape, height: f32) -> Self {
        Self {
            position,
            shape,
            height,
            rotation: Basis2::from_angle(rad(0.0)),
            last_position: position,
            accessible_position: None,
            velocity: Zero::zero(),
            fallen: 0,
        }
    }

    pub fn reset_position(&mut self, new_position: WorldPoint) {
        self.position = new_position;
        self.last_position = new_position;
    }

    pub const fn slice(&self) -> i32 {
        // cant use position.slice() because not const
        self.position.2 as i32
    }

    pub const fn x(&self) -> f32 {
        self.position.0
    }
    pub const fn y(&self) -> f32 {
        self.position.1
    }
    pub const fn z(&self) -> f32 {
        self.position.2
    }

    pub fn bounding_radius(&self) -> f32 {
        self.shape.radius()
    }

    pub fn bounds(&self) -> Bounds {
        // allow tiny overlap
        const MARGIN: f32 = 0.8;
        let radius = self.shape.radius() * MARGIN;
        Bounds::from_radius(self.position, radius, radius)
    }

    pub fn feelers_bounds(&self) -> Bounds {
        let bounding_radius = self.shape.radius();
        let feelers = self.velocity + (self.velocity.normalize() * bounding_radius);
        let centre = self.position + feelers;

        const EXTRA: f32 = 1.25;
        let length = bounding_radius * EXTRA;
        let width = 0.1; // will be floor'd/ceil'd to 0 and 1

        let (x, y) = if feelers.x > feelers.y {
            (width, length)
        } else {
            (length, width)
        };

        Bounds::from_radius(centre, x, y)
    }

    pub fn accessible_position(&self) -> WorldPosition {
        if let Some(pos) = self.accessible_position {
            // known accessible
            pos
        } else {
            // fallback to exact position
            self.position.floor()
        }
    }

    pub fn forwards(&self) -> Vector2 {
        self.rotation.rotate_vector(AXIS_FWD_2)
    }
}

impl<V: Value> ComponentTemplate<V> for TransformComponent {
    fn construct(values: &mut Map<V>) -> Result<Box<dyn ComponentTemplate<V>>, ComponentBuildError>
    where
        Self: Sized,
    {
        let shape: PhysicalShape = values.get("shape")?.into_type()?;
        let height = values.get_float("height")?;

        // position will be customized afterwards
        Ok(Box::new(Self::new(WorldPoint::default(), shape, height)))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        builder.with(self.clone())
    }
}

register_component_template!("transform", TransformComponent);
