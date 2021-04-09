use crate::ecs::EcsWorld;
use crate::{InnerWorldRef, Renderer, ThreadedWorldLoader, WorldViewer};
use color::ColorRgb;
use common::*;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::ops::DerefMut;
use unit::world::WorldPoint;

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
