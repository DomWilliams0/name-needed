use crate::ecs::*;
use crate::input::{InputEvent, SelectType, WorldColumn};
use crate::{RenderComponent, TransformComponent};
use common::*;
use unit::world::WorldPosition;
use world::WorldRef;

pub struct InputSystem<'a> {
    pub events: &'a [InputEvent],
}

/// Marker for entity selection by the player
#[derive(Component, Default)]
#[storage(NullStorage)]
pub struct SelectedComponent;

/// Resource for selected entity - not guaranteed to be alive
/// `get()` will clear it if the entity is dead
#[derive(Default)]
pub struct SelectedEntity(Option<Entity>);

#[derive(Clone)]
pub enum SelectedTiles {
    None,
    Single(WorldPosition),
    Range(WorldPosition, WorldPosition),
}

const TILE_SELECTION_LIMIT: i32 = 64;

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

                const SELECT_THRESHOLD: f32 = 1.0;
                let point = Point3::from(point);
                (&entities, &transform, &render)
                    .join()
                    .find(|(_, transform, _)| {
                        point.distance2(transform.position.into()) < SELECT_THRESHOLD.powi(2)
                    }) // just choose the first in range
                    .map(|(e, _, _)| e)
            })
        };

        for e in self.events {
            match e {
                InputEvent::Click(SelectType::Left, pos) => {
                    // unselect current entity
                    unselect_current(&mut selected, &mut selecteds);

                    // find newly selected entity
                    if let Some(to_select) = resolve_entity(pos) {
                        debug!("selected entity {:?}", to_select);
                        let _ = selecteds.insert(to_select, SelectedComponent);
                        selected.0 = Some(to_select);
                    }
                }

                InputEvent::Select(SelectType::Left, _, _) => {
                    // TODO select multiple entities
                }

                InputEvent::Click(SelectType::Right, _) => {
                    // unselect tile selection
                    *selected_block = SelectedTiles::None
                }

                InputEvent::Select(SelectType::Right, from, to) => {
                    // select tiles
                    *selected_block = match (resolve_walkable_pos(from), resolve_walkable_pos(to)) {
                        (Some(from), Some(to)) => {
                            let (to, from) = {
                                // round away from first point
                                let (mut x, mul_x) = if to.0 > from.0 {
                                    (to.0.ceil() as i32, 1)
                                } else {
                                    (to.0.floor() as i32, -1)
                                };
                                let (mut y, mul_y) = if to.1 > from.1 {
                                    (to.1.ceil() as i32, 1)
                                } else {
                                    (to.1.floor() as i32, -1)
                                };

                                let from = from.round();

                                // minimum 1 along each axis
                                if from.0 == x {
                                    x += mul_x;
                                }
                                if from.1 == y {
                                    y += mul_y;
                                }

                                // maximum along each axis
                                let dx = (from.0 - x).abs();
                                let dy = (from.1 - y).abs();
                                if dx > TILE_SELECTION_LIMIT {
                                    x -= (dx - TILE_SELECTION_LIMIT) * mul_x;
                                }
                                if dy > TILE_SELECTION_LIMIT {
                                    y -= (dy - TILE_SELECTION_LIMIT) * mul_y;
                                }

                                (WorldPosition(x, y, to.slice()), from)
                            };

                            debug!("selected block region {:?} -> {:?}", from, to);
                            SelectedTiles::Range(from, to)
                        }
                        _ => SelectedTiles::None,
                    }
                }
            }
        }
    }
}

fn unselect_current(res: &mut Write<SelectedEntity>, comp: &mut WriteStorage<SelectedComponent>) {
    if let Some(old) = res.0.take() {
        debug!("unselected entity {:?}", old);
        comp.remove(old);
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
}

impl Default for SelectedTiles {
    fn default() -> Self {
        SelectedTiles::None
    }
}

impl SelectedTiles {
    /// Inclusive
    pub fn bounds(&self) -> Option<(WorldPosition, WorldPosition)> {
        match *self {
            SelectedTiles::None => None,
            SelectedTiles::Single(pos) => Some((pos, pos)),
            SelectedTiles::Range(from, to) => {
                let dx = (to.0 - from.0).signum();
                let dy = (to.1 - from.1).signum();

                Some((from, to + (-dx, -dy, 0)))
            }
        }
    }
}
