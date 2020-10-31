# Active tasks

* [X] spawn some non-food haulable things around
* [X] dse to haul things to a chest
* [X] haul thing to pos society job
	* selected from society tab
* [X] society owned communal chests
* [X] add chest block type
* [X] attach entity to block
* [X] show attached block entity in UI
* [X] add owner to container
* [X] handle block and entity destruction
* [X] HaulInto activity haul thing into container
* [X] haul from a container too
* [X] consider "concurrent" access to a container
* [X] sort events for container putting/taking
* [X] regression: add back tile selection limit
* [X] fix half dims confusion with physical component
* [X] bug: panic in inventory validation system in travis
* [X] "bug": they always eat any food they're supposed to be hauling
	* [X] handle haul -> equipped transition
	* [X] add consideration of hauling an item to eating dse
* [X] bug: immediately drops hauled thing because no hands free anymore
* [X] bug: hauling something then issuing another job to haul the same thing results in "dropping" it then teleporting the thing
* [X] bug: reloading when changing world from generate to preset create a gross mixture of the 2
* [X] bug: society job isnt notified or cancelled when job fails
	* [X] fail activity
	* [~] notify job
