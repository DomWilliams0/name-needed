use std::collections::{HashMap, HashSet};
use std::num::NonZeroU16;
use std::rc::Rc;

use specs::BitSet;

use common::*;

use crate::build::{
    BuildMaterial, BuildTemplate, ConsumedMaterialForJobComponent, ReservedMaterialComponent,
};
use crate::definitions::{DefinitionNameComponent, DefinitionRegistry};
use crate::ecs::{EcsWorld, Join, WorldExt};
use crate::item::HaulableItemComponent;
use crate::job::job::{CompletedTasks, SocietyJobImpl};
use crate::job::SocietyTaskResult;
use crate::society::job::{SocietyJobHandle, SocietyTask};
use crate::string::{CachedStr, CachedStringHasher};
use crate::{
    BlockType, ComponentWorld, Entity, ItemStackComponent, TransformComponent, WorldPosition,
};

// TODO build requirement engine for generic material combining
//      each job owns an instance, lends it to UI for rendering
//      consumes generic requirements through enum, "is wood", "can cut" etc. use def names for now
//      reports original requirements (for ui), what's already gathered (for ui), what's remaining (for gather tasks)
//          emits iterator of BuildMaterials, and an impl Display

#[derive(Debug)]
pub struct BuildThingJob {
    // TODO support builds spanning multiple blocks/range
    position: WorldPosition,
    build: Rc<BuildTemplate>,
    required_materials: Vec<BuildMaterial>,
    reserved_materials: HashSet<Entity>,

    /// Cache of extra hands needed for hauling each material
    hands_needed: HashMap<CachedStr, u16, CachedStringHasher>,

    /// Steps completed so far
    progress: u32,

    /// Gross temporary way of tracking remaining materials
    materials_remaining: HashMap<CachedStr, NonZeroU16, CachedStringHasher>,

    /// Set if any material types are invalid e.g. not haulable
    missing_any_requirements: bool,

    /// Set in first call to [populate_initial_tasks]
    this_job: Option<SocietyJobHandle>,

    /// Entity representing this job in UI with UiElementComponent, set post spawn
    ui_element: Option<Entity>,
}

/// Lightweight struct of end goals for a build, to be used for deciding whether to work on a build.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct BuildDetails {
    pub pos: WorldPosition,
    pub target: BlockType,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct BuildProgressDetails {
    pub total_steps_needed: u32,
    pub progress_rate: u32,
    pub steps_completed: u32,
}

#[derive(Debug, Error)]
pub enum BuildThingError {
    #[error("Build materials are invalid")]
    InvalidMaterials,

    #[error("No definition for material {0}")]
    MissingDefinition(Entity),

    #[error("Material '{0}' is not required")]
    MaterialNotRequired(CachedStr),
}

pub enum MaterialReservation {
    /// Whole stack of this size is reserved
    ConsumeAll(u16),
    /// Stack has too many materials, should be split from original stack
    Surplus { surplus: u16, reserved: u16 },
}

impl BuildThingJob {
    pub fn new(block: WorldPosition, build: Rc<BuildTemplate>) -> Self {
        let required_materials = build.materials().to_vec();
        let count = required_materials.len();
        Self {
            position: block,
            build,
            progress: 0,
            required_materials,
            hands_needed: HashMap::with_capacity_and_hasher(count, CachedStringHasher::default()),
            reserved_materials: HashSet::new(),
            materials_remaining: HashMap::with_hasher(CachedStringHasher::default()), // replaced on each call
            missing_any_requirements: false,
            this_job: None,
            ui_element: None,
        }
    }

