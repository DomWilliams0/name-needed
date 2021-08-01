use common::*;

use unit::world::{WorldPoint, WorldPosition};

use crate::ecs::*;
use crate::physics::{Bounds, PhysicsComponent};

use common::cgmath::Rotation;
use serde::Deserialize;
use unit::space::length::{Length, Length3};
use unit::space::volume::Volume;

/// Position and rotation component
#[derive(Debug, Clone, Component, EcsComponent)]
#[storage(VecStorage)]
#[name("transform")]
pub struct TransformComponent {
    /// Position in world, center of entity in x/y and bottom of entity in z
    pub position: WorldPoint,

    /// Used for render interpolation
    pub last_position: WorldPoint,

    /// Last known-accessible block position
    pub accessible_position: Option<WorldPosition>,

    /// 1d rotation around z axis
    pub rotation: Basis2,

    /// Current velocity
    pub velocity: Vector2,
}

/// Physical attributes of an entity
// TODO use newtype units for ingame non-SI units
#[derive(Component, EcsComponent, Clone, Debug)]
#[storage(VecStorage)]
#[name("physical")]
pub struct PhysicalComponent {
    pub volume: Volume,

    /// Bounding dimensions, not positioned around centre
    pub size: Length3,
}

impl TransformComponent {
    pub fn new(position: WorldPoint) -> Self {
        Self {
            position,
            rotation: Basis2::from_angle(rad(0.0)),
            last_position: position,
            accessible_position: None,
            velocity: Zero::zero(),
        }
    }

    pub fn reset_position(&mut self, new_position: WorldPoint) {
        self.position = new_position;
        self.last_position = new_position;
    }

    pub fn slice(&self) -> i32 {
        // cant use position.slice() because not const
        self.position.z() as i32
    }

    pub fn x(&self) -> f32 {
        self.position.x()
    }
    pub fn y(&self) -> f32 {
        self.position.y()
    }
    pub fn z(&self) -> f32 {
        self.position.z()
    }

    pub fn bounds(&self, bounding_radius: f32) -> Bounds {
        // allow tiny overlap
        const MARGIN: f32 = 0.8;
        let radius = bounding_radius * MARGIN;
        Bounds::from_radius(self.position, radius, radius)
    }

    pub fn feelers_bounds(&self, bounding_radius: f32) -> Bounds {
        let feelers = if self.velocity.is_zero() {
            self.velocity // avoid normalizing 0 to get NaN
        } else {
            self.velocity + (self.velocity.normalize() * bounding_radius)
        };
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

    pub fn rotate_to(&mut self, angle: Rad) {
        self.rotation = Basis2::from_angle(angle);
    }
}

impl PhysicalComponent {
    pub fn new(volume: Volume, size: Length3) -> Self {
        PhysicalComponent { volume, size }
    }

    pub fn max_dimension(&self) -> Length {
        self.size.x().max(self.size.y())
    }
}

#[derive(Deserialize)]
struct Size {
    x: u16,
    y: u16,
    z: u16,
}

#[derive(Debug)]
pub struct PhysicalComponentTemplate {
    size: Length3,
    volume: Volume,
}

impl<V: Value> ComponentTemplate<V> for PhysicalComponentTemplate {
    fn construct(values: &mut Map<V>) -> Result<Box<dyn ComponentTemplate<V>>, ComponentBuildError>
    where
        Self: Sized,
    {
        let volume = values.get_int("volume")?;
        let size: Size = values.get("size")?.into_type()?;
        Ok(Box::new(Self {
            volume: Volume::new(volume),
            size: Length3::new(size.x, size.y, size.z),
        }))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        // position will be customized afterwards
        let pos = WorldPoint::default();

        builder
            .with(TransformComponent::new(pos))
            .with(PhysicsComponent::default())
            .with(PhysicalComponent::new(self.volume, self.size))
    }
}

register_component_template!("physical", PhysicalComponentTemplate);
