use futures::FutureExt;
use std::cell::{Ref, RefCell};
use std::cmp::Ordering;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fmt::Debug;
use std::future::Future;
use std::hash::Hash;
use std::mem::{ManuallyDrop, MaybeUninit};
use std::ops::Deref;
use std::time::{Duration, Instant};

use misc::SliceRandom;
use misc::*;
use misc::{Rng, SmallVec};
use petgraph::algo::Measure;
use petgraph::stable_graph::*;
use petgraph::visit::{EdgeRef, IntoEdges, VisitMap};
use petgraph::visit::{NodeRef, Visitable};
use tokio::runtime::{Handle, Runtime};
use tokio::time::timeout;
use unit::world::{
    BlockPosition, ChunkLocation, LocalSliceIndex, SlabLocation, SliceIndex, WorldPoint,
    WorldPosition,
};

use crate::chunk::slab::SliceNavArea;
use crate::chunk::slice_navmesh::SliceAreaIndexAllocator;
use crate::chunk::{SlabAvailability, SlabLoadingStatus};
use crate::navigationv2::world_graph::SearchError::InvalidArea;
use crate::navigationv2::{ChunkArea, NavRequirement, SlabArea, SlabNavEdge, SlabNavGraph};
use crate::world::{SlabSliceDone, WaitResult};
use crate::{InnerWorldRef, World, WorldContext, WorldRef};

/// Area within the world
#[derive(Copy, Clone, Hash, Eq, PartialEq)]
pub struct WorldArea {
    pub chunk_idx: ChunkLocation,
    pub chunk_area: ChunkArea,
}

// TODO hierarchical, nodes should be slabs only
// TODO ensure undirected edges go a consistent direction e.g. src<dst, so edges can be reverserd consistently
type WorldNavGraphType = StableUnGraph<WorldArea, SlabNavEdge, u32>;

pub struct WorldGraph {
    graph: WorldNavGraphType,
    nodes: HashMap<WorldArea, NodeIndex>,
    pathfinding_runtime: Runtime,
}

impl Default for WorldGraph {
    fn default() -> Self {
        let threads = if cfg!(test) { 1 } else { 2 };
        let mut runtime_builder = tokio::runtime::Builder::new_multi_thread();
        runtime_builder
            .worker_threads(threads)
            .thread_name("navigation");

        if cfg!(test) {
            runtime_builder.enable_time(); // for timeout
        }

        Self {
            graph: Default::default(),
            nodes: Default::default(),
            pathfinding_runtime: runtime_builder
                .build()
                .expect("failed to create navigation runtime"),
        }
    }
}

impl WorldGraph {
    pub fn add_inter_slab_edges(
        &mut self,
        from: SlabLocation,
        to: SlabLocation,
        edges: impl Iterator<Item = (SlabArea, SlabArea, SlabNavEdge)>,
    ) {
        // remove old edges between these slabs
        let sorted = |a, b| if a < b { (a, b) } else { (b, a) };
        let (rm_a, rm_b) = sorted(from, to);
        self.graph.retain_edges(|g, e| {
            let (a, b) = g.edge_endpoints(e).unwrap();
            let a = g.node_weight(a).unwrap().slab();
            let b = g.node_weight(b).unwrap().slab();

            let (a, b) = sorted(a, b);
            !(a == rm_a && b == rm_b)
        });

        for (a, b, e) in edges {
            let a = WorldArea::from((from, a));
            let b = WorldArea::from((to, b));

            trace!(" interslab edge {a} -> {b} : {e:?}");
            let src = self.add_node(a);
            let dst = self.add_node(b);

            self.graph.add_edge(src, dst, e);
        }
    }

    pub fn pathfinding_runtime(&self) -> Handle {
        self.pathfinding_runtime.handle().clone()
    }

