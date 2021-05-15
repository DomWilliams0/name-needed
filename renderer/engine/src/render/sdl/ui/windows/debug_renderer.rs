use imgui::{im_str, ImString, TabItem};

use simulation::input::{UiRequest, UiResponse};

use crate::render::sdl::ui::context::UiContext;
use crate::render::sdl::ui::windows::{UiExt, Value, COLOR_BLUE};

pub struct DebugWindow {
    script_input: ImString,
    script_output: ScriptOutput,
}

enum ScriptOutput {
    NoScript,
    Waiting(UiResponse),
    Done(ImString),
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
            // TODO view range
            // let view_range = context.blackboard.world_view.expect("blackboard world view range not populated");
            // context.ui.text(ui_str!(in context.strings, "World range: {} => {} ({})", view_range.bottom().slice(), view_range.top().slice(), view_range.size()));

            if cfg!(feature = "scripting") {
                context.separator();

                context
                    .input_text(im_str!("##scriptpath"), &mut self.script_input)
                    .build();
                if context.button(im_str!("Execute script##script"), [0.0, 0.0]) {
                    let response = context.issue_request(UiRequest::ExecuteScript(
                        self.script_input.to_str().to_owned().into(),
                    ));
                    self.script_output = ScriptOutput::Waiting(response);
                }

                if matches!(self.script_output, ScriptOutput::Done(_)) {
                    context.same_line(0.0);
                    if context.button(im_str!("Clear output##script"), [0.0, 0.0]) {
                        self.script_output = ScriptOutput::NoScript;
                    }
                }

                if let ScriptOutput::Waiting(resp) = &self.script_output {
                    if let Some(resp) = resp.take_response() {
                        self.script_output =
                            ScriptOutput::Done(ImString::from(format!("{}", resp)));
                    };
                }

                let str = match &self.script_output {
                    ScriptOutput::NoScript => None,
                    ScriptOutput::Waiting(_) => Some(im_str!("Executing...")),
                    ScriptOutput::Done(s) => Some(s.as_ref()),
                };

                if let Some(output) = str {
                    let width = context.window_content_region_width();
                    context.key_value(
                        im_str!("Output:"),
                        || Value::MultilineReadonly {
                            label: im_str!("##scriptoutput"),
                            buffer: output,
                            width,
                        },
                        None,
                        COLOR_BLUE,
                    );
                }
            }

            // TODO query world for debug renderers
            // debug renderers
            /*            context.ui.separator();
                        context.checkbox(im_str!("Navigation paths"), "navigation path");
                        context.checkbox(im_str!("Navigation areas"), "navigation areas");
                        context.checkbox(im_str!("Steering direction"), "steering");
                        context.checkbox(im_str!("Senses"), "senses");
                        context.checkbox(im_str!("Feature boundaries"), "feature boundaries");
                        context.checkbox(im_str!("Chunk boundaries"), "chunk boundaries");
            */
        });
    }
}

impl Default for DebugWindow {
    fn default() -> Self {
        let mut script_input = ImString::with_capacity(MAX_PATH_INPUT);

        // TODO proper default script path
        script_input.push_str("script.lua");

        DebugWindow {
            script_input,
            script_output: ScriptOutput::NoScript,
        }
    }
}
