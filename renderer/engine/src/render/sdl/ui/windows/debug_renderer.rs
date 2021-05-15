use imgui::{im_str, ImStr, ImString, TabItem};

use simulation::input::UiCommand;

use crate::render::sdl::ui::windows::UiBundle;
use crate::ui_str;

pub struct DebugWindow {
    script_input: ImString,
}

const MAX_PATH_INPUT: usize = 256;

impl UiBundle<'_> {
    fn checkbox(&mut self, title: &ImStr, ident: &'static str) {
        let mut enabled = self.blackboard.enabled_debug_renderers.contains(ident);
        if self.ui.checkbox(title, &mut enabled) {
            self.commands
                .push(UiCommand::ToggleDebugRenderer { ident, enabled })
        }
    }
}

impl DebugWindow {
    pub fn render(&mut self, bundle: &mut UiBundle) {
        TabItem::new(im_str!("Debug"))
            .build(bundle.ui, || {
                let view_range = bundle.blackboard.world_view.expect("blackboard world view range not populated");
                // TODO helpers in Bundle
                bundle.ui.text(ui_str!(in bundle.strings, "World range: {} => {} ({})", view_range.bottom().slice(), view_range.top().slice(), view_range.size()));

                if cfg!(feature = "scripting") {
                    bundle.ui.separator();

                    bundle.ui.input_text(im_str!("##scriptpath"), &mut self.script_input).build();
                    if bundle.ui.button(im_str!("Execute script"), [0.0, 0.0]) {
                        bundle.commands.push(UiCommand::ExecuteScript(self.script_input.to_str().to_owned().into()))
                    }
                }

                // debug renderers
                bundle.ui.separator();
                bundle.checkbox(im_str!("Navigation paths"), "navigation path");
                bundle.checkbox(im_str!("Navigation areas"), "navigation areas");
                bundle.checkbox(im_str!("Steering direction"), "steering");
                bundle.checkbox(im_str!("Senses"), "senses");
                bundle.checkbox(im_str!("Feature boundaries"), "feature boundaries");
                bundle.checkbox(im_str!("Chunk boundaries"), "chunk boundaries");

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
