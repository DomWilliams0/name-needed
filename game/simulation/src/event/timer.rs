use crate::simulation::Tick;
use common::*;
use std::cmp::Reverse;

pub trait Token: Sized + Copy + Default + Debug + Eq {
    /// Returns previous value and incs self
    fn increment(&mut self) -> Self;
}

pub struct Timer<D, T: Token> {
    end_tick: Tick,
    token: T,
    data: D,
}

pub struct Timers<D, T: Token> {
    timers: Vec<Timer<D, T>>,
    next_token: T,
}

/// Unique token to differentiate timers
#[derive(Eq, PartialEq, Copy, Clone)]
pub struct TimerToken(u64);

impl<D, T: Token> Timer<D, T> {
    pub fn elapsed(&self, current: Tick) -> bool {
        // TODO move this into Tick
        current.value() >= self.end_tick.value()
    }
}

impl<D, T: Token> Default for Timers<D, T> {
    fn default() -> Self {
        Self {
            timers: Vec::with_capacity(64),
            next_token: T::default(),
        }
    }
}

impl<D, T: Token> Timers<D, T> {
    pub fn maintain(&mut self, current: Tick) -> impl Iterator<Item = (T, D)> + '_ {
        // sort in reverse order, for efficient truncating
        // TODO sort by elapsed() bool instead
        // TODO might be better to just insert sorted
        self.timers
            .sort_unstable_by_key(|t| Reverse(t.end_tick.value()));

        let first_elapsed = self
            .timers
            .iter()
            .position(|t| t.elapsed(current))
            .unwrap_or_else(|| self.timers.len());

        self.timers
            .drain(first_elapsed..)
            .rev()
            .map(|t| (t.token, t.data))
    }

    /// Returns (end tick, token)
    pub fn schedule(&mut self, relative_ticks: u32, data: D) -> (Tick, T) {
        self.schedule_with(Tick::fetch(), relative_ticks, data)
    }

    /// Returns (end tick, token)
    fn schedule_with(&mut self, current: Tick, relative_ticks: u32, data: D) -> (Tick, T) {
        let token = self.next_token.increment();

        let end_tick = current + relative_ticks;
        self.timers.push(Timer {
            end_tick,
            token,
            data,
        });

        trace!("scheduled timer for {tick} (+{n})", tick = end_tick.value(), n = relative_ticks; "token" => ?token);

        (end_tick, token)
    }

    pub fn cancel(&mut self, token: T) -> bool {
        if let Some(idx) = self.timers.iter().position(|t| t.token == token) {
            self.timers.swap_remove(idx);
            true
        } else {
            false
        }
    }
}

impl Default for TimerToken {
    fn default() -> Self {
        TimerToken(0x4000)
    }
}

impl Token for TimerToken {
    fn increment(&mut self) -> Self {
        let ret = *self;
        self.0 += 1;
        ret
    }
}

impl Debug for TimerToken {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "TimerToken({:#x})", self.0)
    }
}

impl Token for () {
    fn increment(&mut self) -> Self {
        // nop
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::many_single_char_names)]

    use super::*;

    #[test]
    fn maintain() {
        let mut timers = Timers::<i32, TimerToken>::default();

        let now = Tick::with(10);
        let (_, a) = timers.schedule_with(now, 2, 0);
        let (_, b) = timers.schedule_with(now, 4, 1);
        let (_, d) = timers.schedule_with(now, 8, 2);
        let (_, c) = timers.schedule_with(now, 6, 3);

        let now = Tick::with(11);
        let finished = timers.maintain(now).collect_vec();
        assert!(finished.is_empty());

        let now = Tick::with(15);
        let finished = timers.maintain(now).collect_vec();
        assert_eq!(finished, vec![(a, 0), (b, 1)]);

        // late to the party
        let (_, e) = timers.schedule_with(now, 15, 4);
        let (_, f) = timers.schedule_with(now, 16, 5);

        let now = Tick::with(22);
        let finished = timers.maintain(now).collect_vec();
        assert_eq!(finished, vec![(c, 3), (d, 2)]);

        assert!(timers.cancel(f));
        assert!(!timers.cancel(f)); // already cancelled

        let now = Tick::with(40);
        let finished = timers.maintain(now).collect_vec();
        assert_eq!(finished, vec![(e, 4)]);

        let now = Tick::with(50);
        let finished = timers.maintain(now).collect_vec();
        assert!(finished.is_empty());
    }
}
