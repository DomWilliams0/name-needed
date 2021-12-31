use crate::ecs::*;
use crate::{ItemStackComponent, PlayerSociety, SocietyComponent, Tick, TransformComponent};
use common::*;

use crate::input::{MouseLocation, SelectedComponent};
use crate::spatial::{Spatial, Transforms};
use specs::storage::StorageEntry;
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};
use std::hint::unreachable_unchecked;

// TODO smol string and/or cow and/or pool common strings

/// Describes an entity, e.g. "human", "stone brick", "cat".
#[derive(Component, EcsComponent, Clone, Debug)]
#[storage(VecStorage)]
#[name("kind")]
pub struct KindComponent(String);

/// A name for a living thing e.g. "Steve"
#[derive(Component, EcsComponent, Clone, Debug)]
#[storage(HashMapStorage)]
#[name("name")]
pub struct NameComponent(String);

/// Caches the display string rendered on each entity
#[derive(Component, EcsComponent, Clone)]
#[storage(DenseVecStorage)]
#[name("display")]
pub struct DisplayComponent {
    content: DisplayContent,
}

#[derive(Clone)]
enum DisplayContent {
    Prepared(Option<PreparedDisplay>),
    // TODO smolstr to use the slack space
    // TODO reuse string storage when switching back to prepared
    Rendered(String),
}

#[derive(Clone, Copy)]
enum PreparedDisplay {
    ItemStackCount(u16),
    ItemStackFull(u16),
    Kind,
    Name,
}

const HOVER_RADIUS: f32 = 2.0;

pub struct DisplayTextSystem;

impl<'a> System<'a> for DisplayTextSystem {
    type SystemData = (
        Read<'a, EntitiesRes>,
        Read<'a, PlayerSociety>,
        Read<'a, Spatial>,
        Read<'a, MouseLocation>,
        ReadStorage<'a, TransformComponent>,
        ReadStorage<'a, SelectedComponent>,
        ReadStorage<'a, SocietyComponent>,
        ReadStorage<'a, NameComponent>,
        ReadStorage<'a, ItemStackComponent>,
        WriteStorage<'a, DisplayComponent>,
    );

    fn run(
        &mut self,
        (
            entities,
            player_soc,
            spatial,
            mouse,
            transforms,
            selected,
            society,
            name,
            stack,
            mut display,
        ): Self::SystemData,
    ) {
        // TODO dont bother applying to entities far away from camera/definitely not visible. via custom Joinable type?
        // TODO reuse allocs

        let mut new_displays = HashMap::new();

        for (e, _, society) in (&entities, &name, (&society).maybe()).join() {
            // always display name for named society members
            if *player_soc == society.map(|soc| soc.handle) {
                new_displays.insert(e, PreparedDisplay::Name);
            }
        }

        // show more info for entities close to the mouse
        let nearby = spatial
            .query_in_radius(Transforms::Storage(&transforms), mouse.0, HOVER_RADIUS)
            .map(|(e, _, _)| e)
            .take(32)
            .collect::<HashSet<_>>();

        for (e, stack, selected) in (&entities, (&stack).maybe(), (&selected).maybe()).join() {
            let more_info = selected.is_some() || nearby.contains(&e.into());

            let prep = match stack {
                Some(stack) => {
                    match stack.stack.total_count() {
                        n if n <= 1 => continue,
                        n if more_info => {
                            // show type as well as count
                            PreparedDisplay::ItemStackFull(n)
                        }
                        n => {
                            // just stack count
                            PreparedDisplay::ItemStackCount(n)
                        }
                    }
                }
                None => {
                    if more_info {
                        PreparedDisplay::Kind
                    } else {
                        continue;
                    }
                }
            };

            new_displays.insert(e, prep);
        }

        // apply changes
        // TODO can replacing all components be done better? or just occasionally
        for (e, ty) in new_displays.iter() {
            let entry = display.entry(*e).unwrap(); // wont be wrong gen
            let content = DisplayContent::Prepared(Some(*ty));
            match entry {
                StorageEntry::Occupied(mut e) => e.get_mut().content = content,
                StorageEntry::Vacant(e) => {
                    e.insert(DisplayComponent { content });
                }
            }
        }

        // periodic cleanup
        let tick = Tick::fetch();
        if tick.value() % 50 == 0 {
            // remove unneeded display components
            // TODO reuse alloc
            let to_remove = (&entities, &display)
                .join()
                .filter_map(|(e, _)| {
                    if !new_displays.contains_key(&e) {
                        Some(e)
                    } else {
                        None
                    }
                })
                .collect_vec();

            for e in &to_remove {
                let _ = display.remove(*e);
            }

            let n = to_remove.len();
            trace!("removed {n} unneeded DisplayComponents", n = n);
        } else {
            // just nop them
            let mut display_restricted = display.restrict_mut();
            for (e, mut display) in (&entities, &mut display_restricted).join() {
                if !new_displays.contains_key(&e) {
                    let display = display.get_mut_unchecked();
                    display.content = DisplayContent::Prepared(None);
                }
            }
        }
    }
}

impl DisplayComponent {
    pub fn render<
        'a,
        F: Fn() -> (
            specs::Entity,
            &'a ReadStorage<'a, KindComponent>,
            &'a ReadStorage<'a, NameComponent>,
        ),
    >(
        &mut self,
        fetch: F,
    ) -> Option<&str> {
        if let DisplayContent::Prepared(prep) = &self.content {
            let prep = match prep {
                Some(prep) => prep,
                None => return None,
            };

            let (e, kinds, names) = fetch();
            let rendered = match prep {
                PreparedDisplay::ItemStackCount(n) => {
                    format!("x{}", n)
                }
                PreparedDisplay::ItemStackFull(n) => {
                    let kind = kinds.get(e)?;
                    // TODO use plural
                    format!("{} x{}", kind.0, n)
                }
                PreparedDisplay::Kind => {
                    let kind = kinds.get(e)?;
                    kind.0.to_string()
                }
                PreparedDisplay::Name => {
                    let name = names.get(e)?;
                    name.0.to_string()
                }
            };

            self.content = DisplayContent::Rendered(rendered);
        }

        match &self.content {
            DisplayContent::Rendered(s) => Some(s),
            _ => {
                debug_assert!(false);
                // safety: unconditionally rendered by now
                unsafe { unreachable_unchecked() }
            }
        }
    }
}

impl NameComponent {
    pub fn new(name: String) -> Self {
        Self(name)
    }
}

impl KindComponent {
    pub fn make_stack(&mut self) {
        // TODO
    }
}

impl NameComponent {
    pub fn name(&self) -> &str {
        &self.0
    }
}

impl Display for KindComponent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl<V: Value> ComponentTemplate<V> for KindComponent {
    fn construct(values: &mut Map<V>) -> Result<Box<dyn ComponentTemplate<V>>, ComponentBuildError>
    where
        Self: Sized,
    {
        Ok(Box::new(Self(values.get_string("singular")?)))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        builder.with(self.clone())
    }

    crate::as_any!();
}