    pub fn iter_nodes(&self) -> impl Iterator<Item = WorldArea> + '_ {
        self.graph.node_weights().copied()
    }

    pub fn iter_edges(
        &self,
        node: WorldArea,
    ) -> impl Iterator<Item = (WorldArea, &SlabNavEdge)> + '_ {
        let idx = self.node(&node);
        self.graph.edges(idx).map(move |e| {
            (
                *self
                    .graph
                    .node_weight(if idx == e.source() {
                        e.target()
                    } else {
                        e.source()
                    })
                    .unwrap(),
                e.weight(),
            )
        })
    }

    fn node(&self, area: &WorldArea) -> NodeIndex {
        *self
            .nodes
            .get(area)
            .unwrap_or_else(|| panic!("no node for area {area}"))
    }

    // TODO actually use SlabNavGraph for hierarchical search
    pub fn absorb(&mut self, slab: SlabLocation, graph: &SlabNavGraph) {
        // remove all old from this slab
        let mut nodes_to_remove = HashSet::with_capacity(graph.graph.node_count());
        self.nodes.retain(|a, ni| {
            if a.slab() == slab {
                nodes_to_remove.insert(*ni);
                false
            } else {
                true
            }
        });
        self.graph
            .retain_nodes(|g, n| !nodes_to_remove.contains(&n));

        for slab_area in graph.iter_nodes() {
            let _ = self.add_node(WorldArea::from((slab, slab_area)));
        }
        for (src, dst, e) in graph.iter_edges() {
            let src = self.add_node(WorldArea::from((slab, src)));
            let dst = self.add_node(WorldArea::from((slab, dst)));
            self.graph.add_edge(src, dst, e.clone());
        }
    }

    fn add_node(&mut self, area: WorldArea) -> NodeIndex {
        *self.nodes.entry(area).or_insert_with(|| {
            debug_assert!(!self.graph.node_weights().contains(&area), "duplicate area");
            self.graph.add_node(area)
        })
    }

    pub fn iter_inter_slab_edges(
        &self,
    ) -> impl Iterator<Item = (WorldArea, WorldArea, SlabNavEdge)> + '_ {
        self.graph.edge_indices().filter_map(|e| {
            let (src, dst) = self.graph.edge_endpoints(e).unwrap();
            let src = self.graph.node_weight(src).unwrap();
            let dst = self.graph.node_weight(dst).unwrap();
            (src.slab() != dst.slab()).then(|| {
                let edge = self.graph.edge_weight(e).unwrap();
                (*src, *dst, *edge)
            })
        })
    }
}

impl From<(SlabLocation, SlabArea)> for WorldArea {
    fn from((slab, area): (SlabLocation, SlabArea)) -> Self {
        WorldArea {
            chunk_idx: slab.chunk,
            chunk_area: ChunkArea {
                slab_idx: slab.slab,
                slab_area: area,
            },
        }
    }
}

impl Display for WorldArea {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "WorldArea({}#{}:{})",
            self.slab(),
            self.chunk_area.slab_area.slice_idx,
            self.chunk_area.slab_area.slice_area.0
        )
    }
}

impl Debug for WorldArea {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

impl WorldArea {
    pub fn slab(&self) -> SlabLocation {
        SlabLocation {
            chunk: self.chunk_idx,
            slab: self.chunk_area.slab_idx,
        }
    }

    #[doc(hidden)]
    pub fn dummy_for_tests_only_srsly_dont_use_this() -> Self {
        Self {
            chunk_idx: ChunkLocation(0, 0),
            chunk_area: ChunkArea {
                slab_idx: Default::default(),
                slab_area: SlabArea {
                    slice_idx: LocalSliceIndex::bottom(),
                    slice_area: Default::default(),
                },
            },
        }
    }
}

#[derive(Debug, Clone, Error)]
pub enum SearchError {
    #[error("Source block {0} is not accessible for {1:?}")]
    SourceNotWalkable(WorldPosition, NavRequirement),

    #[error("Destination block {0} is not accessible for {1:?}")]
    DestinationNotWalkable(WorldPosition, NavRequirement),

    #[error("{0} in world graph is not in the graph")]
    InvalidArea(WorldArea),

    #[error("No path found")]
    NoPath,

    #[error("World changed during search, and max retries reached")]
    WorldChanged,

