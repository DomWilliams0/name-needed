use std::marker::PhantomData;
use std::ops::{Deref, DerefMut, Index, IndexMut};

/// 3d only
pub trait Dims {
    fn dims() -> &'static [usize; 3];

    fn full_length() -> usize {
        Self::dims().iter().fold(1, |acc, x| acc * *x)
    }
}

type CoordType = [usize; 3];

pub struct Grid<T, D>
where
    T: Default,
    D: Dims,
{
    array: Vec<T>,
    phantom: PhantomData<D>,
}

impl<T, D> Grid<T, D>
where
    T: Default,
    D: Dims,
{
    pub fn new() -> Self {
        let mut array = Vec::with_capacity(D::full_length());
        array.resize_with(D::full_length(), Default::default);

        Self {
            array,
            phantom: PhantomData,
        }
    }

    fn flatten(coord: &CoordType) -> usize {
        let &[x, y, z] = coord;
        let &[xs, ys, _zs] = D::dims();
        x + xs * (y + ys * z)
    }

    fn unflatten(index: usize) -> CoordType {
        // TODO are %s optimised to bitwise ops if a multiple of 2?
        let &[xs, ys, _zs] = D::dims();
        [index % xs, (index / xs) % ys, index / (ys * xs)]
    }
}

// ---

impl<T, D> Index<&CoordType> for Grid<T, D>
where
    T: Default,
    D: Dims,
{
    type Output = T;

    fn index(&self, index: &CoordType) -> &Self::Output {
        &self.array[Self::flatten(index)]
    }
}

impl<T, D> IndexMut<&CoordType> for Grid<T, D>
where
    T: Default,
    D: Dims,
{
    fn index_mut(&mut self, index: &CoordType) -> &mut Self::Output {
        &mut self.array[Self::flatten(index)]
    }
}

impl<T, D> Deref for Grid<T, D>
where
    T: Default,
    D: Dims,
{
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.array
    }
}

impl<T, D> DerefMut for Grid<T, D>
where
    T: Default,
    D: Dims,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.array
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DimsTest;

    impl Dims for DimsTest {
        fn dims() -> &'static [usize; 3] {
            &[4, 5, 6]
        }
    }

    #[test]
    fn simple() {
        type TestGrid = Grid<u32, DimsTest>;
        let mut grid = TestGrid::new();
        assert_eq!(grid.array.len(), 4 * 5 * 6);
        assert_eq!(TestGrid::flatten(&[0, 0, 0]), 0);
        assert_eq!(TestGrid::flatten(&[1, 0, 0]), 1);
        assert_eq!(TestGrid::flatten(&[0, 1, 0]), 4);
        assert_eq!(TestGrid::flatten(&[0, 0, 1]), 20);

        for i in 0..DimsTest::full_length() {
            let coord = TestGrid::unflatten(i);
            let j = TestGrid::flatten(&coord);
            assert_eq!(i, j);
        }

        for x in grid.iter_mut() {
            *x = 1;
        }

        assert_eq!(grid[&[2, 2, 3]], 1);
    }
}
