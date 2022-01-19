use imgui::{im_str, ChildWindow, Selectable, StyleColor};
use std::fmt::Display;

use simulation::input::{SelectedEntity, SelectedTiles, UiRequest};
use simulation::{AssociatedBlockData, ComponentWorld, PlayerSociety, Societies, SocietyHandle};

use crate::render::sdl::ui::context::{DefaultOpen, UiContext};
use crate::render::sdl::ui::windows::{UiExt, COLOR_BLUE};
use crate::ui_str;

use serde::{Deserialize, Serialize};
use simulation::job::SocietyCommand;

#[derive(Default, Serialize, Deserialize)]
pub struct SocietyWindow {
    build_selection: usize,
}

impl SocietyWindow {
    pub fn render(&mut self, context: &UiContext) {
        let tab = context.new_tab(im_str!("Society"));
        if !tab.is_open() {
            return;
        }

        let ecs = context.simulation().ecs;
        let society_handle = match ecs.resource::<PlayerSociety>().get() {
            None => {
                context.text_disabled("You don't control a society");
                return;
            }
            Some(h) => h,
        };

        let societies = ecs.resource::<Societies>();

        let society = societies.society_by_handle(society_handle);
        context.key_value(
            im_str!("Society:"),
            || {
                society
                    .map(|s| ui_str!(in context, "{}", s.name()))
                    .unwrap_or(im_str!("Error: invalid handle"))
            },
            Some(ui_str!(in context, "{:?}", society_handle)),
            COLOR_BLUE,
        );

        let tabbar = context.new_tab_bar(im_str!("##societytabbar"));
        if !tabbar.is_open() {
            return;
        }

        self.do_control(context, society_handle);
        self.do_jobs(context, society_handle);
    }

    fn do_control(&mut self, context: &UiContext, society_handle: SocietyHandle) {
        let tab = context.new_tab(im_str!("Control"));
        if tab.is_open() {
            let mut any_buttons = false;

            let ecs = context.simulation().ecs;
            let block_selection = ecs.resource::<SelectedTiles>();
            let block_selection = block_selection.current_selected();

            // break selected blocks
            if let Some(sel) = block_selection {
                any_buttons = true;
                if context.button(
                    ui_str!(in context, "Break {} blocks", sel.range().count()),
                    [0.0, 0.0],
                ) {
                    context.issue_request(UiRequest::IssueSocietyCommand(
                        society_handle,
                        SocietyCommand::BreakBlocks(sel.range().clone()),
                    ));
                }

                if context.button(im_str!("Build"), [0.0, 0.0]) {
                    // TODO handle failure better?
                    if let Some(above) = sel.range().above() {
                        if let Some((_, template, _)) =
                            ecs.build_templates().get(self.build_selection)
                        {
                            context.issue_request(UiRequest::IssueSocietyCommand(
                                society_handle,
                                SocietyCommand::Build(above, template.clone()),
                            ));
                        }
                    }
                }

                context.same_line_with_spacing(0.0, 40.0);

                ChildWindow::new("##buildsocietyblocks")
                    .size([0.0, 50.0])
                    .horizontal_scrollbar(true)
                    .movable(false)
                    .build(context.ui(), || {
                        let builds = ecs.build_templates();
                        for (i, (id, _, name)) in builds.iter().enumerate() {
                            let name = match name {
                                Some(s) => s as &dyn Display,
                                None => id as &dyn Display,
                            };
                            if Selectable::new(ui_str!(in context, "{}", name))
                                .selected(self.build_selection == i)
                                .build(context)
                            {
                                self.build_selection = i;
                            }
                        }
                    });
            }

            // entity selection and block selection
            if let Some((entity, target)) = ecs
                .resource::<SelectedEntity>()
                .get_unchecked()
                .zip(block_selection.and_then(|sel| sel.single_tile()))
            {
                if ecs.is_entity_alive(entity) && ecs.has_component_by_name("haulable", entity) {
                    let desc = context.description(entity);
                    any_buttons = true;
                    if context.button(
                        ui_str!(in context, "Haul {} to {}", desc, target),
                        [0.0, 0.0],
                    ) {
                        // hopefully this gets the accessible air above the block
                        let target = target.above();

                        context.issue_request(UiRequest::IssueSocietyCommand(
                            society_handle,
                            SocietyCommand::HaulToPosition(entity, target.centred()),
                        ));
                    }

                    // if target is a container, allow hauling into it too
                    let w = context.simulation().world.borrow();

                    let block_data = w.associated_block_data(target);
                    if let Some(AssociatedBlockData::Container(container)) = block_data {
                        let container_name =
                            context.description(*container).with_fallback(&"container");
                        if context.button(
                            ui_str!(in context, "Haul {} into {}", desc, container_name),
                            [0.0, 0.0],
                        ) {
                            context.issue_request(UiRequest::IssueSocietyCommand(
                                society_handle,
                                SocietyCommand::HaulIntoContainer(entity, *container),
                            ));
                        }
                    }
                }
            }

            if !any_buttons {
                let color = context.style_color(StyleColor::TextDisabled);
                let style = context.push_style_color(StyleColor::Text, color);
                context.text_wrapped(im_str!(
                    "Try selecting an entity, a container and/or some blocks"
                ));
                style.pop(context);
            }
        }
    }

    fn do_jobs(&self, context: &UiContext, society_handle: SocietyHandle) {
        let tab = context.new_tab(im_str!("Jobs"));
        if tab.is_open() {
            let societies = context.simulation().ecs.resource::<Societies>();
            let society = match societies.society_by_handle(society_handle) {
                None => {
                    context.text_disabled("Invalid society");
                    return;
                }
                Some(s) => s,
            };

            // TODO preserve finished jobs and tasks for a bit and display them in the ui too
            let jobs = society.jobs();
            let mut job_node = None;
            jobs.iter_all_filtered(
                |job| {
                    // TODO use table API when available
                    // close previous node first
                    job_node = None;

                    let node =
                        context.new_tree_node(ui_str!(in context, "{}", job), DefaultOpen::Closed);

                    if node.is_open() {
                        job_node = Some(node);
                        true
                    } else {
                        false
                    }
                },
                |task, reservers| {
                    context.text_wrapped(ui_str!(in context, " - {}", task));
                    for reserver in reservers.iter() {
                        context.text_colored(
                            COLOR_BLUE,
                            ui_str!(in context, "  * Reserved by {}", *reserver),
                        );
                    }
                },
            );
        }
    }
}
