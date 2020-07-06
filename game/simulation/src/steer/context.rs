//! http://www.gameaipro.com/GameAIPro2/GameAIPro2_Chapter18_Context_Steering_Behavior-Driven_Steering_at_the_Macro_Scale.pdf
#![allow(dead_code)]

use std::f32::consts::PI;

use ux::u3;

use common::*;
use unit::dim::SmallUnsignedConstant;

/// North is 0, goes clockwise
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
#[allow(dead_code)]
pub(crate) enum Direction {
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest,
}

impl Direction {
    const COUNT: SmallUnsignedConstant = SmallUnsignedConstant::new(8);

    fn next(self) -> Self {
        let u3 = u3::new(self as u8);
        let next = u3.wrapping_add(u3::new(1));
        // safety: 8 directions, u3 range is 8
        unsafe { std::mem::transmute(next) }
    }

    fn prev(self) -> Self {
        let u3 = u3::new(self as u8);
        let prev = u3.wrapping_sub(u3::new(1));
        // safety: 8 directions, u3 range is 8
        unsafe { std::mem::transmute(prev) }
    }
}

impl From<u8> for Direction {
    fn from(u: u8) -> Self {
        debug_assert!(
            u < Direction::COUNT.as_u8(),
            "direction out of range: {:?}",
            u
        );

        // safety: asserted in range above
        unsafe { std::mem::transmute(u) }
    }
}

impl From<Rad> for Direction {
    fn from(angle: Rad) -> Self {
        let divisor = Direction::COUNT.as_f32();
        let normalized_value = angle.normalize().0 / (2.0 * PI);
        let dir = (normalized_value / divisor) * divisor;
        let dir = (dir * divisor) as u8;
        dir.min(Direction::COUNT.as_u8() - 1).into()
    }
}

impl Into<Rad> for Direction {
    fn into(self) -> Rad {
        const MULT: f32 = (2.0 * PI) / Direction::COUNT.as_f32();
        rad((self as u8 as f32) * MULT).normalize()
    }
}

#[derive(Copy, Clone, Default, Ord, PartialOrd, Eq, PartialEq, Debug)]
struct Desire(u8);

impl Desire {
    fn new(val: u8) -> Self {
        Self(val)
    }

    /// (value, falloff value)
    fn from_float(value: f32) -> (Desire, Desire) {
        debug_assert!(
            value >= 0.0 && value <= 1.0,
            "value out of range: {}",
            value
        );

        let as_float = (value * (u8::MAX as f32)).ceil();
        let desire = as_float as u8;
        let falloff = (as_float * FALLOFF) as u8;

        (Desire(desire), Desire(falloff))
    }

    fn zero(&mut self) {
        self.0 = 0
    }
}

impl Into<f32> for Desire {
    fn into(self) -> f32 {
        if self.0 == 255 {
            // special edge case
            1.0
        } else {
            self.0 as f32 / (u8::MAX as f32 + 1.0)
        }
    }
}

pub trait ContextType: Copy + Clone {}

#[derive(Copy, Clone, Default)]
pub struct Interest;
impl ContextType for Interest {}

#[derive(Copy, Clone, Default)]
pub struct Danger;
impl ContextType for Danger {}

pub type InterestsContextMap = SingleContextMap<Interest>;
pub type DangersContextMap = SingleContextMap<Danger>;

const FALLOFF: f32 = 0.8;

#[derive(Default, Copy, Clone)]
pub struct SingleContextMap<C: ContextType> {
    values: [Desire; Direction::COUNT.as_usize()],
    phantom: PhantomData<C>,
}

impl<C: ContextType> SingleContextMap<C> {
    fn write_direct(&mut self, direction: Direction, desire: Desire) {
        let val = &mut self.values[direction as usize];
        if desire > *val {
            *val = desire;
        }
    }

    fn read_direct(self, direction: Direction) -> Desire {
        self.values[direction as usize]
    }

