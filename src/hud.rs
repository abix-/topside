//! What the player reads and handles on screen, painted from modforge's
//! `HudState` and the player's own holders (modforge decides every
//! move; this only draws and forwards): the hotbar always shown, the
//! inventory panel with an open box beside it, the note panel.

use bevy::prelude::*;
use modforge::hud::{
    Holder, Holders, OpenPanel, SlotRef, end_drag, hotbar_key, slot_label, start_drag, toggle_panel,
    transfer_slot, HOTBAR_SLOTS,
};

use crate::actor::{Hotbar, Inventory, INVENTORY_SLOTS, Survival};
use crate::combat::Equipment;
use crate::content::ItemRegistryRes;
use crate::input::{Action, ThisTick};
use crate::interact::{Container, HudRes, OpenContainer};
use crate::player::Player;

/// Slots drawn for an open box; a bigger box shows its first ones.
pub const CONTAINER_SLOTS: usize = 24;
const SLOT_PX: f32 = 64.0;
const SLOT_COLOR: Color = Color::srgba(0.12, 0.11, 0.1, 0.85);
const HELD_COLOR: Color = Color::srgba(0.45, 0.38, 0.2, 0.95);
const HOVER_COLOR: Color = Color::srgba(0.3, 0.3, 0.32, 0.95);

/// One slot on screen and which holder's slot it shows.
#[derive(Component)]
pub struct SlotUi(pub SlotRef);

/// The panel that shows a note's words while it is being read.
#[derive(Component)]
pub struct NotePanel;

/// The inventory panel and the open box's panel.
#[derive(Component)]
pub struct InventoryPanel;
#[derive(Component)]
pub struct ContainerPanel;

/// The slot under the cursor, for T.
#[derive(Resource, Default)]
pub struct HoveredSlot(pub Option<SlotRef>);

fn slot_row(parent: &mut ChildSpawnerCommands, holder: Holder, from: usize, to: usize) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
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
                .observe(drag_from_slot)
                .observe(drop_on_slot);
            }
        });
}

fn panel(parent: &mut ChildSpawnerCommands, marker: impl Bundle, holder: Holder, slots: usize) {
    parent
        .spawn((
            marker,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.05, 0.8)),
            Visibility::Hidden,
        ))
        .with_children(|p| {
            for from in (0..slots).step_by(6) {
                slot_row(p, holder, from, (from + 6).min(slots));
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
    // The inventory and the open box, side by side in the middle.
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            top: Val::Px(60.0),
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(24.0),
            ..default()
        })
        .with_children(|root| {
            panel(root, InventoryPanel, Holder::Inventory, INVENTORY_SLOTS);
            panel(root, ContainerPanel, Holder::Container, CONTAINER_SLOTS);
        });
    // The hotbar, always, along the bottom.
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            bottom: Val::Px(16.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|root| slot_row(root, Holder::Hotbar, 0, HOTBAR_SLOTS));
}

