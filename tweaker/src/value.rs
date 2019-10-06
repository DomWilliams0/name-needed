use log::warn;
use num_traits::cast::cast;
use std::convert::TryFrom;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Int32(i32),
    Float(f64),
    Float32(f32),
    Bool(bool),
}

macro_rules! value {
    ($real_type:ty, $($pattern:path),*) => {

impl TryFrom<Value> for $real_type {
    type Error = ();
    fn try_from(v: Value) -> Result<Self, Self::Error> {
        match v {
        $(
            $pattern(_v) => cast(_v).ok_or(()),
        )*
            _ => {
                warn!("wrong type (expected {}, got {:?})", stringify!($real_type), v);
                Err(())
            }
        }
    }
}
    };
}

value!(i64, Value::Int);
value!(i32, Value::Int, Value::Int32);
value!(f64, Value::Float);
value!(f32, Value::Float, Value::Float32);

// bool is special case
impl TryFrom<Value> for bool {
    type Error = ();

    fn try_from(v: Value) -> Result<Self, Self::Error> {
        i32::try_from(v).map(|i| i != 0)
    }
}