    // TODO fewer temporary allocations
    fn check_materials(
        &mut self,
        world: &EcsWorld,
    ) -> HashMap<CachedStr, NonZeroU16, CachedStringHasher> {
        let this_job = self.this_job.unwrap(); // set before this is called

        let job_pos = self.position.centred();
        let reserveds = world.read_storage::<ReservedMaterialComponent>();
        let transforms = world.read_storage::<TransformComponent>();
        let consumeds = world.read_storage::<ConsumedMaterialForJobComponent>();

        // clear out now-invalid reserved materials
        self.reserved_materials.retain(|e| {
            let unreserve_reason = match world.components(*e, (&reserveds, transforms.maybe(), consumeds.maybe())) {
                Some((reserved, Some(transform), None)) =>  {
                    if reserved.build_job != this_job {
                        Some("reservation changed")
                    }
                    else if !transform.position.is_almost(&job_pos, 3.0) {
                        Some("too far away")
                    } else {
                        // reservation is still fine
                        None
                    }
                },
                Some((_reserved, None, Some(_))) => {
                    // material is still reserved as it's consumed
                    None
                }
                _ => {
                    Some("material is kill")
                },
            };

            match unreserve_reason {
                Some(reason) => {
                    debug!("removing now-invalid reservation for build job"; "material" => e, "reason" => reason);
                    false
                }
                None => true,
            }
        });

        let mut remaining_materials = self
            .required_materials
            .iter()
            .map(|mat| (mat.definition(), mat.quantity().get()))
            .collect::<HashMap<_, _>>();

        let reservations_bitset = self
            .reserved_materials
            .iter()
            .map(|e| e.id())
            .collect::<BitSet>();

        let def_names = world.read_storage::<DefinitionNameComponent>();
        let stacks = world.read_storage::<ItemStackComponent>();
        for (e, def, stack_opt) in (&reservations_bitset, &def_names, stacks.maybe()).join() {
            let entry = remaining_materials
                .get_mut(&def.0)
                .unwrap_or_else(|| panic!("invalid reservation {:?}", def.0));
            // TODO ensure this doesn't happen, or just handle it properly

            let quantity = stack_opt.map(|comp| comp.stack.total_count()).unwrap_or(1);
            *entry = entry.checked_sub(quantity).unwrap_or_else(|| {
                unreachable!(
                    "tried to over-reserve {}/{} of remaining requirement '{}' in job {:?} (material = {})",
                    quantity, *entry, def.0, this_job, e
                )
            });
        }

        // collect the remaining unsatisfied requirements
        remaining_materials
            .iter()
            .filter_map(|(def, n)| NonZeroU16::new(*n).map(|n| (*def, n)))
            .collect()
    }

    /// Returns (Surplus(n), _) if there is a surplus of material to drop in a NEW stack.
    /// Second return val is the now-remaining requirement for this material.
    /// Entity should get a ReservedMaterialComponent on success
    pub fn add_reservation(
        &mut self,
        reservee: Entity,
        world: &EcsWorld,
    ) -> Result<(MaterialReservation, u16), BuildThingError> {
        let stacks = world.read_storage::<ItemStackComponent>();
        let defs = world.read_storage::<DefinitionNameComponent>();

        let def = reservee
            .get(&defs)
            .ok_or(BuildThingError::MissingDefinition(reservee))?;

        let n_required_ref = self
            .materials_remaining
            .get_mut(&def.0)
            .ok_or(BuildThingError::MaterialNotRequired(def.0))?;
        let n_required = n_required_ref.get();

        let n_actual = {
            reservee
                .get(&stacks)
                .map(|stack| stack.stack.total_count())
                .unwrap_or(1)
        };

        let n_to_reserve;
        let result = if n_actual > n_required {
            // trying to reserve too many, keep the original stack but split off some
            let to_drop = n_actual - n_required;
            n_to_reserve = n_required;
            MaterialReservation::Surplus {
                surplus: to_drop,
                reserved: n_required,
            }
        } else {
            n_to_reserve = n_actual;
            MaterialReservation::ConsumeAll(n_actual)
        };

        // reserve material entity
        let _ = self.reserved_materials.insert(reservee);

        // reduce requirement count
        let remaining = match NonZeroU16::new(n_required - n_to_reserve) {
            Some(n) => {
                *n_required_ref = n;
                n.get()
            }
            None => {
                self.materials_remaining.remove(&def.0);
                0
            }
        };

        Ok((result, remaining))
    }

