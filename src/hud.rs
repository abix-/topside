//! What the player reads and handles on screen, painted from modforge's
//! `HudState` and the player's own holders (modforge decides every
//! move; this only draws and forwards): the hotbar always shown, the
//! inventory panel with an open box beside it, the note panel.

use bevy::prelude::*;
use modforge::hud::{
    Holder, Holders, HudState, OpenPanel, SlotRef, drop_from, end_drag, hotbar_key, select_slot, set_split,
    slot_label, start_drag, toggle_panel, transfer_slot, use_slot, HOTBAR_SLOTS,
};
use modforge::item::EQUIPMENT_SLOTS;

use crate::Ground;
use crate::actor::{Dead, Hotbar, Inventory, INVENTORY_SLOTS, Survival};
use modforge::memory::Did;
use crate::combat::Equipment;
use modforge::crafting::{can_craft, craft};
use modforge::storm::StormPhase;

use crate::content::{ItemRegistryRes, RecipeRegistryRes};
use crate::input::{Action, Actions, ThisTick};
use crate::interact::{Container, Hud, OpenContainer, OpenStation};
use crate::player::Player;

/// Slots drawn for an open box; a bigger box shows its first ones.
pub const CONTAINER_SLOTS: usize = 24;
/// Slots across a row of the inventory and the open box (Rust's 6 by 4).
const ACROSS: usize = 6;
const SLOT_PX: f32 = 52.0;
const GAP_PX: f32 = 4.0;
const PAD_PX: f32 = 8.0;
/// Space between the belt and the screen's bottom, and between panels.
const MARGIN_PX: f32 = 16.0;
/// How tall the character's box stands over the equipment row, in slots.
const CHARACTER_SLOTS_TALL: f32 = 4.0;
const PANEL_COLOR: Color = Color::srgba(0.05, 0.05, 0.05, 0.8);
const SLOT_COLOR: Color = Color::srgba(0.12, 0.11, 0.1, 0.85);
const HELD_COLOR: Color = Color::srgba(0.45, 0.38, 0.2, 0.95);
const HOVER_COLOR: Color = Color::srgba(0.3, 0.3, 0.32, 0.95);

/// One slot on screen and which holder's slot it shows.
#[derive(Component)]
pub struct SlotUi(pub SlotRef);

/// The icons a slot shows, bottom to top: its item's and each of its
/// layers' (`model::stacked`), as scripts/models.py rendered them to
/// assets/icons; and the nodes drawing them.
#[derive(Component, Default)]
pub struct SlotIcons {
    /// The item in the slot, if any.
    pub item: Option<String>,
    pub shown: Vec<String>,
    nodes: Vec<Entity>,
}

/// The icons a stack shows: its item's picture and each layer's on it
/// (`model::stacked`, the same models the hand and the ground draw).
pub fn icons_of(
    stack: &modforge::item::ItemStack,
    items: &modforge::item::ItemRegistry,
    layers: &modforge::item::LayerRegistry,
) -> Vec<String> {
    items
        .def(&stack.item)
        .and_then(|def| def.picture.as_deref())
        .map(|base| crate::model::stacked(base, stack, layers))
        .unwrap_or_default()
}

/// What is in `slot` of the player's holders, if anything.
fn stack_at<'a>(
    slot: SlotRef,
    inventory: &'a Inventory,
    hotbar: &'a Hotbar,
    worn: &'a Equipment,
    container: Option<&'a Container>,
) -> Option<&'a modforge::item::ItemStack> {
    let holder = match slot.holder {
        Holder::Inventory => &inventory.0,
        Holder::Hotbar => &hotbar.0.slots,
        Holder::Container => &container?.0,
        Holder::Equipment => return worn.0.get(slot.index),
    };
    holder.slots.get(slot.index).and_then(|s| s.as_ref())
}

/// Draw `wanted` icons in `node`, one over the other, unless it already
/// shows them.
fn show_icons(commands: &mut Commands, assets: &AssetServer, node: Entity, icons: &mut SlotIcons, wanted: Vec<String>) {
    if wanted == icons.shown {
        return;
    }
    for old in icons.nodes.drain(..) {
        commands.entity(old).despawn();
    }
    for name in &wanted {
        let icon = commands
            .spawn((
                ImageNode::new(assets.load(format!("icons/{name}.png"))),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                Pickable::IGNORE,
                ChildOf(node),
            ))
            .id();
        icons.nodes.push(icon);
    }
    icons.shown = wanted;
}

