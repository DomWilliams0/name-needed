use std::collections::{HashMap, HashSet};
use std::ops::Deref;

use daggy::petgraph::visit::{EdgeRef, IntoEdgesDirected};
use daggy::petgraph::Direction;
use daggy::{Dag, Walker};
use serde::de::Error;

use common::derive_more::IntoIterator;
use common::*;

use crate::definitions::loader::step1_deserialization::{Components, DeserializedDefinition};
use crate::definitions::{DefinitionErrorKind, DefinitionErrors};

#[derive(Debug, Clone, Default, IntoIterator)]
pub struct ProcessedComponents(Vec<(String, ComponentFields)>);

#[derive(Debug, Clone, Default)]
pub struct Fields(Vec<(String, ron::Value)>);

#[derive(Debug, Clone)]
pub enum ComponentFields {
    Fields(Vec<(String, ron::Value)>),
    Unit,
    Negate,
}

impl Components {
    pub fn into_processed(self) -> Result<ProcessedComponents, Vec<DefinitionErrorKind>> {
        let mut errors = Vec::new();
        let components: Vec<(String, ComponentFields)> = self
            .into_iter()
            .filter_map(|comp| {
                let ret = extract_map(comp)
                    .and_then(|mut map| {
                        let key_value = map.iter_mut().next().ok_or_else(|| {
                            DefinitionErrorKind::Format(ron::Error::custom(
                                "no nested map for component",
                            ))
                        });

                        key_value.and_then(|(key, value)| {
                            // cant move key out of ron::Map
                            let comp_name = extract_string_ref(key)?.to_owned();
                            let raw_fields = std::mem::replace(value, ron::Value::Unit);
                            Ok((comp_name, raw_fields))
                        })
                    })
                    .map_err(|e| vec![e]);

                let ret = ret.and_then(|(comp_name, fields)| {
                    let fields = ComponentFields::from_map(fields)?;
                    Ok((comp_name, fields))
                });

                match ret {
                    Err(e) => {
                        errors.extend(e);
                        None
                    }
                    Ok(comp) => Some(comp),
                }
            })
            .collect();

        // disallow dups
        let mut dup_detection = HashSet::with_capacity(components.len());
        for (name, _) in components.iter() {
            if !dup_detection.insert(name.as_str()) {
                errors.push(DefinitionErrorKind::DuplicateComponent(name.to_owned()));
            }
        }

        if errors.is_empty() {
            Ok(ProcessedComponents(components))
        } else {
            Err(errors)
        }
    }
}

impl ProcessedComponents {
    fn override_with(&mut self, other: &Self) {
        for (comp_name, comp_fields) in other.0.iter() {
            log_scope!(o!("component" => comp_name.to_owned())); // dirty clone
            debug!("overriding");

            if let Some(my_fields) = self.component(comp_name) {
                debug!("combining fields");
                my_fields.override_with(comp_fields);
            } else {
                // add component unchanged from other
                debug!("adding unchanged inherited component");
                self.add_component(comp_name.to_owned(), comp_fields.to_owned());
            }
        }
    }

    fn clean_negated(&mut self) {
        // remove all negated components
        self.0
            .retain(|(_, fields)| !matches!(fields, ComponentFields::Negate));
    }

    fn add_component(&mut self, name: String, fields: ComponentFields) {
        self.0.push((name, fields));
    }

    fn component(&mut self, name: &str) -> Option<&mut ComponentFields> {
        self.0
            .iter_mut()
            .find(|(comp_name, _)| comp_name == name)
            .map(|(_, fields)| fields)
    }
}
impl ComponentFields {
    pub fn from_map(value: ron::Value) -> Result<Self, Vec<DefinitionErrorKind>> {
        let mut errors = Vec::new();

        let ret = match value {
            ron::Value::Map(mut map) => {
                let fields = map
                    .iter_mut()
                    .filter_map(|(k, v)| {
                        let field_name = match extract_string_ref(k) {
                            Err(e) => {
                                errors.push(e);
                                return None;
                            }
                            Ok(s) => s,
                        };

                        let v = std::mem::replace(v, ron::Value::Unit);
                        Some((field_name.to_owned(), v))
                    })
                    .collect();
                Some(ComponentFields::Fields(fields))
            }
            ron::Value::Option(opt) => {
                if let Some(val) = opt {
                    match Self::from_map(*val) {
                        Err(errs) => {
                            errors.extend(errs);
                            None
                        }
                        Ok(ret) => Some(ret),
                    }
                } else {
                    Some(ComponentFields::Negate)
                }
            }
            ron::Value::Unit => Some(ComponentFields::Unit),
            _ => {
                errors.push(DefinitionErrorKind::Format(ron::Error::custom(
                    "invalid component body",
                )));
                None
            }
        };

        if errors.is_empty() {
            Ok(ret.unwrap())
        } else {
            Err(errors)
        }
    }

    // fn iter(&self) -> impl Iterator<Item=(&ron::Value, &ron::Value)> + '_ {
    //     let mut fields = None;
    //
    //     if let Self::Fields(map) = self {
    //         fields = Some(map);
    //     }
    //
    //     fields.into_iter().flat_map(|map| map.iter())
    // }

