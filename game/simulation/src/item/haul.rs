use common::*;
use unit::world::WorldPoint;

use crate::ecs::*;

use crate::TransformComponent;

/// An item is being hauled
#[derive(Component, EcsComponent, Clone, Debug)]
#[name("hauling")]
#[storage(DenseVecStorage)]
pub struct HauledItemComponent {
    pub hauler: Entity,
    pub haul_type: HaulType,
    first_time: bool,
}

/// An item that is haulable
#[derive(Component, EcsComponent, Clone, Debug)]
#[name("haulable")]
#[storage(VecStorage)]
pub struct HaulableItemComponent {
    /// Number of extra hands more than 1 needed to haul this item
    pub extra_hands: u16,
}

#[derive(Copy, Clone, Debug)]
pub enum HaulType {
    /// 1 person carrying something by themselves
    CarryAlone,
    // TODO multiple people sharing a haul
    // TODO cart/wagon/vehicle
    // TODO carry vs drag
}

pub struct HaulSystem;

impl<'a> System<'a> for HaulSystem {
    type SystemData = (
        Read<'a, EntitiesRes>,
        WriteStorage<'a, HauledItemComponent>,
        WriteStorage<'a, TransformComponent>,
    );

    fn run(&mut self, (entities, mut hauled, mut transforms): Self::SystemData) {
        let mut new_positions = Vec::new();

        // collect new positions
        for (item, hauled) in (&entities, &hauled).join() {
            log_scope!(o!("system" => "haul", E(item)));

            // gross but we cant join on transform storage to be able to query the hauler
            let transform = transforms
                .get(hauled.hauler)
                .expect("hauler has no transform");
            assert!(transforms.get(item).is_some(), "hauled has no transform");

            let hauler_pos =
                position_hauled_item(hauled.haul_type, &transform.position, transform.forwards());
            trace!("moving hauled item to hauler-relative position"; "position" => %hauler_pos);
            new_positions.push(hauler_pos);
        }

        // assign new positions
        for ((hauled, transform), new_pos) in (&mut hauled, &mut transforms)
            .join()
            .zip(new_positions.drain(..))
        {
            // TODO this is awful and should be generalised to a part of the physics system e.g. relative positioned entity
            if std::mem::take(&mut hauled.first_time) {
                // first tick, move directly
                transform.reset_position(new_pos)
            } else {
                transform.last_position = transform.position;
                transform.position = new_pos;
            }
        }
    }
}

fn position_hauled_item(ty: HaulType, hauler_pos: &WorldPoint, hauler_fwd: Vector2) -> WorldPoint {
    match ty {
        HaulType::CarryAlone => {
            let mut pos = *hauler_pos;
            // TODO position at the correct arm(s) location
            // pos.2 -= 0.1; // "arm height"

            // a little bit in front
            pos += hauler_fwd * 0.4;

            pos
        }
    }
}

impl HauledItemComponent {
    pub fn new(hauler: Entity, ty: HaulType) -> Self {
        HauledItemComponent {
            hauler,
            haul_type: ty,
            first_time: true,
        }
    }
}
impl<V: Value> ComponentTemplate<V> for HaulableItemComponent {
    fn construct(values: &mut Map<V>) -> Result<Box<dyn ComponentTemplate<V>>, ComponentBuildError>
    where
        Self: Sized,
    {
        let extra_hands = values.get_int("extra_hands")?;
        Ok(Box::new(HaulableItemComponent { extra_hands }))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        builder.with(self.clone())
    }
}

register_component_template!("haulable", HaulableItemComponent);
