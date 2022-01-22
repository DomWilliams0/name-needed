# Active tasks

* [X] fix over reserving of materials
* [ ] pre-filled society stores of bricks
* [X] show ui block selection as it is dragged
    * [ ] block selection can be added to with ctrl+drag
    * [ ] show selection dimensions in world
* popups and ui selection changes
    * [X] test popups with an invisible imgui window
    * [X] selection-sensitive right click context menu
    * [X] close popup on camera move
    * [X] escape for clearing tile and entity selection (rebind quit)
    * [X] improved ui for ordering people about and issuing society commands via popup
    * [X] close popup after button pressed
    * [ ] build command
* [X] ui for creating jobs for building many blocks
* [ ] ui for wall outline specifically?
    * hovering over button should show outline preview
    * [ ] can specify thickness of wall
    * [ ] can shrink/expand selection by 1 block
* [ ] extend material reservation to include the specific materials in transit for a build,
    to avoid others considering hauling more when it's already on the way
* [X] register builds in data
    * [X] validate build material definition exists?
    * [X] cache build templates intead of iterating all each time
* [X] used cache strings for component names too
* [X] central registry of all builds including a unique identifier to refer to it in ui and its
    requests
* [ ] update controls in readme
* [ ] display basic info about multiple entity selection in ui