/// The INFORMATION of the item information panel: a stack's numbers from
/// its definition, one line each; a layered item's from its layers
/// (`numbers_of`).
pub fn information(
    def: &modforge::item::ItemDef,
    stack: &modforge::item::ItemStack,
    layers: &modforge::item::LayerRegistry,
) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(food) = &def.food {
        for (what, amount) in [("Hunger", food.hunger), ("Thirst", food.thirst), ("Health", food.health)] {
            if amount != 0.0 {
                lines.push(format!("{what} {amount:+.0}"));
            }
        }
    }
    let numbers = modforge::item::numbers_of(def, stack, layers);
    if let Some(armor) = numbers.armor {
        lines.push(format!("Protection {:.0}", armor.amount));
    }
    if let Some(combat) = numbers.combat {
        lines.push(format!("Damage {}", combat.damage));
        lines.push(format!("Reach {:.1} m", combat.reach));
        if combat.spread_degrees > 0.0 {
            lines.push(format!("Spread {:.0} degrees", combat.spread_degrees));
        }
        lines.push(format!("Delay {:.2} s", combat.delay));
        if combat.pellets > 1 {
            lines.push(format!("Pellets {}", combat.pellets));
        }
        if let Some(ammo) = combat.ammo {
            lines.push(format!("Ammo {ammo}"));
        }
    }
    if !stack.layers.is_empty() {
        lines.push(format!("Parts: {}", stack.layers.join(", ")));
    }
    lines
}

/// A button of the item information panel's ACTIONS.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PanelAction {
    /// Eat, drink, use, or wear it (modforge's `use_slot`), named here.
    Use(&'static str),
    Drop,
}

impl PanelAction {
    pub fn label(self) -> &'static str {
        match self {
            PanelAction::Use(name) => name,
            PanelAction::Drop => "Drop",
        }
    }
}

/// The ACTIONS for the item in `slot`: what using it does, if anything
/// (food is eaten, drink drunk, a bandage used, gear worn unless it is
/// worn already), then Drop.
pub fn panel_actions(def: &modforge::item::ItemDef, slot: SlotRef) -> Vec<PanelAction> {
    let mut actions = Vec::new();
    if let Some(food) = &def.food {
        actions.push(PanelAction::Use(if food.hunger > 0.0 {
            "Eat"
        } else if food.thirst > 0.0 {
            "Drink"
        } else {
            "Use"
        }));
    } else if def.armor.is_some() && slot.holder != Holder::Equipment {
        actions.push(PanelAction::Use("Wear"));
    }
    actions.push(PanelAction::Drop);
    actions
}

/// The panel that shows a note's words while it is being read.
#[derive(Component)]
pub struct NotePanel;

/// A panel of slots and the holder it shows: the equipment row, the
/// inventory, the belt, the open box.
#[derive(Component)]
pub struct Panel(pub Holder);

/// The inventory screen, open with the inventory, in Rust's layout: the
/// character with the equipment row under it on the left, the inventory
/// in the middle over the belt, the open box on the right.
#[derive(Component)]
pub struct InventoryScreen;

/// The item information panel (Rust's), over the inventory: the selected
/// slot's item, and what it shows now, for the control plane.
#[derive(Component, Default)]
pub struct ItemPanel {
    pub shown: Option<Shown>,
}

/// What the item information panel shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Shown {
    pub slot: SlotRef,
    pub name: String,
    pub description: String,
    pub information: Vec<String>,
    pub actions: Vec<PanelAction>,
    pub count: u32,
    /// The splitting bar's amount; None is the whole stack.
    pub split: Option<u32>,
}

/// A text of the item information panel, and which.
#[derive(Component, Clone, Copy, PartialEq)]
pub enum PanelText {
    Name,
    Description,
    Information,
    Split,
    Action(usize),
}

/// The `n`th ACTIONS button, and the action it does now.
#[derive(Component)]
pub struct ActionButton(usize, Option<PanelAction>);

/// The splitting bar's row, shown for a stack of more than one.
#[derive(Component)]
pub struct SplitRow;

/// A splitting bar button: take one off the amount, or add one.
#[derive(Component)]
struct SplitButton(i32);

/// The most ACTIONS buttons the panel shows.
const ACTION_BUTTONS: usize = 2;

/// The slot under the cursor, for T.
#[derive(Resource, Default)]
pub struct HoveredSlot(pub Option<SlotRef>);

/// How wide a panel of `across` slots is, its padding included.
fn panel_width(across: usize) -> f32 {
    across as f32 * SLOT_PX + (across - 1) as f32 * GAP_PX + 2.0 * PAD_PX
}

fn slot_row(parent: &mut ChildSpawnerCommands, holder: Holder, from: usize, to: usize) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(GAP_PX),
            ..default()
        })
        .with_children(|row| {
            for index in from..to {
                row.spawn((
                    SlotUi(SlotRef { holder, index }),
                    SlotIcons::default(),
                    Node {
                        width: Val::Px(SLOT_PX),
                        height: Val::Px(SLOT_PX),
                        padding: UiRect::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(SLOT_COLOR),
                    // The words over the icons.
                    children![(Text::new(""), TextFont::from_font_size(11.0), ZIndex(1), Pickable::IGNORE)],
                ))
                .observe(hover_slot)
                .observe(leave_slot)
                .observe(click_slot)
                .observe(drag_from_slot)
                .observe(drop_on_slot);
            }
        });
}

/// A panel of `slots` slots of `holder`, `across` to a row, placed by
/// `place`.
fn panel(parent: &mut ChildSpawnerCommands, holder: Holder, slots: usize, across: usize, place: Node) {
    parent
        .spawn((
            Panel(holder),
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(GAP_PX),
                padding: UiRect::all(Val::Px(PAD_PX)),
                ..place
            },
            BackgroundColor(PANEL_COLOR),
        ))
        .with_children(|p| {
            for from in (0..slots).step_by(across) {
                slot_row(p, holder, from, (from + across).min(slots));
            }
        });
}

