#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
// include!(concat!(env!("OUT_DIR"), "/bulletc.rs"));
mod bulletc;
pub use bulletc::*;

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
