use std::cell::RefCell;
use std::collections::VecDeque;

use ahash::HashSet;

use misc::glam::ivec2;
use misc::{all, debug, some_or_continue, vec2, warn, Vec2};
use unit::world::{WorldPoint, WorldPointRange, WorldPositionRange};

use crate::navigationv2::world_graph::{find_common_edge, WorldGraphNodeIndex};
use crate::navigationv2::WorldArea;
use crate::{NavRequirement, SearchDebug, World, WorldContext};

pub fn does_entity_fit_in_area<C: WorldContext>(
    agent_bounds: &WorldPointRange,
    agent_req: NavRequirement,
    world: &World<C>,
    agent_area: WorldArea,
    save_debug_files: bool,
) -> bool {
    // step down only, do not allow clipping into walls because it's possible to step up
    let allow_step_up = false;
    AccessibilityCalculator::with_graph(
        agent_bounds,
        agent_req,
        world,
        agent_area,
        save_debug_files,
        allow_step_up,
    )
    .process_fully_and_check()
}

#[cfg_attr(feature = "debug-accessibility", derive(serde::Serialize))]
#[derive(Clone)]
struct Rect {
    min: Vec2,
    /// Inclusive
    max: Vec2,
}
pub struct AccessibilityCalculator {
    agent_remaining: Vec<Rect>,
    areas_to_check: Vec<Rect>,
    double_buf: Vec<Rect>,
    #[cfg(feature = "debug-accessibility")]
    dbg: RefCell<Option<debug_renderer::DebugRenderer>>,
}

pub struct MultiAccessibilityCalculators(Vec<AccessibilityCalculator>);

impl MultiAccessibilityCalculators {
    pub fn process_fully_and_check(&mut self, mut debug: Option<&mut SearchDebug>) -> bool {
        !self.0.is_empty()
            && self.0.iter_mut().enumerate().any(|(i, calc)| {
                let res = calc.process_fully_and_check();
                debug
                    .as_deref_mut()
                    .and_then(|d| d.checked_accessibility_ranges.get_mut(i))
                    .map(|(_, res_mut)| *res_mut = Some(res));
                res
            })
    }
}

fn area_range<C: WorldContext>(area: WorldArea, world: &World<C>) -> Option<WorldPositionRange> {
    let ai = world.lookup_area_info(area)?;
    Some(ai.as_range(area))
}

impl AccessibilityCalculator {
    pub fn on_boundary_between_areas<C: WorldContext>(
        src_area: WorldArea,
        dst_area: WorldArea,
        agent_req: NavRequirement,
        world: &World<C>,
        save_debug_files: bool,
        mut debug: Option<&mut SearchDebug>,
    ) -> MultiAccessibilityCalculators {
        // TODO pass in precalculated src bounds
        let (src_bounds, dst_bounds) =
            match area_range(src_area, world).zip(area_range(dst_area, world)) {
                Some(tup) => tup,
                None => {
                    warn!("invalid areas src={src_area} dst={dst_area}");
                    return MultiAccessibilityCalculators(vec![]);
                }
            };

        let (edge_start, edge_vec) = match find_common_edge(&src_bounds, &dst_bounds) {
            Some(e) => e,
            None => {
                warn!("no common area between src={src_area} dst={dst_area}");
                return MultiAccessibilityCalculators(vec![]);
            }
        };

        if let Some(debug) = debug.as_deref_mut() {
            debug.common_edges.push((
                (edge_start.0, edge_start.1, src_bounds.min().2).into(),
                ivec2(edge_vec.0 as i32, edge_vec.1 as i32),
            ));
        }

        // TODO allow edge of map in dev builds?

        let ret = MultiAccessibilityCalculators(
            step_along_edge(
                edge_start,
                edge_vec,
                (agent_req.dims.0.ceil() as u8, agent_req.dims.1.ceil() as u8),
            )
            .map(|agent_centre| {
                // z is not used
                let bounds = agent_req.max_rotated_aabb(
                    WorldPoint::try_from(agent_centre.extend(0.0))
                        .expect("invalid point along edge?"),
                );

                if let Some(debug) = debug.as_deref_mut() {
                    debug
                        .checked_accessibility_ranges
                        .push((bounds.clone(), None));
                }

                AccessibilityCalculator::with_graph(
                    &bounds,
                    agent_req,
                    world,
                    src_area,
                    save_debug_files,
                    true, // allow step up for search
                )
            })
            .collect(),
        );
        ret
    }

