use rlua::prelude::*;
use rlua::{Context, MetaMethod, StdLib, UserData, UserDataMethods, Variadic};

use crate::ecs::EntityWrapper;
use crate::input::SelectedEntity;
use crate::scripting::context::{parse_entity_id, Scripting, ScriptingError, ScriptingResult};
use crate::{ComponentWorld, EcsWorld, PlayerSociety, SocietyHandle, WorldRef};
use common::*;

pub struct LuaScripting {
    runtime: rlua::Lua,
}

/// Key used in lua registry
const GAME_STATE_KEY: &str = "game-state";

/// Temporary references to game state for use in scripts
#[derive(Copy, Clone)]
struct LuaGameState<'a> {
    ecs: &'a EcsWorld,
    world: &'a WorldRef,
}

/// Guard that removes the temporary references from the lua registry on drop
struct LuaGameStateGuard;

impl UserData for LuaGameState<'_> {}

// safety: the "static" references aren't actually static, but:
//  - scripts are run synchronously on the main thread with valid references
//  - scripts finish before returning and can't start any coroutines (stdlib not loaded)
//  - scripts can't access or store these references
unsafe impl Send for LuaGameState<'static> {}

impl UserData for EntityWrapper {}
impl UserData for SocietyHandle {
    fn add_methods<'lua, T: UserDataMethods<'lua, Self>>(_methods: &mut T) {
        _methods.add_meta_function(MetaMethod::ToString, |_, this: Self| {
            Ok(format!("{:?}", this))
        });
    }
}

impl Scripting for LuaScripting {
    fn new() -> Result<Self, ScriptingError> {
        let std = {
            let mut std = StdLib::ALL_NO_DEBUG;
            std.remove(StdLib::COROUTINE); // no threads for you
            std
        };
        let runtime = Lua::new_with(std);
        runtime.set_memory_limit(Some(10 * 1024 * 1024));
        // TODO configure lua GC

        runtime.context(populate_globals)?;

        Ok(Self { runtime })
    }

    fn run(&mut self, script: &[u8], ecs: &EcsWorld, world: &WorldRef) -> ScriptingResult<()> {
        let state = LuaGameState { ecs, world };

        self.runtime
            .context(|ctx| {
                let guard = state.install(ctx)?;
                let result = ctx.load(script).exec();
                guard.uninstall(ctx)?;
                result
            })
            .map_err(Into::into)
    }
}

fn populate_globals(ctx: Context) -> ScriptingResult<()> {
    let globals = ctx.globals();

    macro_rules! define {
        (fn $name:ident $func:expr) => {
            globals.set(stringify!($name), ctx.create_function($func)?)?
        };
    }

    // remove print, use logging levels instead
    globals.set("print", LuaNil)?;

    define!(fn info |_, msg: Variadic<String>| {
        common::info!("lua: {}", msg.into_iter().join(", "));
        Ok(())
    });

    define!(fn debug |_, msg: Variadic<String>| {
        common::debug!("lua: {}", msg.into_iter().join(", "));
        Ok(())
    });

    define!(
        fn GetEntityById |ctx: Context, eid: String| {
            let state: LuaGameState = ctx.named_registry_value(GAME_STATE_KEY)?;

            // parse and check entity is alive
            let entity = parse_entity_id(&eid)
                .ok_or_else(|| ScriptingError::InvalidEntityId(eid.clone()))
                .and_then(|e| state.ensure_alive(e))
                .map_err(rlua::Error::external)?;
            Ok(entity)
        }
    );

    define!(fn SelectEntity |ctx: Context, eid: EntityWrapper| {
        let state: LuaGameState = ctx.named_registry_value(GAME_STATE_KEY)?;

        state
            .ensure_alive(eid)
            .map(|e| {
                let selected = state.ecs.resource_mut::<SelectedEntity>();
                selected.select(state.ecs, e.into());
            })
            .map_err(rlua::Error::external)?;

        Ok(())
    });

    define!(fn UnselectEntity |ctx: Context, _: ()| {
        let state: LuaGameState = ctx.named_registry_value(GAME_STATE_KEY)?;

        let selected = state.ecs.resource_mut::<SelectedEntity>();
        selected.unselect(state.ecs);

        Ok(())
    });

    define!(fn GetPlayerSociety |ctx, _: ()| {
        let state: LuaGameState = ctx.named_registry_value(GAME_STATE_KEY)?;

        let society = state.ecs.resource::<PlayerSociety>();
        Ok(society.0)
    });

    Ok(())
}

impl<'a> LuaGameState<'a> {
    fn install(self, context: Context) -> rlua::Result<LuaGameStateGuard> {
        // safety: registry value is "static" in that it lives for the lifetime of the script, it's
        // removed when the returned guard is dropped
        let state_static: LuaGameState<'static> = unsafe { std::mem::transmute(self) };
        context.set_named_registry_value(GAME_STATE_KEY, state_static)?;
        Ok(LuaGameStateGuard)
    }

    fn ensure_alive(&self, entity: EntityWrapper) -> ScriptingResult<EntityWrapper> {
        if self.ecs.is_entity_alive(entity.into()) {
            Ok(entity)
        } else {
            Err(ScriptingError::DeadEntity(entity))
        }
    }
}

impl LuaGameStateGuard {
    fn uninstall(self, ctx: Context) -> rlua::Result<()> {
        ctx.unset_named_registry_value(GAME_STATE_KEY)
    }
}
