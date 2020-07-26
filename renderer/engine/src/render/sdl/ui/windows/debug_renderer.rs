use imgui::{im_str, CollapsingHeader, ImStr, TreeNode};

use simulation::input::UiCommand;

use crate::render::sdl::ui::windows::UiBundle;
use crate::ui_str;

pub struct DebugWindow;

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
        TreeNode::new(im_str!("Debug"))
            .frame_padding(true)
            .build(bundle.ui, || {
                let view_range = bundle.blackboard.world_view.expect("blackboard world view range not populated");
                // TODO helpers in Bundle
                bundle.ui.text(ui_str!(in bundle.strings, "World range: {} => {} ({})", view_range.bottom().slice(), view_range.top().slice(), view_range.size()));

                if CollapsingHeader::new(im_str!("Debug renderers"))
                    .default_open(true)
                    .build(bundle.ui) {
                    bundle.checkbox(im_str!("Navigation paths"), "navigation path");
                    bundle.checkbox(im_str!("Navigation areas"), "navigation areas");
                    bundle.checkbox(im_str!("Steering direction"), "steering");
                }

            });
    }
}
