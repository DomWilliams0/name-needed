# Active tasks

* [~] update society job in its own system/outside of ai system
	* nah, keep doing it on demand when requested
* [X] task reservation should always not be exclusive, allowing sharing
	* [X] fewer reservations = higher weight
	* [X] sharing depends on task type
* [X] jobs own a vec of tasks that they maintain internally
* [ ] ai system filters jobs on high level requirements before considering its tasks
* [X] activity component holds an optional ref to job and current task (passed from ai decision) to report failure/success/interruption, which the job handles uniquely
* [X] list society jobs in the ui
	* [~] show completed results
* [X] show reserved task in entity ui
* [ ] cache task->DSE conversion
* [X] post task completion to job from activity system
	* [X] unreserve society task on interruption
* [X] replace deprecated job, job list, reservations
* [X] add tests for reservation list
	* [X] bug: multiple reservations assert triggered when adding society job to haul to a position
