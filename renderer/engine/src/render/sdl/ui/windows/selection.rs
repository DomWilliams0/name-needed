use imgui::{im_str, ImStr, TabBar, TabItem, TreeNode, Ui};

use common::InnerSpace;
use simulation::input::{
    BlockPlacement, DivineInputCommand, SelectedEntity, SelectedTiles, UiRequest,
};
use simulation::{
    ActivityComponent, ComponentWorld, ConditionComponent, Container, EdibleItemComponent, Entity,
    FollowPathComponent, HungerComponent, IntoEnumIterator, InventoryComponent, ItemCondition,
    NameComponent, PhysicalComponent, Societies, SocietyComponent, SocietyHandle,
    TransformComponent, E,
};

use crate::render::sdl::ui::context::{DefaultOpen, UiContext};

use crate::render::sdl::ui::windows::{UiExt, Value, COLOR_GREEN, COLOR_ORANGE, COLOR_RED};
use crate::ui_str;

pub struct SelectionWindow {
    block_placement: BlockPlacement,
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
            || Value::Some(ui_str!(in context, "{}", E(e))),
            None,
            COLOR_GREEN,
        );

        context.key_value(
            im_str!("State:"),
            || Value::Some(ui_str!(in context, "{:?}", state)),
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
                || Value::Some(ui_str!(in context, "{}", physical.size)),
                None,
                COLOR_GREEN,
            );

            context.key_value(
                im_str!("Volume:"),
                || Value::Some(ui_str!(in context, "{}", physical.volume)),
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
                    || Value::Some(ui_str!(in context, "{}", condition.0)),
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
                    || Value::Some(ui_str!(in context, "{}", edible.total_nutrition)),
                    None,
                    COLOR_ORANGE,
                );
            }
        }
    }

    pub fn render(&mut self, context: &UiContext) {
        self.entity_selection(context);

        // TODO world selection

        /*
        context.separator();

        // world selection

        let bounds = match context.blackboard.selected_tiles.bounds() {
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
                    Value::Some(ui_str!(in context, "{}x{} ({})", w, h, w*h))
                } else {
                    Value::Some(ui_str!(in context, "{}x{}x{} ({})", w, h,z, w*h*z))
                }
            },
            None,
            COLOR_BLUE,
        );

        context.key_value(
            im_str!("From:"),
            || Value::Some(ui_str!(in context, "{}", from)),
            None,
            COLOR_ORANGE,
        );

        context.key_value(
            im_str!("To:  "),
            || Value::Some(ui_str!(in context, "{}", to)),
            None,
            COLOR_ORANGE,
        );

        TreeNode::new(im_str!("Generation info"))
            .default_open(false)
            .build(context, || {
                let details = match (
                    context.blackboard.selected_block_details.as_ref(),
                    context.blackboard.selected_tiles.single_tile(),
                ) {
                    (None, Some(_)) => {
                        context.text_disabled(im_str!("Incompatible terrain source"));
                        return;
                    }
                    (None, _) => {
                        context.text_disabled(im_str!("Single selection required"));
                        return;
                    }
                    (Some(t), _) => t,
                };

                let (primary, _) = match details.biome_choices.iter().next() {
                    Some(b) => b,
                    None => {
                        context.text_colored(COLOR_RED, im_str!("Error: missing biome"));
                        return;
                    }
                };

                context.key_value(
                    im_str!("Biome:  "),
                    || Value::Some(ui_str!(in context, "{:?}", primary)),
                    None,
                    COLOR_GREEN,
                );

                context.text(ui_str!(in context, "{} candidates", details.biome_choices.len()));
                for (biome, weight) in details.biome_choices.iter() {
                    context.text(ui_str!(in context, " - {:?} ({})", biome, weight));
                }

                context.key_value(
                    im_str!("Coastline proximity:  "),
                    || Value::Some(ui_str!(in context, "{:.4}", details.coastal_proximity)),
                    None,
                    COLOR_GREEN,
                );
                context.key_value(
                    im_str!("Elevation:  "),
                    || Value::Some(ui_str!(in context, "{:.4}", details.base_elevation)),
                    None,
                    COLOR_GREEN,
                );
                context.key_value(
                    im_str!("Temperature:  "),
                    || Value::Some(ui_str!(in context, "{:.4}", details.temperature)),
                    None,
                    COLOR_GREEN,
                );
                context.key_value(
                    im_str!("Moisture:  "),
                    || Value::Some(ui_str!(in context, "{:.4}", details.moisture)),
                    None,
                    COLOR_GREEN,
                );

                if let Some((region, features)) = details.region.as_ref() {
                    context.key_value(
                        im_str!("Region:   "),
                        || Value::Some(ui_str!(in context, "{:?}", region)),
                        None,
                        COLOR_BLUE,
                    );

                    context.text(ui_str!(in context, "{} regional feature(s)", features.len()));
                    for feature in features {
                        context.text_wrapped(ui_str!(in context, " - {}", feature));
                    }
                }
            });

        context.separator();
        context.radio_button(
            im_str!("Set blocks"),
            &mut self.block_placement,
            BlockPlacement::Set,
        );
        context.same_line(0.0);
        context.radio_button(
            im_str!("Place blocks"),
            &mut self.block_placement,
            BlockPlacement::PlaceAbove,
        );

        let mut mk_button = |bt: BlockType| {
            if context.button(ui_str!(in context, "{}", bt), [0.0, 0.0]) {
                context
                    .commands
                    .push(UiCommand::FillSelectedTiles(self.block_placement, bt));
            }
        };

        for mut types in BlockType::into_enum_iter().chunks(3).into_iter() {
            types.next().map(&mut mk_button);
            for bt in types {
                context.same_line(0.0);
                mk_button(bt);
            }
        }

        if let Some((container_entity, container_name, container)) =
            context.blackboard.selected_container
        {
            context.separator();
            context.key_value(
                im_str!("Container: "),
                || Value::Some(ui_str!(in context, "{}", container_name)),
                None,
                COLOR_ORANGE,
            );
            context.key_value(
                im_str!("Owner: "),
                || {
                    if let Some(owner) = container.owner {
                        Value::Some(ui_str!(in context, "{}", E(owner)))
                    } else {
                        Value::None("No owner")
                    }
                },
                None,
                COLOR_ORANGE,
            );
            context.key_value(
                im_str!("Communal: "),
                || {
                    if let Some(society) = container.communal() {
                        Value::Some(ui_str!(in context, "{:?}", society))
                    } else {
                        Value::None("Not communal")
                    }
                },
                None,
                COLOR_ORANGE,
            );
            if let Some(SelectedEntityDetails {
                entity,
                details: EntityDetails::Living { society, .. },
                ..
            }) = context.blackboard.selected_entity
            {
                if context.button(im_str!("Set owner"), [0.0, 0.0]) {
                    context.commands.push(UiCommand::SetContainerOwnership {
                        container: container_entity,
                        owner: Some(Some(entity)),
                        communal: None,
                    });
                }
                if let Some(society) = society {
                    context.same_line(0.0);
                    if context.button(im_str!("Set communal"), [0.0, 0.0]) {
                        context.commands.push(UiCommand::SetContainerOwnership {
                            container: container_entity,
                            owner: None,
                            communal: Some(Some(society)),
                        });
                    }
                }
            }

            if context.button(im_str!("Clear owner"), [0.0, 0.0]) {
                context.commands.push(UiCommand::SetContainerOwnership {
                    container: container_entity,
                    owner: Some(None),
                    communal: None,
                });
            }
            context.same_line(0.0);
            if context.button(im_str!("Clear communal"), [0.0, 0.0]) {
                context.commands.push(UiCommand::SetContainerOwnership {
                    container: container_entity,
                    owner: None,
                    communal: Some(None),
                });
            }
            self.do_container(context, context, im_str!("Contents"), &container.container);
        }*/
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

            self.do_container(
                context,
                ui_str!(in context, "#{}: {}##container", i+1, name),
                container,
            );
        }
    }

    fn do_container(&mut self, context: &UiContext, name: &ImStr, container: &Container) {
        let tree = context.new_tree_node(name, DefaultOpen::Closed);
        if !tree.is_open() {
            return;
        }

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
                .unwrap_or("unnamed");

            context.text_wrapped(ui_str!(in context, " - {} ({})", entity, name));
        }
    }

    fn do_activity(&mut self, context: &UiContext, activity: &ActivityComponent) {
        context.key_value(
            im_str!("Activity:"),
            || Value::Wrapped(ui_str!(in context, "{}", activity.current)),
            None,
            COLOR_ORANGE,
        );

        context.key_value(
            im_str!("Subactivity:"),
            || Value::Wrapped(ui_str!(in context, "{}", activity.current.current_subactivity())),
            None,
            COLOR_ORANGE,
        );
    }
}

impl<'a> SelectedEntityDetails<'a> {
    fn component<T: simulation::Component>(&'a self, ctx: &'a UiContext) -> Option<&'a T> {
        ctx.simulation().ecs.component(self.entity).ok()
    }
}

impl Default for SelectionWindow {
    fn default() -> Self {
        Self {
            block_placement: BlockPlacement::Set,
        }
    }
}
