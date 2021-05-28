use crate::ecs::EcsWorld;
use crate::{
    all_slabs_in_range, InnerWorldRef, Renderer, SlabLocation, ThreadedWorldLoader, WorldViewer,
};
use color::ColorRgb;
use common::*;

use std::borrow::Cow;
use std::ffi::CStr;
use std::ops::DerefMut;
use unit::world::{WorldPoint, CHUNK_SIZE};
use world::RegionLocation;

pub trait DebugRenderer<R: Renderer> {
    fn identifier(&self) -> &'static str;

    /// Must be null terminated, is checked during registration
    fn name(&self) -> &'static str;

    fn render(
        &mut self,
        renderer: &mut R,
        world: &InnerWorldRef,
        world_loader: &ThreadedWorldLoader,
        ecs_world: &EcsWorld,
        viewer: &WorldViewer,
    );
}

pub struct DebugRenderers<R: Renderer> {
    /// (identifier, (enabled, instance))
    renderers: Vec<(&'static str, bool, Box<dyn DebugRenderer<R>>)>,
    descriptors: Vec<DebugRendererDescriptor>,
}

#[derive(Copy, Clone)]
pub struct DebugRendererDescriptor {
    pub identifier: &'static str,
    /// Must be valid utf8 and null terminated for UI to render
    pub name: &'static CStr,
}

#[repr(transparent)]
pub struct DebugRenderersState([DebugRendererDescriptor]);

/// Collects registrations during setup. Can't use `inventory` crate due to generic param
pub struct DebugRenderersBuilder<R: Renderer>(Vec<(&'static str, Box<dyn DebugRenderer<R>>)>);

#[derive(Error, Debug)]
pub enum DebugRendererError {
    #[error("'{0}' already registered")]
    AlreadyRegistered(&'static str),

    #[error("No such renderer '{0}'")]
    NoSuchRenderer(Cow<'static, str>),

    #[error("Renderer name is not null-terminated: '{0}'")]
    BadName(&'static str),
}

impl<R: Renderer> DebugRenderersBuilder<R> {
    pub fn register<T: DebugRenderer<R> + Default + 'static>(
        &mut self,
    ) -> Result<(), DebugRendererError> {
        let renderer = T::default();

        if CStr::from_bytes_with_nul(renderer.name().as_bytes()).is_err() {
            return Err(DebugRendererError::BadName(renderer.name()));
        }

        let ident = renderer.identifier();
        if self.0.iter().any(|(i, _)| *i == ident) {
            Err(DebugRendererError::AlreadyRegistered(ident))
        } else {
            debug!("registered debug renderer"; "identifier" => renderer.identifier());
            self.0.push((ident, Box::new(renderer)));
            Ok(())
        }
    }

    pub fn build(self) -> DebugRenderers<R> {
        DebugRenderers {
            descriptors: self
                .0
                .iter()
                .map(|(ident, r)| DebugRendererDescriptor {
                    identifier: *ident,
                    // safety: checked during registration
                    name: unsafe { CStr::from_bytes_with_nul_unchecked(r.name().as_bytes()) },
                })
                .collect(),
            renderers: self
                .0
                .into_iter()
                .map(|(ident, r)| (ident, false, r))
                .collect(),
        }
    }
}

impl<R: Renderer> DebugRenderers<R> {
    pub fn builder() -> DebugRenderersBuilder<R> {
        DebugRenderersBuilder(Vec::with_capacity(64))
    }

    pub fn disable_all(&mut self) {
        self.renderers
            .iter_mut()
            .for_each(|(_, enabled, _)| *enabled = false);
    }

    pub fn set_enabled(
        &mut self,
        identifier: Cow<'static, str>,
        enabled: bool,
    ) -> Result<(), DebugRendererError> {
        self.renderers
            .iter_mut()
            .find(|(i, _, _)| *i == identifier)
            .map(|(_, e, _)| {
                *e = enabled;
                debug!("toggled debug renderer"; "renderer" => %identifier, "enabled" => enabled);
            })
            .ok_or(DebugRendererError::NoSuchRenderer(identifier))
    }

    pub fn iter_enabled(&mut self) -> impl Iterator<Item = &mut dyn DebugRenderer<R>> {
        self.renderers
            .iter_mut()
            .filter_map(|(_, enabled, renderer)| {
                if *enabled {
                    Some(renderer.deref_mut() as &mut dyn DebugRenderer<R>)
                } else {
                    None
                }
            })
    }

    pub fn state(&self) -> &DebugRenderersState {
        let slice: &[DebugRendererDescriptor] = &self.descriptors[..];
        // safety: repr transparent
        unsafe { &*(slice as *const _ as *const DebugRenderersState) }
    }
}

impl DebugRenderersState {
    pub fn iter_descriptors(&self) -> impl Iterator<Item = DebugRendererDescriptor> + '_ {
        self.0.iter().copied()
    }
}

/// Example renderer that draws lines at the origin along the X and Y axes
#[derive(Default)]
pub struct AxesDebugRenderer;

/// Draws lines along region and chunk boundaries
#[derive(Default)]
pub struct ChunkBoundariesDebugRenderer;

impl<R: Renderer> DebugRenderer<R> for AxesDebugRenderer {
    fn identifier(&self) -> &'static str {
        "axes"
    }

    fn name(&self) -> &'static str {
        "Axes\0"
    }

    fn render(
        &mut self,
        renderer: &mut R,
        _: &InnerWorldRef,
        _: &ThreadedWorldLoader,
        _: &EcsWorld,
        _: &WorldViewer,
    ) {
        renderer.debug_add_line(
            WorldPoint::new_unchecked(0.0, 0.0, 1.0),
            WorldPoint::new_unchecked(1.0, 0.0, 1.0),
            ColorRgb::new(255, 0, 0),
        );
        renderer.debug_add_line(
            WorldPoint::new_unchecked(0.0, 0.0, 1.0),
            WorldPoint::new_unchecked(0.0, 1.0, 1.0),
            ColorRgb::new(0, 255, 0),
        );
    }
}
impl<R: Renderer> DebugRenderer<R> for ChunkBoundariesDebugRenderer {
    fn identifier(&self) -> &'static str {
        "chunk boundaries"
    }

    fn name(&self) -> &'static str {
        "Chunk boundaries\0"
    }

