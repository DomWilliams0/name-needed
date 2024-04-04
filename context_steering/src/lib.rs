use itertools::Itertools;
use std::f32::consts::TAU;

#[derive(Copy, Clone, Default, Ord, PartialOrd, Eq, PartialEq, Debug)]
struct Desire(u8);

impl Desire {
    fn zero(&mut self) {
        self.0 = 0
    }
}

impl From<f32> for Desire {
    fn from(value: f32) -> Self {
        debug_assert!(
            (0.0..=1.0).contains(&value),
            "value out of range: {}",
            value
        );

        let as_float = (value * (u8::MAX as f32)).ceil();
        let desire = as_float as u8;
        Desire(desire)
    }
}

impl From<Desire> for f32 {
    fn from(desire: Desire) -> Self {
        if desire.0 == 255 {
            // special edge case
            1.0
        } else {
            desire.0 as f32 / (u8::MAX as f32 + 1.0)
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct SingleContextMap<const N: usize>([Desire; N]);

impl<const N: usize> Default for SingleContextMap<N> {
    fn default() -> Self {
        Self([Desire::default(); N])
    }
}

/// Normalises radians to 0..2PI
fn normalise_angle(f: f32) -> f32 {
    let rem = f % TAU;
    if rem < 0.0 {
        rem + TAU
    } else {
        rem
    }
}

impl<const N: usize> SingleContextMap<N> {
    fn angle_to_index(angle: f32) -> usize {
        let f = normalise_angle(angle) / TAU;
        (f * N as f32) as usize
    }

    fn index_to_angle(index: usize) -> f32 {
        debug_assert!(index < N);
        (index as f32) / (N as f32) * TAU
    }

    fn write_unchecked(&mut self, val: f32, idx: usize) {
        debug_assert!(idx < N);
        unsafe {
            self.0.get_unchecked_mut(idx).0 = self
                .0
                .get_unchecked(idx)
                .0
                .saturating_add(Desire::from(val).0);
        }
    }

    pub fn write(&mut self, angle: f32, val: f32, fallback_fn: impl Fn(usize) -> f32) {
        let idx = Self::angle_to_index(angle);

        self.write_unchecked(val, idx);

        let mut val = val;
        let mut n = 1;
        loop {
            let falloff = fallback_fn(n);
            debug_assert!((0.0..=1.0).contains(&falloff));

            val *= falloff;
            if val < 0.1 {
                break;
            }

            self.write_unchecked(val, (idx + n) % N);
            self.write_unchecked(val, (idx + N - n) % N);

            n += 1;
        }
    }

    fn merge_into(&mut self, mut other: Self, other_weight: f32) {
        self.0
            .iter_mut()
            .zip(other.0.iter_mut())
            .for_each(|(a, b)| {
                let f = (1.0 - other_weight) * f32::from(*a) + other_weight * f32::from(*b);
                *a = if f < 0.05 { Desire(0) } else { f.into() };
            })
    }
}

#[derive(Default, Copy, Clone)]
pub struct ContextMap<const N: usize> {
    interest: SingleContextMap<N>,
    danger: SingleContextMap<N>,
}

fn fallback_dropoff(i: usize, speed: f32) -> f32 {
    1.0 - (i as f32 * speed)
}

impl<const N: usize> ContextMap<N> {
    pub fn write_interest(&mut self, angle: f32, val: f32) {
        self.interest
            .write(angle, val, |i| fallback_dropoff(i, 0.2));
    }

    pub fn write_danger(&mut self, angle: f32, val: f32) {
        self.danger.write(angle, val, |i| fallback_dropoff(i, 0.5));
    }

    pub fn interests_mut(&mut self) -> &mut SingleContextMap<N> {
        &mut self.interest
    }

    pub fn merge_into(&mut self, other: Self, other_weight: f32) {
        self.interest.merge_into(other.interest, other_weight);
        self.danger.merge_into(other.danger, other_weight);
    }

    pub fn resolve(mut self) -> (f32, f32) {
        // trace!("interest: {:?}", self.interest);
        // trace!("danger  : {:?}", self.danger);
        // find min danger
        let min_danger = *self.danger.0.iter().min().unwrap();

        // mask out higher dangers and the corresponding interests
        self.danger
            .0
            .iter_mut()
            .zip(self.interest.0.iter_mut())
            .for_each(|(d, i)| {
                if *d > min_danger {
                    d.zero();
                    i.zero();
                }
            });

        // choose highest interest
        // TODO follow gradients and choose continuous value
        let best_idx = self.interest.0.iter().position_max().unwrap();

        let rad = SingleContextMap::<N>::index_to_angle(best_idx);
        let desire = self.interest.0[best_idx];
        // trace!("orig danger  : {:?}", prev.danger.values);
        // trace!("orig interest: {:?}", prev.interest.values);
        //
        // trace!("remaining danger  : {:?}", self.danger.values);
        // trace!("remaining interest: {:?}", self.interest.values);
        // trace!(
        //     "min_danger={:?}, best_direction={}",
        //     min_danger,
        //     best_direction
        // );
        (rad, desire.into())
    }

    /// Positive=interest, negative=danger. -1..=1
    pub fn iter(&self) -> impl Iterator<Item = (f32, f32)> + '_ {
        self.interest
            .0
            .iter()
            .zip(self.danger.0.iter())
            .enumerate()
            .map(|(i, (interest, danger))| {
                let direction = SingleContextMap::<N>::index_to_angle(i);
                let desire = if interest > danger {
                    f32::from(*interest)
                } else {
                    -(f32::from(*danger))
                };
                (direction, desire)
            })
    }
}
