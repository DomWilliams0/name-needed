use crate::ecs::EcsWorld;
use crate::{
    all_slabs_in_range, InnerWorldRef, Renderer, SlabLocation, ThreadedWorldLoader, WorldViewer,
};
use color::Color;
use common::*;

use std::borrow::Cow;
use std::ops::DerefMut;
use unit::space::view::ViewPoint;
use unit::world::{WorldPoint, CHUNK_SIZE};

pub trait DebugRenderer<R: Renderer> {
    fn name(&self) -> &'static str;

    fn render(
        &mut self,
        renderer: &mut R,
        world: &InnerWorldRef,
        world_loader: &ThreadedWorldLoader,
        ecs_world: &EcsWorld,
        viewer: &WorldViewer,
    );

    #[allow(unused_variables)]
    fn on_toggle(&mut self, enabled: bool, world: &EcsWorld) {}

    fn identifier(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

pub struct DebugRenderers<R: Renderer> {
    /// (identifier, (enabled, instance))
    renderers: Vec<(&'static str, bool, Box<dyn DebugRenderer<R>>)>,
    descriptors: Vec<DebugRendererDescriptor>,
}

#[derive(Copy, Clone)]
pub struct DebugRendererDescriptor {
    pub identifier: &'static str,
    pub name: &'static str,
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
}

impl<R: Renderer> DebugRenderersBuilder<R> {
    pub fn register<T: DebugRenderer<R> + Default + 'static>(
        &mut self,
    ) -> Result<(), DebugRendererError> {
        let renderer = T::default();

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
                    name: r.name(),
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

    pub fn disable_all(&mut self, world: &EcsWorld) {
        self.renderers.iter_mut().for_each(|(_, enabled, r)| {
            *enabled = false;
            r.on_toggle(false, world);
        });
    }

    pub fn set_enabled(
        &mut self,
        identifier: Cow<'static, str>,
        enabled: bool,
        world: &EcsWorld,
    ) -> Result<(), DebugRendererError> {
        self.renderers
            .iter_mut()
            .find(|(i, _, _)| *i == identifier)
            .map(|(_, e, r)| {
                *e = enabled;
                r.on_toggle(enabled, world);
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
    fn name(&self) -> &'static str {
        "Axes"
    }

    fn render(
        &mut self,
        renderer: &mut R,
        _: &InnerWorldRef,
        _: &ThreadedWorldLoader,
        _: &EcsWorld,
        _: &WorldViewer,
    ) {
        // each line is 1m long
        let origin = ViewPoint::new_unchecked(0.0, 0.0, 1.0);
        let a = ViewPoint::new_unchecked(1.0, 0.0, 1.0);
        let b = ViewPoint::new_unchecked(0.0, 1.0, 1.0);

        renderer.debug_add_line(origin.into(), a.into(), Color::rgb(255, 0, 0));
        renderer.debug_add_line(origin.into(), b.into(), Color::rgb(0, 255, 0));
    }
}

impl<R: Renderer> DebugRenderer<R> for ChunkBoundariesDebugRenderer {
    fn name(&self) -> &'static str {
        "Chunk boundaries"
    }

    fn render(
        &mut self,
        renderer: &mut R,
        _: &InnerWorldRef,
        _: &ThreadedWorldLoader,
        _: &EcsWorld,
        viewer: &WorldViewer,
    ) {
        #[cfg(feature = "worldprocgen")]
        let mut seen_regions = SmallVec::<[world::RegionLocation; 4]>::new();

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
                Color::rgb(5, 5, 240),
            );

            // collect unique regions
            #[cfg(feature = "worldprocgen")]
            if let Some(region) = world::RegionLocation::try_from_chunk(slab.chunk) {
                if !seen_regions.contains(&region) {
                    seen_regions.push(region)
                };
            }
        }

        #[cfg(feature = "worldprocgen")]
        for region in seen_regions.into_iter() {
            let min = WorldPoint::from(region.chunk_bounds().0.get_block(0));
            let sz = (world::RegionLocation::chunks_per_side() * CHUNK_SIZE.as_usize()) as f32;
            renderer.debug_add_quad(
                [
                    min,
                    min + (sz, 0.0, 0.0),
                    min + (sz, sz, 0.0),
                    min + (0.0, sz, 0.0),
                ],
                Color::rgb(5, 240, 5),
            );
        }
    }
}
