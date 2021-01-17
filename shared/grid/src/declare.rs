#[macro_export]
// TODO call usize::from() on dims for nicer use with smallunsignedconstant
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


        impl std::ops::Index<&$crate::CoordType> for $implname {
            type Output = $t;

            fn index(&self, index: &$crate::CoordType) -> &Self::Output {
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

        impl std::ops::IndexMut<&$crate::CoordType> for $implname {
            fn index_mut(&mut self, index: &$crate::CoordType) -> &mut Self::Output {
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
