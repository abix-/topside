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

use crate::actor::{Dead, Hotbar, Inventory, INVENTORY_SLOTS};
use modforge::memory::Did;
use crate::combat::Equipment;
use modforge::crafting::{can_craft, craft};
use modforge::storm::StormPhase;

use crate::content::{ItemRegistryRes, RecipeRegistryRes};
use crate::input::{Action, Actions, ThisTick};
use crate::interact::{Container, Hud, OpenContainer, OpenStation};
use crate::player::Player;

/// What the player's last act came to, read from their memory (a
/// `Did::Done`, `Did::Failed` or `Did::Ate`, written by the path that did
/// it), on the prompt line for as long as its words take to read (modforge
/// `talk::reading_secs`, the one reading rule): "drank at the tap", "could
/// not open the gate: locked". Runs after `talk::look_at` writes the prompt.
pub fn show_act_result(tick: Res<crate::SimTick>, registry: Res<ItemRegistryRes>, mut players: Query<(&mut Hud, &crate::actor::Persona), With<Player>>) {
    let Ok((mut hud, memory)) = players.single_mut() else { return };
    let last = memory.0.memory.done.iter().rev().find(|(_, d)| matches!(d, Did::Done(_) | Did::Failed(..) | Did::Ate(_)));
    let Some((at, did)) = last else { return };
    let words = act_words(did, &registry);
    if tick.0 < at + crate::talk::ticks_of(modforge::talk::reading_secs(&words)) {
        hud.0.prompt = modforge::hud::Prompt { text: words, can_interact: false };
    }
}

/// A thing done in words, as memory's own (`Did::words`), but a drink said
/// as one: water from a source is "drank at the tap", not "ate tap".
pub fn act_words(did: &Did, registry: &ItemRegistryRes) -> String {
    if let Did::Ate(item) = did
        && let Some(def) = registry.0.def(item)
        && def.food.is_some_and(|f| f.thirst > 0.0 && f.hunger <= 0.0)
    {
        return match def.kind {
            modforge::item::ItemKind::Source => format!("drank at the {item}"),
            _ => format!("drank {item}"),
        };
    }
    did.words()
}

/// A note read closes once it has been read: as long after the reading
/// (the `Did::Read` in the reader's memory) as its words take, by the one
/// reading rule talk lines use (modforge `talk::reading_secs`; todo: close a
/// read note by the reading rule, 2026-11-19).
pub fn close_read_notes(tick: Res<crate::SimTick>, mut readers: Query<(&mut Hud, &crate::actor::Persona)>) {
    for (mut hud, memory) in &mut readers {
        let Some(note) = hud.0.reading.as_ref().filter(|_| hud.0.open_panel == OpenPanel::Note) else { continue };
        let Some(read_at) = memory.0.memory.done.iter().rev().find_map(|(t, d)| matches!(d, Did::Read(_)).then_some(*t)) else { continue };
        if tick.0 >= read_at + crate::talk::ticks_of(modforge::talk::reading_secs(&note.text)) {
            toggle_panel(&mut hud.0, OpenPanel::Note);
            hud.0.reading = None;
        }
    }
}

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
/// A stack put on the table in a trade.
const OFFER_COLOR: Color = Color::srgba(0.2, 0.42, 0.25, 0.95);

/// The trade screen (trade.rs): the player's inventory and the other's,
/// side by side, the deal between them.
#[derive(Component)]
pub struct TradeScreen;

/// The trade screen's words: what each side puts on the table, what it is
/// worth, and their answer.
#[derive(Component)]
pub struct TradeText;

/// The trade screen's Deal button.
#[derive(Component)]
pub struct DealButton;

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

/// What is in `slot` of the player's holders, or of the one they trade
/// with (`theirs`), if anything.
fn stack_at<'a>(
    slot: SlotRef,
    inventory: &'a Inventory,
    hotbar: &'a Hotbar,
    worn: &'a Equipment,
    container: Option<&'a Container>,
    theirs: Option<&'a Inventory>,
) -> Option<&'a modforge::item::ItemStack> {
    let holder = match slot.holder {
        Holder::Inventory => &inventory.0,
        Holder::Hotbar => &hotbar.0.slots,
        Holder::Container => &container?.0,
        Holder::Equipment => return worn.0.get(slot.index),
        Holder::Theirs => &theirs?.0,
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
    // Its quality tier, when better than the base (design.md "Loot
    // randomization").
    let tier = stack.quality.map(|q| q.tier);
    if let Some(name) = modforge::quality::name(tier, &crate::content::QUALITY) {
        lines.push(format!("Quality {name}"));
        if numbers.combat.is_some() {
            lines.push(format!("Hits x{:.2}", modforge::quality::power(tier, &crate::content::QUALITY)));
        }
    }
    // Its affixes, each by name.
    if !stack.affixes.is_empty() {
        let names: Vec<&str> = stack.affixes.iter().map(|a| a.name.as_str()).collect();
        lines.push(format!("Affixes {}", names.join(", ")));
    }
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
    /// A container filled at the source in reach (Action::Fill).
    Fill,
    /// A container poured into the open box's, or from it (Action::Pour).
    Pour,
    Drop,
}

