use common::cgmath::num_traits::clamp;
use common::{ArrayVec, Boolinator, Itertools};
use derive_more::*;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::ops::{Deref, DerefMut, Index, IndexMut, Range};

// TODO allow smaller datatypes for dims
pub type CoordType = [i32; 3];

pub trait GridImpl {
    type Item: Default + Clone;
    const DIMS: [i32; 3];
    const FULL_SIZE: usize;

    fn array(&self) -> &[Self::Item];
    fn array_mut(&mut self) -> &mut [Self::Item];
    fn default_boxed() -> Box<Self>;
    fn from_iter<I: Iterator<Item = Self::Item>>(iter: I) -> Box<Self>;

    fn indices(&self) -> Range<usize> {
        0..Self::FULL_SIZE
    }

    fn flatten(coord: &CoordType) -> usize {
        let &[x, y, z] = coord;
        let [xs, ys, _] = Self::DIMS;
        // TODO handle this deadly unwrap!
        usize::try_from(x + xs * (y + ys * z)).unwrap()
    }

    fn unflatten(index: usize) -> CoordType {
        let [xs, ys, _] = Self::DIMS;
        // TODO handle this deadly unwrap!
        let index = i32::try_from(index).unwrap();
        [index % xs, (index / xs) % ys, index / (ys * xs)]
    }

    /// Vertical slice in z direction
    fn slice_range(&self, index: u32) -> (usize, usize) {
        self.slice_range_multiple(index, index + 1)
    }

    /// Vertical slices in z direction, [from..to)
    fn slice_range_multiple(&self, from: u32, to: u32) -> (usize, usize) {
        assert!(from < to);

        let [xs, ys, _] = Self::DIMS;
        let slice_count = (xs * ys) as u32;
        let offset = from * slice_count;
        let n = to - from; // asserted to>from above
        (
            offset as usize,
            (offset + (slice_count * n as u32)) as usize,
        )
    }
}

