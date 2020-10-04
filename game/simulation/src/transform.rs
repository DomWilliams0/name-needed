use common::*;

use unit::world::{WorldPoint, WorldPosition};

use crate::ecs::*;
use crate::physics::{Bounds, PhysicsComponent};

use common::cgmath::Rotation;
use serde::Deserialize;
use unit::length::{Length, Length3};
use unit::volume::Volume;

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

    /// Bounding dimensions around the centre
    ///
    /// TODO clarify in uses and definition that this isn't really half dims! ridiculous
    pub half_dimensions: Length3,
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

    pub fn bounds(&self, bounding_radius: f32) -> Bounds {
        // allow tiny overlap
        const MARGIN: f32 = 0.8;
        let radius = bounding_radius * MARGIN;
        Bounds::from_radius(self.position, radius, radius)
    }

    pub fn feelers_bounds(&self, bounding_radius: f32) -> Bounds {
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

impl PhysicalComponent {
    pub fn new(volume: Volume, half_dimensions: Length3) -> Self {
        PhysicalComponent {
            volume,
            half_dimensions,
        }
    }

    /// Max x/y dimension
    pub fn bounding_radius(&self) -> Length {
        self.half_dimensions.x().max(self.half_dimensions.y())
    }
}

#[derive(Deserialize)]
struct HalfDims {
    x: u16,
    y: u16,
    z: u16,
}

#[derive(Debug)]
pub struct PhysicalComponentTemplate {
    half_dims: Length3,
    volume: Volume,
}
impl<V: Value> ComponentTemplate<V> for PhysicalComponentTemplate {
    fn construct(values: &mut Map<V>) -> Result<Box<dyn ComponentTemplate<V>>, ComponentBuildError>
    where
        Self: Sized,
    {
        let volume = values.get_int("volume")?;
        let half_dims: HalfDims = values.get("half_dims")?.into_type()?;
        Ok(Box::new(Self {
            volume: Volume::new(volume),
            half_dims: Length3::new(half_dims.x, half_dims.y, half_dims.z),
        }))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        // position will be customized afterwards
        let pos = WorldPoint::default();

        builder
            .with(TransformComponent::new(pos))
            .with(PhysicsComponent::default())
            .with(PhysicalComponent::new(self.volume, self.half_dims))
    }
}

register_component_template!("physical", PhysicalComponentTemplate);
