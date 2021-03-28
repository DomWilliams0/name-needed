use std::collections::HashSet;

use unit::world::WorldPoint;
use world::SliceRange;

use crate::activity::ActivityComponent;
use crate::ecs::{EcsWorld, Entity};
use crate::input::{SelectedEntity, SelectedTiles};
use crate::item::{ContainerComponent, EdibleItemComponent, ItemCondition};
use crate::needs::HungerComponent;
use crate::path::FollowPathComponent;
use crate::simulation::AssociatedBlockData;
use crate::society::{PlayerSociety, SocietyComponent};
use crate::{
    ComponentWorld, ConditionComponent, InventoryComponent, NameComponent, PhysicalComponent,
    Societies, SocietyHandle, ThreadedWorldLoader, TransformComponent,
};
use world::loader::BlockDetails;

/// Dump of game info for the UI to render
/// TODO this can probably just hold the world and have some helper functions
pub struct UiBlackboard<'a> {
    pub selected_entity: Option<SelectedEntityDetails<'a>>,
    pub selected_tiles: &'a SelectedTiles,
    pub selected_block_details: Option<BlockDetails>,
    pub selected_container: Option<(Entity, &'a str, &'a ContainerComponent)>,
    pub player_society: PlayerSociety,
    pub societies: &'a Societies,
    pub enabled_debug_renderers: &'a HashSet<&'static str>,
    pub world: &'a EcsWorld,

    /// Populated by backend engine
    pub world_view: Option<SliceRange>,
}

pub struct SelectedEntityDetails<'a> {
    pub entity: Entity,
    pub name: Option<&'a NameComponent>,
    pub transform: &'a TransformComponent,
    pub physical: Option<&'a PhysicalComponent>,
    pub details: EntityDetails<'a>,
}

pub enum EntityDetails<'a> {
    Living {
        activity: Option<&'a ActivityComponent>,
        hunger: Option<&'a HungerComponent>,
        path_target: Option<WorldPoint>,
        society: Option<SocietyHandle>,
        inventory: Option<&'a InventoryComponent>,
    },
    Item {
        condition: &'a ItemCondition,
        edible: Option<&'a EdibleItemComponent>,
    },
}

impl<'a> UiBlackboard<'a> {
    pub fn fetch(
        world: &'a EcsWorld,
        world_loader: &ThreadedWorldLoader,
        debug_renderers: &'a HashSet<&'static str>,
    ) -> Self {
        let selected_entity = world.resource_mut::<SelectedEntity>().get(world).map(|e| {
            let transform = world.component::<TransformComponent>(e).unwrap(); // definitely ok because selected.get() just verified
            let name = world.component::<NameComponent>(e).ok();
            let details = match world.component::<ConditionComponent>(e) {
                Ok(condition) => EntityDetails::Item {
                    condition: &condition.0,
                    edible: world.component(e).ok(),
                },
                _ => EntityDetails::Living {
                    activity: world.component::<ActivityComponent>(e).ok(),
                    hunger: world.component(e).ok(),
                    inventory: world.component(e).ok(),
                    path_target: world
                        .component::<FollowPathComponent>(e)
                        .ok()
                        .and_then(|follow| follow.target()),
                    society: world
                        .component::<SocietyComponent>(e)
                        .map(|s| s.handle)
                        .ok(),
                },
            };

            SelectedEntityDetails {
                entity: e,
                name,
                transform,
                physical: world.component(e).ok(),
                details,
            }
        });

        let selected_tiles = world.resource::<SelectedTiles>();
        let selected_tile_biome = selected_tiles
            .single_tile()
            .and_then(|pos| world_loader.query_block(pos));

        let selected_container = selected_tiles.single_tile().and_then(|pos| {
            let world_ref = world.voxel_world();
            let voxel_world = world_ref.borrow();
            if let Some(AssociatedBlockData::Container(e)) = voxel_world.associated_block_data(pos)
            {
                let name = world
                    .component::<NameComponent>(*e)
                    .ok()
                    .map(|c| c.0.as_str());
                let container = world.component::<ContainerComponent>(*e).ok();
                name.zip(container)
                    .map(|(name, container)| (*e, name, container))
            } else {
                None
            }
        });
        let player_society = world.resource::<PlayerSociety>().clone();
        let societies = world.resource::<Societies>();

        Self {
            selected_entity,
            selected_tiles,
            selected_block_details: selected_tile_biome,
            selected_container,
            player_society,
            societies,
            enabled_debug_renderers: debug_renderers,
            world_view: None,
            world,
        }
    }
}