    fn field_mut<'f>(
        fields: &'f mut [(String, ron::Value)],
        name: &str,
    ) -> Option<&'f mut ron::Value> {
        fields
            .iter_mut()
            .find(|(field_name, _)| name == field_name)
            .map(|(_, value)| value)
    }

    #[cfg(test)]
    pub fn field(&self, name: &str) -> Option<&ron::Value> {
        if let ComponentFields::Fields(fields) = self {
            fields
                .iter()
                .find(|(field_name, _)| name == field_name)
                .map(|(_, value)| value)
        } else {
            None
        }
    }

    fn override_with(&mut self, other: &Self) {
        match (self, other) {
            (myself, ComponentFields::Negate) => {
                // remove component
                debug!("negating");
                *myself = ComponentFields::Negate;
            }
            (myself, ComponentFields::Unit) => {
                debug!("setting to unit");
                *myself = ComponentFields::Unit;
            }
            (ComponentFields::Unit, _) => {
                // nothing to do
                debug!("skipping because self is unit and other is not negate");
            }
            (ComponentFields::Fields(mine), ComponentFields::Fields(urs)) => {
                for (name, value) in urs.iter() {
                    if let Some(existing) = Self::field_mut(mine, name) {
                        *existing = value.clone();
                        debug!("overriding value for field {field}", field = name);
                    } else {
                        mine.push((name.clone(), value.clone()));
                        debug!("adding overridden field {field}", field = name);
                    }
                }
            }
            _ => unreachable!(),
        }
    }
}

impl Deref for ProcessedComponents {
    type Target = Vec<(String, ComponentFields)>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub fn preprocess(defs: &mut Vec<DeserializedDefinition>) -> Result<(), DefinitionErrors> {
    let mut errors = Vec::new();

    let mut dag = Dag::<_, _, u16>::with_capacity(defs.len(), defs.len());
    let mut lookup = HashMap::with_capacity(defs.len());

    // convert components into easy-to-use format
    for def in defs.iter_mut() {
        if let Err(errs) = def.validate_and_process_components() {
            errors.extend(
                errs.into_iter()
                    .map(|(uid, e)| def.make_error(Some(uid), e)),
            );
        }
    }

    // move definitions into tree nodes
    for def in defs.drain(..) {
        let uid = def.uid().to_owned();

        let node = dag.add_node(def);

        if let Some(old) = lookup.insert(uid, node) {
            let def = dag.node_weight(old).unwrap();
            errors.push(def.make_error(
                Some(def.uid().to_owned()),
                DefinitionErrorKind::DuplicateUid,
            ));
        }
    }

    // add parent links
    {
        let mut add_link = |node| {
            let def = dag.node_weight(node).unwrap();

            if let Some(parent) = def.parent() {
                let parent_node = match lookup.get(parent) {
                    Some(n) => *n,
                    None => {
                        return Err(def.make_error(
                            Some(def.uid().to_owned()),
                            DefinitionErrorKind::InvalidParent(parent.to_owned()),
                        ))
                    }
                };

                // parent can't be self
                let mut is_cyclic = node == parent_node;

                if !is_cyclic {
                    is_cyclic |= dag.add_edge(node, parent_node, ()).is_err();
                }

                if is_cyclic {
                    let def = dag.node_weight(node).unwrap();
                    return Err(def.make_error(
                        Some(def.uid().to_owned()),
                        DefinitionErrorKind::CyclicParentRelation(
                            def.uid().to_owned(),
                            def.parent().unwrap().to_owned(),
                        ),
                    ));
                }
            }
            Ok(())
        };

        for &node in lookup.values() {
            if let Err(e) = add_link(node) {
                errors.push(e);
            }
        }
    }

    // apply inheritance overrides
    {
        let mut path = Vec::with_capacity(16);
        for &node in lookup.values() {
            let this = dag.node_weight(node).unwrap();
            debug!("applying inheritance overrides"; "definition" => ?this.uid());
            path.push(this);

            // ascend to root, tracking path on the way
            let root = {
                let mut walker = dag.recursive_walk(node, |graph, node| {
                    graph
                        .edges_directed(node, Direction::Outgoing)
                        .next()
                        .map(|edge| (edge.id(), edge.target()))
                });

                let mut root = node;
                while let Some((_, parent)) = walker.walk_next(&dag) {
                    let this = dag.node_weight(parent).unwrap();
                    path.push(this);

                    root = parent;
                }

                dag.node_weight(root).unwrap()
            };

            debug!(
                "path to root"; "path" => ?path.iter().map(|d| d.uid()).collect_vec(),
            );

            // start with a clean copy of the root node's components, and apply each override's
            // components over the top
            let mut components = path
                .iter()
                .rev()
                .skip(1) // root already cloned
                .fold(root.processed_components().clone(), |mut acc, &def| {
                    debug!("applying components from {ancestor}", ancestor = def.uid(); "result" => ?this.uid(), "root" => ?root.uid());
                    acc.override_with(&*def.processed_components());
                    acc
                });

            // replace def's components
            components.clean_negated();
            *(this.processed_components_mut()) = components;

            path.clear();
        }
    }

    // take non-abstract definitions back out of tree
    let (nodes, _) = dag.into_graph().into_nodes_edges();
    defs.extend(nodes.into_iter().filter_map(|node| {
        if node.weight.is_abstract() {
            None
        } else {
            Some(node.weight)
        }
    }));

    if !errors.is_empty() {
        Err(DefinitionErrors(errors))
    } else {
        Ok(())
    }
}

fn extract_map(value: ron::Value) -> Result<ron::Map, DefinitionErrorKind> {
    match value {
        ron::Value::Map(map) => Ok(map),
        _ => Err(DefinitionErrorKind::Format(ron::Error::custom("not a map"))),
    }
}

fn extract_string_ref(value: &ron::Value) -> Result<&str, DefinitionErrorKind> {
    match value {
        ron::Value::String(string) => Ok(string),
        _ => Err(DefinitionErrorKind::Format(ron::Error::custom(
            "not a string",
        ))),
    }
}
