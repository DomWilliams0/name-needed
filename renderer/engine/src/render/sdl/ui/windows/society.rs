use simulation::{ComponentWorld, PlayerSociety, Societies, SocietyHandle};

use crate::render::sdl::ui::context::{DefaultOpen, UiContext};
use crate::render::sdl::ui::windows::{UiExt, COLOR_BLUE};
use crate::{open_or_ret, ui_str};

use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
pub struct SocietyWindow {
    build_selection: usize,
}

impl SocietyWindow {
    pub fn render(&mut self, context: &UiContext) {
        let _tab = open_or_ret!(context.new_tab("Society"));

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
            "Society:",
            || {
                society
                    .map(|s| ui_str!(in context, "{}", s.name()))
                    .unwrap_or("Error: invalid handle")
            },
            Some(ui_str!(in context, "{:?}", society_handle)),
            COLOR_BLUE,
        );

        let _tab_bar = open_or_ret!(context.new_tab_bar("##societytabbar"));

        self.do_jobs(context, society_handle);
    }

    fn do_jobs(&self, context: &UiContext, society_handle: SocietyHandle) {
        let tab = context.new_tab("Jobs");
        if tab.is_some() {
            let societies = context.simulation().ecs.resource::<Societies>();
            let society = match societies.society_by_handle(society_handle) {
                None => {
                    return context.text_disabled("Invalid society");
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

                    if node.is_some() {
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