pub fn setup(mut commands: Commands) {
    // The note, top left.
    commands.spawn((
        NotePanel,
        Text::new(""),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(40.0),
            left: Val::Px(40.0),
            max_width: Val::Px(520.0),
            padding: UiRect::all(Val::Px(16.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.08, 0.07, 0.06, 0.9)),
        Visibility::Hidden,
    ));
    // Every panel is placed from the middle of the screen: the belt and
    // the inventory centred on it, the character's column to its left,
    // the open box to its right, all standing on the belt.
    let half = panel_width(ACROSS) / 2.0;
    let over_belt = MARGIN_PX + SLOT_PX + 2.0 * PAD_PX + MARGIN_PX;
    let centred = |bottom: f32| Node {
        position_type: PositionType::Absolute,
        bottom: Val::Px(bottom),
        left: Val::Percent(50.0),
        margin: UiRect::left(Val::Px(-half)),
        ..default()
    };
    let whole_screen = || Node {
        position_type: PositionType::Absolute,
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        ..default()
    };
    // The belt, always, along the bottom.
    commands.spawn((whole_screen(), Pickable::IGNORE)).with_children(|root| {
        panel(root, Holder::Hotbar, HOTBAR_SLOTS, HOTBAR_SLOTS, centred(MARGIN_PX));
    });
    commands
        .spawn((InventoryScreen, whole_screen(), Pickable::IGNORE, Visibility::Hidden))
        .with_children(|screen| {
            // The character, the equipment row under it.
            screen
                .spawn(Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(over_belt),
                    right: Val::Percent(50.0),
                    margin: UiRect::right(Val::Px(half + MARGIN_PX)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(GAP_PX),
                    ..default()
                })
                .with_children(|column| {
                    column.spawn((
                        Node {
                            width: Val::Px(panel_width(EQUIPMENT_SLOTS)),
                            height: Val::Px(CHARACTER_SLOTS_TALL * SLOT_PX),
                            ..default()
                        },
                        BackgroundColor(PANEL_COLOR),
                    ));
                    panel(column, Holder::Equipment, EQUIPMENT_SLOTS, EQUIPMENT_SLOTS, Node::default());
                });
            panel(screen, Holder::Inventory, INVENTORY_SLOTS, ACROSS, centred(over_belt));
            panel(
                screen,
                Holder::Container,
                CONTAINER_SLOTS,
                ACROSS,
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(over_belt),
                    left: Val::Percent(50.0),
                    margin: UiRect::left(Val::Px(half + MARGIN_PX)),
                    ..default()
                },
            );
            // The item information panel, over the inventory.
            let inventory_tall = (INVENTORY_SLOTS / ACROSS) as f32 * (SLOT_PX + GAP_PX) - GAP_PX + 2.0 * PAD_PX;
            screen
                .spawn((
                    ItemPanel::default(),
                    Node {
                        width: Val::Px(panel_width(ACROSS)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        padding: UiRect::all(Val::Px(PAD_PX)),
                        ..centred(over_belt + inventory_tall + MARGIN_PX)
                    },
                    BackgroundColor(PANEL_COLOR),
                    Visibility::Hidden,
                ))
                .with_children(|p| {
                    p.spawn(Node {
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|top| {
                        top.spawn(Node {
                            flex_direction: FlexDirection::Column,
                            flex_shrink: 1.0,
                            ..default()
                        })
                        .with_children(|words| {
                            words.spawn((PanelText::Name, Text::new(""), TextFont::from_font_size(16.0)));
                            words.spawn((PanelText::Description, Text::new(""), TextFont::from_font_size(11.0)));
                        });
                        top.spawn((
                            SlotIcons::default(),
                            Node {
                                width: Val::Px(SLOT_PX),
                                height: Val::Px(SLOT_PX),
                                flex_shrink: 0.0,
                                ..default()
                            },
                        ));
                    });
                    p.spawn(Node {
                        column_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|boxes| {
                        let heading = |text: &str| (Text::new(text), TextFont::from_font_size(10.0));
                        boxes
                            .spawn((
                                Node {
                                    flex_direction: FlexDirection::Column,
                                    flex_grow: 1.0,
                                    padding: UiRect::all(Val::Px(4.0)),
                                    ..default()
                                },
                                BackgroundColor(SLOT_COLOR),
                            ))
                            .with_children(|info| {
                                info.spawn(heading("INFORMATION"));
                                info.spawn((PanelText::Information, Text::new(""), TextFont::from_font_size(12.0)));
                            });
                        boxes
                            .spawn((
                                Node {
                                    flex_direction: FlexDirection::Column,
                                    flex_grow: 1.0,
                                    row_gap: Val::Px(4.0),
                                    padding: UiRect::all(Val::Px(4.0)),
                                    ..default()
                                },
                                BackgroundColor(SLOT_COLOR),
                            ))
                            .with_children(|acts| {
                                acts.spawn(heading("ACTIONS"));
                                for n in 0..ACTION_BUTTONS {
                                    acts.spawn((ActionButton(n, None), button_node(), BackgroundColor(HELD_COLOR)))
                                        .with_child((PanelText::Action(n), Text::new(""), TextFont::from_font_size(12.0), Pickable::IGNORE))
                                        .observe(click_action);
                                }
                            });
                    });
                    p.spawn((
                        SplitRow,
                        Node {
                            column_gap: Val::Px(6.0),
                            align_items: AlignItems::Center,
                            ..default()
                        },
                    ))
                    .with_children(|row| {
                        row.spawn((Text::new("SPLITTING"), TextFont::from_font_size(10.0)));
                        for (step, label) in [(-1, "-"), (1, "+")] {
                            if step == 1 {
                                row.spawn((PanelText::Split, Text::new(""), TextFont::from_font_size(12.0)));
                            }
                            row.spawn((SplitButton(step), button_node(), BackgroundColor(HELD_COLOR)))
                                .with_child((Text::new(label), TextFont::from_font_size(12.0), Pickable::IGNORE))
                                .observe(click_split);
                        }
                        row.spawn((Text::new("then drag"), TextFont::from_font_size(10.0)));
                    });
                });
        });
}

/// A button of the item information panel.
fn button_node() -> Node {
    Node {
        padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
        justify_content: JustifyContent::Center,
        ..default()
    }
}

/// An ACTIONS button clicked: the player does it to the selected slot.
fn click_action(
    click: On<Pointer<Click>>,
    buttons: Query<&ActionButton>,
    tick: Res<crate::SimTick>,
    mut players: Query<(&Hud, &mut Actions), With<Player>>,
) {
    let (Ok(ActionButton(_, Some(action))), Ok((hud, mut actions))) = (buttons.get(click.entity), players.single_mut()) else {
        return;
    };
    let Some(slot) = hud.0.selected else {
        return;
    };
    let action = match action {
        PanelAction::Use(_) => Action::UseSlot { slot },
        PanelAction::Drop => Action::Drop { slot: Some(slot) },
    };
    actions.0.push(tick.0 + 1, action);
}

/// A splitting bar button clicked: the amount one less or one more, from
/// the whole stack when none is set.
fn click_split(click: On<Pointer<Click>>, buttons: Query<&SplitButton>, mut commands: Commands) {
    if let Ok(button) = buttons.get(click.entity) {
        commands.run_system_cached_with(step_split, button.0);
    }
}

/// The player's splitting bar moved by `step`.
pub fn step_split(step: In<i32>, mut holders: PlayerHolders) {
    holders.with(|hud, h| {
        let Some(count) = hud.selected.and_then(|slot| h.stack(slot)).map(|s| s.count) else {
            return;
        };
        let now = hud.split.unwrap_or(count) as i32;
        set_split(hud, h, (now + *step).max(1) as u32);
    });
}

/// Paint every slot from its holder, the held and hovered slots
/// marked, and show the screen and the box modforge says are open.
pub fn render(
    mut commands: Commands,
    assets: Res<AssetServer>,
    items: Res<ItemRegistryRes>,
    layers: Res<crate::content::LayerRegistryRes>,
    hovered: Res<HoveredSlot>,
    players: Query<(&Inventory, &Hotbar, &Equipment, &Hud, &OpenContainer), With<Player>>,
    containers: Query<&Container>,
    mut slots: Query<(Entity, &SlotUi, &mut SlotIcons, &Children, &mut BackgroundColor, &mut Visibility)>,
    mut texts: Query<&mut Text, Without<NotePanel>>,
    mut screen: Query<&mut Visibility, (With<InventoryScreen>, Without<SlotUi>, Without<Panel>)>,
    mut panels: Query<(&Panel, &mut Visibility), (Without<SlotUi>, Without<InventoryScreen>)>,
    mut note: Query<(&mut Text, &mut Visibility), (With<NotePanel>, Without<SlotUi>, Without<InventoryScreen>, Without<Panel>)>,
) {
    let Ok((inventory, hotbar, worn, hud, open)) = players.single() else {
        return;
    };
    let container = open.0.and_then(|e| containers.get(e).ok());
    let inventory_open = hud.0.open_panel == OpenPanel::Inventory;
    if let Ok(mut v) = screen.single_mut() {
        *v = if inventory_open { Visibility::Visible } else { Visibility::Hidden };
    }
    for (panel, mut v) in &mut panels {
        if panel.0 == Holder::Container {
            *v = if container.is_some() { Visibility::Inherited } else { Visibility::Hidden };
        }
    }
    for (entity, slot, mut icons, children, mut color, mut visibility) in &mut slots {
        if slot.0.holder == Holder::Container {
            let exists = container.is_some_and(|c| slot.0.index < c.0.slots.len());
            *visibility = if exists { Visibility::Inherited } else { Visibility::Hidden };
        }
        let stack = stack_at(slot.0, inventory, hotbar, worn, container);
        let item = stack.map(|s| s.item.clone());
        if icons.item != item {
            icons.item = item;
        }
        let wanted = stack.map(|s| icons_of(s, &items.0, &layers.0)).unwrap_or_default();
        show_icons(&mut commands, &assets, entity, &mut icons, wanted);
        // With an icon, only the count is written; without, the name too.
        let label = match stack {
            Some(s) if !icons.shown.is_empty() => if s.count > 1 { format!("x{}", s.count) } else { String::new() },
            _ => slot_label(stack),
        };
        let label = if slot.0.holder == Holder::Hotbar {
            format!("{}\n{}", slot.0.index + 1, label)
        } else {
            label
        };
        if let Some(mut text) = children.first().and_then(|c| texts.get_mut(*c).ok()) {
            text.0 = label;
        }
        let held = slot.0.holder == Holder::Hotbar && hotbar.0.held == Some(slot.0.index);
        color.0 = if hovered.0 == Some(slot.0) || hud.0.selected == Some(slot.0) {
            HOVER_COLOR
        } else if held {
            HELD_COLOR
        } else {
            SLOT_COLOR
        };
    }
    if let Ok((mut text, mut visibility)) = note.single_mut() {
        match (&hud.0.open_panel, &hud.0.reading) {
            (OpenPanel::Note, Some(n)) => {
                text.0 = format!("{}\n\n  {}", n.text, n.signed);
                *visibility = Visibility::Visible;
            }
            _ => *visibility = Visibility::Hidden,
        }
    }
}

/// Fill the item information panel from the selected slot: its name,
/// description, icons, INFORMATION, ACTIONS, and for a stack the
/// splitting bar. Hidden with nothing selected.
pub fn render_item_panel(
    mut commands: Commands,
    assets: Res<AssetServer>,
    items: Res<ItemRegistryRes>,
    layers: Res<crate::content::LayerRegistryRes>,
    players: Query<(&Inventory, &Hotbar, &Equipment, &Hud, &OpenContainer), With<Player>>,
    containers: Query<&Container>,
    mut panels: Query<(&mut ItemPanel, &mut Visibility)>,
    mut icons: Query<(Entity, &mut SlotIcons), Without<SlotUi>>,
    mut texts: Query<(&PanelText, &mut Text)>,
    mut buttons: Query<(&mut ActionButton, &mut Visibility), Without<ItemPanel>>,
    mut split_rows: Query<&mut Visibility, (With<SplitRow>, Without<ItemPanel>, Without<ActionButton>)>,
) {
    let (Ok((inventory, hotbar, worn, hud, open)), Ok((mut panel, mut visibility))) = (players.single(), panels.single_mut()) else {
        return;
    };
    let container = open.0.and_then(|e| containers.get(e).ok());
    let shown = hud.0.selected.and_then(|slot| {
        let stack = stack_at(slot, inventory, hotbar, worn, container)?;
        let def = items.0.def(&stack.item)?;
        Some((
            Shown {
                slot,
                name: def.name.to_uppercase(),
                description: def.description.clone(),
                information: information(def, stack, &layers.0),
                actions: panel_actions(def, slot),
                count: stack.count,
                split: hud.0.split,
            },
            icons_of(stack, &items.0, &layers.0),
        ))
    });
    let Some((shown, wanted)) = shown else {
        visibility.set_if_neq(Visibility::Hidden);
        panel.shown = None;
        return;
    };
    visibility.set_if_neq(Visibility::Inherited);
    if let Ok((node, mut icons)) = icons.single_mut() {
        show_icons(&mut commands, &assets, node, &mut icons, wanted);
    }
    if panel.shown.as_ref() == Some(&shown) {
        return;
    }
    for (which, mut text) in &mut texts {
        text.0 = match which {
            PanelText::Name => shown.name.clone(),
            PanelText::Description => shown.description.clone(),
            PanelText::Information => shown.information.join("\n"),
            PanelText::Split => format!("{} of {}", shown.split.unwrap_or(shown.count), shown.count),
            PanelText::Action(n) => shown.actions.get(*n).map(|a| a.label().to_string()).unwrap_or_default(),
        };
    }
    for (mut button, mut v) in &mut buttons {
        button.1 = shown.actions.get(button.0).copied();
        *v = if button.1.is_some() { Visibility::Inherited } else { Visibility::Hidden };
    }
    for mut v in &mut split_rows {
        *v = if shown.count > 1 { Visibility::Inherited } else { Visibility::Hidden };
    }
    panel.shown = Some(shown);
}

/// The vitals line: health, hunger, thirst, always shown.
#[derive(Component)]
pub struct VitalsText;

pub fn setup_vitals(mut commands: Commands) {
    commands.spawn((
        VitalsText,
        Text::new(""),
        TextFont::from_font_size(18.0),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
    ));
}

pub fn render_vitals(
    tick: Res<crate::SimTick>,
    day: Res<crate::survival::Day>,
    players: Query<(&crate::combat::Health, &Survival), With<Player>>,
    mut texts: Query<&mut Text, With<VitalsText>>,
) {
    let (Ok((health, needs)), Ok(mut text)) = (players.single(), texts.single_mut()) else {
        return;
    };
    let time = match crate::storm::phase(&tick, *day) {
        StormPhase::Storm => "THE STORM",
        StormPhase::Warning => "The sky has changed",
        StormPhase::Calm if crate::survival::night(&tick, *day) => "Night",
        StormPhase::Calm => "Day",
    };
    text.0 = format!(
        "Health {:.0}   Hunger {:.0}   Thirst {:.0}   {time}",
        health.0.current, needs.0.hunger, needs.0.thirst
    );
}

/// The sky laid over the world (under the HUD): dark at night, a
/// warning tint when the storm is coming, near white while it rages.
#[derive(Component)]
pub struct SkyLayer;

/// How dark the night is. Tuning.
const NIGHT_DARK: f32 = 0.55;

pub fn setup_sky(mut commands: Commands) {
    commands.spawn((
        SkyLayer,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(Color::NONE),
        Pickable::IGNORE,
        GlobalZIndex(-1),
    ));
}

pub fn render_sky(
    tick: Res<crate::SimTick>,
    day: Res<crate::survival::Day>,
    mut layers: Query<&mut BackgroundColor, With<SkyLayer>>,
) {
    let sky = match crate::storm::phase(&tick, *day) {
        StormPhase::Storm => Color::srgba(0.95, 0.93, 0.88, 0.85),
        StormPhase::Warning => Color::srgba(0.7, 0.3, 0.05, 0.3),
        StormPhase::Calm if crate::survival::night(&tick, *day) => Color::srgba(0.0, 0.02, 0.08, NIGHT_DARK),
        StormPhase::Calm => Color::NONE,
    };
    for mut color in &mut layers {
        color.0 = sky;
    }
}

/// A click on a slot selects it (modforge's `select_slot`): its item
/// shows in the item information panel, Rust's. Using it is the panel's
/// Eat, Drink, Use, or Wear.
fn click_slot(click: On<Pointer<Click>>, slots: Query<&SlotUi>, mut commands: Commands) {
    if let Ok(slot) = slots.get(click.entity) {
        commands.run_system_cached_with(select, slot.0);
    }
}

/// The player selects `slot`.
pub fn select(slot: In<SlotRef>, mut holders: PlayerHolders) {
    holders.with(|hud, h| select_slot(hud, h, *slot));
}

/// Heal by what food or medicine gives, never past full.
fn heal_by(health: &mut crate::combat::Health, heal: f32) {
    health.0.current = (health.0.current + heal).min(health.0.max);
}

/// Someone just used `item`, and modforge (`use_slot`, `hotbar_key`, or
/// `SurvivalStats::eat`) gave `heal`: the heal goes on, and if it was
/// food or drink it goes into their memory as eaten (life.md "What a
/// person did"). Every eat, drink, and heal ends here: the hotbar, a slot,
/// a box, a well, a river, a bush.
pub(crate) fn used(
    item: &str,
    heal: f32,
    registry: &ItemRegistryRes,
    health: &mut crate::combat::Health,
    memory: Option<&mut crate::actor::Memory>,
    now: u64,
) {
    heal_by(health, heal);
    if registry.0.def(item).is_some_and(|d| d.food.is_some())
        && let Some(memory) = memory
    {
        memory.0.did(Did::Ate(item.to_string()), now);
    }
}

/// The death message, shown while the player is dead.
#[derive(Component)]
pub struct DeathPanel;

pub fn setup_death(mut commands: Commands) {
    commands
        .spawn((
            DeathPanel,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            Visibility::Hidden,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("You died.\n\nEverything you carried lies where you fell.\n\nPress Enter to respawn."),
                TextFont::from_font_size(28.0),
                TextLayout::justify(Justify::Center),
            ));
        });
}

