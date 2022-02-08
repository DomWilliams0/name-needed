# Active tasks

* [X] add sheep entity
* [ ] species component
* [ ] herding component
*  herdable entities should start/join a herd with nearby members of same species
    * [ ] herd allocation
    * [ ] debug renderer to show herds
    * [ ] leave herd if alone
* [ ] startling of sheep based on senses
* [ ] propagation of startling through herd
    * could propagate the original startle source, or just be startled at the startlement of another
* [ ] fleeing from startle
    * use another kind of navigation that doesn't use path finding? navigate locally but just away.
        or rather search outward for a flee destination instead of choosing top-down
    * could use this for wandering too
