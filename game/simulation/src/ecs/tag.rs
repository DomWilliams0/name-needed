//! Lightweight components that aren't iterated over and so don't need the full power of ECS

use crate::ecs::*;
use common::{Debug, Formatter};

use std::collections::HashMap;

#[derive(Component, EcsComponent, Clone, Default)]
#[name("tags")]
#[interactive]
#[storage(HashMapStorage)]
pub struct TagsComponent {
    // TODO intern these strings
    tags: HashMap<String, TagValue>,
}

#[derive(Copy, Clone, Debug)]
pub enum TagValue {
    None,
    /// 0 <= x <= 100
    Percentage(u8),
}

impl TagsComponent {
    pub fn has(&self, tag: &str) -> bool {
        self.tags.contains_key(tag)
    }

    pub fn get(&self, tag: &str) -> Option<TagValue> {
        self.tags.get(tag).copied()
    }

    /// No value
    pub fn give_simple(&mut self, tag: &str) {
        self.tags.insert(tag.to_owned(), TagValue::None);
    }

    pub fn give(&mut self, tag: &str, value: TagValue) {
        self.tags.insert(tag.to_owned(), value);
    }

    pub fn remove(&mut self, tag: &str) {
        self.tags.remove(tag);
    }
}

impl<V: Value> ComponentTemplate<V> for TagsComponent {
    fn construct(values: &mut Map<V>) -> Result<Box<dyn ComponentTemplate<V>>, ComponentBuildError>
    where
        Self: Sized,
    {
        if let Some(bad) = values.keys().find(|s| s.chars().any(|c| !c.is_lowercase())) {
            return Err(ComponentBuildError::NotLowercase(bad.to_owned()));
        }

        let mut tags = HashMap::with_capacity(values.len());
        for (k, v) in values.take().into_iter() {
            let tag = if v.as_unit().is_ok() {
                TagValue::None
            } else if let Ok(v) = v.as_int() {
                if (0..=100).contains(&v) {
                    TagValue::Percentage(v as u8)
                } else {
                    return Err(ComponentBuildError::BadPercentage(v));
                }
            } else {
                return Err(ComponentBuildError::UnexpectedTagValue(format!("{:?}", v)));
            };
            common::info!("nice {}={:?}", k, tag);
            tags.insert(k, tag);
        }

        Ok(Box::new(TagsComponent { tags }))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        builder.with(self.clone())
    }
}

register_component_template!("tags", TagsComponent);

impl InteractiveComponent for TagsComponent {
    fn as_debug(&self) -> Option<&dyn Debug> {
        Some(self)
    }
}

impl Debug for TagsComponent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut set = f.debug_set();
        for (k, v) in &self.tags {
            match v {
                TagValue::None => set.entry(&format_args!("{}", k)),
                TagValue::Percentage(v) => set.entry(&format_args!("{}={}", k, v)),
            };
        }
        set.finish()
    }
}
