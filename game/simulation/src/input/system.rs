use common::*;
use unit::world::{WorldPosition, WorldPositionRange, WorldRange};

use crate::ecs::*;
pub use crate::input::system::selected_tiles::SelectedTiles;
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

mod selected_tiles {
    use super::*;
    use crate::input::SelectionModification;
    use crate::InnerWorldRef;
    use std::collections::BTreeMap;
    use std::fmt::Write;
    use unit::world::{all_slabs_in_range, SlabLocation};
    use world::block::BlockType;

    #[derive(Default, Clone)]
    pub struct SelectedTiles {
        current: Option<CurrentSelection>,
        last: Option<(WorldPosition, WorldPosition, SelectionProgress)>,
    }

    #[derive(Clone)]
    pub struct CurrentSelection {
        range: WorldPositionRange,
        progress: SelectionProgress,
        makeup: BTreeMap<BlockType, u32>,
    }

    pub struct BlockOccurrences<'a>(&'a BTreeMap<BlockType, u32>);

    impl SelectedTiles {
        pub fn update(
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

                    self.current = Some(CurrentSelection::new(
                        WorldPositionRange::with_inclusive_range(a, b),
                        progress,
                        self.current.take(),
                        &w,
                    ));

                    if let SelectionProgress::Complete = progress {
                        debug!("selecting tiles"; "min" => %a, "max" => %b);
                    }
                }
            }
        }

        pub fn clear(&mut self) {
            self.current = None;
        }

        /// Includes in-progress selection
        pub fn current(&self) -> Option<&CurrentSelection> {
            self.current.as_ref()
        }

        /// Only complete selection
        pub fn current_selected(&self) -> Option<&CurrentSelection> {
            match self.current.as_ref() {
                Some(
                    sel @ CurrentSelection {
                        progress: SelectionProgress::Complete,
                        ..
                    },
                ) => Some(sel),
                _ => None,
            }
        }

        pub fn modify(&mut self, modification: SelectionModification, world: &WorldRef) {
            if let Some(
                sel @ CurrentSelection {
                    progress: SelectionProgress::Complete,
                    ..
                },
            ) = self.current.as_mut()
            {
                sel.modify(modification, world);
            }
        }

        pub fn on_world_change(&mut self, world: &WorldRef) {
            self.current
                .as_mut()
                .map(|sel| sel.on_world_change(world.borrow()));
        }
    }

    impl CurrentSelection {
        fn new(
            range: WorldPositionRange,
            progress: SelectionProgress,
            prev: Option<Self>,
            world: &InnerWorldRef,
        ) -> Self {
            let makeup = prev
                .map(|mut sel| {
                    sel.makeup.clear();
                    sel.makeup
                })
                .unwrap_or_default();

            let mut sel = Self {
                range,
                progress,
                makeup,
            };

            sel.update_makeup(world);

            sel
        }

        fn on_world_change(&mut self, world: InnerWorldRef) {
            self.update_makeup(&world);
        }

        fn update_makeup(&mut self, world: &InnerWorldRef) {
            self.makeup.clear();
            for (b, _) in world.iterate_blocks(&self.range) {
                self.makeup
                    .entry(b.block_type())
                    .and_modify(|count| *count += 1)
                    .or_insert(1);
            }
        }

        pub fn bounds(&self) -> (WorldPosition, WorldPosition) {
            self.range.bounds()
        }

        pub fn progress(&self) -> SelectionProgress {
            self.progress
        }

        pub fn range(&self) -> &WorldPositionRange {
            &self.range
        }

        pub fn single_tile(&self) -> Option<WorldPosition> {
            match &self.range {
                WorldRange::Single(pos) => Some(*pos),
                WorldRange::Range(a, b) if a == b => Some(*a),
                _ => None,
            }
        }

        pub fn block_occurrences(&self) -> impl Display + '_ {
            BlockOccurrences(&self.makeup)
        }

        pub fn modify(&mut self, modification: SelectionModification, world: &WorldRef) {
            let new = match modification {
                SelectionModification::Up => self.range.above(),
                SelectionModification::Down => self.range.below(),
            };

            if let Some(new) = new {
                let w = world.borrow();
                let (from, to) = new.bounds();

                if w.has_slab(SlabLocation::new(from.slice().slab_index(), from))
                    && w.has_slab(SlabLocation::new(to.slice().slab_index(), to))
                {
                    self.range = new;
                    self.update_makeup(&world.borrow())
                }
            }
        }
    }

    impl Display for BlockOccurrences<'_> {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            f.write_char('[')?;

            let mut comma = false;
            for (b, n) in self.0.iter() {
                if comma {
                    f.write_str(", ")?;
                } else {
                    comma = true;
                }
                write!(f, "{}x{}", n, b)?;
            }

            f.write_char(']')
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::input::system::selected_tiles::BlockOccurrences;
        use std::collections::HashMap;
        use world::block::BlockType;

        #[test]
        fn comma_separated_blocks() {
            let mut map = BTreeMap::new();

            assert_eq!(format!("{}", BlockOccurrences(&map)), "[]");

            map.insert(BlockType::Air, 5);
            assert_eq!(format!("{}", BlockOccurrences(&map)), "[5xAir]");

            map.insert(BlockType::Dirt, 2);
            assert_eq!(format!("{}", BlockOccurrences(&map)), "[5xAir, 2xDirt]");

            map.insert(BlockType::Grass, 10);
            assert_eq!(
                format!("{}", BlockOccurrences(&map)),
                "[5xAir, 2xDirt, 10xGrass]"
            );
        }
    }
}
