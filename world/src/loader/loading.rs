use std::time::{Duration, Instant};

use futures::channel::mpsc as async_channel;
use misc::*;
use unit::world::{
    ChunkLocation, GlobalSliceIndex, LocalSliceIndex, SlabIndex, SlabLocation, SlabPosition,
    SliceIndex, WorldPosition, CHUNK_SIZE, SLAB_SIZE,
};

use crate::chunk::slab::{Slab, SlabType, SliceNavArea};

use crate::world::{
    get_or_collect_slab_areas, get_or_wait_for_slab_vertical_space, ContiguousChunkIterator,
    ListeningLoadNotifier, WorldChangeEvent,
};
use crate::{
    navigationv2, OcclusionChunkUpdate, WorldContext, WorldRef, ABSOLUTE_MAX_FREE_VERTICAL_SPACE,
};

use crate::chunk::slice_navmesh::{SlabVerticalSpace, SliceAreaIndex};
use crate::chunk::{NeighbourAreaHash, SlabNeighbour};
use crate::loader::{AsyncWorkerPool, TerrainSource, TerrainSourceError, WorldTerrainUpdate};
use crate::navigationv2::{as_border_area, SlabArea, SlabNavEdge, SlabNavGraph};
use crate::neighbour::NeighbourOffset;

use ahash::RandomState;

use futures::{FutureExt, SinkExt};
use std::collections::HashSet;
use std::iter::repeat;

use std::sync::Arc;

pub struct WorldLoader<C: WorldContext> {
    source: TerrainSource<C>,
    pool: AsyncWorkerPool,
    world: WorldRef<C>,
    last_batch_size: usize,
}

#[derive(Debug, Error)]
pub enum BlockForAllError {
    #[error("A batch of chunks must be requested first")]
    NoBatch,

    #[error("Timed out")]
    TimedOut,

    #[error("Failed to load terrain: {0}")]
    Error(#[from] TerrainSourceError),
}

mod load_task {
    use super::*;
    use crate::block::BlockOpacity;
    use crate::chunk::slice::Slice;
    use crate::chunk::{SparseGrid, SparseGridExtension};
    use crate::loader::worker_pool::LoadSuccessTx;
    use crate::occlusion::{
        NeighbourOpacity, OcclusionOpacity, OcclusionUpdateType, RelativeSlabs,
    };
    use crate::world::get_or_wait_for_slab;
    use crate::{flatten_coords, iter_slice_xy, BlockOcclusion, OcclusionFace};
    use futures::pin_mut;
    use grid::GridImpl;
    use std::cell::{RefCell, UnsafeCell};
    use std::sync::Weak;
    use unit::world::{RangePosition, SlabPositionAsCoord, SliceBlock};

    enum ExtraInfo<C: WorldContext> {
        Generated {
            entities: Vec<C::GeneratedEntityDesc>,
            terrain: Slab<C>,
            vs: Arc<SlabVerticalSpace>,
            occlusion: SparseGrid<BlockOcclusion>,
        },
        Updated,
    }

    struct LoadContext<C: WorldContext> {
        world: WorldRef<C>,
        this_slab: SlabLocation,
        extra: ExtraInfo<C>,
        success_tx: LoadSuccessTx,
    }

    async fn the_ultimate_load_task<C: WorldContext>(mut ctx: LoadContext<C>) {
        let slab = ctx.this_slab;
        // ----- slab is currently Requested

        // get terrain and vertical space
        let is_update = matches!(ctx.extra, ExtraInfo::Updated);
        let (terrain, new_terrain, vs, entities, occlusion) = match ctx.extra {
            ExtraInfo::Generated {
                entities,
                terrain,
                vs,
                occlusion,
            } => (terrain.clone(), Some(terrain), vs, entities, occlusion),
            ExtraInfo::Updated => {
                let w = ctx.world.borrow();

                let terrain = if let Some(t) = w
                    .find_chunk_with_pos(slab.chunk)
                    .and_then(|c| c.terrain().slab(slab.slab))
                {
                    t.clone()
                } else {
                    debug!("changed {} doesn't exist anymore", ctx.this_slab);
                    return;
                };

                // recalculate vertical space for this slab. it is capped to the top of
                // this slab, but its only use is for area discovery while we also have the above one
                // available, so no matter for loading
                let vs = SlabVerticalSpace::discover(&terrain);
                let occlusion = init_internal_occlusion(slab, &terrain, &vs);

                (terrain, None, vs, vec![], occlusion)
            }
        };

        // put terrain into world so other slabs can access it asap
        let mut notifications;
        {
            let mut w = ctx.world.borrow_mut();
            notifications = w.load_notifications().start_listening(); // might be pointless sometimes

            // spawn entities if necessary
            if !entities.is_empty() {
                trace!(
                    "passing {n} entity descriptions from slab to world to spawn next tick",
                    n = entities.len()
                );

                // TODO maybe delay until nav is done?
                w.queue_entities_to_spawn(entities.into_iter());
            }

            // put terrain into chunk
            w.populate_chunk_with_slab(slab, new_terrain, vs.clone(), occlusion);
        }

        // ----- slab is now TerrainInWorld

        // notify loader
        if let Err(e) = ctx.success_tx.send(Ok(slab)).await {
            error!("failed to send terrain result"; "error" => %e);
            return;
        }

        // init occlusion using neighbouring slabs
        update_neighbour_occlusion(&vs, slab, &mut notifications, &ctx.world).await;

        // new slab can now be made visible
        {
            let mut w = ctx.world.borrow_mut();
            w.mark_slab_dirty(slab);
        }

        // generate nav areas, skipping the bottom slice initially.
        // needs vs from slab above. doesnt matter that it is capped locally, the max vs is
        // much less than a slab height, so no impact on this currently updating slab.
        debug_assert!(ABSOLUTE_MAX_FREE_VERTICAL_SPACE < SLAB_SIZE.as_u8());
        let (mut areas, above_vs) = discover_areas_with_vertical_neighbours(
            slab,
            &vs,
            &mut notifications,
            &terrain,
            &ctx.world,
            None,
        )
        .await;

        // discover links between internal areas
        trace!("{} has {} areas: {:?}", slab, areas.len(), areas);
        let graph = SlabNavGraph::discover(&areas);

        // spawn a task to add any new areas to the slab above
        if let Some(above_vs) = above_vs {
            let above_slab = slab.above();
            {
                // mark above slab as updating
                let w = ctx.world.borrow();
                if let Some(c) = w.find_chunk_with_pos(above_slab.chunk) {
                    c.mark_slab_as_updating(above_slab.slab);
                }
            }

            tokio::spawn(update_slab_above(
                ctx.world.clone(),
                above_slab,
                above_vs,
                vs.clone(),
            ));
        }

        // put this into world and mark as DoneInIsolation
        let current_hashes = match replace_slab_nav_graph(&ctx.world, slab, graph, &areas) {
            None => return,
            Some(h) if is_update => h,
            _ => Default::default(), // all zeroes for newly generated slab
        };

        // ------ slab is now DoneInIsolation

        link_up_with_neighbours(slab, &areas, notifications, &ctx.world, current_hashes).await;

        // that's all
        {
            let mut w = ctx.world.borrow_mut();

            if let Some(chunk) = w.find_chunk_with_pos_mut(slab.chunk) {
                chunk.mark_slab_as_done(slab.slab);
            }
        }
    }

