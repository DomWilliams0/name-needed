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
	* wander should not take them up into stupid places like atop chests
		* consider different edge costs for climbing ontop of stupid things, not considered for wandering/walking
* path invalidation on world change
* walk speed enum scale (wander, dawdle, walk, sprint, etc)
* bug: area path finding seems to needlessly poke into other areas
	* add higher weight difference for inter-area edges
	* an inappropriate block in an area port is chosen
	* very indirect paths within areas too, edge costs need adjusting
* tweak arrival threshold for path waypoints, it's a bit jerky
* bug: recalculating a path while already following one causes hiccup as the path starts 1 block behind them
* apply gravity to item entities too, for when block beneath them is mined

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
* resolve an entity to its displayable name including state in the UI layer only
	* e.g. get item name ("Apple (rotten)"), person name ("Steve (unconscious)")

## Entity behaviour
* more society level jobs
	* place blocks, destroying what's currently there (DAG for dependencies)
	* place walls (hollow rectangle)
		* specify wall thickness and height
* ai incentive to choose the same action as last tick
* (sub)activities take an amount of ticks to complete
* be able to block subactivities for a number of ticks, show progress bar up until end tick
* food/drink vessels and wastage
* consider defining AI in definitions with a collection of "features" rather than raw DSEs/behaviours
* if only have 2 hands but have a very high priority to carry something, they can jam it somewhere (armpit or somewhere) and carry more capacity at a slow speed/high risk of falling/tripping/dropping
* if new activity fails immediately on the first tick, they stand there stupidly doing nothing until the next system tick - can this schedule an activity update next tick for them?
* bug: society job is not notified if a subtask fails, causing it to be infinitely attempted
	* e.g. haul things into a container but it's full
	* a set of completed tasks should be maintained per job

## World generation
* biomes
* features e.g. trees, hills
	* trees are entities, not (only) blocks
	* accurate-ish rivers, caves
		* varying river width from streams to large uncrossable rivers
		* varying river flow speed
	* magma very low down, or it just gets too hot
	* volcano affects world gen in past
* finite pregenerated world in xy (planet), infinite in z
	* wrapping x,y coordinates is a beefy task, for something that doesnt happen very often
		world loader wraps coords so it never requests slabs out of bounds of the planet
		chunks are loaded and rendered at their true wrapped positions e.g. if worldsize=8, chunks x=0, x=8, x=-8 are the same chunk
		entities must be aware of this! all distance checks must take this into account (https://blog.demofox.org/2017/10/01/calculating-the-distance-between-points-in-wrap-around-toroidal-space/)
		use different base noise for biomes and blend (http://parzivail.com/procedural-terrain-generaion/)
	* chunk and region resolution should wrap around explicitly/fail in generator. should the world loader wrap coords
* unique species and settlements with societies to discover in different environments
	* underground species with no eyes, cave houses
	* underwater people
	* mud crabs with human arms
	* savage cavemen who sneak around in darkness, break bones then drag them back to the cave
* generate new terrain when society member explores, rather than just camera movement. config option!
* bug: a change in the middle of 1 chunk triggers bogus occlusion updates across neighbouring slabs. something to do with occlusion comparison
* grass colour and flora depends on biome/moisture

## Voxel world mechanics
* fluid blocks
	* infinite sources/flows at the world edges
* modification
	* entities building/placing blocks
	* block damage e.g. from explosion
	* side effect of interacting with a block
* puddles/spills/splatters on the ground
	* picked up and spread by entities
* blocks that technically solid but possible to pass through
	* hedges, bushes
* map chunks to torus and make the world wrap-around

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
* perfect hashing for component name lookup
* terrain finalizer should not propogate to neighbours if single block changes arent on boundary
* unchecked_unwrap
* inventory and physical body lookups/searches could be expensive, cache unchanged
* biggy: consider using separate ECS universes for long and short living entities, if having multiple geneations alive at a time has large memory usage
* dynstack for ai dses and considerations, to avoid the huge amount of boxing
* experiment with PGO
* consider replacing expensive area link checking (extending into neighbour to check each block) with simple global lookup of (blockpos, direction, length)
* physics system is unnecessarily checking the bounds of every entity every tick - skip this expensive check if stationary and slab hasn't changed
* when submitting slab changes to the worker pool, cancel any other tasks queued for the same slab as they're now outdated

### Memory usage
* CoW terrain slabs
* store sparse block metadata in the containing slab instead of in each block
* LEAK: opengl sub buffers are being leaked, ~3MB per restart

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
* bug: occlusion flickers after world changes, updates are probably being queued for too long

## Building and testing
* separate config and preset for tests
* fuzzing
* stress test
* code coverage in CI
* smoke tests i.e. world+entity+food, should pickup and eat some. could use events to make sure or just query world after a time

## Code quality
* track down unwraps/expects and replace with results
* less repetition in chunk/terrain/chunkbuilder/chunkbuilderapply/slicemut
* define rates, scales, units etc in unit crate e.g. metabolism, durabilities
* error context chaining would be useful
* consider using `bugsalot` macros to replace .unwrap()/.expect() with logging and make them continuable

## Engine
* explicit namespacing for entity definitions e.g. "core:food_apple"
* detect if debugger is present/breakpoint is hit and pause the gameloop, to avoid the insane catch up after continuing
* separate binary for definition file validation
* instead of sleeping to wait for world to load, check if panicked every second
* consider replacing 1:1 world threadpool with async threadpool

## Entity diversity
* animal species
	* dogs, cats, birds, deer
* individual stats for needs
* cats chase mice
* birds/small animals swarm food crumbs left by someone eating

### Dogs
* dogs pick up sticks, move faster than humans, chase cats
* breeds have different characteristics
	* soft mouth vs hard mouth
	* some more likely to pick up items
	* some more likely to accidewntally break things in their mouth
* can be tamed, become part of society?
* can bond to 1 or 2 humans, follow them about, distressed when separated
* if stressed/not fulfilled, chew and damage things
* smell food carried by others, whine and follow them
	* stronger smell emitted if in hand or in the open, less so if in sealed bag/container
* people (who like dogs) see dogs and go over to pet for emotional support
	* can also play fetch with a stick and other riveting games
* dogs play together e.g. tag


## Simulation depth
* entity interaction
	* taming, hunting, social
* entity needs
	* drink
	* toilet
	* social
	* sleep
* enemies/hostiles can break blocks
* thieves/desperate people (e.g. dying of hunger) can ignore item/container ownership and steal things
* animals and plants provide various resources aside from meat
	* skin/leather/bladder for waterskins, clothes, armour
	* bones for tools
	* tree bark as a very weak material
	* woven plant materials
	* milk
	* fur

### Physical wellbeing
* distinct body parts
* wellbeing of individual parts affects stats
* gradual healing and tending
* track injury causes e.g. arrow in leg, fired by X at time Y with weapon Z
* blood flow that can be blocked off
* inventory system should be on top of physical body, which defines number and availability of slots
	* e.g. parts are marked as mounting points, mounting affects the availability of other mounts (backpack (shoulder) vs briefcase (hand) vs ... skin pocket?? (wherever it is))
