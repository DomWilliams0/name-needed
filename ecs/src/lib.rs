mod system;
mod wrapper;

#[cfg(test)]
mod tests {
    use pyro::{All, Read, SoaStorage, World, Write};

    use crate::system::{System, TickFrequency};
    use crate::wrapper::ECSBuilder;

    #[test]
    fn tick_frequencies() {
        struct CriticalSystem(i32);

        impl System for CriticalSystem {
            fn tick_frequency(&self) -> TickFrequency {
                TickFrequency::Critical
            }

            fn tick(&mut self, world: &World<SoaStorage>, ticks_since_last: usize) {
                assert_eq!(ticks_since_last, self.tick_frequency().stride());

                self.0 += 1;
                assert!(self.0 <= 12);
            }
        }

        struct RegularSystem(i32);
        impl System for RegularSystem {
            fn tick_frequency(&self) -> TickFrequency {
                TickFrequency::Regularly
            }

            fn tick(&mut self, world: &World<SoaStorage>, ticks_since_last: usize) {
                assert_eq!(ticks_since_last, self.tick_frequency().stride());

                self.0 += 1;
                assert!(self.0 <= 3);
            }
        }

        struct RareSystem;
        impl System for RareSystem {
            fn tick_frequency(&self) -> TickFrequency {
                TickFrequency::Rarely
            }

            fn tick(&mut self, world: &World<SoaStorage>, ticks_since_last: usize) {
                assert!(false, "should not have been called");
            }
        }

        let mut ecs = ECSBuilder::new()
            .with(CriticalSystem(0))
            .with(RegularSystem(0))
            .with(RareSystem)
            .build();

        for _ in 0..12 {
            ecs.tick();
        }
    }
}
