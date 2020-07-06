use crate::render::sdl::ui::memory::PerFrameStrings;
use crate::render::sdl::ui::windows::{
    DebugWindow, PerformanceWindow, SelectionWindow, SocietyWindow, UiBundle,
};
use imgui::{im_str, Condition, Context, FontConfig, FontSource, Style};
use imgui_opengl_renderer::Renderer;
use imgui_sdl2::ImguiSdl2;
use sdl2::event::Event;
use sdl2::mouse::MouseState;
use sdl2::video::Window;
use sdl2::VideoSubsystem;
use simulation::input::{InputCommand, UiBlackboard};
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
    society: SocietyWindow,
    debug: DebugWindow,
}

impl Ui {
    pub fn new(window: &Window, video: &VideoSubsystem) -> Self {
        let mut imgui = Context::create();

        // load settings
        if let Ok(settings) = std::fs::read_to_string(imgui.ini_filename().unwrap()) {
            imgui.load_ini_settings(&settings);
        }

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
            selection: SelectionWindow::default(),
            society: SocietyWindow,
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
        blackboard: UiBlackboard,
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

        imgui::Window::new(im_str!("Debug"))
            .always_auto_resize(true)
            .position([10.0, 112.0], Condition::FirstUseEver)
            .always_use_window_padding(true)
            .build(bundle.ui, || {
                self.selection.render(&mut bundle);
                self.society.render(&mut bundle);
                self.debug.render(&mut bundle);
            });
    }
}