impl PanelAction {
    pub fn label(self) -> &'static str {
        match self {
            PanelAction::Use(name) => name,
            PanelAction::Fill => "Fill",
            PanelAction::Pour => "Pour",
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
    } else if def.holds_litres.is_some() {
        // A container (modforge fluid): drunk from, filled at a source,
        // poured into or out of the open box.
        actions.extend([PanelAction::Use("Drink"), PanelAction::Fill, PanelAction::Pour]);
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
    // The trade screen: the player's inventory left of the middle, the
    // other's right of it, the deal over them (Fallout's barter screen).
    commands
        .spawn((TradeScreen, whole_screen(), Pickable::IGNORE, Visibility::Hidden))
        .with_children(|screen| {
            let side = |left: bool| Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(over_belt),
                left: if left { Val::Auto } else { Val::Percent(50.0) },
                right: if left { Val::Percent(50.0) } else { Val::Auto },
                margin: if left { UiRect::right(Val::Px(MARGIN_PX / 2.0)) } else { UiRect::left(Val::Px(MARGIN_PX / 2.0)) },
                ..default()
            };
            panel(screen, Holder::Inventory, INVENTORY_SLOTS, ACROSS, side(true));
            panel(screen, Holder::Theirs, INVENTORY_SLOTS, ACROSS, side(false));
            let inventory_tall = (INVENTORY_SLOTS / ACROSS) as f32 * (SLOT_PX + GAP_PX) - GAP_PX + 2.0 * PAD_PX;
            screen
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        bottom: Val::Px(over_belt + inventory_tall + MARGIN_PX),
                        left: Val::Percent(50.0),
                        margin: UiRect::left(Val::Px(-panel_width(ACROSS))),
                        width: Val::Px(2.0 * panel_width(ACROSS)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        padding: UiRect::all(Val::Px(PAD_PX)),
                        ..default()
                    },
                    BackgroundColor(PANEL_COLOR),
                ))
                .with_children(|bar| {
                    bar.spawn((TradeText, Text::new(""), TextFont::from_font_size(14.0)));
                    bar.spawn((DealButton, button_node(), BackgroundColor(HELD_COLOR)))
                        .with_child((Text::new("Deal"), TextFont::from_font_size(14.0), Pickable::IGNORE))
                        .observe(click_deal);
                });
        });
}

/// The Deal button clicked: the player puts the deal to the one they trade
/// with.
fn click_deal(_: On<Pointer<Click>>, tick: Res<crate::SimTick>, mut players: Query<&mut Actions, With<Player>>) {
    if let Ok(mut actions) = players.single_mut() {
        actions.0.push(tick.0 + 1, Action::Deal);
    }
}