#[derive(Clone, Deref, DerefMut)]
#[repr(transparent)]
pub struct Grid<I: GridImpl>(#[deref(forward)] Box<I>);

#[derive(Serialize, Deserialize)]
pub struct DynamicGrid<T> {
    dims: [usize; 3],
    data: Box<[T]>,
}

pub trait GridCoord<T: Default> {
    fn into_index(self, grid: &DynamicGrid<T>) -> usize;
    fn into_coord(self, grid: &DynamicGrid<T>) -> [usize; 3];
}

pub enum CoordRange {
    All,
    Single(usize),
    /// [from..to)
    Range(usize, usize),
}

impl<T: Default> DynamicGrid<T> {
    pub fn new(dims: [usize; 3]) -> Self {
        let len = dims[0] * dims[1] * dims[2];
        assert_ne!(len, 0);

        let data = {
            let mut vec = Vec::with_capacity(len);
            vec.resize_with(len, T::default);
            vec.into_boxed_slice()
        };

        DynamicGrid { dims, data }
    }

    pub fn flatten_coords(&self, [x, y, z]: [usize; 3]) -> usize {
        let [xs, ys, _zs] = self.dims;
        x + xs * (y + ys * z)
    }

    pub fn unflatten_index(&self, index: usize) -> [usize; 3] {
        let [xs, ys, _zs] = self.dims;
        [index % xs, (index / xs) % ys, index / (ys * xs)]
    }

    #[inline]
    pub fn is_coord_in_range(&self, [x, y, z]: [usize; 3]) -> bool {
        x < self.dims[0] && y < self.dims[1] && z < self.dims[2]
    }

    #[inline]
    pub fn is_in_range(&self, idx: usize) -> bool {
        idx < self.data.len()
    }

    // TODO profile and improve coord wrapping
    #[inline]
    pub fn wrap_coord(&self, mut coord: [isize; 3]) -> [usize; 3] {
        let [x0, y0, z0] = &mut coord;
        let [x1, y1, z1] = [
            self.dims[0] as isize,
            self.dims[1] as isize,
            self.dims[2] as isize,
        ];

        if *x0 < 0 || *x0 >= x1 {
            *x0 = x0.rem_euclid(x1);
        }
        if *y0 < 0 || *y0 >= y1 {
            *y0 = y0.rem_euclid(y1);
        }

        // clamp, dont wrap z
        *z0 = clamp(*z0, 0, z1 - 1);

        let new_coord = [*x0 as usize, *y0 as usize, *z0 as usize];
        debug_assert!(
            self.is_coord_in_range(new_coord),
            "wrapped {:?} to bad coord {:?}",
            coord,
            new_coord
        );
        new_coord
    }

    pub fn dimensions(&self) -> [usize; 3] {
        self.dims
    }

    pub fn dimensions_xy(&self) -> [usize; 2] {
        [self.dims[0], self.dims[1]]
    }

    pub fn iter_coords(&self) -> impl Iterator<Item = ([usize; 3], &T)> + '_ {
        self.iter_coords_with_z_range(CoordRange::All)
    }

    pub fn iter_coords_mut(&mut self) -> impl Iterator<Item = ([usize; 3], &mut T)> + '_ {
        self.iter_coords_with_z_range_mut(CoordRange::All)
    }

    // TODO return <C: GridCoord>
    pub fn iter_coords_with_z_range(
        &self,
        z_range: CoordRange,
    ) -> impl Iterator<Item = ([usize; 3], &T)> + '_ {
        let (iter, start) = self.iter_coords_alone(z_range);
        iter.zip(self.data.iter().skip(start))
    }

    pub fn iter_coords_with_z_range_mut(
        &mut self,
        z_range: CoordRange,
    ) -> impl Iterator<Item = ([usize; 3], &mut T)> + '_ {
        let (iter, start) = self.iter_coords_alone(z_range);
        iter.zip(self.data.iter_mut().skip(start))
    }

    #[inline]
    fn iter_coords_alone(&self, z_range: CoordRange) -> (impl Iterator<Item = [usize; 3]>, usize) {
        Self::iter_coords_alone_static(z_range, self.dims)
    }

    pub fn iter_coords_alone_static(
        z_range: CoordRange,
        dims: [usize; 3],
    ) -> (impl Iterator<Item = [usize; 3]>, usize) {
        let (min_z, max_z) = match z_range {
            CoordRange::All => (0, dims[2]),
            CoordRange::Single(i) => (i, i + 1),
            CoordRange::Range(i, j) => (i, j),
        };

        let z_start = min_z * dims[0] * dims[1];
        let iter = (min_z..max_z)
            .cartesian_product(0..dims[1])
            .cartesian_product(0..dims[0])
            .map(move |((z, y), x)| [x, y, z]);
        (iter, z_start)
    }

    /*    /// y-1
        pub fn coord_above(&self, [x, y, z]: [usize; 3]) -> Option<usize> {
            let new = [x, y.wrapping_sub(1), z];
            self.is_coord_in_range(new)
                .as_some_from(|| self.flatten_coords(new))
        }

        /// y+1
        pub fn coord_below(&self, [x, y, z]: [usize; 3]) -> Option<usize> {
            let new = [x, y + 1, z];
            self.is_coord_in_range(new)
                .as_some_from(|| self.flatten_coords(new))
        }

        /// x-1
        pub fn coord_left(&self, [x, y, z]: [usize; 3]) -> Option<usize> {
            let new = [x.wrapping_sub(1), y, z];
            self.is_coord_in_range(new)
                .as_some_from(|| self.flatten_coords(new))
        }
        /// x+1
        pub fn coord_right(&self, [x, y, z]: [usize; 3]) -> Option<usize> {
            let new = [x + 1, y, z];
            self.is_coord_in_range(new)
                .as_some_from(|| self.flatten_coords(new))
        }
    */

    /// Filters out out-of-bounds neighbours
    pub fn neighbours(&self, index: usize) -> impl Iterator<Item = usize> + '_ {
        // profiling shows it's better to pass around an idx and unflatten than it is to pass
        // around [usize; 3]
        let coord = self.unflatten_index(index);

        let x0 = Some(coord[0]);
        let xp1 = Some(coord[0] + 1);
        let xs1 = coord[0].checked_sub(1);

        let y0 = Some(coord[1]);
        let yp1 = Some(coord[1] + 1);
        let ys1 = coord[1].checked_sub(1);

        ArrayVec::from([
            x0.zip(ys1),
            #[cfg(feature = "8neighbours")]
            xp1.zip(ys1),
            xp1.zip(y0),
            #[cfg(feature = "8neighbours")]
            xp1.zip(yp1),
            x0.zip(yp1),
            #[cfg(feature = "8neighbours")]
            xs1.zip(yp1),
            xs1.zip(y0),
            #[cfg(feature = "8neighbours")]
            xs1.zip(ys1),
        ])
        .into_iter()
        .filter_map(|xy| xy)
        .filter_map(move |(x, y)| {
            let coord = [x, y, 0];
            if self.is_coord_in_range(coord) {
                Some(self.flatten_coords(coord))
            } else {
                None
            }
        })
    }

    /// Wraps xy, clamps z
    pub fn wrapping_neighbours_3d(
        &self,
        coord: impl GridCoord<T>,
    ) -> impl Iterator<Item = (usize, [isize; 3])> + '_ {
        let [x, y, z] = coord.into_coord(self);

        let below = (z > 0).as_some_from(|| z - 1);
        let this = Some(z);
        let above = (z < self.dims[2]).as_some_from(|| z + 1);

        let zs = below.into_iter().chain(this).chain(above.into_iter());
        zs.flat_map(move |z| {
            self.wrapping_neighbours([x, y, z])
                .map(move |(n, [x, y])| (n, [x, y, z as isize]))
        })
    }

    pub fn wrapping_neighbours(
        &self,
        coord: impl GridCoord<T>,
    ) -> impl ExactSizeIterator<Item = (usize, [isize; 2])> + Clone + '_ {
        let [x, y, z] = coord.into_coord(self);
        let coord = [x as isize, y as isize, z as isize];

        let x0 = coord[0];
        let xp1 = coord[0] + 1;
        let xs1 = coord[0] - 1;

        let y0 = coord[1];
        let yp1 = coord[1] + 1;
        let ys1 = coord[1] - 1;

        ArrayVec::from([
            (x0, ys1),
            #[cfg(feature = "8neighbours")]
            (xp1, ys1),
            (xp1, y0),
            #[cfg(feature = "8neighbours")]
            (xp1, yp1),
            (x0, yp1),
            #[cfg(feature = "8neighbours")]
            (xs1, yp1),
            (xs1, y0),
            #[cfg(feature = "8neighbours")]
            (xs1, ys1),
        ])
        .into_iter()
        .map(move |(x, y)| {
            let coord = [x, y, 0];
            let coord = self.wrap_coord(coord);
            (self.flatten_coords(coord), [x, y])
        })
    }
}

