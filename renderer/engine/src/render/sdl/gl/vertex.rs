use std::cell::Cell;
use std::ops::{Deref, DerefMut};
use std::ptr::null;

use gl::types::*;

use common::*;

use crate::errchk;
use crate::render::sdl::gl::{GlError, GlResult};

// TODO dont bother unbinding?

#[derive(Copy, Clone)]
pub enum AttribType {
    Float32,
    UByte,
}

#[derive(Copy, Clone)]
pub enum Normalized {
    FixedPoint,
    Normalized,
}

pub enum Divisor {
    /// glVertexAttribDivisor(0)
    PerVertex,
    /// glVertexAttribDivisor(n)
    PerInstances(u32),
}

impl From<AttribType> for GLenum {
    fn from(a: AttribType) -> Self {
        match a {
            AttribType::Float32 => gl::FLOAT,
            AttribType::UByte => gl::UNSIGNED_BYTE,
        }
    }
}

impl From<Divisor> for GLuint {
    fn from(divisor: Divisor) -> Self {
        match divisor {
            Divisor::PerVertex => 0,
            Divisor::PerInstances(i) => i as Self,
        }
    }
}

impl From<Normalized> for GLboolean {
    fn from(normalized: Normalized) -> Self {
        match normalized {
            Normalized::FixedPoint => gl::FALSE,
            Normalized::Normalized => gl::TRUE,
        }
    }
}

impl AttribType {
    pub fn byte_size(self, count: u32) -> u32 {
        let one = match self {
            AttribType::Float32 => 4,
            AttribType::UByte => 1,
        };

        one * count
    }

    pub fn size(self) -> u32 {
        self.byte_size(1)
    }
}

#[derive(Clone)]
pub struct Vao(GLuint);

impl Vao {
    pub fn new() -> Self {
        let mut vao = 0;
        unsafe {
            gl::GenVertexArrays(1, &mut vao);
        }
        Self(vao)
    }
}

impl Drop for Vao {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteVertexArrays(1, &self.0);
        }
    }
}

impl Bindable for Vao {
    unsafe fn bind(&self) {
        gl::BindVertexArray(self.0);
    }

    unsafe fn unbind(&self) {
        gl::BindVertexArray(0);
    }
}

impl<'a> ScopedBind<'a, Vao> {
    pub fn vertex_attribs(&self) -> SimpleVertexAttribBuilder {
        SimpleVertexAttribBuilder::new()
    }

    pub fn vertex_attribs_manual(&self) -> ManualVertexAttribBuilder {
        ManualVertexAttribBuilder
    }
}

#[derive(Clone, Copy)]
enum VboBind {
    ArrayBuffer,
    ElementArrayBuffer,
}

impl From<VboBind> for GLenum {
    fn from(b: VboBind) -> Self {
        match b {
            VboBind::ArrayBuffer => gl::ARRAY_BUFFER,
            VboBind::ElementArrayBuffer => gl::ELEMENT_ARRAY_BUFFER,
        }
    }
}

#[derive(Clone)]
pub struct Vbo {
    obj: GLuint,
    /// Bytes
    len: Cell<usize>,

    bind: VboBind,
}

impl Vbo {
    fn new(bind: VboBind) -> Self {
        let mut obj = 0;
        unsafe {
            gl::GenBuffers(1, &mut obj as *mut GLuint);
        }

        Self {
            obj,
            len: Cell::new(0),
            bind,
        }
    }

    pub fn array_buffer() -> Self {
        Self::new(VboBind::ArrayBuffer)
    }

    pub fn index_buffer() -> Self {
        Self::new(VboBind::ElementArrayBuffer)
    }
}

impl Drop for Vbo {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteBuffers(1, &self.obj as *const _);
        }
    }
}

impl Bindable for Vbo {
    unsafe fn bind(&self) {
        gl::BindBuffer(self.bind.into(), self.obj);
    }

    unsafe fn unbind(&self) {
        gl::BindBuffer(self.bind.into(), 0);
    }
}

pub enum BufferUsage {
    StaticDraw,
    DynamicDraw,
    StreamDraw,
}