    async fn link_up_with_neighbours<C: WorldContext>(
        this_slab: SlabLocation,
        this_areas: &[SliceNavArea],
        notifications: ListeningLoadNotifier,
        world: &WorldRef<C>,
        current_hashes: [NeighbourAreaHash; 6],
    ) {
        use UpdateNeighbour::*;
        let mut ctx = UpdateNeighbourContext {
            this_slab,
            this_areas,
            current_hashes,
            this_border_areas: vec![],
            neighbour_border_areas: vec![],
            new_world_edges: Default::default(),
            notifications,
        };

        // TODO could do these in parallel, but would need own allocations then
        for n in [North, East, South, West, Above] {
            n.apply(&mut ctx, world).await;

            ctx.this_border_areas.clear();
            ctx.neighbour_border_areas.clear();
        }
    }

    async fn update_slab_above<C: WorldContext>(
        world: WorldRef<C>,
        slab: SlabLocation,
        this_vs: Arc<SlabVerticalSpace>,
        below_vs: Arc<SlabVerticalSpace>,
    ) {
        trace!("running update_slab_above for {slab}");
        let mut notifications;
        let current_hashes;
        let terrain;
        {
            let w = world.borrow();
            notifications = w.load_notifications().start_listening();

            if let Some((chunk, data)) = w
                .find_chunk_with_pos(slab.chunk)
                .and_then(|c| c.terrain().slab_data(slab.slab).map(|d| (c, d)))
            {
                terrain = data.terrain.clone();
                current_hashes = data.neighbour_edge_hashes;
            } else {
                warn!("missing slab {} in update task", slab);
                return;
            }
        }

        // discover new bottom slice areas using vs and below vs
        let mut bottom_areas = vec![];
        terrain.discover_bottom_slice_areas(&this_vs, &below_vs, &mut bottom_areas); // order should be same as previous discovery, because they are discovered the same way
        let new_hash = NeighbourAreaHash::for_areas(bottom_areas.iter().copied());

        trace!(
            "{} has {} bottom areas: {:?}. hash is {:?}, prev was {:?}",
            slab,
            bottom_areas.len(),
            bottom_areas,
            new_hash,
            current_hashes[SlabNeighbour::Bottom as usize],
        );

        // check previous bottom areas
        if new_hash == current_hashes[SlabNeighbour::Bottom as usize] {
            trace!("bottom areas did not change, skipping all changes");
            if let Some(c) = world.borrow().find_chunk_with_pos(slab.chunk) {
                c.mark_slab_as_done(slab.slab);
            }

            return;
        }

        // need to redo graph
        // TODO can probably skip some work here as only the bottom areas changed

        let (areas, above_vs) = discover_areas_with_vertical_neighbours(
            slab,
            &this_vs,
            &mut notifications,
            &terrain,
            &world,
            Some(bottom_areas),
        )
        .await;

        let graph = SlabNavGraph::discover(&areas);
        replace_slab_nav_graph(&world, slab, graph, &areas);

        // now DoneInIsolation

        link_up_with_neighbours(slab, &areas, notifications, &world, current_hashes).await;

        // slab is Done
        if let Some(c) = world.borrow().find_chunk_with_pos(slab.chunk) {
            c.mark_slab_as_done(slab.slab);
        }
    }

    pub async fn generate<C: WorldContext>(
        world: WorldRef<C>,
        this_slab: SlabLocation,
        slab_type: SlabType,
        source: TerrainSource<C>,
        mut success_tx: LoadSuccessTx,
    ) {
        let mut entities = vec![];
        let result = if let SlabType::Placeholder = slab_type {
            // empty placeholder
            Ok(None)
        } else {
            source.load_slab(this_slab).await.map(|generated| {
                entities = generated.entities;
                Some(generated.terrain)
            })
        };

        let (terrain, vs) = match result {
            Ok(Some(terrain)) => {
                // TODO use shared reference of all air/all X terrain. then use a shared verticalspace reference for all air/all solid
                let vs = SlabVerticalSpace::discover(&terrain);
                (terrain, vs)
            }
            Ok(None) => {
                debug!("adding placeholder slab to the top of the chunk"; this_slab);
                (Slab::empty_placeholder(), SlabVerticalSpace::empty())
            }
            Err(TerrainSourceError::SlabOutOfBounds(slab)) => {
                // soft error, we're at the world edge. treat as all air instead of
                // crashing and burning
                debug!("slab is out of bounds, swapping in an empty one"; this_slab);

                // TODO shared instance of CoW for empty slab
                (Slab::empty_placeholder(), SlabVerticalSpace::empty())
            }
            Err(err) => {
                if let Err(e) = success_tx.send(Err(err)).await {
                    error!("failed to send failed terrain result"; "error" => %e);
                }
                return;
            }
        };

        // discover occlusion internal to this slab
        let occlusion = init_internal_occlusion(this_slab, &terrain, &vs);

        the_ultimate_load_task(LoadContext {
            world,
            this_slab,
            success_tx,
            extra: ExtraInfo::Generated {
                entities,
                terrain,
                vs,
                occlusion,
            },
        })
        .await
    }

