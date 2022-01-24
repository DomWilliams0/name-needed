use std::fmt::{Display, Formatter};
use std::num::NonZeroU16;
use std::rc::Rc;

use serde::Deserialize;

use world::block::BlockType;

use crate::ecs::*;
use crate::string::StringCache;
use crate::BuildMaterial;

#[derive(Debug)]
pub struct BuildTemplate {
    materials: Vec<BuildMaterial>,
    steps: u32,
    rate: u32,
    output: BlockType,
}

impl BuildTemplate {
    pub const fn output(&self) -> BlockType {
        self.output
    }

    /// (number of steps required, ticks to sleep between each step)
    pub const fn progression(&self) -> (u32, u32) {
        (self.steps, self.rate)
    }

    pub fn materials(&self) -> &[BuildMaterial] {
        &self.materials
    }

    pub fn supports_outline(&self) -> bool {
        true // TODO
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn new(materials: Vec<BuildMaterial>, steps: u32, rate: u32, output: BlockType) -> Self {
        Self {
            materials,
            steps,
            rate,
            output,
        }
    }
}

impl<V: Value> ComponentTemplate<V> for BuildTemplate {
    fn construct(
        values: &mut Map<V>,
        string_cache: &StringCache,
    ) -> Result<Rc<dyn ComponentTemplate<V>>, ComponentBuildError>
    where
        Self: Sized,
    {
        #[derive(Deserialize, Debug)]
        struct Material(String, u16);

        let materials = {
            let raw: Vec<Material> = values.get("materials").and_then(|val| val.into_type())?;
            let mut materials = Vec::with_capacity(raw.len());
            for mat in raw {
                let n = NonZeroU16::new(mat.1).ok_or_else(|| {
                    ComponentBuildError::TemplateSpecific(format!(
                        "material count for {:?} cannot be zero",
                        mat.0
                    ))
                })?;
                materials.push(BuildMaterial::new(string_cache.get(&mat.0), n))
            }
            materials
        };

        let steps = values.get_int("steps")?;
        let rate = values.get_int("rate")?;
        let output = {
            let name = values.get_string("output")?;
            name.parse::<BlockType>().map_err(|_| {
                ComponentBuildError::TemplateSpecific(format!("invalid block type {:?}", name))
            })?
        };

        Ok(Rc::new(BuildTemplate {
            materials,
            steps,
            rate,
            output,
        }))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        builder
    }

    crate::as_any!();
}

register_component_template!("build", BuildTemplate);
