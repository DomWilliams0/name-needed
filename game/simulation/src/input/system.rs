use common::*;
use unit::world::{WorldPosition, WorldPositionRange, WorldRange};

use crate::ecs::*;
use crate::input::{InputEvent, SelectType, WorldColumn};
use crate::WorldRef;
use crate::{RenderComponent, TransformComponent};

pub struct InputSystem<'a> {
    pub events: &'a [InputEvent],
}

/// Marker for entity selection by the player
#[derive(Component, EcsComponent, Default)]
#[storage(NullStorage)]
#[name("selected")]
pub struct SelectedComponent;

/// Resource for selected entity - not guaranteed to be alive
/// `get()` will clear it if the entity is dead
#[derive(Default)]
pub struct SelectedEntity(Option<Entity>);

#[derive(Default, Clone)]
pub struct SelectedTiles(Option<WorldPositionRange>);

const TILE_SELECTION_LIMIT: f32 = 50.0;

impl<'a> System<'a> for InputSystem<'a> {
    type SystemData = (
        Read<'a, WorldRef>,
        Read<'a, EntitiesRes>,
        Write<'a, SelectedEntity>,
        Write<'a, SelectedTiles>,
        WriteStorage<'a, SelectedComponent>,
        ReadStorage<'a, TransformComponent>,
        ReadStorage<'a, RenderComponent>,
    );

    fn run(
        &mut self,
        (world, entities, mut selected, mut selected_block, mut selecteds, transform, render): Self::SystemData,
    ) {
        let resolve_walkable_pos = |select_pos: &WorldColumn| {
            let world = (*world).borrow();
            select_pos.find_highest_walkable(&world)
        };

        let resolve_entity = |select_pos: &WorldColumn| {
            resolve_walkable_pos(select_pos).and_then(|point| {
                // TODO spatial query rather than checking every entity ever
                // TODO multiple clicks in the same place should iterate through all entities in selection range

                const SELECT_THRESHOLD: f32 = 1.25;
                (&entities, &transform, &render)
                    .join()
                    .find(|(_, transform, _)| {
                        transform.position.is_almost(&point, SELECT_THRESHOLD)
                    }) // just choose the first in range for now
                    .map(|(e, _, _)| e.into())
            })
        };

        for e in self.events {
            match e {
                InputEvent::Click(SelectType::Left, pos) => {
                    // unselect current entity regardless of click location
                    selected.unselect_with_comps(&mut selecteds);

                    // find newly selected entity
                    if let Some(to_select) = resolve_entity(pos) {
                        selected.select_with_comps(&mut selecteds, to_select);
                    }
                }

                InputEvent::Select(SelectType::Left, _, _) => {
                    // TODO select multiple entities
                }

                InputEvent::Click(SelectType::Right, _) => {
                    // unselect tile selection
                    selected_block.0 = None;
                }

                InputEvent::Select(SelectType::Right, from, to) => {
                    // select tiles
                    let w = (*world).borrow();

                    // limit selection size by moving the second point placed. this isn't totally
                    // accurate and may allow selections of 1 block bigger, but meh who cares
                    let to = {
                        let dx = (from.x - to.x).abs();
                        let dy = (from.y - to.y).abs();

                        let x_overlap = TILE_SELECTION_LIMIT - dx;
                        let y_overlap = TILE_SELECTION_LIMIT - dy;

                        let mut to = *to;
                        if x_overlap < 0.0 {
                            let mul = if from.x > to.x { 1.0 } else { -1.0 };
                            to.x -= mul * x_overlap;
                        }

                        if y_overlap < 0.0 {
                            let mul = if from.y > to.y { 1.0 } else { -1.0 };
                            to.y -= mul * y_overlap;
                        }

                        to
                    };

                    selected_block.0 = from.find_min_max_walkable(&to, &w).map(|(min, max)| {
                        let mut a = min.floor();
                        let mut b = max.floor();

                        // these blocks are walkable air blocks, move them down 1 to select the
                        // actual block beneath
                        a.2 -= 1;
                        b.2 -= 1;

                        debug!("selecting tiles"; "min" => %a, "max" => %b);
                        WorldPositionRange::with_inclusive_range(a, b)
                    });
                }
            }
        }
    }
}

impl SelectedEntity {
    pub fn get<W: ComponentWorld>(&mut self, world: &W) -> Option<Entity> {
        match self.0 {
            None => None,
            Some(e) if world.component::<TransformComponent>(e).is_err() => {
                // entity is dead or no longer has transform
                self.0 = None;
                None
            }
            nice => nice, // still alive
        }
    }

    /// Entity may not be alive
    pub fn get_unchecked(&self) -> Option<Entity> {
        self.0
    }

    pub fn select(&mut self, world: &EcsWorld, e: Entity) {
        let mut selecteds = world.write_storage();
        self.select_with_comps(&mut selecteds, e)
    }

    fn select_with_comps(&mut self, selecteds: &mut WriteStorage<SelectedComponent>, e: Entity) {
        // unselect current entity
        self.unselect_with_comps(selecteds);

        debug!("selected entity"; e);
        let _ = selecteds.insert(e.into(), SelectedComponent);
        self.0 = Some(e);
    }

    pub fn unselect(&mut self, world: &EcsWorld) {
        let mut selecteds = world.write_storage();
        self.unselect_with_comps(&mut selecteds)
    }

    fn unselect_with_comps(&mut self, comp: &mut WriteStorage<SelectedComponent>) {
        if let Some(old) = self.0.take() {
            debug!("unselected entity"; old);
            comp.remove(old.into());
        }
    }
}

impl SelectedTiles {
    pub fn range(&self) -> Option<WorldPositionRange> {
        self.0.as_ref().cloned()
    }
    pub fn bounds(&self) -> Option<(WorldPosition, WorldPosition)> {
        self.0.as_ref().map(|range| range.bounds())
    }

    pub fn single_tile(&self) -> Option<WorldPosition> {
        self.0.clone().and_then(|range| match range {
            WorldRange::Single(pos) => Some(pos),
            WorldRange::Range(a, b) if a == b => Some(a),
            _ => None,
        })
    }
}
