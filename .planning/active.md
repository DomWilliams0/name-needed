# Active tasks

* [o] item stacks
	* [X] stack of homogenous items
	* [ ] split into multiple stacks, or remove single items
	* [ ] show number of items beneath a stack
	* [X] inspect a stack's contents in the ui
* [X] components should declare if they should be copied into/out of stacks
* [.] building blocks
	* [X] no materials, single block only, near-instant completion
	* [O] require materials
		* [ ] check inventory for materials
		* [ ] check society containers for materials
		* [X] check nearby area for materials
		* [X] reserve materials
	* [ ] build progression
	* [ ] continuable progress after interruption
	* [ ] define materials in data (wood, stone)
	* [ ] define wall archetype in data
* [ ] allow multiple concurrent workers
* [X] chore: replace direct uses of specs Entity with our own wrapper
* [ ] ui commands to build
* [ ] fix HasExtraHandsForHauling returning 1.0 eagerly for any haul task, causing constant churn of activity choice
