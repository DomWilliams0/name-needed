# Active tasks

* [X] runtime and task types
* [X] repurpose event system to use async tasks as subscribers, not entities
* [o] async-safe activity context to get access to the world
	* [ ] replace EcsWorldFrameRef with this
* [ ] remove timer elapsed event
* [o] new activity system with async activities
	* [X] nop that only uses a timer
	* [X] wander that uses goto
	* [ ] goto and break block
	* [ ] goto and haul
	* [ ] goto and pickup
	* [ ] goto and equip
	* [ ] ...the remaining activities
* [X] report activity and subactivities in ui
* [ ] reintegrate society job with activity
* [X] consider removing unnecessary Arc and thread safety from single threaded runtime
* [X] cancelling/interrupting of current activity
* [ ] refactor event queue consumption to not require event cloning
	* [ ] also dont expect immediate unsubscription from event
* [ ] avoid unconditional cloning of taskref during runtime polling
* [ ] ensure extra events in queue after unsubscripting/switching task are not a problem
* [ ] add safe !Send wrappers for component references that can't be held across awaits in activities
* [ ] consider pinninig the runtime too to avoid overhead of looking up resource in world/ref counting
* [ ] consider parking the task to implement timers (like events) to avoid overhead of manually triggered future allocation
* [ ] reuse status updater allocation when changing activities
* [ ] add exertion to activity status
