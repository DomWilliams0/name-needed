use imgui::Ui;

mod debug_renderer;
mod perf;
mod selection;
mod society;

pub(crate) use debug_renderer::DebugWindow;
pub(crate) use perf::PerformanceWindow;
pub(crate) use selection::SelectionWindow;
pub(crate) use society::SocietyWindow;

enum Value<'a> {
    Hide,
    None(&'static str),
    Some(&'a str),
    Wrapped(&'a str),
    MultilineReadonly {
        label: &'a str,
        /// Has to be mutated to add nul-terminator :(
        buffer: &'a mut String,
        width: f32,
    },
}

trait UiExt {
    fn key_value<'a, V: Into<Value<'a>>>(
        &'a self,
        key: &str,
        value: impl FnOnce() -> V,
        tooltip: Option<&str>,
        color: [f32; 4],
    );
}

impl UiExt for Ui<'_> {
    fn key_value<'a, V: Into<Value<'a>>>(
        &'a self,
        key: &str,
        value: impl FnOnce() -> V,
        tooltip: Option<&str>,
        color: [f32; 4],
    ) {
        let value = value().into();
        if let Value::Hide = value {
            return;
        }

        let group = self.begin_group();

        // label
        self.text_colored(color, key);

        if !matches!(value, Value::MultilineReadonly { .. }) {
            self.same_line_with_spacing(self.calc_text_size(key)[0], 10.0);
        }

        match value {
            Value::Some(val) => {
                self.text(val);
            }
            Value::Wrapped(val) => {
                self.text_wrapped(val);
            }
            Value::None(val) => self.text_disabled(val),
            Value::MultilineReadonly {
                label,
                buffer,
                width,
            } => {
                let _ = self
                    .input_text_multiline(label, buffer, [width, 0.0])
                    .read_only(true)
                    .build();
            }
            Value::Hide => unreachable!(),
        };

        group.end();

        // add tooltip to group
        if let Some(tooltip) = tooltip {
            if self.is_item_hovered() {
                self.tooltip_text(tooltip);
            }
        }
    }
}

impl<'a> From<Option<&'a str>> for Value<'a> {
    fn from(opt: Option<&'a str>) -> Self {
        match opt {
            Some(s) => Value::Some(s),
            None => Value::Hide,
        }
    }
}

impl<'a> From<Result<&'a str, &'static str>> for Value<'a> {
    fn from(res: Result<&'a str, &'static str>) -> Self {
        match res {
            Ok(s) => Value::Some(s),
            Err(err) => Value::None(err),
        }
    }
}

impl<'a> From<&'a str> for Value<'a> {
    fn from(str: &'a str) -> Value<'a> {
        Value::Some(str)
    }
}

const COLOR_GREEN: [f32; 4] = [0.4, 0.77, 0.33, 1.0];
const COLOR_ORANGE: [f32; 4] = [1.0, 0.46, 0.2, 1.0];
const COLOR_BLUE: [f32; 4] = [0.2, 0.66, 1.0, 1.0];
// const COLOR_RED: [f32; 4] = [0.9, 0.3, 0.2, 1.0];