    pub async fn update<C: WorldContext>(
        world: WorldRef<C>,
        this_slab: SlabLocation,
        success_tx: LoadSuccessTx,
    ) {
        the_ultimate_load_task(LoadContext {
            world,
            this_slab,

            success_tx,
            extra: ExtraInfo::Updated,
        })
        .await
    }

    /// Waits on above and below slabs if necessary. Returns new areas for this slab and the slab above's vs
    async fn discover_areas_with_vertical_neighbours<C: WorldContext>(
        slab: SlabLocation,
        vs: &SlabVerticalSpace,
        notifications: &mut ListeningLoadNotifier,
        terrain: &Slab<C>,
        world: &WorldRef<C>,
        bottom_areas: Option<Vec<SliceNavArea>>,
    ) -> (Vec<SliceNavArea>, Option<Arc<SlabVerticalSpace>>) {
        let needs_bottom = bottom_areas.is_none();
        let mut areas = bottom_areas.unwrap_or_default();

        // wait for slab above to be loaded if needed
        let needs_above = vs
            .above()
            .iter()
            .any(|h| *h != ABSOLUTE_MAX_FREE_VERTICAL_SPACE); // TODO calc once and cache

        let above = if needs_above {
            get_or_wait_for_slab_vertical_space(notifications, world, slab.above()).await
        } else {
            None
        };

        if needs_above {
            trace!(
                "{} waited for above {}: {}",
                slab,
                slab.above(),
                if above.is_some() { "present" } else { "absent" }
            );
        }

        // generate nav areas skipping the bottom slice
        terrain.discover_navmesh(&vs, above.as_ref(), &mut areas);

        if needs_bottom {
            // bottom slice nav requires below slab vertical space, now wait for that
            let below =
                get_or_wait_for_slab_vertical_space(notifications, &world, slab.below()).await;
            trace!(
                "{} waited for below {}: {}",
                slab,
                slab.below(),
                if below.is_some() { "present" } else { "absent" }
            );

            if let Some(below) = below.as_ref() {
                terrain.discover_bottom_slice_areas(&vs, below, &mut areas);
            }
        }

        // ensure consistent order
        areas.sort_unstable();
        debug_assert!(areas
            .iter()
            .tuple_windows()
            .all(|(a, b)| a.slice <= b.slice));

        (areas, above)
    }

    /// Puts graph into the slab and marks slab as DoneInIsolation. Returns prev neighbour hashes
    fn replace_slab_nav_graph<C: WorldContext>(
        world: &WorldRef<C>,
        slab: SlabLocation,
        graph: SlabNavGraph,
        areas: &[SliceNavArea],
    ) -> Option<[NeighbourAreaHash; 6]> {
        let mut w = world.borrow_mut();

        // temporary: just put all nodes and edges into world graph directly (TODO)
        w.nav_graph_mut().absorb(slab, &graph);

        if let Some(chunk) = w.find_chunk_with_pos_mut(slab.chunk) {
            // clear old slice areas
            chunk.replace_all_slice_areas(slab.slab, areas);

            let hashes = chunk.replace_slab_nav_graph(slab.slab, graph, areas);

            // let slab = w.get_slab_mut(slab)?;
            // TODO clear previous if indicated - but remember update to bottom slice is additive
            // slab.apply_navigation_updates(&update.new_areas, true);
            return hashes;
        }

        None
    }

    #[derive(Copy, Clone)]
    enum UpdateNeighbour {
        North,
        East,
        South,
        West,
        Above,
    }

    struct UpdateNeighbourContext<'a> {
        this_slab: SlabLocation,
        this_areas: &'a [SliceNavArea],
        this_border_areas: Vec<(SliceNavArea, SliceAreaIndex)>,
        current_hashes: [NeighbourAreaHash; 6],

        neighbour_border_areas: Vec<(SliceNavArea, SliceAreaIndex)>,
        new_world_edges: HashSet<(SlabArea, SlabArea, SlabNavEdge), RandomState>,

        notifications: ListeningLoadNotifier,
    }

    impl UpdateNeighbour {
        fn to_hash_index(self) -> SlabNeighbour {
            // TODO rearrange both enums for quick 1:1 mapping
            use SlabNeighbour as N;
            use UpdateNeighbour::*;
            match self {
                Above => N::Top,
                North => N::North,
                East => N::East,
                South => N::South,
                West => N::West,
            }
        }

