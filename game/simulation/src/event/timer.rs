use std::cmp::Reverse;

use common::*;

use crate::simulation::Tick;

/// Unique token to differentiate timers
#[derive(Eq, PartialEq, Copy, Clone)]
pub struct TimerToken(u64);

pub struct Timer<D> {
    end_tick: Tick,
    token: TimerToken,
    data: D,
}

pub struct Timers<D> {
    timers: Vec<Timer<D>>,
    next_token: TimerToken,
    modified: bool,
}

impl<D> Timer<D> {
    pub fn elapsed(&self, current: Tick) -> bool {
        // TODO move this into Tick
        current.value() >= self.end_tick.value()
    }
}

impl<D> Default for Timers<D> {
    fn default() -> Self {
        Self {
            timers: Vec::with_capacity(64),
            next_token: TimerToken::default(),
            modified: false,
        }
    }
}

impl<D> Timers<D> {
    pub fn maintain(&mut self, current: Tick) -> impl Iterator<Item = (TimerToken, D)> + '_ {
        // sort by end tick in reverse order, for efficient truncating
        if self.modified {
            self.timers
                .sort_unstable_by_key(|t| Reverse(t.end_tick.value()));
            self.modified = false;
        } else {
            debug_assert_eq!(
                self.timers
                    .iter()
                    .map(|t| (t.token, t.end_tick))
                    .sorted_unstable_by_key(|(_, end)| Reverse(end.value()))
                    .map(|(t, _)| t)
                    .collect_vec(),
                self.timers.iter().map(|t| t.token).collect_vec(),
                "not sorted"
            )
        }

        let first_elapsed = self
            .timers
            .iter()
            .position(|t| t.elapsed(current))
            .unwrap_or(self.timers.len());

        self.timers
            .drain(first_elapsed..)
            .rev()
            .map(|t| (t.token, t.data))
    }

    /// Returns (end tick, token)
    pub fn schedule(&mut self, relative_ticks: u32, data: D) -> (Tick, TimerToken) {
        self.schedule_with(Tick::fetch(), relative_ticks, data)
    }

    /// Returns (end tick, token)
    fn schedule_with(&mut self, current: Tick, relative_ticks: u32, data: D) -> (Tick, TimerToken) {
        let token = self.next_token.increment();

        let end_tick = current + relative_ticks;
        self.timers.push(Timer {
            end_tick,
            token,
            data,
        });
        self.modified = true;

        trace!("scheduled timer for {tick} (+{n})", tick = end_tick.value(), n = relative_ticks; "token" => ?token);

        (end_tick, token)
    }

    pub fn cancel(&mut self, token: TimerToken) -> bool {
        if let Some(idx) = self.timers.iter().position(|t| t.token == token) {
            self.timers.swap_remove(idx);
            self.modified = true;
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

impl TimerToken {
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

#[cfg(test)]
mod tests {
    #![allow(clippy::many_single_char_names)]

    use super::*;

    #[test]
    fn maintain() {
        let mut timers = Timers::<i32>::default();

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
