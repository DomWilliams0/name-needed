# Active tasks

* [X] impl clone for dyn Dse
* [X] impl eq for dyn Dse
* [X] store parallel array of dse scores in ai instead of alongside dses
* [X] store stream dses within intelligence
* [X] multistep decision progress pipeline for choosing the best candidate for a task
    * [X] add None variant to intelligent decision, default to nop
* [X] make WeightedDse more ergonomic
* [X] fix tests
* [X] update all society jobs once per tick
* [ ] use frame alloc for boxed ai blackboard without lifetime
* [X] fix reservation flip-flop
* [X] fix break block job terminating early
* [ ] fix over-gathering of materials when already on the way
* [X] use rust-derivative to avoid manually implementing clone/debug/default etc
* [X] fix wander and eating (blackboard invalidated?)
* [X] when thinking, should reset subactivity status
* [X] rebalance weight of proximity-to a build/break job
* [ ] when evaluating a break/build thing in a range, dont always do all blocks in the same order
* [o] add check for enough hands before considering finding food to pickup
* [X] remove AiBox from best candidate choosing
* [X] improve WorldPosition/WorldPoint debug impls to be same as display
* [X] add support for targets to dses
*   * [X] port food and item gathering to use this
* [ ] lazily calculate targets for dses? or easier: specific filter for when the dse is impossible
    regardless of targets
        * when finding food nearby, the possible targets should only be evaluated if hungry, which
        is the first consideration (returns 0.0 if not hungry). but currently all local food will be
        searched for and expanded into targeted dses regardless of hunger level.
* [ ] ensure a single entity doesnt appear in all best candidate lists and dominate a society job
    when its chosen for 1 only
* [X] build ui element in GatherAndBuild test is invisible
* [X] fix gather material haul decision "change" when split stack is picked up
        * [X] remove last action access from blackboard
* [X] pause
* [ ] fast forward
* [ ] update readme controls for pause+ff
* [ ] bug: if noone considers a dse as initial choice, its best candidates are never tracked, then
    everyone can choose the same one (manifests if a society job is assigned while paused)
* [ ] bug: 1 completion is asserted wrongly in Haul society job, can get many cancellations
