use imgui::{im_str, StyleColor};

use simulation::input::{SelectedEntity, SelectedTiles, UiRequest};
use simulation::{AssociatedBlockData, ComponentWorld, PlayerSociety, Societies, SocietyHandle};

use crate::render::sdl::ui::context::{DefaultOpen, UiContext};
use crate::render::sdl::ui::windows::{UiExt, COLOR_BLUE};
use crate::ui_str;
use serde::{Deserialize, Serialize};
use simulation::job::SocietyCommand;

#[derive(Default, Serialize, Deserialize)]
pub struct SocietyWindow;

impl SocietyWindow {
    pub fn render(&mut self, context: &UiContext) {
        let tab = context.new_tab(im_str!("Society"));
        if !tab.is_open() {
            return;
        }

        let ecs = context.simulation().ecs;
        let society_handle = match ecs.resource::<PlayerSociety>().0 {
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

    fn do_control(&self, context: &UiContext, society_handle: SocietyHandle) {
        let tab = context.new_tab(im_str!("Control"));
        if tab.is_open() {
            let mut any_buttons = false;

            let ecs = context.simulation().ecs;
            let block_selection = ecs.resource::<SelectedTiles>();

            // break selected blocks
            if let Some(range) = block_selection.range() {
                any_buttons = true;
                if context.button(im_str!("Break blocks"), [0.0, 0.0]) {
                    context.issue_request(UiRequest::IssueSocietyCommand(
                        society_handle,
                        SocietyCommand::BreakBlocks(range),
                    ));
                }
            }

            // entity selection and block selection
            if let Some((entity, target)) = ecs
                .resource::<SelectedEntity>()
                .get_unchecked()
                .zip(block_selection.single_tile())
            {
                if ecs.is_entity_alive(entity) && ecs.has_component_by_name("haulable", entity) {
                    let name = ecs.name(entity);
                    any_buttons = true;
                    if context.button(
                        ui_str!(in context, "Haul {} to {}", name, target),
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
                        let container_name = ecs.name_or_default(*container, &"container");
                        if context.button(
                            ui_str!(in context, "Haul {} into {}", name, container_name),
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
