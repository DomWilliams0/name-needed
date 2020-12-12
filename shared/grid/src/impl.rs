use derive_more::*;
use std::convert::TryFrom;
use std::ops::Range;

// TODO allow smaller datatypes for dims
pub type CoordType = [i32; 3];

pub trait GridImpl {
    type Item: Default + Clone;
    const DIMS: [i32; 3];
    const FULL_SIZE: usize;

    fn array(&self) -> &[Self::Item];
    fn array_mut(&mut self) -> &mut [Self::Item];
    fn default_boxed() -> Box<Self>;

    fn indices(&self) -> Range<usize> {
        0..Self::FULL_SIZE
    }

    fn flatten(&self, coord: &CoordType) -> usize {
        let &[x, y, z] = coord;
        let [xs, ys, _zs] = Self::DIMS;
        usize::try_from(x + xs * (y + ys * z)).unwrap()
    }

    fn unflatten(&self, index: usize) -> CoordType {
        let [xs, ys, _zs] = Self::DIMS;
        //        let xs = usize::try_from(xs).unwrap();
        //        let ys = usize::try_from(ys).unwrap();
        let index = i32::try_from(index).unwrap();
        [index % xs, (index / xs) % ys, index / (ys * xs)]
    }

    /// Vertical slice in z direction
    fn slice_range(&self, index: i32) -> (usize, usize) {
        let [xs, ys, _zs] = Self::DIMS;
        let slice_count = xs * ys;
        let offset = index * slice_count;
        (offset as usize, (offset + slice_count) as usize)
    }
}

#[derive(Clone, Deref, DerefMut)]
#[repr(transparent)]
pub struct Grid<I: GridImpl>(#[deref(forward)] Box<I>);

pub struct DynamicGrid<T> {
    dims: [usize; 3],
    data: Box<[T]>,
}

impl <T:Default> DynamicGrid<T> {
    pub fn new(dims :(usize,usize,usize)) -> Self {
        let len = dims.0 * dims.1 * dims.2;
        assert_ne!(len ,0);

        let data = {
            let mut vec = Vec::with_capacity(len);
            vec.resize_with(len, T::default);
            vec.into_boxed_slice()
        };

        DynamicGrid {
            dims: [dims.0, dims.1, dims.2],
            data,
        }
    }

    pub fn index(&self, idx: usize) -> &T {
        &self.data[idx]
    }

    pub fn index_mut(&mut self, idx: usize) -> &mut T {
        &mut self.data[idx]
    }

    pub fn index_with_coords(&self, coords: [usize; 3]) -> &T {
        self.index(self.flatten_coords(coords))
    }

    pub fn index_with_coords_mut(&mut self, coords: [usize; 3]) -> &mut T {
        self.index_mut(self.flatten_coords(coords))
    }

    fn flatten_coords(&self, [x,y,z]:[usize;3]) -> usize {
        let [xs, ys, _zs] = self.dims;
        x + xs * (y + ys * z)
    }

    fn unflatten_index(&self, index: usize) -> [usize; 3] {
        let [xs, ys, _zs] = self.dims;
        [index % xs, (index / xs) % ys, index / (ys * xs)]
    }

    pub fn dimensions(&self) -> [usize; 3] {self.dims}

    pub fn dimensions_xy(&self) -> [usize; 2] {[self.dims[0], self.dims[1]]}

    pub fn iter(&self) -> impl Iterator<Item=([usize;3], &T)>{
        self.data.iter().enumerate().map(move |(i, val)| (self.unflatten_index(i), val))
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
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn simple() {
        // grid of u32s with dimensions 4x5x6
        grid_declare!(struct TestGrid<TestImpl, u32>, 4, 5, 6);
        let mut grid = TestGrid::default();

        // check the number of elements is as expected
        assert_eq!(TestGrid::FULL_SIZE, 4 * 5 * 6);
        assert_eq!(TestGrid::FULL_SIZE, grid.indices().len());
        // check coordinate resolution works
        assert_eq!(grid.flatten(&[0, 0, 0]), 0);
        assert_eq!(grid.flatten(&[1, 0, 0]), 1);
        assert_eq!(grid.flatten(&[0, 1, 0]), 4);
        assert_eq!(grid.flatten(&[0, 0, 1]), 20);

        for i in grid.indices() {
            let coord = grid.unflatten(i);
            let j = grid.flatten(&coord);
            assert_eq!(i, j);
        }

        // check iter_mut works and actually sets values
        for x in grid.0.array.iter_mut() {
            *x = 1;
        }

        assert_eq!(grid[&[2, 2, 3]], 1);
    }

    #[test]
    fn huge_grid_not_on_stack() {
        // 8MiB
        grid_declare!(struct HugeGrid<HugeGridImpl, u64>, 1, 1024, 1024);

        let huge = HugeGrid::default();
        assert_eq!(std::mem::size_of_val(&huge), std::mem::size_of::<usize>()); // heap ptr only
    }
}
