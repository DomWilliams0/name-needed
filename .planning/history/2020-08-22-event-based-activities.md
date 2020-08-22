# Active tasks

* [X] separate ActivitySystem from AiSystem
* [X] refactor activities to block on specified events
	* [X] add back eating
	* [X] divine orders
		* [X] remove divine completion system and component
* [X] rework item pickup, remove pickup system
* [X] remove old unused activity system once replaced
* [X] remove arrivedattargeteventcomponent
* [X] allow event subscribers to subscribe to arbitrary entity events
* [X] add subactivty to activity component to calculate exertion() and display in ui
* [X] tests for event queue when complete
* [~] replace derive_more::Error with nicer thiserror
* [X] bitmask for event subscription, so All isn't a special case
* [X] assign opaque token to path assignments for future comparisons
* [X] allow cancelling of path finding
* [X] fix repetitive verbose event logging
* [X] lite runner config
* [~] definition validator separate bin
* [X] component world event posting helper
* [X] fix "unreachable" panic with many entities going for food, typical
* [X] unreserve interrupted society commands
* [X] allow ci build failures on nightly
* [X] bug: possible to falsely assume stuck in inaccessible place, just because transform.position.floor() happens to be for a split second
	* track current/last-known accessible WorldPosition in transform, to use for path finding/local item search/wander dest search
* [X] customisable nop activity
* [X] possible bug: they spend a lot of time between activities in Nop
	* [X] allow a threshold of number of ticks in nop before warning
* [X] emit arrived event failure when prev target is aborted
* [X] clear current path shouldnt assign a token
* [X] remove my_??? prefix from logging macros
* [X] logging thread panics from overflow, then world thread panics from no logger, yet CI lite runner marks this as a success
* [~] bug: get stuck "doing nothing" but circling and failing to arrive at a point
