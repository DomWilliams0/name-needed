use std::fmt::Display;

pub use action::AiAction;
use ai::AiBox;
pub use items::ItemsToPickUp;
pub use misc::NopActivity;

use crate::ai::activity::items::{GoPickUpItemActivity, UseHeldItemActivity};
use crate::ai::activity::movement::WanderActivity;
use crate::ai::activity::world::BreakBlockActivity;
use crate::ecs::{ComponentWorld, Entity};
use crate::queued_update::QueuedUpdates;

#[derive(Copy, Clone)]
pub enum ActivityResult {
    Ongoing,
    Finished(Finish),
}

#[derive(Debug, Copy, Clone)]
pub enum Finish {
    Succeeded,
    Failed,
    // TODO failure/interrupt reason
    Interrupted,
}

pub struct ActivityContext<'a, W: ComponentWorld> {
    pub entity: Entity,
    /// Immutable getters only! Use lazy_updates for adding/removing components
    pub world: &'a W,
    pub updates: &'a QueuedUpdates,
}

pub trait Activity<W: ComponentWorld>: Display {
    fn on_start(&mut self, ctx: &ActivityContext<W>);
    fn on_tick(&mut self, ctx: &ActivityContext<W>) -> ActivityResult;
    fn on_finish(&mut self, finish: Finish, ctx: &ActivityContext<W>);

    fn exertion(&self) -> f32;
}

impl<W: ComponentWorld + 'static> From<AiAction> for AiBox<dyn Activity<W>> {
    fn from(a: AiAction) -> Self {
        macro_rules! activity {
            ($act:expr) => {
                AiBox::new($act) as Box<dyn Activity<W>>
            };
        }
        match a {
            AiAction::Nop => activity!(NopActivity),
            AiAction::Wander => activity!(WanderActivity),
            AiAction::GoPickUp(ItemsToPickUp(_, items)) => {
                activity!(GoPickUpItemActivity::new(items))
            }
            AiAction::UseHeldItem(item) => activity!(UseHeldItemActivity::new(item)),
            AiAction::Goto { target, .. } => activity!(movement::goto::<W>(target)),
            AiAction::GoBreakBlock(block) => activity!(BreakBlockActivity::new(block)),
        }
    }
}

mod action;
mod items;
mod misc;
mod movement;
mod world;
