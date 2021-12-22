use common::*;
use unit::world::WorldPoint;

use crate::ecs::*;

use crate::TransformComponent;

/// An item is being hauled
#[derive(Component, EcsComponent, Clone, Debug)]
#[name("hauling")]
#[storage(DenseVecStorage)]
#[clone(disallow)]
pub struct HauledItemComponent {
    pub hauler: Entity,
    pub haul_type: HaulType,
    first_time: bool,

    /// What to do if the haul is interrupted
    pub interrupt_behaviour: EndHaulBehaviour,
}

#[derive(Clone, Debug)]
pub enum EndHaulBehaviour {
    /// Drop the item on the floor
    Drop,

    /// Stop hauling but keep the thing in hands
    KeepEquipped,
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
            let item = Entity::from(item);
            log_scope!(o!("system" => "haul", item));

            // gross but we cant join on transform storage to be able to query the hauler
            let transform = hauled
                .hauler
                .get(&transforms)
                .expect("hauler has no transform");
            assert!(item.get(&transforms).is_some(), "hauled has no transform");

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
            // TODO this is awful and should be generalised with a separate transform child/parent component
            // TODO also update rotation when hauling
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
            interrupt_behaviour: EndHaulBehaviour::default(),
        }
    }
}

impl Default for EndHaulBehaviour {
    fn default() -> Self {
        Self::Drop
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

    crate::as_any!();
}

register_component_template!("haulable", HaulableItemComponent);
