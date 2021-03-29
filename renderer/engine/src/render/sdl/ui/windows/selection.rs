use imgui::{im_str, CollapsingHeader, ImStr, TabItem, TreeNode, Ui};

use common::{InnerSpace, Itertools};
use simulation::input::{
    BlockPlacement, DivineInputCommand, EntityDetails, SelectedEntityDetails, UiCommand,
};
use simulation::{
    ActivityComponent, BlockType, ComponentWorld, Container, IntoEnumIterator, InventoryComponent,
    NameComponent, E,
};

use crate::render::sdl::ui::memory::PerFrameStrings;
use crate::render::sdl::ui::windows::{
    UiBundle, UiExt, Value, COLOR_BLUE, COLOR_GREEN, COLOR_ORANGE, COLOR_RED,
};
use crate::ui_str;

pub struct SelectionWindow {
    block_placement: BlockPlacement,
}

impl SelectionWindow {
    pub fn render(&mut self, bundle: &mut UiBundle) {
        let ui = bundle.ui;
        let strings = bundle.strings;

        TabItem::new(im_str!("Selection")).build(ui, || {
            match &bundle.blackboard.selected_entity {
                None => {
                    ui.text_disabled(im_str!("No entity selected"));
                }
                Some(selection) => {
                    ui.key_value(
                        im_str!("Entity:"),
                        || Value::Some(ui_str!(in strings, "{:?}", selection.entity)),
                        None,
                        COLOR_GREEN,
                    );
                    ui.key_value(
                        im_str!("Name:"),
                        || {
                            if let Some(name) = selection.name {
                                Value::Some(ui_str!(in strings, "{}", name.0))
                            } else {
                                Value::None("Unnamed")
                            }
                        },
                        None,
                        COLOR_GREEN,
                    );
                    ui.key_value(
                        im_str!("Position:"),
                        || Value::Some(ui_str!(in strings, "{}", selection.transform.position)),
                        None,
                        COLOR_GREEN,
                    );

                    let title = match &selection.details {
                        EntityDetails::Living { .. } => im_str!("Living entity"),
                        EntityDetails::Item { .. } => im_str!("Item"),
                    };

                    if CollapsingHeader::new(title)
                        .frame_padding(true)
                        .default_open(true)
                        .build(ui)
                    {
                        match &selection.details {
                            EntityDetails::Living {
                                activity,
                                hunger,
                                path_target,
                                society,
                                inventory,
                            } => {
                                ui.key_value(
                                    im_str!("Velocity:"),
                                    || {
                                        Value::Some(ui_str!(in strings,
                                        "{:.2}m/s",
                                        selection.transform.velocity.magnitude()
                                        ))
                                    },
                                    None,
                                    COLOR_ORANGE,
                                );

                                ui.key_value(
                                    im_str!("Satiety:"),
                                    || {
                                        if let Some(hunger) = hunger {
                                            let (current, max) = hunger.satiety();
                                            Value::Some(ui_str!(in strings, "{}/{}", current, max))
                                        } else {
                                            Value::Hide
                                        }
                                    },
                                    None,
                                    COLOR_ORANGE,
                                );

                                ui.key_value(
                                    im_str!("Navigating to:"),
                                    || {
                                        if let Some(target) = path_target {
                                            Value::Some(ui_str!(in strings, "{}", target))
                                        } else {
                                            Value::None("Nowhere")
                                        }
                                    },
                                    None,
                                    COLOR_ORANGE,
                                );

                                ui.key_value(
                                    im_str!("Society:"),
                                    || {
                                        let society_name = society.and_then(|handle| {
                                            bundle
                                                .blackboard
                                                .societies
                                                .society_by_handle(handle)
                                                .map(|s| s.name())
                                        });

                                        if society.is_some() {
                                            Value::Some(if let Some(name) = society_name {
                                                ui_str!(in strings, "{}", name)
                                            } else {
                                                im_str!("Invalid handle")
                                            })
                                        } else {
                                            Value::None("None")
                                        }
                                    },
                                    society.map(|handle| ui_str!(in strings, "{:?}", handle)),
                                    COLOR_ORANGE,
                                );

                                self.do_inventory(bundle, inventory);

                                ui.separator();

                                self.do_activity(ui, strings, activity);

                                TreeNode::new(im_str!("Divine control")).build(ui, || {
                                    if let Some(tile) =
                                        bundle.blackboard.selected_tiles.single_tile()
                                    {
                                        if ui.button(im_str!("Go to selected block"), [0.0, 0.0]) {
                                            bundle.commands.push(UiCommand::IssueDivineCommand(
                                                DivineInputCommand::Goto(tile.above()),
                                            ));
                                        }

                                        if ui.button(im_str!("Break selected block"), [0.0, 0.0]) {
                                            bundle.commands.push(UiCommand::IssueDivineCommand(
                                                DivineInputCommand::Break(tile),
                                            ));
                                        }
                                    }
                                });
                            }
                            EntityDetails::Item { condition, edible } => {
                                // TODO list components on item that are relevant (i.e. not transform etc)
                                ui.key_value(
                                    im_str!("Condition:"),
                                    || Value::Some(ui_str!(in strings, "{}", condition)),
                                    None,
                                    COLOR_BLUE,
                                );

                                if let Some(physical) = selection.physical {
                                    ui.key_value(
                                        im_str!("Volume:"),
                                        || Value::Some(ui_str!(in strings, "{}", physical.volume)),
                                        None,
                                        COLOR_BLUE,
                                    );

                                    ui.key_value(
                                        im_str!("Size:"),
                                        || Value::Some(ui_str!(in strings, "{}", physical.size)),
                                        None,
                                        COLOR_BLUE,
                                    );
                                }

                                ui.key_value(
                                    im_str!("Nutrition:"),
                                    || {
                                        if let Some(edible) = edible {
                                            Value::Some(
                                                ui_str!(in strings, "{}", edible.total_nutrition),
                                            )
                                        } else {
                                            Value::None("Inedible")
                                        }
                                    },
                                    None,
                                    COLOR_BLUE,
                                );
                            }
                        };
                    }
                }
            };

            ui.separator();

            // world selection

            let bounds = match bundle.blackboard.selected_tiles.bounds() {
                None => {
                    ui.text_disabled(im_str!("No tile selection"));
                    return;
                }
                Some(bounds) => bounds,
            };

            let (from, to) = bounds;
            let w = (to.0 - from.0).abs() + 1;
            let h = (to.1 - from.1).abs() + 1;
            let z = (to.2 - from.2).abs().slice() + 1;

            ui.key_value(
                im_str!("Size:"),
                || {
                    if z == 1 {
                        Value::Some(ui_str!(in strings, "{}x{} ({})", w, h, w*h))
                    } else {
                        Value::Some(ui_str!(in strings, "{}x{}x{} ({})", w, h,z, w*h*z))
                    }
                },
                None,
                COLOR_BLUE,
            );

            ui.key_value(
                im_str!("From:"),
                || Value::Some(ui_str!(in strings, "{}", from)),
                None,
                COLOR_ORANGE,
            );

            ui.key_value(
                im_str!("To:  "),
                || Value::Some(ui_str!(in strings, "{}", to)),
                None,
                COLOR_ORANGE,
            );

            TreeNode::new(im_str!("Generation info"))
                .default_open(false)
                .build(ui, || {
                    let details = match (
                        bundle.blackboard.selected_block_details.as_ref(),
                        bundle.blackboard.selected_tiles.single_tile(),
                    ) {
                        (None, Some(_)) => {
                            ui.text_disabled(im_str!("Incompatible terrain source"));
                            return;
                        }
                        (None, _) => {
                            ui.text_disabled(im_str!("Single selection required"));
                            return;
                        }
                        (Some(t), _) => t,
                    };

                    let (primary, _) = match details.biome_choices.iter().next() {
                        Some(b) => b,
                        None => {
                            ui.text_colored(COLOR_RED, im_str!("Error: missing biome"));
                            return;
                        }
                    };

                    ui.key_value(
                        im_str!("Biome:  "),
                        || Value::Some(ui_str!(in strings, "{:?}", primary)),
                        None,
                        COLOR_GREEN,
                    );

                    ui.text(ui_str!(in strings, "{} candidates", details.biome_choices.len()));
                    for (biome, weight) in details.biome_choices.iter() {
                        ui.text(ui_str!(in strings, " - {:?} ({})", biome, weight));
                    }

                    ui.key_value(
                        im_str!("Coastline proximity:  "),
                        || Value::Some(ui_str!(in strings, "{:.4}", details.coastal_proximity)),
                        None,
                        COLOR_GREEN,
                    );
                    ui.key_value(
                        im_str!("Elevation:  "),
                        || Value::Some(ui_str!(in strings, "{:.4}", details.base_elevation)),
                        None,
                        COLOR_GREEN,
                    );
                    ui.key_value(
                        im_str!("Temperature:  "),
                        || Value::Some(ui_str!(in strings, "{:.4}", details.temperature)),
                        None,
                        COLOR_GREEN,
                    );
                    ui.key_value(
                        im_str!("Moisture:  "),
                        || Value::Some(ui_str!(in strings, "{:.4}", details.moisture)),
                        None,
                        COLOR_GREEN,
                    );
                });

            ui.separator();
            ui.radio_button(
                im_str!("Set blocks"),
                &mut self.block_placement,
                BlockPlacement::Set,
            );
            ui.same_line(0.0);
            ui.radio_button(
                im_str!("Place blocks"),
                &mut self.block_placement,
                BlockPlacement::PlaceAbove,
            );

            let mut mk_button = |bt: BlockType| {
                if ui.button(ui_str!(in strings, "{}", bt), [0.0, 0.0]) {
                    bundle
                        .commands
                        .push(UiCommand::FillSelectedTiles(self.block_placement, bt));
                }
            };

            for mut types in BlockType::into_enum_iter().chunks(3).into_iter() {
                types.next().map(&mut mk_button);
                for bt in types {
                    ui.same_line(0.0);
                    mk_button(bt);
                }
            }

            if let Some((container_entity, container_name, container)) =
                bundle.blackboard.selected_container
            {
                ui.separator();
                ui.key_value(
                    im_str!("Container: "),
                    || Value::Some(ui_str!(in strings, "{}", container_name)),
                    None,
                    COLOR_ORANGE,
                );
                ui.key_value(
                    im_str!("Owner: "),
                    || {
                        if let Some(owner) = container.owner {
                            Value::Some(ui_str!(in strings, "{}", E(owner)))
                        } else {
                            Value::None("No owner")
                        }
                    },
                    None,
                    COLOR_ORANGE,
                );
                ui.key_value(
                    im_str!("Communal: "),
                    || {
                        if let Some(society) = container.communal() {
                            Value::Some(ui_str!(in strings, "{:?}", society))
                        } else {
                            Value::None("Not communal")
                        }
                    },
                    None,
                    COLOR_ORANGE,
                );
                if let Some(SelectedEntityDetails {
                    entity,
                    details: EntityDetails::Living { society, .. },
                    ..
                }) = bundle.blackboard.selected_entity
                {
                    if ui.button(im_str!("Set owner"), [0.0, 0.0]) {
                        bundle.commands.push(UiCommand::SetContainerOwnership {
                            container: container_entity,
                            owner: Some(Some(entity)),
                            communal: None,
                        });
                    }
                    if let Some(society) = society {
                        ui.same_line(0.0);
                        if ui.button(im_str!("Set communal"), [0.0, 0.0]) {
                            bundle.commands.push(UiCommand::SetContainerOwnership {
                                container: container_entity,
                                owner: None,
                                communal: Some(Some(society)),
                            });
                        }
                    }
                }

                if ui.button(im_str!("Clear owner"), [0.0, 0.0]) {
                    bundle.commands.push(UiCommand::SetContainerOwnership {
                        container: container_entity,
                        owner: Some(None),
                        communal: None,
                    });
                }
                ui.same_line(0.0);
                if ui.button(im_str!("Clear communal"), [0.0, 0.0]) {
                    bundle.commands.push(UiCommand::SetContainerOwnership {
                        container: container_entity,
                        owner: None,
                        communal: Some(None),
                    });
                }
                self.do_container(ui, strings, im_str!("Contents"), &container.container);
            }
        });
    }

