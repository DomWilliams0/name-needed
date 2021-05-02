use crate::ecs::EcsWorld;
use crate::{
    all_slabs_in_range, InnerWorldRef, Renderer, SlabLocation, ThreadedWorldLoader, WorldViewer,
};
use color::ColorRgb;
use common::*;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::ops::DerefMut;
use unit::world::{WorldPoint, CHUNK_SIZE};
use world::RegionLocation;

pub trait DebugRenderer<R: Renderer> {
    fn identifier(&self) -> &'static str;
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
    map: HashMap<&'static str, (bool, Box<dyn DebugRenderer<R>>)>,
    summary: HashSet<&'static str>,
}

#[derive(Error, Debug)]
pub enum DebugRendererError {
    #[error("'{0}' already registered")]
    AlreadyRegistered(&'static str),
    #[error("No such renderer '{0}'")]
    NoSuchRenderer(&'static str),
}

/// Example renderer that draws lines at the origin along the X and Y axes
pub struct AxesDebugRenderer;

pub struct ChunkBoundariesDebugRenderer;

impl<R: Renderer> DebugRenderers<R> {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            summary: HashSet::with_capacity(32),
        }
    }

    pub fn register<T: DebugRenderer<R> + 'static>(
        &mut self,
        renderer: T,
        enabled: bool,
    ) -> Result<(), DebugRendererError> {
        match self.map.entry(renderer.identifier()) {
            Entry::Occupied(_) => Err(DebugRendererError::AlreadyRegistered(renderer.identifier())),
            Entry::Vacant(e) => {
                debug!("registered debug renderer"; "renderer" => e.key(), "enabled" => enabled);
                e.insert((enabled, Box::new(renderer)));
                Ok(())
            }
        }
    }

    pub fn set_enabled(
        &mut self,
        identifier: &'static str,
        enabled: bool,
    ) -> Result<(), DebugRendererError> {
        self.map
            .get_mut(identifier)
            .map(|(e, _)| {
                *e = enabled;
                debug!("toggled debug renderer"; "renderer" => identifier, "enabled" => enabled);
            })
            .ok_or(DebugRendererError::NoSuchRenderer(identifier))
    }

    pub fn iter_enabled(&mut self) -> impl Iterator<Item = &mut dyn DebugRenderer<R>> {
        self.map.values_mut().filter_map(|(enabled, renderer)| {
            if *enabled {
                Some(renderer.deref_mut() as &mut dyn DebugRenderer<R>)
            } else {
                None
            }
        })
    }

    pub fn summarise(&mut self) -> &HashSet<&'static str> {
        self.summary.clear();
        self.summary
            .extend(self.map.values().filter_map(|(enabled, renderer)| {
                if *enabled {
                    Some(renderer.identifier())
                } else {
                    None
                }
            }));
        &self.summary
    }
}

impl<R: Renderer> DebugRenderer<R> for AxesDebugRenderer {
    fn identifier(&self) -> &'static str {
        "axes"
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
            WorldPoint(0.0, 0.0, 1.0),
            WorldPoint(1.0, 0.0, 1.0),
            ColorRgb::new(255, 0, 0),
        );
        renderer.debug_add_line(
            WorldPoint(0.0, 0.0, 1.0),
            WorldPoint(0.0, 1.0, 1.0),
            ColorRgb::new(0, 255, 0),
        );
    }
}
impl<R: Renderer> DebugRenderer<R> for ChunkBoundariesDebugRenderer {
    fn identifier(&self) -> &'static str {
        "chunk boundaries"
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
