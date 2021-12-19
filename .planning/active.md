# Active tasks

* [X] item stacks
	* [X] stack of homogenous items
	* [X] split into multiple stacks, or remove single items
	* [X] inspect a stack's contents in the ui
* [X] components should declare if they should be copied into/out of stacks
* [o] building blocks
	* [X] no materials, single block only, near-instant completion
	* [O] require materials
		* [X] check inventory for materials
		* [ ] check society containers for materials
		* [X] check nearby area for materials
		* [X] reserve materials
	* [o] build activity
	* [.] build progression block
		* [X] spawn when job is created
		* [X] remove on completion
		* [X] define progress rate per build
		* [X] continuable progress after interruption
		* [ ] drop unconsumed materials on cancel/destruction
	* [X] split a stack to reserve only the exact number needed
		* [ ] check before destroying all reserved materials
	* [ ] define materials in data (wood, stone)
	* [ ] define wall archetype in data
* [ ] allow multiple concurrent workers
* [X] chore: replace direct uses of specs Entity with our own wrapper
* [ ] ui commands to build
* [X] show build details in ui
* [X] fix constant churn of gather materials DSE choice
* [ ] bug: if multiple people are sharing a gather task, they will keep collecting even after the last one is delivered - panics on extra unexpected delivery
* [ ] allow smooth changing of material gathering target job without dropping the current haul
* [ ] prioritise material gathering for the most complete job, rather than random/round robin
* [ ] text rendering
	* [ ] render a string anywhere in the world through Renderer trait
	* [ ] show number of items underneath a stack
	* [ ] show NameComponent under entities (selected only?)
		* [X] add "unnamed" fallback to component
	* [ ] show building job info
* [X] remove destroy_container() helper and move impl into kill_entity
* [ ] lua api for building things
* [ ] define builds in data rather than code
* [ ] build material requirement engine
* [ ] stacks split into 1 should be "downgraded" into a single item automatically
* [ ] allow stacks greater than 65535
* [ ] log entity events for stack operations
* [ ] fix sdl2 building on ci
