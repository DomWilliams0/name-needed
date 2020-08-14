# Backlog

An unorganized, unordered list of tasks to eventually get to. Tasks are deleted from here and moved into `active.md`.


## Entity movement
* navigation graph edges for larger step sizes
	* can fall e.g. 2 or 3m
	* cats can jump 2m
	* humans can jump 1m
* lazy path evaluation (area at a time)
* path optimisation (line of sight)
* wandering should choose a close location instead of random in the world
	* new SearchGoal to cut short path to N blocks
* path invalidation on world change
* walk speed enum scale (wander, dawdle, walk, sprint, etc)
* bug: area path finding seems to needlessly poke into other areas

## UI/input
* graph for fps/tps history
	* measure ticks per second in perf window
* better tile selection
	* live updating selection region
	* selection shows if selected blocks are occluded
	* depth selection
* better entity selection
	* click and drag to select multiple
	* multiple clicks in the same place to iterate through overlapping entities
* persist ui state over restarts (open treenodes, toggled debug renderers etc)
* interactive terminal to replace/extend dev ui
	* custom log! handler to show warnings and errors
	* in-game OR pop out OR out of process [ncurses] terminal console that persists over restarts
* fast forward toggle
	* update gameloop to allow changing TPS
	* limit gameloop interpolation to 1.0: can be greater if ticks take too long
* add inventory to UI


## Entity behaviour
* more society level jobs
	* pick up/haul an item
	* place blocks, destroying what's currently there (DAG for dependencies)
	* place walls (hollow rectangle)
		* specify wall thickness and height
* ai incentive to choose the same action as last tick
* (sub)activities take an amount of ticks to complete

## World generation
* biomes
* features e.g. trees, hills
	* trees are entities, not blocks
	* accurate-ish rivers, caves
	* magma very low down, or it just gets too hot

## Voxel world mechanics
* fluid blocks
	* infinite sources/flows at the world edges
* modification
	* entities digging/building
	* block damage e.g. from explosion
	* side effect of interacting with a block
* puddles/spills/splatters on the ground
	* picked up and spread by entities

## Optimizations
### Performance
* allocation reuse
	* cap cached alloc size so memory usage doesnt only go upwards
	* raii struct i.e. `fn start(&mut self) -> TempVec;` `TempVec::drop() {self.backing.clear()}`
* pooled allocations
* per-tick arena allocator
* spatial queries for entities
* path finding worker thread
	* short term path cache for src -> dst: e.g. ai system requests path for calculating cost, then movement reuses that path for following
* periodic and staggered systems
	* preserve determinism if possible
* slice-aware chunk mesh caching
* influence map for density, sound
* remove unneeded Debug impls/cfg_attr them to speed up compilation
* mesh generation on worker thread
* replace all hashmaps with faster non crypto hashes
* terrain finalizer should not propogate to neighbours if single block changes arent on boundary
* unchecked_unwrap

### Memory usage
* CoW terrain slabs
* store sparse block metadata in the containing slab instead of in each block

## Crate release
* voxel world
* world update batcher
* iaus ai
* config with watcher

## Rendering
* debug renderer to show chunk boundaries
* textures/sprites/animations
* improved terrain colour palette
* very simple oval shadows beneath entities to show height

## Building and testing
* separate config and preset for tests
* fuzzing
* stress test
* code coverage in CI

## Code quality
* track down unwraps/expects and replace with results
* less repetition in chunk/terrain/chunkbuilder/chunkbuilderapply/slicemut
* on panic in any thread, process should exit with backtrace
* define rates, scales, units etc in unit crate e.g. metabolism, durabilities
* error context chaining would be useful

## Engine
* explicit namespacing for entity definitions e.g. "core:food_apple"
* structured logging with slog
	* log to a file in /tmp too
	* consider moving log sink/drain to another thread with async io
	* `Entity`s can be formatted automatically by the drain, huzzah

## Entity diversity
* animal species
	* dogs, cats, birds, deer
* individual stats for needs

## Simulation depth
* entity interaction
	* taming, hunting, social
* entity needs
	* drink
	* toilet
	* social
	* sleep

### Physical wellbeing
* distinct body parts
* wellbeing of individual parts affects stats
* gradual healing and tending
* track injury causes e.g. arrow in leg, fired by X at time Y with weapon Z
* blood flow that can be blocked off
* inventory system should be on top of physical body, which defines number and availability of slots
	* e.g. parts are marked as mounting points, mounting affects the availability of other mounts (backpack (shoulder) vs briefcase (hand) vs ... skin pocket?? (wherever it is))
