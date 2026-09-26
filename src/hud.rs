//! What the player reads and handles on screen, painted from modforge's
//! `HudState` and the player's own holders (modforge decides every
//! move; this only draws and forwards): the hotbar always shown, the
//! inventory panel with an open box beside it, the note panel.

use bevy::prelude::*;
use modforge::hud::{
    Holder, Holders, HudState, OpenPanel, SlotRef, end_drag, hotbar_key, slot_label, start_drag, toggle_panel,
    transfer_slot, use_slot, HOTBAR_SLOTS,
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
                    Node {
                        width: Val::Px(SLOT_PX),
                        height: Val::Px(SLOT_PX),
                        padding: UiRect::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(SLOT_COLOR),
                    children![(Text::new(""), TextFont::from_font_size(11.0), Pickable::IGNORE)],
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
        });
}

/// Paint every slot from its holder, the held and hovered slots
/// marked, and show the screen and the box modforge says are open.
pub fn render(
    hovered: Res<HoveredSlot>,
    players: Query<(&Inventory, &Hotbar, &Equipment, &Hud, &OpenContainer), With<Player>>,
    containers: Query<&Container>,
    mut slots: Query<(&SlotUi, &Children, &mut BackgroundColor, &mut Visibility)>,
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
    for (slot, children, mut color, mut visibility) in &mut slots {
        let holder = match slot.0.holder {
            Holder::Inventory => Some(&inventory.0),
            Holder::Hotbar => Some(&hotbar.0.slots),
            Holder::Container => container.map(|c| &c.0),
            Holder::Equipment => None,
        };
        if slot.0.holder == Holder::Container {
            let exists = holder.is_some_and(|h| slot.0.index < h.slots.len());
            *visibility = if exists { Visibility::Inherited } else { Visibility::Hidden };
        }
        let stack = match slot.0.holder {
            Holder::Equipment => worn.0.get(slot.0.index),
            _ => holder.and_then(|h| h.slots.get(slot.0.index)).and_then(|s| s.as_ref()),
        };
        let label = slot_label(stack);
        let label = if slot.0.holder == Holder::Hotbar {
            format!("{}\n{}", slot.0.index + 1, label)
        } else {
            label
        };
        if let Some(mut text) = children.first().and_then(|c| texts.get_mut(*c).ok()) {
            text.0 = label;
        }
        let held = slot.0.holder == Holder::Hotbar && hotbar.0.held == Some(slot.0.index);
        color.0 = if hovered.0 == Some(slot.0) {
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

/// A click on a slot uses what is in it (modforge's `use_slot`): food
/// and drink are eaten, worn gear is put on.
fn click_slot(
    click: On<Pointer<Click>>,
    slots: Query<&SlotUi>,
    tick: Res<crate::SimTick>,
    mut players: Query<&mut Actions, With<Player>>,
) {
    if let (Ok(slot), Ok(mut actions)) = (slots.get(click.entity), players.single_mut()) {
        actions.0.push(tick.0 + 1, Action::UseSlot { slot: slot.0 });
    }
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
