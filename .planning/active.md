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
* [o] tree sub feature block placement
	* [X] place blocks within root slab
	* [X] place blocks across slab boundaries for unloaded neighbours
	* [ ] place blocks across slab boundaries for already loaded neighbours
* [X] remove redundant matches dependency
* [X] bug: procgen world is vertically flipped ingame
* [ ] remove unwraps in grid coord (un)flattening and handle properly
* [X] bug: deadlock loading terrain
* [X] bug: crashes on assert that feature boundary intersects with a slab boundary on panning upwards on test seed
* [ ] consider caching region/features in planet cache
* [X] feature polygon debug renderer should cache outlines when mutex can not be taken
* [ ] restarting the game while terrain is loading triggers a panic "chunk finalization error threshold passed" - detect restarting?
* [ ] bug: panic "chunk should be present" when zoom=10.0
* [ ] enforce loading all of a region's neighbours before generating slabs (to ensure features are generated and merged fully before placing blocks)
	* [ ] load regions adjacent to already loaded regions only (except initial)
	* [ ] region load status can be unloaded, fully (can have slabs generated), partially (as a neighbour to a fully loaded region)
	* [ ] does this make any tree merging across boundaries pointless, because trees are only placed during slab generation?
