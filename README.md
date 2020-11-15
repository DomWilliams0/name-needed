# name-needed

[![Build Status](https://travis-ci.com/DomWilliams0/name-needed.svg?branch=develop)](https://travis-ci.com/DomWilliams0/name-needed)
[![Devlog](https://img.shields.io/badge/devlog-domwillia.ms-orange)](https://domwillia.ms)
[![Lines](https://tokei.rs/b1/github/DomWilliams0/name-needed)](https://github.com/XAMPPRocky/tokei)
[![Files](https://tokei.rs/b1/github/DomWilliams0/name-needed?category=files)](https://github.com/XAMPPRocky/tokei)

A one man effort to produce an **open source**, **intuitive** and **high performance**  Dwarf Fortress-esque game. Needs a name.

* * *

## Progress log
* 22 Aug 2020: <em>World modification and collective society task queue with a bunch of internal restructuring, including data driven entity definitions, structured logging and event-based AI.</em>
<p style="margin: auto">
    <img src=".screenshots/world-modification.gif"/>
</p>

* 14 Jun 2020: <em>Basic, boring procedural terrain generation.</em>
<p style="margin: auto">
    <img src=".screenshots/procgen-basic.png"/>
</p>

*[Continued here](PROGRESS.md)*

## Building and running

The engine uses SDL2 and OpenGL, and is developed primarily on Linux, although it seems to work fine on Windows too.

If you don't have SDL2 installed, the [bundled](https://github.com/Rust-SDL2/rust-sdl2/blob/ed465322d137e207b03403a6f452d176ef9efda0/README.md#bundled-feature) feature of SDL can download and compile it for you (requires a C compiler).

I use the latest stable Rust toolchain and the newest fanciest language features, so no promises for a Minimal Supported Rust Version.

```
$ git clone https://github.com/DomWilliams0/name-needed

$ cd name-needed/

$ # optionally modify game config, see below

$ cargo run
```

### Configuration

The game config can be found in `resources/config.ron`. This contains settings for the game engine, world generation and entity spawning parameters.

Entity definitions live in `resources/definitions/` and define the stats and capabilities of all entities, both living and inanimate.

The environment variable `NN_LOG` configures logging, set it to one of `trace`, `debug`, `info` (default), `warning`, `error`, `critical`.

The `--scenario` parameter chooses a specific situation to spawn entities in, for example people hauling things to a chest or wandering around and picking up food. Provide an invalid scenario to list all available ones (sorry, what an awful interface).


### Usage

*Note: the "game" is currently very much a demo and not very playable in the slightest. Abandon all expectations!*

* <kbd>Esc</kbd> to exit (most importantly)
* <kbd>R</kbd> to restart
* <kbd>Left-click</kbd> to select an entity and view their stats in the debug menu
	* Command them to go to or break a selected block via `Divine control`
	* Select an item and order it to be hauled to the tile selection in the `Society` menu
* <kbd>Right-click</kbd> to drag a selection over blocks in the world
	* Command them to collaborate to break blocks via the `Society` menu
	* Set and place blocks via the `Selection` menu
* Move the camera sideways with <kbd>WASD</kbd>, and vertically with the <kbd>Arrow keys</kbd>
