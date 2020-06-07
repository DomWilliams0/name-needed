use imgui::{im_str, CollapsingHeader, Condition, ImStr, Ui, Window};

use common::InnerSpace;
use simulation::input::EntityDetails;

use crate::render::sdl::ui::windows::UiBundle;
use crate::ui_str;

pub struct SelectionWindow;

enum Value<'a> {
    Hide,
    None(&'static str),
    Some(&'a ImStr),
    Wrapped(&'a ImStr),
}

trait UiExt {
    fn key_value<'a, F: FnOnce() -> Value<'a>>(&'a self, key: &ImStr, value: F, color: [f32; 4]);
}

impl UiExt for Ui<'_> {
    fn key_value<'a, F: FnOnce() -> Value<'a>>(&'a self, key: &ImStr, value: F, color: [f32; 4]) {
        let value = value();
        if let Value::Hide = value {
            return;
        }

        self.text_colored(color, key);
        self.same_line_with_spacing(self.calc_text_size(key, false, 0.0)[0], 20.0);
        match value {
            Value::Some(val) => {
                self.text(val);
            }
            Value::Wrapped(val) => {
                self.text_wrapped(&val);
            }
            Value::None(val) => self.text_disabled(val),
            _ => unreachable!(),
        }
    }
}

const COLOR_GREEN: [f32; 4] = [0.4, 0.77, 0.33, 1.0];
const COLOR_ORANGE: [f32; 4] = [1.0, 0.46, 0.2, 1.0];
const COLOR_BLUE: [f32; 4] = [0.2, 0.66, 1.0, 1.0];

impl SelectionWindow {
    pub fn render<'ui>(&mut self, bundle: &mut UiBundle) {
        let ui = bundle.ui;
        let strings = bundle.strings;

        Window::new(im_str!("Selection"))
            .position([10.0, 140.0], Condition::Appearing)
            .always_auto_resize(true)
            .build(ui, || {
                let selection = match &bundle.blackboard.selected {
                    None => {
                        ui.text_disabled(im_str!("No entity selected"));
                        return;
                    }
                    Some(e) => e,
                };

                ui.key_value(
                    im_str!("Entity:"),
                    || Value::Some(ui_str!(in strings, "{:?}", selection.entity)),
                    COLOR_GREEN,
                );
                ui.key_value(
                    im_str!("Position:"),
                    || Value::Some(ui_str!(in strings, "{}", selection.transform.position)),
                    COLOR_GREEN,
                );

                let title = match &selection.details {
                    EntityDetails::Living { .. } => im_str!("Living entity"),
                    EntityDetails::Item { .. } => im_str!("Item"),
                };

                if CollapsingHeader::new(title)
                    .default_open(true)
                    .frame_padding(true)
                    .build(ui)
                {
                    match &selection.details {
                        EntityDetails::Living {
                            activity,
                            hunger,
                            path_target,
                        } => {
                            ui.key_value(
                                im_str!("Velocity:"),
                                || {
                                    Value::Some(ui_str!(in strings,
                                        "{:.2}m/s",
                                        selection.transform.velocity.magnitude()
                                    ))
                                },
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
                                COLOR_ORANGE,
                            );
                        }
                        EntityDetails::Item { item, edible } => {
                            ui.key_value(
                                im_str!("Name:"),
                                || Value::Some(ui_str!(in strings, "{}", item.name)),
                                COLOR_BLUE,
                            );
                            ui.key_value(
                                im_str!("Class:"),
                                || Value::Some(ui_str!(in strings, "{:?}", item.class)),
                                COLOR_BLUE,
                            );
                            ui.key_value(
                                im_str!("Condition:"),
                                || Value::Some(ui_str!(in strings, "{}", item.condition)),
                                COLOR_BLUE,
                            );
                            ui.key_value(
                                im_str!("Mass:"),
                                || Value::Some(ui_str!(in strings, "{}kg", item.mass)),
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
                                COLOR_BLUE,
                            );
                        }
                    };
                }
            });
    }
}
