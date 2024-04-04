use misc::glam::DVec3;
use misc::*;
use unit::space::view::ViewPoint;
use unit::world::{ChunkLocation, GlobalSliceIndex, SliceIndex, WorldPoint, WorldPosition};

use crate::world::ContiguousChunkIterator;
use crate::{BlockType, InnerWorldRef, OcclusionFace, SliceRange, WorldContext};

#[derive(Debug, Clone)]
pub struct VoxelRay {
    pos: ViewPoint,
    dir: DVec3,
}

pub struct VoxelRayOutput {
    pub ray: VoxelRay,
    pub points: Vec<WorldPoint>,
    pub blocks: Vec<(WorldPosition, bool)>,
    hit_block: Option<WorldPosition>,
}

impl VoxelRayOutput {
    fn new(ray: &VoxelRay) -> Self {
        Self {
            ray: ray.clone(),
            points: vec![],
            blocks: vec![],
            hit_block: None,
        }
    }
    fn on_point(&mut self, p: WorldPoint) {
        self.points.push(p);
    }

    fn on_block(&mut self, p: WorldPosition, accepted: bool) {
        self.blocks.push((p, accepted));
    }

    pub fn result(&self) -> Option<WorldPosition> {
        self.hit_block
    }
}

impl VoxelRay {
    pub fn new(pos: ViewPoint, dir: DVec3) -> Self {
        Self {
            pos,
            dir: dir.normalize(),
        }
    }

    pub fn origin(&self) -> ViewPoint {
        self.pos
    }

    pub fn direction(&self) -> DVec3 {
        self.dir
    }

    // TODO if found block is fully occluded, go upwards/some direction to find better candidate
    pub fn find_first_visible<C: WorldContext>(
        &self,
        world: &InnerWorldRef<C>,
        range: SliceRange,
    ) -> VoxelRayOutput {
        let mut output = VoxelRayOutput::new(self);
        let res = self.dew_it(world, |pos| range.contains(pos.slice()), &mut output);
        output.hit_block = res;
        output
    }

    fn dew_it<C: WorldContext>(
        &self,
        world: &InnerWorldRef<C>,
        mut filter: impl FnMut(WorldPosition) -> bool,
        output: &mut VoxelRayOutput,
    ) -> Option<WorldPosition> {
        if self.dir.length_squared() < 0.9 {
            misc::warn!("invalid raycast direction {:?}", self.dir);
            return None;
        }

        // TODO skip ahead over unloaded chunks
        let mut step = self.dir.to_array();
        step.iter_mut().for_each(|c| *c /= 10.0);

        // https://gamedev.stackexchange.com/a/49423
        let range = 800.0;
        let cam_pos = WorldPoint::from(self.pos);
        let mut pos = dvec3(cam_pos.x() as f64, cam_pos.y() as f64, cam_pos.z() as f64);
        let dir = self.dir;
        // TODO this is not very accurate
        let mut t_max = dvec3(
            intbound(pos.x, dir.x),
            intbound(pos.y, dir.y),
            intbound(pos.z, dir.z),
        );

        let t_delta = dvec3(step[0] / dir.x, step[1] / dir.y, step[2] / dir.z);
        let mut face = OcclusionFace::Top;

        let mut last_block = WorldPosition::new(i32::MIN, i32::MIN, GlobalSliceIndex::bottom());
        let mut has_seen_a_block = false;
        let mut chunk_iter = ContiguousChunkIterator::new(world);
        loop {
            let point = WorldPoint::new_unchecked(pos.x as f32, pos.y as f32, pos.z as f32);
            output.on_point(point);

            let block_pos = point.floor();
            let same_as_last = last_block == block_pos;
            let filtered = same_as_last || {
                let filtered = filter(block_pos);
                output.on_block(block_pos, filtered);
                filtered
            };

            if filtered {
                // TODO filter out invisible here
                if block_pos != last_block {
                    let block = chunk_iter
                        .next(ChunkLocation::from(block_pos))
                        .and_then(|chunk| chunk.terrain().get_block(block_pos.into()));

                    if let Some(b) = block {
                        has_seen_a_block = true;

                        if !b.block_type().is_air() {
                            // found a solid block
                            // TODO the exact point is on the given face of a block. inverse project to get the real block from that instead of flooring which is wrong
                            return Some(block_pos);
                        }
                    } else if has_seen_a_block {
                        // we have visited some blocks but passed through the whole world, abort
                        break;
                    }

                    last_block = block_pos;
                }
            }

            face = if t_max[0] < t_max[1] {
                if t_max[0] < t_max[2] {
                    if t_max[0] > range {
                        break;
                    }

                    pos.x += step[0];
                    t_max[0] += t_delta[0];

                    // (-step[0], 0.0, 0.0)
                    if step[0].is_sign_positive() {
                        OcclusionFace::West
                    } else {
                        OcclusionFace::East
                    }
                } else {
                    if t_max[2] > range {
                        break;
                    }
                    pos.z += step[2];
                    t_max[2] += t_delta[2];
                    // (0.0, 0.0, -step[2])
                    OcclusionFace::Top
                }
            } else {
                if t_max[1] < t_max[2] {
                    if t_max[1] > range {
                        break;
                    }
                    pos.y += step[1];
                    t_max[1] += t_delta[1];

                    // (0.0, -step[1], 0.0)
                    if step[1].is_sign_positive() {
                        OcclusionFace::South
                    } else {
                        OcclusionFace::North
                    }
                } else {
                    if t_max[2] > range {
                        break;
                    }
                    pos.z += step[2];
                    t_max[2] += t_delta[2];
                    // (0.0, 0.0, -step[2])
                    OcclusionFace::Top
                }
            };
        }

        None
    }
}

fn intbound(s: f64, ds: f64) -> f64 {
    if ds < 0.0 {
        intbound(-s, -ds)
    } else {
        1.0 - (s.rem_euclid(1.0) as f64)
    }
}

fn modulus(value: f64, modulus: f64) -> f64 {
    (value.rem_euclid(modulus) + modulus).rem_euclid(modulus)
}
