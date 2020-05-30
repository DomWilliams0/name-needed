# TODOs (140)
 * [.travis.yml](.travis.yml) (1)
   * `# TODO windows and osx`
 * [game/ai/src/decision.rs](game/ai/src/decision.rs) (3)
   * `fn considerations(&self) -> Vec<AiBox<dyn Consideration<C>>>; // TODO pooled vec/slice`
   * `// TODO optimization: dont consider all considerations every time`
   * `// TODO put in common test utils?`
 * [game/ai/src/intelligence.rs](game/ai/src/intelligence.rs) (4)
   * `// TODO pool/arena`
   * `// TODO optimize`
   * `// TODO + momentum to discourage changing so often`
   * `// TODO dumber agents shouldn't always choose the best`
 * [game/ai/src/lib.rs](game/ai/src/lib.rs) (1)
   * `// TODO pool/slab`
 * [game/simulation/src/ai/activity/items.rs](game/simulation/src/ai/activity/items.rs) (5)
   * `// TODO proper exertion calculation for item use`
   * `// TODO equipping will depend on the item's size in base+mounted inventories, not yet implemented`
   * `// TODO add ItemUseType which hints at which slot to use`
   * `// TODO the item moved while going to pick it up, what do`
   * `// TODO dont manually set the exact follow speed - choose a preset e.g. wander,dawdle,walk,fastwalk,run,sprint`
 * [game/simulation/src/ai/activity/mod.rs](game/simulation/src/ai/activity/mod.rs) (1)
   * `// TODO failure/interrupt reason`
 * [game/simulation/src/ai/activity/movement.rs](game/simulation/src/ai/activity/movement.rs) (2)
   * `// TODO wander *activity* exertion should be 0, but added to the exertion of walking at X speed`
   * `// TODO remove WANDER_SPEED constant when this is done`
 * [game/simulation/src/ai/dse/items.rs](game/simulation/src/ai/dse/items.rs) (1)
   * `// TODO "I can/want to move" consideration`
 * [game/simulation/src/ai/input.rs](game/simulation/src/ai/input.rs) (4)
   * `// TODO HasInInventoryGraded - returns number,quality of matches`
   * `// TODO old results are a subset of new results, should reuse`
   * `// TODO arena allocated vec return value`
   * `// TODO clearly needs some spatial partitioning here`
 * [game/simulation/src/ai/mod.rs](game/simulation/src/ai/mod.rs) (1)
   * `/// TODO ideally this would use ai::Context<'a> to represent the AI tick lifetime: https://github.com/rust-lang/rust/issues/44265`
 * [game/simulation/src/ai/system.rs](game/simulation/src/ai/system.rs) (3)
   * `// TODO only run occasionally - FIXME TERRIBLE HACK`
   * `// TODO use arena/bump allocator and share instance between entities`
   * `// TODO struclog events`
 * [game/simulation/src/dev.rs](game/simulation/src/dev.rs) (1)
   * `// TODO always make sure that putting an item into a contents removes its transform? only do this via a system`
 * [game/simulation/src/entity_builder.rs](game/simulation/src/entity_builder.rs) (2)
   * `// TODO add must_use to all builder patterns`
   * `nutrition: u16, /* TODO Fuel */`
 * [game/simulation/src/item/component.rs](game/simulation/src/item/component.rs) (10)
   * `// TODO this could do with a builder`
   * `pub class: ItemClass, // TODO possible for an item to have multiple classes?`
   * `// TODO proper nutritional value`
   * `// TODO food debris - the last X fuel/proportion is inedible and has to be disposed of`
   * `// TODO depending on their mood/personality this will be tossed to the ground or taken to a proper place`
   * `// TODO use item mass to determine how far it flies? or also aerodynamic-ness`
   * `// TODO drinkable`
   * `// TODO splatterable (after throw, if walked on)`
   * `// TODO weapon (damage to target per hit, damage to own condition per hit, attack speed, cooldown)`
   * `/// Item must be in base inventory to use TODO is this needed?`
 * [game/simulation/src/item/inventory/component.rs](game/simulation/src/item/inventory/component.rs) (6)
   * `dominant_base: SlotIndex, // TODO option`
   * `// TODO cache result of search until they change (specs::storage::Tracked?)`
   * `// TODO free up base slots for items bigger than 1`
   * `// TODO swap items bigger than 1`
   * `// TODO add a component that allows accessing your mounted storage - animals can wear them but not use!`
   * `// TODO what if original item is bigger than 1?`
 * [game/simulation/src/item/inventory/contents.rs](game/simulation/src/item/inventory/contents.rs) (3)
   * `// TODO item slot disabled by (lack of) physical wellbeing e.g. missing hand`
   * `// TODO can this be on the stack?`
   * `// TODO handle different item sizes`
 * [game/simulation/src/item/pickup.rs](game/simulation/src/item/pickup.rs) (1)
   * `// TODO store this in the system and reuse the allocation`
 * [game/simulation/src/needs/hunger.rs](game/simulation/src/needs/hunger.rs) (8)
   * `// fuel used per tick TODO depends on time rate`
   * `// TODO species metabolism`
   * `// TODO generic needs component with hunger/thirst/toilet/social etc`
   * `ReadStorage<'a, ActivityComponent>, // for current exertion TODO moving average`
   * `// TODO individual metabolism rate`
   * `// TODO compensate multipliers`
   * `let fuel_to_consume = BASE_EAT_RATE; // TODO individual rate`
   * `// TODO while eating/for a short time afterwards, add a hunger multiplier e.g. 0.2`
 * [game/simulation/src/path/debug.rs](game/simulation/src/path/debug.rs) (1)
   * `// TODO debug path renderer`
 * [game/simulation/src/path/mod.rs](game/simulation/src/path/mod.rs) (1)
   * `// TODO remove WANDER_SPEED`
 * [game/simulation/src/path/system.rs](game/simulation/src/path/system.rs) (2)
   * `warn!("failed to find path to target {:?}: {:?}", target, e); // TODO {} for error`
   * `// FIXME GROSS HACK`
 * [game/simulation/src/queued_update.rs](game/simulation/src/queued_update.rs) (1)
   * `// TODO pool/reuse these boxes`
 * [game/simulation/src/simulation.rs](game/simulation/src/simulation.rs) (4)
   * `#[allow(dead_code)] // TODO will be used when world can be modified`
   * `// TODO limit time/count`
   * `// TODO sort out systems so they always have a ecs_world reference`
   * `// TODO needs interpolation?`
 * [game/simulation/src/steer/context.rs](game/simulation/src/steer/context.rs) (2)
   * `// TODO average with previous for less sudden movements`
   * `// TODO follow gradients and choose continuous value`
 * [game/simulation/src/steer/system.rs](game/simulation/src/steer/system.rs) (2)
   * `// TODO struclog event`
   * `// TODO populate danger interests from world/other entity collisions`
 * [game/world/src/chunk/builder.rs](game/world/src/chunk/builder.rs) (1)
   * `// TODO create slice if missing`
 * [game/world/src/chunk/chunk.rs](game/world/src/chunk/chunk.rs) (1)
   * `// TODO still does a lot of unnecessary initialization`
 * [game/world/src/chunk/double_sided_vec.rs](game/world/src/chunk/double_sided_vec.rs) (1)
   * `// TODO refactor to use a single vec allocation`
 * [game/world/src/chunk/slab.rs](game/world/src/chunk/slab.rs) (1)
   * `// TODO does a slab really need to know this?`
 * [game/world/src/chunk/slice.rs](game/world/src/chunk/slice.rs) (1)
   * `// TODO make not pub`
 * [game/world/src/chunk/terrain.rs](game/world/src/chunk/terrain.rs) (8)
   * `// TODO expensive to clone, use Cow if actually necessary`
   * `// TODO could skip next slice because it cant be walkable if this one was?`
   * `// TODO set_block trait to reuse in ChunkBuilder (#46)`
   * `// TODO cow for empty slab`
   * `// TODO reuse a buffer for each slab`
   * `// TODO discover internal area links`
   * `// TODO transmute lifetimes instead`
   * `// TODO 1 area at z=0`
 * [game/world/src/grid.rs](game/world/src/grid.rs) (2)
   * `// TODO pub hardcoded :(`
   * `// TODO are %s optimised to bitwise ops if a multiple of 2?`
 * [game/world/src/loader/mod.rs](game/world/src/loader/mod.rs) (7)
   * `// TODO cache full finalized chunks`
   * `// TODO reuse/pool bufs`
   * `let mut area_edges = Vec::new(); // TODO reuse buf`
   * `let mut links = Vec::new(); // TODO reuse buf`
   * `let mut ports = Vec::new(); // TODO reuse buf`
   * `// TODO build up area graph nodes and edges (using a map in self of all loaded chunks->edge opacity/walkability?)`
   * `// TODO finally take WorldRef write lock and 1) update nav graph 2) add chunk`
 * [game/world/src/loader/terrain_source/mod.rs](game/world/src/loader/terrain_source/mod.rs) (1)
   * `fn all_chunks(&mut self) -> Vec<ChunkPosition>; // TODO gross`
 * [game/world/src/loader/worker_pool.rs](game/world/src/loader/worker_pool.rs) (2)
   * `// TODO if this thread panics, propagate to main game thread`
   * `// TODO detect this err condition?`
 * [game/world/src/mesh.rs](game/world/src/mesh.rs) (4)
   * `let mut vertices = Vec::<V>::new(); // TODO reuse/calculate needed capacity first`
   * `// TODO skip if slice knows it is empty`
   * `// TODO also rotate texture`
   * `// TODO half blocks`
 * [game/world/src/navigation/area_navigation.rs](game/world/src/navigation/area_navigation.rs) (2)
   * `|edge| edge.weight().cost.weight(), // TODO could prefer wider ports`
   * `// TODO dont allocate and throw away path`
 * [game/world/src/navigation/astar.rs](game/world/src/navigation/astar.rs) (4)
   * `// TODO reuse allocations`
   * `// TODO reuse allocation`
   * `// TODO reuse allocation`
   * `// TODO this might be expensive, can we build up the vec in order`
 * [game/world/src/navigation/block_navigation.rs](game/world/src/navigation/block_navigation.rs) (2)
   * `// TODO use vertical distance differently?`
   * `// TODO reuse vec allocation`
 * [game/world/src/navigation/cost.rs](game/world/src/navigation/cost.rs) (1)
   * `// TODO currently arbitrary, should depend on physical attributes`
 * [game/world/src/navigation/discovery.rs](game/world/src/navigation/discovery.rs) (2)
   * `// TODO shouldnt be pub`
   * `/// flood fill queue, pair of (pos, pos this was reached from) TODO share between slabs`
 * [game/world/src/navigation/path.rs](game/world/src/navigation/path.rs) (2)
   * `// TODO smallvecs`
   * `// TODO derive(Error)`
 * [game/world/src/viewer.rs](game/world/src/viewer.rs) (3)
   * `assert!(size > 0); // TODO Result`
   * `// TODO slice-aware chunk mesh caching, moving around shouldn't regen meshes constantly`
   * `// TODO cache?`
 * [game/world/src/world.rs](game/world/src/world.rs) (4)
   * `// TODO optimize path with raytracing (#50)`
   * `// TODO only calculate path for each area as needed (#51)`
   * `// TODO only invalidate lighting`
   * `// TODO build area graph in loader`
 * [game/world/src/world_ref.rs](game/world/src/world_ref.rs) (1)
   * `// TODO don't unwrap()`
 * [renderer/engine/src/engine.rs](renderer/engine/src/engine.rs) (1)
   * `// TODO separate faster rate for physics?`
 * [renderer/engine/src/render/sdl/backend.rs](renderer/engine/src/render/sdl/backend.rs) (1)
   * `// TODO cascade through other handlers`
 * [renderer/engine/src/render/sdl/camera.rs](renderer/engine/src/render/sdl/camera.rs) (3)
   * `// keep screen centre in the same place TODO only sometimes?`
   * `// TODO zoom`
   * `// TODO cache`
 * [renderer/engine/src/render/sdl/gl/vertex.rs](renderer/engine/src/render/sdl/gl/vertex.rs) (1)
   * `// TODO smallvec`
 * [renderer/engine/src/render/sdl/render/entity.rs](renderer/engine/src/render/sdl/render/entity.rs) (2)
   * `// TODO use buffersubdata to reuse allocation if len <=`
   * `// TODO cursor interface in ScopedMap`
 * [renderer/engine/src/render/sdl/render/mod.rs](renderer/engine/src/render/sdl/render/mod.rs) (1)
   * `// TODO use glBufferSubData to reuse the allocation if <= len`
 * [renderer/main/src/main.rs](renderer/main/src/main.rs) (2)
   * `// TODO preserve camera position and other runtime settings?`
   * `// TODO more granular - n for engine setup, n for sim setup, n for each frame?`
 * [renderer/main/src/presets/mod.rs](renderer/main/src/presets/mod.rs) (1)
   * `panic!("failed to wait for world to load: {:?}", err); // TODO return result`
 * [shared/color/src/lib.rs](shared/color/src/lib.rs) (1)
   * `/// TODO will this work with big endian?`
 * [shared/common/Cargo.toml](shared/common/Cargo.toml) (1)
   * `# TODO feature for cgmath`
 * [shared/config/src/load.rs](shared/config/src/load.rs) (1)
   * `// TODO add a variant that returns a default instead of panicking`
 * [shared/struclog/Cargo.toml](shared/struclog/Cargo.toml) (1)
   * `# TODO bincode = {version = "^1.2", optional = true}`
 * [shared/unit/src/dim.rs](shared/unit/src/dim.rs) (1)
   * `// TODO helper for this-1`
 * [shared/unit/src/world/slice_index.rs](shared/unit/src/world/slice_index.rs) (2)
   * `// TODO differentiate slice in a slab and slice in a chunk`
   * `// TODO move slab to unit`
 * [shared/unit/src/world/world_position.rs](shared/unit/src/world/world_position.rs) (1)
   * `// TODO refactor self.2 usage to use this everywhere`
