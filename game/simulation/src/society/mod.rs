mod component;
pub mod job;
mod registry;
mod society;

pub use self::registry::{PlayerSociety, Societies, SocietyHandle};
pub use self::society::Society;
pub use component::SocietyComponent;
