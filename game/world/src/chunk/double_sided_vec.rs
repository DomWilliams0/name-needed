use std::i32;

pub struct DoubleSidedVec<T> {
    /// positive indices including 0
    positive: Vec<T>,

    /// negative indices
    negative: Vec<T>,
}

impl<T> DoubleSidedVec<T> {
    pub fn with_capacity(extent_capacity: usize) -> Self {
        Self {
            positive: Vec::with_capacity(extent_capacity),
            negative: Vec::with_capacity(extent_capacity),
        }
    }

    pub fn len(&self) -> usize {
        self.positive.len() + self.negative.len()
    }

    pub fn get(&self, index: i32) -> Option<&T> {
        match index.signum() {
            0 => self.positive.get(0),
            1 => self.positive.get(index as usize),
            -1 => self.negative.get((-index) as usize - 1),
            _ => unreachable!(),
        }
    }

    pub fn get_mut(&mut self, index: i32) -> Option<&mut T> {
        match index.signum() {
            0 => self.positive.get_mut(0),
            1 => self.positive.get_mut(index as usize),
            -1 => self.negative.get_mut((-index) as usize - 1),
            _ => unreachable!(),
        }
    }

    pub fn add(&mut self, value: T, index: i32) {
        let (idx, vec, expected_idx) = match index.signum() {
            0 => {
                let vec = &mut self.positive;
                if !vec.is_empty() {
                    panic!("zero can only be added if empty");
                }
                (index as usize, vec, 0)
            }
            1 => {
                let expected = self.positive.len();
                (index as usize, &mut self.positive, expected)
            }
            -1 => {
                let expected = self.negative.len() + 1;
                ((-index) as usize, &mut self.negative, expected)
            }
            _ => unreachable!(),
        };

        if idx != expected_idx {
            panic!(
                "no gaps allowed, next {} index must be {}",
                if index.is_positive() {
                    "positive"
                } else {
                    "negative"
                },
                index
            )
        }

        vec.push(value);
    }

    pub fn fill_until<F: Fn(i32) -> T>(&mut self, index: i32, val: F) {
        match index.signum() {
            0 => {
                // just zero
                self.add(val(0), 0);
            }
            1 => {
                let max = self.positive.len() as i32;
                for idx in max..=index {
                    self.add(val(idx), idx);
                }
            }
            -1 => {
                let max = -(self.negative.len() as i32 + 1);
                for idx in (index..=max).rev() {
                    self.add(val(idx), idx);
                }
            }
            _ => unreachable!(),
        };
    }

    pub fn iter_increasing(&self) -> impl Iterator<Item = &T> {
        self.negative.iter().rev().chain(self.positive.iter())
    }

    pub fn iter_decreasing(&self) -> impl Iterator<Item = &T> {
        self.positive.iter().rev().chain(self.negative.iter())
    }

    pub fn indices_increasing(&self) -> impl Iterator<Item = i32> {
        let lowest = -(self.negative.len() as i32);
        let highest = self.positive.len() as i32;

        lowest..highest
    }

    pub fn iter_mut_increasing(&mut self) -> impl Iterator<Item = &mut T> {
        self.negative
            .iter_mut()
            .rev()
            .chain(self.positive.iter_mut())
    }

    pub fn iter_mut_decreasing(&mut self) -> impl Iterator<Item = &mut T> {
        self.positive
            .iter_mut()
            .rev()
            .chain(self.negative.iter_mut())
    }
}

#[cfg(test)]
mod tests {
    use crate::chunk::double_sided_vec::DoubleSidedVec;

    #[test]
    fn expected() {
        let mut v = DoubleSidedVec::<i32>::with_capacity(8);
        assert_eq!(v.len(), 0);

        v.add(0, 0);
        v.add(1, 1);
        v.add(-1, -1);
        v.add(-2, -2);

        assert_eq!(v.get(0), Some(&0));
        assert_eq!(v.get(1), Some(&1));
        assert_eq!(v.get(2), None);
        assert_eq!(v.get(-1), Some(&-1));

        *v.get_mut(-2).unwrap() = 100;
        assert_eq!(v.get(-2), Some(&100));
        *v.get_mut(-2).unwrap() = -2; // set it back for next tests

        let collected: Vec<_> = v.iter_increasing().copied().collect();
        assert_eq!(collected, vec![-2, -1, 0, 1]);

        let collected: Vec<_> = v.indices_increasing().collect();
        assert_eq!(collected, vec![-2, -1, 0, 1]);

        let collected: Vec<_> = v.iter_decreasing().copied().collect();
        assert_eq!(collected, vec![1, 0, -1, -2]);

        for i in 2..100 {
            v.add(i, i)
        }

        let collected: Vec<_> = v.iter_increasing().copied().collect();
        assert_eq!(collected, (-2..100).collect::<Vec<_>>());
    }

    #[test]
    #[should_panic]
    fn bad_first() {
        let mut v = DoubleSidedVec::<i32>::with_capacity(8);
        v.add(0, 10);
    }

    #[test]
    #[should_panic]
    fn gaps_positive() {
        let mut v = DoubleSidedVec::<i32>::with_capacity(8);
        v.add(0, 0);
        v.add(1, 1);
        v.add(3, 3);
    }

    #[test]
    #[should_panic]
    fn gaps_negative() {
        let mut v = DoubleSidedVec::<i32>::with_capacity(8);
        v.add(0, 0);
        v.add(-1, -1);
        v.add(-3, -3);
    }

    #[test]
    #[should_panic]
    fn no_dupes_zero() {
        let mut v = DoubleSidedVec::<i32>::with_capacity(8);
        v.add(0, 0);
        v.add(0, 0);
    }

    #[test]
    #[should_panic]
    fn no_dupes() {
        let mut v = DoubleSidedVec::<i32>::with_capacity(8);
        v.add(0, 0);
        v.add(1, 0);
        v.add(2, 0);
        v.add(2, 0);
    }

    #[test]
    fn fill_until() {
        let mut v = DoubleSidedVec::<i32>::with_capacity(8);

        // expand to 0
        v.fill_until(0, |i| i);
        assert_eq!(v.indices_increasing().collect::<Vec<_>>(), vec![0]);

        // expand upwards
        v.fill_until(3, |i| i);
        assert_eq!(v.indices_increasing().collect::<Vec<_>>(), vec![0, 1, 2, 3]);

        // expand downwards
        v.fill_until(-4, |i| i);
        assert_eq!(
            v.indices_increasing().collect::<Vec<_>>(),
            vec![-4, -3, -2, -1, 0, 1, 2, 3]
        );

        // no change needed
        v.fill_until(2, |i| i);
        assert_eq!(
            v.indices_increasing().collect::<Vec<_>>(),
            vec![-4, -3, -2, -1, 0, 1, 2, 3]
        );
        v.fill_until(-3, |i| i);
        assert_eq!(
            v.indices_increasing().collect::<Vec<_>>(),
            vec![-4, -3, -2, -1, 0, 1, 2, 3]
        );
    }

    #[test]
    fn iter_mut() {
        let mut v = DoubleSidedVec::<i32>::with_capacity(4);
        v.add(0, 0);
        v.add(1, 1);
        v.add(-1, -1);
        v.add(-2, -2);

        assert_eq!(
            v.iter_mut_increasing().map(|x| *x).collect::<Vec<_>>(),
            vec![-2, -1, 0, 1]
        );
        assert_eq!(
            v.iter_mut_decreasing().map(|x| *x).collect::<Vec<_>>(),
            vec![1, 0, -1, -2]
        );
    }
}