        async fn apply<C: WorldContext>(
            self,
            ctx: &mut UpdateNeighbourContext<'_>,
            world: &WorldRef<C>,
        ) {
            let n_slab_loc = self.neighbour_slab(ctx);

            // collect border edges
            debug_assert!(ctx.this_border_areas.is_empty());
            let direction = self.direction();
            if let Some(dir) = direction {
                ctx.this_border_areas
                    .extend(navigationv2::filter_border_areas(
                        ctx.this_areas.iter().copied(),
                        dir,
                    ));
            } else {
                ctx.this_border_areas.extend(navigationv2::filter_top_areas(
                    ctx.this_areas.iter().copied(),
                ));
            }

            // hash and compare to previous
            let new_hash =
                NeighbourAreaHash::for_areas(ctx.this_border_areas.iter().map(|tup| tup.0));
            let cur_hash = ctx.current_hashes[self.to_hash_index() as usize];
            if new_hash == cur_hash {
                trace!(
                    "no change in {} areas between {} and {:?} neighbour (cur={:?}, new={:?})",
                    ctx.this_border_areas.len(),
                    ctx.this_slab,
                    self.to_hash_index(),
                    cur_hash,
                    new_hash
                );
                return;
            }

            if !ctx.this_border_areas.is_empty() {
                // wait for neighbour and collect from them too
                if let Some(dir) = direction {
                    let neighbour_dir = dir.opposite();
                    get_or_collect_slab_areas(
                        &mut ctx.notifications,
                        world,
                        n_slab_loc,
                        |a, ai| as_border_area(*a, ai, neighbour_dir),
                        &mut ctx.neighbour_border_areas,
                    )
                    .await;
                } else {
                    get_or_collect_slab_areas(
                        &mut ctx.notifications,
                        world,
                        n_slab_loc,
                        |area, info| {
                            (area.slab_area.slice_idx.slice() <= ABSOLUTE_MAX_FREE_VERTICAL_SPACE)
                                .then_some((
                                    SliceNavArea {
                                        slice: area.slab_area.slice_idx,
                                        from: info.range.0,
                                        to: info.range.1,
                                        height: info.height,
                                    },
                                    area.slab_area.slice_area,
                                ))
                        },
                        &mut ctx.neighbour_border_areas,
                    )
                    .await;
                }

                if !ctx.neighbour_border_areas.is_empty() {
                    navigationv2::discover_border_edges(
                        &ctx.this_border_areas,
                        &ctx.neighbour_border_areas,
                        direction,
                        |src, dst, edge| {
                            let e = (src, dst, edge);
                            // allow duplicates here for now
                            if !ctx.new_world_edges.insert(e) {
                                warn!("duplicate graph edge: {src} -> {dst} : {edge:?}")
                            }
                        },
                    );
                }
            }

            // add edges to world nav graph, replacing old ones
            {
                let mut w = world.borrow_mut();
                w.nav_graph_mut().add_inter_slab_edges(
                    ctx.this_slab,
                    n_slab_loc,
                    ctx.new_world_edges.drain(),
                );
            }
        }

        fn neighbour_slab(self, ctx: &UpdateNeighbourContext) -> SlabLocation {
            use UpdateNeighbour::*;
            ctx.this_slab.with_chunk_offset(match self.direction() {
                Some(dir) => dir.offset(),
                None => return ctx.this_slab.above(),
            })
        }

        fn direction(self) -> Option<NeighbourOffset> {
            use UpdateNeighbour::*;
            Some(match self {
                Above => return None,
                North => NeighbourOffset::North,
                East => NeighbourOffset::East,
                South => NeighbourOffset::South,
                West => NeighbourOffset::West,
            })
        }
    }

    fn init_internal_occlusion<C: WorldContext>(
        this_slab: SlabLocation,
        slab: &Slab<C>,
        vs: &SlabVerticalSpace,
    ) -> SparseGrid<BlockOcclusion> {
        let mut grid = SparseGrid::default();
        let mut grid_ext = grid.extend();

        #[inline]
        fn dew_it<'a, C: WorldContext>(
            this_slab: SlabLocation,
            grid_ext: &mut SparseGridExtension<BlockOcclusion>,
            blocks: impl Iterator<Item = SlabPosition>,
            do_top: bool,
            slices: impl Fn(
                LocalSliceIndex,
            ) -> (Option<Slice<'a, C>>, Slice<'a, C>, Option<Slice<'a, C>>),
        ) {
            for pos in blocks {
                let mut occlusion = BlockOcclusion::default();

                let (slice_below, slice_this, slice_above) = slices(pos.z());

                let mut update_info = OcclusionUpdateType::InitThisSlab {
                    slice_this,
                    slice_above,
                    slice_below,
                };

                if do_top {
                    let mut top_occlusion = NeighbourOpacity::default();
                    NeighbourOpacity::with_slice_above_other_slabs_possible(
                        pos,
                        &mut update_info,
                        |i, op| top_occlusion[i] = OcclusionOpacity::Known(op),
                    )
                    .now_or_never()
                    .expect("future should not await for internal occlusion");
                    occlusion.set_face(OcclusionFace::Top, top_occlusion);
                }

                // side faces
                for face in OcclusionFace::SIDE_FACES {
                    let mut neighbour_opacity = NeighbourOpacity::default();
                    NeighbourOpacity::with_neighbouring_slices_other_slabs_possible(
                        pos,
                        &mut update_info,
                        face,
                        |i, op| neighbour_opacity[i] = OcclusionOpacity::Known(op),
                    )
                    .now_or_never()
                    .expect("future should not await for internal occlusion");

                    occlusion.set_face(face, neighbour_opacity)
                }

                grid_ext.add_new(pos, occlusion);
            }
        }

        // use vertical space to skip all known air blocks
        dew_it(
            this_slab,
            &mut grid_ext,
            vs.iter_blocks().filter_map(|(air, _)| air.below()),
            true,
            |idx| {
                let this = slab.slice(idx);
                (slab.slice_below(this), this, slab.slice_above(this))
            },
        );

        // top slice is special case that needs to be done separately, there is no vertical space above it
        let top_slice = slab.slice(LocalSliceIndex::top());
        let slice_below_top = slab.slice_below(top_slice);
        dew_it(
            this_slab,
            &mut grid_ext,
            vs.iter_above()
                .filter_map(|(pos, h)| (h == 0).then_some(pos))
                .map(|slice_block| slice_block.to_slab_position(LocalSliceIndex::top())),
            false, // cant look above
            |s| {
                debug_assert_eq!(s, LocalSliceIndex::top());
                (slice_below_top, top_slice, None)
            },
        );

        drop(grid_ext);
        grid
    }

