use log::info;
use std::cell::Cell;
use std::time::Instant;

pub struct GameLoop {
    start_time: Instant,

    skip_ticks: usize,
    max_frameskip: u32,

    next_game_tick: Cell<usize>, // ms
}

pub struct FrameGuard<'a> {
    game_loop: &'a GameLoop,
}

pub struct FrameActions<'a> {
    game_loop: &'a GameLoop,

    loops: u32,
    rendered: bool,
}

#[derive(Debug)]
pub enum FrameAction {
    Tick,
    Render { interpolation: f64 },
}

impl GameLoop {
    pub fn new(tps: usize, max_frameskip: u32) -> Self {
        let start_time = Instant::now();
        let skip_ticks = 1000 / tps;
        info!(
            "initialized with {} ticks/second ({}ms/tick), with a max frame skip of {}",
            tps, skip_ticks, max_frameskip
        );
        Self {
            start_time,
            max_frameskip,
            skip_ticks,
            next_game_tick: Cell::new(0),
        }
    }

    pub fn start_frame(&self) -> FrameGuard {
        FrameGuard { game_loop: self }
    }

    fn tick_count(&self) -> usize {
        self.start_time.elapsed().as_millis() as usize
    }

    fn increment_next_game_tick(&self) {
        self.next_game_tick
            .set(self.next_game_tick.get() + self.skip_ticks)
    }
}

impl<'a> FrameGuard<'a> {
    pub fn actions(self) -> FrameActions<'a> {
        FrameActions {
            game_loop: self.game_loop,
            loops: 0,
            rendered: false,
        }
    }
}

impl<'a> Iterator for FrameActions<'a> {
    type Item = FrameAction;

    fn next(&mut self) -> Option<Self::Item> {
        let next_tick = &self.game_loop.next_game_tick;

        if self.game_loop.tick_count() > next_tick.get()
            && self.loops < self.game_loop.max_frameskip
        {
            self.game_loop.increment_next_game_tick();
            self.loops += 1;
            return Some(FrameAction::Tick);
        }

        if !self.rendered {
            self.rendered = true;

            let render_time = self.game_loop.tick_count();
            let skip_ticks = self.game_loop.skip_ticks;
            let interpolation: f64 =
                ((render_time + skip_ticks - next_tick.get()) as f64) / (skip_ticks as f64);

            return Some(FrameAction::Render { interpolation });
        }

        None
    }
}
