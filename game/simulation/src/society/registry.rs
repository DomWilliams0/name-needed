use crate::society::society::Society;

// TODO keep society registry sorted by handle for quick lookup

/// World resource to hold society registry
pub struct Societies {
    registry: Vec<(SocietyHandle, Society)>,
    next_handle: SocietyHandle,
}

/// World resource to represent the player's "home" society that they control
#[derive(Default, Clone)]
pub struct PlayerSociety(pub Option<SocietyHandle>);

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct SocietyHandle(u32);

macro_rules! ensure_handle {
    ($soc:expr, $handle:expr) => {{
        debug_assert_eq!(
            $soc.handle(),
            $handle,
            "expected society handle {:?} does not match actual {:?} ({:?})",
            $handle,
            $soc.handle(),
            $soc
        );
        $soc
    }};
}

impl Societies {
    pub fn new_society(&mut self, name: String) -> Option<SocietyHandle> {
        if self.society_by_name_mut(&name).is_some() {
            return None;
        }

        let handle = self.next_handle;
        self.next_handle.0 += 1;

        let society = Society::with_name(handle, name);
        self.registry.push((handle, society));

        Some(handle)
    }

    pub fn society_by_name_mut(&mut self, name: &str) -> Option<&mut Society> {
        self.registry
            .iter_mut()
            .find(|(_, s)| s.name() == name)
            .map(|(handle, society)| ensure_handle!(society, *handle))
    }

    pub fn society_by_handle_mut(&mut self, handle: SocietyHandle) -> Option<&mut Society> {
        self.registry
            .iter_mut()
            .find(|(h, _)| *h == handle)
            .map(|(handle, society)| ensure_handle!(society, *handle))
    }

    pub fn society_by_handle(&self, handle: SocietyHandle) -> Option<&Society> {
        self.registry
            .iter()
            .find(|(h, _)| *h == handle)
            .map(|(handle, society)| ensure_handle!(society, *handle))
    }
}

impl Default for Societies {
    fn default() -> Self {
        Self {
            registry: Vec::with_capacity(8),
            next_handle: SocietyHandle(100),
        }
    }
}

impl PartialEq<SocietyHandle> for PlayerSociety {
    fn eq(&self, other: &SocietyHandle) -> bool {
        self.0.map(|me| me == *other).unwrap_or(false)
    }
}