/// The trade screen, shown while trading: what each side puts on the table
/// and what it is worth (modforge `trade::Values`), and whether the last
/// deal put was refused.
pub fn render_trade(
    values: Res<crate::content::ItemValuesRes>,
    players: Query<(&Hud, &Inventory), With<Player>>,
    people: Query<(&crate::actor::Actor, &Inventory), Without<Player>>,
    mut screen: Query<&mut Visibility, With<TradeScreen>>,
    mut words: Query<&mut Text, With<TradeText>>,
) {
    let Ok((hud, mine)) = players.single() else {
        return;
    };
    let trading = hud.0.trading.as_ref().filter(|_| hud.0.open_panel == OpenPanel::Trade);
    if let Ok(mut v) = screen.single_mut() {
        *v = if trading.is_some() { Visibility::Visible } else { Visibility::Hidden };
    }
    let (Some(t), Ok(mut text)) = (trading, words.single_mut()) else {
        return;
    };
    let theirs = people.iter().find(|(a, _)| a.0.0 == t.with).map(|(_, i)| i);
    let side = |inventory: Option<&Inventory>, offer: &modforge::trade::Offer| {
        let stacks = inventory.and_then(|i| modforge::trade::offered(&i.0, offer)).unwrap_or_default();
        let names: Vec<String> = stacks.iter().map(|s| if s.count > 1 { format!("{} x{}", s.item, s.count) } else { s.item.clone() }).collect();
        (if names.is_empty() { "nothing".to_string() } else { names.join(", ") }, values.0.worth(&stacks))
    };
    let (give, give_worth) = side(Some(mine), &t.mine);
    let (get, get_worth) = side(theirs, &t.theirs);
    let answer = if t.refused {
        format!("\n{}: \"Not for that.\"", t.name)
    } else if t.no_room {
        "\nNo room to carry it all.".to_string()
    } else {
        String::new()
    };
    text.0 = format!("Trading with {}\nYou give: {give} (worth {give_worth})\nYou get: {get} (worth {get_worth}){answer}", t.name);
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
        PanelAction::Fill => Action::Fill { slot },
        PanelAction::Pour => Action::Pour { slot },
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
    people: Query<(&crate::actor::Actor, &Inventory), Without<Player>>,
) {
    let Ok((inventory, hotbar, worn, hud, open)) = players.single() else {
        return;
    };
    let container = open.0.and_then(|e| containers.get(e).ok());
    // The one traded with, and what is on the table.
    let trading = hud.0.trading.as_ref();
    let theirs = trading.and_then(|t| people.iter().find(|(a, _)| a.0.0 == t.with)).map(|(_, i)| i);
    let on_table = |slot: SlotRef| {
        trading.is_some_and(|t| match slot.holder {
            Holder::Inventory => t.mine.iter().any(|&(s, _)| s == slot.index),
            Holder::Theirs => t.theirs.iter().any(|&(s, _)| s == slot.index),
            _ => false,
        })
    };
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
        if slot.0.holder == Holder::Theirs {
            let exists = theirs.is_some_and(|i| slot.0.index < i.0.slots.len());
            *visibility = if exists { Visibility::Inherited } else { Visibility::Hidden };
        }
        let stack = stack_at(slot.0, inventory, hotbar, worn, container, theirs);
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
        } else if on_table(slot.0) {
            OFFER_COLOR
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
        let stack = stack_at(slot, inventory, hotbar, worn, container, None)?;
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
    storms: Res<crate::storm::StormRes>,
    reality: Res<crate::Reality>,
    players: Query<(&crate::combat::Health, &crate::actor::Persona, Option<&crate::exposure::Exposure>, &Transform), With<Player>>,
    world: Option<Res<crate::terrain::WorldRes>>,
    biomes: Res<crate::content::BiomeRegistryRes>,
    mut texts: Query<&mut Text, With<VitalsText>>,
) {
    let (Ok((health, needs, exposure, at)), Ok(mut text)) = (players.single(), texts.single_mut()) else {
        return;
    };
    let time = match crate::storm::phase(&tick, *day, &storms) {
        StormPhase::Storm => "THE STORM",
        StormPhase::Warning => "The sky has changed",
        StormPhase::Calm if first_sky_change(&tick) => "The sky has changed",
        StormPhase::Calm if crate::survival::night(&tick, *day, &reality) => "Night",
        StormPhase::Calm => "Day",
    };
    // Temperature and radiation (todo 20) beside hunger and thirst.
    let exposure = exposure.map(|e| e.0).unwrap_or_default();
    // The weather where the player stands (todo 21).
    let weather = world.as_deref().map_or("", |w| crate::weather::at(w, &biomes, tick.0, *day, crate::Ground::ground(at.translation)).name());
    text.0 = format!(
        "Health {:.0}   Hunger {:.0}   Thirst {:.0}   Temp {:.1}C   Rad {:.0}   {time}   {weather}",
        health.0.current, needs.0.needs.hunger, needs.0.needs.thirst, exposure.warmth, exposure.dose
    );
}