/// Paint every slot from its holder, the held and hovered slots
/// marked, and show the panels modforge says are open.
pub fn render(
    hud: Res<HudRes>,
    hovered: Res<HoveredSlot>,
    open: Res<OpenContainer>,
    players: Query<(&Inventory, &Hotbar), With<Player>>,
    containers: Query<&Container>,
    mut slots: Query<(&SlotUi, &Children, &mut BackgroundColor, &mut Visibility)>,
    mut texts: Query<&mut Text, Without<NotePanel>>,
    mut inventory_panel: Query<&mut Visibility, (With<InventoryPanel>, Without<SlotUi>, Without<ContainerPanel>)>,
    mut container_panel: Query<&mut Visibility, (With<ContainerPanel>, Without<SlotUi>, Without<InventoryPanel>)>,
    mut note: Query<(&mut Text, &mut Visibility), (With<NotePanel>, Without<SlotUi>, Without<InventoryPanel>, Without<ContainerPanel>)>,
) {
    let Ok((inventory, hotbar)) = players.single() else {
        return;
    };
    let container = open.0.and_then(|e| containers.get(e).ok());
    let inventory_open = hud.0.open_panel == OpenPanel::Inventory;
    if let Ok(mut v) = inventory_panel.single_mut() {
        *v = if inventory_open { Visibility::Visible } else { Visibility::Hidden };
    }
    if let Ok(mut v) = container_panel.single_mut() {
        *v = if inventory_open && container.is_some() { Visibility::Visible } else { Visibility::Hidden };
    }
    for (slot, children, mut color, mut visibility) in &mut slots {
        let holder = match slot.0.holder {
            Holder::Inventory => Some(&inventory.0),
            Holder::Hotbar => Some(&hotbar.0.slots),
            Holder::Container => container.map(|c| &c.0),
        };
        let exists = holder.is_some_and(|h| slot.0.index < h.slots.len());
        if slot.0.holder == Holder::Container {
            *visibility = if exists { Visibility::Inherited } else { Visibility::Hidden };
        }
        let stack = holder.and_then(|h| h.slots.get(slot.0.index)).and_then(|s| s.as_ref());
        let label = slot_label(stack);
        let label = if slot.0.holder == Holder::Hotbar {
            format!("{}\n{}", (slot.0.index + 1) % 10, label)
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

/// The player's holders and the open box, borrowed together.
#[derive(bevy::ecs::system::SystemParam)]
pub struct PlayerHolders<'w, 's> {
    players: Query<'w, 's, (&'static mut Inventory, &'static mut Hotbar), With<Player>>,
    containers: Query<'w, 's, &'static mut Container>,
    open: Res<'w, OpenContainer>,
}

impl PlayerHolders<'_, '_> {
    fn with<R>(&mut self, f: impl FnOnce(&mut Holders<'_>) -> R) -> Option<R> {
        let open = self.open.0;
        let (mut inventory, mut hotbar) = self.players.single_mut().ok()?;
        let mut container = open.and_then(|e| self.containers.get_mut(e).ok());
        let mut holders = Holders {
            inventory: &mut inventory.0,
            hotbar: &mut hotbar.0.slots,
            container: container.as_mut().map(|c| &mut c.0),
        };
        Some(f(&mut holders))
    }
}

/// A drag starts on `slot` (modforge's `start_drag`).
pub fn drag_start(slot: In<SlotRef>, mut hud: ResMut<HudRes>, mut holders: PlayerHolders) {
    holders.with(|h| start_drag(&mut hud.0, h, *slot));
}

/// A drag ends on `slot` (modforge's `end_drag`).
pub fn drag_drop(slot: In<SlotRef>, mut hud: ResMut<HudRes>, registry: Res<ItemRegistryRes>, mut holders: PlayerHolders) {
    holders.with(|h| end_drag(&mut hud.0, h, *slot, &registry.0));
}

/// The player's panel and hotbar keys this tick, each through modforge:
/// Tab or I toggles the inventory, Esc closes whatever is open, T moves
/// the hovered stack across, a number key holds that hotbar slot.
pub fn hud_keys(
    mut hud: ResMut<HudRes>,
    mut open: ResMut<OpenContainer>,
    hovered: Res<HoveredSlot>,
    registry: Res<ItemRegistryRes>,
    mut players: Query<(&ThisTick, &mut Survival, &mut Hotbar, &mut Equipment, &mut Inventory), With<Player>>,
    mut containers: Query<&mut Container>,
) {
    let Ok((this_tick, mut needs, mut hotbar, mut worn, mut inventory)) = players.single_mut() else {
        return;
    };
    for stamped in &this_tick.0 {
        match stamped.action {
            Action::Inventory => toggle_panel(&mut hud.0, OpenPanel::Inventory),
            Action::Cancel => hud.0.open_panel = OpenPanel::None,
            Action::Hotbar { index } => {
                let _ = hotbar_key(&mut needs.0, &mut hotbar.0, index, &registry.0, &mut worn.0);
            }
            Action::Transfer { half } => {
                let Some(slot) = hovered.0 else {
                    continue;
                };
                let mut container = open.0.and_then(|e| containers.get_mut(e).ok());
                let mut holders = Holders {
                    inventory: &mut inventory.0,
                    hotbar: &mut hotbar.0.slots,
                    container: container.as_mut().map(|c| &mut c.0),
                };
                transfer_slot(&mut holders, slot, half, &registry.0);
            }
            _ => {}
        }
    }
    // A closed inventory closes the box with it.
    if hud.0.open_panel != OpenPanel::Inventory {
        open.0 = None;
    }
}
