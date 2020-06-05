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

## UI
* cascading input handlers
* imgui gui
	* performance info (fps, avg. tick time, avg. render time)
	* details about selected entity
* entity selection and debug information
* tile selection and debug information
	* tile region?

## Entity control
* select entity and walk to block
* order something to be picked up/carried, a suitable entity will choose to do it

## World generation
* simple terrain generation
* features e.g. trees, hills

## Voxel world mechanics
* fluid blocks
* modification
	* entities digging/building
	* block damage e.g. from explosion
	* side effect of interacting with a block
* puddles/spills/splatters on the ground
	* picked up and spread by entities

## Performance
* allocation reuse
* pooled allocations
* per-tick arena allocator
* spatial queries for entities
* path finding worker thread
* periodic and staggered systems
* slice-aware chunk mesh caching
* influence map for density, sound

## Crate release
* gameloop
* voxel world
* iaus ai

## Rendering
* textures/sprites/animations
* improved terrain colour palette
* z slice boundary interpolation (instead of black void)
* fix camera zoom centering bug

## Building and testing
* separate config and preset for tests
* fuzzing
* stress test
* code coverage in CI

## Code quality
* track down unwraps/expects and replace with results
* less repetition in chunk/terrain/chunkbuilder/chunkbuilderapply/slicemut

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
