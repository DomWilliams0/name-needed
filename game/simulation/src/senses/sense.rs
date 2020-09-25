use bitflags::bitflags;
use common::{InnerSpace, OrderedFloat, Rad, Vector2, Zero};
use std::hash::{Hash, Hasher};
use unit::world::WorldPoint;

#[derive(Debug, Clone)]
pub struct VisionCone {
    /// Length/height of the vision cone, i.e. distance
    pub length: f32,

    pub angle: Rad,

    /// Clockwise offset to add to angle e.g. for when 1 eye is missing
    pub angle_offset: Rad,
}

#[derive(Debug, Default, Clone)]
pub struct HearingSphere {
    /// Radius of the hearing sphere
    pub radius: f32,
}

bitflags! {
    pub struct Sense: u8 {
        const VISION = 0b01;
        const HEARING = 0b10;
    }
}

impl Default for VisionCone {
    fn default() -> Self {
        Self {
            length: 0.0,
            angle: Rad::zero(),
            angle_offset: Rad::zero(),
        }
    }
}

impl Hash for VisionCone {
    fn hash<H: Hasher>(&self, state: &mut H) {
        OrderedFloat(self.length).hash(state);
        OrderedFloat(self.angle.0).hash(state);
        OrderedFloat(self.angle_offset.0).hash(state);
    }
}

impl Hash for HearingSphere {
    fn hash<H: Hasher>(&self, state: &mut H) {
        OrderedFloat(self.radius).hash(state);
    }
}

impl VisionCone {
    #[inline]
    pub fn senses(
        &self,
        my_forward: &Vector2,
        my_pos: &WorldPoint,
        ur_pos: &WorldPoint,
        distance: f32,
    ) -> bool {
        // quick radius check
        if distance > self.length {
            return false;
        }

        let visible_range = self.angle_offset..(self.angle + self.angle_offset);

        // TODO this is really expensive
        let angle = my_forward.angle((*ur_pos - *my_pos).into()) + (self.angle / 2.0);
        visible_range.contains(&angle)
    }
}

impl HearingSphere {
    pub fn senses(&self, distance: f32) -> bool {
        distance <= self.radius
    }
}