impl From<BufferUsage> for GLenum {
    fn from(usage: BufferUsage) -> Self {
        match usage {
            BufferUsage::StaticDraw => gl::STATIC_DRAW,
            BufferUsage::DynamicDraw => gl::DYNAMIC_DRAW,
            BufferUsage::StreamDraw => gl::STREAM_DRAW,
        }
    }
}

#[derive(Copy, Clone)]
pub enum Primitive {
    Triangles,
    Lines,
    TriangleStrip,
}

impl From<Primitive> for GLenum {
    fn from(primitive: Primitive) -> Self {
        match primitive {
            Primitive::Triangles => gl::TRIANGLES,
            Primitive::Lines => gl::LINES,
            Primitive::TriangleStrip => gl::TRIANGLE_STRIP,
        }
    }
}

impl<'a> ScopedBind<'a, Vbo> {
    pub fn buffer_data<T: Sized>(&self, buf: &[T], usage: BufferUsage) -> GlResult<()> {
        unsafe {
            let ptr = if buf.is_empty() { null() } else { buf.as_ptr() };
            let len = std::mem::size_of::<T>() * buf.len();

            errchk!(gl::BufferData(
                self.bind.into(),
                len as isize,
                ptr as *const _,
                usage.into(),
            ))
            .map(|_| self.len.set(len))
        }
    }

    pub fn buffer_sub_data<T: Sized>(&self, offset: usize, buf: &[T]) -> GlResult<()> {
        let len = std::mem::size_of::<T>() * buf.len();
        let offset = std::mem::size_of::<T>() * offset;

        if offset + len > self.len.get() {
            return Err(GlError::BufferTooSmall {
                real_len: self.len.get(),
                requested_len: offset + len,
            });
        }

        unsafe {
            let ptr = if buf.is_empty() { null() } else { buf.as_ptr() };

            errchk!(gl::BufferSubData(
                self.bind.into(),
                offset as GLintptr,
                len as GLsizeiptr,
                ptr as *const _,
            ))
        }
    }

    pub fn buffer_data_uninitialized<T: Sized>(
        &self,
        len: usize,
        usage: BufferUsage,
    ) -> GlResult<()> {
        unsafe {
            let len = std::mem::size_of::<T>() * len;

            errchk!(gl::BufferData(
                self.bind.into(),
                len as isize,
                null(),
                usage.into(),
            ))
            .map(|_| self.len.set(len))
        }
    }

    pub fn draw_array(&self, primitive: Primitive) {
        unsafe {
            gl::DrawArrays(primitive.into(), 0, self.len.get() as GLint);
        }
    }

    /*
    pub fn draw_array_instanced(
        &self,
        primitive: Primitive,
        first: usize,
        vertex_count: usize,
        instance_count: usize,
    ) -> GlResult<()> {
        if first + vertex_count >= self.len.get() {
            return Err(GlError::BufferTooSmall {
                real_len: self.len.get(),
                requested_len: first + vertex_count,
            });
        }
        unsafe {
            errchk!(gl::DrawArraysInstanced(
                primitive.into(),
                first as GLint,
                vertex_count as GLsizei,
                instance_count as GLsizei,
            ))
        }
    }
    */

    /// Assumes indices are u16
    pub fn draw_elements_instanced(
        &self,
        primitive: Primitive,
        start_ptr: usize,
        element_count: usize,
        instance_start: usize,
        instance_count: usize,
    ) -> GlResult<()> {
        unsafe {
            errchk!(gl::DrawElementsInstancedBaseInstance(
                primitive.into(),
                element_count as GLsizei,
                gl::UNSIGNED_SHORT,
                (start_ptr * std::mem::size_of::<u16>()) as *const _,
                instance_count as GLsizei,
                instance_start as GLuint,
            ))
        }
    }

    pub fn map_write_only<T>(&self) -> GlResult<Option<ScopedMapMut<T>>> {
        if self.len.get() == 0 {
            return Ok(None);
        }

        unsafe {
            let sizeof = std::mem::size_of::<T>();
            let count = self.len.get() / sizeof;
            debug_assert_eq!(self.len.get() % sizeof, 0);

            let ptr = errchk!(gl::MapBuffer(self.bind.into(), gl::WRITE_ONLY))? as *mut T;
            debug_assert!(!ptr.is_null());

            Ok(Some(ScopedMapMut {
                ptr,
                len: count,
                bind: self.bind,
            }))
        }
    }

