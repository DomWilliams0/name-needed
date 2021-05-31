use imgui::{im_str, ChildWindow, InputTextMultiline, Selectable};

use common::*;
use simulation::input::{
    BlockPlacement, DivineInputCommand, SelectedEntity, SelectedTiles, UiRequest,
};
use simulation::{
    ActivityComponent, AssociatedBlockData, AssociatedBlockDataType, BlockType, ComponentWorld,
    ConditionComponent, Container, ContainerComponent, EdibleItemComponent, Entity,
    EntityLoggingComponent, FollowPathComponent, HungerComponent, IntoEnumIterator,
    InventoryComponent, NameComponent, PhysicalComponent, Societies, SocietyComponent,
    TransformComponent, E,
};

use crate::render::sdl::ui::context::{DefaultOpen, UiContext};

use crate::render::sdl::ui::windows::{
    with_fake_owned_imstr, UiExt, Value, COLOR_BLUE, COLOR_GREEN, COLOR_ORANGE,
};
use crate::ui_str;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct SelectionWindow {
    /// Index into [BlockType::into_enum_iter]
    edit_selection: usize,
}

struct SelectedEntityDetails<'a> {
    entity: Entity,
    name: Option<&'a NameComponent>,
    transform: Option<&'a TransformComponent>,
    physical: Option<&'a PhysicalComponent>,
    ty: EntityType<'a>,
}

enum EntityType<'a> {
    Living,
    Item(&'a ConditionComponent),
}

#[derive(Debug)]
enum EntityState {
    Alive,
    Dead,
    Inanimate,
}

impl SelectionWindow {
    pub fn render(&mut self, context: &UiContext) {
        self.entity_selection(context);
        self.world_selection(context);
    }

