use crate::society::society::Society;
use common::{slog_value_debug, Formatter};
use std::fmt::Debug;
use std::num::NonZeroU32;

// TODO keep society registry sorted by handle for quick lookup

/// World resource to hold society registry
pub struct Societies {
    registry: Vec<(SocietyHandle, Society)>,
    next_handle: SocietyHandle,
}

/// World resource to represent the player's "home" society that they control
#[derive(Default, Clone)]
pub struct PlayerSociety {
    own: Option<SocietyHandle>,
    visibility: SocietyVisibility,
}

#[derive(Copy, Clone)]
pub enum SocietyVisibility {
    All,
    JustOwn,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct SocietyHandle(NonZeroU32);

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
        if self.society_by_name(&name).is_some() {
            return None;
        }

        let handle = self.next_handle;
        self.next_handle.0 = unsafe { NonZeroU32::new_unchecked(self.next_handle.0.get() + 1) };

        let society = Society::with_name(handle, name);
        self.registry.push((handle, society));

        Some(handle)
    }

    pub fn society_by_name(&mut self, name: &str) -> Option<&Society> {
        self.registry
            .iter()
            .find(|(_, s)| s.name() == name)
            .map(|(handle, society)| ensure_handle!(society, *handle))
    }

    /// Fallible in case societies can be removed
    pub fn society_by_handle_mut(&mut self, handle: SocietyHandle) -> Option<&mut Society> {
        self.registry
            .iter_mut()
            .find(|(h, _)| *h == handle)
            .map(|(handle, society)| ensure_handle!(society, *handle))
    }

    /// Fallible in case societies can be removed
    pub fn society_by_handle(&self, handle: SocietyHandle) -> Option<&Society> {
        self.registry
            .iter()
            .find(|(h, _)| *h == handle)
            .map(|(handle, society)| ensure_handle!(society, *handle))
    }

    pub fn iter(&self) -> impl Iterator<Item = &Society> + '_ {
        self.registry.iter().map(|(_, s)| s)
    }
}

impl Default for Societies {
    fn default() -> Self {
        Self {
            registry: Vec::with_capacity(8),
            next_handle: SocietyHandle(unsafe { NonZeroU32::new_unchecked(100) }),
        }
    }
}

impl PartialEq<SocietyHandle> for PlayerSociety {
    fn eq(&self, other: &SocietyHandle) -> bool {
        if let SocietyVisibility::All = self.visibility {
            true
        } else if let Some(me) = self.own {
            me == *other
        } else {
            false
        }
    }
}

impl PartialEq<Option<SocietyHandle>> for PlayerSociety {
    fn eq(&self, other: &Option<SocietyHandle>) -> bool {
        if let SocietyVisibility::All = self.visibility {
            true
        } else if let Some((me, other)) = self.own.zip(*other) {
            me == other
        } else {
            false
        }
    }
}

impl PlayerSociety {
    pub fn with_society(soc: SocietyHandle) -> Self {
        Self {
            own: Some(soc),
            ..Self::default()
        }
    }

    /// Don't use for visibility checking
    pub fn get(&self) -> Option<SocietyHandle> {
        self.own
    }

    pub fn has(&self) -> bool {
        matches!(self.visibility, SocietyVisibility::All) || self.own.is_some()
    }

    pub fn set_visibility(&mut self, vis: SocietyVisibility) {
        self.visibility = vis
    }
}

impl Default for SocietyVisibility {
    fn default() -> Self {
        Self::JustOwn
    }
}

impl Debug for SocietyHandle {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "SocietyHandle({})", self.0)
    }
}

slog_value_debug!(SocietyHandle);
