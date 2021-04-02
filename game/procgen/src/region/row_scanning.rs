use crate::region::region::RegionChunk;
use crate::region::RegionLocationUnspecialized;
use crate::BiomeType;
use common::{once, ArrayVec, Itertools};
use unit::world::CHUNK_SIZE;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[cfg_attr(test, derive(Ord, PartialOrd))]
pub enum RegionNeighbour {
    /// y-1
    Up = 1,
    /// y+1
    Down,
    /// x-1
    Left,
    /// x+1
    Right,
}

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(Eq, PartialEq))]
pub struct BiomeRow<const SIZE: usize> {
    /// Column idx in grid of blocks_per_chunk_side*chunks_per_region_side
    pub col: usize,

    /// Row start as block index in row
    pub start: RowIndex,

    /// Inclusive row end as block index in row
    pub end: RowIndex,
}

#[derive(Copy, Clone, Debug)]
#[cfg_attr(test, derive(Eq, PartialEq))]
pub enum RowIndex {
    Continued,

    Index(usize),
}

/// Scans rows of blocks within the region to collect points that form a concave hull around blocks
/// of the same biome
pub fn scan<const SIZE: usize>(
    chunks: &[RegionChunk<SIZE>],
    biome: BiomeType,
    mut per_row: impl FnMut(BiomeRow<SIZE>),
) -> ArrayVec<RegionNeighbour, 4> {
    let region_side_length = SIZE * CHUNK_SIZE.as_usize();
    // let convert_idx = |col, i| (col * region_side_length) + i;

    let rows = chunks
        .iter()
        .flat_map(|chunk| chunk.description().blocks())
        .chunks(region_side_length);

    // indexed by RegionNeighbour idx - 1
    let mut overflows = [None; 4];
    let mut add_overflow = |rn: RegionNeighbour| {
        overflows[rn as usize - 1] = Some(rn);
    };

    for (col, row) in (&rows).into_iter().enumerate() {
        let mut row = row.enumerate().peekable();
        loop {
            let start = match row.find(|(_, b)| b.biome() == biome) {
                Some((i, _)) => {
                    if i == 0 {
                        RowIndex::Continued
                    } else {
                        RowIndex::Index(i)
                    }
                }
                None => break, // next row
            };
            let end = match row.find(|(_, b)| b.biome() != biome) {
                Some((i, _)) => RowIndex::Index(i - 1), // -1 to make inclusive
                None => RowIndex::Continued,
            };

            // calculate possible overflows
            if let RowIndex::Continued = start {
                add_overflow(RegionNeighbour::Left);
            }

            if let RowIndex::Continued = end {
                add_overflow(RegionNeighbour::Right);
            }

            if col == 0 {
                add_overflow(RegionNeighbour::Up);
            }

            if col == region_side_length - 1 {
                add_overflow(RegionNeighbour::Down);
            }

            per_row(BiomeRow { col, start, end });

            if row.peek().is_none() {
                // row finished
                break;
            }
        }
    }

    overflows.iter().filter_map(|opt| *opt).collect()
}

impl<const SIZE: usize> BiomeRow<SIZE> {
    pub fn into_points(
        self,
        region: RegionLocationUnspecialized<SIZE>,
    ) -> impl Iterator<Item = (u32, u32)> {
        let row_length = CHUNK_SIZE.as_usize() * SIZE;
        let row_start_idx = self.col * row_length;
        let (row_x, row_y) = {
            let (rx, ry) = region.xy();
            (rx, ry + row_start_idx as u32)
        };

        let start_idx = self.start.into_option().unwrap_or(0);
        let start = (row_x + start_idx as u32, row_y);

        let end_idx = self.end.into_option().unwrap_or(row_length - 1);
        let end = if end_idx == start_idx {
            // single block, only yield 1 point
            None
        } else {
            Some((row_x + end_idx as u32, row_y))
        };

        once(start).chain(end)
    }
}

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
    use crate::region::region::{ChunkHeightMap, Region, RegionChunk, Regions};
    use crate::region::unit::RegionLocation;
    use grid::GridImpl;

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
        let mut overflow = scan(&region_chunks, BiomeType::Forest, |nice| {
            rows.push(nice);
        });

        overflow.sort(); // for equality check
        (rows, overflow.to_vec())
    }

    fn row(col: usize) -> usize {
        (SIZE * CHUNK_SIZE.as_usize()) * col
    }

    #[test]
    fn scan_self_contained() {
        let row_start = row(1);
        let (rows, overflow) = do_scan(|chunks| {
            // 2nd row, 1-5
            (**chunks[0].biomes_mut())[row_start + 1..row_start + 6]
                .iter_mut()
                .for_each(|b| b.set_biome(BiomeType::Forest));
        });

        assert_eq!(
            rows,
            vec![BiomeRow {
                col: 1,
                start: RowIndex::Index(1),
                end: RowIndex::Index(5)
            }]
        );
        assert!(overflow.is_empty());

        let y_offset = row_start as u32; // 2nd row in 2x2 region chunk grid
        assert_eq!(
            rows[0]
                .clone()
                .into_points(SmolRegionLocation::new(0, 0))
                .collect_vec(),
            vec![(1, y_offset), (5, y_offset)]
        );
    }

    #[test]
    fn scan_single_block() {
        let idx = row(4) + 5; // arbitrary
        let (rows, overflow) = do_scan(|chunks| {
            // single block
            (**chunks[0].biomes_mut())[idx].set_biome(BiomeType::Forest);
        });

        assert_eq!(
            rows,
            vec![BiomeRow {
                col: 4,
                start: RowIndex::Index(5),
                end: RowIndex::Index(5)
            }]
        );
        assert!(overflow.is_empty());

        // only 1 point
        assert_eq!(
            rows[0]
                .clone()
                .into_points(SmolRegionLocation::new(3, 4))
                .count(),
            1
        );
    }

    #[test]
    fn scan_first_row_overflow_up() {
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
                end: RowIndex::Index(9)
            }]
        );
        assert_eq!(overflow, vec![RegionNeighbour::Up]);
    }

    #[test]
    fn scan_first_row_overflow_up_and_left() {
        let (rows, overflow) = do_scan(|chunks| {
            // 1st row full
            (**chunks[0].biomes_mut())[0..SIZE * CHUNK_SIZE.as_usize()]
                .iter_mut()
                .for_each(|b| b.set_biome(BiomeType::Forest));
        });

        assert_eq!(
            rows,
            vec![BiomeRow {
                col: 0,
                start: RowIndex::Continued,
                end: RowIndex::Continued,
            }]
        );
        assert_eq!(
            overflow,
            vec![
                RegionNeighbour::Up,
                RegionNeighbour::Left,
                RegionNeighbour::Right
            ]
        );
    }

    #[test]
    fn scan_multiple_on_same_row() {
        let row = row(2);
        let (rows, overflow) = do_scan(|chunks| {
            // 3rd row has 2 separate
            (**chunks[0].biomes_mut())[row + 1..row + 4]
                .iter_mut()
                .for_each(|b| b.set_biome(BiomeType::Forest));

            (**chunks[0].biomes_mut())[row + 10..row + 12]
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
                },
                BiomeRow {
                    col: 2,
                    start: RowIndex::Index(10),
                    end: RowIndex::Index(11),
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
            })
            .collect();

        assert_eq!(rows, expected_rows);
        assert_eq!(
            overflow,
            vec![
                RegionNeighbour::Up,
                RegionNeighbour::Down,
                RegionNeighbour::Left,
                RegionNeighbour::Right
            ]
        );
    }
}