    #[error("Channel disconnected waiting for slabs to load")]
    WaitingForSlabLoading,
}

fn edge_cost(e: EdgeReference<SlabNavEdge>) -> f32 {
    // TODO edge cost
    /*
       if too high to step up: infnite cost
       drop of more than n: higher cost (but possible)
       shitty surface type: higher cost
       doors/gates etc: higher
    */
    1.0
}

fn heuristic<C: WorldContext>(n: NodeIndex, dst: WorldPosition, world: &World<C>) -> f32 {
    let area = world.nav_graph().graph.node_weight(n).unwrap();
    world
        .find_chunk_with_pos(area.chunk_idx)
        .and_then(|c| c.area_info(area.chunk_area.slab_idx, area.chunk_area.slab_area))
        .map(|ai| ai.centre_pos(*area))
        .map(|pos| pos.distance2(dst) as f32 * 1.4)
        .unwrap_or(f32::INFINITY)
}

struct SearchState {
    visited: <WorldNavGraphType as Visitable>::Map,
    visit_next: BinaryHeap<MinScored<f32, WorldArea>>,
    scores: HashMap<WorldArea, f32>,
    path_tracker: PathTracker<WorldArea, SlabNavEdge>,
}

/// [(area, edge to leave this area)]. Missing goal. Empty if already in goal area
type PathNodes = Vec<(WorldArea, SlabNavEdge)>;

pub struct Path {
    areas: PathNodes,
    source: WorldPoint,
    target: (WorldPoint, WorldArea),
}

impl Path {
    pub fn source(&self) -> WorldPoint {
        self.source
    }

    pub fn target_point(&self) -> WorldPoint {
        self.target.0
    }

    pub fn target(&self) -> WorldArea {
        self.target.1
    }

    pub fn iter_areas(&self) -> impl Iterator<Item = WorldArea> + '_ {
        self.areas
            .iter()
            .map(|(a, _)| *a)
            .chain(once(self.target.1))
    }

    pub fn area_count(&self) -> usize {
        self.areas.len()
    }

    pub fn route(&self) -> impl Iterator<Item = (WorldArea, SlabNavEdge)> + '_ {
        self.areas.iter().copied()
    }
}

pub enum SearchResult {
    Success(Path),
    Failed(SearchError),
    /// Wait on these slabs to load then try again
    WorldChanged(ArrayVec<SlabLocation, 4>),
}

pub struct SearchResultFuture {
    task: ManuallyDrop<tokio::task::JoinHandle<SearchResult>>,
    present: bool,
}

impl Drop for SearchResultFuture {
    fn drop(&mut self) {
        if let SearchResultFuture {
            task,
            present: true,
        } = self
        {
            unsafe {
                ManuallyDrop::drop(task);
            }
        }
    }
}

impl SearchResultFuture {
    pub fn cancel(&self) {
        assert!(self.present);
        self.task.abort();
    }

    pub fn take_future(&mut self) -> tokio::task::JoinHandle<SearchResult> {
        assert!(std::mem::replace(&mut self.present, false));
        unsafe { ManuallyDrop::take(&mut self.task) }
    }
}

impl SearchResult {
    fn into_result(self) -> Result<Path, SearchError> {
        match self {
            SearchResult::Success(path) => Ok(path),
            SearchResult::Failed(err) => Err(err),
            SearchResult::WorldChanged(_) => Err(SearchError::WorldChanged),
        }
    }
}

pub trait SearchEndpoint: Debug + Send + Copy {
    fn into_area<C: WorldContext>(
        self,
        world: &World<C>,
        req: NavRequirement,
    ) -> Result<WorldArea, WorldPosition>;

    fn as_point(&self) -> WorldPoint;
}

impl SearchEndpoint for (WorldPoint, WorldArea) {
    fn into_area<C: WorldContext>(
        self,
        _: &World<C>,
        _: NavRequirement,
    ) -> Result<WorldArea, WorldPosition> {
        Ok(self.1)
    }

    fn as_point(&self) -> WorldPoint {
        self.0
    }
}

impl SearchEndpoint for WorldPoint {
    fn into_area<C: WorldContext>(
        self,
        world: &World<C>,
        req: NavRequirement,
    ) -> Result<WorldArea, WorldPosition> {
        let pos = self.floor();
        world.find_area_for_block(pos, req).ok_or(pos)
    }

    fn as_point(&self) -> WorldPoint {
        *self
    }
}

trait EdgeRefExt {
    fn directional_height(&self, source: NodeIndex) -> i8;
}

impl<'a> EdgeRefExt for EdgeReference<'a, SlabNavEdge> {
    fn directional_height(&self, source: NodeIndex) -> i8 {
        if self.source() == source {
            self.weight().height_diff as i8
        } else {
            debug_assert_eq!(source, self.target());
            -(self.weight().height_diff as i8)
        }
    }
}

