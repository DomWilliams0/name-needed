use imgui::{im_str, Condition, Context, FontConfig, FontSource, Style};
use imgui_opengl_renderer::Renderer;
use imgui_sdl2::ImguiSdl2;
use sdl2::event::Event;
use sdl2::mouse::MouseState;
use sdl2::video::Window;
use sdl2::VideoSubsystem;
use serde::{Deserialize, Serialize};

use simulation::input::{UiCommand, UiCommands, UiRequest};
use simulation::{PerfAvg, SimulationRef};

use crate::render::sdl::ui::context::UiContext;
use crate::render::sdl::ui::memory::PerFrameStrings;
use crate::render::sdl::ui::windows::{
    DebugWindow, PerformanceWindow, SelectionWindow, SocietyWindow,
};
use common::BoxedResult;

use std::io::{ErrorKind, Read, Write};
use std::path::Path;

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

/// Persisted across restarts
#[derive(Default, Serialize, Deserialize)]
struct State {
    perf: PerformanceWindow,
    selection: SelectionWindow,
    society: SocietyWindow,
    debug: DebugWindow,
}

impl Ui {
    /// Called once during initialization of persistent backend
    pub fn new(window: &Window, video: &VideoSubsystem, serialized_path: &Path) -> Self {
        let mut imgui = Context::create();

        // deserialize state and imgui settings
        imgui.set_ini_filename(None); // serialized inline
        let state = match Self::load_state(&mut imgui, serialized_path) {
            Ok(Some(state)) => state,
            Ok(None) => State::default(), // not an error
            Err(err) => {
                common::warn!(
                    "failed to load ui state from {}: {}",
                    serialized_path.display(),
                    err
                );

                State::default()
            }
        };

        Style::use_dark_colors(imgui.style_mut());
        imgui.fonts().add_font(&[FontSource::DefaultFontData {
            config: Some(FontConfig {
                size_pixels: 20.0,
                ..Default::default()
            }),
        }]);

        let imgui_sdl2 = ImguiSdl2::new(&mut imgui, window);
        let renderer = Renderer::new(&mut imgui, |s| video.gl_get_proc_address(s) as _);

        Self {
            imgui,
            imgui_sdl2,
            renderer,
            state,
            strings_arena: PerFrameStrings::new(),
        }
    }

    /// Called each time the game (re)starts
    pub fn on_start(&mut self, commands: &mut UiCommands) {
        // instruct game to enable debug renderers
        let debug_renderers = self.state.debug.enabled_debug_renderers();
        commands.reserve(debug_renderers.len() + 1);

        commands.push(UiCommand::new(UiRequest::DisableAllDebugRenderers));
        commands.extend(debug_renderers.map(|ident| {
            UiCommand::new(UiRequest::SetDebugRendererEnabled {
                ident: ident.clone(),
                enabled: true,
            })
        }));
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

    /// Persist to file. Any returned error is not treated as fatal
    pub fn on_exit(&mut self, path: &Path) -> BoxedResult<()> {
        if config::get().display.persist_ui {
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .open(path)?;

            self.serialize_to(file)?;
        }

        Ok(())
    }

    /// Ok(None): no save file found
    fn load_state(imgui_ctx: &mut Context, path: &Path) -> BoxedResult<Option<State>> {
        if !config::get().display.persist_ui {
            return Ok(None);
        }

        let file = match std::fs::File::open(path) {
            Err(e) if e.kind() == ErrorKind::NotFound => {
                // not an error
                return Ok(None);
            }
            Err(err) => return Err(err.into()),
            Ok(f) => f,
        };

        let state = Self::deserialize_from(imgui_ctx, file)?;
        Ok(Some(state))
    }

    fn serialize_to(&mut self, writer: impl Write) -> Result<(), ron::Error> {
        #[derive(Serialize)]
        #[repr(C)]
        struct SerializedState<'a> {
            state: &'a State,
            imgui: &'a str,
        }

        let mut imgui = String::new();
        self.imgui.save_ini_settings(&mut imgui);

        let serialized = SerializedState {
            state: &self.state,
            imgui: &imgui,
        };

        ron::ser::to_writer(writer, &serialized)
    }

    fn deserialize_from(imgui_ctx: &mut Context, reader: impl Read) -> Result<State, ron::Error> {
        #[derive(Deserialize)]
        #[repr(C)]
        struct SerializedState {
            state: State,
            imgui: String,
        }

        let SerializedState { state, imgui } = ron::de::from_reader(reader)?;

        imgui_ctx.load_ini_settings(&imgui);
        Ok(state)
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
                self.society.render(&context);
                self.debug.render(&context);
            });
    }
}
