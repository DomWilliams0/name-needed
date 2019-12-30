#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(clippy::all)] // autogenerated

pub use bulletc::*;

mod bulletc;

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn bullet_hello_world() {
        unsafe {
            hello_world_example();
        }
    }
}