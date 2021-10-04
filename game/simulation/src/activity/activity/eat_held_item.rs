use crate::activity::activity::{Activity};
use crate::activity::status::Status;
use crate::ecs::*;
use crate::event::EntityEvent;
use crate::{EdibleItemComponent, Entity};
use async_trait::async_trait;
use common::*;
use std::fmt::Formatter;
use crate::activity::context::{ActivityContext, ActivityResult, InterruptResult};

#[derive(Debug, Clone)]
pub struct EatHeldItemActivity(Entity);

enum State {
    Equipping,
    Eating,
}

#[derive(Debug, Error)]
pub enum EatHeldItemError {
    #[error("Item is not edible or dead")]
    NotEdible(#[from] ComponentGetError),
}

#[async_trait]
impl Activity for EatHeldItemActivity {
    fn description(&self) -> Box<dyn Display> {
        Box::new(self.clone())
    }

    async fn dew_it(&self, ctx: &ActivityContext) -> ActivityResult {
        // ensure enough hands are free for eating
        let extra_hands = match ctx.world().component::<EdibleItemComponent>(self.0) {
            Ok(comp) => comp.extra_hands,
            Err(err) => return Err(EatHeldItemError::NotEdible(err).into()),
        };

        // equip the food
        ctx.update_status(State::Equipping);
        ctx.equip(self.0, extra_hands).await?;

        // eaty eaty
        ctx.update_status(State::Eating);
        ctx.eat(self.0).await?;

        Ok(())
    }

    //noinspection DuplicatedCode
    fn on_unhandled_event(&self, event: EntityEvent) -> InterruptResult {
        if event.subject == self.0 && event.payload.is_destructive() {
            debug!("item has been destroyed, cancelling eat");
            InterruptResult::Cancel
        } else {
            InterruptResult::Continue
        }
    }
}

impl EatHeldItemActivity {
    pub fn new(item: Entity) -> Self {
        Self(item)
    }
}

impl Display for EatHeldItemActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Eating {}", self.0)
    }
}

//noinspection DuplicatedCode
impl Display for State {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            State::Equipping => "Equipping food",
            State::Eating => "Eating food",
        })
    }
}

impl Status for State {
    fn exertion(&self) -> f32 {
        match self {
            State::Equipping => 0.5,
            State::Eating => {
                // TODO varying exertion per food
                0.3
            }
        }
    }
}
