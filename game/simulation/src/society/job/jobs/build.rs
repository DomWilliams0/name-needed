use crate::build::{
    Build, BuildMaterial, ConsumedMaterialForJobComponent, ReservedMaterialComponent,
};
use crate::definitions::{DefinitionNameComponent, Registry};
use crate::ecs::{EcsWorld, Join, WorldExt};
use crate::item::HaulableItemComponent;
use crate::job::job::{CompletedTasks, SocietyJobImpl};
use crate::job::SocietyTaskResult;
use crate::society::job::{SocietyJobHandle, SocietyTask};
use crate::{
    BlockType, ComponentWorld, Entity, ItemStackComponent, TransformComponent, WorldPosition,
};
use common::*;
use specs::BitSet;
use std::cell::Cell;
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub struct BuildThingJob {
    // TODO support builds spanning multiple blocks/range
    position: WorldPosition,
    build: Box<dyn Build>,
    required_materials: Vec<BuildMaterial>,
    reserved_materials: HashSet<Entity>,
    /// Set if any material types are invalid e.g. not haulable
    missing_any_requirements: Cell<bool>,

    /// Set in first call to [populate_initial_tasks]
    this_job: Cell<Option<SocietyJobHandle>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct BuildDetails {
    pub pos: WorldPosition,
    pub target: BlockType,
}

#[derive(Debug, Error)]
pub enum BuildThingError {
    #[error("Build materials are invalid")]
    InvalidMaterials,
}

impl BuildThingJob {
    pub fn new(block: WorldPosition, build: impl Build + 'static) -> Self {
        let mut materials = Vec::new();
        build.materials(&mut materials);
        Self {
            position: block,
            build: Box::new(build),
            required_materials: materials,
            reserved_materials: HashSet::new(),
            missing_any_requirements: Cell::new(false),
            this_job: Cell::new(None),
        }
    }

    // TODO fewer temporary allocations
    fn check_materials(&mut self, world: &EcsWorld) -> HashMap<&str, u16> {
        let this_job = self.this_job.get().unwrap(); // set before this is called

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
            .map(|mat| (mat.definition(), mat.quantity()))
            .collect::<HashMap<_, _>>();

        let reservations_bitset = self
            .reserved_materials
            .iter()
            .map(|e| e.id())
            .collect::<BitSet>();

        let def_names = world.read_storage::<DefinitionNameComponent>();
        let stacks = world.read_storage::<ItemStackComponent>();
        for (_, def, stack_opt) in (&reservations_bitset, &def_names, stacks.maybe()).join() {
            let entry = remaining_materials
                .get_mut(def.0.as_str())
                .unwrap_or_else(|| panic!("invalid reservation {:?}", def.0));
            // TODO ensure this doesn't happen, or just handle it properly

            let quantity = stack_opt.map(|comp| comp.stack.total_count()).unwrap_or(1);
            // TODO checked_sub instead, ensure only the exact number is reserved
            *entry = entry.saturating_sub(quantity);
        }

        // collect the remaining unsatisfied requirements
        remaining_materials.retain(|_, n| *n > 0);
        remaining_materials
    }

    /// Returns false if already reserved. Entity should have ReservedMaterialComponent
    pub fn add_reservation(&mut self, reservee: Entity) -> bool {
        self.reserved_materials.insert(reservee)
    }

    pub fn reserved_materials(&self) -> impl Iterator<Item = Entity> + '_ {
        self.reserved_materials.iter().copied()
    }

    pub fn build(&self) -> &dyn Build {
        &*self.build
    }

    pub fn details(&self) -> BuildDetails {
        BuildDetails {
            pos: self.position,
            target: self.build.output(),
        }
    }
}

impl SocietyJobImpl for BuildThingJob {
    fn populate_initial_tasks(
        &self,
        world: &EcsWorld,
        out: &mut Vec<SocietyTask>,
        this_job: SocietyJobHandle,
    ) {
        self.this_job.set(Some(this_job));

        // TODO allow "building" of a non-air block, and automatically emit a break task first?
        //  maybe that should be at a higher level than this

        // gather materials first
        out.extend(self.required_materials.iter().cloned().flat_map(|mat| {
            let extra_hands = {
                let definitions = world.resource::<Registry>();
                match definitions.lookup_template(mat.definition(), "haulable") {
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
                        return None;
                    }
                }
            };

            Some(SocietyTask::GatherMaterials {
                target: self.position,
                material: mat,
                job: this_job,
                extra_hands_needed_for_haul: extra_hands,
            })
        }));

        self.missing_any_requirements
            .set(out.len() != self.required_materials.len());
    }

    fn refresh_tasks(
        &mut self,
        world: &EcsWorld,
        tasks: &mut Vec<SocietyTask>,
        completions: CompletedTasks,
    ) -> Option<SocietyTaskResult> {
        if self.missing_any_requirements.get() {
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

        tasks.retain(|task| {
            let req = match task {
                SocietyTask::GatherMaterials { material, .. } => material,
                SocietyTask::Build(_, _) => return false, // filter out build task at this point, readd after
                _ => unreachable!(),
            };

            if outstanding_requirements.get(req.definition()).is_some() {
                true
            } else {
                trace!("removing completed requirement"; "material" => ?req);
                false
            }
        });

        if tasks.is_empty() {
            // all gather requirements are satisfied, do the build
            // TODO some builds could have multiple workers

            let this_job = self.this_job.get().unwrap(); // set unconditionally
            tasks.push(SocietyTask::Build(this_job, self.details()));
        }

        None // use number of tasks to determine completion
    }

    crate::as_any_impl!();
}

impl Display for BuildThingJob {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        // TODO better display impl for builds
        write!(f, "Build {:?} at {}", self.build, self.position)
    }
}
