use common::*;

use crate::ecs::*;
use crate::society::registry::SocietyHandle;
use crate::society::Society;
use crate::Societies;

#[derive(Component, EcsComponent, Clone)]
#[storage(DenseVecStorage)]
#[name("society")]
pub struct SocietyComponent(SocietyHandle);

impl SocietyComponent {
    pub fn new(handle: SocietyHandle) -> Self {
        Self(handle)
    }

    pub const fn handle(&self) -> SocietyHandle {
        self.0
    }

    /// Logs a warning if society is not found
    pub fn resolve<'s>(&self, societies: &'s Societies) -> Option<&'s Society> {
        societies.society_by_handle(self.0).or_else(|| {
            warn!("bad society handle in component"; "handle" => ?self.handle());
            None
        })
    }
}
