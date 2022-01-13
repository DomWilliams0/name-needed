use crate::ai::consideration::ConstantConsideration;
use crate::ai::{AiAction, AiContext};
use ai::{Considerations, Context, DecisionWeightType, Dse};

pub struct ObeyDivineCommandDse(pub AiAction);

impl Dse<AiContext> for ObeyDivineCommandDse {
    fn considerations(&self, out: &mut Considerations<AiContext>) {
        out.add(ConstantConsideration(1.0));
    }

    fn weight_type(&self) -> DecisionWeightType {
        DecisionWeightType::AbsoluteOverride
    }

    fn action(&self, _: &mut <AiContext as Context>::Blackboard) -> <AiContext as Context>::Action {
        self.0.clone()
    }
}
