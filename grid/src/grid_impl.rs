use std::convert::TryFrom;
use std::ops::Range;

use derive_more::*;

use std::fmt::Debug;

pub trait CoordType: Debug + Copy + Sized {
    fn try_get(self) -> Option<[usize; 3]>;
    fn from_coord(coord: [usize; 3]) -> Option<Self>;
}

pub trait GridImpl {
    type Item: Default + Clone;
    const DIMS: [usize; 3];
    const FULL_SIZE: usize;

    fn array(&self) -> &[Self::Item];
    fn array_mut(&mut self) -> &mut [Self::Item];

    fn indices(&self) -> Range<usize> {
        0..Self::FULL_SIZE
    }

    fn flatten(coord: impl CoordType) -> Option<usize> {
        let [x, y, z] = coord.try_get()?;
        let [xs, ys, _] = Self::DIMS;
        if x < xs && y < ys {
            // TODO can still panic
            Some(x + xs * (y + ys * z))
        } else {
            None
        }
    }

    #[inline]
    fn flatten_panic(coord: impl CoordType) -> usize {
        Self::flatten(coord).unwrap_or_else(|| panic!("invalid coordinate {:?}", coord))
    }

    #[inline]
    fn unflatten_panic<C: CoordType>(index: usize) -> C {
        Self::unflatten(index).unwrap_or_else(|| panic!("invalid index {}", index))
    }

    fn unflatten<C: CoordType>(index: usize) -> Option<C> {
        let [xs, ys, _] = Self::DIMS;
        let coord = [index % xs, (index / xs) % ys, index / (ys * xs)];
        C::from_coord(coord)
    }

    #[inline]
    fn index(&self, idx: usize) -> Option<&Self::Item> {
        self.array().get(idx)
    }

    #[inline]
    fn index_mut(&mut self, idx: usize) -> Option<&mut Self::Item> {
        self.array_mut().get_mut(idx)
    }

    #[inline]
    fn get(&self, coord: impl CoordType) -> Option<&Self::Item> {
        Self::flatten(coord).and_then(|idx| self.index(idx))
    }

    #[inline]
    fn get_mut(&mut self, coord: impl CoordType) -> Option<&mut Self::Item> {
        Self::flatten(coord).and_then(move |idx| self.index_mut(idx))
    }

    #[inline]
    fn get_unchecked(&self, coord: impl CoordType) -> &Self::Item {
        Self::flatten(coord)
            .and_then(|idx| self.index(idx))
            .unwrap_or_else(|| panic!("invalid coords: {:?}", coord))
    }

    #[inline]
    fn get_unchecked_mut(&mut self, coord: impl CoordType) -> &mut Self::Item {
        Self::flatten(coord)
            .and_then(move |idx| self.index_mut(idx))
            .unwrap_or_else(|| panic!("invalid coords: {:?}", coord))
    }

    /// Vertical slice in z direction
    fn slice_range(index: u32) -> (usize, usize) {
        Self::slice_range_multiple(index, index + 1)
    }

    /// Vertical slices in z direction, [from..to)
    fn slice_range_multiple(from: u32, to: u32) -> (usize, usize) {
        debug_assert!(from < to);

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

pub trait GridImplExt: GridImpl {
    fn default_boxed() -> Box<Self>;

    fn from_iter<I: Iterator<Item = Self::Item>>(iter: I) -> Box<Self>;
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
    pub fn flatten(coord: impl CoordType) -> Option<usize> {
        I::flatten(coord)
    }

    #[inline]
    pub fn flatten_panic(coord: impl CoordType) -> usize {
        I::flatten_panic(coord)
    }

    #[inline]
    pub fn unflatten_panic<C: CoordType>(index: usize) -> C {
        I::unflatten_panic(index)
    }

    #[inline]
    pub fn unflatten<C: CoordType>(index: usize) -> Option<C> {
        I::unflatten(index)
    }

    pub fn from_iter(items: impl Iterator<Item = I::Item>) -> Self {
        let inner = I::from_iter(items);
        Self(inner)
    }
}

// should use safe Option-returning version instead
impl<G: GridImpl> std::ops::Index<usize> for Grid<G> {
    type Output = G::Item;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0.array()[index]
    }
}

// should use safe Option-returning version instead
impl<G: GridImpl> std::ops::IndexMut<usize> for Grid<G> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0.array_mut()[index]
    }
}

