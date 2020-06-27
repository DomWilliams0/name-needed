use std::iter::once;

use common::derive_more::*;
use unit::world::{ChunkPosition, SlabIndex, SlabPosition, WorldPosition};

use crate::block::BlockType;
use crate::loader::update::split::split_range_across_slabs;

// TODO include reason for terrain update? (god magic, explosion, tool, etc)

/// A change to the terrain in the world, regardless of chunk boundaries
#[derive(Clone)]
#[cfg_attr(test, derive(Debug))]
pub struct WorldTerrainUpdate(GenericTerrainUpdate<WorldPosition>);

/// A change to the terrain in a slab
pub type SlabTerrainUpdate = GenericTerrainUpdate<SlabPosition>;

#[derive(Clone)]
#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
pub enum GenericTerrainUpdate<P> {
    /// A single block change
    Block(P, BlockType),

    /// Fill the entire range (inclusive in x,y,z)
    Range((P, P), BlockType),
}

/// Ecs resource for terrain updates generated by the simulation
#[derive(Default, Deref, DerefMut)]
pub struct TerrainUpdatesRes(pub Vec<WorldTerrainUpdate>);

impl WorldTerrainUpdate {
    pub fn into_slab_updates(
        self,
    ) -> impl Iterator<Item = (ChunkPosition, SlabIndex, SlabTerrainUpdate)> {
        let mut block_iter = None;
        let mut range_iter = None;

        match self.0 {
            GenericTerrainUpdate::Block(pos, bt) => {
                let chunk = ChunkPosition::from(pos);
                let block = SlabPosition::from(pos);
                let slab = pos.slice().slab_index();
                let result = (chunk, slab, SlabTerrainUpdate::Block(block, bt));
                block_iter = Some(once(result));
            }
            GenericTerrainUpdate::Range((a, b), bt) => {
                range_iter = Some(split_range_across_slabs(a, b, bt));
            }
        };

        block_iter
            .into_iter()
            .flatten()
            .chain(range_iter.into_iter().flatten())
    }

    pub fn with_block(pos: WorldPosition, bt: BlockType) -> Self {
        Self(GenericTerrainUpdate::Block(pos, bt))
    }

    pub fn with_range(from: WorldPosition, to: WorldPosition, bt: BlockType) -> Self {
        Self(GenericTerrainUpdate::Range((from, to), bt))
    }
}

mod split {
    use std::iter::once;

    use common::*;
    use unit::dim::CHUNK_SIZE;
    use unit::world::{ChunkPosition, GlobalSliceIndex, SlabIndex, WorldPosition, SLAB_SIZE};

    use crate::block::BlockType;
    use crate::loader::update::SlabTerrainUpdate;

    pub fn split_range_across_slabs(
        from: WorldPosition,
        to: WorldPosition,
        bt: BlockType,
    ) -> impl Iterator<Item = (ChunkPosition, SlabIndex, SlabTerrainUpdate)> {
        let (ax, bx) = if from.0 < to.0 {
            (from.0, to.0)
        } else {
            (to.0, from.0)
        };
        let (ay, by) = if from.1 < to.1 {
            (from.1, to.1)
        } else {
            (to.1, from.1)
        };
        let (az, bz) = if from.2 < to.2 {
            (from.2.slice(), to.2.slice())
        } else {
            (to.2.slice(), from.2.slice())
        };

        // discover chunk/slab boundaries, skipping the first if its a duplicate (i.e. the point is already
        // on a boundary)
        let boundaries_x = inter_chunk_boundaries(ax, bx).skip_while(move |x| *x == ax);
        let boundaries_y = inter_chunk_boundaries(ay, by).skip_while(move |y| *y == ay);
        let boundaries_z = inter_slab_boundaries(az, bz).skip_while(move |z| *z == az);

        let boundaries_x_inc = once(Coord::Original(ax))
            .chain(boundaries_x.clone().map(Coord::Boundary))
            .chain(once(Coord::Original(bx)));
        let boundaries_y_inc = once(Coord::Original(ay))
            .chain(boundaries_y.clone().map(Coord::Boundary))
            .chain(once(Coord::Original(by)));
        let boundaries_z_inc = once(Coord::Original(az))
            .chain(boundaries_z.clone().map(Coord::Boundary))
            .chain(once(Coord::Original(bz)));

        // combine into cuboids
        (boundaries_x_inc
            .tuple_windows()
            .cartesian_product(boundaries_y_inc.tuple_windows()))
        .cartesian_product(boundaries_z_inc.tuple_windows())
        .map(move |(((x1, x2), (y1, y2)), (z1, z2))| {
            let corner_bl = WorldPosition::from((x1.as_from(), y1.as_from(), z1.as_from()));
            let corner_tr = WorldPosition::from((x2.as_to(), y2.as_to(), z2.as_to()));

            let chunk = ChunkPosition::from(corner_bl);
            let slab = GlobalSliceIndex::new(z1.as_from()).slab_index();
            let update = SlabTerrainUpdate::Range((corner_bl.into(), corner_tr.into()), bt);
            (chunk, slab, update)
        })
    }

