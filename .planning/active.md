# Active tasks

* [X] benchmark for region chunk creation
* add types for region coords instead of (f64, f64)
	* [X] planet point
	* [ ] more for points within chunks and conversions to/from planet point
* [X] rough large scale feature placement
	* [X] detect forest bounds
		* [X] within a single region
		* [X] across region boundaries
* [X] individual tree placement via poisson disks
	* [X] initial tree placement can be invalid
	* [~] doesn't work across some chunk boundaries - multiple forest instances?
		* region loading must be adjacent
* [X] tree sub feature block placement
	* [X] place blocks within root slab
	* [X] place blocks across slab boundaries for unloaded neighbours
	* [X] place blocks across slab boundaries for already loaded neighbours
		* [X] race condition - subfeature world updates are queued and applied, but a finalized LoadedSlab is prepared in the background and plops right over the top. applying updates should wait for a slab if its currently being finalized
* [X] remove redundant matches dependency
* [X] bug: procgen world is vertically flipped ingame
* [X] remove unwraps in grid coord (un)flattening and handle properly
* [ ] overhaul world unit types to hide internals, have a constructor that returns option, and an unchecked version
* [X] bug: deadlock loading terrain
* [X] bug: crashes on assert that feature boundary intersects with a slab boundary on panning upwards on test seed
* [~] consider caching region/features in planet cache
	* no - features depend on the order of discovered region chunks
* [X] feature polygon debug renderer should cache outlines when mutex can not be taken
* [ ] restarting the game while terrain is loading triggers a panic "chunk finalization error threshold passed" - detect restarting?
* [ ] bug: panic "chunk should be present" when zoom=10.0
* [ ] enforce loading all of a region's neighbours before generating slabs (to ensure features are generated and merged fully before placing blocks)
	* [ ] load regions adjacent to already loaded regions only (except initial)
	* [ ] region load status can be unloaded, fully (can have slabs generated), partially (as a neighbour to a fully loaded region)
	* [ ] does this make any tree merging across boundaries pointless, because trees are only placed during slab generation?
* [ ] update readme to suggest downloading release instead of building from scratch
* [ ] investigate perf issue of thousands of occlusion updates for empty all-air chunks
* [ ] bug: if there's no path to a society job, they get stuck for ages constantly trying to nagivate
* [ ] bug: entities glitch up through tree trunks and get stuck at the top when wandering past
* [ ] bug: occlusion shadows cast by blocks above current viewing slice look weird
