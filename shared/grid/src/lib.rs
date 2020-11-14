use derive_more::*;
use std::convert::TryFrom;
use std::ops::Range;

#[macro_export]
macro_rules! grid_declare {
    ($vis:vis struct $name:ident < $implname:ident, $t:ty > , $x:expr, $y:expr, $z:expr) => {
        $vis type $name = Grid<$implname>;

        #[derive(Clone)]
        #[repr(transparent)]
        $vis struct $implname {
            array: [$t; Self::FULL_SIZE],
        }

        impl GridImpl for $implname {
            type Item = $t;
            const DIMS: [i32; 3] = [$x as i32, $y as i32, $z as i32];
            const FULL_SIZE: usize = $x * $y * $z;

            fn array(&self) -> &[Self::Item] {
                &self.array
            }

            fn array_mut(&mut self) -> &mut [Self::Item] {
                &mut self.array
            }

            /// The grid may be too big for the stack, build directly on the heap
            fn default_boxed() -> Box<Self> {
                let mut vec: Vec<$t> = Vec::with_capacity(Self::FULL_SIZE);

                // safety: length is same as capacity
                unsafe {
                    vec.set_len(Self::FULL_SIZE);
                }

                let mut slice = vec.into_boxed_slice();
                for elem in &mut slice[..] {*elem = Default::default(); }


                // safety: pointer comes from Box::into_raw
                unsafe {
                    let raw_slice = Box::into_raw(slice);
                    Box::from_raw(raw_slice as *mut Self)
                }
            }
        }

        impl std::ops::Index<&$crate::grid::CoordType> for $implname {
            type Output = $t;

            fn index(&self, index: &$crate::grid::CoordType) -> &Self::Output {
                let index = self.flatten(index);
                &self.array()[index]
            }
        }

        impl std::ops::Index<usize> for $implname {
            type Output = $t;

            fn index(&self, index: usize) -> &Self::Output {
                &self.array()[index]
            }
        }

        impl std::ops::IndexMut<&$crate::grid::CoordType> for $implname {
            fn index_mut(&mut self, index: &$crate::grid::CoordType) -> &mut Self::Output {
                let index = self.flatten(index);
                &mut self.array_mut()[index]
            }
        }

        impl std::ops::IndexMut<usize> for $implname {
            fn index_mut(&mut self, index: usize) -> &mut Self::Output {
                &mut self.array_mut()[index]
            }
        }

    };
}

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
        // TODO are %s optimised to bitwise ops if a multiple of 2?
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

#[derive(Deref, From)]
pub struct GridRef<'a, I: GridImpl>(&'a I);

#[derive(Deref, DerefMut, From)]
pub struct GridRefMut<'a, I: GridImpl>(&'a mut I);

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
    use super::*;

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
