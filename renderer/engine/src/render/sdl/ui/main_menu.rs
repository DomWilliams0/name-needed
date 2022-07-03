use crate::render::sdl::ui::{EventConsumed, Ui};
use imgui::{Condition, ListBox, Ui as ImguiUi};
use sdl2::event::Event;
use sdl2::mouse::MouseState;
use sdl2::video::Window;
use simulation::{MainMenuAction, MainMenuConfig, Scenario};
use std::borrow::Cow;

pub struct MainMenu<'ui> {
    ui: &'ui mut Ui,
    scenarios: &'ui [Scenario],
    scenario_idx: usize,
}

impl<'ui> MainMenu<'ui> {
    pub fn new(ui: &'ui mut Ui, scenarios: &'ui [Scenario]) -> Self {
        MainMenu {
            ui,
            scenarios,
            scenario_idx: 0,
        }
    }

    pub fn handle_event(&mut self, event: &Event) -> EventConsumed {
        self.ui.handle_event(event)
    }

    pub fn render_main_menu(
        &mut self,
        window: &Window,
        mouse_state: &MouseState,
        _config: &mut MainMenuConfig,
    ) -> Option<MainMenuAction> {
        self.ui
            .imgui_sdl2
            .prepare_frame(self.ui.imgui.io_mut(), window, mouse_state);
        let ui = self.ui.imgui.frame();
        let screen_size = ui.io().display_size;

        let mut action = None;
        imgui::Window::new("Main menu")
            .size([600.0, 400.0], Condition::Always)
            .position(
                [screen_size[0] / 2.0, screen_size[1] / 2.0],
                Condition::Always,
            )
            .position_pivot([0.5, 0.5])
            .movable(false)
            .resizable(false)
            .title_bar(false)
            .collapsible(false)
            .always_use_window_padding(true)
            .build(&ui, || {
                ui.text("Choose a scenario:");
                ListBox::new("##scenarios").size([200.0, 0.0]).build_simple(
                    &ui,
                    &mut self.scenario_idx,
                    self.scenarios,
                    &|s| Cow::Borrowed(s.name),
                );
                ui.same_line();
                ui.text_wrapped(&self.scenarios[self.scenario_idx].desc);

                if ui.button("Play") {
                    action = Some(MainMenuAction::PlayScenario(Some(
                        self.scenarios[self.scenario_idx].id,
                    )));
                }

                ui.same_line();

                if ui.button("Quit") {
                    action = Some(MainMenuAction::Exit);
                }
            });

        self.ui.imgui_sdl2.prepare_render(&ui, window);
        self.ui.renderer.render(ui);

        action
    }
}
