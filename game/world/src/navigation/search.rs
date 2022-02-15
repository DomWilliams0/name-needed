//! Based on petgraph

use std::cell::{Ref, RefCell};
use std::cmp::Ordering;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::{BinaryHeap, HashMap};
use std::fmt::Debug;
use std::hash::Hash;
use std::ops::Deref;

use petgraph::algo::Measure;
use petgraph::visit::{EdgeRef, IntoEdges, VisitMap, Visitable};

use common::Rng;
use common::{SliceRandom, SmallVec};

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
    result: Vec<(N, E)>,
}

/// Path is populated in context, left empty if search failed. On success, doesn't include goal node
pub fn astar<G, F, H, K, IsGoal>(
    graph: G,
    start: G::NodeId,
    mut is_goal: IsGoal,
    mut edge_cost: F,
    mut estimate_cost: H,
    context: &SearchContext<G::NodeId, G::EdgeId, K, G::Map>,
)
// TODO return nothing, just read from context
where
    G: IntoEdges + Visitable,
    IsGoal: FnMut(G::NodeId) -> bool,
    G::NodeId: Eq + Hash + Copy,
    F: FnMut(G::EdgeRef) -> K,
    H: FnMut(G::NodeId) -> K,
    K: Measure + Copy,
{
    let mut ctx = context.0.borrow_mut();
    ctx.reset_for(graph);

    let zero_score = K::default();
    ctx.scores.insert(start, zero_score);
    ctx.visit_next.push(MinScored(estimate_cost(start), start));

    while let Some(MinScored(_, node)) = ctx.visit_next.pop() {
        if is_goal(node) {
            {
                // safety: not referenced anywhere else
                let result = unsafe { &mut *(&mut ctx.result as *mut _) };
                ctx.path_tracker.reconstruct_path_to(node, result);
            }
            return; // success
        }

        // Don't visit the same node several times, as the first time it was visited it was using
        // the shortest available path.
        if !ctx.visited.visit(node) {
            continue;
        }

        // This lookup can be unwrapped without fear of panic since the node was necessarily scored
        // before adding him to `visit_next`.
        let node_score = ctx.scores[&node];

        for edge in graph.edges(node) {
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

    // leave result empty
    debug_assert!(ctx.result.is_empty())
}

/// Goes until no more neighours (edge of graph) or out of fuel. Path so far is in context.
/// Aborts if filter returns true
pub fn explore<G, K, R, F>(
    graph: G,
    start: G::NodeId,
    fuel: &mut u32,
    mut is_at_edge: impl FnMut(G::NodeId) -> bool,
    context: &SearchContext<G::NodeId, G::EdgeId, K, G::Map>,
    mut rand: R,
    filter: F,
) where
    G: IntoEdges + Visitable,
    G::NodeId: Eq + Hash + Copy + Debug,
    K: Measure + Copy,
    R: Rng,
    F: Fn(G::NodeId) -> bool,
{
    let mut ctx = context.0.borrow_mut();
    ctx.reset_for(graph);

    let mut current = start;
    while *fuel > 0 {
        debug_assert!(!ctx.visited.is_visited(&current));
        ctx.visited.visit(current);

        let mut edges = graph
            .edges(current)
            .map(|e| {
                let weight = if !ctx.visited.is_visited(&e.target()) {
                    1
                } else {
                    0
                };
                ((e.target(), e.id()), weight)
            })
            .collect::<SmallVec<[_; 4]>>();

        // straight ahead is bit more likely
        if edges.len() == 4 {
            edges[2].1 *= 3;
        }

        let (next, edge) = match edges.choose_weighted(&mut rand, |(_, w)| *w) {
            Ok((step, _)) => *step,
            Err(_) => {
                // no neighbours, nvm
                break;
            }
        };

        // add to path
        ctx.result.push((next, edge));

        current = next;
        *fuel -= 1;

        // have a chance to terminate at the edge
        if is_at_edge(current) && rand.gen_bool(0.25) {
            break;
        }

        if filter(current) {
            break;
        }
    }
}

struct PathTracker<N, E>
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

    /// Returns (node, edge leaving it), missing the goal node
    fn reconstruct_path_to(&self, last: N, path_out: &mut Vec<(N, E)>) {
        path_out.clear();
        path_out.reserve(self.came_from.len() / 2);

        let mut current = last;
        while let Some(&(previous, edge)) = self.came_from.get(&current) {
            path_out.push((current, edge));
            current = previous;
        }

        // TODO this might be expensive, can we build up the vec in order
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
struct MinScored<K, T>(pub K, pub T);

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
            result: Vec::new(),
        }))
    }

    pub fn result(&self) -> impl Deref<Target = [(N, E)]> + '_ {
        Ref::map(self.0.borrow(), |inner| &inner.result[..])
    }
}

impl<N, E, K, V> SearchContextInner<N, E, K, V>
where
    N: Eq + Hash + Copy,
    E: Copy,
    K: Measure + Copy,
    V: VisitMap<N>,
{
    fn reset_for(&mut self, graph: impl Visitable<Map = V>) {
        graph.reset_map(&mut self.visited);
        self.visit_next.clear();
        self.scores.clear();
        self.path_tracker.came_from.clear();
        self.result.clear();
    }
}
