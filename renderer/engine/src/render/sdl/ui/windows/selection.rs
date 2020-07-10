use imgui::{im_str, CollapsingHeader, TreeNode};

use common::{InnerSpace, Itertools};
use simulation::input::{BlockPlacement, DivineInputCommand, EntityDetails, InputCommand};

use crate::render::sdl::ui::windows::{
    UiBundle, UiExt, Value, COLOR_BLUE, COLOR_GREEN, COLOR_ORANGE,
};
use crate::ui_str;
use simulation::{BlockType, IntoEnumIterator};

pub struct SelectionWindow {
    block_placement: BlockPlacement,
}

impl SelectionWindow {
    pub fn render(&mut self, bundle: &mut UiBundle) {
        let ui = bundle.ui;
        let strings = bundle.strings;

        TreeNode::new(im_str!("Entity selection"))
            .frame_padding(true)
            .build(ui, || {
                let selection = match &bundle.blackboard.selected_entity {
                    None => {
                        ui.text_disabled(im_str!("No entity selected"));
                        return;
                    }
                    Some(e) => e,
                };

                ui.key_value(
                    im_str!("Entity:"),
                    || Value::Some(ui_str!(in strings, "{:?}", selection.entity)),
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

                if CollapsingHeader::new(title).frame_padding(true).build(ui) {
                    match &selection.details {
                        EntityDetails::Living {
                            activity,
                            hunger,
                            path_target,
                            society,
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
                                im_str!("Activity:"),
                                || {
                                    if let Some(activity) = activity {
                                        Value::Wrapped(ui_str!(in strings, "{}", activity))
                                    } else {
                                        Value::None("None")
                                    }
                                },
                                None,
                                COLOR_ORANGE,
                            );

                            let society_name = society.and_then(|handle| {
                                bundle
                                    .blackboard
                                    .societies
                                    .society_by_handle(handle)
                                    .map(|s| s.name())
                            });
                            ui.key_value(
                                im_str!("Society:"),
                                || {
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

                            if CollapsingHeader::new(im_str!("Divine control"))
                                .leaf(true)
                                .build(ui)
                            {
                                if let Some(tile) =
                                    bundle.blackboard.selected_tiles.bounds_single_tile()
                                {
                                    if ui.button(im_str!("Go to selected block"), [0.0, 0.0]) {
                                        bundle.commands.push(InputCommand::IssueDivineCommand(
                                            DivineInputCommand::Goto(tile),
                                        ));
                                    }

                                    if ui.button(im_str!("Break selected block"), [0.0, 0.0]) {
                                        bundle.commands.push(InputCommand::IssueDivineCommand(
                                            DivineInputCommand::Break(tile),
                                        ));
                                    }
                                }
                            }
                        }
                        EntityDetails::Item { item, edible } => {
                            ui.key_value(
                                im_str!("Name:"),
                                || Value::Some(ui_str!(in strings, "{}", item.name)),
                                None,
                                COLOR_BLUE,
                            );
                            ui.key_value(
                                im_str!("Class:"),
                                || Value::Some(ui_str!(in strings, "{:?}", item.class)),
                                None,
                                COLOR_BLUE,
                            );
                            ui.key_value(
                                im_str!("Condition:"),
                                || Value::Some(ui_str!(in strings, "{}", item.condition)),
                                None,
                                COLOR_BLUE,
                            );
                            ui.key_value(
                                im_str!("Mass:"),
                                || Value::Some(ui_str!(in strings, "{}kg", item.mass)),
                                None,
                                COLOR_BLUE,
                            );

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
            });

        TreeNode::new(im_str!("Tile selection"))
            .frame_padding(true)
            .build(ui, || {
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
                            .push(InputCommand::FillSelectedTiles(self.block_placement, bt));
                    }
                };

                for mut types in BlockType::into_enum_iter().chunks(3).into_iter() {
                    types.next().map(&mut mk_button);
                    for bt in types {
                        ui.same_line(0.0);
                        mk_button(bt);
                    }
                }
            });
    }
}

impl Default for SelectionWindow {
    fn default() -> Self {
        Self {
            block_placement: BlockPlacement::Set,
        }
    }
}