impl<G: GridImpl> std::ops::Index<Range<usize>> for Grid<G> {
    type Output = [G::Item];

    fn index(&self, range: Range<usize>) -> &Self::Output {
        &self.0.array()[range]
    }
}

impl<G: GridImpl> std::ops::IndexMut<Range<usize>> for Grid<G> {
    fn index_mut(&mut self, range: Range<usize>) -> &mut Self::Output {
        &mut self.0.array_mut()[range]
    }
}

impl<G: GridImpl> GridImplExt for G {
    /// The grid may be too big for the stack, build directly on the heap
    fn default_boxed() -> Box<Self> {
        let vec = (0..G::FULL_SIZE)
            .map(|_| Default::default())
            .collect::<Vec<G::Item>>();

        // safety: pointer comes from Box::into_raw and self is #[repr(transparent)]
        unsafe {
            let raw_slice = Box::into_raw(vec.into_boxed_slice());
            Box::from_raw(raw_slice as *mut Self)
        }
    }

    fn from_iter<T: IntoIterator<Item = G::Item>>(iter: T) -> Box<Self> {
        let slice: Box<[G::Item]> = iter.into_iter().take(G::FULL_SIZE).collect();
        assert_eq!(slice.len(), G::FULL_SIZE, "grid iterator is wrong length");

        // safety: pointer comes from Box::into_raw and self is #[repr(transparent)]
        unsafe {
            let raw_slice = Box::into_raw(slice);
            Box::from_raw(raw_slice as *mut Self)
        }
    }
}

impl CoordType for [usize; 3] {
    fn try_get(self) -> Option<[usize; 3]> {
        Some(self)
    }

    fn from_coord(coord: [usize; 3]) -> Option<Self> {
        Some(coord)
    }
}

impl CoordType for [u8; 3] {
    fn try_get(self) -> Option<[usize; 3]> {
        let [x, y, z] = self;
        Some([usize::from(x), usize::from(y), usize::from(z)])
    }

    fn from_coord(coord: [usize; 3]) -> Option<Self> {
        let x = u8::try_from(coord[0]);
        let y = u8::try_from(coord[1]);
        let z = u8::try_from(coord[2]);
        match (x, y, z) {
            (Ok(x), Ok(y), Ok(z)) => Some([x, y, z]),
            _ => None,
        }
    }
}

impl CoordType for [i32; 3] {
    fn try_get(self) -> Option<[usize; 3]> {
        let x = usize::try_from(self[0]);
        let y = usize::try_from(self[1]);
        let z = usize::try_from(self[2]);
        match (x, y, z) {
            (Ok(x), Ok(y), Ok(z)) => Some([x, y, z]),
            _ => None,
        }
    }

    fn from_coord(coord: [usize; 3]) -> Option<Self> {
        let x = i32::try_from(coord[0]);
        let y = i32::try_from(coord[1]);
        let z = i32::try_from(coord[2]);
        match (x, y, z) {
            (Ok(x), Ok(y), Ok(z)) => Some([x, y, z]),
            _ => None,
        }
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
        assert_eq!(TestGrid::flatten_panic([0, 0, 0]), 0);
        assert_eq!(TestGrid::flatten_panic([1, 0, 0]), 1);
        assert_eq!(TestGrid::flatten_panic([0, 1, 0]), 4);
        assert_eq!(TestGrid::flatten_panic([0, 0, 1]), 20);

        for i in grid.indices() {
            let coord = TestGrid::unflatten_panic::<[usize; 3]>(i);
            let j = TestGrid::flatten_panic(coord);
            assert_eq!(i, j);
        }

        // check iter_mut works and actually sets values
        for x in grid.0.array.iter_mut() {
            *x = 1;
        }

        assert_eq!(*grid.get_unchecked([2, 2, 3]), 1);
    }

    #[test]
    fn cache_efficiency() {
        grid_declare!(struct TestGrid<TestImpl, u32>, 4, 5, 6);

        let mut last = None;
        for z in 0..TestImpl::DIMS[2] {
            for y in 0..TestImpl::DIMS[1] {
                for x in 0..TestImpl::DIMS[0] {
                    let idx = TestGrid::flatten_panic([x, y, z]);

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

    #[test]
    fn invalid_coords() {
        grid_declare!(struct TestGrid<TestImpl, u32>, 4, 5, 6);

        // let grid = TestGrid::default();
        assert!(TestGrid::flatten([5000, 0, 0]).is_none());
    }
}