    async fn update_neighbour_occlusion<C: WorldContext>(
        vs: &SlabVerticalSpace,
        this_slab: SlabLocation,
        notifications: &mut ListeningLoadNotifier,
        world: &WorldRef<C>,
    ) {
        let mut changes = vec![];
        let mut update_neighbour_info = OcclusionUpdateType::UpdateFromNeighbours {
            relative_slabs: RelativeSlabs::new(this_slab, notifications, world),
        };

        use OcclusionFace::*;
        for pos in vs.iter_blocks().filter_map(|(air, _)| air.below()) {
            // because this block comes below an accessible air block in this slab, we know it
            // cannot be the top slice and so doesnt need the slab above
            debug_assert_ne!(pos.z(), LocalSliceIndex::top());

            // skip Top (idx 0)
            let mut side_faces = [false; OcclusionFace::ORDINALS.len() - 1];
            debug_assert_eq!(Top as usize, 0);

            let mut mark_needed = |face: OcclusionFace| {
                let idx = face as usize - 1; // skipping Top
                for i in [
                    (idx + OcclusionFace::SIDE_FACES.len() - 1) % OcclusionFace::SIDE_FACES.len(),
                    idx,
                    (idx + 1) % OcclusionFace::SIDE_FACES.len(),
                ] {
                    unsafe {
                        *side_faces.get_unchecked_mut(i) = true;
                    }
                }
            };

            if pos.y() == 0 {
                mark_needed(South)
            } else if pos.y() == CHUNK_SIZE.as_block_coord() - 1 {
                mark_needed(North)
            }
            if pos.x() == 0 {
                mark_needed(West);
            } else if pos.x() == CHUNK_SIZE.as_block_coord() - 1 {
                mark_needed(East);
            }

            let faces = side_faces
                .into_iter()
                .zip(OcclusionFace::SIDE_FACES.into_iter())
                .filter_map(|(b, f)| b.then_some(f));

            for face in faces {
                NeighbourOpacity::with_neighbouring_slices_other_slabs_possible(
                    pos,
                    &mut update_neighbour_info,
                    face,
                    |i, opacity| changes.push((pos, face, i as u8, opacity)),
                )
                .await;
            }

            NeighbourOpacity::with_slice_above_other_slabs_possible(
                pos,
                &mut update_neighbour_info,
                |i, opacity| changes.push((pos, Top, i as u8, opacity)),
            )
            .await;
        }

        // do top slice separately
        for pos in vs
            .iter_above()
            .filter_map(|(pos, h)| (h == 0).then_some(pos))
            .map(|slice_block| slice_block.to_slab_position(LocalSliceIndex::top()))
        {
            // needs all faces
            for face in OcclusionFace::SIDE_FACES {
                NeighbourOpacity::with_neighbouring_slices_other_slabs_possible(
                    pos,
                    &mut update_neighbour_info,
                    face,
                    |i, opacity| changes.push((pos, face, i as u8, opacity)),
                )
                .await;
            }

            NeighbourOpacity::with_slice_above_other_slabs_possible(
                pos,
                &mut update_neighbour_info,
                |i, opacity| changes.push((pos, Top, i as u8, opacity)),
            )
            .await;
        }

        // redo bottom slice too to now use slab below
        for pos in iter_slice_xy()
            .filter(|pos| vs.below_at(pos.xy()) == 0)
            .map(|b| b.to_slab_position(LocalSliceIndex::bottom()))
        {
            for face in OcclusionFace::SIDE_FACES {
                NeighbourOpacity::with_neighbouring_slices_other_slabs_possible(
                    pos,
                    &mut update_neighbour_info,
                    face,
                    |i, opacity| changes.push((pos, face, i as u8, opacity)),
                )
                .await;
            }
        }

        if !changes.is_empty() {
            let mut w = world.borrow_mut();
            let slab_data = w
                .find_chunk_with_pos_mut(this_slab.chunk)
                .and_then(|c| c.terrain_mut().slab_data_mut(this_slab.slab));

            if let Some(slab_data) = slab_data {
                let mut ext = slab_data.occlusion.extend();

                for (slab_pos, opacities) in &changes.into_iter().group_by(|tup| tup.0) {
                    let mut occlusion = ext.get_or_insert(slab_pos);

                    for (face, changes) in &opacities.group_by(|tup| tup.1) {
                        let mut face_occlusion = occlusion.get_face_mut(face);
                        for (_, _, i, op) in changes {
                            face_occlusion[i as usize] = OcclusionOpacity::Known(op);
                        }
                    }
                }
            }
        }
    }
}

impl<C: WorldContext> WorldLoader<C> {
    pub fn new<S: Into<TerrainSource<C>>>(source: S, mut pool: AsyncWorkerPool) -> Self {
        let world = WorldRef::default();

        Self {
            source: source.into(),
            pool,
            world,
            last_batch_size: 0,
        }
    }

    pub fn world(&self) -> WorldRef<C> {
        self.world.clone()
    }

    /// Requests slabs as a single batch. Must be sorted as per [self.request_slabs_with_count]
    pub fn request_slabs(&mut self, slabs: impl ExactSizeIterator<Item = SlabLocation> + Clone) {
        let count = slabs.len();
        self.request_slabs_with_count(slabs, count)
    }

    // TODO debug renderer to flicker chunks that are updated (nav,terrain,occlusion) on block change, to ensure not too much is changed