/// The one time in the first hour the sky changes with no storm behind
/// it (design.md "The first hour" rule 6: "the sky has changed once, so
/// the player knows it will"), the storm's own warning tint:
/// `SKY_CHANGE_AFTER_SECS` after the player wakes, for `SKY_CHANGE_SECS`.
fn first_sky_change(tick: &crate::SimTick) -> bool {
    let awake = crate::survival::Day::secs(tick) - crate::content::HISTORY_SECS;
    let from = crate::content::SKY_CHANGE_AFTER_SECS;
    (from..from + crate::content::SKY_CHANGE_SECS).contains(&awake)
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
    storms: Res<crate::storm::StormRes>,
    reality: Res<crate::Reality>,
    mut layers: Query<&mut BackgroundColor, With<SkyLayer>>,
) {
    let sky = match crate::storm::phase(&tick, *day, &storms) {
        StormPhase::Storm => Color::srgba(0.95, 0.93, 0.88, 0.85),
        StormPhase::Warning => Color::srgba(0.7, 0.3, 0.05, 0.3),
        StormPhase::Calm if first_sky_change(&tick) => Color::srgba(0.7, 0.3, 0.05, 0.3),
        StormPhase::Calm if crate::survival::night(&tick, *day, &reality) => Color::srgba(0.0, 0.02, 0.08, NIGHT_DARK),
        StormPhase::Calm => Color::NONE,
    };
    for mut color in &mut layers {
        color.0 = sky;
    }
}

