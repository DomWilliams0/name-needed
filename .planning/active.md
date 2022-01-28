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
* [ ] fix reservation flip-flop
* [X] fix break block job terminating early
* [ ] fix over-gathering of materials when already on the way
* [ ] use rust-derivative to avoid manually implementing clone/debug/default etc
* [X] fix wander and eating (blackboard invalidated?)
* [ ] when thinking, should reset subactivity status
* [X] rebalance weight of proximity-to a build/break job
* [ ] when evaluating a break/build thing in a range, dont always do all blocks in the same order
