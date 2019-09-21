#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
}

impl From<Value> for i64 {
    fn from(v: Value) -> Self {
        match v {
            Value::Int(i) => i,
            _ => panic!("[tweaker] wrong type"),
        }
    }
}

impl From<Value> for f64 {
    fn from(v: Value) -> Self {
        match v {
            Value::Float(f) => f,
            _ => panic!("[tweaker] wrong type"),
        }
    }
}
