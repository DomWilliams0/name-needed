use imgui::{ImStr, ImString, Ui};

mod debug_renderer;
mod perf;
// mod selection;
// mod society;

pub(crate) use debug_renderer::DebugWindow;
pub(crate) use perf::PerformanceWindow;
// pub(crate) use selection::SelectionWindow;
// pub(crate) use society::SocietyWindow;

enum Value<'a> {
    Hide,
    None(&'static str),
    Some(&'a ImStr),
    Wrapped(&'a ImStr),
    MultilineReadonly {
        label: &'a ImStr,
        buffer: &'a ImStr,
        width: f32,
    },
}

trait UiExt {
    fn key_value<'a, F: FnOnce() -> Value<'a>>(
        &'a self,
        key: &ImStr,
        value: F,
        tooltip: Option<&ImStr>,
        color: [f32; 4],
    );
}

impl UiExt for Ui<'_> {
    fn key_value<'a, F: FnOnce() -> Value<'a>>(
        &'a self,
        key: &ImStr,
        value: F,
        tooltip: Option<&ImStr>,
        color: [f32; 4],
    ) {
        let value = value();
        if let Value::Hide = value {
            return;
        }

        self.text_colored(color, key);

        if !matches!(value, Value::MultilineReadonly {..}) {
            self.same_line_with_spacing(self.calc_text_size(key, false, 0.0)[0], 10.0);
        }

        match value {
            Value::Some(val) => {
                self.text(val);
            }
            Value::Wrapped(val) => {
                self.text_wrapped(&val);
            }
            Value::None(val) => self.text_disabled(val),
            Value::MultilineReadonly {
                label,
                buffer,
                width,
            } => {
                let buffer = buffer.to_str();
                // safety: faking a mutable ImString from this immutable ImStr in a READONLY
                // multiline. fake allocations are forgotten
                let mut buf = unsafe {
                    // fake an owned string around the immutable buffer
                    let mut string = String::from_raw_parts(
                        buffer.as_ptr() as *mut _,
                        buffer.len(),
                        buffer.len(),
                    );

                    // fake vec reference to inner vec
                    let vec_ref = string.as_mut_vec();

                    // fake owned inner vec
                    let vec_copy: Vec<u8> = std::mem::transmute_copy(vec_ref);

                    // forget fake owned string
                    std::mem::forget(string);

                    // fake ImString thinks it owns its vec
                    ImString::from_utf8_with_nul_unchecked(vec_copy)
                };

                let _ = self
                    .input_text_multiline(label, &mut buf, [width, 0.0])
                    .read_only(true)
                    .build();

                // forget fake owned string
                std::mem::forget(buf);
            }
            Value::Hide => unreachable!(),
        };

        if let Some(tooltip) = tooltip {
            if self.is_item_hovered() {
                self.tooltip_text(tooltip);
            }
        }
    }
}

const COLOR_GREEN: [f32; 4] = [0.4, 0.77, 0.33, 1.0];
const COLOR_ORANGE: [f32; 4] = [1.0, 0.46, 0.2, 1.0];
const COLOR_BLUE: [f32; 4] = [0.2, 0.66, 1.0, 1.0];
const COLOR_RED: [f32; 4] = [0.9, 0.3, 0.2, 1.0];