    fn inter_chunk_boundaries(from: i32, to: i32) -> impl Iterator<Item = i32> + Clone {
        find_boundaries(from, to, CHUNK_SIZE.as_i32())
    }

    fn inter_slab_boundaries(from: i32, to: i32) -> impl Iterator<Item = i32> + Clone {
        find_boundaries(from, to, SLAB_SIZE.as_i32())
    }

    fn find_boundaries(from: i32, to: i32, multiple: i32) -> impl Iterator<Item = i32> + Clone {
        fn round_to_nearest_multiple(val: i32, multiple: i32) -> i32 {
            // https://stackoverflow.com/a/9194117
            debug_assert_eq!(multiple % 2, 0); // multiple of 2 only
            (val + multiple - 1) & -multiple
        }

        debug_assert!(from <= to);

        // round up to nearest boundary
        let first_boundary = round_to_nearest_multiple(from, multiple);

        let range = from..=to;
        (first_boundary..)
            .step_by(multiple as usize)
            .take_while(move |coord| range.contains(coord))
    }

    #[derive(Clone, Copy)]
    enum Coord {
        Original(i32),
        Boundary(i32),
    }

    impl Coord {
        fn as_from(self) -> i32 {
            match self {
                Coord::Original(i) => i,
                Coord::Boundary(i) => i,
            }
        }

