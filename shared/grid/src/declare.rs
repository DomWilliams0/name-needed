use crate::GridImpl;
#[macro_export]
macro_rules! grid_declare {
    ($vis:vis struct $name:ident < $implname:ident, $t:ty > , $x:expr, $y:expr, $z:expr) => {
        $vis type $name = $crate::Grid<$implname>;

        #[derive(Clone)]
        #[repr(transparent)]
        $vis struct $implname {
            array: [$t; <Self as $crate::GridImpl>::FULL_SIZE],
        }

        impl $crate::GridImpl for $implname {
            type Item = $t;
            const DIMS: [usize; 3] = [$x as usize, $y as usize, $z as usize];
            const FULL_SIZE: usize = <Self as $crate::GridImpl>::DIMS[0] * <Self as $crate::GridImpl>::DIMS[1] * <Self as $crate::GridImpl>::DIMS[2];

            fn array(&self) -> &[<Self as $crate::GridImpl>::Item] {
                &self.array
            }

            fn array_mut(&mut self) -> &mut [<Self as $crate::GridImpl>::Item] {
                &mut self.array
            }
        }
    };
}
