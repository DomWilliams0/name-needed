use std::borrow::Cow;

use common::Rng;
use unit::world::{WorldPoint, WorldPosition};
use world_types::{EntityDescription, PlantDescription};

use crate::region::subfeature::{Rasterizer, Subfeature, SubfeatureEntity};

#[derive(Debug)]
pub struct Fauna {
    pub species: &'static str,
}

impl Subfeature for Fauna {
    fn rasterize(
        &mut self,
        root: WorldPosition,
        rasterizer: &mut Rasterizer,
    ) -> Option<SubfeatureEntity> {
        // no blocks

        let position = {
            let pos = root.centred();
            let rng = rasterizer.rng();
            const VARIATION: f32 = 0.2;
            WorldPoint::new_unchecked(
                pos.x() + rng.gen_range(-VARIATION, VARIATION),
                pos.y() + rng.gen_range(-VARIATION, VARIATION),
                pos.z() + 1.0,
            )
        };
        Some(SubfeatureEntity(EntityDescription {
            position,
            desc: PlantDescription {
                species: Cow::Borrowed(self.species),
            },
        }))
    }
}
