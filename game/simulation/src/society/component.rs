use crate::ecs::*;
use crate::society::registry::SocietyHandle;
use crate::society::Society;
use crate::Societies;
use common::*;

#[derive(Component)]
#[storage(DenseVecStorage)]
pub struct SocietyComponent {
    pub handle: SocietyHandle,
}

impl SocietyComponent {
    pub fn new(handle: SocietyHandle) -> Self {
        Self { handle }
    }

    /// Logs a warning if society is not found
    pub fn resolve<'s>(&self, societies: &'s mut Societies) -> Option<&'s mut Society> {
        societies.society_by_handle_mut(self.handle).or_else(|| {
            warn!("bad society handle in component"; "handle" => ?self.handle);
            None
        })
    }
}
