use crate::world::ContiguousChunkIterator;
use crate::{BaseTerrain, BlockType, InnerWorldRef, SliceRange, WorldContext};
use misc::cgmath::Array;
use misc::cgmath::Vector3;
use misc::num_traits::signum;
use misc::{debug, InnerSpace, F};
use unit::space::view::ViewPoint;
use unit::world::{
    BlockPosition, ChunkLocation, GlobalSliceIndex, WorldPoint, WorldPosition, BLOCKS_PER_METRE,
};

#[derive(Debug, Clone)]
pub struct VoxelRay {
    pos: ViewPoint,
    dir: Vector3<F>,
}

impl VoxelRay {
    pub fn new(pos: ViewPoint, dir: Vector3<F>) -> Self {
        Self {
            pos,
            dir: dir.normalize(),
        }
    }

    pub fn origin(&self) -> ViewPoint {
        self.pos
    }

    pub fn direction(&self) -> Vector3<F> {
        self.dir
    }

    // TODO if found block is fully occluded, go upwards/some direction to find better candidate
    pub fn find_first_visible<C: WorldContext>(
        &self,
        world: &InnerWorldRef<C>,
        range: SliceRange,
    ) -> Option<WorldPosition> {
        self.find_first_with_callback(world, |_| {}, |pos| range.contains(pos.slice()))
    }

    pub fn find_first_with_callback<C: WorldContext>(
        &self,
        world: &InnerWorldRef<C>,
        mut cb: impl FnMut(WorldPosition),
        mut filter: impl FnMut(WorldPosition) -> bool,
    ) -> Option<WorldPosition> {
        if self.dir.magnitude2() < 0.9 {
            misc::warn!("invalid raycast direction {:?}", self.dir);
            return None;
        }

        // TODO skip ahead over unloaded chunks
        let step: [f64; 3] = {
            let vec = (self.dir / 10.0).cast::<f64>().unwrap(); // cant fail
            *vec.as_ref()
        };

        // https://gamedev.stackexchange.com/a/49423j
        let range = 800.0;
        let cam_pos = WorldPoint::from(self.pos);
        let mut pos = Vector3::new(cam_pos.x() as f64, cam_pos.y() as f64, cam_pos.z() as f64);
        let dir = self.dir.cast::<f64>().unwrap(); // cant fail
        let mut t_max = Vector3::new(
            intbound(pos.x, dir.x),
            intbound(pos.y, dir.y),
            intbound(pos.z, dir.z),
        );

        let t_delta = Vector3::new(step[0] / dir.x, step[1] / dir.y, step[2] / dir.z);

        let mut last_block = None;
        let mut has_seen_a_block = false;
        let mut chunk_iter = ContiguousChunkIterator::new(world);
        loop {
            let block_pos =
                WorldPoint::new_unchecked(pos.x as f32, pos.y as f32, pos.z as f32).floor();
            if filter(block_pos) {
                // TODO filter out invisible here
                if Some(block_pos) != last_block {
                    let block = chunk_iter
                        .next(ChunkLocation::from(block_pos))
                        .and_then(|chunk| chunk.get_block(block_pos.into()));

                    if let Some(b) = block {
                        has_seen_a_block = true;
                        if !b.block_type().is_air() {
                            // found a solid block
                            // TODO capture face
                            // TODO return a point instead of block
                            return Some(block_pos);
                        }
                    } else if has_seen_a_block {
                        // we have visited some blocks but passed through the whole world, abort
                        break;
                    }

                    last_block = Some(block_pos);
                    cb(block_pos);
                }
            }

            let face = if t_max[0] < t_max[1] {
                if t_max[0] < t_max[2] {
                    if t_max[0] > range {
                        break;
                    }

                    pos.x += step[0];
                    t_max[0] += t_delta[0];

                    (-step[0], 0.0, 0.0)
                } else {
                    if t_max[2] > range {
                        break;
                    }
                    pos.z += step[2];
                    t_max[2] += t_delta[2];
                    (0.0, 0.0, -step[2])
                }
            } else {
                if t_max[1] < t_max[2] {
                    if t_max[1] > range {
                        break;
                    }
                    pos.y += step[1];
                    t_max[1] += t_delta[1];
                    (0.0, -step[1], 0.0)
                } else {
                    if t_max[2] > range {
                        break;
                    }
                    pos.z += step[2];
                    t_max[2] += t_delta[2];
                    (0.0, 0.0, -step[2])
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
