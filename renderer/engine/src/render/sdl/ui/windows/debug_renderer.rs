use std::array::IntoIter;
use std::borrow::Cow;

use simulation::input::{UiRequest, UiResponse};

use crate::render::sdl::ui::context::UiContext;
use crate::render::sdl::ui::windows::{UiExt, Value, COLOR_BLUE};
use crate::{open_or_ret, ui_str};

pub struct DebugWindow {
    script_input: String,
    script_output: ScriptOutput,
    enabled_debug_renderers: Vec<Cow<'static, str>>,
}

enum ScriptOutput {
    NoScript,
    Waiting(UiResponse),
    Done(String),
}

const MAX_PATH_INPUT: usize = 256;

impl DebugWindow {
    pub fn render(&mut self, context: &UiContext) {
        let _tab = open_or_ret!(context.new_tab("Debug"));

        let view_range = context.simulation().viewer.terrain_range();
        context.text(ui_str!(in context, "World range: {} => {} ({})",
                view_range.bottom().slice(),
                view_range.top().slice(),
                view_range.size()));

        if cfg!(feature = "scripting") {
            context.separator();

            context
                .input_text("##scriptpath", &mut self.script_input)
                .build();

            if context.button("Execute script##script") {
                let response = context
                    .issue_request(UiRequest::ExecuteScript(self.script_input.clone().into()));
                self.script_output = ScriptOutput::Waiting(response);
            }

            if matches!(self.script_output, ScriptOutput::Done(_)) {
                context.same_line();
                if context.button("Clear output##script") {
                    self.script_output = ScriptOutput::NoScript;
                }
            }

            if let ScriptOutput::Waiting(resp) = &self.script_output {
                if let Some(resp) = resp.take_response() {
                    self.script_output = ScriptOutput::Done(resp.to_string());
                };
            }

            if let ScriptOutput::Waiting(_) | ScriptOutput::Done(_) = &self.script_output {
                let width = context.window_content_region_width();
                let value = match &mut self.script_output {
                    ScriptOutput::Waiting(_) => Value::Some("Executing..."),
                    ScriptOutput::Done(s) => Value::MultilineReadonly {
                        label: "##scriptoutput",
                        buffer: s,
                        width,
                    },
                    ScriptOutput::NoScript => unreachable!(),
                };
                context.key_value("Output:", || value, None, COLOR_BLUE);
            }
        }

        let debug_renderers = context.simulation().debug_renderers;
        for descriptor in debug_renderers.iter_descriptors() {
            let (mut enabled, idx) = match self
                .enabled_debug_renderers
                .iter()
                .position(|s| s == descriptor.identifier)
            {
                Some(i) => (true, i),
                None => (false, 0 /* unused */),
            };

            if context.checkbox(descriptor.name, &mut enabled) {
                context.issue_request(UiRequest::SetDebugRendererEnabled {
                    ident: Cow::Borrowed(descriptor.identifier),
                    enabled,
                });

                // update local state
                if enabled {
                    self.enabled_debug_renderers
                        .push(Cow::Borrowed(descriptor.identifier));
                } else {
                    self.enabled_debug_renderers.swap_remove(idx);
                }
            }
        }
    }

    pub fn enabled_debug_renderers(
        &self,
    ) -> impl Iterator<Item = &Cow<'static, str>> + ExactSizeIterator + '_ {
        self.enabled_debug_renderers.iter()
    }
}

impl Default for DebugWindow {
    fn default() -> Self {
        let mut script_input = String::with_capacity(MAX_PATH_INPUT);

        // TODO proper default script path
        script_input.push_str("resources/script.lua");

        // default debug renderers
        let enabled_debug_renderers = {
            let mut vec = Vec::with_capacity(16);
            vec.extend(IntoIter::new(["axes", "steering"]).map(Cow::Borrowed));
            vec
        };

        DebugWindow {
            script_input,
            script_output: ScriptOutput::NoScript,
            enabled_debug_renderers,
        }
    }
}

mod serialization {
    use std::borrow::Cow;

    use serde::de::Deserializer;
    use serde::ser::Serializer;
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Serialize)]
    struct SerializedDebugWindow<'a> {
        script_input: &'a str,

        enabled_debug_renderers: &'a [Cow<'static, str>],
    }

    #[derive(Deserialize)]
    struct DeserializedDebugWindow<'a> {
        #[serde(borrow)]
        script_input: Cow<'a, str>,

        #[serde(borrow)]
        enabled_debug_renderers: Cow<'a, [String]>,
    }

    impl Serialize for DebugWindow {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let serialized = SerializedDebugWindow {
                script_input: &self.script_input,
                enabled_debug_renderers: &self.enabled_debug_renderers,
            };

            serialized.serialize(serializer)
        }
    }

    impl<'de> Deserialize<'de> for DebugWindow {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let deserialized = DeserializedDebugWindow::deserialize(deserializer)?;
            let script_input = {
                // must preserve capacity
                let mut str = String::with_capacity(MAX_PATH_INPUT);
                str.push_str(&deserialized.script_input);
                str
            };

            let enabled_debug_renderers = deserialized
                .enabled_debug_renderers
                .into_owned()
                .into_iter()
                .map(Cow::Owned)
                .collect();

            Ok(DebugWindow {
                script_input,
                script_output: ScriptOutput::NoScript, // forget script output
                enabled_debug_renderers,
            })
        }
    }
}