pub fn render_death(
    players: Query<Has<Dead>, With<Player>>,
    mut panels: Query<&mut Visibility, With<DeathPanel>>,
) {
    let dead = players.single().unwrap_or(false);
    if let Ok(mut v) = panels.single_mut() {
        *v = if dead { Visibility::Visible } else { Visibility::Hidden };
    }
}

fn hover_slot(over: On<Pointer<Over>>, slots: Query<&SlotUi>, mut hovered: ResMut<HoveredSlot>) {
    if let Ok(slot) = slots.get(over.entity) {
        hovered.0 = Some(slot.0);
    }
}

fn leave_slot(out: On<Pointer<Out>>, slots: Query<&SlotUi>, mut hovered: ResMut<HoveredSlot>) {
    if slots.get(out.entity).is_ok_and(|s| hovered.0 == Some(s.0)) {
        hovered.0 = None;
    }
}

fn drag_from_slot(start: On<Pointer<DragStart>>, slots: Query<&SlotUi>, mut commands: Commands) {
    if let Ok(slot) = slots.get(start.entity) {
        commands.run_system_cached_with(drag_start, slot.0);
    }
}

fn drop_on_slot(dropped: On<Pointer<DragDrop>>, slots: Query<&SlotUi>, mut commands: Commands) {
    if let Ok(slot) = slots.get(dropped.entity) {
        commands.run_system_cached_with(drag_drop, slot.0);
    }
}