    /*
    pub fn replace<'b>(self, other: &'b Vbo) -> ScopedBind<'b, Vbo> {
        std::mem::forget(self);
        other.scoped_bind()
    }
    */
}

pub trait Bindable {
    unsafe fn bind(&self);
    unsafe fn unbind(&self);
}

pub struct ScopedBind<'a, T: Bindable>(&'a T);

impl<'a, T: Bindable> ScopedBind<'a, T> {
    fn new(obj: &'a T) -> Self {
        unsafe { obj.bind() };
        Self(obj)
    }
}
impl<'a, T: Bindable> Drop for ScopedBind<'a, T> {
    fn drop(&mut self) {
        unsafe { self.0.unbind() };
    }
}

impl<'a, T: Bindable> Deref for ScopedBind<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

pub trait ScopedBindable: Bindable + Sized {
    fn scoped_bind(&self) -> ScopedBind<Self> {
        ScopedBind::new(self)
    }
}

impl<T: Bindable> ScopedBindable for T {}

pub struct SimpleVertexAttribBuilder {
    // TODO smallvec
    attribs: Vec<(u32, AttribType, Normalized)>,
}

impl SimpleVertexAttribBuilder {
    fn new() -> Self {
        Self {
            attribs: Vec::with_capacity(8),
        }
    }

    pub fn add(mut self, size: u32, attrib_type: AttribType, normalized: Normalized) -> Self {
        self.attribs.push((size, attrib_type, normalized));
        self
    }

    pub fn build(self) -> GlResult<()> {
        let stride: u32 = self
            .attribs
            .iter()
            .map(|(count, atype, _)| atype.byte_size(*count))
            .sum();
        let mut offset = 0;

        for (i, &(count, atype, normalized)) in self.attribs.iter().enumerate() {
            let normalized = if let Normalized::Normalized = normalized {
                gl::TRUE
            } else {
                gl::FALSE
            };
            let index = i as GLuint;

            unsafe {
                gl::EnableVertexAttribArray(index);
                gl::VertexAttribPointer(
                    index,
                    count as GLint,
                    atype.into(),
                    normalized,
                    stride as GLint,
                    offset as *const _,
                );
                errchk!(())?
            }
            offset += atype.byte_size(count);
        }

        Ok(())
    }
}

pub struct ManualVertexAttribBuilder;

impl ManualVertexAttribBuilder {
    #[allow(clippy::too_many_arguments)]
    pub fn attrib(
        self,
        index: u32,
        count: u32,
        type_: AttribType,
        normalized: Normalized,
        divisor: Divisor,
        stride: u32,
        offset: u32,
    ) -> GlResult<Self> {
        unsafe {
            gl::EnableVertexAttribArray(index);
            gl::VertexAttribDivisor(index, divisor.into());
            gl::VertexAttribPointer(
                index,
                count as GLint,
                type_.into(),
                normalized.into(),
                stride as GLint,
                offset as *const _,
            );
            errchk!(self)
        }
    }

    pub fn attrib_matrix(
        self,
        start_index: u32,
        normalized: Normalized,
        divisor: Divisor,
        stride: u32,
        start_offset: u32,
    ) -> GlResult<Self> {
        unsafe {
            let divisor = divisor.into();
            for i in 0..4 {
                let index = start_index + i;
                let offset = start_offset + AttribType::Float32.byte_size(i * 4);

                gl::EnableVertexAttribArray(index);
                gl::VertexAttribDivisor(index, divisor);
                gl::VertexAttribPointer(
                    index,
                    4,
                    AttribType::Float32.into(),
                    normalized.into(),
                    stride as GLint,
                    offset as *const _,
                );
                errchk!(())?;
            }

            errchk!(self)
        }
    }
}

pub struct ScopedMapMut<T> {
    ptr: *mut T,
    /// Number of T
    len: usize,
    bind: VboBind,
}

impl<T> Deref for ScopedMapMut<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }
}
impl<T> DerefMut for ScopedMapMut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

impl<T> Drop for ScopedMapMut<T> {
    fn drop(&mut self) {
        unsafe {
            gl::UnmapBuffer(self.bind.into());
            if let Err(e) = errchk!(()) {
                warn!("glUnmapBuffer failed"; "error" => %e);
            }
        }
    }
}
