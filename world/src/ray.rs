use crate::{BlockType, InnerWorldRef, WorldContext};
use misc::num_traits::signum;
use misc::{InnerSpace, Vector3};
use unit::space::view::ViewPoint;
use unit::world::{BlockPosition, GlobalSliceIndex, WorldPoint, WorldPosition, BLOCKS_PER_METRE};

#[derive(Debug, Clone)]
pub struct VoxelRay {
    pos: ViewPoint,
    dir: Vector3,
}

impl VoxelRay {
    pub fn new(pos: ViewPoint, dir: Vector3) -> Self {
        Self {
            pos,
            dir: dir.normalize(),
        }
    }

    pub fn origin(&self) -> ViewPoint {
        self.pos
    }

    pub fn direction(&self) -> Vector3 {
        self.dir
    }

    pub fn find_first<C: WorldContext>(&self, world: &InnerWorldRef<C>) -> Option<WorldPosition> {
        self.find_first_with_callback(world, |_| {})
    }

    pub fn find_first_with_callback<C: WorldContext>(
        &self,
        world: &InnerWorldRef<C>,
        mut cb: impl FnMut(WorldPosition),
    ) -> Option<WorldPosition> {
        if self.dir.magnitude2() < 0.9 {
            return None;
        }

        // TODO optimise to reuse chunk ref and avoid duplicate block pos checks
        let step: [f32; 3] = {
            let vec = self.dir / 10.0;
            *vec.as_ref()
        };

        // https://gamedev.stackexchange.com/a/49423j
        let range = 128.0;
        let cam_pos = WorldPoint::from(self.pos);
        let mut pos = Vector3::new(cam_pos.x(), cam_pos.y(), cam_pos.z());
        let mut t_max = Vector3::new(
            intbound(pos.x, self.dir.x),
            intbound(pos.y, self.dir.y),
            intbound(pos.z, self.dir.z),
        );

        let t_delta = Vector3::new(
            step[0] / self.dir.x,
            step[1] / self.dir.y,
            step[2] / self.dir.z,
        );

        let mut last_block = WorldPosition::new(0, 0, GlobalSliceIndex::bottom());
        loop {
            let block = WorldPoint::new_unchecked(pos.x, pos.y, pos.z).round();
            if block != last_block {
                if let Some(b) = world.block(block) {
                    if !b.block_type().is_air() {
                        // found a solid block
                        // TODO capture face
                        // TODO skip if slab is not visible to the player
                        return Some(block);
                    }
                }

                last_block = block;
                cb(block);
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

fn intbound(s: f32, ds: f32) -> f32 {
    if ds < 0.0 {
        intbound(-s, -ds)
    } else {
        1.0 - (s.rem_euclid(1.0))
    }
}

fn modulus(value: f32, modulus: f32) -> f32 {
    (value.rem_euclid(modulus) + modulus).rem_euclid(modulus)
}
