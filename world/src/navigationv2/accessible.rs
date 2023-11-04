use std::cell::RefCell;
use std::collections::VecDeque;

use ahash::HashSet;

use misc::{some_or_continue, vec2, Vec2};
use unit::world::{WorldPointRange, WorldPositionRange};

use crate::navigationv2::world_graph::WorldGraphNodeIndex;
use crate::navigationv2::WorldArea;
use crate::{NavRequirement, World, WorldContext};

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
    #[cfg(feature = "debug-accessibility")]
    dbg: RefCell<Option<debug_renderer::DebugRenderer>>,
}
impl AccessibilityCalculator {
    pub fn with_graph<C: WorldContext>(
        agent_bounds: &WorldPointRange,
        agent_req: NavRequirement,
        world: &World<C>,
        agent_area: WorldArea,
        filter_fn: impl Fn(WorldGraphNodeIndex) -> bool,
        save_debug_files: bool,
    ) -> Self {
        let graph = world.nav_graph();

        let agent_rect = Rect::from(agent_bounds);
        let start_node = graph.node(&agent_area);
        let mut areas_to_check = Vec::with_capacity(8);

        let mut visited = HashSet::default();
        let mut frontier = VecDeque::default();

        visited.insert(start_node);
        frontier.push_back((start_node, agent_area));

        // step down only, do not allow clipping into walls because it's possible to step up
        let edge_filter = |n, step| step <= 0 && filter_fn(n);
        while let Some((node, area)) = frontier.pop_front() {
            if !filter_fn(node) {
                continue;
            }

            let ai = some_or_continue!(world.lookup_area_info(area));
            let rect = Rect::from(ai.as_range(area));

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
            #[cfg(feature = "debug-accessibility")]
            dbg: RefCell::new(save_debug_files.then(|| debug_renderer::DebugRenderer::default())),
        };

        #[cfg(feature = "debug-accessibility")]
        this.dbg.borrow_mut().as_mut().map(|dbg| dbg.init(&this));

        this
    }

    fn process(&mut self) {
        let mut new_sub_rects = vec![]; // TODO reuse
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
}
