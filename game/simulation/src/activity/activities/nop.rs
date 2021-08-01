use crate::activity::activity::{ActivityFinish, ActivityResult, SubActivity};
use crate::activity::{Activity, ActivityContext};
use crate::nop_subactivity;

use common::*;

const WARN_THRESHOLD: u32 = 50;

#[derive(Debug, Default)]
pub struct NopActivity(u32);

impl Activity for NopActivity {
    fn on_tick<'a>(&mut self, _: &'a mut ActivityContext<'_>) -> ActivityResult {
        self.0 += 1;
        if self.0 >= WARN_THRESHOLD {
            warn!(
                "ticked nop activity {} times, possible infinite loop",
                self.0
            );
        }

        ActivityResult::Ongoing
    }

    fn on_finish(&mut self, _: &ActivityFinish, _: &mut ActivityContext) -> BoxedResult<()> {
        Ok(())
    }

    fn current_subactivity(&self) -> &dyn SubActivity {
        nop_subactivity!()
    }
}

impl Display for NopActivity {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Doing nothing")
    }
}
