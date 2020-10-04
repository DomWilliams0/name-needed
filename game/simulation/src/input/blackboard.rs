use crate::activity::ActivityComponent;
use crate::ecs::Entity;
use crate::input::{SelectedEntity, SelectedTiles};
use crate::item::{BaseItemComponent, EdibleItemComponent};
use crate::needs::HungerComponent;
use crate::path::FollowPathComponent;
use crate::society::{PlayerSociety, SocietyComponent};
use crate::{
    ComponentWorld, Inventory2Component, PhysicalComponent, Societies, SocietyHandle,
    TransformComponent,
};
use std::collections::HashSet;
use unit::world::WorldPoint;
use world::SliceRange;

/// Dump of game info for the UI to render
pub struct UiBlackboard<'a> {
    pub selected_entity: Option<SelectedEntityDetails<'a>>,
    pub selected_tiles: &'a SelectedTiles,
    pub player_society: PlayerSociety,
    pub societies: &'a Societies,
    pub enabled_debug_renderers: &'a HashSet<&'static str>,

    /// Populated by backend engine
    pub world_view: Option<SliceRange>,
}

pub struct SelectedEntityDetails<'a> {
    pub entity: Entity,
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
        inventory: Option<&'a Inventory2Component>,
    },
    Item {
        item: &'a BaseItemComponent,
        edible: Option<&'a EdibleItemComponent>,
    },
}

impl<'a> UiBlackboard<'a> {
    pub fn fetch<W: ComponentWorld>(
        world: &'a W,
        debug_renderers: &'a HashSet<&'static str>,
    ) -> Self {
        let selected_entity = world.resource_mut::<SelectedEntity>().get(world).map(|e| {
            let transform = world.component::<TransformComponent>(e).unwrap(); // definitely ok because selected.get() just verified
            let details = match world.component::<BaseItemComponent>(e) {
                Ok(item) => EntityDetails::Item {
                    item,
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
                transform,
                physical: world.component(e).ok(),
                details,
            }
        });

        let selected_tiles = world.resource::<SelectedTiles>();
        let player_society = world.resource::<PlayerSociety>().clone();
        let societies = world.resource::<Societies>();

        Self {
            selected_entity,
            selected_tiles,
            player_society,
            societies,
            enabled_debug_renderers: debug_renderers,
            world_view: None,
        }
    }
}
