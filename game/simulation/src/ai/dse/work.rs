use crate::ai::consideration::ConstantConsideration;
use crate::ai::{AiAction, AiContext};

use crate::society::work_item::WorkItemRef;
use ai::{AiBox, Consideration, Context, DecisionWeightType, Dse};

pub struct WorkOnWorkItemDse {
    pub work_item: WorkItemRef,
}

impl Dse<AiContext> for WorkOnWorkItemDse {
    fn name(&self) -> &'static str {
        "Work on work item"
    }

    fn considerations(&self) -> Vec<AiBox<dyn Consideration<AiContext>>> {
        // TODO actual considerations
        vec![AiBox::new(ConstantConsideration(1.0))]
    }

    fn weight_type(&self) -> DecisionWeightType {
        DecisionWeightType::Normal
    }

    fn action(&self, _: &mut <AiContext as Context>::Blackboard) -> <AiContext as Context>::Action {
        AiAction::GoWorkOnWorkItem(self.work_item.clone())
    }
}
