use crate::ai::ActivityComponent;
use crate::ecs::Entity;
use crate::input::{SelectedEntity, SelectedTiles};
use crate::item::{BaseItemComponent, EdibleItemComponent};
use crate::needs::HungerComponent;
use crate::path::FollowPathComponent;
use crate::society::{PlayerSociety, SocietyComponent};
use crate::{ComponentWorld, Societies, SocietyHandle, TransformComponent};
use std::collections::HashSet;
use unit::world::WorldPoint;
use world::SliceRange;

/// Dump of game info for the UI to render
pub struct UiBlackboard<'a> {
    pub selected_entity: Option<SelectedEntityDetails>,
    pub selected_tiles: SelectedTiles,
    pub player_society: PlayerSociety,
    pub societies: &'a Societies,
    pub enabled_debug_renderers: &'a HashSet<&'static str>,

    /// Populated by backend engine
    pub world_view: Option<SliceRange>,
}

pub struct SelectedEntityDetails {
    pub entity: Entity,
    pub transform: TransformComponent,
    pub details: EntityDetails,
}

pub enum EntityDetails {
    Living {
        activity: Option<String>,
        hunger: Option<HungerComponent>,
        path_target: Option<WorldPoint>,
        society: Option<SocietyHandle>,
    },
    Item {
        item: BaseItemComponent,
        edible: Option<EdibleItemComponent>,
    },
}

impl<'a> UiBlackboard<'a> {
    // TODO use ui allocation arena here too
    pub fn fetch<W: ComponentWorld>(
        world: &'a W,
        debug_renderers: &'a HashSet<&'static str>,
    ) -> Self {
        let selected_entity = world.resource_mut::<SelectedEntity>().get(world).map(|e| {
            let transform = world.component::<TransformComponent>(e).unwrap(); // definitely ok because selected.get() just verified
            let details = match world.component::<BaseItemComponent>(e) {
                Ok(item) => EntityDetails::Item {
                    item: item.clone(),
                    edible: world.component(e).ok().cloned(),
                },
                _ => EntityDetails::Living {
                    activity: world
                        .component::<ActivityComponent>(e)
                        .map(|activity| format!("{}", activity.current))
                        .ok(),
                    hunger: world.component(e).ok().cloned(),
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
                transform: transform.clone(),
                details,
            }
        });

        let selected_tiles = world.resource::<SelectedTiles>();
        let player_society = world.resource::<PlayerSociety>().clone();
        let societies = world.resource::<Societies>();

        Self {
            selected_entity,
            selected_tiles: selected_tiles.clone(),
            player_society,
            societies,
            enabled_debug_renderers: debug_renderers,
            world_view: None,
        }
    }
}
