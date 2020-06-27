# Backlog

An unorganized, unordered list of tasks to eventually get to. Tasks are deleted from here and moved into `active.md`.


## Entity movement
* explicit jumping instead of teleporting to target z position
* avoid walls/needless collisions with danger context
* height aware path finding
* need to be able to recover if they get stuck on the edge of a ledge
	* fixed most of the time but still possible to sometimes get fully lodged
* lazy path evaluation (area at a time)
* path optimisation (line of sight)
* wandering should choose a close location instead of random in the world
* path invalidation on world change
* walk speed enum scale (wander, dawdle, walk, sprint, etc)
* bug: area path finding seems to needlessly poke into other areas

## UI
* graph for fps/tps history
	* measure ticks per second in perf window
* better tile selection
	* live updating selection region
	* selection shows if selected blocks are occluded
	* depth selection

## Entity control
* society-level job list, populated by player actions
	* select block(s) to destroy
	* pick up/haul an item


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
* pooled allocations
* per-tick arena allocator
* spatial queries for entities
* path finding worker thread
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
* textures/sprites/animations
* improved terrain colour palette

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
