#[macro_export]
macro_rules! grid_declare {
    ($vis:vis struct $name:ident < $implname:ident, $t:ty > , $x:expr, $y:expr, $z:expr) => {
        $vis type $name = $crate::Grid<$implname>;

        #[derive(Clone)]
        #[repr(transparent)]
        $vis struct $implname {
            array: [$t; Self::FULL_SIZE],
        }

        impl $crate::GridImpl for $implname {
            type Item = $t;
            const DIMS: [usize; 3] = [$x as usize, $y as usize, $z as usize];
            const FULL_SIZE: usize = Self::DIMS[0] * Self::DIMS[1] * Self::DIMS[2];

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


                // safety: pointer comes from Box::into_raw and self is #[repr(transparent)]
                unsafe {
                    let raw_slice = Box::into_raw(slice);
                    Box::from_raw(raw_slice as *mut Self)
                }
            }

            fn from_iter<T: IntoIterator<Item = $t>>(iter: T) -> Box<Self> {
                let vec: Vec<$t> = iter.into_iter().collect();
                assert_eq!(vec.len(), Self::FULL_SIZE, "grid iterator is wrong length");

                // safety: pointer comes from Box::into_raw and self is #[repr(transparent)]
                unsafe {
                    let slice = vec.into_boxed_slice();
                    let raw_slice = Box::into_raw(slice);
                    Box::from_raw(raw_slice as *mut Self)
                }
            }
        }

        // use safe Option-returning version instead
        #[cfg(test)]
        impl std::ops::Index<usize> for $implname {
            type Output = $t;

            fn index(&self, index: usize) -> &Self::Output {
                &self.array()[index]
            }
        }

        // use safe Option-returning version instead
        #[cfg(test)]
        impl std::ops::IndexMut<usize> for $implname {
            fn index_mut(&mut self, index: usize) -> &mut Self::Output {
                &mut self.array_mut()[index]
            }
        }

        impl std::ops::Index<std::ops::Range<usize>> for $implname {
            type Output = [$t];

            fn index(&self, range: std::ops::Range<usize>) -> &Self::Output {
                &self.array()[range]
            }
        }

        impl std::ops::IndexMut<std::ops::Range<usize>> for $implname {
            fn index_mut(&mut self, range: std::ops::Range<usize>) -> &mut Self::Output {
                &mut self.array_mut()[range]
            }
        }
    };
}
