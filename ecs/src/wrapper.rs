use pyro::{SoaStorage, World};

use crate::system::{System, SystemWrapper};

type Systems = Vec<SystemWrapper>;

pub struct ECS {
    world: World<SoaStorage>,
    systems: Systems,
    current_tick: usize,
}

impl ECS {
    fn new(systems: Systems) -> Self {
        Self {
            world: World::new(),
            systems,
            current_tick: 0,
        }
    }

    pub fn tick(&mut self) {
        // TODO sort systems into buckets instead of doing these checks every time
        for sys in &mut self.systems {
            let ticks_since_last = self.current_tick - sys.last_tick;

            if ticks_since_last >= sys.system.tick_frequency().stride() {
                sys.last_tick = self.current_tick;
                sys.system.tick(&self.world, ticks_since_last);
            }
        }

        self.current_tick += 1;
    }
}

pub struct ECSBuilder {
    systems: Systems,
}

impl ECSBuilder {
    pub fn new() -> Self {
        Self {
            systems: Systems::new(),
        }
    }

    pub fn with<S: System + 'static>(mut self, system: S) -> Self {
        let wrapper = SystemWrapper {
            system: Box::new(system),
            last_tick: 0,
        };
        self.systems.push(wrapper);
        self
    }

    pub fn build(self) -> ECS {
        ECS::new(self.systems)
    }
}
