use crate::world::BlockCoord;

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct SmallUnsignedConstant(u32);

// TODO unsafe unchecked casts with no panicking code
impl SmallUnsignedConstant {
    #[inline]
    pub const fn as_f32(self) -> f32 {
        self.0 as f32
    }

    #[inline]
    pub const fn as_i32(self) -> i32 {
        self.0 as i32
    }

    #[inline]
    pub const fn as_u32(self) -> u32 {
        self.0 as u32
    }

    #[inline]
    pub const fn as_u16(self) -> u16 {
        self.0 as u16
    }

    #[inline]
    pub const fn as_i16(self) -> i16 {
        self.0 as i16
    }

    #[inline]
    pub const fn as_i8(self) -> i8 {
        self.0 as i8
    }

    #[inline]
    pub const fn as_u8(self) -> u8 {
        self.0 as u8
    }

    #[inline]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub const fn as_f64(self) -> f64 {
        self.0 as f64
    }

    #[inline]
    pub const fn as_block_coord(self) -> BlockCoord {
        // TODO helper for this-1
        self.0 as BlockCoord
    }

    #[inline]
    pub const fn new(u: u32) -> Self {
        Self(u)
    }
}

macro_rules! impl_from {
    ($ty:ty) => {
        paste::paste! {
            impl_from!($ty, [< as_ $ty >]);
        }
    };

    ($ty:ty, $name:ident) => {
        impl From<SmallUnsignedConstant> for $ty {
            fn from(constant: SmallUnsignedConstant) -> Self {
                constant.$name()
            }
        }
    };
}

impl_from!(f32);
impl_from!(i32);
impl_from!(u32);
impl_from!(u16);
impl_from!(i16);
impl_from!(u8);
impl_from!(usize);
impl_from!(f64);