impl<C: WorldContext> World<C> {
    pub fn poll_path(&self, fut: &mut SearchResultFuture) -> Option<Result<Path, SearchError>> {
        assert!(fut.present);

        if !fut.task.is_finished() {
            return None;
        }

        let owned_fut = fut.take_future();

        Some(
            owned_fut
                .now_or_never()
                .expect("future is apparently finished")
                .expect("path finding panicked")
                .into_result(),
        )
    }

    pub fn find_path_async(
        self_: WorldRef<C>,
        from: impl SearchEndpoint + 'static,
        to: impl SearchEndpoint + 'static,
        requirement: NavRequirement,
    ) -> SearchResultFuture {
        let from_point = from.as_point();
        let to_point = to.as_point();

        let task = self_.nav_runtime().spawn(async move {
            let world_ref = self_.clone();
            const MAX_RETRIES: usize = 8;
            for retry in 0..MAX_RETRIES {
                trace!("path finding"; "attempt" => retry+1, "from" => ?from, "to" => ?to, "req" => ?requirement);
                let slabs_to_wait_for;
                let mut listener;
                {
                    let world = world_ref.borrow();

                    // start listening for load notifications now, so all loads during search are captured too
                    listener = world.load_notifications().start_listening();

                    slabs_to_wait_for = match world.find_abortable_path(from, to, requirement) {
                        Ok(Either::Left((path, dst))) => return SearchResult::Success(Path {
                            areas: path,
                            source: from_point,
                            target: (to_point, dst),
                        }),
                        Ok(Either::Right(loading_slabs)) => {
                            loading_slabs
                        }
                        Err(err) => return SearchResult::Failed(err),
                    };
                }

                debug_assert!(!slabs_to_wait_for.is_empty());
                match listener.wait_for_slabs(SlabSliceDone::new(&slabs_to_wait_for, SlabLoadingStatus::Done)).await {
                    WaitResult::Success | WaitResult::Retry => continue, // try again
                    WaitResult::Disconnected => {
                        return SearchResult::Failed(SearchError::WaitingForSlabLoading)
                    }
                }
            }

            SearchResult::Failed(SearchError::WorldChanged)
        });

        SearchResultFuture {
            task: ManuallyDrop::new(task),
            present: true,
        }
    }