        fn as_to(self) -> i32 {
            match self {
                Coord::Original(i) => i,
                Coord::Boundary(i) => i - 1,
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use common::*;
        use unit::dim::CHUNK_SIZE;
        use unit::world::{ChunkPosition, SlabIndex, SLAB_SIZE};

        use crate::block::BlockType;
        use crate::loader::update::split::inter_chunk_boundaries;
        use crate::loader::update::SlabTerrainUpdate;
        use crate::loader::WorldTerrainUpdate;

        #[test]
        fn discover_boundaries() {
            // same chunk
            assert_eq!(
                inter_chunk_boundaries(2, 4).collect_vec(),
                Vec::<i32>::new()
            );

            // across 1 chunk
            assert_eq!(
                inter_chunk_boundaries(2, CHUNK_SIZE.as_i32() + 2).collect_vec(),
                vec![CHUNK_SIZE.as_i32()]
            );

            // across 3 chunks
            assert_eq!(
                inter_chunk_boundaries(-2, (CHUNK_SIZE.as_i32() * 2) + 2).collect_vec(),
                vec![0, CHUNK_SIZE.as_i32(), CHUNK_SIZE.as_i32() * 2,]
            );

            // exactly on the chunk boundary still yields it - not this functions responsibility
            assert_eq!(
                inter_chunk_boundaries(0, CHUNK_SIZE.as_i32() - 1).collect_vec(),
                vec![0]
            );
        }

        #[test]
        fn within_chunk() {
            let update = WorldTerrainUpdate::with_range(
                (1, 1, 1).into(),
                (3, 3, 3).into(),
                BlockType::Stone,
            );
            assert_eq!(update.into_slab_updates().count(), 1);
        }

        fn slab_update(
            chunk: (i32, i32),
            slab: SlabIndex,
            from: (i32, i32, i32),
            to: (i32, i32, i32),
        ) -> (ChunkPosition, SlabIndex, SlabTerrainUpdate) {
            (
                chunk.into(),
                slab,
                SlabTerrainUpdate::Range((from.into(), to.into()), BlockType::Stone),
            )
        }

        fn world_update(
            from: (i32, i32, i32),
            to: (i32, i32, i32),
        ) -> Vec<(ChunkPosition, SlabIndex, SlabTerrainUpdate)> {
            WorldTerrainUpdate::with_range(from.into(), to.into(), BlockType::Stone)
                .into_slab_updates()
                .sorted_by(|(ca, sa, _), (cb, sb, _)| ca.cmp(cb).then(sa.cmp(sb)))
                .collect_vec()
        }

        #[test]
        fn across_single_chunk_single_axis() {
            let updates = world_update((-1, 3, 0), (1, 4, 0));
            assert_eq!(
                updates,
                vec![
                    slab_update(
                        (-1, 0),
                        SlabIndex(0),
                        (CHUNK_SIZE.as_i32() - 1, 3, 0),
                        (CHUNK_SIZE.as_i32() - 1, 4, 0)
                    ),
                    slab_update((0, 0), SlabIndex(0), (0, 3, 0), (1, 4, 0))
                ]
            );
        }

        #[test]
        fn across_multiple_chunks_single_axis() {
            let updates = world_update((-1, 3, 0), (CHUNK_SIZE.as_i32() + 4, 5, 0));
            assert_eq!(
                updates,
                vec![
                    slab_update(
                        (-1, 0),
                        SlabIndex(0),
                        (CHUNK_SIZE.as_i32() - 1, 3, 0),
                        (CHUNK_SIZE.as_i32() - 1, 5, 0)
                    ),
                    slab_update(
                        (0, 0),
                        SlabIndex(0),
                        (0, 3, 0),
                        (CHUNK_SIZE.as_i32() - 1, 5, 0)
                    ),
                    slab_update((1, 0), SlabIndex(0), (0, 3, 0), (4, 5, 0)),
                ]
            );
        }

        #[test]
        fn across_multiple_axes() {
            let updates = world_update((-2, -2, -2), (1, 1, 1));
            assert_eq!(
                updates,
                vec![
                    slab_update(
                        (-1, -1),
                        SlabIndex(-1),
                        (
                            CHUNK_SIZE.as_i32() - 2,
                            CHUNK_SIZE.as_i32() - 2,
                            SLAB_SIZE.as_i32() - 2
                        ),
                        (
                            CHUNK_SIZE.as_i32() - 1,
                            CHUNK_SIZE.as_i32() - 1,
                            SLAB_SIZE.as_i32() - 1
                        ),
                    ),
                    slab_update(
                        (-1, -1),
                        SlabIndex(0),
                        (CHUNK_SIZE.as_i32() - 2, CHUNK_SIZE.as_i32() - 2, 0),
                        (CHUNK_SIZE.as_i32() - 1, CHUNK_SIZE.as_i32() - 1, 1),
                    ),
                    slab_update(
                        (-1, 0),
                        SlabIndex(-1),
                        (CHUNK_SIZE.as_i32() - 2, 0, SLAB_SIZE.as_i32() - 2),
                        (CHUNK_SIZE.as_i32() - 1, 1, SLAB_SIZE.as_i32() - 1),
                    ),
                    slab_update(
                        (-1, 0),
                        SlabIndex(0),
                        (CHUNK_SIZE.as_i32() - 2, 0, 0),
                        (CHUNK_SIZE.as_i32() - 1, 1, 1),
                    ),
                    slab_update(
                        (0, -1),
                        SlabIndex(-1),
                        (0, CHUNK_SIZE.as_i32() - 2, SLAB_SIZE.as_i32() - 2),
                        (1, CHUNK_SIZE.as_i32() - 1, SLAB_SIZE.as_i32() - 1)
                    ),
                    slab_update(
                        (0, -1),
                        SlabIndex(0),
                        (0, CHUNK_SIZE.as_i32() - 2, 0),
                        (1, CHUNK_SIZE.as_i32() - 1, 1)
                    ),
                    slab_update(
                        (0, 0),
                        SlabIndex(-1),
                        (0, 0, SLAB_SIZE.as_i32() - 2),
                        (1, 1, SLAB_SIZE.as_i32() - 1)
                    ),
                    slab_update((0, 0), SlabIndex(0), (0, 0, 0), (1, 1, 1)),
                ]
            );
        }
    }
}
