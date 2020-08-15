# Active tasks

* [o] separate ActivitySystem from AiSystem
* [o] refactor activities to block on specified events
	* [X] add back eating
	* [ ] divine orders
* [X] rework item pickup, remove pickup system
* [ ] remove old unused activity system once replaced
* [ ] remove arrivedattargeteventcomponent
* [X] allow event subscribers to subscribe to arbitrary entity events
* [X] add subactivty to activity component to calculate exertion() and display in ui
* [X] tests for event queue when complete
* [ ] replace derive_more::Error with nicer thiserror
* [X] bitmask for event subscription, so All isn't a special case
* [X] assign opaque token to path assignments for future comparisons
* [X] allow cancelling of path finding
* [ ] fix repetitive verbose event logging
* [ ] lite runner config
* [ ] definition validator separate bin
* [X] component world event posting helper
* [ ] fix "unreachable" panic with many entities going for food, typical
