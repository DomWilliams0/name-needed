use crate::activity::activity::{ActivityResult, SubActivity};
use crate::activity::ActivityContext;
use crate::ComponentWorld;
use common::*;

/// Nop subactivity
pub struct ThinkingSubActivity;

impl<W: ComponentWorld> SubActivity<W> for ThinkingSubActivity {
    fn init(&self, _: &mut ActivityContext<W>) -> ActivityResult {
        ActivityResult::Ongoing
    }

    fn on_finish(&self, _: &mut ActivityContext<W>) -> BoxedResult<()> {
        Ok(())
    }

    fn exertion(&self) -> f32 {
        0.0
    }
}

impl Display for ThinkingSubActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Thinking")
    }
}