    pub fn with_graph<C: WorldContext>(
        agent_bounds: &WorldPointRange,
        agent_req: NavRequirement,
        world: &World<C>,
        agent_area: WorldArea,
        save_debug_files: bool,
        allow_step_up: bool,
    ) -> Self {
        let graph = world.nav_graph();

        let agent_rect = Rect::from(agent_bounds);
        let start_node = graph.node(&agent_area);
        let mut areas_to_check = Vec::with_capacity(8);

        let mut visited = HashSet::default();
        let mut frontier = VecDeque::default();

        visited.insert(start_node);
        frontier.push_back((start_node, agent_area));

        let max_step = if allow_step_up {
            agent_req.step_size as i8
        } else {
            0
        };
        let edge_filter = |n, step| step <= max_step;
        while let Some((node, area)) = frontier.pop_front() {
            let ai = some_or_continue!(world.lookup_area_info(area));
            let rect = Rect::from(ai.as_range(area)).expanded(0.01);

            if !rect.intersects(&agent_rect) {
                continue;
            }

            areas_to_check.push(rect);

            for (other_area, edge) in
                world
                    .nav_graph()
                    .iter_accessible_edges(node, agent_req, &edge_filter)
            {
                if visited.insert(edge.other_node()) {
                    frontier.push_back((edge.other_node(), other_area))
                }
            }
        }

        let mut this = Self {
            agent_remaining: vec![agent_rect],
            areas_to_check,
            double_buf: Default::default(),
            #[cfg(feature = "debug-accessibility")]
            dbg: RefCell::new(save_debug_files.then(|| debug_renderer::DebugRenderer::default())),
        };

        #[cfg(feature = "debug-accessibility")]
        this.dbg.borrow_mut().as_mut().map(|dbg| dbg.init(&this));

        this
    }

    fn process(&mut self) {
        let mut new_sub_rects = std::mem::take(&mut self.double_buf);
        debug_assert!(new_sub_rects.is_empty());
        while let Some(b) = self.areas_to_check.pop() {
            self.agent_remaining.retain_mut(|a| {
                if a.is_fully_covered_by(&b) {
                    false
                } else {
                    let len_before = new_sub_rects.len();
                    a.subtract(&b, &mut new_sub_rects);
                    // nothing added if no intersection, so then dont remove?
                    len_before == new_sub_rects.len()
                }
            });
            self.agent_remaining.extend(new_sub_rects.drain(..));

            #[cfg(feature = "debug-accessibility")]
            self.dbg
                .borrow_mut()
                .as_mut()
                .map(|dbg| dbg.add_frame(self));
        }

        #[cfg(feature = "debug-accessibility")]
        self.dbg.borrow_mut().as_mut().map(|dbg| dbg.finish(self));

        let empty = std::mem::replace(&mut self.double_buf, new_sub_rects);
        debug_assert!(empty.is_empty());
        std::mem::forget(empty);
    }

    fn was_it_fine(&self) -> bool {
        self.agent_remaining.is_empty()
    }

    pub fn process_fully_and_check(&mut self) -> bool {
        self.process();
        self.was_it_fine()
    }
}

#[cfg(feature = "debug-accessibility")]
mod debug_renderer {
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering::Relaxed;

    use serde::Serialize;

    use misc::debug;

