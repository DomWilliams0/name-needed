use crate::render::sdl::ui::windows::{UiBundle, UiExt, Value, COLOR_BLUE, COLOR_RED};
use crate::ui_str;
use imgui::{im_str, TreeNode};
use simulation::input::{SocietyInputCommand, UiCommand};

pub struct SocietyWindow;

impl SocietyWindow {
    pub fn render(&mut self, bundle: &mut UiBundle) {
        let ui = bundle.ui;

        TreeNode::new(im_str!("Society")).build(ui, || {
            let society_handle = match bundle.blackboard.player_society.0 {
                None => {
                    ui.text_disabled("You don't control a society");
                    return;
                }
                Some(s) => s,
            };

            let society = bundle
                .blackboard
                .societies
                .society_by_handle(society_handle);
            ui.key_value(
                im_str!("Society:"),
                || {
                    if let Some(society) = society {
                        Value::Some(ui_str!(in bundle.strings, "{}", society.name()))
                    } else {
                        Value::Some(im_str!("Invalid handle"))
                    }
                },
                Some(ui_str!(in bundle.strings, "{:?}", society_handle)),
                if society.is_none() {
                    COLOR_RED
                } else {
                    COLOR_BLUE
                },
            );

            match bundle.blackboard.selected_tiles.range() {
                None => ui.text_disabled("No block selection"),
                Some(range) => {
                    if ui.button(im_str!("Break blocks"), [0.0, 0.0]) {
                        bundle.commands.push(UiCommand::IssueSocietyCommand(
                            society_handle,
                            SocietyInputCommand::BreakBlocks(range),
                        ));
                    }
                }
            }
        });
    }
}
