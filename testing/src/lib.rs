use crate::tests::{TestHelper, TestWrapper};
use simulation::input::UiCommands;
use simulation::SimulationRefLite;
use std::any::Any;
use std::mem::MaybeUninit;
use std::sync::Once;

inventory::collect!(TestDeclaration);

macro_rules! declare_test {
    ($test:ty) => {
        pub fn register() {}
        impl $test {
            fn __create() -> Box<dyn std::any::Any> {
                Box::new(Self::default())
            }
        }
        inventory::submit! { TestDeclaration {
            name: stringify!($test),
            init: crate::cast_fn(<$test>::on_init),
            tick: crate::cast_fn(<$test>::on_tick),
            create_instance: <$test>::__create,
        }}
    };
}

pub mod tests;

pub struct HookContext<'a> {
    pub simulation: SimulationRefLite<'a>,
    pub commands: &'a mut UiCommands,
}

pub enum HookResult {
    KeepGoing,
    TestSuccess,
    TestFailure(String),
}

pub(crate) type FnGeneric<T> = fn(&mut T, TestHelper, &'_ HookContext) -> HookResult;
pub(crate) fn cast_fn<T>(generic: FnGeneric<T>) -> Tick {
    unsafe { std::mem::transmute(generic) }
}

pub type TickHookThunk = fn(&'_ HookContext) -> HookResult;

pub type Tick = fn(&'_ mut (), TestHelper, &'_ HookContext) -> HookResult;
pub type Init = Tick;

pub struct TestDeclaration {
    pub name: &'static str,
    pub(crate) init: Init,
    pub(crate) tick: Tick,
    pub(crate) create_instance: fn() -> Box<dyn Any>,
}

pub struct TestInstance {
    pub name: &'static str,
    init: Init,
    tick: Tick,
    instance: TestWrapper,
}

pub const TEST_NAME_VAR: &str = "NN_TEST_NAME_CURRENT";

fn current() -> &'static mut TestInstance {
    static ONCE: Once = Once::new();
    static mut INSTANCE: MaybeUninit<&'static mut TestInstance> = MaybeUninit::uninit();
    ONCE.call_once(|| {
        register_tests();

        let name = std::env::var(TEST_NAME_VAR)
            .unwrap_or_else(|_| panic!("missing env var {:?}", TEST_NAME_VAR));
        let test = inventory::iter::<TestDeclaration>
            .into_iter()
            .find(|t| t.name == &name)
            .unwrap_or_else(|| panic!("test with name {:?} not found", name));

        let instance = Box::new(TestInstance {
            name: test.name,
            init: test.init,
            tick: test.tick,
            instance: TestWrapper::new((test.create_instance)()),
        });

        unsafe {
            INSTANCE = MaybeUninit::new(Box::leak(instance));
        }
    });

    unsafe { *INSTANCE.as_mut_ptr() }
}

/// Called by engine
pub fn init_hook(ctx: &HookContext) -> HookResult {
    let test = current();
    let helper = test.instance.helper();
    test.instance
        .invoke_with_self(|this| (test.init)(this, helper, ctx))
}

/// Called by engine
pub fn tick_hook(ctx: &HookContext) -> HookResult {
    let test = current();
    let helper = test.instance.helper();
    test.instance
        .invoke_with_self(|this| (test.tick)(this, helper, ctx))
}

/// inventory doesn't work unless the test module object is actually referenced, defeating the
/// whole purpose of using inventory
pub fn register_tests() {
    use tests::*;

    dummy::register();
}
