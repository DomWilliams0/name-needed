use imgui::{im_str, ImStr, ImString, TabItem};

use simulation::input::UiCommand;

use crate::render::sdl::ui::context::UiContext;
use crate::ui_str;

pub struct DebugWindow {
    script_input: ImString,
}

const MAX_PATH_INPUT: usize = 256;

// TODO free function instead of method
// impl UiContext<'_> {
//     fn checkbox(&mut self, title: &ImStr, ident: &'static str) {
//         let mut enabled = self.blackboard.enabled_debug_renderers.contains(ident);
//         if self.ui.checkbox(title, &mut enabled) {
//             self.commands
//                 .push(UiCommand::ToggleDebugRenderer { ident, enabled })
//         }
//     }
// }

impl DebugWindow {
    pub fn render(&mut self, context: &mut UiContext) {
        TabItem::new(im_str!("Debug")).build(context.ui(), || {
            /* let view_range = context.blackboard.world_view.expect("blackboard world view range not populated");
            context.ui.text(ui_str!(in context.strings, "World range: {} => {} ({})", view_range.bottom().slice(), view_range.top().slice(), view_range.size()));

            if cfg!(feature = "scripting") {
                context.ui.separator();

                context.ui.input_text(im_str!("##scriptpath"), &mut self.script_input).build();
                if context.ui.button(im_str!("Execute script"), [0.0, 0.0]) {
                    context.commands.push(UiCommand::ExecuteScript(self.script_input.to_str().to_owned().into()))
                }
            }

            // TODO query world instead
            // debug renderers
            context.ui.separator();
            context.checkbox(im_str!("Navigation paths"), "navigation path");
            context.checkbox(im_str!("Navigation areas"), "navigation areas");
            context.checkbox(im_str!("Steering direction"), "steering");
            context.checkbox(im_str!("Senses"), "senses");
            context.checkbox(im_str!("Feature boundaries"), "feature boundaries");
            context.checkbox(im_str!("Chunk boundaries"), "chunk boundaries");*/
        });
    }
}

impl Default for DebugWindow {
    fn default() -> Self {
        let mut script_input = ImString::with_capacity(MAX_PATH_INPUT);

        // TODO proper default script path
        script_input.push_str("script.lua");

        DebugWindow { script_input }
    }
}
