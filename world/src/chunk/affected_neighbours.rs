use std::hint::unreachable_unchecked;
use std::ops::RangeInclusive;

use misc::ArrayVec;
use strum::{EnumCount, IntoEnumIterator};
use unit::world::{
    LocalSliceIndex, RangePosition, SlabPosition, SliceIndex, WorldRange, CHUNK_SIZE, SLAB_SIZE,
};

#[derive(Copy, Clone, strum::EnumCount, strum::EnumIter)]
#[repr(u8)]
#[cfg_attr(test, derive(strum::EnumString, Debug, Eq, PartialEq, Ord, PartialOrd))]
pub enum OcclusionAffectedNeighbourSlab {
    BelowSouth,
    BelowSouthEast,
    BelowEast,
    BelowNorthEast,
    BelowNorth,
    BelowNorthWest,
    BelowWest,
    BelowSouthWest,
    Below,

    South,
    SouthEast,
    East,
    NorthEast,
    North,
    NorthWest,
    West,
    SouthWest,

    AboveSouth,
    AboveSouthEast,
    AboveEast,
    AboveNorthEast,
    AboveNorth,
    AboveNorthWest,
    AboveWest,
    AboveSouthWest,
    Above,
}

impl OcclusionAffectedNeighbourSlab {
    pub const fn offset(self) -> [i8; 3] {
        use OcclusionAffectedNeighbourSlab::*;
        match self {
            BelowSouth => [0, -1, -1],
            BelowSouthEast => [1, -1, -1],
            BelowEast => [1, 0, -1],
            BelowNorthEast => [1, 1, -1],
            BelowNorth => [0, 1, -1],
            BelowNorthWest => [-1, 1, -1],
            BelowWest => [-1, 0, -1],
            BelowSouthWest => [-1, -1, -1],
            Below => [0, 0, -1],

            South => [0, -1, 0],
            SouthEast => [1, -1, 0],
            East => [1, 0, 0],
            NorthEast => [1, 1, 0],
            North => [0, 1, 0],
            NorthWest => [-1, 1, 0],
            West => [-1, 0, 0],
            SouthWest => [-1, -1, 0],

            AboveSouth => [0, -1, 1],
            AboveSouthEast => [1, -1, 1],
            AboveEast => [1, 0, 1],
            AboveNorthEast => [1, 1, 1],
            AboveNorth => [0, 1, 1],
            AboveNorthWest => [-1, 1, 1],
            AboveWest => [-1, 0, 1],
            AboveSouthWest => [-1, -1, 1],
            Above => [0, 0, 1],
        }
    }

    fn from_offset(offset: [i8; 3]) -> Option<OcclusionAffectedNeighbourSlab> {
        use OcclusionAffectedNeighbourSlab::*;
        Some(match offset {
            [0, -1, -1] => BelowSouth,
            [1, -1, -1] => BelowSouthEast,
            [1, 0, -1] => BelowEast,
            [1, 1, -1] => BelowNorthEast,
            [0, 1, -1] => BelowNorth,
            [-1, 1, -1] => BelowNorthWest,
            [-1, 0, -1] => BelowWest,
            [-1, -1, -1] => BelowSouthWest,
            [0, 0, -1] => Below,

            [0, -1, 0] => South,
            [1, -1, 0] => SouthEast,
            [1, 0, 0] => East,
            [1, 1, 0] => NorthEast,
            [0, 1, 0] => North,
            [-1, 1, 0] => NorthWest,
            [-1, 0, 0] => West,
            [-1, -1, 0] => SouthWest,

            [0, -1, 1] => AboveSouth,
            [1, -1, 1] => AboveSouthEast,
            [1, 0, 1] => AboveEast,
            [1, 1, 1] => AboveNorthEast,
            [0, 1, 1] => AboveNorth,
            [-1, 1, 1] => AboveNorthWest,
            [-1, 0, 1] => AboveWest,
            [-1, -1, 1] => AboveSouthWest,
            [0, 0, 1] => Above,

            [0, 0, 0] => return None,
            _ => {
                debug_assert!(false);
                unsafe { unreachable_unchecked() }
            }
        })
    }
}

