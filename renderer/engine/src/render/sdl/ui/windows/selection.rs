use std::cell::RefCell;
use std::fmt::Write;

use imgui::{ChildWindow, Selectable, StyleColor};
use serde::{Deserialize, Serialize};

use common::*;
use simulation::input::{
    BlockPlacement, SelectedEntities, SelectedTiles, SelectionModification, SelectionProgress,
    UiRequest,
};
use simulation::job::BuildThingJob;
use simulation::{
    ActivityComponent, AssociatedBlockData, AssociatedBlockDataType, BlockType, ComponentRef,
    ComponentWorld, ConditionComponent, Container, ContainerComponent, EdibleItemComponent, Entity,
    EntityLoggingComponent, FollowPathComponent, HungerComponent, IntoEnumIterator,
    InventoryComponent, ItemStackComponent, NameComponent, PhysicalComponent, Societies,
    SocietyComponent, SpeciesComponent, TransformComponent, UiElementComponent,
};

use crate::render::sdl::ui::context::{DefaultOpen, EntityDesc, UiContext};
use crate::render::sdl::ui::windows::{UiExt, Value, COLOR_BLUE, COLOR_GREEN, COLOR_ORANGE};
use crate::{open_or_ret, ui_str};

#[derive(Default, Serialize, Deserialize)]
pub struct SelectionWindow {
    /// Index into [BlockType::into_enum_iter]
    edit_selection: usize,
}

struct SelectedEntityDetails<'a> {
    entity: Entity,
    transform: Option<ComponentRef<'a, TransformComponent>>,
    physical: Option<ComponentRef<'a, PhysicalComponent>>,
    ty: EntityType<'a>,
}

