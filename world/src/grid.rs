use std::convert::TryFrom;
use std::ops::{Deref, DerefMut, Index, IndexMut, Range};

#[macro_export]
macro_rules! grid_declare {
    (struct $name:ident < $implname:ident, $t:ty > , $x:expr, $y:expr, $z:expr) => {
        pub type $name = Grid<$implname>;

        pub struct $implname {
            // TODO pub hardcoded :(
            array: [$t; Self::FULL_SIZE],
        }

        impl GridImpl for $implname {
            type Item = $t;
            const DIMS: [i32; 3] = [$x as i32, $y as i32, $z as i32];
            const FULL_SIZE: usize = $x * $y * $z;

            #[inline(always)]
            fn array(&self) -> &[Self::Item] {
                &self.array
            }

            #[inline(always)]
            fn array_mut(&mut self) -> &mut [Self::Item] {
                &mut self.array
            }
        }

        impl Default for $implname {
            fn default() -> Self {
                let mut array: [std::mem::MaybeUninit<$t>; Self::FULL_SIZE] =
                    unsafe { std::mem::MaybeUninit::uninit().assume_init() };

                for elem in &mut array[..] {
                    unsafe {
                        std::ptr::write(elem.as_mut_ptr(), Default::default());
                    }
                }
                let array = unsafe { std::mem::transmute(array) };
                Self { array }
            }
        }
    };
}

pub type CoordType = [i32; 3];

pub trait GridImpl: Default {
    type Item: Default;
    const DIMS: [i32; 3];
    const FULL_SIZE: usize;

    fn array(&self) -> &[Self::Item];
    fn array_mut(&mut self) -> &mut [Self::Item];
}

#[derive(Default)]
pub struct Grid<I: GridImpl>(I);

impl<I: GridImpl> Grid<I> {
    pub fn indices(&self) -> Range<usize> {
        0..I::FULL_SIZE
    }

    pub fn full_size() -> usize {
        I::FULL_SIZE
    }

    fn flatten(coord: &CoordType) -> usize {
        let &[x, y, z] = coord;
        let [xs, ys, _zs] = I::DIMS;
        usize::try_from(x + xs * (y + ys * z)).unwrap()
    }

    fn unflatten(index: usize) -> CoordType {
        // TODO are %s optimised to bitwise ops if a multiple of 2?
        let [xs, ys, _zs] = I::DIMS;
        //        let xs = usize::try_from(xs).unwrap();
        //        let ys = usize::try_from(ys).unwrap();
        let index = i32::try_from(index).unwrap();
        [index % xs, (index / xs) % ys, index / (ys * xs)]
    }

    pub fn unflatten_index(&self, index: usize) -> CoordType {
        Self::unflatten(index)
    }

    /// Vertical slice in z direction
    pub fn slice_range(index: i32) -> (usize, usize) {
        let [xs, ys, _zs] = I::DIMS;
        let slice_count = xs * ys;
        let offset = index * slice_count;
        (offset as usize, (offset + slice_count) as usize)
    }

    pub fn slice_count() -> i32 {
        I::DIMS[2]
    }
}

// ---

impl<I: GridImpl> Index<&CoordType> for Grid<I> {
    type Output = I::Item;

    fn index(&self, index: &CoordType) -> &Self::Output {
        &self.0.array()[Self::flatten(index)]
    }
}

impl<I: GridImpl> Index<usize> for Grid<I> {
    type Output = I::Item;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0.array()[index]
    }
}

impl<I: GridImpl> IndexMut<&CoordType> for Grid<I> {
    fn index_mut(&mut self, index: &CoordType) -> &mut Self::Output {
        &mut self.0.array_mut()[Self::flatten(index)]
    }
}

impl<I: GridImpl> IndexMut<usize> for Grid<I> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0.array_mut()[index]
    }
}

impl<I: GridImpl> Deref for Grid<I> {
    type Target = [I::Item];

    fn deref(&self) -> &Self::Target {
        self.0.array()
    }
}

impl<I: GridImpl> DerefMut for Grid<I> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.array_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // grid of u32s with dimensions 4x5x6
    grid_declare!(struct TestGrid<TestImpl, u32>, 4, 5, 6);

    #[test]
    fn simple() {
        let mut grid = TestGrid::default();

        // check the number of elements is as expected
        assert_eq!(TestGrid::full_size(), 4 * 5 * 6);
        assert_eq!(TestGrid::full_size(), grid.indices().len());

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
        for x in grid.iter_mut() {
            *x = 1;
        }

        assert_eq!(grid[&[2, 2, 3]], 1);
    }
}
