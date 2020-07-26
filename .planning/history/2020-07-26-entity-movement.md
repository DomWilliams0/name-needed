# Active tasks

* [X] gravity so entities dont get stuck in midair
* [X] use world block lookup to avoid the world instead of full collision mesh
* [~] add new path finding mode to cut short to N blocks, to use for wander
* [~] debug renderer to show chunk boundaries
* [X] bug: modifying terrain leads to weird disconnected areas
	* [X] debug renderer to show nav areas
* [~] manually persist gui state e.g. treenode open state
* [X] fix exclusive vs inclusive bounds
* [~] bug: can't select entities on grey floor in one chunk wonder world
* [X] bug: entities sprint to crappy food, give up on it as they arrive, and repeat
	* they were floating 3m above the ground because there was no gravity, oops
* [X] bug: path finding generates bad head-knocking path through staircase
* [X] divine control no longer works
* [X] remove floor_then_ceil hack
