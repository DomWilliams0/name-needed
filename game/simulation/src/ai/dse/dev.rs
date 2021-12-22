use crate::ai::consideration::ConstantConsideration;
use crate::ai::{AiAction, AiContext};
use ai::{AiBox, Consideration, Context, DecisionWeightType, Dse};

pub struct ObeyDivineCommandDse(pub AiAction);

impl Dse<AiContext> for ObeyDivineCommandDse {
    fn considerations(&self) -> Vec<AiBox<dyn Consideration<AiContext>>> {
        vec![AiBox::new(ConstantConsideration(1.0))]
    }

    fn weight_type(&self) -> DecisionWeightType {
        DecisionWeightType::AbsoluteOverride
    }

    fn action(&self, _: &mut <AiContext as Context>::Blackboard) -> <AiContext as Context>::Action {
        self.0.clone()
    }
}
