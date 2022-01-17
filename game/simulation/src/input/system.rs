use common::*;
use unit::world::{WorldPosition, WorldPositionRange, WorldRange};

use crate::ecs::*;
use crate::input::{InputEvent, SelectType, SelectionProgress, WorldColumn};
use crate::spatial::{Spatial, Transforms};
use crate::TransformComponent;
use crate::{UiElementComponent, WorldRef};

pub struct InputSystem<'a> {
    pub events: &'a [InputEvent],
}

/// Marker for entity selection by the player
#[derive(Component, EcsComponent, Default)]
#[storage(NullStorage)]
#[name("selected")]
#[clone(disallow)]
pub struct SelectedComponent;

/// Resource for selected entity - not guaranteed to be alive
/// `get()` will clear it if the entity is dead
#[derive(Default)]
pub struct SelectedEntity(Option<Entity>);

#[derive(Default, Clone)]
pub struct SelectedTiles {
    current: Option<(WorldPositionRange, SelectionProgress)>,
    last: Option<(WorldPosition, WorldPosition, SelectionProgress)>,
}

const TILE_SELECTION_LIMIT: f32 = 50.0;

impl<'a> System<'a> for InputSystem<'a> {
    type SystemData = (
        Read<'a, WorldRef>,
        Read<'a, EntitiesRes>,
        Read<'a, Spatial>,
        Write<'a, SelectedEntity>,
        Write<'a, SelectedTiles>,
        WriteStorage<'a, SelectedComponent>,
        ReadStorage<'a, TransformComponent>,
        ReadStorage<'a, UiElementComponent>,
    );

    fn run(
        &mut self,
        (world, entities, spatial, mut selected, mut selected_block, mut selecteds, transform, ui): Self::SystemData,
    ) {
        let resolve_walkable_pos = |select_pos: &WorldColumn| {
            let world = (*world).borrow();
            select_pos.find_highest_walkable(&world)
        };

        let resolve_entity = |select_pos: &WorldColumn| {
            resolve_walkable_pos(select_pos).and_then(|point| {
                // TODO multiple clicks in the same place should iterate through all entities in selection range

                const RADIUS: f32 = 1.25;

                // prioritise ui elements first
                // TODO spatial lookup for ui elements too
                let ui_elem = (&entities, &transform, &ui)
                    .join()
                    .find(|(_, transform, _)| transform.position.is_almost(&point, RADIUS))
                    .map(|(e, _, _)| e.into());

                // fallack to looking for normal entities
                ui_elem.or_else(|| {
                    spatial
                        .query_in_radius(Transforms::Storage(&transform), point, RADIUS)
                        .next()
                        .map(|(e, _, _)| e)
                })
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

                InputEvent::Select {
                    select: SelectType::Left,
                    ..
                } => {
                    // TODO select multiple entities
                }

                InputEvent::Click(SelectType::Right, _) => {
                    // unselect tile selection
                    selected_block.clear();
                }

                InputEvent::Select {
                    select: SelectType::Right,
                    from,
                    to,
                    progress,
                } => {
                    // update tile selection
                    selected_block.update((*from, *to), *progress, &world);
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
    fn update(
        &mut self,
        range: (WorldColumn, WorldColumn),
        progress: SelectionProgress,
        world: &WorldRef,
    ) {
        let (from, mut to) = range;

        // limit selection size by moving the second point placed. this isn't totally
        // accurate and may allow selections of 1 block bigger, but meh who cares
        let to = {
            let dx = (from.x - to.x).abs();
            let dy = (from.y - to.y).abs();

            let x_overlap = TILE_SELECTION_LIMIT - dx;
            let y_overlap = TILE_SELECTION_LIMIT - dy;

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

        let w = world.borrow();
        if let Some((min, max)) = from.find_min_max_walkable(&to, &w) {
            let mut a = min.floor();
            let mut b = max.floor();

            // these blocks are walkable air blocks, move them down 1 to select the
            // actual block beneath
            a.2 -= 1;
            b.2 -= 1;

            if Some((a, b, progress)) != self.last {
                self.last = Some((a, b, progress));
                self.current = Some((WorldPositionRange::with_inclusive_range(a, b), progress));

                if let SelectionProgress::Complete = progress {
                    debug!("selecting tiles"; "min" => %a, "max" => %b);
                }
            }
        }
    }

    fn clear(&mut self) {
        self.current = None;
    }

    /// Includes in-progress selection
    pub fn bounds(&self) -> Option<(SelectionProgress, (WorldPosition, WorldPosition))> {
        self.current
            .as_ref()
            .map(|(range, progress)| (*progress, range.bounds()))
    }

    fn selected(&self) -> Option<&WorldPositionRange> {
        match self.current.as_ref() {
            Some((range, SelectionProgress::Complete)) => Some(range),
            _ => None,
        }
    }

    pub fn selected_range(&self) -> Option<WorldPositionRange> {
        self.selected().cloned()
    }

    pub fn selected_bounds(&self) -> Option<(WorldPosition, WorldPosition)> {
        self.selected().map(|range| range.bounds())
    }

    pub fn single_tile(&self) -> Option<WorldPosition> {
        self.selected().and_then(|range| match *range {
            WorldRange::Single(pos) => Some(pos),
            WorldRange::Range(a, b) if a == b => Some(a),
            _ => None,
        })
    }
}
