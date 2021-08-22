# Active tasks

* [X] runtime and task types
* [X] repurpose event system to use async tasks as subscribers, not entities
* [o] async-safe activity context to get access to the world
	* [ ] replace EcsWorldFrameRef with this
* [ ] remove timer elapsed event
* [ ] new activity system with async activities
	* [X] nop that only uses a timer
	* [ ] wander that uses goto
	* [ ] ...the remaining activities
* [X] consider removing unnecessary Arc and thread safety from single threaded runtime
* [X] cancelling/interrupting of current activity
