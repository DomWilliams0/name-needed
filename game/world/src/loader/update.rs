use std::iter::once;

use unit::world::{BlockPosition, ChunkPosition, WorldPosition};

use crate::block::BlockType;
use crate::loader::update::split::split_range_across_chunks;

// TODO include reason for terrain update? (god magic, explosion, tool, etc)

/// A change to the terrain in the world, regardless of chunk boundaries
#[derive(Clone)]
pub struct WorldTerrainUpdate(GenericTerrainUpdate<WorldPosition>);

// TODO SlabTerrainUpdate instead of chunk level
/// A change to the terrain in a chunk
pub type ChunkTerrainUpdate = GenericTerrainUpdate<BlockPosition>;

#[derive(Clone)]
#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
pub enum GenericTerrainUpdate<P> {
    /// A single block change
    Block(P, BlockType),

    /// Fill the entire range (inclusive in x,y,z)
    Range((P, P), BlockType),
}

impl WorldTerrainUpdate {
    pub fn into_chunk_updates(self) -> impl Iterator<Item = (ChunkPosition, ChunkTerrainUpdate)> {
        let mut block_iter = None;
        let mut range_iter = None;

        match self.0 {
            GenericTerrainUpdate::Block(pos, bt) => {
                let chunk = ChunkPosition::from(pos);
                let block = BlockPosition::from(pos);
                let result = (chunk, ChunkTerrainUpdate::Block(block, bt));
                block_iter = Some(once(result));
            }
            GenericTerrainUpdate::Range((a, b), bt) => {
                range_iter = Some(split_range_across_chunks(a, b, bt));
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
    use unit::world::{ChunkPosition, WorldPosition};

    use crate::block::BlockType;
    use crate::loader::ChunkTerrainUpdate;

    pub fn split_range_across_chunks(
        from: WorldPosition,
        to: WorldPosition,
        bt: BlockType,
    ) -> impl Iterator<Item = (ChunkPosition, ChunkTerrainUpdate)> {
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

        // discover chunk boundaries, skipping the first if its a duplicate (i.e. the point is already
        // on a boundary)
        let boundaries_x = inter_chunk_boundaries(ax, bx).skip_while(move |x| *x == ax);
        let boundaries_y = inter_chunk_boundaries(ay, by).skip_while(move |y| *y == ay);

        let boundaries_x_inc = once(Coord::Original(ax))
            .chain(boundaries_x.clone().map(Coord::Boundary))
            .chain(once(Coord::Original(bx)));
        let boundaries_y_inc = once(Coord::Original(ay))
            .chain(boundaries_y.clone().map(Coord::Boundary))
            .chain(once(Coord::Original(by)));

        // combine into rectangles
        boundaries_x_inc
            .tuple_windows()
            .cartesian_product(boundaries_y_inc.tuple_windows())
            .map(move |((x1, x2), (y1, y2))| {
                let corner_bl = WorldPosition::from((x1.as_from(), y1.as_from(), from.2));
                let corner_tr = WorldPosition::from((x2.as_to(), y2.as_to(), from.2));

                let chunk = ChunkPosition::from(corner_bl);
                let update = ChunkTerrainUpdate::Range((corner_bl.into(), corner_tr.into()), bt);
                (chunk, update)
            })
    }

    fn inter_chunk_boundaries(from: i32, to: i32) -> impl Iterator<Item = i32> + Clone {
        fn round_to_nearest_multiple(val: i32, multiple: i32) -> i32 {
            // https://stackoverflow.com/a/9194117
            debug_assert_eq!(multiple % 2, 0); // multiple of 2 only
            (val + multiple - 1) & -multiple
        }

        debug_assert!(from <= to);

        // round up to nearest boundary
        let first_boundary = round_to_nearest_multiple(from, CHUNK_SIZE.as_i32());

        let range = from..=to;
        (first_boundary..)
            .step_by(CHUNK_SIZE.as_usize())
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
        use unit::world::ChunkPosition;

        use crate::block::BlockType;
        use crate::loader::update::split::inter_chunk_boundaries;
        use crate::loader::{ChunkTerrainUpdate, WorldTerrainUpdate};

        #[test]
        fn discover_boundaries() {
            // same chunk
            assert_eq!(inter_chunk_boundaries(2, 4).collect_vec(), vec![]);

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
            assert_eq!(update.into_chunk_updates().count(), 1);
        }

        fn chunk_update(
            chunk: (i32, i32),
            from: (i32, i32, i32),
            to: (i32, i32, i32),
        ) -> (ChunkPosition, ChunkTerrainUpdate) {
            (
                chunk.into(),
                ChunkTerrainUpdate::Range((from.into(), to.into()), BlockType::Stone),
            )
        }

        fn world_update(from: (i32, i32, i32), to: (i32, i32, i32)) -> WorldTerrainUpdate {
            WorldTerrainUpdate::with_range(from.into(), to.into(), BlockType::Stone)
        }

        #[test]
        fn across_single_chunk_single_axis() {
            let update = world_update((-1, 3, 0), (1, 4, 0));
            let updates = update
                .into_chunk_updates()
                .sorted_by_key(|(c, _)| *c)
                .collect_vec();
            assert_eq!(
                updates,
                vec![
                    chunk_update(
                        (-1, 0),
                        (CHUNK_SIZE.as_i32() - 1, 3, 0),
                        (CHUNK_SIZE.as_i32() - 1, 4, 0)
                    ),
                    chunk_update((0, 0), (0, 3, 0), (1, 4, 0))
                ]
            );
        }

        #[test]
        fn across_multiple_chunks_single_axis() {
            let update = world_update((-1, 3, 0), (CHUNK_SIZE.as_i32() + 4, 5, 0));
            let updates = update
                .into_chunk_updates()
                .sorted_by_key(|(c, _)| c.0)
                .collect_vec();
            assert_eq!(
                updates,
                vec![
                    chunk_update(
                        (-1, 0),
                        (CHUNK_SIZE.as_i32() - 1, 3, 0),
                        (CHUNK_SIZE.as_i32() - 1, 5, 0)
                    ),
                    chunk_update((0, 0), (0, 3, 0), (CHUNK_SIZE.as_i32() - 1, 5, 0)),
                    chunk_update((1, 0), (0, 3, 0), (4, 5, 0)),
                ]
            );
        }

        #[test]
        fn across_multiple_axes() {
            let update = world_update((-2, -2, 0), (1, 1, 0));
            let updates = update
                .into_chunk_updates()
                .sorted_by_key(|(c, _)| *c)
                .collect_vec();
            assert_eq!(
                updates,
                vec![
                    chunk_update(
                        (-1, -1),
                        (CHUNK_SIZE.as_i32() - 2, CHUNK_SIZE.as_i32() - 2, 0),
                        (CHUNK_SIZE.as_i32() - 1, CHUNK_SIZE.as_i32() - 1, 0),
                    ),
                    chunk_update(
                        (-1, 0),
                        (CHUNK_SIZE.as_i32() - 2, 0, 0),
                        (CHUNK_SIZE.as_i32() - 1, 1, 0),
                    ),
                    chunk_update(
                        (0, -1),
                        (0, CHUNK_SIZE.as_i32() - 2, 0),
                        (1, CHUNK_SIZE.as_i32() - 1, 0)
                    ),
                    chunk_update((0, 0), (0, 0, 0), (1, 1, 0)),
                ]
            );
        }
    }
}