    /// On success (Left=(path, target area), Right=[slabs to wait for])
    fn find_abortable_path(
        &self,
        from: impl SearchEndpoint,
        to: impl SearchEndpoint,
        requirement: NavRequirement,
    ) -> Result<Either<(PathNodes, WorldArea), SmallVec<[SlabLocation; 2]>>, SearchError> {
        let world_graph = self.nav_graph();

        let to_pos = to.as_point().floor();

        // resolve positions to areas
        let src = from
            .into_area(self, requirement)
            .map_err(|p| SearchError::SourceNotWalkable(p, requirement))?;
        let dst = to
            .into_area(self, requirement)
            .map_err(|p| SearchError::DestinationNotWalkable(p, requirement))?;

        trace!("path areas"; "src" => %src, "dst" => %dst);

        if src == dst {
            // empty path
            return Ok(Either::Left((PathNodes::new(), dst)));
        }

        let mut ctx =
            SearchContextInner::<_, EdgeIndex, _, <WorldNavGraphType as Visitable>::Map>::new(
                world_graph.graph.visit_map(),
            );
        let src_node = *world_graph.nodes.get(&src).ok_or(InvalidArea(src))?;
        let dst_node = *world_graph.nodes.get(&dst).ok_or(InvalidArea(dst))?;

        let estimate_cost = |n| heuristic(n, to_pos, self);
        let is_goal = |n| n == dst_node;
        let node_weight = |n| {
            let opt = world_graph.graph.node_weight(n);
            debug_assert!(opt.is_some(), "bad node {:?}", n);
            unsafe { *opt.unwrap_unchecked() }
        };
        let edge_weight = |e| {
            let opt = world_graph.graph.edge_weight(e);
            debug_assert!(opt.is_some(), "bad edge {:?}", e);
            unsafe { *opt.unwrap_unchecked() }
        };

        let latest_slab_version = |slab: SlabLocation| {
            self.find_chunk_with_pos(slab.chunk)
                .map(|c| c.slab_availability(slab.slab))
        };

        let start_time = Instant::now();

        ctx.scores.insert(src_node, 0.0);
        ctx.visit_next
            .push(MinScored(estimate_cost(src_node), src_node));
        while let Some(MinScored(_, node)) = ctx.visit_next.pop() {
            if is_goal(node) {
                let mut path = Vec::new();
                ctx.path_tracker.reconstruct_path_to(
                    node,
                    |n, e| {
                        let n = node_weight(n);
                        let e = edge_weight(e);
                        (n, e)
                    },
                    &mut path,
                );

                // ensure nodes from slabs havent changed since we visited them
                let changed_slabs = path
                    .iter()
                    .map(|(n, _)| n.slab())
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .filter(|s| match latest_slab_version(*s) {
                        Some(SlabAvailability::Present(t)) if t <= start_time => false,
                        _ => true,
                    })
                    .collect::<SmallVec<[_; 2]>>();
                return Ok(if !changed_slabs.is_empty() {
                    Either::Right(changed_slabs)
                } else {
                    Either::Left((path, dst))
                });
            }

            // Don't visit the same node several times, as the first time it was visited it was using
            // the shortest available path.
            if !ctx.visited.visit(node) {
                continue;
            }

            // This lookup can be unwrapped without fear of panic since the node was necessarily scored
            // before adding him to `visit_next`.
            let node_score = ctx.scores[&node];

            /*
               get all edges from this node in world graph, which can be an edge OR a placeholder for a loading slab
               if loading slab: await on it (but this adds new nodes to graph, so need to release reference somehow). then continue
            */

            let step_size = requirement.step_size as i8;
            let filtered_edges = world_graph
                .graph
                .edges(node)
                .filter(|e| e.directional_height(node) <= step_size);

            // iter edges to find if neighbouring slabs are loading/being modified, and abort if so
            let this_slab = node_weight(node).slab();
            let slabs = filtered_edges.clone().filter_map(|e| {
                let src_slab = node_weight(e.source()).slab();
                if src_slab != this_slab {
                    return Some(src_slab);
                }
                let dst_slab = node_weight(e.target()).slab();
                if dst_slab != this_slab {
                    return Some(dst_slab);
                }

                None
            });

            let changed_slabs = slabs
                .collect::<HashSet<_>>() // dedupe because parallel edges exist
                .into_iter()
                .filter(|slab| {
                    let chunk = match self.find_chunk_with_pos(slab.chunk) {
                        None => {
                            debug!("chunk {:?} has disappeared, aborting search", slab.chunk);
                            return true;
                        }
                        Some(c) => c,
                    };

                    match chunk.slab_availability(slab.slab) {
                        SlabAvailability::NotRequested => false,
                        SlabAvailability::InProgress => {
                            debug!("slab {:?} is in progress, aborting search", slab.slab);
                            true
                        }
                        SlabAvailability::Present(t) => start_time <= t,
                    }
                })
                .collect::<SmallVec<_>>();

            if !changed_slabs.is_empty() {
                return Ok(Either::Right(changed_slabs));
            }

            for edge in filtered_edges {
                let next = edge.target();
                if ctx.visited.is_visited(&next) {
                    continue;
                }

                let mut next_score = node_score + edge_cost(edge);
                match ctx.scores.entry(next) {
                    Occupied(ent) => {
                        let old_score = *ent.get();
                        if next_score < old_score {
                            *ent.into_mut() = next_score;
                            ctx.path_tracker.set_predecessor(next, node, edge.id());
                        } else {
                            next_score = old_score;
                        }
                    }
                    Vacant(ent) => {
                        ent.insert(next_score);
                        ctx.path_tracker.set_predecessor(next, node, edge.id());
                    }
                }

                let next_estimate_score = next_score + estimate_cost(next);
                ctx.visit_next.push(MinScored(next_estimate_score, next));
            }
        }

        Err(SearchError::NoPath)
    }

    #[cfg(test)]
    pub fn find_path_now(
        self_: WorldRef<C>,
        from: WorldPoint,
        to: WorldPoint,
        requirement: NavRequirement,
    ) -> Result<Path, SearchError> {
        let h = self_.nav_runtime();
        let mut fut = Self::find_path_async(self_, from, to, requirement);
        h.block_on(async { timeout(Duration::from_secs_f32(0.5), fut.take_future()).await })
            .expect("path finding timed out")
            .expect("path finding panicked")
            .into_result()
    }
}

/// Contains allocations to reuse
pub struct SearchContext<N, E, K, V>(RefCell<SearchContextInner<N, E, K, V>>)
where
    N: Eq + Hash + Copy,
    E: Copy,
    K: Measure + Copy,
    V: VisitMap<N>;