    fn render(
        &mut self,
        renderer: &mut R,
        _: &InnerWorldRef,
        _: &ThreadedWorldLoader,
        _: &EcsWorld,
        viewer: &WorldViewer,
    ) {
        let mut seen_regions = SmallVec::<[RegionLocation; 4]>::new();

        let (from, to) = viewer.chunk_range();
        for slab in all_slabs_in_range(SlabLocation::new(0, from), SlabLocation::new(0, to)).0 {
            let min = WorldPoint::from(slab.chunk.get_block(0));
            let sz = CHUNK_SIZE.as_f32();
            renderer.debug_add_quad(
                [
                    min,
                    min + (sz, 0.0, 0.0),
                    min + (sz, sz, 0.0),
                    min + (0.0, sz, 0.0),
                ],
                ColorRgb::new(5, 5, 240),
            );

            // collect unique regions
            if let Some(region) = RegionLocation::try_from_chunk(slab.chunk) {
                if !seen_regions.contains(&region) {
                    seen_regions.push(region)
                };
            }
        }

        for region in seen_regions.into_iter() {
            let min = WorldPoint::from(region.chunk_bounds().0.get_block(0));
            let sz = (RegionLocation::chunks_per_side() * CHUNK_SIZE.as_usize()) as f32;
            renderer.debug_add_quad(
                [
                    min,
                    min + (sz, 0.0, 0.0),
                    min + (sz, sz, 0.0),
                    min + (0.0, sz, 0.0),
                ],
                ColorRgb::new(5, 240, 5),
            );
        }
    }
}
