use std::hash::Hash;
use std::iter::once;

use unit::world::{
    ChunkLocation, RangePosition, SlabLocation, SlabPosition, WorldPosition, WorldPositionRange,
    WorldRange,
};

use crate::loader::update::split::split_range_across_slabs;
use crate::WorldContext;
use misc::Derivative;

// TODO include reason for terrain update? (god magic, explosion, tool, etc)

/// A change to the terrain in the world, regardless of chunk boundaries
#[derive(Derivative)]
#[derivative(
    Clone(bound = ""),
    PartialEq(bound = ""),
    Hash(bound = ""),
    Eq(bound = "")
)]
#[cfg_attr(test, derivative(Debug(bound = "")))]
pub struct WorldTerrainUpdate<C: WorldContext>(GenericTerrainUpdate<C, WorldPosition>);

/// A change to the terrain in a slab
pub type SlabTerrainUpdate<C> = GenericTerrainUpdate<C, SlabPosition>;

#[derive(Derivative)]
#[derivative(
    Clone(bound = "P: Clone"),
    PartialEq(bound = "P: PartialEq"),
    Hash(bound = "P: Hash"),
    Eq(bound = "P: Eq")
)]
#[cfg_attr(test, derivative(Debug(bound = "P: std::fmt::Debug")))]
pub struct GenericTerrainUpdate<C: WorldContext, P: RangePosition>(
    pub WorldRange<P>,
    pub C::BlockType,
);

impl<C: WorldContext> WorldTerrainUpdate<C> {
    pub fn into_slab_updates(self) -> impl Iterator<Item = (SlabLocation, SlabTerrainUpdate<C>)> {
        let mut block_iter = None;
        let mut range_iter = None;

        let WorldTerrainUpdate(GenericTerrainUpdate(range, block_type)) = self;

        match range {
            WorldRange::Single(pos) => {
                let chunk = ChunkLocation::from(pos);
                let block = SlabPosition::from(pos);
                let slab = pos.slice().slab_index();
                let result = (
                    SlabLocation::new(slab, chunk),
                    GenericTerrainUpdate(WorldRange::Single(block), block_type),
                );
                block_iter = Some(once(result));
            }
            range @ WorldRange::Range(_, _) => {
                range_iter = Some(split_range_across_slabs(range, block_type));
            }
        };

        block_iter
            .into_iter()
            .flatten()
            .chain(range_iter.into_iter().flatten())
    }

    pub fn new(range: WorldPositionRange, block_type: C::BlockType) -> Self {
        Self(GenericTerrainUpdate(range, block_type))
    }

    #[cfg(test)]
    pub fn inner(&self) -> &GenericTerrainUpdate<C, WorldPosition> {
        &self.0
    }
}

mod split {
    use std::iter::once;

    use misc::*;
    use unit::world::CHUNK_SIZE;
    use unit::world::{
        ChunkLocation, GlobalSliceIndex, SlabLocation, WorldPosition, WorldPositionRange,
        WorldRange, SLAB_SIZE,
    };

    use crate::loader::update::{GenericTerrainUpdate, SlabTerrainUpdate};
    use crate::WorldContext;

    pub fn split_range_across_slabs<C: WorldContext>(
        range: WorldPositionRange,
        bt: C::BlockType,
    ) -> impl Iterator<Item = (SlabLocation, SlabTerrainUpdate<C>)> {
        let ((ax, bx), (ay, by), (az, bz)) = range.ranges();

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

            let chunk = ChunkLocation::from(corner_bl);
            let slab = GlobalSliceIndex::new(z1.as_from()).slab_index();
            let update =
                GenericTerrainUpdate(WorldRange::Range(corner_bl.into(), corner_tr.into()), bt);
            (SlabLocation::new(slab, chunk), update)
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
        use misc::*;
        use unit::world::{ChunkLocation, SlabIndex, WorldPositionRange, WorldRange, SLAB_SIZE};
        use unit::world::{SlabPosition, CHUNK_SIZE};

        use crate::helpers::{DummyBlockType, DummyWorldContext};
        use crate::loader::update::split::inter_chunk_boundaries;
        use crate::loader::update::{GenericTerrainUpdate, SlabTerrainUpdate};
        use crate::loader::WorldTerrainUpdate;
        use std::convert::TryFrom;

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
            let update = WorldTerrainUpdate::<DummyWorldContext>::new(
                WorldPositionRange::with_inclusive_range((1, 1, 1), (3, 3, 3)),
                DummyBlockType::Stone,
            );
            assert_eq!(update.into_slab_updates().count(), 1);
        }

        fn slab_update(
            chunk: (i32, i32),
            slab: SlabIndex,
            from: (i32, i32, i32),
            to: (i32, i32, i32),
        ) -> (
            ChunkLocation,
            SlabIndex,
            SlabTerrainUpdate<DummyWorldContext>,
        ) {
            let from = SlabPosition::try_from([from.0, from.1, from.2]).unwrap();
            let to = SlabPosition::try_from([to.0, to.1, to.2]).unwrap();
            (
                chunk.into(),
                slab,
                GenericTerrainUpdate(WorldRange::Range(from, to), DummyBlockType::Stone),
            )
        }

        fn world_update(
            from: (i32, i32, i32),
            to: (i32, i32, i32),
        ) -> Vec<(
            ChunkLocation,
            SlabIndex,
            SlabTerrainUpdate<DummyWorldContext>,
        )> {
            WorldTerrainUpdate::new(
                WorldPositionRange::with_inclusive_range(from, to),
                DummyBlockType::Stone,
            )
            .into_slab_updates()
            .sorted_by(|(a, _), (b, _)| a.chunk.cmp(&b.chunk).then_with(|| a.slab.cmp(&b.slab)))
            .map(|(loc, update)| (loc.chunk, loc.slab, update))
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
