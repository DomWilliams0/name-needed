use crate::region::feature::FeatureZRange;
use crate::region::region::RegionChunksBlockRows;
use crate::region::unit::PlanetPoint;
use crate::region::RegionLocationUnspecialized;
use crate::BiomeType;
use common::{ArrayVec, Itertools};

use std::array::IntoIter;
use std::convert::identity;
use unit::world::CHUNK_SIZE;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[cfg_attr(test, derive(Ord, PartialOrd))]
pub enum RegionNeighbour {
    /// y+1
    Up = 0,
    /// y-1
    Down,
    /// x-1
    Left,
    /// x+1
    Right,
    UpLeft,
    UpRight,
    DownRight,
    DownLeft,
}

#[derive(Clone, Debug)]
pub struct BiomeRow<const SIZE: usize> {
    /// Column idx in grid of blocks_per_chunk_side*chunks_per_region_side
    pub col: usize,

    /// Row start as block index in row
    pub start: RowIndex,

    /// Inclusive row end as block index in row
    pub end: RowIndex,

    /// Inclusive range of ground height between start and end
    pub z_range: FeatureZRange,
}

#[derive(Copy, Clone, Debug)]
#[cfg_attr(test, derive(Eq, PartialEq))]
pub enum RowIndex {
    Continued,

    Index(usize),
}

/// Scans rows of blocks within the region to collect points that form a concave hull around blocks
/// of the same biome
///
/// (region neighbours, diagonal region neighbours derived from aligned neighbours)
pub fn scan<const SIZE: usize>(
    chunks: RegionChunksBlockRows<SIZE>,
    biome: BiomeType,
    mut per_row: impl FnMut(BiomeRow<SIZE>),
) -> ArrayVec<RegionNeighbour, 8> {
    let region_side_length = SIZE * CHUNK_SIZE.as_usize();

    let rows = chunks.blocks().chunks(region_side_length);

    // indexed by RegionNeighbour idx, aligned directions only
    let mut overflows = [None; 4];
    let mut add_overflow = |rn: RegionNeighbour| {
        debug_assert!(rn.aligned());
        overflows[rn as usize] = Some(rn);
    };

    for (col, row) in (&rows).into_iter().enumerate() {
        let mut row = row.enumerate().peekable();
        loop {
            let (start_ground, start_idx) = match row.find(|(_, b)| b.biome() == biome) {
                Some((i, b)) => (
                    b.ground(),
                    if i == 0 {
                        RowIndex::Continued
                    } else {
                        RowIndex::Index(i)
                    },
                ),
                None => break, // next row
            };
            let (end_ground, end_idx) = {
                let mut highest_ground = start_ground;
                let idx = match row.find(|(_, b)| {
                    highest_ground = highest_ground.max(b.ground());
                    b.biome() != biome
                }) {
                    Some((i, _)) => RowIndex::Index(i - 1), // -1 to make inclusive
                    None => RowIndex::Continued,
                };

                (highest_ground, idx)
            };

            // calculate possible overflows
            if let RowIndex::Continued = start_idx {
                add_overflow(RegionNeighbour::Left);
            }

            if let RowIndex::Continued = end_idx {
                add_overflow(RegionNeighbour::Right);
            }

            if col == 0 {
                add_overflow(RegionNeighbour::Down);
            }

            if col == region_side_length - 1 {
                add_overflow(RegionNeighbour::Up);
            }

            per_row(BiomeRow {
                col,
                start: start_idx,
                end: end_idx,
                z_range: FeatureZRange::new(start_ground, end_ground),
            });

            if row.peek().is_none() {
                // row finished
                break;
            }
        }
    }

    let mut neighbours = IntoIter::new(overflows)
        .filter_map(identity)
        .collect::<ArrayVec<RegionNeighbour, 8>>();

    // append diagonals
    calculate_regional_diagonals(&overflows, |diag| neighbours.push(diag));
    neighbours
}

/// Input should be in enum declaration order
fn calculate_regional_diagonals(
    neighbours: &[Option<RegionNeighbour>; 4],
    mut per_diag: impl FnMut(RegionNeighbour),
) {
    let slice = &neighbours[0..4];

    macro_rules! diag {
        ($indices:expr, $diag:expr) => {
            let (a, b) = $indices;
            // TODO ensure no bounds checking here
            if slice[a].is_some() && slice[b].is_some() {
                per_diag($diag);
            }
        };
    }
    use RegionNeighbour::*;

    // [up, down, left, right]
    //  0   1     2     3
    diag!((0, 2), UpLeft);
    diag!((0, 3), UpRight);
    diag!((1, 2), DownLeft);
    diag!((1, 3), DownRight);
}

