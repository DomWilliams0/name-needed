#![cfg(feature = "testing")]
use crate::tests::{TestHelper, TestWrapper};
use common::BoxedResult;
use simulation::input::UiCommands;
use simulation::{EntityEvent, SimulationRefLite};
use std::mem::{ManuallyDrop, MaybeUninit};
use std::sync::Once;

inventory::collect!(TestDeclaration);

macro_rules! declare_test {
    ($($test:ty)+ ) => {
        pub fn register() {}

        $(
            inventory::submit! { TestDeclaration {
                name: stringify!($test),
                init: crate::cast_init_fn(<$test>::on_init),
                tick: crate::cast_tick_fn(<$test>::on_tick),
                destructor: crate::cast_destructor_fn(::std::ptr::drop_in_place::<$test>),
            }}
        )+
    };
}

mod helpers;
pub mod tests;

pub struct HookContext<'a> {
    pub simulation: SimulationRefLite<'a>,
    pub commands: &'a mut UiCommands,
    /// Events triggered since last tick
    pub events: Vec<EntityEvent>,
}

pub enum InitHookResult<T> {
    Success(Box<T>),
    TestSuccess,
    TestFailure(String),
}

pub enum HookResult {
    KeepGoing,
    TestSuccess,
    TestFailure(String),
}

pub(crate) fn cast_tick_fn<T>(func: fn(&mut T, TestHelper, &'_ HookContext) -> HookResult) -> Tick {
    unsafe { std::mem::transmute(func) }
}

pub(crate) fn cast_init_fn<T>(func: fn(TestHelper, &'_ HookContext) -> InitHookResult<T>) -> Init {
    unsafe { std::mem::transmute(func) }
}

pub(crate) fn cast_destructor_fn<T>(func: unsafe fn(*mut T)) -> Destructor {
    unsafe { std::mem::transmute(func) }
}

pub type TickHookThunk = fn(&'_ HookContext) -> HookResult;

pub type Tick = fn(&'_ mut (), TestHelper, &'_ HookContext) -> HookResult;
pub type Init = fn(TestHelper, &'_ HookContext) -> InitHookResult<()>;
pub type Destructor = unsafe fn(*mut ());

pub struct TestDeclaration {
    pub name: &'static str,
    pub(crate) init: Init,
    pub(crate) tick: Tick,
    pub(crate) destructor: Destructor,
}

pub struct TestInstance {
    pub name: &'static str,
    init: Init,
    tick: Tick,
    destructor: Destructor,
    instance: TestWrapper,
}

pub const TEST_NAME_VAR: &str = "NN_TEST_NAME_CURRENT";

static mut DESTROYED: bool = false;
static mut INSTANCE: MaybeUninit<&'static mut TestInstance> = MaybeUninit::uninit();

fn current() -> &'static mut TestInstance {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        register_tests();

        let name = std::env::var(TEST_NAME_VAR)
            .unwrap_or_else(|_| panic!("missing env var {:?}", TEST_NAME_VAR));
        let test = inventory::iter::<TestDeclaration>
            .into_iter()
            .find(|t| t.name == name)
            .unwrap_or_else(|| panic!("test with name {:?} not found", name));

        let instance = Box::new(TestInstance {
            name: test.name,
            init: test.init,
            tick: test.tick,
            destructor: test.destructor,
            instance: TestWrapper::default(),
        });

        unsafe {
            INSTANCE = MaybeUninit::new(&mut *Box::into_raw(instance));
        }
    });

    unsafe {
        assert!(!DESTROYED, "test instance has been destroyed");
        *INSTANCE.as_mut_ptr()
    }
}

/// Called by engine
pub fn init_hook(ctx: &HookContext) -> HookResult {
    let test = current();
    let helper = test.instance.helper();
    test.instance.invoke_init(|| (test.init)(helper, ctx))
}

pub fn destroy_hook() {
    unsafe {
        assert!(!DESTROYED, "test instance has already been destroyed");
        let _instance: Box<TestInstance> = Box::from_raw(current() as *mut _);
        DESTROYED = true;
    }
}

/// Called by engine
pub fn tick_hook(ctx: &HookContext) -> HookResult {
    let test = current();
    let helper = test.instance.helper();
    test.instance
        .invoke_with_self(|this| (test.tick)(this, helper, ctx))
}

/// Called by engine
pub fn current_test_name() -> &'static str {
    current().name
}

/// inventory doesn't work unless the test module object is actually referenced, defeating the
/// whole purpose of using inventory
pub fn register_tests() {
    use tests::*;

    build::register();
    dummy::register();
    equip_with_pickup::register();
    haul::register();
}

impl HookResult {
    pub fn try_ongoing(res: BoxedResult<()>) -> Self {
        match res {
            Ok(_) => Self::KeepGoing,
            Err(err) => Self::TestFailure(format!("{}", err)),
        }
    }
}

impl<T> From<BoxedResult<T>> for InitHookResult<T> {
    fn from(res: BoxedResult<T>) -> Self {
        match res {
            Ok(res) => Self::Success(Box::new(res)),
            Err(err) => Self::TestFailure(format!("{}", err)),
        }
    }
}

impl Drop for TestInstance {
    fn drop(&mut self) {
        let test_instance = self.instance.take_inner();
        if let Some(instance) = test_instance {
            let this_ptr = Box::leak(ManuallyDrop::into_inner(instance)); // leak the box, doesn't matter

            unsafe {
                (self.destructor)(this_ptr as *mut _);
            }
        }
    }
}
