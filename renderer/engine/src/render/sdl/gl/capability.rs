use gl::types::*;

pub struct ScopedCapability {
    cap: GLenum,
    enable: bool,
}

pub enum Capability {
    DebugOutput,
    CullFace,
    DepthTest,
    LineSmooth,
    Blend,
}

impl From<Capability> for GLenum {
    fn from(cap: Capability) -> Self {
        match cap {
            Capability::DebugOutput => gl::DEBUG_OUTPUT,
            Capability::CullFace => gl::CULL_FACE,
            Capability::DepthTest => gl::DEPTH_TEST,
            Capability::LineSmooth => gl::LINE_SMOOTH,
            Capability::Blend => gl::BLEND,
        }
    }
}

impl Capability {
    pub fn scoped_enable(self) -> ScopedCapability {
        ScopedCapability::new(self, true)
    }
    pub fn scoped_disable(self) -> ScopedCapability {
        ScopedCapability::new(self, false)
    }

    pub fn enable(self) {
        unsafe { gl::Enable(self.into()) }
    }

    pub fn disable(self) {
        unsafe { gl::Disable(self.into()) }
    }
}

impl ScopedCapability {
    fn new(cap: Capability, enable: bool) -> Self {
        let cap = cap.into();
        unsafe {
            if enable {
                gl::Enable(cap);
            } else {
                gl::Disable(cap);
            }
        }

        Self { cap, enable }
    }
}

impl Drop for ScopedCapability {
    fn drop(&mut self) {
        unsafe {
            if self.enable {
                gl::Disable(self.cap);
            } else {
                gl::Enable(self.cap);
            }
        }
    }
}
