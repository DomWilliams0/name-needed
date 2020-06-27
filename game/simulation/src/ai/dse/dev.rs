use crate::ai::consideration::HasDivineCommandConsideration;
use crate::ai::{AiContext, DivineCommandComponent};
use crate::ComponentWorld;
use ai::{AiBox, Consideration, Context, DecisionWeight, Dse};

pub struct ObeyDivineCommandDse;

impl Dse<AiContext> for ObeyDivineCommandDse {
    fn name(&self) -> &str {
        "Dev - obey divine command"
    }

    fn considerations(&self) -> Vec<AiBox<dyn Consideration<AiContext>>> {
        vec![AiBox::new(HasDivineCommandConsideration)]
    }

    fn weight(&self) -> DecisionWeight {
        DecisionWeight::AbsoluteOverride
    }

    fn action(
        &self,
        blackboard: &mut <AiContext as Context>::Blackboard,
    ) -> <AiContext as Context>::Action {
        let divine_command = blackboard
            .world
            .component::<DivineCommandComponent>(blackboard.entity)
            .expect("divine command expected to be present");

        divine_command.0.clone()
    }
}