struct SearchContextInner<N, E, K, V>
where
    N: Eq + Hash + Copy,
    E: Copy,
    K: Measure + Copy,
    V: VisitMap<N>,
{
    visited: V,
    visit_next: BinaryHeap<MinScored<K, N>>,
    scores: HashMap<N, K>,
    path_tracker: PathTracker<N, E>,
}
pub struct PathTracker<N, E>
where
    N: Eq + Hash,
    E: Copy,
{
    came_from: HashMap<N, (N, E)>,
}

impl<N, E> PathTracker<N, E>
where
    N: Eq + Hash + Copy,
    E: Copy,
{
    fn new() -> Self {
        PathTracker {
            came_from: HashMap::new(),
        }
    }

    fn set_predecessor(&mut self, node: N, previous: N, edge: E) {
        self.came_from.insert(node, (previous, edge));
    }

    /// Path is (node, edge leaving it). Missing goal node
    fn reconstruct_path_to<RealNode, RealEdge, Resolve: Fn(N, E) -> (RealNode, RealEdge)>(
        &self,
        last: N,
        resolve: Resolve,
        path_out: &mut Vec<(RealNode, RealEdge)>,
    ) {
        path_out.reserve(self.came_from.len() / 2);

        let mut current = last;
        while let Some(&(previous, edge)) = self.came_from.get(&current) {
            current = previous;
            path_out.push((resolve(previous, edge)));
        }

        path_out.reverse();
    }
}

/// `MinScored<K, T>` holds a score `K` and a scored object `T` in
/// a pair for use with a `BinaryHeap`.
///
/// `MinScored` compares in reverse order by the score, so that we can
/// use `BinaryHeap` as a min-heap to extract the score-value pair with the
/// least score.
///
/// **Note:** `MinScored` implements a total order (`Ord`), so that it is
/// possible to use float types as scores.
#[derive(Copy, Clone, Debug)]
pub struct MinScored<K, T>(pub K, pub T);

impl<K: PartialOrd, T> PartialEq for MinScored<K, T> {
    #[inline]
    fn eq(&self, other: &MinScored<K, T>) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<K: PartialOrd, T> Eq for MinScored<K, T> {}

impl<K: PartialOrd, T> PartialOrd for MinScored<K, T> {
    #[inline]
    fn partial_cmp(&self, other: &MinScored<K, T>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[allow(clippy::eq_op)]
impl<K: PartialOrd, T> Ord for MinScored<K, T> {
    #[inline]
    fn cmp(&self, other: &MinScored<K, T>) -> Ordering {
        let a = &self.0;
        let b = &other.0;
        if a == b {
            Ordering::Equal
        } else if a < b {
            Ordering::Greater
        } else if a > b {
            Ordering::Less
        } else if a != a && b != b {
            // these are the NaN cases
            Ordering::Equal
        } else if a != a {
            // Order NaN less, so that it is last in the MinScore order
            Ordering::Less
        } else {
            Ordering::Greater
        }
    }
}

impl<N, E, K, V> SearchContext<N, E, K, V>
where
    N: Eq + Hash + Copy,
    E: Copy,
    K: Measure + Copy,
    V: VisitMap<N>,
{
    pub fn new<G: Visitable<Map = V> + Default>() -> Self {
        let graph = G::default();
        Self::new_with(&graph)
    }

    pub fn new_with(graph: impl Visitable<Map = V>) -> Self {
        Self(RefCell::new(SearchContextInner {
            visited: graph.visit_map(),
            visit_next: BinaryHeap::new(),
            scores: HashMap::new(),
            path_tracker: PathTracker::new(),
        }))
    }
}

impl<N, E, K, V> SearchContextInner<N, E, K, V>
where
    N: Eq + Hash + Copy,
    E: Copy,
    K: Measure + Copy,
    V: VisitMap<N>,
{
    // TODO eurgh constructor sucks
    fn new(visitmap: V) -> Self {
        Self {
            visited: visitmap,
            visit_next: Default::default(),
            scores: Default::default(),
            path_tracker: PathTracker {
                came_from: Default::default(),
            },
        }
    }

    fn reset_for(&mut self, graph: impl Visitable<Map = V>) {
        graph.reset_map(&mut self.visited);
        self.visit_next.clear();
        self.scores.clear();
        self.path_tracker.came_from.clear();
    }
}