    pub fn reserved_materials(&self) -> impl Iterator<Item = Entity> + '_ {
        self.reserved_materials.iter().copied()
    }

    pub fn details(&self) -> BuildDetails {
        BuildDetails {
            pos: self.position,
            target: self.build.output(),
        }
    }

    pub fn progress(&self) -> BuildProgressDetails {
        let (total_steps_needed, progress_rate) = self.build.progression();
        BuildProgressDetails {
            total_steps_needed,
            progress_rate,
            steps_completed: self.progress,
        }
    }

    pub fn set_ui_element(&mut self, e: Entity) {
        assert!(self.ui_element.is_none(), "ui element already set");
        self.ui_element = Some(e);
    }

    pub fn remaining_requirements(&self) -> impl Iterator<Item = BuildMaterial> + '_ {
        self.materials_remaining
            .iter()
            .map(|(s, n)| BuildMaterial::new(*s, *n))
    }

    /// Returns new progress
    pub fn make_progress(&mut self) -> u32 {
        self.progress += 1;
        self.progress
    }
}

impl SocietyJobImpl for BuildThingJob {
    fn populate_initial_tasks(
        &mut self,
        world: &EcsWorld,
        out: &mut Vec<SocietyTask>,
        this_job: SocietyJobHandle,
    ) {
        self.this_job = Some(this_job);

        // TODO allow "building" of a non-air block, and automatically emit a break task first?
        //  maybe that should be at a higher level than this

        // preprocess materials to get hands needed for hauling
        let definitions = world.resource::<DefinitionRegistry>();
        for mat in self.required_materials.iter() {
            let def = definitions
                .lookup_definition(mat.definition())
                .and_then(|def| def.find_component("haulable"));
            let hands = match def {
                Some(any) => {
                    let haulable = any
                        .downcast_ref::<HaulableItemComponent>()
                        .expect("bad type for haulable template");
                    debug!("{:?} needs {} hands", mat, haulable.extra_hands);
                    haulable.extra_hands
                }
                None => {
                    // TODO job is destined to fail...
                    warn!("build material is not haulable"; "material" => ?mat);
                    self.missing_any_requirements = true;
                    return;
                }
            };

            self.hands_needed.insert(mat.definition(), hands);
        }

        // gather materials first
        out.extend(self.required_materials.iter().cloned().flat_map(|mat| {
            let extra_hands = *self.hands_needed.get(&mat.definition()).unwrap(); // just inserted

            Some(SocietyTask::GatherMaterials {
                build_pos: self.position,
                material: mat,
                job: this_job,
                extra_hands_needed_for_haul: extra_hands,
            })
        }));

        self.missing_any_requirements = out.len() != self.required_materials.len();
    }

    fn refresh_tasks(
        &mut self,
        world: &EcsWorld,
        tasks: &mut Vec<SocietyTask>,
        completions: CompletedTasks,
    ) -> Option<SocietyTaskResult> {
        if self.missing_any_requirements {
            return Some(SocietyTaskResult::Failure(
                BuildThingError::InvalidMaterials.into(),
            ));
        }

        // ignore completions for gathering, only use for checking the build outcome
        if let Some((_, result)) = completions
            .iter_mut()
            .find(|(t, _)| matches!(t, SocietyTask::Build(_, _)))
        {
            // build is complete
            let result = std::mem::replace(result, SocietyTaskResult::Success);
            return Some(result);
        }

        // TODO dont run this every tick, only when something changes or intermittently
        let outstanding_requirements = self.check_materials(world);

        // recreate tasks for outstanding materials
        // TODO this changes the order
        tasks.clear();
        let this_job = self.this_job.unwrap(); // set unconditionally
        for (def, count) in outstanding_requirements.iter() {
            let extra_hands = *self.hands_needed.get(def).unwrap(); // already inserted

            let task = SocietyTask::GatherMaterials {
                build_pos: self.position,
                material: BuildMaterial::new(*def, *count),
                job: this_job,
                extra_hands_needed_for_haul: extra_hands,
            };

            tasks.push(task);
        }

        // store this to show in the ui
        self.materials_remaining = outstanding_requirements;

        if tasks.is_empty() {
            // all gather requirements are satisfied, do the build
            // TODO some builds could have multiple workers

            tasks.push(SocietyTask::Build(this_job, self.details()));
        }

        None // use number of tasks to determine completion
    }

    crate::as_any_impl!();
}

impl Display for BuildThingJob {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        // TODO better display impl for builds
        write!(f, "Build {} at {}", self.build.output(), self.position)
    }
}