    // TODO add more efficient version that takes chunk+multiple slabs
    /// Must be sorted by chunk then by ascending slab (debug asserted). All slabs are loaded from
    /// scratch, it's the caller's responsibility to ensure slabs that are already loaded are not
    /// passed in here
    pub fn request_slabs_with_count(
        &mut self,
        slabs: impl Iterator<Item = SlabLocation> + Clone,
        count: usize,
    ) {
        // bomb out early if nothing to do
        if count == 0 {
            return;
        }

        let mut world_mut = self.world.borrow_mut();

        // check order of slabs is as expected
        if cfg!(debug_assertions) {
            let sorted = slabs
                .clone()
                .sorted_by(|a, b| a.chunk.cmp(&b.chunk).then_with(|| a.slab.cmp(&b.slab)));

            assert_equal(slabs.clone(), sorted);
        }

        let mut extra_slabs = SmallVec::<[SlabLocation; 16]>::new();

        // calculate chunk range
        let (mut chunk_min, mut chunk_max) = (ChunkLocation::MAX, ChunkLocation::MIN);

        // first iterate slabs to register them all with their chunk, so they know if their
        // neighbours have been requested/are being loaded too
        for (chunk_loc, slabs) in slabs.clone().group_by(|slab| slab.chunk).into_iter() {
            log_scope!(o!(chunk_loc));

            let chunk = world_mut.ensure_chunk(chunk_loc);

            // track the highest slab
            let mut highest = SlabIndex::MIN;

            for slab in slabs {
                chunk.mark_slab_requested(slab.slab);

                debug_assert!(slab.slab > highest, "slabs should be in ascending order");
                highest = slab.slab;
            }

            // should have seen some slabs
            assert_ne!(highest, SlabIndex::MIN);

            // request the slab above the highest as all-air if it's missing
            let empty = highest + 1;
            if !chunk.has_slab(empty) {
                extra_slabs.push(SlabLocation::new(empty, chunk.pos()));
                chunk.mark_slab_requested(empty);
            }

            // track chunk range
            chunk_min = chunk_min.min(chunk_loc);
            chunk_max = chunk_max.max(chunk_loc);
        }

        let count = count + extra_slabs.len();
        let mut real_count = 0;

        let all_slabs = {
            let real_slabs = slabs.zip(repeat(SlabType::Normal));
            let air_slabs = extra_slabs.into_iter().zip(repeat(SlabType::Placeholder));
            real_slabs.chain(air_slabs)
        };

        // let the terrain source know what's coming so it can kick off region generation
        {
            let source = self.source.clone();
            self.pool.submit_any_async_with_handle(async move {
                source.prepare_for_chunks((chunk_min, chunk_max)).await;
            });
        }

        for (slab, slab_type) in all_slabs {
            log_scope!(o!(slab));

            let source = self.source.clone();

            debug!(
                "requesting slab";
                slab,
            );

            // load raw terrain and do as much processing in isolation as possible on a worker thread
            let world = self.world();
            let mut terrain_tx = self.pool.success_tx();

            self.pool.submit_any_async_with_handle(load_task::generate(
                world,
                slab,
                slab_type,
                source.clone(),
                terrain_tx.clone(),
            ));

            real_count += 1;
        }

        debug!("slab batch of size {size} submitted", size = count);

        assert_eq!(
            real_count, count,
            "expected batch of {} but actually got {}",
            count, real_count
        );

        self.last_batch_size = count;
    }

    /// Note changes are made immediately to the terrain but are delayed for navigation.
    pub fn apply_terrain_updates(
        &mut self,
        terrain_updates: &mut HashSet<WorldTerrainUpdate<C>>,
        changes_out: &mut Vec<WorldChangeEvent>,
    ) {
        let world_ref = self.world.clone();

        let (slab_updates, upper_slab_limit) = {
            // translate world -> slab updates, preserving original mapping
            // TODO reuse vec allocs
            let mut slab_updates = terrain_updates
                .iter()
                .cloned()
                .flat_map(|world_update| {
                    world_update
                        .clone()
                        .into_slab_updates()
                        .map(move |update| (world_update.clone(), update))
                })
                .collect_vec();
            let mut slab_updates_to_keep = Vec::with_capacity(slab_updates.len());

            // sort then group by chunk and slab, so each slab is touched only once
            slab_updates.sort_unstable_by(|(_, (a, _)), (_, (b, _))| {
                a.chunk.cmp(&b.chunk).then_with(|| a.slab.cmp(&b.slab))
            });

            let world = world_ref.borrow();
            let mut chunks_iter = ContiguousChunkIterator::new(&*world);
            for (slab, updates) in &slab_updates.into_iter().group_by(|(_, (slab, _))| *slab) {
                enum UpdateApplication {
                    /// Pop updates from set and apply now
                    Apply,
                    /// Don't apply now, defer until another tick
                    Defer,
                    /// Don't apply ever, remove from set
                    Drop,
                }

                let application = match chunks_iter.next(slab.chunk) {
                    Some(chunk) => {
                        if chunk.is_slab_loaded(slab.slab) {
                            UpdateApplication::Apply
                        } else {
                            UpdateApplication::Defer
                        }
                    }
                    None => UpdateApplication::Drop,
                };

                match application {
                    UpdateApplication::Apply => {
                        // updates to be applied now
                        slab_updates_to_keep.extend(updates.into_iter().map(
                            |(original, update)| {
                                // remove from update set
                                terrain_updates.remove(&original);

                                // remove now unnecessary original mapping from update
                                update
                            },
                        ));
                    }

                    UpdateApplication::Defer => {
                        if cfg!(debug_assertions) {
                            let count = updates.count();
                            trace!("deferring {count} terrain updates for slab because it's currently loading", count = count; slab.chunk);
                        } else {
                            // avoid consuming expensive iterator when not logging
                            trace!("deferring terrain updates for slab because it's currently loading"; slab.chunk);
                        };
                    }
                    UpdateApplication::Drop => {
                        // remove from update set
                        let mut count = 0;
                        for (orig, _) in updates.dedup_by(|(a, _), (b, _)| *a == *b) {
                            terrain_updates.remove(&orig);
                            count += 1;
                        }

                        debug!("dropping {count} terrain updates for chunk because it's not loaded", count = count; slab.chunk);
                    }
                };
            }

            // count slabs for vec allocation, upper limit because some might be filtered out.
            // no allocations in dedup because vec is sorted
            let upper_slab_limit = slab_updates_to_keep
                .iter()
                .dedup_by(|(a, _), (b, _)| a == b)
                .count();

            (slab_updates_to_keep, upper_slab_limit)
        };

        if upper_slab_limit == 0 {
            // nothing to do
            return;
        }

        // group per slab so each slab is fetched and modified only once
        let grouped_updates = slab_updates.into_iter().group_by(|(slab, _)| *slab);
        let grouped_updates = grouped_updates
            .into_iter()
            .map(|(slab, updates)| (slab, updates.map(|(_, update)| update)));

        // modify slabs in place - even though the changes won't be fully visible in the game yet (in terms of
        // navigation or rendering), world queries in the next game tick will be current with the
        // changes applied now. the slabs loading state is returned to Requested
        // TODO reuse buf
        let mut slab_locs = Vec::with_capacity(upper_slab_limit);
        {
            let mut w = world_ref.borrow_mut();
            w.apply_terrain_updates_in_place(grouped_updates.into_iter(), |slab_loc| {
                slab_locs.push(slab_loc)
            });
        }

        let real_slab_count = slab_locs.len();
        debug!(
            "applied terrain updates to {count} slabs",
            count = real_slab_count
        );
        debug_assert_eq!(upper_slab_limit, slab_locs.capacity());

        for slab in slab_locs.into_iter() {
            let mut terrain_tx = self.pool.success_tx();
            self.pool.submit_any_async_with_handle(load_task::update(
                world_ref.clone(),
                slab,
                terrain_tx.clone(),
            ));
        }

        self.last_batch_size = real_slab_count;
    }