impl<T: Default> GridCoord<T> for usize {
    fn into_index(self, _: &DynamicGrid<T>) -> usize {
        self
    }

    fn into_coord(self, grid: &DynamicGrid<T>) -> [usize; 3] {
        grid.unflatten_index(self)
    }
}

impl<T: Default> GridCoord<T> for [usize; 3] {
    fn into_index(self, grid: &DynamicGrid<T>) -> usize {
        grid.flatten_coords(self)
    }

    fn into_coord(self, _: &DynamicGrid<T>) -> [usize; 3] {
        self
    }
}

impl<T> Index<usize> for DynamicGrid<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.data[index]
    }
}

impl<T> IndexMut<usize> for DynamicGrid<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.data[index]
    }
}

impl<T: Default> Index<[usize; 3]> for DynamicGrid<T> {
    type Output = T;

    fn index(&self, coords: [usize; 3]) -> &Self::Output {
        self.index(self.flatten_coords(coords))
    }
}

impl<T: Default> IndexMut<[usize; 3]> for DynamicGrid<T> {
    fn index_mut(&mut self, coords: [usize; 3]) -> &mut Self::Output {
        self.index_mut(self.flatten_coords(coords))
    }
}

impl<T> AsRef<[T]> for DynamicGrid<T> {
    fn as_ref(&self) -> &[T] {
        &self.data
    }
}

impl<T> Deref for DynamicGrid<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> DerefMut for DynamicGrid<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl<I: GridImpl> Default for Grid<I> {
    fn default() -> Self {
        Self(I::default_boxed())
    }
}

impl<I: GridImpl> Grid<I> {
    pub const FULL_SIZE: usize = I::FULL_SIZE;
    // pub const SLICE_COUNT: i32 = I::DIMS[2];
    // pub const SLICE_SIZE: usize = (I::DIMS[0] * I::DIMS[1]) as usize;

    pub fn into_boxed_impl(self) -> Box<I> {
        self.0
    }

    #[inline]
    pub fn flatten(coord: &CoordType) -> usize {
        I::flatten(coord)
    }

