use crate::render::sdl::ui::memory::PerFrameStrings;
use crate::render::sdl::ui::windows::{DebugWindow, PerformanceWindow, SelectionWindow, UiBundle};
use imgui::{Context, FontConfig, FontSource, Style};
use imgui_opengl_renderer::Renderer;
use imgui_sdl2::ImguiSdl2;
use sdl2::event::Event;
use sdl2::mouse::MouseState;
use sdl2::video::Window;
use sdl2::VideoSubsystem;
use simulation::input::{Blackboard, InputCommand};
use simulation::PerfAvg;

pub struct Ui {
    imgui: Context,
    imgui_sdl2: ImguiSdl2,
    renderer: Renderer,

    state: State,
    strings_arena: PerFrameStrings,
}

pub enum EventConsumed {
    Consumed,
    NotConsumed,
}

/// Holds window state, but there may not actually be any
pub struct State {
    perf: PerformanceWindow,
    selection: SelectionWindow,
    debug: DebugWindow,
}

impl Ui {
    pub fn new(window: &Window, video: &VideoSubsystem) -> Self {
        let mut imgui = Context::create();
        imgui.set_ini_filename(None);
        Style::use_dark_colors(imgui.style_mut());
        imgui.fonts().add_font(&[FontSource::DefaultFontData {
            config: Some(FontConfig {
                size_pixels: 20.0,
                ..Default::default()
            }),
        }]);

        let imgui_sdl2 = ImguiSdl2::new(&mut imgui, window);
        let renderer = Renderer::new(&mut imgui, |s| video.gl_get_proc_address(s) as _);
        let state = State {
            perf: PerformanceWindow,
            selection: SelectionWindow,
            debug: DebugWindow,
        };

        Self {
            imgui,
            imgui_sdl2,
            renderer,
            state,
            strings_arena: PerFrameStrings::new(),
        }
    }

    pub fn handle_event(&mut self, event: &Event) -> EventConsumed {
        self.imgui_sdl2.handle_event(&mut self.imgui, event);
        if self.imgui_sdl2.ignore_event(event) {
            EventConsumed::Consumed
        } else {
            EventConsumed::NotConsumed
        }
    }

    pub fn render(
        &mut self,
        window: &Window,
        mouse_state: &MouseState,
        perf: &PerfAvg,
        blackboard: Blackboard,
        input_commands: &mut Vec<InputCommand>,
    ) {
        self.imgui_sdl2
            .prepare_frame(self.imgui.io_mut(), window, mouse_state);
        let ui = self.imgui.frame();

        let bundle = UiBundle {
            ui: &ui,
            strings: &self.strings_arena,
            perf,
            blackboard: &blackboard,
            commands: input_commands,
        };

        self.state.render(bundle);

        self.imgui_sdl2.prepare_render(&ui, window);
        self.renderer.render(ui);

        self.strings_arena.reset();
    }
}

impl State {
    fn render(&mut self, mut bundle: UiBundle) {
        self.perf.render(&bundle);
        self.selection.render(&mut bundle);
        self.debug.render(&mut bundle);
    }
}