    pub fn block_on_next_finalization(
        &mut self,
        timeout: Duration,
        bail: &impl Fn() -> bool,
    ) -> Option<Result<SlabLocation, TerrainSourceError>> {
        let end_time = Instant::now() + timeout;
        loop {
            if bail() {
                break Some(Err(TerrainSourceError::Bailed));
            }

            let this_timeout = {
                let now = Instant::now();
                if now >= end_time {
                    break None; // finished
                }
                let left = end_time - now;
                left.min(Duration::from_secs(1))
            };

            if let ret @ Some(_) = self.pool.block_on_next_finalize(this_timeout) {
                break ret;
            }
        }
    }

    pub fn block_for_last_batch(&mut self, timeout: Duration) -> Result<(), BlockForAllError> {
        self.block_for_last_batch_with_bail(timeout, || false)
    }

    pub fn block_for_last_batch_with_bail(
        &mut self,
        timeout: Duration,
        bail: impl Fn() -> bool,
    ) -> Result<(), BlockForAllError> {
        match std::mem::take(&mut self.last_batch_size) {
            0 => Err(BlockForAllError::NoBatch),
            count => {
                let start_time = Instant::now();
                for i in 0..count {
                    let elapsed = start_time.elapsed();
                    let timeout = match timeout.checked_sub(elapsed) {
                        None => return Err(BlockForAllError::TimedOut),
                        Some(t) => t,
                    };

                    trace!("waiting for slab {index}/{total}", index = i + 1, total = count; "timeout" => ?timeout);
                    match self.block_on_next_finalization(timeout, &bail) {
                        None => return Err(BlockForAllError::TimedOut),
                        Some(Err(e)) => return Err(BlockForAllError::Error(e)),
                        Some(Ok(_)) => continue,
                    }
                }

                Ok(())
            }
        }
    }

    pub fn iter_occlusion_updates(&mut self, mut f: impl FnMut(OcclusionChunkUpdate)) {
        // TODO occlusion updates
        // while let Ok(Some(update)) = self.chunk_updates_rx.try_next() {
        //     f(update)
        // }
    }

    pub fn get_ground_level(
        &self,
        block: WorldPosition,
    ) -> Result<GlobalSliceIndex, TerrainSourceError> {
        let fut = self.source.get_ground_level(block);
        self.pool.runtime().block_on(fut)
    }

    #[cfg(feature = "worldprocgen")]
    pub fn query_block(&self, block: WorldPosition) -> Option<C::GeneratedBlockDetails> {
        let fut = self.source.query_block(block);
        self.pool.runtime().block_on(fut)
    }

    pub fn is_generated(&self) -> bool {
        #[cfg(feature = "worldprocgen")]
        return matches!(self.source, TerrainSource::Generated(_));

        #[cfg(not(feature = "worldprocgen"))]
        false
    }

    pub fn is_in_bounds(&self, slab: SlabLocation) -> bool {
        if let TerrainSource::Memory(src) = &self.source {
            src.read().is_in_bounds(slab)
        } else {
            true
        }
    }

    /// Nop if any mutexes cannot be taken immediately
    pub fn feature_boundaries_in_range(
        &self,
        chunks: &[ChunkLocation],
        z_range: (GlobalSliceIndex, GlobalSliceIndex),
        output: &mut Vec<(usize, WorldPosition)>,
    ) {
        let fut = self
            .source
            .feature_boundaries_in_range(chunks, z_range, output);
        let _ = fut.now_or_never();
    }