    fn entity_selection(&mut self, context: &UiContext) {
        let tab = context.new_tab(im_str!("Entity"));
        if !tab.is_open() {
            return;
        }

        let ecs = context.simulation().ecs;

        let (e, details, state) = match ecs.resource::<SelectedEntity>().get_unchecked() {
            Some(e) if ecs.is_entity_alive(e) => {
                let transform = ecs.component(e).ok();
                let name = ecs.component(e).ok();
                let physical = ecs.component(e).ok();
                let (ty, state) = match ecs.component::<ConditionComponent>(e) {
                    Ok(condition) => (EntityType::Item(condition), EntityState::Inanimate),
                    _ => (EntityType::Living, EntityState::Alive),
                };

                (
                    e,
                    Some(SelectedEntityDetails {
                        entity: e,
                        name,
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
            None => {
                context.text_disabled(im_str!("No entity selected"));
                return;
            }
        };

        context.key_value(
            im_str!("Entity:"),
            || ui_str!(in context, "{}", E(e)),
            None,
            COLOR_GREEN,
        );

        context.key_value(
            im_str!("State:"),
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

        context.key_value(
            im_str!("Name:"),
            || {
                details
                    .name
                    .map(|n| ui_str!(in context, "{}", n.0))
                    .ok_or("Unnamed")
            },
            None,
            COLOR_GREEN,
        );
        context.key_value(
            im_str!("Position:"),
            || {
                details
                    .transform
                    .map(|t| ui_str!(in context, "{}", t.position))
                    .ok_or("Unknown")
            },
            None,
            COLOR_GREEN,
        );

        if let Some(physical) = details.physical {
            context.key_value(
                im_str!("Size:"),
                || ui_str!(in context, "{}", physical.size),
                None,
                COLOR_GREEN,
            );

            context.key_value(
                im_str!("Volume:"),
                || ui_str!(in context, "{}", physical.volume),
                None,
                COLOR_GREEN,
            );
        }

        let tabbar = context.new_tab_bar(im_str!("##entitydetailstabbar"));
        if tabbar.is_open() {
            match details.ty {
                EntityType::Living => self.do_living(context, &details),
                EntityType::Item(condition) => self.do_item(context, &details, condition),
            }
        }

        self.do_logs(context, &details);
    }

    //noinspection DuplicatedCode
    fn do_living(&mut self, context: &UiContext, details: &SelectedEntityDetails) {
        let ecs = context.simulation().ecs;

        {
            let tab = context.new_tab(im_str!("Living"));
            if tab.is_open() {
                context.key_value(
                    im_str!("Velocity:"),
                    || {
                        details
                            .transform
                            .map(|t| ui_str!(in context, "{:.2}m/s", t.velocity.magnitude() ))
                    },
                    None,
                    COLOR_ORANGE,
                );

                context.key_value(
                    im_str!("Satiety:"),
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
                    im_str!("Navigating to:"),
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
                    im_str!("Society:"),
                    || {
                        society
                            .map(|comp| {
                                let name = societies
                                    .society_by_handle(comp.handle)
                                    .map(|s| s.name())
                                    .unwrap_or("Invalid handle");
                                ui_str!(in context, "{}", name)
                            })
                            .ok_or("None")
                    },
                    society.map(|comp| ui_str!(in context, "{:?}", comp.handle)),
                    COLOR_ORANGE,
                );
            }
        }

        // inventory
        if let Some(inv) = details.component::<InventoryComponent>(context) {
            let tab = context.new_tab(im_str!("Inventory"));
            if tab.is_open() {
                self.do_inventory(context, inv);
            }
        }

        // activity
        if let Some(activity) = details.component::<ActivityComponent>(context) {
            let tab = context.new_tab(im_str!("Activity"));
            if tab.is_open() {
                self.do_activity(context, activity);
            }
        }

        // control
        {
            let tab = context.new_tab(im_str!("Control"));
            if tab.is_open() {
                let world_selection = ecs.resource::<SelectedTiles>();
                if let Some(tile) = world_selection.single_tile() {
                    if context.button(im_str!("Go to selected block"), [0.0, 0.0]) {
                        context.issue_request(UiRequest::IssueDivineCommand(
                            DivineInputCommand::Goto(tile.above()),
                        ));
                    }

                    if context.button(im_str!("Break selected block"), [0.0, 0.0]) {
                        context.issue_request(UiRequest::IssueDivineCommand(
                            DivineInputCommand::Break(tile),
                        ));
                    }
                } else {
                    context.text_disabled("Select a single block");
                }
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
            let tab = context.new_tab(im_str!("Item"));
            if tab.is_open() {
                context.key_value(
                    im_str!("Condition:"),
                    || ui_str!(in context, "{}", condition.0),
                    None,
                    COLOR_ORANGE,
                );
            }
        }

        if let Some(edible) = details.component::<EdibleItemComponent>(context) {
            let tab = context.new_tab(im_str!("Nutrition"));
            if tab.is_open() {
                context.key_value(
                    im_str!("Nutrition:"),
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
            context.same_line(0.0);
            context.text(ui_str!(in context, "{} ", slot));
        }

        context.separator();
        context.text_disabled(
            ui_str!(in context, "{} containers", inventory.containers_unresolved().len()),
        );

        for (i, (e, container)) in inventory.containers(ecs).enumerate() {
            let name = ecs
                .component::<NameComponent>(e)
                .map(|n| n.0.as_str())
                .unwrap_or("unnamed");

            let tree = context.new_tree_node(
                ui_str!(in context, "#{}: {}##container", i+1, name),
                DefaultOpen::Closed,
            );

            if tree.is_open() {
                self.do_container(context, container);
            }
        }
    }

    fn do_container(&mut self, context: &UiContext, container: &Container) {
        let (max_vol, max_size) = container.limits();
        let capacity = container.current_capacity();
        context.text_colored(
            COLOR_GREEN,
            ui_str!(in context, "Capacity {}/{}, size {}", capacity, max_vol, max_size),
        );

        let ecs = context.simulation().ecs;
        for entity in container.contents() {
            let name = ecs
                .component::<NameComponent>(entity.entity)
                .map(|n| n.0.as_str())
                .unwrap_or("unnamed"); // TODO stop writing "unnamed" everywhere

            context.text_wrapped(
                ui_str!(in context, " - {} ({}, vol {})", name, E(entity.entity), entity.volume),
            );
        }
    }

    fn do_activity(&mut self, context: &UiContext, activity: &ActivityComponent) {
        context.key_value(
            im_str!("Activity:"),
            || Value::Wrapped(ui_str!(in context, "{}", activity.current())),
            None,
            COLOR_ORANGE,
        );

        context.key_value(
            im_str!("Subactivity:"),
            || Value::Wrapped(ui_str!(in context, "{}", activity.current().current_subactivity())),
            None,
            COLOR_ORANGE,
        );

        context.separator();

        let reservation = activity.current_society_task();
        context.key_value(
            im_str!("Reserved task:"),
            || {
                if let Some((_, task)) = reservation {
                    Value::Wrapped(ui_str!(in context, "{}", task))
                } else {
                    Value::None("None")
                }
            },
            None,
            COLOR_GREEN,
        );

        context.key_value(
            im_str!("Job:"),
            || {
                if let Some((job, _)) = reservation {
                    Value::Wrapped(ui_str!(in context, "{}", job))
                } else {
                    Value::None("None")
                }
            },
            None,
            COLOR_GREEN,
        );
    }

    fn do_logs(&mut self, context: &UiContext, details: &SelectedEntityDetails) {
        let tab = context.new_tab(im_str!("Logs"));
        if !tab.is_open() {
            return;
        }

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
            let str = ui_str!(in context, "{}", EntityLogs(logs));
            // safety: readonly textbox
            unsafe {
                with_fake_owned_imstr(str, |str| {
                    InputTextMultiline::new(
                        context.ui(),
                        im_str!("##entitylogs"),
                        str,
                        [context.window_content_region_width(), 0.0],
                    )
                    .no_horizontal_scroll(false)
                    .read_only(true)
                    .build();
                });
            }
        };

        let mut req = None;
        match context
            .simulation()
            .ecs
            .component::<EntityLoggingComponent>(details.entity)
        {
            Ok(comp) => {
                if context.button(im_str!("Disable logs"), [0.0, 0.0]) {
                    req = Some(false);
                } else {
                    render_logs(comp);
                }
            }
            _ => {
                if context.button(im_str!("Enable logs"), [0.0, 0.0]) {
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

    fn world_selection(&mut self, context: &UiContext) {
        let tab = context.new_tab(im_str!("World"));
        if !tab.is_open() {
            return;
        }

        let selection = context.simulation().ecs.resource::<SelectedTiles>();
        let bounds = match selection.bounds() {
            None => {
                context.text_disabled(im_str!("No tile selection"));
                return;
            }
            Some(bounds) => bounds,
        };

        let (from, to) = bounds;
        let w = (to.0 - from.0).abs() + 1;
        let h = (to.1 - from.1).abs() + 1;
        let z = (to.2 - from.2).abs().slice() + 1;

        context.key_value(
            im_str!("Size:"),
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
            im_str!("From:"),
            || ui_str!(in context, "{}", from),
            None,
            COLOR_ORANGE,
        );

        context.key_value(
            im_str!("To:  "),
            || ui_str!(in context, "{}", to),
            None,
            COLOR_ORANGE,
        );

        let tab_bar = context.new_tab_bar(im_str!("##worldselectiontabbar"));
        if !tab_bar.is_open() {
            return;
        }

        // single block
        {
            let tab = context.new_tab(im_str!("Block"));
            if tab.is_open() {
                self.do_single_block(context, selection);
            }
        }

        // generation
        {
            let tab = context.new_tab(im_str!("Generation"));
            if tab.is_open() {
                self.do_generation(context, selection);
            }
        }

        // modification
        {
            let tab = context.new_tab(im_str!("Edit"));
            if tab.is_open() {
                self.do_edit(context);
            }
        }
    }

    fn do_single_block(&mut self, context: &UiContext, selection: &SelectedTiles) {
        let pos = match selection.single_tile() {
            Some(pos) => pos,
            None => {
                context.text_disabled(im_str!("Single block selection required"));
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
            im_str!("Type:"),
            || ui_str!(in context, "{}", block.block_type()),
            None,
            COLOR_ORANGE,
        );

        context.key_value(
            im_str!("Accessibility:"),
            || {
                above
                    .walkable_area()
                    .map(|area_index| ui_str!(in context, "{:?}", area_index))
                    .unwrap_or(im_str!("Inaccessible"))
            },
            Some(im_str!("Accessibility of the block above the selection")),
            COLOR_ORANGE,
        );

        context.key_value(
            im_str!("Durability:"),
            || ui_str!(in context, "{}", block.durability()),
            None,
            COLOR_ORANGE,
        );

        context.key_value(
            im_str!("AO:"),
            || Value::Wrapped(ui_str!(in context, "{:?}", block.occlusion())),
            Some(im_str!("Ambient occlusion")),
            COLOR_ORANGE,
        );

        let block_data = world.associated_block_data(pos);
        context.key_value(
            im_str!("Block data:"),
            || {
                block_data
                    .map(|data| ui_str!(in context, "{:?}", AssociatedBlockDataType::from(data)))
                    .unwrap_or(im_str!("None"))
            },
            None,
            COLOR_ORANGE,
        );

        if let Some(data) = block_data {
            let name = ui_str!(in context, "{:?}", AssociatedBlockDataType::from(data));
            let tree = context.new_tree_node(name, DefaultOpen::Closed);
            if tree.is_open() {
                self.do_single_block_associated_data(context, data);
            }
        }
    }

    fn do_single_block_associated_data(&mut self, context: &UiContext, data: &AssociatedBlockData) {
        let ecs = context.simulation().ecs;
        match *data {
            AssociatedBlockData::Container(container_entity) => {
                let name = ecs
                    .component::<NameComponent>(container_entity)
                    .ok()
                    .map(|c| c.0.as_str())
                    .unwrap_or("Unnamed");

                context.key_value(
                    im_str!("Name:"),
                    || ui_str!(in context, "{}", name),
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
                    im_str!("Owner:"),
                    || {
                        container
                            .owner
                            .map(|o| ui_str!(in context, "{}", E(o)))
                            .unwrap_or(im_str!("No owner"))
                    },
                    None,
                    COLOR_ORANGE,
                );

                context.key_value(
                    im_str!("Communal:"),
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
                            .unwrap_or(im_str!("Not communal"))
                    },
                    None,
                    COLOR_ORANGE,
                );

                let entity_selection = ecs.resource::<SelectedEntity>().get_unchecked();
                // TODO proper way of checking if an entity is living
                let living_entity = entity_selection.and_then(|e| {
                    if ecs.is_entity_alive(e) && !ecs.has_component::<ConditionComponent>(e) {
                        Some(e)
                    } else {
                        None
                    }
                });

                if let Some(living) = living_entity {
                    if context.button(im_str!("Set owner"), [0.0, 0.0]) {
                        context.issue_request(UiRequest::SetContainerOwnership {
                            container: container_entity,
                            owner: Some(Some(living)),
                            communal: None,
                        });
                    }

                    if let Ok(society) = ecs.component::<SocietyComponent>(living) {
                        context.same_line(0.0);
                        if context.button(im_str!("Set communal"), [0.0, 0.0]) {
                            context.issue_request(UiRequest::SetContainerOwnership {
                                container: container_entity,
                                owner: None,
                                communal: Some(Some(society.handle)),
                            });
                        }
                    }
                }

                if context.button(im_str!("Clear owner"), [0.0, 0.0]) {
                    context.issue_request(UiRequest::SetContainerOwnership {
                        container: container_entity,
                        owner: Some(None),
                        communal: None,
                    });
                }
                context.same_line(0.0);
                if context.button(im_str!("Clear communal"), [0.0, 0.0]) {
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

    fn do_generation(&mut self, context: &UiContext, selection: &SelectedTiles) {
        let loader = context.simulation().loader;

        if !loader.is_generated() {
            context.text_disabled(im_str!("World is not generated"));
            return;
        }

        let single_tile = selection.single_tile();
        let block_query = single_tile.and_then(|pos| loader.query_block(pos));
        let details = match (block_query, single_tile) {
            (None, Some(_)) => {
                context.text_disabled(im_str!("Query failed"));
                return;
            }
            (None, _) => {
                context.text_disabled(im_str!("Single selection required"));
                return;
            }
            (Some(t), _) => t,
        };

        let (primary_biome, _) = details.biome_choices.iter().next().expect("missing biome");

        context.key_value(
            im_str!("Biome:"),
            || ui_str!(in context, "{:?}", primary_biome),
            None,
            COLOR_GREEN,
        );

        context.text(ui_str!(in context, "{} biome candidates", details.biome_choices.len()));
        for (biome, weight) in details.biome_choices.iter() {
            context.text(ui_str!(in context, "   {:?} ({})", biome, weight));
        }

        context.key_value(
            im_str!("Coastline proximity:"),
            || ui_str!(in context, "{:.4}", details.coastal_proximity),
            None,
            COLOR_GREEN,
        );
        context.key_value(
            im_str!("Elevation:"),
            || ui_str!(in context, "{:.4}", details.base_elevation),
            None,
            COLOR_GREEN,
        );
        context.key_value(
            im_str!("Temperature:"),
            || ui_str!(in context, "{:.4}", details.temperature),
            None,
            COLOR_GREEN,
        );
        context.key_value(
            im_str!("Moisture:"),
            || ui_str!(in context, "{:.4}", details.moisture),
            None,
            COLOR_GREEN,
        );

        context.separator();

        if let Some((region, features)) = details.region.as_ref() {
            context.key_value(
                im_str!("Region:"),
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
            if context.button(im_str!(" Set "), [0.0, 0.0]) {
                placement = Some(BlockPlacement::Set);
            }

            if context.button(im_str!("Place"), [0.0, 0.0]) {
                placement = Some(BlockPlacement::PlaceAbove);
            }

            if let Some(placement) = placement {
                if let Some(bt) = BlockType::into_enum_iter().nth(self.edit_selection) {
                    context.issue_request(UiRequest::FillSelectedTiles(placement, bt));
                } else {
                    // reset to a valid one
                    debug_assert!(BlockType::into_enum_iter().count() > 0);
                    self.edit_selection = 0;
                }
            }
        });
        context.same_line(0.0);

        ChildWindow::new("##editblocktypes")
            .size([0.0, 120.0])
            .horizontal_scrollbar(true)
            .movable(false)
            .build(context.ui(), || {
                for (i, ty) in BlockType::into_enum_iter().enumerate() {
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
    fn component<T: simulation::Component>(&'a self, ctx: &'a UiContext) -> Option<&'a T> {
        ctx.simulation().ecs.component(self.entity).ok()
    }
}

impl Default for SelectionWindow {
    fn default() -> Self {
        Self { edit_selection: 0 }
    }
}
