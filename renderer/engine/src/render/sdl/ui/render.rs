use imgui::{im_str, Condition, Context, FontConfig, FontSource, Style};
use imgui_opengl_renderer::Renderer;
use imgui_sdl2::ImguiSdl2;
use sdl2::event::Event;
use sdl2::mouse::MouseState;
use sdl2::video::Window;
use sdl2::VideoSubsystem;

use simulation::input::UiCommands;
use simulation::{PerfAvg, SimulationRef};

use crate::render::sdl::ui::context::UiContext;
use crate::render::sdl::ui::memory::PerFrameStrings;
use crate::render::sdl::ui::windows::{DebugWindow, PerformanceWindow, SelectionWindow};

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

#[derive(Default)]
struct State {
    max_window_width: f32,
    perf: PerformanceWindow,
    selection: SelectionWindow,
    // society: SocietyWindow,
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
        let state = State::default();

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

    /// Prepares imgui frame and calls [State::render]
    pub fn render(
        &mut self,
        window: &Window,
        mouse_state: &MouseState,
        perf: PerfAvg,
        simulation: SimulationRef,
        commands: &mut UiCommands,
    ) {
        self.imgui_sdl2
            .prepare_frame(self.imgui.io_mut(), window, mouse_state);
        let ui = self.imgui.frame();

        // generate windows
        let context = UiContext::new(&ui, &self.strings_arena, simulation, commands, perf);
        self.state.render(context);

        // render windows
        self.imgui_sdl2.prepare_render(&ui, window);
        self.renderer.render(ui);

        // cleanup
        self.strings_arena.reset();
    }
}

impl State {
    /// Renders ui windows
    fn render(&mut self, context: UiContext) {
        imgui::Window::new(im_str!("Debug"))
            .size([400.0, 500.0], Condition::FirstUseEver)
            .position([10.0, 10.0], Condition::FirstUseEver)
            .title_bar(false)
            .always_use_window_padding(true)
            .resizable(true)
            .build(context.ui(), || {
                // perf fixed at the top
                self.perf.render(&context);

                let tabbar = context.new_tab_bar(im_str!("Debug Tabs"));
                if !tabbar.is_open() {
                    return;
                }

                self.selection.render(&context);
                // self.society.render(&mut context);
                self.debug.render(&context);
            });

        // ensure window doesn't resize itself all the time
        let window_size = context.window_content_region_width();
        self.max_window_width = self.max_window_width.max(window_size);
    }
}
