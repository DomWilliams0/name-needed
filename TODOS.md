# TODOs (312)
 * [game/ai/src/consideration.rs](game/ai/src/consideration.rs) (1)
   * `// TODO impl Display for considerations instead`
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
 * [game/procgen/src/biome.rs](game/procgen/src/biome.rs) (3)
   * `// TODO dont use a String here, return useful info`
   * `// TODO make poles more moist`
   * `// TODO elevation needs refining, and shouldn't be so smooth/uniform across the full range (0-1).`
 * [game/procgen/src/cache.rs](game/procgen/src/cache.rs) (1)
   * `// TODO cache global features too`
 * [game/procgen/src/climate.rs](game/procgen/src/climate.rs) (9)
   * `// TODO moisture and temperature carried by wind`
   * `// TODO wind movingbrings air to level out pressure`
   * `// TODO wind is not being affected by terrain at all`
   * `// TODO wind is getting stuck low down and not rising`
   * `// TODO reuse alloc`
   * `// TODO distribute across neighbours more smoothly, advection?`
   * `// TODO if too big (>0.01) we end up with little pockets of unchanging high pressure :(`
   * `// TODO cold high air falls?`
   * `// TODO height doesnt change, calculate this once in a separate grid`
 * [game/procgen/src/continent.rs](game/procgen/src/continent.rs) (6)
   * `// TODO agree api and stop making everything public`
   * `// TODO validate values with result type`
   * `// TODO reject if continent or land blob count is too low`
   * `let mut vertices = [(0.0, 0.0); CIRCLE_VERTICES]; // TODO could be uninitialized`
   * `// TODO intersecting polygons!!`
   * `// TODO reimplement or add back density if needed`
 * [game/procgen/src/params.rs](game/procgen/src/params.rs) (3)
   * `// TODO remove overhead of option and default to 0`
   * `// TODO return a result instead of panicking`
   * `// TODO clap AppSettings::AllArgsOverrideSelf`
 * [game/procgen/src/planet.rs](game/procgen/src/planet.rs) (5)
   * `// TODO actual error type`
   * `// TODO could have separate copy of planet params per thread if immutable`
   * `// TODO radius no longer makes sense`
   * `// TODO wrap chunks rather than ignoring those out of range`
   * `.filter_map(|(cx, cy)| RegionLocation::try_from_chunk(ChunkLocation(cx, cy))) // TODO`
 * [game/procgen/src/progress.rs](game/procgen/src/progress.rs) (1)
   * `// TODO every thread returns the same pathbuf`
 * [game/procgen/src/rasterize.rs](game/procgen/src/rasterize.rs) (1)
   * `// TODO custom block types for procgen that are translated to game blocks`
 * [game/procgen/src/region/feature.rs](game/procgen/src/region/feature.rs) (4)
   * `// TODO make this struct a dst and store trait object inline without extra indirection`
   * `// TODO ensure these are optimised out`
   * `// TODO give each feature a guid instead`
   * `// TODO faster and non-random hash`
 * [game/procgen/src/region/features/forest.rs](game/procgen/src/region/features/forest.rs) (10)
   * `// TODO remove magic value, use real max tree height`
   * `// TODO tree roots`
   * `// TODO attempt to place tree model at location in this slab`
   * `// TODO actual validation`
   * `// TODO consider rtree params`
   * `// TODO const generic size param`
   * `// TODO this does SO many temporary allocations`
   * `const SIZE: usize = CHUNKS_PER_REGION_SIDE; // TODO add const generic (and use the unspecialised PlanetPoint)`
   * `// TODO replace this rtree with a new bulk loaded one?`
   * `// TODO PR to move nodes out of the tree instead of copy`
 * [game/procgen/src/region/region.rs](game/procgen/src/region/region.rs) (7)
   * `// TODO when const generics can be used in evaluations, remove stupid SIZE_2 type param (SIZE * SIZE)`
   * `// TODO rename me`
   * `// TODO will need to filter on feature type when there are multiple`
   * `// TODO null params for benchmark`
   * `// TODO depends on many local parameters e.g. biome, humidity`
   * `// TODO could do this multiple slices at a time`
   * `// TODO calculate these better, and store them in data`
 * [game/procgen/src/region/regions.rs](game/procgen/src/region/regions.rs) (2)
   * `/// TODO use a global vec/channel instead (in tests only)`
   * `// TODO move directly with pointer magic instead`
 * [game/procgen/src/region/row_scanning.rs](game/procgen/src/region/row_scanning.rs) (1)
   * `// TODO ensure no bounds checking here`
 * [game/procgen/src/region/subfeature.rs](game/procgen/src/region/subfeature.rs) (8)
   * `// TODO pass in a "mask" of xyz ranges that can optionally be used to trim trying to place blocks in a neighbour`
   * `// TODO inline dyn subfeature or use pooled allocation`
   * `// TODO use dynstack here`
   * `// TODO reuse borrowed vec allocation`
   * `/// TODO handle case where block is multiple slabs over from root slab`
   * `// TODO if continuations is None, set a flag to ignore boundary leaks`
   * `// TODO neighbour slab should wrap around the planet`
   * `// TODO beware that subfeatures dont live for long so the pointer is likely to be reused`
 * [game/procgen/src/region/subfeatures/tree.rs](game/procgen/src/region/subfeatures/tree.rs) (2)
   * `// TODO actual tree shape`
   * `// TODO tree configuration based on its planet location - branch count, leaf spread, etc`
 * [game/procgen/src/render.rs](game/procgen/src/render.rs) (2)
   * `// TODO per land layer?`
   * `// TODO fix log_scope crashing with async`
 * [game/simulation/src/activity/activities/eat_held_item.rs](game/simulation/src/activity/activities/eat_held_item.rs) (1)
   * `// TODO sanity check equipper is this entity`
 * [game/simulation/src/activity/activities/follow.rs](game/simulation/src/activity/activities/follow.rs) (1)
   * `// TODO will probably need porting to a follow subactivity`
 * [game/simulation/src/activity/activities/go_break_block.rs](game/simulation/src/activity/activities/go_break_block.rs) (4)
   * `// TODO block breaking/world interacting should be done in a system`
   * `// TODO get current held tool to determine how fast the block can be broken`
   * `// TODO breaking blocks with your hand hurts!`
   * `// TODO define proper scale/enum/consts for block and tool durability`
 * [game/simulation/src/activity/activities/go_haul.rs](game/simulation/src/activity/activities/go_haul.rs) (10)
   * `// TODO support for hauling multiple things at once to the same loc, if the necessary amount of hands are available`
   * `// TODO support hauling multiple things to multiple locations`
   * `// TODO haul target should hold pos+item radius, assigned once on creation`
   * `// TODO events for items entering/exiting containers`
   * `// TODO arrival radius depends on the size of the item`
   * `// TODO could the item ever move while we're going to it? only by gravity?`
   * `// TODO this should be in the/a subactivity`
   * `// TODO don't always drop item in centre`
   * `// TODO explicit access side for container, e.g. front of chest`
   * `// TODO format the other entity better e.g. get item name. or do this in the ui layer?`
 * [game/simulation/src/activity/activities/go_pickup.rs](game/simulation/src/activity/activities/go_pickup.rs) (2)
   * `// TODO detect other destructive events e.g. entity removal`
   * `// TODO other destructive events happening to the item`
 * [game/simulation/src/activity/activities/go_to.rs](game/simulation/src/activity/activities/go_to.rs) (1)
   * `// TODO reason specification should be type level and used everywhere. ties into localization`
 * [game/simulation/src/activity/activities/mod.rs](game/simulation/src/activity/activities/mod.rs) (1)
   * `// TODO helpers for GoToThen, EquipItemThen, etc`
 * [game/simulation/src/activity/activities/wander.rs](game/simulation/src/activity/activities/wander.rs) (1)
   * `// TODO add additional DSEs while wandering and loitering e.g. whistling, waving, humming`
 * [game/simulation/src/activity/mod.rs](game/simulation/src/activity/mod.rs) (1)
   * `// TODO move subactivity errors somewhere else`
 * [game/simulation/src/activity/subactivities/go_to.rs](game/simulation/src/activity/subactivities/go_to.rs) (3)
   * `// TODO helper on ctx to get component`
   * `// TODO better exertion calculation for movement speed`
   * `// TODO use movement speed enum for display e.g. wandering to, running to`
 * [game/simulation/src/activity/subactivities/haul.rs](game/simulation/src/activity/subactivities/haul.rs) (4)
   * `// TODO apply slowness effect to holder`
   * `// TODO subscribe to container being destroyed`
   * `// TODO remove slowness effect if any`
   * `// TODO depends on the weight of the item(s)`
 * [game/simulation/src/activity/subactivities/item_eat.rs](game/simulation/src/activity/subactivities/item_eat.rs) (1)
   * `// TODO varying exertion per food`
 * [game/simulation/src/activity/subactivities/item_equip.rs](game/simulation/src/activity/subactivities/item_equip.rs) (1)
   * `// TODO inventory operations should not be immediate`
 * [game/simulation/src/activity/subactivities/pickup.rs](game/simulation/src/activity/subactivities/pickup.rs) (1)
   * `// TODO exertion of picking up item depends on item weight`
 * [game/simulation/src/activity/system.rs](game/simulation/src/activity/system.rs) (2)
   * `let mut subscriptions = Vec::new(); // TODO reuse allocation in system`
   * `// TODO consider allowing consideration of a new activity while doing one, then swapping immediately with no pause`
 * [game/simulation/src/ai/action.rs](game/simulation/src/ai/action.rs) (1)
   * `// TODO speed should be specified as an enum for all go??? actions`
 * [game/simulation/src/ai/consideration/items.rs](game/simulation/src/ai/consideration/items.rs) (1)
   * `// TODO also count currently occupied hands as "available", could drop current item to haul this`
 * [game/simulation/src/ai/dse/food.rs](game/simulation/src/ai/dse/food.rs) (1)
   * `// TODO "I can/want to move" consideration`
 * [game/simulation/src/ai/dse/world.rs](game/simulation/src/ai/dse/world.rs) (2)
   * `// TODO calculate path and use length, cache path which can be reused by movement system`
   * `// TODO has the right tool/is the right tool nearby/close enough in society storage`
 * [game/simulation/src/ai/input.rs](game/simulation/src/ai/input.rs) (4)
   * `// TODO HasInInventoryGraded - returns number,quality of matches`
   * `// TODO old results are a subset of new results, should reuse`
   * `// TODO use accessible position?`
   * `// TODO lowercase BlockType`
 * [game/simulation/src/ai/mod.rs](game/simulation/src/ai/mod.rs) (1)
   * `/// TODO ideally this would use ai::Context<'a> to represent the AI tick lifetime: https://github.com/rust-lang/rust/issues/44265`
 * [game/simulation/src/ai/system.rs](game/simulation/src/ai/system.rs) (5)
   * `// TODO only run occasionally - FIXME TERRIBLE HACK`
   * `// TODO use arena/bump allocator and share instance between entities`
   * `// TODO provide READ ONLY DSEs to ai intelligence`
   * `// TODO use dynstack to avoid so many small temporary allocations?`
   * `// TODO fix (eventually) false assumption that all stream DSEs come from a society`
 * [game/simulation/src/definitions/builder.rs](game/simulation/src/definitions/builder.rs) (1)
   * `// TODO avoid box by resolving here and storing result`
 * [game/simulation/src/definitions/loader/load.rs](game/simulation/src/definitions/loader/load.rs) (1)
   * `// TODO remove abstract definitions`
 * [game/simulation/src/definitions/loader/mod.rs](game/simulation/src/definitions/loader/mod.rs) (1)
   * `// TODO consider using `nested` vecs as an optimization`
 * [game/simulation/src/definitions/mod.rs](game/simulation/src/definitions/mod.rs) (1)
   * `// TODO include which key caused the problem`
 * [game/simulation/src/ecs/component.rs](game/simulation/src/ecs/component.rs) (1)
   * `// TODO should be a Box<dyn Error>`
 * [game/simulation/src/ecs/mod.rs](game/simulation/src/ecs/mod.rs) (1)
   * `// TODO perfect hashing`
 * [game/simulation/src/event/queue.rs](game/simulation/src/event/queue.rs) (2)
   * `// TODO event queue generic over event type`
   * `// TODO track by game tick instead of just number of ops`
 * [game/simulation/src/event/timer.rs](game/simulation/src/event/timer.rs) (2)
   * `// TODO sort by elapsed() bool instead`
   * `// TODO might be better to just insert sorted`
 * [game/simulation/src/input/blackboard.rs](game/simulation/src/input/blackboard.rs) (1)
   * `/// TODO this can probably just hold the world and have some helper functions`
 * [game/simulation/src/input/system.rs](game/simulation/src/input/system.rs) (3)
   * `// TODO spatial query rather than checking every entity ever`
   * `// TODO multiple clicks in the same place should iterate through all entities in selection range`
   * `// TODO select multiple entities`
 * [game/simulation/src/item/component.rs](game/simulation/src/item/component.rs) (8)
   * `// TODO smol string`
   * `// TODO proper nutritional value`
   * `// TODO food debris - the last X fuel/proportion is inedible and has to be disposed of`
   * `// TODO depending on their mood/personality this will be tossed to the ground or taken to a proper place`
   * `// TODO add aerodynamic-ness field`
   * `// TODO drinkable`
   * `// TODO splatterable (after throw, if walked on)`
   * `// TODO weapon (damage to target per hit, damage to own condition per hit, attack speed, cooldown)`
 * [game/simulation/src/item/filter.rs](game/simulation/src/item/filter.rs) (1)
   * `// TODO filters on other fields e.g. mass, size, condition, etc`
 * [game/simulation/src/item/haul.rs](game/simulation/src/item/haul.rs) (5)
   * `// TODO multiple people sharing a haul`
   * `// TODO cart/wagon/vehicle`
   * `// TODO carry vs drag`
   * `// TODO this is awful and should be generalised to a part of the physics system e.g. relative positioned entity`
   * `// TODO position at the correct arm(s) location`
 * [game/simulation/src/item/inventory/component.rs](game/simulation/src/item/inventory/component.rs) (7)
   * `// TODO owner should be handled in the same way as communal i.e. mirror state elsewhere`
   * `/// TODO it's possible some hands have been freed up while returning false anyway`
   * `// TODO loop along all held items rather than only checking the first`
   * `// TODO configurable drop equipped items to make space instead of failing`
   * `// TODO possibly add search cache keyed by entity, if there are many repeated searches for the same entity`
   * `// TODO impl this when a scenario is found to hit this code path :^)`
   * `// TODO this is the same as is used by PhysicalComponent`
 * [game/simulation/src/item/inventory/container.rs](game/simulation/src/item/inventory/container.rs) (1)
   * `// TODO sort by some item type identifier so common items are grouped together`
 * [game/simulation/src/item/inventory/equip.rs](game/simulation/src/item/inventory/equip.rs) (1)
   * `// TODO equip slots will require a lot of integration with the body tree, so dont flesh out properly`
 * [game/simulation/src/movement.rs](game/simulation/src/movement.rs) (2)
   * `// TODO actually use body health to determine how much movement is allowed`
   * `// TODO scale max speed based on applied effects?`
 * [game/simulation/src/needs/food.rs](game/simulation/src/needs/food.rs) (9)
   * `// TODO newtype for Fuel`
   * `// fuel used per tick TODO depends on time rate`
   * `// TODO species metabolism`
   * `// TODO generic needs component with hunger/thirst/toilet/social etc`
   * `ReadStorage<'a, ActivityComponent>, // for current exertion TODO moving average`
   * `// TODO individual metabolism rate`
   * `// TODO compensate multipliers`
   * `let fuel_to_consume = BASE_EAT_RATE; // TODO individual rate`
   * `// TODO while eating/for a short time afterwards, add a hunger multiplier e.g. 0.2`
 * [game/simulation/src/path/debug.rs](game/simulation/src/path/debug.rs) (1)
   * `// TODO only render the top area in each slice`
 * [game/simulation/src/path/follow.rs](game/simulation/src/path/follow.rs) (1)
   * `// TODO dont manually set the exact follow speed - choose a preset e.g. wander,dawdle,walk,fastwalk,run,sprint`
 * [game/simulation/src/path/mod.rs](game/simulation/src/path/mod.rs) (1)
   * `// TODO remove WANDER_SPEED`
 * [game/simulation/src/perf.rs](game/simulation/src/perf.rs) (1)
   * `// TODO detect if changed`
 * [game/simulation/src/physics/bounds.rs](game/simulation/src/physics/bounds.rs) (1)
   * `// TODO vertical height too`
 * [game/simulation/src/physics/system.rs](game/simulation/src/physics/system.rs) (2)
   * `// TODO apply fall damage if applicable`
   * `// TODO lerp towards new rotation`
 * [game/simulation/src/queued_update.rs](game/simulation/src/queued_update.rs) (2)
   * `// TODO use dynstack for updates to avoid a separate box per entry`
   * `// TODO pool/reuse these boxes`
 * [game/simulation/src/render/renderer.rs](game/simulation/src/render/renderer.rs) (1)
   * `// TODO render translucent quad over selected blocks, showing which are visible/occluded. cache this mesh`
 * [game/simulation/src/render/shape.rs](game/simulation/src/render/shape.rs) (1)
   * `// TODO physical shape wastes so much space`
 * [game/simulation/src/senses/sense.rs](game/simulation/src/senses/sense.rs) (1)
   * `// TODO this is really expensive`
 * [game/simulation/src/senses/system.rs](game/simulation/src/senses/system.rs) (5)
   * `/// TODO maybe the ecs bitmask can be reused here instead of a huge alloc per entity`
   * `// TODO system is expensive, dont run every tick`
   * `// TODO consider using expiry times rather than decrementing a decay counter`
   * `// TODO specialize query e.g. only detect those with a given component combo e.g. Transform + Render (+ Visible/!Invisible?)`
   * `.filter(|(entity, _, _)| *entity != e) // TODO self is probably the first in the list`
 * [game/simulation/src/simulation.rs](game/simulation/src/simulation.rs) (5)
   * `/// TODO if order matters, use an IndexSet instead`
   * `// TODO sort out systems so they all have an ecs_world reference and can keep state`
   * `// TODO limit time/count`
   * `let discovered = empty(); // TODO include slabs discovered by members of player's society`
   * `r.register(FeatureBoundaryDebugRenderer::default(), true)?; // TODO TEMPORARY TRUE`
 * [game/simulation/src/society/job/job.rs](game/simulation/src/society/job/job.rs) (1)
   * `// TODO return a dyn error in result`
 * [game/simulation/src/society/job/jobs/haul.rs](game/simulation/src/society/job/jobs/haul.rs) (1)
   * `// TODO differentiate hauling types, reasons and container choices e.g. to any container (choose in ai), to nearby a build project, to specific container`
 * [game/simulation/src/society/job/list.rs](game/simulation/src/society/job/list.rs) (3)
   * `// TODO use dynstack instead of boxes for society jobs`
   * `// TODO reuse allocation`
   * `// TODO dont recalculate all unreserved tasks every tick for every entity`
 * [game/simulation/src/society/job/task.rs](game/simulation/src/society/job/task.rs) (2)
   * `// TODO PlaceBlocks(block type, at position)`
   * `// TODO temporary box allocation is gross, use dynstack for dses`
 * [game/simulation/src/society/registry.rs](game/simulation/src/society/registry.rs) (1)
   * `// TODO keep society registry sorted by handle for quick lookup`
 * [game/simulation/src/spatial.rs](game/simulation/src/spatial.rs) (1)
   * `// TODO reimplement with octree`
 * [game/simulation/src/steer/context.rs](game/simulation/src/steer/context.rs) (2)
   * `// TODO average with previous for less sudden movements`
   * `// TODO follow gradients and choose continuous value`
 * [game/simulation/src/steer/system.rs](game/simulation/src/steer/system.rs) (1)
   * `// TODO cache allocation in system`
 * [game/simulation/src/transform.rs](game/simulation/src/transform.rs) (1)
   * `// TODO use newtype units for ingame non-SI units`
 * [game/world/src/block.rs](game/world/src/block.rs) (5)
   * `// TODO store sparse block data in the slab instead of inline in the block`
   * `// TODO define block types in data instead of code`
   * `// TODO this should return an Option if area is uninitialized`
   * `// TODO define these in data`
   * `/// TODO very temporary "walkability" for block types`
 * [game/world/src/chunk/double_sided_vec.rs](game/world/src/chunk/double_sided_vec.rs) (1)
   * `// TODO refactor to use a single vec allocation`
 * [game/world/src/chunk/slab.rs](game/world/src/chunk/slab.rs) (5)
   * `// TODO detect when slab is all air and avoid expensive processing`
   * `// TODO if exclusive we're in deep water with CoW`
   * `// TODO discover internal area links`
   * `// TODO consider resizing/populating changes_out initially with empty events for performance`
   * `// TODO reserve space in changes_out first`
 * [game/world/src/chunk/slice.rs](game/world/src/chunk/slice.rs) (2)
   * `// TODO consider generalising Slice{,Mut,Owned} to hold other types than just Block e.g. opacity`
   * `// TODO make not pub`
 * [game/world/src/chunk/terrain.rs](game/world/src/chunk/terrain.rs) (6)
   * `// TODO actually add get_{mut_}unchecked to slabs for performance`
   * `// TODO could skip next slice because it cant be walkable if this one was?`
   * `// TODO this is sometimes a false positive, triggering unnecessary copies`
   * `// TODO use an enum for the slice range rather than Options`
   * `// TODO set_block trait to reuse in ChunkBuilder (#46)`
   * `// TODO 1 area at z=0`
 * [game/world/src/loader/finalizer.rs](game/world/src/loader/finalizer.rs) (9)
   * `// TODO mark chunk as "not ready" so its mesh is only rendered when it is finalized`
   * `let mut area_edges = Vec::new(); // TODO reuse buf`
   * `let mut links = Vec::new(); // TODO reuse buf`
   * `let mut ports = Vec::new(); // TODO reuse buf`
   * `// TODO is it worth combining occlusion+nav by doing cross chunk iteration only once?`
   * `// TODO only propagate across chunk boundaries if the changes were near to a boundary?`
   * `// TODO reuse/pool bufs, and initialize with proper expected size`
   * `// TODO is it worth attempting to filter out updates that have no effect during the loop, or keep filtering them during consumption instead`
   * `// TODO prevent mesh being rendered if there are queued occlusion changes?`
 * [game/world/src/loader/loading.rs](game/world/src/loader/loading.rs) (4)
   * `// TODO add more efficient version that takes chunk+multiple slabs`
   * `// TODO shared instance of CoW for empty slab`
   * `// TODO reuse vec allocs`
   * `// TODO reuse buf`
 * [game/world/src/loader/terrain_source/generate.rs](game/world/src/loader/terrain_source/generate.rs) (1)
   * `// TODO handle wrapping of slabs around planet boundaries`
 * [game/world/src/loader/update.rs](game/world/src/loader/update.rs) (1)
   * `// TODO include reason for terrain update? (god magic, explosion, tool, etc)`
 * [game/world/src/loader/worker_pool.rs](game/world/src/loader/worker_pool.rs) (1)
   * `// TODO detect this as an error condition?`
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
 * [game/world/src/navigation/discovery.rs](game/world/src/navigation/discovery.rs) (3)
   * `/// flood fill queue, pair of (pos, pos this was reached from) TODO share between slabs`
   * `// indices are certainly valid - TODO unchecked unwrap`
   * `// TODO use unchecked unwrap here`
 * [game/world/src/navigation/path.rs](game/world/src/navigation/path.rs) (1)
   * `// TODO smallvecs`
 * [game/world/src/occlusion.rs](game/world/src/occlusion.rs) (3)
   * `/// TODO bitset of Opacities will be much smaller, 2 bits each`
   * `// TODO this is different to the actual Default!`
   * `// TODO return a transmuted u16 when bitset is used, much cheaper to create and compare`
 * [game/world/src/viewer.rs](game/world/src/viewer.rs) (8)
   * `assert!(size > 0); // TODO Result`
   * `chunk_range: (initial_chunk, initial_chunk), // TODO is this ok?`
   * `// TODO do mesh generation on a worker thread? or just do this bit in a parallel iter`
   * `// TODO slice-aware chunk mesh caching, moving around shouldn't regen meshes constantly`
   * `// TODO limit to loaded slab bounds if camera is not discovering`
   * `// TODO only request slabs that are newly visible`
   * `// TODO which direction to stretch view range in? automatically determine or player input?`
   * `// TODO submit only the new chunks in range`
 * [game/world/src/world.rs](game/world/src/world.rs) (6)
   * `// TODO optimize path with raytracing (#50)`
   * `// TODO only calculate path for each area as needed (#51)`
   * `// TODO benchmark filter_blocks_in_range, then optimize slab and slice lookups`
   * `// TODO filter_blocks_in_range should pass chunk+slab reference to predicate`
   * `// TODO build area graph in loader`
   * `// TODO make stresser use generated terrain again`
 * [game/world/src/world_ref.rs](game/world/src/world_ref.rs) (1)
   * `// TODO don't unwrap()`
 * [renderer/engine/src/render/sdl/camera.rs](renderer/engine/src/render/sdl/camera.rs) (2)
   * `// TODO zoom`
   * `// TODO cache`
 * [renderer/engine/src/render/sdl/gl/vertex.rs](renderer/engine/src/render/sdl/gl/vertex.rs) (1)
   * `// TODO smallvec`
 * [renderer/engine/src/render/sdl/render/entity.rs](renderer/engine/src/render/sdl/render/entity.rs) (2)
   * `// TODO use buffersubdata to reuse allocation if len <=`
   * `// TODO cursor interface in ScopedMap`
 * [renderer/engine/src/render/sdl/render/mod.rs](renderer/engine/src/render/sdl/render/mod.rs) (3)
   * `// TODO render head at head height, not the ground`
   * `// TODO add proper support for quads and other debug shapes`
   * `// TODO use glBufferSubData to reuse the allocation if <= len`
 * [renderer/engine/src/render/sdl/ui/windows/debug_renderer.rs](renderer/engine/src/render/sdl/ui/windows/debug_renderer.rs) (1)
   * `// TODO helpers in Bundle`
 * [renderer/engine/src/render/sdl/ui/windows/selection.rs](renderer/engine/src/render/sdl/ui/windows/selection.rs) (1)
   * `// TODO list components on item that are relevant (i.e. not transform etc)`
 * [renderer/main/src/main.rs](renderer/main/src/main.rs) (2)
   * `// TODO more granular - n for engine setup, n for sim setup, n for each frame?`
   * `// TODO use error chaining when stable (https://github.com/rust-lang/rust/issues/58520)`
 * [renderer/main/src/presets/mod.rs](renderer/main/src/presets/mod.rs) (1)
   * `// TODO middle of requested chunk instead of corner`
 * [resources/definitions/living/dog.ron](resources/definitions/living/dog.ron) (1)
   * `// TODO dog mouth inventory`
 * [shared/color/src/lib.rs](shared/color/src/lib.rs) (1)
   * `/// TODO will this work with big endian?`
 * [shared/common/Cargo.toml](shared/common/Cargo.toml) (1)
   * `# TODO feature for cgmath`
 * [shared/common/src/newtype.rs](shared/common/src/newtype.rs) (1)
   * `// TODO support f64 too`
 * [shared/config/src/load.rs](shared/config/src/load.rs) (1)
   * `// TODO add a variant that returns a default instead of panicking`
 * [shared/grid/src/dynamic.rs](shared/grid/src/dynamic.rs) (3)
   * `// TODO use same CoordType for DynamicGrid`
   * `// TODO profile and improve coord wrapping`
   * `// TODO return <C: GridCoord>`
 * [shared/grid/src/grid_impl.rs](shared/grid/src/grid_impl.rs) (1)
   * `// TODO can still panic`
 * [shared/logging/src/init.rs](shared/logging/src/init.rs) (1)
   * `// TODO configure to write to file as text`
 * [shared/metrics/src/lib.rs](shared/metrics/src/lib.rs) (1)
   * `// TODO return error to caller`
 * [shared/unit/src/dim.rs](shared/unit/src/dim.rs) (2)
   * `// TODO unsafe unchecked casts with no panicking code`
   * `// TODO helper for this-1`
 * [shared/unit/src/lib.rs](shared/unit/src/lib.rs) (1)
   * `// TODO pub mod hunger;`
 * [shared/unit/src/world/block_position.rs](shared/unit/src/world/block_position.rs) (1)
   * `// TODO return Option/implement TryFrom for all coord types instead of asserts`
 * [shared/unit/src/world/mod.rs](shared/unit/src/world/mod.rs) (1)
   * `// TODO overhaul all *Position and *Point to impl common traits, to reduce repeated code and From/Intos`
 * [shared/unit/src/world/slab_position.rs](shared/unit/src/world/slab_position.rs) (2)
   * `// TODO consider using same generic pattern as SliceIndex for all points and positions`
   * `// TODO return option instead of asserting`
 * [shared/unit/src/world/slice_block.rs](shared/unit/src/world/slice_block.rs) (1)
   * `// TODO try_new constructor that returns option, with unchecked version. make fields non pub`
 * [shared/unit/src/world/slice_index.rs](shared/unit/src/world/slice_index.rs) (1)
   * `// TODO return option and have unchecked version`
 * [shared/unit/src/world/world_point.rs](shared/unit/src/world/world_point.rs) (1)
   * `// TODO assert fields are not NaN in points`
