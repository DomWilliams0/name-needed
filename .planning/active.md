# Active tasks

* [X] runtime and task types
* [X] repurpose event system to use async tasks as subscribers, not entities
* [X] async-safe activity context to get access to the world
	* [X] replace EcsWorldFrameRef with this
* [X] remove timer elapsed event
* [o] new activity system with async activities
	* [X] nop that only uses a timer
	* [X] wander that uses goto
	* [X] goto and break block
	* [X] goto and equip
		* [X] collapse pickup into this as it's the same problem
		* [X] cancel early if item is destroyed
	* [X] eat
	* [ ] goto and haul
* [o] e2e tests
		* [X] equipping an item activity
			* [X] far away and needs pickup, inventory is empty
			* [X] far away and needs pickup, equip slots are full
			* [X] in inv but not equipped
			* [X] already equipped
		* [ ] ensure test runner is run from project root via --bin
		* [X] allow tests to receive entity events OOB
		* [ ] hauling activity
* [X] report activity and subactivities in ui
* [X] reintegrate society job with activity
* [X] consider removing unnecessary Arc and thread safety from single threaded runtime
* [X] cancelling/interrupting of current activity
* [ ] refactor event queue consumption to not require event cloning
	* [ ] also dont expect immediate unsubscription from event
* [ ] avoid unconditional cloning of taskref during runtime polling
* [ ] ensure extra events in queue after unsubscripting/switching task are not a problem
* [ ] add safe !Send wrappers for component references that can't be held across awaits in activities
* [ ] consider pinninig the runtime too to avoid overhead of looking up resource in world/ref counting
* [X] consider parking the task to implement timers (like events) to avoid overhead of manually triggered future allocation
* [X] reuse status updater allocation when changing activities
* [X] add exertion to activity status
* [ ] remove old deprecated Activities and remove 2 suffix
* [ ] add check for space anywhere in inventory before deciding to go pick something up
* [X] enable activities to call each other directly
* [X] replace outdated game preset with just a config file path argument, where config specifies world loader
* [ ] bug: entities are not rendered on 1 particular linux laptop