impl<const SIZE: usize> BiomeRow<SIZE> {
    #[inline]
    pub fn into_points(
        self,
        region: RegionLocationUnspecialized<SIZE>,
    ) -> ArrayVec<PlanetPoint<SIZE>, 2> {
        self.into_points_with_expansion(region, 0.0)
    }

    /// Expansion is subtracted from start point and added to end point (if any)
    pub fn into_points_with_expansion(
        self,
        region: RegionLocationUnspecialized<SIZE>,
        expansion: f64,
    ) -> ArrayVec<PlanetPoint<SIZE>, 2> {
        let mut points = ArrayVec::new();

        // find coordinates in 2d block grid within regions

        // block grid is this length along each side
        let region_block_grid_size = CHUNK_SIZE.as_usize() * SIZE;
        let (row_x, row_y) = {
            let (rx, ry) = region.xy();
            (
                rx * region_block_grid_size as u32,
                (ry * region_block_grid_size as u32) + self.col as u32,
            )
        };

        let mut add_point = |block: (u32, u32), expansion: f64| {
            points.push(PlanetPoint::new(
                (block.0 as f64 / region_block_grid_size as f64) + expansion,
                (block.1 as f64 / region_block_grid_size as f64) + expansion,
            ))
        };

        let start_idx = self.start.into_option().unwrap_or(0);
        let start = (row_x + start_idx as u32, row_y);
        add_point(start, -expansion);

        let end_idx = self.end.into_option().unwrap_or(region_block_grid_size - 1);
        if end_idx == start_idx {
            // single block, only yield 1 point
        } else {
            add_point((row_x + end_idx as u32, row_y), expansion)
        };

        points
    }
}

impl RegionNeighbour {
    pub fn offset<const SIZE: usize>(self) -> (i32, i32) {
        use RegionNeighbour::*;
        match self {
            Up => (0, 1),
            Down => (0, -1),
            Left => (-1, 0),
            Right => (1, 0),
            UpLeft => (-1, 1),
            UpRight => (1, 1),
            DownRight => (1, -1),
            DownLeft => (-1, -1),
        }
    }

    pub fn opposite(self) -> Self {
        use RegionNeighbour::*;
        match self {
            Up => Down,
            Down => Up,
            Left => Right,
            Right => Left,
            UpLeft => DownRight,
            UpRight => DownLeft,
            DownRight => UpLeft,
            DownLeft => UpRight,
        }
    }

    fn aligned(self) -> bool {
        use RegionNeighbour::*;
        matches!(self, Up | Down | Left | Right)
    }
}

#[cfg(test)]
impl<const SIZE: usize> PartialEq for BiomeRow<SIZE> {
    fn eq(&self, other: &Self) -> bool {
        self.col == other.col && self.start == other.start && self.end == other.end
    }
}

#[cfg(test)]
impl<const SIZE: usize> Eq for BiomeRow<SIZE> {}

impl RowIndex {
    fn into_option(self) -> Option<usize> {
        match self {
            RowIndex::Continued => None,
            RowIndex::Index(i) => Some(i),
        }
    }
}

