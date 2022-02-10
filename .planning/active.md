# Active tasks

* [X] add sheep entity
* [X] species component
* [X] herding component
*  herdable entities should start/join a herd with nearby members of same species
    * [X] herd allocation
    * [X] debug renderer to show herds
    * [X] leave herd if alone
    * [X] add cow species to ensure they form their own herds
* herd formation
    * [X] dont leave immediately after leaving the radius, but rather slowly decay over a few ticks
    * [X] if parts of a herd become disconnected, split into 2
    * [X] the herd that wins during merging should be the biggest, not just the first one found
* [ ] startling of sheep based on senses
* [ ] propagation of startling through herd
    * could propagate the original startle source, or just be startled at the startlement of another
* [ ] fleeing from startle
    * use another kind of navigation that doesn't use path finding? navigate locally but just away.
        or rather search outward for a flee destination instead of choosing top-down
    * could use this for wandering too
* [X] use type name for debug renderer idenfier
* [ ] add chained modify_x|y|z helper to worldpoint
* [X] frame allocator helpers for debug/display/vec
* [ ] reuse some allocaions in herd joining system
