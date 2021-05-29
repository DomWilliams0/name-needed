# Active tasks

* [~] update society job in its own system/outside of ai system
	* nah, keep doing it on demand when requested
* [.] task reservation should always not be exclusive, allowing sharing
	* [ ] fewer reservations = higher weight
	* [.] sharing depends on task type
* [X] jobs own a vec of tasks that they maintain internally
* [ ] ai system filters jobs on high level requirements before considering its tasks
* [X] activity component holds an optional ref to job and current task (passed from ai decision) to report failure/success/interruption, which the job handles uniquely
* [ ] list society jobs in the ui
* [ ] show reserved task in entity ui
* [ ] tasks should pregenerate a DSE
* [O] post task completion to job from activity system
	* [ ] unreserve society task on interruption
* [.] replace deprecated job, job list, reservations