    pub fn steal_queued_block_updates(&self, out: &mut HashSet<WorldTerrainUpdate<C>>) {
        let fut = self.source.steal_queued_block_updates(out);
        self.pool.runtime().block_on(fut)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use unit::world::{
        all_slabs_in_range, ChunkLocation, SlabPosition, WorldPosition, WorldPositionRange,
        CHUNK_SIZE,
    };

    use crate::chunk::ChunkBuilder;
    use crate::helpers::{test_world_timeout, DummyBlockType};
    use crate::loader::loading::WorldLoader;
    use crate::loader::terrain_source::MemoryTerrainSource;
    use crate::loader::{AsyncWorkerPool, WorldTerrainUpdate};
    use crate::world::helpers::DummyWorldContext;
    use misc::{Itertools, Rng, SeedableRng, SliceRandom, SmallRng};
    use std::collections::{HashMap, HashSet};
    use unit::world::SlabLocation;

    #[test]
    fn thread_flow() {
        let a = ChunkBuilder::new()
            .set_block((0, 4, 60), DummyBlockType::Stone)
            .into_inner();

        let b = ChunkBuilder::new()
            .set_block((CHUNK_SIZE.as_i32() - 1, 4, 60), DummyBlockType::Grass)
            .into_inner();

        let source =
            MemoryTerrainSource::from_chunks(vec![((0, 0), a), ((-1, 0), b)].into_iter()).unwrap();

        let mut loader =
            WorldLoader::<DummyWorldContext>::new(source, AsyncWorkerPool::new_blocking().unwrap());
        loader.request_slabs(vec![SlabLocation::new(1, (0, 0))].into_iter());

        let finalized = loader.block_on_next_finalization(Duration::from_secs(15), &|| false);
        assert_eq!(finalized.unwrap().unwrap(), SlabLocation::new(1, (0, 0)));

        assert_eq!(loader.world.borrow().all_chunks().count(), 1);
    }

    #[test]
    #[ignore]
    /// Ensure block updates are applied as expected when stressed. Came out of debugging a race
    /// condition when applying terrain updates while a chunk is being finalized, but didn't actually
    /// help to reproduce it! Keeping it around as a regression test anyway
    fn block_updates_sanity_check() {
        const WORLD_SIZE: i32 = 8;
        const UPDATE_COUNT: usize = 1000;
        const BATCH_SIZE_RANGE: (usize, usize) = (5, 200);
        const Z_RANGE: i32 = 8;

        // misc::logging::for_tests();
        let source = {
            let chunks = (-WORLD_SIZE..WORLD_SIZE)
                .cartesian_product(-WORLD_SIZE..WORLD_SIZE)
                .map(|pos| (pos, ChunkBuilder::new().into_inner()));
            MemoryTerrainSource::from_chunks(chunks).unwrap()
        };

        let slabs_to_load = all_slabs_in_range(
            SlabLocation::new(-Z_RANGE, (-WORLD_SIZE, -WORLD_SIZE)),
            SlabLocation::new(Z_RANGE, (WORLD_SIZE, WORLD_SIZE)),
        )
        .0
        .collect_vec();

        let mut loader = WorldLoader::<DummyWorldContext>::new(
            source,
            AsyncWorkerPool::new(num_cpus::get()).unwrap(),
        );

        // create block updates before requesting slabs so there's no wait
        let mut rando = SmallRng::from_entropy();
        let blocks_to_set = {
            let block_types = vec![DummyBlockType::Stone, DummyBlockType::Dirt];

            // set each block once only
            (0..UPDATE_COUNT)
                .map(|_| {
                    const XY_RANGE: i32 = WORLD_SIZE * CHUNK_SIZE.as_i32();
                    let x = rando.gen_range(-XY_RANGE, XY_RANGE);
                    let y = rando.gen_range(-XY_RANGE, XY_RANGE);
                    let z = rando.gen_range(-Z_RANGE, Z_RANGE);
                    let pos = WorldPosition::from((x, y, z));
                    let block_type = block_types.choose(&mut rando).unwrap().to_owned();

                    (pos, block_type)
                })
                .collect::<HashMap<_, _>>()
        };
        // prepare update batches
        let mut update_batches = {
            let mut all = blocks_to_set
                .iter()
                .map(|(pos, block)| {
                    WorldTerrainUpdate::new(WorldPositionRange::with_single(*pos), *block)
                })
                .collect_vec();
            // could do this without so many allocs but it doesn't matter

            let mut batches = vec![];
            while !all.is_empty() {
                let (min, max) = BATCH_SIZE_RANGE;
                let n = rando.gen_range(min, max).min(all.len());
                let batch = all.drain(0..n).collect::<HashSet<_>>();
                batches.push(batch);
            }

            batches
        };
        let mut _changes = Vec::with_capacity(blocks_to_set.len());

        // load all slabs and wait for them to be present, otherwise the terrain updates are dropped
        loader.request_slabs(slabs_to_load.into_iter());
        assert!(
            loader.block_for_last_batch(test_world_timeout()).is_ok(),
            "timed out waiting for initial world finalization"
        );

        while !update_batches.is_empty() {
            let mut batch = update_batches.pop().unwrap(); // not empty
            let log_str = batch.iter().map(|x| format!("{:?}", x)).join("\n");
            // gross
            misc::trace!(
                "test: requesting batch of {} updates {}",
                batch.len(),
                log_str
            );
            loader.apply_terrain_updates(&mut batch, &mut _changes);
            if !batch.is_empty() {
                // push to back of "queue"
                update_batches.insert(0, batch);
            }
        }

        // wait for everything to settle down
        let timeout = test_world_timeout();
        loop {
            misc::info!(
                "test: waiting {:?} for world to settle down",
                timeout.min(Duration::from_secs(10))
            );
            let _ = loader.block_for_last_batch(timeout); // idk block longer
            if loader
                .block_on_next_finalization(timeout, &|| false)
                .is_none()
            {
                // timed out
                break;
            }

            // consume updates to keep memory down
            loader.iter_occlusion_updates(|_| {});
        }

        let world = loader.world();
        let world = world.borrow();

        // collect all non air blocks in the world
        let set_blocks = {
            let mut actual_blocks = vec![];
            let mut chunk_blocks = vec![];
            for chunk in world.all_chunks() {
                let blocks = chunk.terrain().blocks(&mut chunk_blocks);
                for (block_pos, block) in blocks.drain(..) {
                    let block_type = block.block_type();

                    if !matches!(block_type, DummyBlockType::Air) {
                        let world_pos = block_pos.to_world_position(chunk.pos());
                        actual_blocks.push((world_pos, block_type));
                    }
                }
            }

            actual_blocks
        };

        fn log_block(pos: WorldPosition) {
            let slab = SlabLocation::new(pos.slice(), ChunkLocation::from(pos));
            let block = SlabPosition::from(pos);
            eprintln!("test: btw {} is {:?} in slab {}", pos, block, slab);
        }

        // ensure the only non-air blocks are the ones we set
        for (pos, ty) in set_blocks {
            log_block(pos);
            match blocks_to_set.get(&pos) {
                None => panic!(
                    "unexpected block set at {:?}, should be air but is {:?}",
                    pos, ty
                ),
                Some(expected_ty) => {
                    assert_eq!(
                        *expected_ty, ty,
                        "block at {:?} should be {:?} but is actually {:?}",
                        pos, expected_ty, ty
                    );
                }
            }
        }

        // ensure all the blocks we set are non-air
        for (pos, expected_ty) in blocks_to_set.iter() {
            log_block(*pos);
            match world.block(*pos) {
                None => panic!("expected block {:?} does not exist", pos),
                Some(b) => {
                    let ty = b.block_type();
                    if let DummyBlockType::Air = ty {
                        panic!(
                            "block at {:?} is unset but should been set to {:?}",
                            pos, ty
                        );
                    } else {
                        assert_eq!(
                            *expected_ty, ty,
                            "block at {:?} should have been set to {:?} but is actually {:?}",
                            pos, expected_ty, ty
                        );
                    }
                }
            }
        }
    }
}