    fn write(&mut self, direction: Rad, value: f32) {
        let dir = Direction::from(direction);
        let (desire, falloff) = Desire::from_float(value);

        self.values[dir.prev() as usize] = falloff;
        self.values[dir as usize] = desire;
        self.values[dir.next() as usize] = falloff;
    }

    pub fn update_from(&mut self, _prev: Self) {
        // TODO average with previous for less sudden movements
        todo!()
    }
}

impl SingleContextMap<Interest> {
    pub fn write_interest(&mut self, direction: Rad, value: f32) {
        self.write(direction, value)
    }
}

impl SingleContextMap<Danger> {
    pub fn write_danger(&mut self, direction: Rad, value: f32) {
        self.write(direction, value)
    }
}

#[derive(Default, Copy, Clone)]
pub struct ContextMap {
    interest: InterestsContextMap,
    danger: DangersContextMap,
}

impl ContextMap {
    pub fn write_interest(&mut self, direction: Rad, value: f32) {
        self.interest.write(direction, value);
    }

    pub fn write_danger(&mut self, direction: Rad, value: f32) {
        self.danger.write(direction, value);
    }

    pub fn resolve(mut self) -> (Rad, f32) {
        // find min danger
        let min_danger = *self.danger.values.iter().min().unwrap();

        // mask out higher dangers and the corresponding interests
        self.danger
            .values
            .iter_mut()
            .zip(self.interest.values.iter_mut())
            .for_each(|(d, i)| {
                if *d > min_danger {
                    d.zero();
                    i.zero();
                }
            });

        // choose highest interest
        // TODO follow gradients and choose continuous value
        let best_direction = self.interest.values.iter().position_max().unwrap();

        let direction = Direction::from(best_direction as u8);
        let desire = self.interest.values[best_direction];

        (direction.into(), desire.into())
    }

    pub fn interests_mut(&mut self) -> &mut InterestsContextMap {
        &mut self.interest
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::PI;
    use std::f32::EPSILON;

    use common::*;

    use crate::steer::context::{ContextMap, Desire, Direction, Interest, SingleContextMap};

    #[test]
    fn direction_wrapping() {
        // to keep this test relevant
        assert_eq!(Direction::North as usize, 0);

        assert_eq!(Direction::North.next(), Direction::NorthEast);
        assert_eq!(Direction::North.prev(), Direction::NorthWest);

        assert_eq!(Direction::South.next(), Direction::SouthWest);
        assert_eq!(Direction::South.prev(), Direction::SouthEast);
    }

    #[test]
    fn desire_conversion() {
        // back and forth conversion should stay the same
        for val in 0u8..=255 {
            let desire = Desire::new(val);

            let f: f32 = desire.into();
            let (d, _) = Desire::from_float(f);

            assert_eq!(desire, d, "{:?} -> {:?} & {:?}", desire, f, d);
        }

        for &val in &[0.0, 0.125, 0.25, 0.375, 0.5, 0.875, 1.0] {
            let (desire, _) = Desire::from_float(val);
            let float: f32 = desire.into();
            assert!(
                val.approx_eq(float, (EPSILON, 2)),
                "{:?} != {:?}",
                val,
                float
            );
        }
    }

    #[test]
    fn interest_only() {
        let mut map = ContextMap::default();
        map.write_interest(Rad::turn_div_2(), 0.5);

        let (dir, speed) = map.resolve();
        assert!(dir.0.approx_eq(PI, (EPSILON, 2)));
        assert!(speed.approx_eq(0.5, (EPSILON, 2)));
    }

    #[test]
    fn falloff() {
        let mut map = SingleContextMap::<Interest>::default();
        map.write(rad(0.0), 0.5);

        let expected_value = Desire(128);
        assert_eq!(map.read_direct(Direction::North), expected_value);

        let (a, b) = (
            map.read_direct(Direction::NorthWest),
            map.read_direct(Direction::NorthEast),
        );
        assert!(a > Desire(0) && a < expected_value);
        assert_eq!(a, b);

        assert_eq!(map.read_direct(Direction::East), Desire(0));
        assert_eq!(map.read_direct(Direction::West), Desire(0));
    }
}