enum EntityType<'a> {
    Living,
    Item(ComponentRef<'a, ConditionComponent>),
    UiElement(ComponentRef<'a, UiElementComponent>),
}

#[derive(Debug)]
enum EntityState {
    Alive,
    Dead,
    Inanimate,
    UiElement,
}

/// Oneshot
struct CommaSeparatedDebugIter<I: Iterator<Item = T>, T: Debug>(RefCell<I>);

impl SelectionWindow {
    pub fn render(&mut self, context: &UiContext) {
        self.entity_selection(context);
        self.world_selection(context);
    }

    fn entity_selection(&mut self, context: &UiContext) {
        let _tab = open_or_ret!(context.new_tab("Entity"));

        let ecs = context.simulation().ecs;

        let entity_sel = ecs.resource::<SelectedEntities>();
        let (e, details, state) = match entity_sel.just_one() {
            Some(e) if ecs.is_entity_alive(e) => {
                let transform = ecs.component(e).ok();
                let physical = ecs.component(e).ok();
                let (ty, state) = match (
                    ecs.component::<ConditionComponent>(e),
                    ecs.component::<UiElementComponent>(e),
                ) {
                    (_, Ok(ui)) => (EntityType::UiElement(ui), EntityState::UiElement),
                    (Ok(condition), _) => (EntityType::Item(condition), EntityState::Inanimate),
                    _ => (EntityType::Living, EntityState::Alive),
                };

                (
                    e,
                    Some(SelectedEntityDetails {
                        entity: e,
                        transform,
                        physical,
                        ty,
                    }),
                    state,
                )
            }
            Some(e) => {
                // entity is dead
                (e, None, EntityState::Dead)
            }
            None if entity_sel.count() > 1 => {
                // TODO better support for multiple entity selection

                context.key_value(
                    "Entities:",
                    || {
                        Value::Wrapped(ui_str!(in context, "{}",
                            CommaSeparatedDebugIter(RefCell::new(entity_sel.iter().iter()))
                        ))
                    },
                    None,
                    COLOR_GREEN,
                );

                let style = context.push_style_color(
                    StyleColor::Text,
                    context.style_color(StyleColor::TextDisabled),
                );
                context.text_wrapped(ui_str!(in context,
                    "Multiple entity control not yet supported, reduce selection to a single entity",
                ));
                return style.pop();
            }
            None => return context.text_disabled("No entity selected"),
        };

        context.key_value(
            "Entity:",
            || ui_str!(in context, "{}", e),
            None,
            COLOR_GREEN,
        );

        context.key_value(
            "State:",
            || ui_str!(in context, "{:?}", state),
            None,
            COLOR_GREEN,
        );

        // TODO maintain own arena allocator to maintain UI after an entity dies
        let details = match details {
            Some(details) => details,
            None => return,
        };
        debug_assert!(!matches!(state, EntityState::Dead));

        if let Ok(name) = ecs.component::<NameComponent>(details.entity) {
            context.key_value(
                "Name:",
                || ui_str!(in context, "{}", *name),
                None,
                COLOR_GREEN,
            );
        }

        let kind = context.description(details.entity);
        if !matches!(kind, EntityDesc::Fallback(_)) {
            context.key_value(
                "Kind:",
                || ui_str!(in context, "{}", kind),
                None,
                COLOR_GREEN,
            );
        }

        context.key_value(
            "Position:",
            || {
                details
                    .transform
                    .as_ref()
                    .map(|t| ui_str!(in context, "{}", t.position))
                    .ok_or("Unknown")
            },
            None,
            COLOR_GREEN,
        );

        if let Some(physical) = details.physical.as_ref() {
            context.key_value(
                "Size:",
                || ui_str!(in context, "{}", physical.size),
                None,
                COLOR_GREEN,
            );

            context.key_value(
                "Volume:",
                || ui_str!(in context, "{}", physical.volume),
                None,
                COLOR_GREEN,
            );
        }

        // item stack contents
        if let Some(stack) = details.component::<ItemStackComponent>(context) {
            let node = context.new_tree_node("Item stack", DefaultOpen::Closed);
            if node.is_some() {
                self.do_stack(context, &stack);
            }
        }

        let components_node = context.new_tree_node("Components", DefaultOpen::Closed);
        if components_node.is_some() {
            ChildWindow::new("scrolledcomponents")
                .size([0.0, 150.0])
                .build(context, || {
                    // TODO component-specific widget
                    for (name, component) in
                        context.simulation().ecs.all_components_for(details.entity)
                    {
                        let interactive = match component.as_interactive() {
                            None => {
                                // just show name
                                context.text(ui_str!(in context, " - {}", name));
                                continue;
                            }
                            Some(i) => i,
                        };

                        // nice tree node
                        let title = ui_str!(in context, "{}", name);
                        let node = context.new_tree_node(title, DefaultOpen::Closed);
                        if node.is_none() {
                            continue;
                        }

                        context.key_value(
                            "Summary",
                            || {
                                if let Some(dbg) = interactive.as_debug() {
                                    Value::Wrapped(ui_str!(in context, "{:?}", dbg))
                                } else {
                                    Value::None("Not implemented")
                                }
                            },
                            None,
                            COLOR_ORANGE,
                        );
                    }
                });
        }

        let tabbar = context.new_tab_bar("##entitydetailstabbar");
        if tabbar.is_some() {
            match details.ty {
                EntityType::Living => self.do_living(context, &details),
                EntityType::Item(ref condition) => self.do_item(context, &details, condition),
                EntityType::UiElement(ref ui) => self.do_ui_element(context, ui),
            }
        }

        if !matches!(details.ty, EntityType::UiElement(_)) {
            self.do_logs(context, &details);
        }
    }

    //noinspection DuplicatedCode
    fn do_living(&mut self, context: &UiContext, details: &SelectedEntityDetails) {
        let ecs = context.simulation().ecs;

        {
            let tab = context.new_tab("Living");
            if tab.is_some() {
                context.key_value(
                    "Species:",
                    || {
                        details
                            .component::<SpeciesComponent>(context)
                            .map(|s| ui_str!(in context, "{}", s))
                    },
                    None,
                    COLOR_ORANGE,
                );
                context.key_value(
                    "Velocity:",
                    || {
                        details
                            .transform
                            .as_ref()
                            .map(|t| ui_str!(in context, "{:.2}m/s", t.velocity.magnitude() ))
                    },
                    None,
                    COLOR_ORANGE,
                );

                context.key_value(
                    "Satiety:",
                    || {
                        details.component::<HungerComponent>(context).map(|h| {
                            let (current, max) = h.satiety();
                            ui_str!(in context, "{}/{}", current, max)
                        })
                    },
                    None,
                    COLOR_ORANGE,
                );

                context.key_value(
                    "Navigating to:",
                    || {
                        details
                            .component::<FollowPathComponent>(context)
                            .and_then(|comp| comp.target())
                            .map(|tgt| ui_str!(in context, "{}", tgt))
                            .ok_or("Nowhere")
                    },
                    None,
                    COLOR_ORANGE,
                );

                let societies = ecs.resource::<Societies>();
                let society = details.component::<SocietyComponent>(context);
                context.key_value(
                    "Society:",
                    || {
                        society
                            .as_ref()
                            .map(|comp| {
                                let name = societies
                                    .society_by_handle(comp.handle())
                                    .map(|s| s.name())
                                    .unwrap_or("Invalid handle");
                                ui_str!(in context, "{}", name)
                            })
                            .ok_or("None")
                    },
                    society
                        .as_ref()
                        .map(|comp| ui_str!(in context, "{:?}", comp.handle())),
                    COLOR_ORANGE,
                );
            }
        }

        // inventory
        if let Some(inv) = details.component::<InventoryComponent>(context) {
            let tab = context.new_tab("Inventory");
            if tab.is_some() {
                self.do_inventory(context, &*inv);
            }
        }

        // activity
        if let Some(activity) = details.component::<ActivityComponent>(context) {
            let tab = context.new_tab("Activity");
            if tab.is_some() {
                self.do_activity(context, &*activity);
            }
        }
    }

    fn do_item(
        &mut self,
        context: &UiContext,
        details: &SelectedEntityDetails,
        condition: &ConditionComponent,
    ) {
        // TODO list components on item that are relevant (i.e. not transform etc)

        {
            let tab = context.new_tab("Item");
            if tab.is_some() {
                context.key_value(
                    "Condition:",
                    || ui_str!(in context, "{}", condition.0),
                    None,
                    COLOR_ORANGE,
                );
            }
        }

        if let Some(edible) = details.component::<EdibleItemComponent>(context) {
            let tab = context.new_tab("Nutrition");
            if tab.is_some() {
                context.key_value(
                    "Nutrition:",
                    || ui_str!(in context, "{}", edible.total_nutrition),
                    None,
                    COLOR_ORANGE,
                );
            }
        }
    }

    fn do_inventory(&mut self, context: &UiContext, inventory: &InventoryComponent) {
        let ecs = context.simulation().ecs;

        context.text_colored(
            COLOR_GREEN,
            ui_str!(in context, "{} hands:", inventory.equip_slots().len()),
        );

        for slot in inventory.equip_slots() {
            context.same_line();
            context.text(ui_str!(in context, "{} ", slot));
        }

        context.separator();
        context.text_disabled(
            ui_str!(in context, "{} containers", inventory.containers_unresolved().len()),
        );

        for (i, (e, container)) in inventory.containers(ecs).enumerate() {
            let tree = context.new_tree_node(
                ui_str!(in context, "#{}: {}##container", i+1, context.description(e)),
                DefaultOpen::Closed,
            );

            if tree.is_some() {
                self.do_container(context, &container);
            }
        }
    }

    fn do_stack(&mut self, context: &UiContext, stack: &ItemStackComponent) {
        let (count, limit) = stack.stack.filled();
        context.text_colored(
            COLOR_GREEN,
            ui_str!(in context, "Capacity {}/{}", count, limit),
        );

        for (entity, count) in stack.stack.contents() {
            context.text_wrapped(
                ui_str!(in context, " - {}x {} ({})", count, context.description(entity), entity),
            );
        }
    }

    fn do_container(&mut self, context: &UiContext, container: &Container) {
        let (max_vol, max_size) = container.limits();
        let capacity = container.current_capacity();
        context.text_colored(
            COLOR_GREEN,
            ui_str!(in context, "Capacity {}/{}, size {}", capacity, max_vol, max_size),
        );

        for entity in container.contents() {
            context.text_wrapped(
                ui_str!(in context, " - {} ({}, vol {})", context.description(entity.entity), entity.entity, entity.volume),
            );
        }
    }

    fn do_activity(&mut self, context: &UiContext, activity: &ActivityComponent) {
        if let Some((activity, status)) = activity.status() {
            context.key_value(
                "Activity:",
                || Value::Wrapped(ui_str!(in context, "{}", activity)),
                None,
                COLOR_ORANGE,
            );

            context.key_value(
                "Status:",
                || Value::Wrapped(ui_str!(in context, "{}", &*status)),
                None,
                COLOR_ORANGE,
            );

            context.separator();

            // TODO society task
            // let reservation = activity.current_society_task();
            // context.key_value(
            //     ("Reserved task:",
            //     || {
            //         if let Some((_, task)) = reservation {
            //             Value::Wrapped(ui_str!(in context, "{}", task))
            //         } else {
            //             Value::None("None")
            //         }
            //     },
            //     None,
            //     COLOR_GREEN,
            // );
            //
            // context.key_value(
            //     ("Job:",
            //     || {
            //         if let Some((job, _)) = reservation {
            //             Value::Wrapped(ui_str!(in context, "{}", job))
            //         } else {
            //             Value::None("None")
            //         }
            //     },
            //     None,
            //     COLOR_GREEN,
            // );
        }
    }

    fn do_logs(&mut self, context: &UiContext, details: &SelectedEntityDetails) {
        let _tab = open_or_ret!(context.new_tab("Logs"));

        // TODO persist logs after entity is dead
        let render_logs = |logs: &EntityLoggingComponent| {
            struct EntityLogs<'a>(&'a EntityLoggingComponent);

            impl<'a> Display for EntityLogs<'a> {
                fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
                    for event in self.0.iter_logs().rev() {
                        writeln!(f, "{}", event)?;
                    }
                    Ok(())
                }
            }
            // TODO switch to table API when available
            let mut backing_str = {
                let mut s = context.entity_log_cached_string_mut();
                let _ = write!(&mut s, "{}", EntityLogs(logs));
                s
            };

            context
                .input_text_multiline(
                    "##entitylogs",
                    &mut backing_str,
                    [context.window_content_region_width(), 0.0],
                )
                .no_horizontal_scroll(false)
                .read_only(true)
                .build();
        };

        let mut req = None;
        match context
            .simulation()
            .ecs
            .component::<EntityLoggingComponent>(details.entity)
        {
            Ok(comp) => {
                if context.button("Disable logs") {
                    req = Some(false);
                } else {
                    render_logs(&*comp);
                }
            }
            _ => {
                if context.button("Enable logs") {
                    req = Some(true);
                }
            }
        };

        if let Some(req) = req {
            context.issue_request(UiRequest::ToggleEntityLogging {
                entity: details.entity,
                enabled: req,
            });
        }
    }

    #[allow(clippy::needless_return)]
    fn do_ui_element(&mut self, context: &UiContext, ui: &UiElementComponent) {
        let _tab = open_or_ret!(context.new_tab("Build"));

        let ecs = context.simulation().ecs;
        let ret = ui
            .build_job
            .resolve_and_cast(ecs.resource(), move |build: &BuildThingJob| {
                let deets = build.details();
                let progress = build.progress();

                // TODO use the arena for this
                let reqs = format!(
                    "{}",
                    CommaSeparatedDebugIter(RefCell::new(build.remaining_requirements()))
                );

                context.key_value(
                    "Target:",
                    || Value::Some(ui_str!(in context, "{}", deets.target)),
                    None,
                    COLOR_ORANGE,
                );

                context.key_value(
                    "Requirements:",
                    move || Value::Wrapped(ui_str!(in context, "{}", reqs)),
                    None,
                    COLOR_ORANGE,
                );

                context.key_value(
                    "Progress:",
                    || Value::Some(ui_str!(in context, "{}/{}", progress.steps_completed, progress.total_steps_needed)),
                    None,
                    COLOR_ORANGE
                );
            });

        if ret.is_none() {
            return context.text_disabled("Error: invalid job");
        }

        // TODO other job tabs
    }

    fn world_selection(&mut self, context: &UiContext) {
        let _tab = open_or_ret!(context.new_tab("World"));

        let selection_res = context.simulation().ecs.resource::<SelectedTiles>();
        let selection = match selection_res.current() {
            None => return context.text_disabled("No tile selection"),
            Some(sel) => sel,
        };

        let (from, to) = selection.bounds();
        let w = (to.0 - from.0).abs() + 1;
        let h = (to.1 - from.1).abs() + 1;
        let z = (to.2 - from.2).abs().slice() + 1;

        context.key_value(
            "Size:",
            || {
                if z == 1 {
                    ui_str!(in context, "{}x{} ({})", w, h, w*h)
                } else {
                    ui_str!(in context, "{}x{}x{} ({})", w, h,z, w*h*z)
                }
            },
            None,
            COLOR_BLUE,
        );

        context.key_value(
            "Progress:",
            || {
                ui_str!(in context, "{}", match selection.progress() {
                    SelectionProgress::Complete => "Selected",
                    SelectionProgress::InProgress => "Ongoing",
                })
            },
            None,
            COLOR_BLUE,
        );

        context.key_value(
            "From:",
            || ui_str!(in context, "{}", from),
            None,
            COLOR_ORANGE,
        );

        context.key_value(
            "To:  ",
            || ui_str!(in context, "{}", to),
            None,
            COLOR_ORANGE,
        );

        // selection modification buttons
        {
            let mut modification = None;
            if context.button("Move up") {
                modification = Some(SelectionModification::Up);
            }

            context.ui().same_line();

            if context.button("Move down") {
                modification = Some(SelectionModification::Down);
            }

            if let Some(modification) = modification {
                context.issue_request(UiRequest::ModifySelection(modification));
            }
        }

        context.key_value(
            "Blocks:",
            || Value::Wrapped(ui_str!(in context, "{}", selection.block_occurrences())),
            None,
            COLOR_ORANGE,
        );

        let _tab_bar = open_or_ret!(context.new_tab_bar("##worldselectiontabbar"));

        // single block
        {
            let tab = context.new_tab("Block");
            if tab.is_some() {
                self.do_single_block(context, selection_res);
            }
        }

        // generation
        #[cfg(feature = "procgen")]
        {
            let tab = context.new_tab("Generation");
            if tab.is_some() {
                self.do_generation(context, selection_res);
            }
        }

        // modification
        {
            let tab = context.new_tab("Edit");
            if tab.is_some() {
                self.do_edit(context);
            }
        }
    }

    fn do_single_block(&mut self, context: &UiContext, selection: &SelectedTiles) {
        let pos = match selection
            .current_selected()
            .and_then(|sel| sel.single_tile())
        {
            Some(pos) => pos,
            None => {
                context.text_disabled("Single block selection required");
                return;
            }
        };

        let world = context.simulation().world;
        let world = world.borrow();

        let (block, above) = match world.block(pos).zip(world.block(pos.above())) {
            Some(blocks) => blocks,
            _ => {
                context.text_disabled("Error: block not found");
                return;
            }
        };

        context.key_value(
            "Type:",
            || ui_str!(in context, "{}", block.block_type()),
            None,
            COLOR_ORANGE,
        );

        context.key_value(
            "Accessibility:",
            || {
                above
                    .walkable_area()
                    .map(|area_index| ui_str!(in context, "{:?}", area_index))
                    .unwrap_or("Inaccessible")
            },
            Some("Accessibility of the block above the selection"),
            COLOR_ORANGE,
        );

        context.key_value(
            "Durability:",
            || ui_str!(in context, "{}", block.durability()),
            None,
            COLOR_ORANGE,
        );

        context.key_value(
            "AO:",
            || Value::Wrapped(ui_str!(in context, "{:?}", block.occlusion())),
            Some("Ambient occlusion"),
            COLOR_ORANGE,
        );

        let block_data = world.associated_block_data(pos);
        context.key_value(
            "Block data:",
            || {
                block_data
                    .map(|data| ui_str!(in context, "{:?}", AssociatedBlockDataType::from(data)))
                    .unwrap_or("None")
            },
            None,
            COLOR_ORANGE,
        );

        if let Some(data) = block_data {
            let name = ui_str!(in context, "{:?}", AssociatedBlockDataType::from(data));
            let tree = context.new_tree_node(name, DefaultOpen::Closed);
            if tree.is_some() {
                self.do_single_block_associated_data(context, data);
            }
        }
    }

    fn do_single_block_associated_data(&mut self, context: &UiContext, data: &AssociatedBlockData) {
        let ecs = context.simulation().ecs;
        match *data {
            AssociatedBlockData::Container(container_entity) => {
                let kind = context.description(container_entity);

                context.key_value(
                    "Kind:",
                    || ui_str!(in context, "{}", kind),
                    None,
                    COLOR_ORANGE,
                );

                let container = match ecs.component::<ContainerComponent>(container_entity).ok() {
                    None => {
                        context.text_disabled("Error: missing container");
                        return;
                    }
                    Some(c) => c,
                };

                context.key_value(
                    "Owner:",
                    || {
                        container
                            .owner
                            .map(|o| ui_str!(in context, "{}", o))
                            .unwrap_or("No owner")
                    },
                    None,
                    COLOR_ORANGE,
                );

                context.key_value(
                    "Communal:",
                    || {
                        container
                            .communal()
                            .map(|society| {
                                let name = ecs
                                    .resource::<Societies>()
                                    .society_by_handle(society)
                                    .map(|s| s.name())
                                    .unwrap_or("Error: bad society");

                                ui_str!(in context, "{}", name)
                            })
                            .unwrap_or("Not communal")
                    },
                    None,
                    COLOR_ORANGE,
                );

                let entity_selection = ecs.resource::<SelectedEntities>();
                // TODO proper way of checking if an entity is living e.g. separate component for container ownership
                let living_entity = entity_selection.just_one().and_then(|e| {
                    if ecs.is_entity_alive(e) && !ecs.has_component::<ConditionComponent>(e) {
                        Some(e)
                    } else {
                        None
                    }
                });

                if let Some(living) = living_entity {
                    if context.button("Set owner") {
                        context.issue_request(UiRequest::SetContainerOwnership {
                            container: container_entity,
                            owner: Some(Some(living)),
                            communal: None,
                        });
                    }

                    if let Ok(society) = ecs.component::<SocietyComponent>(living) {
                        context.same_line();
                        if context.button("Set communal") {
                            context.issue_request(UiRequest::SetContainerOwnership {
                                container: container_entity,
                                owner: None,
                                communal: Some(Some(society.handle())),
                            });
                        }
                    }
                }

                if context.button("Clear owner") {
                    context.issue_request(UiRequest::SetContainerOwnership {
                        container: container_entity,
                        owner: Some(None),
                        communal: None,
                    });
                }
                context.same_line();
                if context.button("Clear communal") {
                    context.issue_request(UiRequest::SetContainerOwnership {
                        container: container_entity,
                        owner: None,
                        communal: Some(None),
                    });
                }

                self.do_container(context, &container.container);
            }
            _ => context.text_disabled("Unimplemented"),
        }
    }

    #[cfg(feature = "procgen")]
    fn do_generation(&mut self, context: &UiContext, selection: &SelectedTiles) {
        let loader = context.simulation().loader;

        if !loader.is_generated() {
            return context.text_disabled("World is not generated");
        }

        let single_tile = selection
            .current_selected()
            .and_then(|sel| sel.single_tile());
        let block_query = single_tile.and_then(|pos| loader.query_block(pos));
        let details = match (block_query, single_tile) {
            (None, Some(_)) => {
                return context.text_disabled("Query failed");
            }
            (None, _) => {
                return context.text_disabled("Single selection required");
            }
            (Some(t), _) => t,
        };

        let (primary_biome, _) = details.biome_choices.iter().next().expect("missing biome");

        context.key_value(
            "Biome:",
            || ui_str!(in context, "{:?}", primary_biome),
            None,
            COLOR_GREEN,
        );

        context.text(ui_str!(in context, "{} biome candidates", details.biome_choices.len()));
        for (biome, weight) in details.biome_choices.iter() {
            context.text(ui_str!(in context, "   {:?} ({})", biome, weight));
        }

        context.key_value(
            "Coastline proximity:",
            || ui_str!(in context, "{:.4}", details.coastal_proximity),
            None,
            COLOR_GREEN,
        );
        context.key_value(
            "Elevation:",
            || ui_str!(in context, "{:.4}", details.base_elevation),
            None,
            COLOR_GREEN,
        );
        context.key_value(
            "Temperature:",
            || ui_str!(in context, "{:.4}", details.temperature),
            None,
            COLOR_GREEN,
        );
        context.key_value(
            "Moisture:",
            || ui_str!(in context, "{:.4}", details.moisture),
            None,
            COLOR_GREEN,
        );

        context.separator();

        if let Some((region, features)) = details.region.as_ref() {
            context.key_value(
                "Region:",
                || ui_str!(in context, "{:?}", region),
                None,
                COLOR_BLUE,
            );

            context.text(ui_str!(in context, "{} regional feature(s)", features.len()));
            for feature in features {
                context.text_wrapped(ui_str!(in context, " - {}", feature));
            }
        }
    }

    fn do_edit(&mut self, context: &UiContext) {
        context.group(|| {
            let mut placement = None;
            if context.button(" Set ") {
                placement = Some(BlockPlacement::Set);
            }

            if context.button("Place") {
                placement = Some(BlockPlacement::PlaceAbove);
            }

            if let Some(placement) = placement {
                if let Some(bt) = BlockType::iter().nth(self.edit_selection) {
                    context.issue_request(UiRequest::FillSelectedTiles(placement, bt));
                } else {
                    // reset to a valid one
                    debug_assert!(BlockType::iter().count() > 0);
                    self.edit_selection = 0;
                }
            }
        });
        context.same_line();

        ChildWindow::new("##editblocktypes")
            .size([0.0, 120.0])
            .horizontal_scrollbar(true)
            .movable(false)
            .build(context.ui(), || {
                for (i, ty) in BlockType::iter().enumerate() {
                    if Selectable::new(ui_str!(in context, "{}", ty))
                        .selected(self.edit_selection == i)
                        .build(context)
                    {
                        self.edit_selection = i;
                    }
                }
            });
    }
}

impl<'a> SelectedEntityDetails<'a> {
    fn component<T: simulation::Component>(
        &'a self,
        ctx: &'a UiContext,
    ) -> Option<ComponentRef<'a, T>> {
        ctx.simulation().ecs.component(self.entity).ok()
    }
}

impl<I: Iterator<Item = T>, T: Debug> Display for CommaSeparatedDebugIter<I, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut iter = self.0.borrow_mut();
        f.debug_list().entries(&mut *iter).finish()
    }
}