    use super::*;

    #[derive(Default)]
    pub struct DebugRenderer {
        frames: Vec<Frame>,
        id: String,
    }

    #[derive(Serialize)]
    struct Frame {
        agent_rects: Vec<Rect>,
        to_check: Vec<Rect>,
    }

    impl DebugRenderer {
        pub fn init(&mut self, calc: &AccessibilityCalculator) {
            self.frames.clear();
            self.frames.push(calc.as_frame());

            static COUNTER: AtomicUsize = AtomicUsize::new(1);
            let n = COUNTER.fetch_add(1, Relaxed);
            self.id = format!("{}-{n}", std::process::id());
        }

        pub fn add_frame(&mut self, calc: &AccessibilityCalculator) {
            self.frames.push(calc.as_frame());
        }

        pub fn finish(&mut self, calc: &AccessibilityCalculator) {
            assert!(!self.id.is_empty());
            self.frames.push(calc.as_frame());
            let json = serde_json::to_string(&self.frames).unwrap();
            let path = format!("/tmp/debug-accessibility-{}.json", self.id);
            debug!("writing {} debug frames to {}", self.frames.len(), path);
            std::fs::write(path, json).expect("writing debug file");
        }
    }

    impl AccessibilityCalculator {
        fn as_frame(&self) -> Frame {
            Frame {
                agent_rects: self.agent_remaining.clone(),
                to_check: self.areas_to_check.clone(),
            }
        }
    }
}

/*
placement check
    check entity bounds positioned at given worldpoint, query world graph for connections
    ensure world rects fully overlap the bounds

search check
    same check but between 2 areas.
    first: halfway point between current search pos (centre of area) and next potential edge->area
    second: in centre of next potential edge->area
 */

impl From<&WorldPointRange> for Rect {
    fn from(range: &WorldPointRange) -> Self {
        let (min, max) = range.bounds();
        Rect {
            min: vec2(min.x(), min.y()),
            max: vec2(max.x() + 1.0, max.y() + 1.0),
        }
    }
}

impl From<&WorldPositionRange> for Rect {
    fn from(range: &WorldPositionRange) -> Self {
        let (min, max) = range.bounds();
        Rect {
            min: vec2(min.0 as f32, min.1 as f32),
            max: vec2((max.0 + 1) as f32, (max.1 + 1) as f32),
        }
    }
}

impl From<WorldPositionRange> for Rect {
    fn from(range: WorldPositionRange) -> Self {
        (&range).into()
    }
}

impl Rect {
    fn is_fully_covered_by(&self, subrect: &Rect) -> bool {
        subrect.min.x <= self.min.x
            && subrect.min.y <= self.min.y
            && subrect.max.x >= self.max.x
            && subrect.max.y >= self.max.y
    }

    fn intersects(&self, other: &Rect) -> bool {
        self.min.x < other.max.x
            && self.max.x > other.min.x
            && self.min.y < other.max.y
            && self.max.y > other.min.y
    }

    fn subtract(&self, other: &Rect, result: &mut Vec<Rect>) {
        // Calculate the intersection of the two rectangles
        let x1 = self.min.x.max(other.min.x);
        let x2 = self.max.x.min(other.max.x);
        let y1 = self.min.y.max(other.min.y);
        let y2 = self.max.y.min(other.max.y);

        // Check if there is an intersection
        if x1 < x2 && y1 < y2 {
            // Left rectangle
            if self.min.x < x1 {
                result.push(Rect {
                    min: vec2(self.min.x, self.min.y),
                    max: vec2(x1, self.max.y),
                });
            }

            // Right rectangle
            if self.max.x > x2 {
                result.push(Rect {
                    min: vec2(x2, self.min.y),
                    max: vec2(self.max.x, self.max.y),
                });
            }

            // Top rectangle
            if self.min.y < y1 {
                result.push(Rect {
                    min: vec2(x1, self.min.y),
                    max: vec2(x2, y1),
                });
            }

            // Bottom rectangle
            if self.max.y > y2 {
                result.push(Rect {
                    min: vec2(x1, y2),
                    max: vec2(x2, self.max.y),
                });
            }
        }
    }

