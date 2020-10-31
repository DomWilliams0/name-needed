use imgui::{im_str, TabItem};

use simulation::input::{SelectedEntityDetails, UiCommand};
use simulation::job::SocietyCommand;
use simulation::{AssociatedBlockData, ComponentWorld, NameComponent};

use crate::render::sdl::ui::windows::{UiBundle, UiExt, Value, COLOR_BLUE, COLOR_RED};
use crate::ui_str;

pub struct SocietyWindow;

impl SocietyWindow {
    pub fn render(&mut self, bundle: &mut UiBundle) {
        let ui = bundle.ui;

        TabItem::new(im_str!("Society")).build(ui, || {
            let society_handle = match bundle.blackboard.player_society.0 {
                None => {
                    ui.text_disabled("You don't control a society");
                    return;
                }
                Some(s) => s,
            };

            let society = bundle
                .blackboard
                .societies
                .society_by_handle(society_handle);
            ui.key_value(
                im_str!("Society:"),
                || {
                    if let Some(society) = society {
                        Value::Some(ui_str!(in bundle.strings, "{}", society.name()))
                    } else {
                        Value::Some(im_str!("Invalid handle"))
                    }
                },
                Some(ui_str!(in bundle.strings, "{:?}", society_handle)),
                if society.is_none() {
                    COLOR_RED
                } else {
                    COLOR_BLUE
                },
            );

            if let Some(range) = bundle.blackboard.selected_tiles.range() {
                if ui.button(im_str!("Break blocks"), [0.0, 0.0]) {
                    bundle.commands.push(UiCommand::IssueSocietyCommand(
                        society_handle,
                        SocietyCommand::BreakBlocks(range),
                    ));
                }
            }

            if let Some((SelectedEntityDetails { entity, .. }, target)) = bundle
                .blackboard
                .selected_entity
                .as_ref()
                .zip(bundle.blackboard.selected_tiles.single_tile())
            {
                // ensure entity is haulable
                if bundle
                    .blackboard
                    .world
                    .has_component_by_name("haulable", *entity)
                {
                    let name = bundle
                        .blackboard
                        .world
                        .component::<NameComponent>(*entity)
                        .map(|n| n.0.as_str())
                        .unwrap_or("thing");

                    if ui.button(
                        ui_str!(in bundle.strings, "Haul {} to {}", name, target),
                        [0.0, 0.0],
                    ) {
                        // hopefully this gets the accessible air above the block
                        let target = target.above();

                        bundle.commands.push(UiCommand::IssueSocietyCommand(
                            society_handle,
                            SocietyCommand::HaulToPosition(*entity, target),
                        ));
                    }

                    // if target is a container, allow hauling into it too
                    let w = bundle.blackboard.world.voxel_world();
                    let w = w.borrow();
                    if let Some(AssociatedBlockData::Container(container)) =
                        w.associated_block_data(target)
                    {
                        let container_name = bundle
                            .blackboard
                            .world
                            .component::<NameComponent>(*container)
                            .map(|n| n.0.as_str())
                            .unwrap_or("container");

                        if ui.button(
                            ui_str!(in bundle.strings, "Haul {} into {}", name, container_name),
                            [0.0, 0.0],
                        ) {
                            bundle.commands.push(UiCommand::IssueSocietyCommand(
                                society_handle,
                                SocietyCommand::HaulIntoContainer(*entity, *container),
                            ));
                        }
                    }
                }
            }
        });
    }
}
