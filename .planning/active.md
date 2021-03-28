# Active tasks

* [X] represent continents as polygons rather than a region grid
* [X] split up polygon continents into simple biomes
	* coastlines are now identifiable!
	* [X] add render zoom level to see polygon edges clearly
* [o] global temperature map
* [o] global moisure map
* [X] biomes dictate basic height range
	* e.g. sea = -40-0, coast = 0-20, hilly = 15-40 etc
	* [X] choose biome(s) with weights for each block
	* [o] smooth transitions between biome heights
	* [o] tweak biome height ranges
	* [X] specify height range and colour in biomes.cfg
* [ ] fix temperature when combined with elevation
* [X] ensure heightmap is still present underwater
* [ ] override initial camera chunk with 0,0 for preset worlds
* [X] restrict slab loading to planet boundary, wrapping can wait
* [ ] ui button to jump camera up/down to next surface
* [~] remove old grid code
* [ ] rough up coastline edges
* [ ] rough up elevation some more (more octaves?)
* [ ] merge intersecting continents
* [X] return Result from loading biome map
* [X] noise sampling is non determinstic
