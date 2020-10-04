use crate::render::sdl::ui::memory::PerFrameStrings;
use crate::render::sdl::ui::windows::{
    DebugWindow, PerformanceWindow, SelectionWindow, SocietyWindow, UiBundle,
};
use imgui::{im_str, Condition, Context, FontConfig, FontSource, Style, TabBar};
use imgui_opengl_renderer::Renderer;
use imgui_sdl2::ImguiSdl2;
use sdl2::event::Event;
use sdl2::mouse::MouseState;
use sdl2::video::Window;
use sdl2::VideoSubsystem;
use simulation::input::{UiBlackboard, UiCommand};
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
    max_window_width: f32,
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
            max_window_width: 0.0,
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
        commands: &mut Vec<UiCommand>,
    ) {
        self.imgui_sdl2
            .prepare_frame(self.imgui.io_mut(), window, mouse_state);
        let ui = self.imgui.frame();

        let bundle = UiBundle {
            ui: &ui,
            strings: &self.strings_arena,
            perf,
            blackboard: &blackboard,
            commands,
        };

        self.state.render(bundle);

        self.imgui_sdl2.prepare_render(&ui, window);
        self.renderer.render(ui);

        self.strings_arena.reset();
    }
}

impl State {
    fn render(&mut self, mut bundle: UiBundle) {
        let window = imgui::Window::new(im_str!("Debug"))
            .size([self.max_window_width, 0.0], Condition::Always)
            .position([10.0, 10.0], Condition::FirstUseEver)
            .title_bar(false)
            .always_use_window_padding(true);

        if let Some(token) = window.begin(bundle.ui) {
            // Perf fixed at the top
            self.perf.render(&bundle);

            TabBar::new(im_str!("Debug Tabs")).build(bundle.ui, || {
                self.selection.render(&mut bundle);
                self.society.render(&mut bundle);
                self.debug.render(&mut bundle);
            });

            token.end(bundle.ui);

            let window_size = bundle.ui.window_content_region_width();
            self.max_window_width = self.max_window_width.max(window_size);
        }
    }
}
