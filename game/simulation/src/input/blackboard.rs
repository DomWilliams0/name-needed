use crate::ai::ActivityComponent;
use crate::ecs::Entity;
use crate::input::SelectedEntity;
use crate::item::{BaseItemComponent, EdibleItemComponent};
use crate::needs::HungerComponent;
use crate::path::FollowPathComponent;
use crate::{ComponentWorld, TransformComponent};
use std::collections::HashSet;
use unit::world::WorldPoint;
use world::SliceRange;

/// Dump of game info for the UI to render
pub struct Blackboard<'a> {
    pub selected: Option<SelectedEntityDetails>,
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
    },
    Item {
        item: BaseItemComponent,
        edible: Option<EdibleItemComponent>,
    },
}

impl<'a> Blackboard<'a> {
    pub fn fetch<W: ComponentWorld>(world: &W, debug_renderers: &'a HashSet<&'static str>) -> Self {
        let selected = world
            .resource_mut(|selected: &mut SelectedEntity| selected.get(world))
            .map(|e| {
                let transform = *world.component::<TransformComponent>(e).unwrap(); // definitely ok because selected.get() just verified
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
                    },
                };

                SelectedEntityDetails {
                    entity: e,
                    transform,
                    details,
                }
            });

        Self {
            selected,
            enabled_debug_renderers: debug_renderers,
            world_view: None,
        }
    }
}
