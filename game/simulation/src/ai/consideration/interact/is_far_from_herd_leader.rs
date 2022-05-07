use ai::{Consideration, ConsiderationParameter, Context, Curve};

use crate::ai::{AiContext, AiInput};

/// Scores highly if too far from herd leader/centre
pub struct IsFarFromHerdLeaderConsideration;

/// Further than this is 1.0
// TODO should depend on herd
const MAX_DISTANCE: f32 = 20.0;

impl Consideration<AiContext> for IsFarFromHerdLeaderConsideration {
    fn curve(&self) -> Curve {
        Curve::Linear(2.0, -1.0)
    }

    fn input(&self) -> <AiContext as Context>::Input {
        AiInput::MyDistance2ToHerd
    }

    fn parameter(&self) -> ConsiderationParameter {
        ConsiderationParameter::Range {
            min: 0.0,
            max: MAX_DISTANCE.powi(2),
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use common::NormalizedFloat;
//
//     #[test]
//     fn curve_tests() {
//         let curve = IsFarFromHerdLeaderConsideration.curve();
//         let do_it = |x: f32| {
//             let x = IsFarFromHerdLeaderConsideration.parameter().apply(x * x);
//             curve.evaluate(x)
//         };
//         dbg!(do_it(0.0));
//         dbg!(do_it(2.0));
//         dbg!(do_it(4.0));
//         dbg!(do_it(10.0));
//         dbg!(do_it(15.0));
//         dbg!(do_it(25.0));
//         dbg!(do_it(30.0));
//         dbg!(do_it(40.0));
//     }
// }
