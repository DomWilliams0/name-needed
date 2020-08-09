use crate::ecs::Entity;
use crate::ComponentWorld;
use unit::world::WorldPoint;

#[derive(Copy, Clone)]
enum PickupItemsState {
    Uninit,
    GoingTo(Entity, WorldPoint),
    PickingUp(Entity),
}
struct PickupItemsActivity {
    items: Vec<(Entity, WorldPoint)>,
    state: PickupItemsState,
}

impl PickupItemsActivity {
    fn with_items(items: Vec<(Entity, WorldPoint)>) -> Self {
        Self {
            items,
            state: PickupItemsState::Uninit,
        }
    }

    fn best_item<W: ComponentWorld>(&mut self, world: &W) -> Option<(usize, (Entity, WorldPoint))> {
        // TODO
        None
    }
}
