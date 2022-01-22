use common::*;
use unit::world::{WorldPoint, WorldPointRange, WorldPosition, WorldPositionRange, WorldRange};

use crate::ecs::*;
use crate::input::popup::{PopupContentType, UiPopup};
pub use crate::input::system::selected_tiles::SelectedTiles;
use crate::input::{InputEvent, InputModifier, SelectType, SelectionProgress, WorldColumn};
use crate::spatial::{Spatial, Transforms};
use crate::TransformComponent;
use crate::{Tick, UiElementComponent, WorldRef};

pub struct InputSystem<'a> {
    events: &'a [InputEvent],
}

const TILE_SELECTION_LIMIT: f32 = 50.0;
const DISTANCE_THRESHOLD: f32 = 2.0;

/// Marker for entity selection by the player
#[derive(Component, EcsComponent, Default)]
#[storage(NullStorage)]
#[name("selected")]
#[clone(disallow)]
pub struct SelectedComponent;

#[derive(Default)]
pub struct SelectedEntities {
    entities: Vec<Entity>,
    drag_in_progress: Option<WorldPointRange>,
}

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
        let resolve_entity_near_point = |point: WorldPoint| {
            // TODO multiple clicks in the same place should iterate through all entities in selection range
            // TODO spatial lookup for ui elements too
            // prioritise ui elements first
            let ui_elem = (&entities, &transform, &ui)
                .join()
                .find(|(_, transform, _)| transform.position.is_almost(&point, DISTANCE_THRESHOLD))
                .map(|(e, _, _)| e.into());

            // fallback to looking for normal entities
            ui_elem.or_else(|| {
                spatial
                    .query_in_radius(Transforms::Storage(&transform), point, DISTANCE_THRESHOLD)
                    .next()
                    .map(|(e, _, _)| e)
            })
        };

        let resolve_entity = |select_pos: &WorldColumn| {
            self.resolve_walkable_pos(select_pos, &world)
                .and_then(resolve_entity_near_point)
        };

        let resolve_all_entities_in_range = |range: WorldPointRange| {
            // TODO spatial lookup for all entities contained in the given range
            (&entities, &transform)
                .join()
                .filter_map(move |(e, transform)| {
                    if range.contains(&transform.position) {
                        Some(e.into())
                    } else {
                        None
                    }
                })
        };

        for e in self.events {
            match e {
                InputEvent::Select {
                    select: SelectType::Left,
                    from,
                    to,
                    progress,
                    modifiers,
                } => {
                    let additive = modifiers.contains(InputModifier::SHIFT);

                    if !additive {
                        // unselect all entities first
                        entity_sel.unselect_all_with_comps(&mut selecteds);
                    }

                    match progress {
                        SelectionProgress::InProgress => {
                            entity_sel.update_dragged_selection((*from, *to));
                        }
                        SelectionProgress::Complete => {
                            // no lasting selection
                            entity_sel.clear_dragged_selection();

                            let range = SelectedEntities::calculate_drag_range((*from, *to));
                            for e in resolve_all_entities_in_range(range) {
                                entity_sel.select_with_comps(&mut selecteds, e);
                            }
                        }
                    }
                }

                InputEvent::Select {
                    select: SelectType::Right,
                    from,
                    to,
                    progress,
                    ..
                } => {
                    // TODO additive tile selection
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
                        popups.open(PopupContentType::TileSelection);
                    } else if let Some(walkable) = self.resolve_walkable_pos(pos, &world) {
                        let popup = if let Some(target) = resolve_entity_near_point(walkable) {
                            PopupContentType::TargetEntity(target)
                        } else {
                            PopupContentType::TargetPoint(walkable)
                        };

                        popups.open(popup)
                    }
                }
            }
        }
    }
}

impl SelectedEntities {
    pub fn select(&mut self, world: &EcsWorld, e: Entity) {
        let mut selecteds = world.write_storage();
        self.select_with_comps(&mut selecteds, e)
    }

    fn select_with_comps(&mut self, selecteds: &mut WriteStorage<SelectedComponent>, e: Entity) {
        let res = selecteds.insert(e.into(), SelectedComponent);
        if let Ok(None) = res {
            debug_assert!(!self.entities.contains(&e));
            self.entities.push(e);
            debug!("selected entity"; e);
        }
    }

    pub fn unselect(&mut self, world: &EcsWorld, e: Entity) {
        let idx = self.entities.iter().position(|selected| *selected == e);
        if let Some(idx) = idx {
            self.entities.remove(idx); // preserve order, this isn't done often
            world.remove_now::<SelectedComponent>(e);
        }
    }

    pub fn unselect_all(&mut self, world: &EcsWorld) {
        let mut selecteds = world.write_storage();
        self.unselect_all_with_comps(&mut selecteds)
    }

    fn unselect_all_with_comps(&mut self, comp: &mut WriteStorage<SelectedComponent>) {
        for e in self.entities.drain(..) {
            debug!("unselected entity"; e);
            comp.remove(e.into());
        }
    }

    pub fn iter(&self) -> &[Entity] {
        &self.entities
    }

    pub fn just_one(&self) -> Option<Entity> {
        if self.entities.len() == 1 {
            let iter = self.entities.first();
            // safety: checked length
            let e = unsafe { iter.unwrap_unchecked() };
            Some(*e)
        } else {
            None
        }
    }

    pub fn count(&self) -> usize {
        self.entities.len()
    }

    pub fn drag_in_progress(&self) -> Option<&WorldPointRange> {
        self.drag_in_progress.as_ref()
    }

    fn update_dragged_selection(&mut self, range: (WorldColumn, WorldColumn)) {
        self.drag_in_progress = Some(Self::calculate_drag_range(range));
    }

    fn calculate_drag_range((from, to): (WorldColumn, WorldColumn)) -> WorldPointRange {
        let min_z = NotNan::new(from.slice_range.bottom().slice() as f32)
            .expect("i32 -> f32 should be valid");
        let max_z =
            NotNan::new(to.slice_range.top().slice() as f32).expect("i32 -> f32 should be valid");
        WorldPointRange::with_inclusive_range((from.x, from.y, min_z), (to.x, to.y, max_z))
    }

    fn clear_dragged_selection(&mut self) {
        self.drag_in_progress = None;
    }

    pub fn prune(&mut self, world: &EcsWorld) {
        self.entities.retain(|e| {
            let keep = world.is_entity_alive(*e);
            if !keep {
                debug!("pruning dead entity from selection"; e)
            }
            keep
        });
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
