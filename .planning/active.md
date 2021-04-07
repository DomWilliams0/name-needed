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
	* [ ] doesn't work across some chunk boundaries - multiple forest instances?
* [ ] tree sub feature block placement
* [X] remove redundant matches dependency
* [X] bug: procgen world is vertically flipped ingame
* [ ] remove unwraps in grid coord (un)flattening and handle properly
* [X] bug: deadlock loading terrain