    fn do_inventory(&mut self, bundle: &UiBundle, inventory: &Option<&InventoryComponent>) {
        let ui = bundle.ui;
        let strings = bundle.strings;

        if let Some(inventory) = inventory {
            TreeNode::new(im_str!("Inventory"))
                .default_open(false)
                .build(ui, || {
                    ui.text_colored(
                        COLOR_GREEN,
                        ui_str!(in strings, "{} hands:", inventory.equip_slots().len()),
                    );

                    for slot in inventory.equip_slots() {
                        ui.same_line(0.0);
                        ui.text(ui_str!(in strings, "{} ", slot));
                    }

                    ui.separator();
                    ui.text_disabled(
                        ui_str!(in strings, "{} containers", inventory.containers_unresolved().len()),
                    );

                    for (i, (e, container)) in inventory.containers(bundle.blackboard.world).enumerate() {
                        let name = bundle.blackboard.world.component::<NameComponent>(e).map(|n| n.0.as_str()).unwrap_or("unnamed");
                        self.do_container(
                            ui,
                            strings,
                            ui_str!(in strings, "#{}: {}", i+1, name),
                            container,
                        );
                    }
                });
        }
    }

    fn do_container(
        &mut self,
        ui: &Ui,
        strings: &PerFrameStrings,
        name: &ImStr,
        container: &Container,
    ) {
        TreeNode::new(name).build(ui, || {
            let (max_vol, max_size) = container.limits();
            let capacity = container.current_capacity();
            ui.text_colored(
                COLOR_GREEN,
                ui_str!(in strings, "Capacity {}/{}, size {}", capacity, max_vol, max_size),
            );
            for entity in container.contents() {
                ui.text_wrapped(ui_str!(in strings, " - {}", entity));
            }
        });
    }

    fn do_activity(
        &mut self,
        ui: &Ui,
        strings: &PerFrameStrings,
        activity: &Option<&ActivityComponent>,
    ) {
        ui.key_value(
            im_str!("Activity:"),
            || {
                if let Some(activity) = activity {
                    Value::Wrapped(ui_str!(in strings, "{}", activity.current))
                } else {
                    Value::None("None")
                }
            },
            None,
            COLOR_ORANGE,
        );

        ui.key_value(
            im_str!("Subactivity:"),
            || {
                if let Some(activity) = activity {
                    Value::Wrapped(
                        ui_str!(in strings, "{}", activity.current.current_subactivity()),
                    )
                } else {
                    Value::None("None")
                }
            },
            None,
            COLOR_ORANGE,
        );
    }
}

impl Default for SelectionWindow {
    fn default() -> Self {
        Self {
            block_placement: BlockPlacement::Set,
        }
    }
}
