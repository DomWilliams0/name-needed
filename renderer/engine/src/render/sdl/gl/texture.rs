use gl::types::*;
use std::ptr::null;

use common::*;

use crate::errchk;
use crate::render::sdl::gl::GlResult;

pub struct Texture(GLuint);

/// Won't delete texture on drop
pub struct BorrowedTexture(GLuint);

pub struct BoundTexture<'a>(GLuint, PhantomData<&'a ()>);

impl Texture {
    pub fn new_2d(w: u32, h: u32) -> GlResult<Self> {
        let mut name = 0;
        unsafe {
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
            gl::GenTextures(1, &mut name);
            gl::BindTexture(gl::TEXTURE_2D, name);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as _);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as _);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as _);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as _);
            errchk!(gl::TexImage2D(
                gl::TEXTURE_2D,
                0, // LOD
                gl::RED as _,
                w as _,
                h as _,
                0, // reserved
                gl::RED,
                gl::UNSIGNED_BYTE,
                std::ptr::null(),
            ))?;
        }

        Ok(Self(name))
    }

    pub fn borrow(&self) -> BorrowedTexture {
        BorrowedTexture(self.0)
    }
}

impl BorrowedTexture {
    pub fn bind(&self) -> BoundTexture {
        unsafe { gl::BindTexture(gl::TEXTURE_2D, self.0) };
        BoundTexture(self.0, PhantomData)
    }
}

impl BoundTexture<'_> {
    pub fn sub_image(
        &self,
        xy_offset: [u32; 2],
        width_height: [u32; 2],
        data: &[u8],
    ) -> GlResult<()> {
        let ptr = if data.is_empty() {
            null()
        } else {
            data.as_ptr()
        };

        unsafe {
            errchk!(gl::TexSubImage2D(
                gl::TEXTURE_2D,
                0,
                xy_offset[0] as _,
                xy_offset[1] as _,
                width_height[0] as _,
                width_height[1] as _,
                gl::RED,
                gl::UNSIGNED_BYTE,
                ptr as _,
            ))
        }
    }

    pub fn clear(&self) -> GlResult<()> {
        unsafe {
            errchk!(gl::ClearTexImage(
                self.0,
                0, // LOD
                gl::RED,
                gl::UNSIGNED_BYTE,
                null() as *const _,
            ))
        }
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        unsafe { gl::DeleteTextures(1, &self.0) }
    }
}
