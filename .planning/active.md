# Active tasks

* [X] benchmark for region chunk creation
* add types for region coords instead of (f64, f64)
	* [X] planet point
	* [ ] more for points within chunks and conversions to/from planet point
* [X] rough large scale feature placement
	* [X] detect forest bounds
		* [X] within a single region
		* [X] across region boundaries
* [o] individual tree placement via poisson disks
	* [X] initial tree placement can be invalid
	* [ ] doesn't work across some chunk boundaries - multiple forest instances?
	* [ ] remove bad trees across region boundaries
* [ ] tree sub feature block placement
* [X] remove redundant matches dependency
* [X] bug: procgen world is vertically flipped ingame
* [ ] remove unwraps in grid coord (un)flattening and handle properly
* [X] bug: deadlock loading terrain
* [ ] bug: crashes on assert that feature boundary intersects with a slab boundary on panning upwards on test seed
* [ ] consider caching region/features in planet cache
* [X] feature polygon debug renderer should cache outlines when mutex can not be taken
* [ ] restarting the game while terrain is loading triggers a panic "chunk finalization error threshold passed" - detect restarting?
