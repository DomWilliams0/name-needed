# Active tasks

* [X] tile selection
	* [X] single block
	* [X] drag rectangle to select 2d region
		* [X] limit to sane size
	* [X] debug info about the region
		* [X] dimensions, block count
	* [~] customisable depth?
	* [X] only if entity or tile are in the viewer range
	* [~] tile selection overlay, showing if the blocks are occluded
	* [~] update selection while dragging instead of only once released
* [X] instantly set contents of region to block type
	* [X] combine all debug/dev menus into 1 in UI
	* [X] save ui to .ini
	* [X] terrain update abstraction
	* [X] post terrain updates to world thread pool
	* [X] benchmark for tall chunks and big updates
	* [X] split cross-chunk ranges into multiple, single chunk ranges
	* [X] allow placing blocks as well as replacing