#[derive(Default)]
pub struct OcclusionAffectedNeighbourSlabs {
    /// Maps to ordinals
    affected: [bool; OcclusionAffectedNeighbourSlab::COUNT],
}

impl OcclusionAffectedNeighbourSlabs {
    pub fn update(&mut self, changed_blocks: WorldRange<SlabPosition>) {
        if let Some(b) = changed_blocks.as_single() {
            self.update_block(b)
        } else {
            let ((x1, x2), (y1, y2), (z1, z2)) = changed_blocks.ranges();
            let p = |x, y, z| {
                SlabPosition::new_srsly_unchecked(x, y, LocalSliceIndex::new_srsly_unchecked(z))
            };
            self.update_block(p(x1, y1, z1));

            let expand_x = x1 != x2;
            let expand_y = y1 != y2;
            let expand_z = z1 != z2;
            if expand_x {
                self.update_block(p(x2, y1, z1));
            }
            if expand_y {
                self.update_block(p(x1, y2, z1));
            }
            if expand_z {
                self.update_block(p(x1, y1, z2));
            }
            if expand_x && expand_y {
                self.update_block(p(x2, y2, z1));
            }
            if expand_x && expand_z {
                self.update_block(p(x2, y1, z2));
            }
            if expand_y && expand_z {
                self.update_block(p(x1, y2, z2));
            }
            if expand_x && expand_y && expand_z {
                self.update_block(p(x2, y2, z2));
            }
        }
    }

    fn update_block(&mut self, changed_block: SlabPosition) {
        let (x, y, z) = changed_block.xyz();

        let cmp = |val, limit| match val {
            0 => -1..=0,
            _ if val == limit => 0..=1,
            _ => 0..=0,
        };
        let dx = cmp(x, CHUNK_SIZE.as_block_coord() - 1);
        let dy = cmp(y, CHUNK_SIZE.as_block_coord() - 1);
        let dz = cmp(z, SLAB_SIZE.as_block_coord() - 1);

        for z in dz {
            for y in dy.clone() {
                for x in dx.clone() {
                    if let Some(n) = OcclusionAffectedNeighbourSlab::from_offset([x, y, z]) {
                        self.touch(n);
                    }
                }
            }
        }
    }

    fn touch(&mut self, neighbour: OcclusionAffectedNeighbourSlab) {
        self.affected[neighbour as usize] = true;
    }

