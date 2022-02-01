use crate::string::StringCache;
use std::collections::HashMap;
use std::fmt::{Display, Formatter, Write};
use std::hint::unreachable_unchecked;
use std::rc::Rc;

use specs::storage::StorageEntry;

use ::world::block::BlockType;
use common::*;

use crate::build::ReservedMaterialComponent;
use crate::ecs::*;
use crate::input::{MouseLocation, SelectedComponent};
use crate::job::BuildThingJob;
use crate::simulation::EcsWorldRef;
use crate::spatial::Spatial;
use crate::{
    ItemStackComponent, PlayerSociety, Societies, SocietyComponent, Tick, UiElementComponent,
};

// TODO smol string and/or cow and/or pool common strings

/// Describes an entity, e.g. "human", "stone brick", "cat".
#[derive(Component, EcsComponent, Clone, Debug)]
#[storage(VecStorage)]
#[name("kind")]
pub struct KindComponent(String, Option<KindModifier>);

#[derive(Clone, Debug)]
enum KindModifier {
    Stack,
}

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
    Rendered {
        // TODO smolstr to use the slack space
        string: String,
        /// For comparisons to avoid redundant string rendering
        prep: PreparedDisplay,
    },
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum PreparedDisplay {
    /// Stack count only
    ItemStackCount(u16),
    /// Stack count and item kind
    ItemStackFull(u16),
    /// Just kind
    Kind,
    /// Just name
    Name,
    /// Just build progress percentage
    BuildProgressLight(BuildProgress),
    /// Build target and progress percentage
    BuildProgress(BlockType, BuildProgress),
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum BuildProgress {
    MaterialsNeeded,
    /// Percentage
    Building(u8),
}

const HOVER_RADIUS: f32 = 2.0;

#[derive(Default)]
pub struct DisplayTextSystem {
    preparation: HashMap<specs::Entity, PreparedDisplay>,
    removal_cache: Vec<specs::Entity>,
}

impl<'a> System<'a> for DisplayTextSystem {
    type SystemData = (
        Read<'a, EntitiesRes>,
        Read<'a, PlayerSociety>,
        Read<'a, Spatial>,
        Read<'a, MouseLocation>,
        Read<'a, Societies>,
        Read<'a, EcsWorldRef>,
        ReadStorage<'a, SelectedComponent>,
        ReadStorage<'a, SocietyComponent>,
        ReadStorage<'a, NameComponent>,
        ReadStorage<'a, ItemStackComponent>,
        ReadStorage<'a, UiElementComponent>,
        WriteStorage<'a, DisplayComponent>,
        ReadStorage<'a, ReservedMaterialComponent>,
    );

    fn run(
        &mut self,
        (
            entities,
            player_soc,
            spatial,
            mouse,
            societies,
            world,
            selected,
            society,
            name,
            stack,
            ui,
            mut display,
            reserved,
        ): Self::SystemData,
    ) {
        // TODO dont bother applying to entities far away from camera/definitely not visible. via custom Joinable type?

        for (e, _, society) in (&entities, &name, (&society).maybe()).join() {
            // always display name for named society members
            if *player_soc == society.map(|soc| soc.handle()) {
                self.preparation.insert(e, PreparedDisplay::Name);
            }
        }

        // show more info for entities close to the mouse
        let nearby = spatial
            .query_in_radius(&world, mouse.0, HOVER_RADIUS)
            .map(|(e, _, _)| specs::Entity::from(e))
            .take(8)
            .collect::<ArrayVec<_, 8>>();

        for (e, ui, stack, selected, _) in (
            &entities,
            (&ui).maybe(),
            (&stack).maybe(),
            (&selected).maybe(),
            !(&reserved),
        )
            .join()
        {
            let more_info = selected.is_some() || nearby.contains(&e);

            let prep = if let Some(stack) = stack {
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
            } else if let Some(ui) = ui {
                let prep = ui
                    .build_job
                    .resolve_and_cast(&societies, |build: &BuildThingJob| {
                        let progress = if build.remaining_requirements().next().is_none() {
                            let prog = build.progress();
                            let percent = (prog.steps_completed * 200 + prog.total_steps_needed)
                                / (prog.total_steps_needed * 2);
                            BuildProgress::Building(percent as u8)
                        } else {
                            BuildProgress::MaterialsNeeded
                        };

                        if more_info {
                            PreparedDisplay::BuildProgress(build.details().target, progress)
                        } else {
                            PreparedDisplay::BuildProgressLight(progress)
                        }
                    });

                match prep {
                    Some(prog) => prog,
                    _ => continue,
                }
            } else if more_info {
                PreparedDisplay::Kind
            } else {
                continue;
            };

            // dont overwrite
            self.preparation.entry(e).or_insert(prep);
        }

        // apply changes
        // TODO can replacing all components be done better? or just occasionally
        for (e, ty) in self.preparation.iter() {
            let entry = display.entry(*e).unwrap(); // wont be wrong gen
            match entry {
                StorageEntry::Occupied(mut e) => {
                    // if prepared with the same again, don't bother rendering an identical string
                    let skip = match e.get().content {
                        DisplayContent::Rendered { prep, .. } => prep == *ty,
                        _ => false,
                    };
                    if !skip {
                        e.get_mut().content = DisplayContent::Prepared(Some(*ty));
                    }
                }
                StorageEntry::Vacant(e) => {
                    e.insert(DisplayComponent {
                        content: DisplayContent::Prepared(Some(*ty)),
                    });
                }
            }
        }

        // periodic cleanup
        let tick = Tick::fetch();
        if tick.value() % 200 == 0 {
            // remove unneeded display components
            let mut removal = std::mem::take(&mut self.removal_cache);
            removal.extend((&entities, &display).join().filter_map(|(e, _)| {
                if !self.preparation.contains_key(&e) {
                    Some(e)
                } else {
                    None
                }
            }));
            let n = removal.len();
            for e in removal.drain(..) {
                let _ = display.remove(e);
            }
            trace!("removed {n} unneeded DisplayComponents", n = n);

            let empty = std::mem::replace(&mut self.removal_cache, removal);
            debug_assert!(empty.is_empty());
            std::mem::forget(empty);
        } else {
            // just nop them
            let mut display_restricted = display.restrict_mut();
            for (e, mut display) in (&entities, &mut display_restricted).join() {
                if !self.preparation.contains_key(&e) {
                    let display = display.get_mut_unchecked();
                    display.content = DisplayContent::Prepared(None);
                }
            }
        }

        self.preparation.clear();
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
                Some(prep) => *prep,
                None => return None,
            };

            let (e, kinds, names) = fetch();
            let mut string = String::new();
            let _ = match prep {
                PreparedDisplay::ItemStackCount(n) => {
                    write!(&mut string, "x{}", n)
                }
                PreparedDisplay::ItemStackFull(n) => {
                    let kind = kinds.get(e)?;
                    write!(&mut string, "{} x{}", kind.0, n)
                }
                PreparedDisplay::Kind => {
                    let kind = kinds.get(e)?;
                    string.write_str(&kind.0)
                }
                PreparedDisplay::Name => {
                    let name = names.get(e)?;
                    string.write_str(&name.0)
                }
                PreparedDisplay::BuildProgressLight(prog) => {
                    let percent = match prog {
                        BuildProgress::MaterialsNeeded => 0,
                        BuildProgress::Building(p) => p,
                    };
                    write!(&mut string, "{}%", percent)
                }
                PreparedDisplay::BuildProgress(target, prog) => match prog {
                    BuildProgress::MaterialsNeeded => {
                        write!(&mut string, "{} (incomplete)", target)
                    }
                    BuildProgress::Building(p) => write!(&mut string, "{} ({}%)", target, p),
                },
            };

            self.content = DisplayContent::Rendered { string, prep };
        }

        match &self.content {
            DisplayContent::Rendered { string, .. } => Some(string),
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
        self.1 = Some(KindModifier::Stack);
    }

    pub fn from_display(s: &dyn Display) -> Self {
        Self(s.to_string(), None)
    }
}

impl Display for KindComponent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.1 {
            // TODO use plural
            Some(KindModifier::Stack) => {
                write!(f, "Stack of ")?;
                self.0
                    .chars()
                    .flat_map(|c| c.to_lowercase())
                    .try_for_each(|c| f.write_char(c))
            }
            None => Display::fmt(&self.0, f),
        }
    }
}

impl Display for NameComponent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl<V: Value> ComponentTemplate<V> for KindComponent {
    fn construct(
        values: &mut Map<V>,
        _: &StringCache,
    ) -> Result<Rc<dyn ComponentTemplate<V>>, ComponentBuildError>
    where
        Self: Sized,
    {
        Ok(Rc::new(Self(values.get_string("singular")?, None)))
    }

    fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
        builder.with(self.clone())
    }

    crate::as_any!();
}
