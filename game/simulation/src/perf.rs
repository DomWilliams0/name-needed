use arraydeque::{Array, ArrayDeque, Wrapping};
use std::time::Instant;

pub struct MovingAverage<A: Array<Item = f64>> {
    data: Box<ArrayDeque<A, Wrapping>>,
}

pub type Tick = MovingAverage<[f64; 20]>;
pub type Render = MovingAverage<[f64; 64]>;

#[derive(Default)]
pub struct Perf {
    pub tick: Tick,
    pub render: Render,
}

pub struct PerfAvg {
    pub tick: f64,
    pub render: f64,
}

pub struct Timing<'a, A: Array<Item = f64>> {
    start: Instant,
    consumer: &'a mut MovingAverage<A>,
}

impl<A: Array<Item = f64>> Default for MovingAverage<A> {
    fn default() -> Self {
        let data = Box::new(ArrayDeque::new());
        Self { data }
    }
}

impl<A: Array<Item = f64>> MovingAverage<A> {
    pub fn next(&mut self, value: f64) {
        self.data.push_back(value);
    }

    pub fn calculate(&self) -> f64 {
        // TODO detect if changed
        let count = self.data.len() as f64;
        self.data.iter().sum::<f64>() / count
    }

    pub fn time(&mut self) -> Timing<A> {
        Timing {
            start: Instant::now(),
            consumer: self,
        }
    }
}

impl<A: Array<Item = f64>> Drop for Timing<'_, A> {
    fn drop(&mut self) {
        let time = self.start.elapsed();
        self.consumer.next(time.as_secs_f64());
    }
}

impl Perf {
    pub fn calculate(&self) -> PerfAvg {
        PerfAvg {
            tick: self.tick.calculate(),
            render: self.render.calculate(),
        }
    }
}