//noinspection DuplicatedCode
#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::region::{Region, RegionChunk, Regions};
    use crate::region::unit::RegionLocation;
    use grid::GridImpl;
    use unit::world::ChunkLocation;

    const SIZE: usize = 2;
    const SIZE_2: usize = SIZE * SIZE;
    type SmolRegionLocation = RegionLocation<SIZE>;
    type SmolRegion = Region<SIZE, SIZE_2>;
    type SmolRegions = Regions<SIZE, SIZE_2>;
    type SmolRegionChunk = RegionChunk<SIZE>;

    fn do_scan(
        setup: impl FnOnce(&mut [SmolRegionChunk; SIZE_2]),
    ) -> (Vec<BiomeRow<SIZE>>, Vec<RegionNeighbour>) {
        let mut region_chunks: [_; SIZE_2] = [
            SmolRegionChunk::empty(),
            SmolRegionChunk::empty(),
            SmolRegionChunk::empty(),
            SmolRegionChunk::empty(),
        ];

        setup(&mut region_chunks);

        let mut rows = vec![];
        let mut overflow = scan(
            RegionChunksBlockRows::with_chunks(&region_chunks),
            BiomeType::Forest,
            |nice| {
                rows.push(nice);
            },
        );

        overflow.sort(); // for equality check
        (rows, overflow.to_vec())
    }

    #[test]
    fn scan_self_contained() {
        let idx = (2 * CHUNK_SIZE.as_usize()) + 5;
        let (rows, overflow) = do_scan(|chunks| {
            // a few in a row
            (**chunks[0].biomes_mut())[idx..idx + 4]
                .iter_mut()
                .for_each(|b| b.set_biome(BiomeType::Forest));
        });

        assert_eq!(
            rows,
            vec![BiomeRow {
                col: 2,
                start: RowIndex::Index(5),
                end: RowIndex::Index(8),
                z_range: FeatureZRange::null()
            }]
        );
        assert!(overflow.is_empty());

        assert_eq!(
            rows[0]
                .clone()
                .into_points(SmolRegionLocation::new(0, 0))
                .into_iter()
                .map(|point| point.into_block(0.into()))
                .map(|pos| (pos.0 as u32, pos.1 as u32))
                .collect_vec(),
            vec![(5, 2), (8, 2)]
        );
    }

    #[test]
    fn scan_over_chest_boundary() {
        let row = 2 * CHUNK_SIZE.as_usize(); // start on 3rd row
        let (rows, overflow) = do_scan(|chunks| {
            // fill up row 4 to end
            (**chunks[0].biomes_mut())[row + 4..row + CHUNK_SIZE.as_usize()]
                .iter_mut()
                .for_each(|b| b.set_biome(BiomeType::Forest));

            // continue row into next chunk
            (**chunks[1].biomes_mut())[row..row + 4]
                .iter_mut()
                .for_each(|b| b.set_biome(BiomeType::Forest));
        });

        assert_eq!(
            rows,
            vec![BiomeRow {
                col: 2,
                start: RowIndex::Index(4),
                end: RowIndex::Index(CHUNK_SIZE.as_usize() + 3),
                z_range: FeatureZRange::null()
            }]
        );
        assert!(overflow.is_empty());
    }
    #[test]
    fn scan_single_block() {
        // just (3,6) in a few region chunks
        let idx = (6 * CHUNK_SIZE.as_usize()) + 3;

        let (rows, overflow) = do_scan(|chunks| {
            (**chunks[0].biomes_mut())[idx].set_biome(BiomeType::Forest);
            (**chunks[1].biomes_mut())[idx].set_biome(BiomeType::Forest);
            (**chunks[2].biomes_mut())[idx].set_biome(BiomeType::Forest);
        });

        assert_eq!(
            rows,
            vec![
                // in region chunk 0 (BL)
                BiomeRow {
                    col: 6,
                    start: RowIndex::Index(3),
                    end: RowIndex::Index(3),
                    z_range: FeatureZRange::null()
                },
                // in region chunk 1 (BR), so offset a chunk width to the right
                BiomeRow {
                    col: 6,
                    start: RowIndex::Index(3 + CHUNK_SIZE.as_usize()),
                    end: RowIndex::Index(3 + CHUNK_SIZE.as_usize()),
                    z_range: FeatureZRange::null()
                },
                // in region chunk 2 (TL), so offset a chunk height upwards
                BiomeRow {
                    col: 6 + CHUNK_SIZE.as_usize(),
                    start: RowIndex::Index(3),
                    end: RowIndex::Index(3),
                    z_range: FeatureZRange::null()
                },
            ]
        );
        assert!(overflow.is_empty());

        assert_eq!(
            rows.iter()
                .flat_map(|p| p.clone().into_points(SmolRegionLocation::new(3, 4)))
                .count(),
            3
        );
    }

    #[test]
    fn scan_first_row_overflow_down() {
        let (rows, overflow) = do_scan(|chunks| {
            // 1st row, 4-9
            (**chunks[0].biomes_mut())[4..10]
                .iter_mut()
                .for_each(|b| b.set_biome(BiomeType::Forest));
        });

        assert_eq!(
            rows,
            vec![BiomeRow {
                col: 0,
                start: RowIndex::Index(4),
                end: RowIndex::Index(9),
                z_range: FeatureZRange::null()
            }]
        );
        assert_eq!(overflow, vec![RegionNeighbour::Down]);
    }

    #[test]
    fn scan_overflow_up_and_right() {
        // fill top row of region
        let idx = (CHUNK_SIZE.as_usize().pow(2)) - CHUNK_SIZE.as_usize();
        let (rows, overflow) = do_scan(|chunks| {
            (**chunks[2].biomes_mut())[idx..idx + CHUNK_SIZE.as_usize()]
                .iter_mut()
                .for_each(|b| b.set_biome(BiomeType::Forest));
            (**chunks[3].biomes_mut())[idx..idx + CHUNK_SIZE.as_usize()]
                .iter_mut()
                .for_each(|b| b.set_biome(BiomeType::Forest));
        });

        assert_eq!(
            rows,
            vec![BiomeRow {
                col: 2 * CHUNK_SIZE.as_usize() - 1, // top row
                start: RowIndex::Continued,
                end: RowIndex::Continued,
                z_range: FeatureZRange::null()
            }]
        );
        use RegionNeighbour::*;
        assert_eq!(overflow, vec![Up, Left, Right, UpLeft, UpRight,]);
    }

    #[test]
    fn scan_multiple_on_same_row() {
        let start = 2 * CHUNK_SIZE.as_usize(); // 3rd row

        let (rows, overflow) = do_scan(|chunks| {
            // 3rd row has 2 separate
            (**chunks[0].biomes_mut())[start + 1..start + 4]
                .iter_mut()
                .for_each(|b| b.set_biome(BiomeType::Forest));

            (**chunks[0].biomes_mut())[start + 10..start + 12]
                .iter_mut()
                .for_each(|b| b.set_biome(BiomeType::Forest));
        });

        assert_eq!(
            rows,
            vec![
                BiomeRow {
                    col: 2,
                    start: RowIndex::Index(1),
                    end: RowIndex::Index(3),
                    z_range: FeatureZRange::null()
                },
                BiomeRow {
                    col: 2,
                    start: RowIndex::Index(10),
                    end: RowIndex::Index(11),
                    z_range: FeatureZRange::null()
                }
            ]
        );
        assert!(overflow.is_empty());
    }

    #[test]
    fn scan_full_region() {
        let (rows, overflow) = do_scan(|chunks| {
            // whole region is filled
            chunks.iter_mut().for_each(|chunk| {
                chunk
                    .biomes_mut()
                    .array_mut()
                    .iter_mut()
                    .for_each(|b| b.set_biome(BiomeType::Forest));
            });
        });

        let expected_rows: Vec<_> = (0..(SIZE * CHUNK_SIZE.as_usize()))
            .map(|col| BiomeRow {
                col,
                start: RowIndex::Continued,
                end: RowIndex::Continued,
                z_range: FeatureZRange::null(),
            })
            .collect();

        assert_eq!(rows, expected_rows);

        use RegionNeighbour::*;
        assert_eq!(
            overflow,
            vec![Up, Down, Left, Right, UpLeft, UpRight, DownRight, DownLeft,]
        );

        // ensure all planet points when converted back to WorldPositions are within the region
        let reg = SmolRegionLocation::new(1, 1);
        rows.iter()
            .flat_map(|row| row.clone().into_points(reg))
            .map(|p| p.into_block(0.into()))
            .for_each(|block| {
                let chunk = ChunkLocation::from(block);
                let this_reg = RegionLocation::try_from_chunk(chunk).expect("should be good");
                assert_eq!(reg, this_reg);
            });
    }

    #[test]
    fn diagonal_region_neighbours() {
        fn diagonals(neighbours: Vec<RegionNeighbour>, mut expected: Vec<RegionNeighbour>) {
            let mut arr = [None; 4];
            for n in neighbours {
                arr[n as usize] = Some(n);
            }

            let mut diags = vec![];
            calculate_regional_diagonals(&arr, |diag| diags.push(diag));

            diags.retain(|n| !n.aligned()); // keep diags only
            diags.sort();
            expected.sort();

            assert_eq!(diags, expected);
        }

        use RegionNeighbour::*;

        diagonals(vec![], vec![]);
        diagonals(vec![Up, Down], vec![]);
        diagonals(vec![Left, Right], vec![]);
        diagonals(vec![Up, Left], vec![UpLeft]);
        diagonals(vec![Up, Down, Left], vec![UpLeft, DownLeft]);
        diagonals(vec![Down, Right, Left], vec![DownRight, DownLeft]);
    }
}
