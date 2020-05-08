//! Copied from petgraph for modifications

use std::cmp::Ordering;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::{BinaryHeap, HashMap};
use std::hash::Hash;

use petgraph::algo::Measure;
use petgraph::visit::{EdgeRef, GraphBase, IntoEdges, VisitMap, Visitable};

/// Doesnt include goal node
pub fn astar<G, F, H, K, IsGoal>(
    graph: G,
    start: G::NodeId,
    mut is_goal: IsGoal,
    mut edge_cost: F,
    mut estimate_cost: H,
) -> Option<Vec<(G::NodeId, G::EdgeId)>>
where
    G: IntoEdges + Visitable,
    IsGoal: FnMut(G::NodeId) -> bool,
    G::NodeId: Eq + Hash,
    F: FnMut(G::EdgeRef) -> K,
    H: FnMut(G::NodeId) -> K,
    K: Measure + Copy,
{
    // TODO reuse allocations
    let mut visited = graph.visit_map();
    let mut visit_next = BinaryHeap::new();
    let mut scores = HashMap::new();
    let mut path_tracker = PathTracker::<G>::new();

    let zero_score = K::default();
    scores.insert(start, zero_score);
    visit_next.push(MinScored(estimate_cost(start), start));

    while let Some(MinScored(_, node)) = visit_next.pop() {
        if is_goal(node) {
            let path = path_tracker.reconstruct_path_to(node);
            // let cost = scores[&node];
            return Some(path);
        }

        // Don't visit the same node several times, as the first time it was visited it was using
        // the shortest available path.
        if !visited.visit(node) {
            continue;
        }

        // This lookup can be unwrapped without fear of panic since the node was necessarily scored
        // before adding him to `visit_next`.
        let node_score = scores[&node];

        for edge in graph.edges(node) {
            let next = edge.target();
            if visited.is_visited(&next) {
                continue;
            }

            let mut next_score = node_score + edge_cost(edge);

            match scores.entry(next) {
                Occupied(ent) => {
                    let old_score = *ent.get();
                    if next_score < old_score {
                        *ent.into_mut() = next_score;
                        path_tracker.set_predecessor(next, node, edge.id());
                    } else {
                        next_score = old_score;
                    }
                }
                Vacant(ent) => {
                    ent.insert(next_score);
                    path_tracker.set_predecessor(next, node, edge.id());
                }
            }

            let next_estimate_score = next_score + estimate_cost(next);
            visit_next.push(MinScored(next_estimate_score, next));
        }
    }

    None
}

struct PathTracker<G>
where
    G: GraphBase,
    G::NodeId: Eq + Hash,
{
    came_from: HashMap<G::NodeId, (G::NodeId, G::EdgeId)>,
}

impl<G> PathTracker<G>
where
    G: GraphBase,
    G::NodeId: Eq + Hash,
{
    fn new() -> PathTracker<G> {
        PathTracker {
            // TODO reuse allocation
            came_from: HashMap::new(),
        }
    }

    fn set_predecessor(&mut self, node: G::NodeId, previous: G::NodeId, edge: G::EdgeId) {
        self.came_from.insert(node, (previous, edge));
    }

    /// Returns (node, edge leaving it), missing the goal node
    fn reconstruct_path_to(&self, last: G::NodeId) -> Vec<(G::NodeId, G::EdgeId)> {
        // TODO reuse allocation
        let mut path = Vec::with_capacity(self.came_from.len() / 2);

        let mut current = last;
        while let Some(&(previous, edge)) = self.came_from.get(&current) {
            path.push((current, edge));
            current = previous;
        }

        // TODO this might be expensive, can we build up the vec in order
        path.reverse();

        path
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
