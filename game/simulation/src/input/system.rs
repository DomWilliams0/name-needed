use common::*;
pub use selected_entities::{SelectedComponent, SelectedEntities};
use unit::world::{WorldPoint, WorldPosition, WorldPositionRange, WorldRange};

use crate::ecs::*;
use crate::input::popup::{PopupContentType, UiPopup};
pub use crate::input::system::selected_tiles::SelectedTiles;
use crate::input::{InputEvent, InputModifier, SelectType, SelectionProgress, WorldColumn};
use crate::spatial::{Spatial, Transforms};
use crate::TransformComponent;
use crate::{UiElementComponent, WorldRef};

pub struct InputSystem<'a> {
    events: &'a [InputEvent],
}

const TILE_SELECTION_LIMIT: f32 = 50.0;

impl<'a> InputSystem<'a> {
    /// Events for this tick
    pub fn with_events(events: &'a [InputEvent]) -> Self {
        Self { events }
    }

    fn resolve_walkable_pos(
        &self,
        select_pos: &WorldColumn,
        world: &WorldRef,
    ) -> Option<WorldPoint> {
        let world = (*world).borrow();
        select_pos.find_highest_walkable(&world)
    }
}

impl<'a> System<'a> for InputSystem<'a> {
    type SystemData = (
        Read<'a, WorldRef>,
        Read<'a, EntitiesRes>,
        Read<'a, Spatial>,
        Write<'a, SelectedEntities>,
        Write<'a, SelectedTiles>,
        Write<'a, UiPopup>,
        WriteStorage<'a, SelectedComponent>,
        ReadStorage<'a, TransformComponent>,
        ReadStorage<'a, UiElementComponent>,
    );

    fn run(
        &mut self,
        (
            world,
            entities,
            spatial,
            mut entity_sel,
            mut tile_sel,
            mut popups,
            mut selecteds,
            transform,
            ui,
        ): Self::SystemData,
    ) {
        let resolve_entity = |select_pos: &WorldColumn| {
            self.resolve_walkable_pos(select_pos, &world)
                .and_then(|point| {
                    // TODO multiple clicks in the same place should iterate through all entities in selection range
                    const RADIUS: f32 = 1.25;

                    // prioritise ui elements first
                    // TODO spatial lookup for ui elements too
                    let ui_elem = (&entities, &transform, &ui)
                        .join()
                        .find(|(_, transform, _)| transform.position.is_almost(&point, RADIUS))
                        .map(|(e, _, _)| e.into());

                    // fallback to looking for normal entities
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
                InputEvent::Select {
                    select: SelectType::Left,
                    ..
                } => {
                    // TODO select multiple entities
                }

                InputEvent::Select {
                    select: SelectType::Right,
                    from,
                    to,
                    progress,
                    ..
                } => {
                    // update tile selection
                    tile_sel.update((*from, *to), *progress, &world);
                }

                InputEvent::Click(SelectType::Left, pos, modifier) => {
                    let additive = modifier.contains(InputModifier::SHIFT);

                    if !additive {
                        // unselect all entities first
                        entity_sel.unselect_all_with_comps(&mut selecteds);
                    }

                    // find newly selected entity
                    if let Some(to_select) = resolve_entity(pos) {
                        entity_sel.select_with_comps(&mut selecteds, to_select);
                    }
                }

                InputEvent::Click(SelectType::Right, pos, _) => {
                    if tile_sel.is_right_click_relevant(pos) {
                        // show popup for selection
                        popups.open(PopupContentType::TileSelection);
                        // } else if let Some(entity) = resolve_entity(pos) {
                        //     // show popup for entity
                        //     popups.open(PopupContentType::Entity(entity));
                    }
                }
            }
        }
    }
}

mod selected_tiles {
    use std::collections::BTreeMap;
    use std::fmt::Write;

    use unit::world::SlabLocation;
    use world::block::BlockType;

    use crate::input::SelectionModification;
    use crate::InnerWorldRef;

    use super::*;

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
            if let Some(sel) = self.current.as_mut() {
                sel.on_world_change(world.borrow())
            }
        }

        /// Is the given right click position close enough to the bottom right of the active
        /// selection
        pub fn is_right_click_relevant(&self, pos: &WorldColumn) -> bool {
            const DISTANCE_THRESHOLD: f32 = 2.0;
            self.current_selected()
                .map(|sel| {
                    // distance check to bottom right corner, ignoring z axis

                    let corner = {
                        let ((_, x2), (y1, _), _) = sel.range.ranges();
                        Point2::new(x2 as f32, y1 as f32)
                    };
                    let click = Point2::new(pos.x.into_inner(), pos.y.into_inner());

                    corner.distance2(click) <= DISTANCE_THRESHOLD * DISTANCE_THRESHOLD
                })
                .unwrap_or(false)
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
            for (b, _) in world.iterate_blocks(self.range.clone()) {
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
        use world::block::BlockType;

        use crate::input::system::selected_tiles::BlockOccurrences;

        use super::*;

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

mod selected_entities {
    use std::collections::HashSet;

    use common::*;

    use crate::ecs::*;

    /// Marker for entity selection by the player
    #[derive(Component, EcsComponent, Default)]
    #[storage(NullStorage)]
    #[name("selected")]
    #[clone(disallow)]
    pub struct SelectedComponent;

    #[derive(Default)]
    pub struct SelectedEntities {
        entities: HashSet<Entity>,
        last: Option<Entity>,
    }

    impl SelectedEntities {
        pub fn select(&mut self, world: &EcsWorld, e: Entity) {
            let mut selecteds = world.write_storage();
            self.select_with_comps(&mut selecteds, e)
        }

        pub fn select_with_comps(
            &mut self,
            selecteds: &mut WriteStorage<SelectedComponent>,
            e: Entity,
        ) {
            debug!("selected entity"; e);
            if self.entities.insert(e) {
                let _ = selecteds.insert(e.into(), SelectedComponent);
            }
            self.last = Some(e);
        }

        pub fn unselect(&mut self, world: &EcsWorld, e: Entity) {
            if self.entities.remove(&e) {
                world.remove_now::<SelectedComponent>(e);

                if self.last == Some(e) {
                    self.last = self.entities.iter().next().copied();
                }
            }
        }

        pub fn unselect_all(&mut self, world: &EcsWorld) {
            let mut selecteds = world.write_storage();
            self.unselect_all_with_comps(&mut selecteds)
        }

        pub fn unselect_all_with_comps(&mut self, comp: &mut WriteStorage<SelectedComponent>) {
            for e in self.entities.drain() {
                debug!("unselected entity"; e);
                comp.remove(e.into());
            }
            self.last = None;
        }

        /// Could be dead
        pub fn iter_unchecked(&self) -> impl Iterator<Item = Entity> + '_ {
            self.entities.iter().copied()
        }

        pub fn primary(&self) -> Option<Entity> {
            self.last
        }

        pub fn just_one(&self) -> Option<Entity> {
            self.entities.iter().exactly_one().ok().copied()
        }

        pub fn count(&self) -> usize {
            self.entities.len()
        }
    }
}