/// The player's holders, panels, and open box, borrowed together.
#[derive(bevy::ecs::system::SystemParam)]
pub struct PlayerHolders<'w, 's> {
    players: Query<
        'w,
        's,
        (
            &'static mut Inventory,
            &'static mut Hotbar,
            &'static mut Equipment,
            &'static mut Hud,
            &'static OpenContainer,
        ),
        With<Player>,
    >,
    containers: Query<'w, 's, &'static mut Container>,
}

impl PlayerHolders<'_, '_> {
    fn with<R>(&mut self, f: impl FnOnce(&mut HudState, &mut Holders<'_>) -> R) -> Option<R> {
        let (mut inventory, mut hotbar, mut worn, mut hud, open) = self.players.single_mut().ok()?;
        let mut container = open.0.and_then(|e| self.containers.get_mut(e).ok());
        let mut holders = Holders {
            inventory: &mut inventory.0,
            hotbar: &mut hotbar.0.slots,
            container: container.as_mut().map(|c| &mut c.0),
            equipment: &mut worn.0,
        };
        Some(f(&mut hud.0, &mut holders))
    }
}

/// A drag starts on `slot` (modforge's `start_drag`). Dragging is the
/// mouse's way to move a stack; the brain moves stacks with Transfer.
pub fn drag_start(slot: In<SlotRef>, mut holders: PlayerHolders) {
    holders.with(|hud, h| start_drag(hud, h, *slot));
}

/// A drag ends on `slot` (modforge's `end_drag`).
pub fn drag_drop(slot: In<SlotRef>, registry: Res<ItemRegistryRes>, mut holders: PlayerHolders) {
    holders.with(|hud, h| end_drag(hud, h, *slot, &registry.0));
}

/// Every person's panel, hotbar, and slot actions this tick, the
/// player's keys and clicks and the brain's alike, each through
/// modforge: Inventory toggles the inventory, Cancel closes whatever is
/// open, Hotbar holds that slot, Transfer moves a stack across between
/// the person and the box they have open, UseSlot eats, drinks, or puts
/// on, Craft makes a recipe of the station they have open.
pub fn hud_keys(
    mut commands: Commands,
    tick: Res<crate::SimTick>,
    registry: Res<ItemRegistryRes>,
    recipes: Res<RecipeRegistryRes>,
    mut people: Query<
        (
            (&ThisTick, &Transform),
            (&mut Hud, &mut OpenContainer, &mut OpenStation),
            &mut Survival,
            &mut Hotbar,
            &mut Equipment,
            &mut Inventory,
            &mut crate::combat::Health,
            Option<&mut crate::actor::Memory>,
        ),
        Without<Dead>,
    >,
    mut containers: Query<&mut Container>,
) {
    for (
        (this_tick, at),
        (mut hud, mut open, mut station),
        mut needs,
        mut hotbar,
        mut worn,
        mut inventory,
        mut health,
        mut memory,
    ) in &mut people
    {
        for stamped in &this_tick.0 {
            match stamped.action {
                Action::Inventory => toggle_panel(&mut hud.0, OpenPanel::Inventory),
                Action::Cancel => hud.0.open_panel = OpenPanel::None,
                Action::Hotbar { index } => {
                    let item = hotbar.0.slots.slots.get(index).and_then(|s| s.as_ref()).map(|s| s.item.clone());
                    if let Ok(heal) = hotbar_key(&mut needs.0, &mut hotbar.0, index, &registry.0, &mut worn.0)
                        && let Some(item) = item
                    {
                        used(&item, heal, &registry, &mut health, memory.as_deref_mut(), tick.0);
                    }
                }
                Action::Transfer { half, slot: Some(slot) } => {
                    let mut container = open.0.and_then(|e| containers.get_mut(e).ok());
                    let with_box = container.is_some();
                    let mut holders = Holders {
                        inventory: &mut inventory.0,
                        hotbar: &mut hotbar.0.slots,
                        container: container.as_mut().map(|c| &mut c.0),
                        equipment: &mut worn.0,
                    };
                    let before = stack_in(&holders, slot);
                    transfer_slot(&mut holders, slot, half, &registry.0);
                    // Out of the open box: taken; into it: put in.
                    if let (true, Some((item, count))) = (with_box, before) {
                        let left = stack_in(&holders, slot).filter(|(i, _)| *i == item).map_or(0, |(_, n)| n);
                        let moved = count.saturating_sub(left);
                        // What they did goes into their memory (life.md
                        // "What a person did").
                        if moved > 0
                            && let Some(memory) = memory.as_mut()
                        {
                            let what = if slot.holder == Holder::Container { Did::Took(item, moved) } else { Did::Stocked(item, moved) };
                            memory.0.did(what, tick.0);
                        }
                    }
                }
                // Out of any holder, worn gear too, onto the ground at the
                // feet.
                Action::Drop { slot: Some(slot) } => {
                    let mut container = open.0.and_then(|e| containers.get_mut(e).ok());
                    let mut holders = Holders {
                        inventory: &mut inventory.0,
                        hotbar: &mut hotbar.0.slots,
                        container: container.as_mut().map(|c| &mut c.0),
                        equipment: &mut worn.0,
                    };
                    if let Some(stack) = drop_from(&mut holders, slot)
                        && let Some(def) = registry.0.def(&stack.item)
                    {
                        crate::interact::spawn_item(&mut commands, def, stack, at.translation.ground());
                    }
                }
                // In the open box too: you eat what you need where it is.
                Action::UseSlot { slot } => {
                    let mut container = open.0.and_then(|e| containers.get_mut(e).ok());
                    let holder = match slot.holder {
                        Holder::Inventory => &mut inventory.0,
                        Holder::Hotbar => &mut hotbar.0.slots,
                        Holder::Container => match container.as_mut() {
                            Some(c) => &mut c.0,
                            None => continue,
                        },
                        // Worn gear comes off by dragging, not by a click.
                        Holder::Equipment => continue,
                    };
                    let item = holder.slots.get(slot.index).and_then(|s| s.as_ref()).map(|s| s.item.clone());
                    if let Ok(heal) = use_slot(&mut needs.0, holder, slot.index, &registry.0, &mut worn.0)
                        && let Some(item) = item
                    {
                        used(&item, heal, &registry, &mut health, memory.as_deref_mut(), tick.0);
                    }
                }
                Action::Craft { index } => {
                    let Some(recipe) = station.0.and_then(|kind| recipes.0.for_station(kind).get(index).copied()) else {
                        continue;
                    };
                    // modforge's `craft`, the one way anything is made; what
                    // is made goes into the inventory, or at the feet when it
                    // is full.
                    let Some(made) = craft(recipe, &mut inventory.0, &registry.0, &[], tick.0 as f32, tick.0) else {
                        continue;
                    };
                    let Some(def) = registry.0.def(&made.item) else {
                        continue;
                    };
                    if let Some(left) = inventory.0.add(made, def.max_stack) {
                        crate::interact::spawn_item(&mut commands, def, left, at.translation.ground());
                    }
                }
                _ => {}
            }
        }
        // A closed inventory closes the box with it; a closed station
        // panel, the station.
        if hud.0.open_panel != OpenPanel::Inventory {
            open.0 = None;
            hud.0.selected = None;
            hud.0.split = None;
        }
        if hud.0.open_panel != OpenPanel::Station {
            station.0 = None;
        }
    }
}

/// The item and how many are in `slot`, if anything is.
fn stack_in(holders: &Holders, slot: SlotRef) -> Option<(String, u32)> {
    holders
        .get(slot.holder)
        .and_then(|h| h.slots.get(slot.index))
        .and_then(|s| s.as_ref())
        .map(|s| (s.item.clone(), s.count))
}

/// The station panel: every recipe of the open station, one row each,
/// what it takes and what it makes; a click crafts it.
#[derive(Component)]
pub struct StationPanel;

/// One recipe row and which recipe of the open station's list it is.
#[derive(Component)]
pub struct RecipeRow(pub usize);

/// The most recipe rows a station panel shows.
const RECIPE_ROWS: usize = 12;

pub fn setup_station(mut commands: Commands) {
    commands
        .spawn((
            StationPanel,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(60.0),
                right: Val::Px(40.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.05, 0.85)),
            Visibility::Hidden,
        ))
        .with_children(|p| {
            for _ in 0..RECIPE_ROWS {
                p.spawn((
                    RecipeRow(0),
                    Node {
                        padding: UiRect::all(Val::Px(6.0)),
                        ..default()
                    },
                    BackgroundColor(SLOT_COLOR),
                    children![(Text::new(""), TextFont::from_font_size(14.0), Pickable::IGNORE)],
                ))
                .observe(click_recipe);
            }
        });
}

