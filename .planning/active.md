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
		* [X] check inventory for materials
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
* [X] fix constant churn of gather materials DSE choice
* [ ] if multiple people are sharing a gather task, they will keep collecting even after the last one is delivered
* [ ] allow smooth changing of material gathering target job without dropping the current haul
* [ ] prioritise material gathering for the most complete job, rather than random/round robin