    pub fn finish(&self) -> impl Iterator<Item = OcclusionAffectedNeighbourSlab> {
        self.affected
            .into_iter()
            .zip(OcclusionAffectedNeighbourSlab::iter())
            .filter_map(|(b, n)| b.then_some(n))
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use misc::{lazy_static, o, Itertools};
    use rstest::rstest;
    use unit::world::{BlockCoord, LocalSliceIndex};

    use super::*;

    fn p((x, y, z): (i32, i32, i32)) -> SlabPosition {
        SlabPosition::new_unchecked(
            x as BlockCoord,
            y as BlockCoord,
            LocalSliceIndex::new_unchecked(z),
        )
    }

    #[derive(Debug, Eq, PartialEq)]
    struct RangeInput(Vec<WorldRange<SlabPosition>>);
    #[derive(Debug, Eq, PartialEq)]
    struct AffectedOutput(Vec<OcclusionAffectedNeighbourSlab>);

    #[rstest]
    #[case("[]", "[]")]
    #[case("[(5,5,5), ((2,2,1):(8,8,4)]", "[]")] // all internal
    // single block in middle
    #[case("[(5,0,5)]", "[South]")]
    #[case("[(5,15,5)]", "[North]")]
    #[case("[(15,5,5)]", "[East]")]
    // all along left side
    #[case("[(0,0,5):(0,15,5)]", "[West, SouthWest, NorthWest, North, South]")]
    // bottom corner
    #[case(
        "[(0,0,0)]",
        "[South, West, SouthWest, BelowSouth, BelowWest, BelowSouthWest, Below]"
    )]
    // all along left side on bottom
    #[case(
        "[(0,0,0):(0,15,1)]",
        "[South, West, North, NorthWest, SouthWest,\
          BelowSouth, BelowWest, BelowNorth, BelowNorthWest, BelowSouthWest,\
          Below]"
    )]
    // full slab
    #[case("[(0,0,0):(15,15,31)]", "<all>")]
    fn check_affected_neighbours(
        #[case] inputs: RangeInput,
        #[case] mut expected_output: AffectedOutput,
    ) {
        println!("inputs: {:?}", inputs.0);
        let mut output = OcclusionAffectedNeighbourSlabs::default();
        for range in inputs.0 {
            output.update(range);
        }

        let mut actual_output = output.finish().collect_vec();
        actual_output.sort();

        expected_output.0.sort();
        assert_eq!(expected_output.0, actual_output);
    }

    mod parse_str {
        use crate::chunk::affected_neighbours::OcclusionAffectedNeighbourSlab::SouthEast;

        use super::*;

        impl FromStr for RangeInput {
            type Err = ();

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let chars = s
                    .chars()
                    .filter(|&c| c.is_ascii_digit() || c == ',' || c == ':')
                    .group_by(|c| c.is_ascii_digit());
                let mut tokens = chars
                    .into_iter()
                    .map(|(_, chars)| chars.collect::<String>())
                    .peekable();

                fn consume_tuple(it: &mut impl Iterator<Item = String>) -> (SlabPosition, bool) {
                    let x = it.next().expect("expected int").parse().expect("bad int");
                    assert_eq!(it.next().expect("expected comma"), ",");
                    let y = it.next().expect("expected int").parse().expect("bad int");
                    assert_eq!(it.next().expect("expected comma"), ",");
                    let z = it.next().expect("expected int").parse().expect("bad int");

                    let pos = (x, y, z);
                    let multi = match it.next().as_deref() {
                        Some(":") => true,
                        Some(")" | ",") | None => false,
                        c => panic!("unexpected {c:?} after {pos:?}"),
                    };
                    (p(pos), multi)
                }

                let mut out = vec![];
                while tokens.peek().is_some() {
                    let (tup, more) = consume_tuple(&mut tokens);
                    if !more {
                        out.push(WorldRange::with_single(tup))
                    } else {
                        let (to, more) = consume_tuple(&mut tokens);
                        assert!(!more);
                        out.push(WorldRange::with_inclusive_range(tup, to))
                    }
                }

                Ok(RangeInput(out))
            }
        }

        impl FromStr for AffectedOutput {
            type Err = ();

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                use strum::IntoEnumIterator;
                if s == "<all>" {
                    return Ok(AffectedOutput(
                        OcclusionAffectedNeighbourSlab::iter().collect(),
                    ));
                };

                Ok(AffectedOutput(
                    s.trim_matches(&['[', ']'][..])
                        .split(',')
                        .filter(|s| !s.trim().is_empty())
                        .map(|s| s.trim().parse().expect("bad neighbour string"))
                        .collect(),
                ))
            }
        }

        #[test]
        fn parsing() {
            assert_eq!(
                "[(5,15,5), ((2,2,1):(8,8,4)]".parse(),
                Ok(RangeInput(vec![
                    WorldRange::with_single(p((5, 15, 5))),
                    WorldRange::with_inclusive_range(p((2, 2, 1)), p((8, 8, 4))),
                ]))
            );
            assert_eq!("[]".parse(), Ok(RangeInput(vec![])));
            assert_eq!("[]".parse(), Ok(AffectedOutput(vec![])));
            use OcclusionAffectedNeighbourSlab::*;
            assert_eq!(
                "[South, SouthEast,Above]".parse(),
                Ok(AffectedOutput(vec![South, SouthEast, Above]))
            );
        }
    }
}