    #[inline]
    pub fn unflatten(index: usize) -> CoordType {
        I::unflatten(index)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use std::ptr::null;

    #[test]
    fn simple() {
        // grid of u32s with dimensions 4x5x6
        grid_declare!(struct TestGrid<TestImpl, u32>, 4, 5, 6);
        let mut grid = TestGrid::default();

        // check the number of elements is as expected
        assert_eq!(TestGrid::FULL_SIZE, 4 * 5 * 6);
        assert_eq!(TestGrid::FULL_SIZE, grid.indices().len());
        // check coordinate resolution works
        assert_eq!(TestGrid::flatten(&[0, 0, 0]), 0);
        assert_eq!(TestGrid::flatten(&[1, 0, 0]), 1);
        assert_eq!(TestGrid::flatten(&[0, 1, 0]), 4);
        assert_eq!(TestGrid::flatten(&[0, 0, 1]), 20);

        for i in grid.indices() {
            let coord = TestGrid::unflatten(i);
            let j = TestGrid::flatten(&coord);
            assert_eq!(i, j);
        }

        // check iter_mut works and actually sets values
        for x in grid.0.array.iter_mut() {
            *x = 1;
        }

        assert_eq!(grid[&[2, 2, 3]], 1);
    }

    #[test]
    fn cache_efficiency() {
        grid_declare!(struct TestGrid<TestImpl, u32>, 4, 5, 6);

        let mut last = None;
        for z in 0..TestImpl::DIMS[2] {
            for y in 0..TestImpl::DIMS[1] {
                for x in 0..TestImpl::DIMS[0] {
                    let idx = TestGrid::flatten(&[x, y, z]);

                    if let Some(last) = last {
                        assert_eq!(idx, last + 1);
                    }

                    last = Some(idx);
                    eprintln!("{} -> {:?}", idx, (x, y, z));
                }
            }
        }
    }

    #[test]
    fn huge_grid_not_on_stack() {
        // 8MiB
        grid_declare!(struct HugeGrid<HugeGridImpl, u64>, 1, 1024, 1024);

        let huge = HugeGrid::default();
        assert_eq!(std::mem::size_of_val(&huge), std::mem::size_of::<usize>()); // heap ptr only
    }

    /*    #[test]
        fn dynamic_grid_relative_indices() {
            let grid = DynamicGrid::<i32>::new([5, 4, 1]);

            // sanity check direction of indices
            assert_eq!(grid.flatten_coords([0, 3, 0]), 15);
            assert_eq!(grid.flatten_coords([4, 0, 0]), 4);

            let a = [2, 2, 0];
            assert_eq!(grid.flatten_coords(a), 12);
            assert_eq!(grid.coord_above(a), Some(7));
            assert_eq!(grid.coord_below(a), Some(17));
            assert_eq!(grid.coord_left(a), Some(11));
            assert_eq!(grid.coord_right(a), Some(13));

            let b = [4, 0, 0]; // corner
            assert_eq!(grid.flatten_coords(b), 4);
            assert_eq!(grid.coord_above(b), None);
            assert_eq!(grid.coord_below(b), Some(9));
            assert_eq!(grid.coord_left(b), Some(3));
            assert_eq!(grid.coord_right(b), None);

            assert_eq!(grid.coord_left([0, 0, 0]), None);
        }
    */
    #[test]
    fn dynamic_grid_iter() {
        let grid = DynamicGrid::<()>::new([5, 4, 3]);

        let dumb_expected = grid
            .data
            .iter()
            .enumerate()
            .map(|(i, val)| (grid.unflatten_index(i), val))
            .collect::<Vec<_>>();

        let actual = grid.iter_coords().collect::<Vec<_>>();

        assert_eq!(dumb_expected, actual);
    }

    #[test]
    fn dynamic_grid_wrap_coord() {
        let grid = DynamicGrid::<()>::new([5, 4, 3]);

        assert_eq!(grid.wrap_coord([1, 1, 1]), [1, 1, 1]);

        assert_eq!(grid.wrap_coord([0, 1, 1]), [0, 1, 1]);
        assert_eq!(grid.wrap_coord([-1, 1, 1]), [4, 1, 1]);
        assert_eq!(grid.wrap_coord([-2, 1, 1]), [3, 1, 1]);

        let grid = DynamicGrid::<()>::new([128, 128, 1]);
        assert_eq!(grid.wrap_coord([128, 127, 0]), [0, 127, 0]);
    }

    #[test]
    fn dynamic_grid_wrapping_original() {
        let grid = DynamicGrid::<()>::new([3, 3, 3]);
        let neighbours = grid.wrapping_neighbours([2, 0, 0]);

        assert!(neighbours.clone().any(|n| n == (0, [3, 0])));
        assert!(neighbours.clone().any(|n| n == (1, [1, 0])));
        assert!(neighbours.clone().any(|n| n == (8, [2, -1])));

        for n in neighbours {
            eprintln!("{:?}", n);
        }
    }

    #[test]
    fn dynamic_grid_non_serializable_type() {
        struct A(*const i32);

        impl Default for A {
            fn default() -> Self {
                Self(null())
            }
        }

        // wew it compiles, that's a relief
        let _grid = DynamicGrid::<A>::new([1, 2, 3]);
    }
}
