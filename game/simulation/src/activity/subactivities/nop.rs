use crate::activity::activity::{ActivityFinish, ActivityResult, SubActivity};
use crate::activity::ActivityContext;
use crate::ComponentWorld;
use common::*;

/// Nop subactivity with customized Display impl
pub struct NopSubActivity {
    pub display: &'static str,
    pub exertion: f32,
}

impl<W: ComponentWorld> SubActivity<W> for NopSubActivity {
    fn init(&self, _: &mut ActivityContext<W>) -> ActivityResult {
        ActivityResult::Ongoing
    }

    fn on_finish(&self, _: &ActivityFinish, _: &mut ActivityContext<W>) -> BoxedResult<()> {
        Ok(())
    }

    fn exertion(&self) -> f32 {
        self.exertion
    }
}

impl Display for NopSubActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", self.display)
    }
}

#[macro_export]
macro_rules! nop_subactivity {
    ($display:expr, $exertion:expr) => {
        &$crate::activity::subactivities::NopSubActivity {
            display: $display,
            exertion: $exertion,
        }
    };

    () => {
        nop_subactivity!("Thinking", 0.0)
    };
}