    fn expanded(mut self, by: f32) -> Self {
        let delta = vec2(by, by);
        Self {
            min: self.min - delta,
            max: self.max + delta,
        }
    }
}

/// Yields centre pos of each non overlapping position along the line (except the last)
fn step_along_edge(
    edge_start: (i32, i32),
    edge_vector: (u8, u8),
    dims_blocks: (u8, u8),
) -> impl Iterator<Item = Vec2> {
    debug_assert!(edge_vector.0 == 0 || edge_vector.1 == 0);

    let dims = vec2(dims_blocks.0 as f32, dims_blocks.1 as f32);
    let step = if edge_vector.0 == 0 {
        vec2(0.0, dims.y)
    } else {
        vec2(dims.x, 0.0)
    };

    let start = vec2(edge_start.0 as f32, edge_start.1 as f32);

    let end = vec2(
        (edge_start.0 + edge_vector.0 as i32) as f32,
        (edge_start.1 + edge_vector.1 as i32) as f32,
    );

    let last = {
        let (dim, edge_len) = if edge_vector.0 == 0 {
            (dims_blocks.1, edge_vector.1)
        } else {
            (dims_blocks.0, edge_vector.0)
        };

        (edge_len % dim != 0).then(|| {
            start
                + if edge_vector.0 == 0 {
                    vec2(dims.x * 0.5, edge_len as f32 - dims.y * 0.5)
                } else {
                    vec2(edge_len as f32 - dims.x * 0.5, dims.y * 0.5)
                }
        })
    };

    std::iter::successors(Some(start + (dims * 0.5)), move |p| Some(*p + step))
        .take_while(move |p| {
            if edge_vector.0 == 0 {
                p.y <= end.y
            } else {
                p.x <= end.x
            }
        })
        .chain(last.into_iter())
}

#[cfg(test)]
mod tests {
    use super::*;
    use misc::Itertools;

    #[test]
    fn step_along_hor() {
        let edge_start = (10, 50);
        let edge_delta = (15, 0);
        let dims_blocks = (3, 4);

        assert_eq!(
            step_along_edge(edge_start, edge_delta, dims_blocks).collect_vec(),
            vec![
                vec2(11.5, 52.0),
                vec2(14.5, 52.0),
                vec2(17.5, 52.0),
                vec2(20.5, 52.0),
                vec2(23.5, 52.0),
            ]
        );
    }

    #[test]
    fn step_along_ver() {
        let edge_start = (10, 50);
        let edge_delta = (0, 15);
        let dims_blocks = (3, 4);

        assert_eq!(
            step_along_edge(edge_start, edge_delta, dims_blocks).collect_vec(),
            vec![
                vec2(11.5, 52.0),
                vec2(11.5, 56.0),
                vec2(11.5, 60.0),
                vec2(11.5, 64.0),
                vec2(11.5, 63.0),
            ]
        );
    }

    #[test]
    fn step_along_ver_short() {
        let edge_start = (0, 0);
        let edge_delta = (0, 5);
        let dims_blocks = (3, 4);

        assert_eq!(
            step_along_edge(edge_start, edge_delta, dims_blocks).collect_vec(),
            vec![vec2(1.5, 2.0), vec2(1.5, 3.0),]
        );
    }

    #[test]
    fn step_along_ver_short_exact() {
        let edge_start = (0, 0);
        let edge_delta = (0, 8);
        let dims_blocks = (3, 4);

        assert_eq!(
            step_along_edge(edge_start, edge_delta, dims_blocks).collect_vec(),
            vec![vec2(1.5, 2.0), vec2(1.5, 6.0),]
        );
    }
}