/// A click on a slot selects it (modforge's `select_slot`): its item
/// shows in the item information panel, Rust's. Using it is the panel's
/// Eat, Drink, Use, or Wear. While trading, a click puts its stack on the
/// table or takes it off (`Action::OfferSlot`, trade.rs).
fn click_slot(
    click: On<Pointer<Click>>,
    slots: Query<&SlotUi>,
    tick: Res<crate::SimTick>,
    mut players: Query<(&Hud, &mut Actions), With<Player>>,
    mut commands: Commands,
) {
    let Ok(slot) = slots.get(click.entity) else {
        return;
    };
    if let Ok((hud, mut actions)) = players.single_mut()
        && hud.0.open_panel == OpenPanel::Trade
    {
        actions.0.push(tick.0 + 1, Action::OfferSlot { slot: slot.0 });
        return;
    }
    commands.run_system_cached_with(select, slot.0);
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
    memory: Option<&mut crate::actor::Persona>,
    now: u64,
) {
    heal_by(health, heal);
    if registry.0.def(item).is_some_and(|d| d.food.is_some())
        && let Some(memory) = memory
    {
        memory.0.memory.did(Did::Ate(item.to_string()), now);
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
    layers: Res<crate::content::LayerRegistryRes>,
    mut research: (Res<crate::research::TechTreeRes>, ResMut<crate::research::Researched>, ResMut<crate::skills::Practised>),
    stations: Query<(&crate::interact::Interactable, &Transform, Option<&crate::power::Powered>)>,
    mut people: Query<
        (
            (Entity, Option<&crate::actor::Actor>, &ThisTick, &Transform, Has<Player>, Option<&crate::actor::Npc>),
            (&mut Hud, &mut OpenContainer, &mut OpenStation),
            &mut crate::actor::Persona,
            &mut Hotbar,
            &mut Equipment,
            &mut Inventory,
            &mut crate::combat::Health,
        ),
        Without<Dead>,
    >,
    mut containers: Query<&mut Container>,
    mut ids: ResMut<crate::interact::ThingIssuer>,
) {
    // In ActorId order (authority.md rule 5): who takes from a box first,
    // and whose dropped thing is issued an id first, is the same after a
    // load.
    let order = crate::actor::in_actor_order(people.iter().map(|((entity, actor, ..), ..)| (actor.map(|a| a.0), entity)));
    for entity in order {
        let Ok((
            (_, _, this_tick, at, is_player, npc),
            (mut hud, mut open, mut station),
            mut person,
            mut hotbar,
            mut worn,
            mut inventory,
            mut health,
        )) = people.get_mut(entity)
        else {
            continue;
        };
        for stamped in &this_tick.0 {
            match stamped.action {
                Action::Inventory => toggle_panel(&mut hud.0, OpenPanel::Inventory),
                // Escape closes the panel, and ends a conversation.
                Action::Cancel => {
                    hud.0.open_panel = OpenPanel::None;
                    hud.0.talking = None;
                }
                Action::Hotbar { index } => {
                    let item = hotbar.0.slots.slots.get(index).and_then(|s| s.as_ref()).map(|s| s.item.clone());
                    if let Ok(heal) = hotbar_key(&mut person.0.needs, &mut hotbar.0, index, &registry.0, &mut worn.0)
                        && let Some(item) = item
                    {
                        used(&item, heal, &registry, &mut health, Some(&mut *person), tick.0);
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
                        if moved > 0 {
                            let what = if slot.holder == Holder::Container { Did::Took(item, moved) } else { Did::Stocked(item, moved) };
                            person.0.memory.did(what, tick.0);
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
                        crate::interact::spawn_item(&mut commands, &mut ids, def, stack, crate::interact::feet_of(at));
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
                        // Worn gear comes off by dragging, not by a click;
                        // what someone else carries is not the player's to use.
                        Holder::Equipment | Holder::Theirs => continue,
                    };
                    let item = holder.slots.get(slot.index).and_then(|s| s.as_ref()).map(|s| s.item.clone());
                    if let Ok(heal) = use_slot(&mut person.0.needs, holder, slot.index, &registry.0, &mut worn.0)
                        && let Some(item) = item
                    {
                        used(&item, heal, &registry, &mut health, Some(&mut *person), tick.0);
                    }
                }
                // A container poured (modforge fluid): from the bags into
                // the open box's containers (a bottle into the barrel), or
                // from the open box into the containers carried (a bottle
                // filled from the barrel).
                Action::Pour { slot } => {
                    let Some(mut container) = open.0.and_then(|e| containers.get_mut(e).ok()) else {
                        continue;
                    };
                    let from = match slot.holder {
                        Holder::Inventory => inventory.0.slots.get_mut(slot.index),
                        Holder::Hotbar => hotbar.0.slots.slots.get_mut(slot.index),
                        Holder::Container => container.0.slots.get_mut(slot.index),
                        Holder::Equipment | Holder::Theirs => continue,
                    };
                    let Some(Some(from)) = from else {
                        continue;
                    };
                    let mut litres = std::mem::take(&mut from.litres);
                    if slot.holder == Holder::Container {
                        modforge::fluid::pour_into(&mut inventory.0, &mut litres, &registry.0);
                        modforge::fluid::pour_into(&mut hotbar.0.slots, &mut litres, &registry.0);
                    } else {
                        modforge::fluid::pour_into(&mut container.0, &mut litres, &registry.0);
                    }
                    // What found no room stays where it was.
                    let back = match slot.holder {
                        Holder::Inventory => inventory.0.slots.get_mut(slot.index),
                        Holder::Hotbar => hotbar.0.slots.slots.get_mut(slot.index),
                        _ => container.0.slots.get_mut(slot.index),
                    };
                    if let Some(Some(back)) = back {
                        back.litres += litres;
                    }
                }
                Action::Craft { index } => {
                    // The row picked, of what this person's group is shown
                    // (todo 28): a topic at a research station is paid for
                    // and researched; a locked recipe is never listed.
                    let Some(kind) = station.0 else {
                        continue;
                    };
                    let group = crate::research::group_of(is_player, npc);
                    let (tree, researched, practised) = (&research.0, &mut research.1, &mut research.2);
                    let known = researched.0.get(&group).cloned().unwrap_or_default();
                    let recipe = match crate::research::rows(kind, &recipes.0, &tree.0, &known).into_iter().nth(index) {
                        Some(crate::research::Row::Recipe(recipe)) => recipe,
                        Some(crate::research::Row::Topic(topic)) => {
                            // Research takes power (todo 29).
                            if !crate::power::station_powered(at.translation, kind, &stations) {
                                continue;
                            }
                            // A practised researcher's topic takes less (todo 31).
                            let cost = crate::skills::research_cost(tree.0.cost(topic, &known), Some(&*person));
                            if crate::research::pay(&mut inventory.0, &cost) {
                                let name = topic.name.clone();
                                tree.0.research(&name, researched.0.entry(group).or_default());
                                practised.0.push((entity, crate::skills::RESEARCH, crate::skills::RESEARCH_XP));
                            }
                            continue;
                        }
                        None => continue,
                    };
                    // modforge's `craft`, the one way anything is made; what
                    // is made goes into the inventory, or at the feet when it
                    // is full. A practised crafter's odds lean to better
                    // quality (todo 31).
                    let odds = modforge::practice::lean_odds(
                        &modforge::quality::odds(&crate::content::QUALITY, 0),
                        crate::skills::mastery(Some(&*person), crate::skills::CRAFTING),
                        crate::skills::CRAFT_LEAN,
                    );
                    let Some(made) = craft(recipe, &mut inventory.0, &registry.0, &layers.0, &odds, tick.0 as f32, tick.0) else {
                        continue;
                    };
                    practised.0.push((entity, crate::skills::CRAFTING, crate::skills::CRAFT_XP));
                    let Some(def) = registry.0.def(&made.item) else {
                        continue;
                    };
                    if let Some(left) = inventory.0.add(made, def.max_stack) {
                        crate::interact::spawn_item(&mut commands, &mut ids, def, left, crate::interact::feet_of(at));
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

/// Fill the station panel from the open station's recipes, top to bottom
/// in the panel's own order, rows past the last recipe taking no room: a
/// recipe the inventory can pay for is lit.
pub fn render_station(
    recipes: Res<RecipeRegistryRes>,
    items: Res<crate::content::ItemRegistryRes>,
    research: (Res<crate::research::TechTreeRes>, Res<crate::research::Researched>),
    stations: Query<(&crate::interact::Interactable, &Transform, Option<&crate::power::Powered>)>,
    players: Query<(&Inventory, &OpenStation, &Transform, Option<&crate::actor::Persona>), With<Player>>,
    mut panels: Query<(&mut Visibility, &Children), (With<StationPanel>, Without<RecipeRow>)>,
    mut rows: Query<(&mut RecipeRow, &Children, &mut BackgroundColor, &mut Node)>,
    mut texts: Query<&mut Text>,
) {
    let (Ok((mut panel, in_order)), Ok((inventory, station, at, skills))) = (panels.single_mut(), players.single()) else {
        return;
    };
    let Some(kind) = station.0 else {
        *panel = Visibility::Hidden;
        return;
    };
    *panel = Visibility::Visible;
    // The player's group's research decides what is listed (todo 28).
    let (tree, researched) = research;
    let known = researched.0.get(crate::content::PLAYER_FACTION).cloned().unwrap_or_default();
    let list = crate::research::rows(kind, &recipes.0, &tree.0, &known);
    for (i, child) in in_order.iter().enumerate() {
        let Ok((mut row, children, mut color, mut node)) = rows.get_mut(child) else {
            continue;
        };
        let Some(entry) = list.get(i) else {
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        row.0 = i;
        let (line, can) = match entry {
            crate::research::Row::Recipe(recipe) => {
                let takes: Vec<String> = recipe.inputs.iter().map(|(item, n)| format!("{item} x{n}")).collect();
                (format!("{} x{}  <-  {}", recipe.output.0, recipe.output.1, takes.join(", ")), can_craft(recipe, &inventory.0, &items.0))
            }
            crate::research::Row::Topic(topic) => {
                let cost = crate::skills::research_cost(tree.0.cost(topic, &known), skills);
                let takes: Vec<String> = cost.iter().map(|(item, n)| format!("{item} x{n}")).collect();
                let level = known.level(&topic.name) + 1;
                let name = if topic.endless { format!("Research {} {level}", topic.name) } else { format!("Research {}", topic.name) };
                // No power, no research (todo 29).
                let powered = crate::power::station_powered(at.translation, kind, &stations);
                let power = if powered { "" } else { "  (no power)" };
                (format!("{name}  <-  {}{power}", takes.join(", ")), powered && crate::research::affords(&inventory.0, &cost))
            }
        };
        if let Some(mut text) = children.first().and_then(|c| texts.get_mut(*c).ok()) {
            text.0 = line;
        }
        color.0 = if can { HELD_COLOR } else { SLOT_COLOR };
    }
}

/// A conversation on screen (episodes.md "How a conversation looks";
/// Fallout: New Vegas's box, read from its screenshots 2026-10-01; the
/// mockup docs/mockups/conversation.png): one see-through box low in the
/// middle, its words amber. On top the name of the one talked to and the
/// last line said; under a line across the box, the player's choices,
/// numbered, said by their number keys; a threat shown <Threaten> in red.
/// The world never pauses and nothing covers the crosshair. Painted from
/// `HudState::talking` only.
#[derive(Component)]
pub struct TalkBox;

/// The name of the one the player talks to, on top of the box.
#[derive(Component)]
pub struct TalkName;

/// The last line said, with who said it, under the name.
#[derive(Component)]
pub struct TalkLine;

/// The choices part of the box: the line across, the rows, the keys.
#[derive(Component)]
pub struct ChoiceList;

/// One choice row: its number; its spans are the <Threaten> tag and the
/// words.
#[derive(Component)]
pub struct ChoiceRow;

/// The most choices shown at once: the number keys 1 to 6.
const CHOICE_ROWS: usize = 6;

/// The box's words and border (New Vegas's amber), a choice's words, a
/// threat's tag, and the box behind them.
const TALK_AMBER: Color = Color::srgb(1.0, 0.71, 0.26);
const TALK_DIM: Color = Color::srgb(0.71, 0.51, 0.2);
const TALK_RED: Color = Color::srgb(0.9, 0.27, 0.2);
const TALK_SHADE: Color = Color::srgba(0.04, 0.03, 0.02, 0.75);

pub fn setup_talk(mut commands: Commands) {
    commands
        .spawn((
            TalkBox,
            Node {
                position_type: PositionType::Absolute,
                // Above the belt.
                bottom: Val::Px(100.0),
                left: Val::Percent(20.0),
                width: Val::Percent(60.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(Val::Px(18.0), Val::Px(12.0)),
                border: UiRect::all(Val::Px(2.0)),
                row_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(TALK_SHADE),
            BorderColor::all(TALK_AMBER),
            Visibility::Hidden,
            Pickable::IGNORE,
        ))
        .with_children(|p| {
            p.spawn((TalkName, Text::new(""), TextFont::from_font_size(20.0), TextColor(TALK_AMBER), Pickable::IGNORE));
            p.spawn((TalkLine, Text::new(""), TextFont::from_font_size(18.0), TextColor(TALK_AMBER), Pickable::IGNORE));
            p.spawn((
                ChoiceList,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .with_children(|list| {
                list.spawn((
                    Node {
                        height: Val::Px(1.0),
                        margin: UiRect::bottom(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(TALK_DIM),
                    Pickable::IGNORE,
                ));
                for n in 1..=CHOICE_ROWS {
                    list.spawn((
                        ChoiceRow,
                        Text::new(format!("{n})  ")),
                        TextFont::from_font_size(18.0),
                        TextColor(TALK_AMBER),
                        Pickable::IGNORE,
                        children![
                            (TextSpan::new(""), TextFont::from_font_size(18.0), TextColor(TALK_RED)),
                            (TextSpan::new(""), TextFont::from_font_size(18.0), TextColor(TALK_AMBER)),
                        ],
                    ));
                }
                list.spawn((
                    Text::new("Number keys: say    Walk away: leave"),
                    TextFont::from_font_size(14.0),
                    TextColor(TALK_DIM),
                    Node {
                        margin: UiRect::top(Val::Px(4.0)),
                        ..default()
                    },
                    Pickable::IGNORE,
                ));
            });
        });
}

/// The basic controls the player has done (design.md "The bunker you
/// start in"): each hint shows until done once, in the bunker below the
/// ground (modforge `hud::Hint`).
#[derive(Component, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct HintsDone(pub Vec<modforge::hud::Hint>);

/// Every step, once the queues have drained: each control the player did
/// this step is done.
pub fn learn_hints(mut players: Query<(&crate::input::ThisTick, &mut HintsDone), With<Player>>) {
    for (this_tick, mut done) in &mut players {
        for hint in this_tick.0.iter().filter_map(|s| modforge::hud::Hint::of(&s.action)) {
            if !done.0.contains(&hint) {
                done.0.push(hint);
            }
        }
    }
}

/// The hints still to do, on the left of the screen.
#[derive(Component)]
pub struct HintList;

pub fn setup_hints(mut commands: Commands) {
    commands.spawn((
        HintList,
        Text::new(""),
        TextFont::from_font_size(16.0),
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.75)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(24.0),
            top: Val::Percent(40.0),
            ..default()
        },
        Pickable::IGNORE,
    ));
}

/// The hints not yet done, one a line, while the player is below the
/// ground; none topside.
pub fn hints_shown(done: &HintsDone, feet: f32) -> Vec<&'static str> {
    if feet > -0.5 {
        return Vec::new();
    }
    modforge::hud::Hint::ALL.iter().filter(|h| !done.0.contains(h)).map(|h| h.text()).collect()
}

pub fn render_hints(players: Query<(&HintsDone, &Transform), With<Player>>, mut list: Query<&mut Text, With<HintList>>) {
    let (Ok((done, at)), Ok(mut text)) = (players.single(), list.single_mut()) else {
        return;
    };
    text.0 = hints_shown(done, crate::interact::feet_of(at).y).join("\n");
}

/// What the player can do with what they look at: "[E] Talk to Hank"
/// (talk::look_at writes it), under the middle of the screen.
#[derive(Component)]
pub struct PromptText;

pub fn setup_prompt(mut commands: Commands) {
    commands.spawn((
        PromptText,
        Text::new(""),
        TextFont::from_font_size(18.0),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(56.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        Pickable::IGNORE,
    ));
}

/// The prompt while no panel is open and no choices are showing (E then
/// says the highlighted choice).
pub fn render_prompt(players: Query<&Hud, With<Player>>, mut texts: Query<&mut Text, With<PromptText>>) {
    let (Ok(hud), Ok(mut text)) = (players.single(), texts.single_mut()) else {
        return;
    };
    let choosing = hud.0.talking.as_ref().is_some_and(|t| !t.choices.is_empty());
    text.0 = if hud.0.open_panel == OpenPanel::None && !choosing { hud.0.prompt.text.clone() } else { String::new() };
}

/// Paint the conversation from `HudState::talking`: the box while there is
/// one, the name, the last line (the player's own with "You:"), and the
/// choices part only on the player's turn, rows past the last choice
/// taking no room.
#[allow(clippy::type_complexity)]
pub fn render_talk(
    players: Query<&Hud, With<Player>>,
    mut talk_box: Query<&mut Visibility, (With<TalkBox>, Without<ChoiceList>)>,
    mut name: Query<&mut Text, (With<TalkName>, Without<TalkLine>, Without<ChoiceRow>)>,
    mut line: Query<(&mut Text, &mut Node), (With<TalkLine>, Without<TalkName>, Without<ChoiceRow>, Without<ChoiceList>)>,
    mut list: Query<(&mut Node, &Children), (With<ChoiceList>, Without<ChoiceRow>)>,
    mut rows: Query<(&Children, &mut Node), With<ChoiceRow>>,
    mut spans: Query<&mut TextSpan>,
) {
    let (Ok(hud), Ok(mut shown), Ok(mut name), Ok((mut line, mut line_node)), Ok((mut list, in_order))) =
        (players.single(), talk_box.single_mut(), name.single_mut(), line.single_mut(), list.single_mut())
    else {
        return;
    };
    let Some(talking) = hud.0.talking.as_ref() else {
        *shown = Visibility::Hidden;
        return;
    };
    *shown = Visibility::Visible;
    name.0 = talking.name.clone();
    line.0 = match &talking.line {
        Some((who, words)) if *who == talking.name => format!("\"{words}\""),
        Some((who, words)) => format!("{who}: \"{words}\""),
        None => String::new(),
    };
    // Nothing said yet in this conversation: the line takes no room.
    line_node.display = if line.0.is_empty() { Display::None } else { Display::Flex };
    list.display = if talking.choices.is_empty() { Display::None } else { Display::Flex };
    // Top to bottom in the list's own order.
    let in_order: Vec<Entity> = in_order.iter().filter(|c| rows.contains(*c)).collect();
    for (i, child) in in_order.into_iter().enumerate() {
        let Ok((children, mut node)) = rows.get_mut(child) else {
            continue;
        };
        let Some(choice) = talking.choices.get(i) else {
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        let tag = if choice.threat { "<Threaten> " } else { "" };
        for (span, text) in children.iter().zip([tag, choice.words.as_str()]) {
            if let Ok(mut span) = spans.get_mut(span) {
                span.0 = text.to_string();
            }
        }
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

/// The episode's ending as a text screen before the storm (docs/episodes/,
/// "Which ending": shown as a text screen; todo 11ae): its name over what
/// it says happened, the whole window.
#[derive(Component)]
pub struct EndingScreen;

/// The ending screen's words.
#[derive(Component)]
pub struct EndingText;

pub fn setup_ending(mut commands: Commands) {
    commands
        .spawn((
            EndingScreen,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::BLACK),
            GlobalZIndex(i32::MAX - 1),
            Visibility::Hidden,
        ))
        .with_child((EndingText, Text::new("")));
}

/// Shown while the player's HUD holds an ending, with its name and words.
pub fn render_ending(
    players: Query<&crate::interact::Hud, With<crate::player::Player>>,
    mut screens: Query<&mut Visibility, With<EndingScreen>>,
    mut texts: Query<&mut Text, With<EndingText>>,
) {
    let ending = players.single().ok().and_then(|h| h.0.ending.clone());
    for mut visibility in &mut screens {
        visibility.set_if_neq(if ending.is_some() { Visibility::Visible } else { Visibility::Hidden });
    }
    if let (Some(ending), Ok(mut text)) = (ending, texts.single_mut()) {
        let words = format!("{}\n\n{}", ending.name, ending.words);
        if text.0 != words {
            text.0 = words;
        }
    }
}