/// Fill the station panel from the open station's recipes: a recipe the
/// inventory can pay for is lit.
pub fn render_station(
    recipes: Res<RecipeRegistryRes>,
    players: Query<(&Inventory, &OpenStation), With<Player>>,
    mut panels: Query<&mut Visibility, (With<StationPanel>, Without<RecipeRow>)>,
    mut rows: Query<(&mut RecipeRow, &Children, &mut BackgroundColor, &mut Visibility)>,
    mut texts: Query<&mut Text>,
) {
    let (Ok(mut panel), Ok((inventory, station))) = (panels.single_mut(), players.single()) else {
        return;
    };
    let Some(kind) = station.0 else {
        *panel = Visibility::Hidden;
        return;
    };
    *panel = Visibility::Visible;
    let list = recipes.0.for_station(kind);
    for (i, (mut row, children, mut color, mut visibility)) in rows.iter_mut().enumerate() {
        let Some(recipe) = list.get(i) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        *visibility = Visibility::Inherited;
        row.0 = i;
        let takes: Vec<String> = recipe.inputs.iter().map(|(item, n)| format!("{item} x{n}")).collect();
        if let Some(mut text) = children.first().and_then(|c| texts.get_mut(*c).ok()) {
            text.0 = format!("{} x{}  <-  {}", recipe.output.0, recipe.output.1, takes.join(", "));
        }
        color.0 = if can_craft(recipe, &inventory.0) { HELD_COLOR } else { SLOT_COLOR };
    }
}

fn click_recipe(
    click: On<Pointer<Click>>,
    rows: Query<&RecipeRow>,
    tick: Res<crate::SimTick>,
    mut players: Query<&mut Actions, With<Player>>,
) {
    if let (Ok(row), Ok(mut actions)) = (rows.get(click.entity), players.single_mut()) {
        actions.0.push(tick.0 + 1, Action::Craft { index: row.0 });
    }
}
