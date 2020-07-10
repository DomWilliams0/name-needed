# TODOs (169)
 * [.travis.yml](.travis.yml) (1)
   * `# TODO windows and osx`
 * [game/ai/src/decision.rs](game/ai/src/decision.rs) (2)
   * `/// TODO pooled vec/slice rather than Vec each time`
   * `// TODO optimization: dont consider all considerations every time`
 * [game/ai/src/intelligence.rs](game/ai/src/intelligence.rs) (7)
   * `// TODO pool/arena allocator`
   * `// TODO optimize: not all decisions need to be checked each time, but at least zero all scores`
   * `// TODO DSEs should be immutable, with scores stored somewhere else e.g. parallel array`
   * `// TODO add momentum to discourage changing mind so often`
   * `// TODO reuse allocation`
   * `// TODO benchmark adding and popping smarts`
   * `// TODO reuse allocation`
 * [game/procgen/src/lib.rs](game/procgen/src/lib.rs) (1)
   * `// TODO generate lower res noise and scale up`
 * [game/simulation/src/ai/activity/items.rs](game/simulation/src/ai/activity/items.rs) (4)
   * `// TODO proper exertion calculation for item use`
   * `// TODO equipping will depend on the item's size in base+mounted inventories, not yet implemented`
   * `// TODO add ItemUseType which hints at which slot to use`
   * `// TODO dont manually set the exact follow speed - choose a preset e.g. wander,dawdle,walk,fastwalk,run,sprint`
 * [game/simulation/src/ai/activity/mod.rs](game/simulation/src/ai/activity/mod.rs) (1)
   * `// TODO failure/interrupt reason`
 * [game/simulation/src/ai/activity/movement.rs](game/simulation/src/ai/activity/movement.rs) (3)
   * `// TODO exertion depends on speed`
   * `// TODO wander *activity* exertion should be 0, but added to the exertion of walking at X speed`
   * `// TODO remove WANDER_SPEED constant when this is done`
 * [game/simulation/src/ai/activity/world.rs](game/simulation/src/ai/activity/world.rs) (5)
   * `// TODO get block type we're about to break, and equip the best tool for it`
   * `// TODO get current held tool to determine how fast the block can be broken`
   * `// TODO breaking blocks with your hand hurts!`
   * `// TODO define proper scale/enum/consts for block and tool durability`
   * `// TODO exertion depends on the tool and block`
 * [game/simulation/src/ai/dse/items.rs](game/simulation/src/ai/dse/items.rs) (1)
   * `// TODO "I can/want to move" consideration`
 * [game/simulation/src/ai/dse/world.rs](game/simulation/src/ai/dse/world.rs) (2)
   * `// TODO calculate path and use length, cache path which can be reused by movement system`
   * `// TODO has the right tool/is the right tool nearby/close enough in society storage`
 * [game/simulation/src/ai/input.rs](game/simulation/src/ai/input.rs) (5)
   * `// TODO HasInInventoryGraded - returns number,quality of matches`
   * `// TODO old results are a subset of new results, should reuse`
   * `// TODO arena allocated vec return value`
   * `// TODO clearly needs some spatial partitioning here`
   * `// TODO lowercase BlockType`
 * [game/simulation/src/ai/mod.rs](game/simulation/src/ai/mod.rs) (1)
   * `/// TODO ideally this would use ai::Context<'a> to represent the AI tick lifetime: https://github.com/rust-lang/rust/issues/44265`
 * [game/simulation/src/ai/system.rs](game/simulation/src/ai/system.rs) (4)
   * `// TODO only run occasionally - FIXME TERRIBLE HACK`
   * `// TODO use arena/bump allocator and share instance between entities`
   * `// TODO provide READ ONLY DSEs to ai intelligence`
   * `// TODO use dynstack to avoid so many small temporary allocations?`
 * [game/simulation/src/backend.rs](game/simulation/src/backend.rs) (1)
   * `type Error: Debug; // TODO Error!!`
 * [game/simulation/src/dev.rs](game/simulation/src/dev.rs) (1)
   * `// TODO always make sure that putting an item into a contents removes its transform? only do this via a system`
 * [game/simulation/src/entity_builder.rs](game/simulation/src/entity_builder.rs) (1)
   * `// TODO add must_use to all builder patterns`
 * [game/simulation/src/input/blackboard.rs](game/simulation/src/input/blackboard.rs) (1)
   * `// TODO use ui allocation arena here too`
 * [game/simulation/src/input/command.rs](game/simulation/src/input/command.rs) (1)
   * `// TODO just use a dyn Job instead of redefining jobs as an identical enum?`
 * [game/simulation/src/input/system.rs](game/simulation/src/input/system.rs) (4)
   * `// TODO spatial query rather than checking every entity ever`
   * `// TODO multiple clicks in the same place should iterate through all entities in selection range`
   * `// TODO select multiple entities`
   * `// TODO can no longer select a single block, its always 2x2`
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
 * [game/simulation/src/path/mod.rs](game/simulation/src/path/mod.rs) (1)
   * `// TODO remove WANDER_SPEED`
 * [game/simulation/src/path/system.rs](game/simulation/src/path/system.rs) (3)
   * `/// TODO should be an enum and represent interruption too, i.e. path was invalidated`
   * `warn!("failed to find path to target {:?}: {:?}", target, e); // TODO {} for error`
   * `// FIXME GROSS HACK`
 * [game/simulation/src/perf.rs](game/simulation/src/perf.rs) (1)
   * `// TODO detect if changed`
 * [game/simulation/src/queued_update.rs](game/simulation/src/queued_update.rs) (1)
   * `// TODO pool/reuse these boxes`
 * [game/simulation/src/render/renderer.rs](game/simulation/src/render/renderer.rs) (1)
   * `// TODO render translucent quad over selected blocks, showing which are visible/occluded. cache this mesh`
 * [game/simulation/src/simulation.rs](game/simulation/src/simulation.rs) (4)
   * `// TODO return Result instead of panic!, even though this only happens during game init`
   * `// TODO sort out systems so they all have an ecs_world reference and can keep state`
   * `// TODO limit time/count`
   * `// TODO per tick alloc/reuse buf`
 * [game/simulation/src/society/job/list.rs](game/simulation/src/society/job/list.rs) (1)
   * `// TODO reuse allocation`
 * [game/simulation/src/society/job/task.rs](game/simulation/src/society/job/task.rs) (3)
   * `// TODO HaulBlocks(block type, near position)`
   * `// TODO PlaceBlocks(block type, at position)`
   * `// TODO temporary box allocation is gross`
 * [game/simulation/src/steer/context.rs](game/simulation/src/steer/context.rs) (2)
   * `// TODO average with previous for less sudden movements`
   * `// TODO follow gradients and choose continuous value`
 * [game/simulation/src/steer/system.rs](game/simulation/src/steer/system.rs) (2)
   * `// TODO struclog event`
   * `// TODO populate danger interests from world/other entity collisions`
 * [game/world/src/block.rs](game/world/src/block.rs) (1)
   * `// TODO store sparse block data in the slab instead of inline in the block`
 * [game/world/src/chunk/chunk.rs](game/world/src/chunk/chunk.rs) (1)
   * `// TODO still does a lot of unnecessary initialization`
 * [game/world/src/chunk/double_sided_vec.rs](game/world/src/chunk/double_sided_vec.rs) (1)
   * `// TODO refactor to use a single vec allocation`
 * [game/world/src/chunk/slice.rs](game/world/src/chunk/slice.rs) (1)
   * `// TODO make not pub`
 * [game/world/src/chunk/terrain.rs](game/world/src/chunk/terrain.rs) (8)
   * `// TODO actually add get_{mut_}unchecked to slabs for performance`
   * `// TODO could skip next slice because it cant be walkable if this one was?`
   * `// TODO set_block trait to reuse in ChunkBuilder (#46)`
   * `// TODO shared cow instance for empty slab`
   * `// TODO reuse a buffer for each slab`
   * `// TODO discover internal area links`
   * `// TODO transmute lifetimes instead`
   * `// TODO 1 area at z=0`
 * [game/world/src/grid.rs](game/world/src/grid.rs) (1)
   * `// TODO are %s optimised to bitwise ops if a multiple of 2?`
 * [game/world/src/loader/mod.rs](game/world/src/loader/mod.rs) (7)
   * `// TODO cache full finalized chunks`
   * `// TODO sort out the lifetimes instead of cheating and using transmute`
   * `// TODO reuse/pool bufs, and initialize with proper expected size`
   * `// TODO is it worth attempting to filter out updates that have no effect during the loop, or keep filtering them during consumption instead`
   * `let mut area_edges = Vec::new(); // TODO reuse buf`
   * `let mut links = Vec::new(); // TODO reuse buf`
   * `let mut ports = Vec::new(); // TODO reuse buf`
 * [game/world/src/loader/terrain_source/mod.rs](game/world/src/loader/terrain_source/mod.rs) (1)
   * `fn all_chunks(&mut self) -> Vec<ChunkPosition>; // TODO gross`
 * [game/world/src/loader/update.rs](game/world/src/loader/update.rs) (1)
   * `// TODO include reason for terrain update? (god magic, explosion, tool, etc)`
 * [game/world/src/loader/worker_pool.rs](game/world/src/loader/worker_pool.rs) (2)
   * `// TODO if this thread panics, propagate to main game thread`
   * `// TODO detect this err condition?`
 * [game/world/src/mesh.rs](game/world/src/mesh.rs) (5)
   * `let mut vertices = Vec::<V>::new(); // TODO reuse/calculate needed capacity first`
   * `// TODO skip if slice knows it is empty`
   * `// TODO blocks filling in gaps should be tinted the colour of the block they're suggesting`
   * `// TODO consider rendering a blurred buffer of slices below`
   * `// TODO also rotate texture`
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
 * [game/world/src/navigation/discovery.rs](game/world/src/navigation/discovery.rs) (1)
   * `/// flood fill queue, pair of (pos, pos this was reached from) TODO share between slabs`
 * [game/world/src/navigation/path.rs](game/world/src/navigation/path.rs) (2)
   * `// TODO smallvecs`
   * `// TODO derive(Error) for NavigationError`
 * [game/world/src/occlusion.rs](game/world/src/occlusion.rs) (2)
   * `/// TODO bitset of Opacitys will be much smaller, 2 bits each`
   * `// TODO return a transmuted u16 when bitset is used, much cheaper to create and compare`
 * [game/world/src/viewer.rs](game/world/src/viewer.rs) (7)
   * `assert!(size > 0); // TODO Result`
   * `// TODO return Result from from_world`
   * `// TODO intelligently choose an initial view range`
   * `// TODO do mesh generation on a worker thread`
   * `// TODO slice-aware chunk mesh caching, moving around shouldn't regen meshes constantly`
   * `// TODO cache world slice_bounds()`
   * `// TODO which direction to stretch view range in? automatically determine or player input?`
 * [game/world/src/world.rs](game/world/src/world.rs) (6)
   * `// TODO optimize path with raytracing (#50)`
   * `// TODO only calculate path for each area as needed (#51)`
   * `// TODO reuse hashset allocation`
   * `// TODO benchmark filter_blocks_in_range, then optimize slab and slice lookups`
   * `// TODO filter_blocks_in_range should pass chunk+slab reference to predicate`
   * `// TODO build area graph in loader`
 * [game/world/src/world_ref.rs](game/world/src/world_ref.rs) (1)
   * `// TODO don't unwrap()`
 * [renderer/engine/src/render/sdl/backend.rs](renderer/engine/src/render/sdl/backend.rs) (1)
   * `// TODO cascade through other handlers`
 * [renderer/engine/src/render/sdl/camera.rs](renderer/engine/src/render/sdl/camera.rs) (2)
   * `// TODO zoom`
   * `// TODO cache`
 * [renderer/engine/src/render/sdl/gl/vertex.rs](renderer/engine/src/render/sdl/gl/vertex.rs) (1)
   * `// TODO smallvec`
 * [renderer/engine/src/render/sdl/render/entity.rs](renderer/engine/src/render/sdl/render/entity.rs) (2)
   * `// TODO use buffersubdata to reuse allocation if len <=`
   * `// TODO cursor interface in ScopedMap`
 * [renderer/engine/src/render/sdl/render/mod.rs](renderer/engine/src/render/sdl/render/mod.rs) (1)
   * `// TODO use glBufferSubData to reuse the allocation if <= len`
 * [renderer/engine/src/render/sdl/ui/windows/debug_renderer.rs](renderer/engine/src/render/sdl/ui/windows/debug_renderer.rs) (1)
   * `// TODO helpers in Bundle`
 * [renderer/main/src/main.rs](renderer/main/src/main.rs) (1)
   * `// TODO more granular - n for engine setup, n for sim setup, n for each frame?`
 * [renderer/main/src/presets/dev.rs](renderer/main/src/presets/dev.rs) (1)
   * `// TODO GamePreset::world() should return a Result`
 * [renderer/main/src/presets/mod.rs](renderer/main/src/presets/mod.rs) (1)
   * `panic!("failed to wait for world to load: {:?}", err); // TODO return result`
 * [shared/color/src/lib.rs](shared/color/src/lib.rs) (1)
   * `/// TODO will this work with big endian?`
 * [shared/common/Cargo.toml](shared/common/Cargo.toml) (1)
   * `# TODO feature for cgmath`
 * [shared/config/src/load.rs](shared/config/src/load.rs) (1)
   * `// TODO add a variant that returns a default instead of panicking`
 * [shared/unit/src/dim.rs](shared/unit/src/dim.rs) (1)
   * `// TODO helper for this-1`
 * [shared/unit/src/world/mod.rs](shared/unit/src/world/mod.rs) (1)
   * `// TODO overhaul all *Position and *Point to impl common traits, to reduce repeated code and From/Intos`
 * [shared/unit/src/world/slab_position.rs](shared/unit/src/world/slab_position.rs) (1)
   * `// TODO consider using same generic pattern as SliceIndex for all points and positions`
 * [shared/unit/src/world/world_point.rs](shared/unit/src/world/world_point.rs) (2)
   * `// TODO assert fields are not NaN in points`
   * `// TODO floor_then_ceil is terribly inefficient, try without the lazy eval`