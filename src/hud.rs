//! The HUD: writer systems fill HudRes from game state, one
//! renderer reads only HudRes to paint the screen text. The
//! inventory panel and the hotbar paint the player's holders in
//! place and forward gestures to modforge's decisions (start_drag,
//! end_drag, use_slot, drop_slot). Atlas controls (design.md).

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use modforge::hud::{
    HOTBAR_SLOTS, Holder, Holders, HudState, OpenPanel, SlotRef, drop_slot, end_drag,
    hotbar_slot, prompt_for, slot_label, start_drag, toggle_panel, transfer_slot, use_slot,
};

use crate::actor::{Hotbar, INVENTORY_SLOTS, Inventory};
use crate::cel::CelMaterial;
use crate::content::ItemRegistryRes;
use crate::interact::{
    CONTAINER_SLOTS, Container, LookingAt, OpenContainer, WorldItem, spawn_item,
};
use crate::player::Player;
use crate::survival::Survival;

#[derive(Resource)]
pub struct HudRes(pub HudState);

/// The slot under the cursor, for the hover keys (O drops, T
/// transfers). Atlas: shortcuts act on what the mouse is over.
#[derive(Resource, Default)]
pub struct HoveredSlot(pub Option<SlotRef>);

#[derive(Component)]
pub struct Hud;

/// The inventory panel root; its children are one `Slot` each.
#[derive(Component)]
pub struct InventoryPanel;

/// The hotbar root; its children are one `Slot` each.
#[derive(Component)]
pub struct HotbarBar;

/// The open container's panel root, beside the inventory panel.
#[derive(Component)]
pub struct ContainerPanel;

/// One slot button on screen: which holder, which index.
#[derive(Component)]
pub struct Slot(pub SlotRef);

const SLOT_COLUMNS: usize = 5;
const SLOT_SIZE: f32 = 96.0;
const HOTBAR_SLOT_SIZE: f32 = 64.0;

pub fn setup(mut commands: Commands) {
    commands.spawn((
        Hud,
        Text::new(""),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(8.0),
            ..default()
        },
    ));
    // The inventory panel, hidden until opened.
    commands
        .spawn((
            InventoryPanel,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(20.0),
                left: Val::Percent(25.0),
                display: Display::Grid,
                grid_template_columns: RepeatedGridTrack::px(SLOT_COLUMNS as u16, SLOT_SIZE),
                row_gap: Val::Px(4.0),
                column_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.06, 0.9)),
            Visibility::Hidden,
        ))
        .with_children(|panel| {
            for index in 0..INVENTORY_SLOTS {
                spawn_slot(panel, Holder::Inventory, index, SLOT_SIZE);
            }
        });
    // The hotbar, always on screen along the bottom.
    commands
        .spawn((
            HotbarBar,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(8.0),
                left: Val::Percent(50.0),
                margin: UiRect::left(Val::Px(
                    -(HOTBAR_SLOTS as f32) * (HOTBAR_SLOT_SIZE + 4.0) / 2.0,
                )),
                display: Display::Flex,
                column_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.06, 0.7)),
        ))
        .with_children(|bar| {
            for index in 0..HOTBAR_SLOTS {
                spawn_slot(bar, Holder::Hotbar, index, HOTBAR_SLOT_SIZE);
            }
        });
    // The open container's panel, right of the inventory, hidden
    // until a crate is opened.
    commands
        .spawn((
            ContainerPanel,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(20.0),
                left: Val::Percent(62.0),
                display: Display::Grid,
                grid_template_columns: RepeatedGridTrack::px(4, SLOT_SIZE),
                row_gap: Val::Px(4.0),
                column_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.08, 0.06, 0.04, 0.9)),
            Visibility::Hidden,
        ))
        .with_children(|panel| {
            for index in 0..CONTAINER_SLOTS {
                spawn_slot(panel, Holder::Container, index, SLOT_SIZE);
            }
        });
}

/// One slot button. The same observers serve the bar and the panel,
/// so dragging between them is one move across the two holders.
fn spawn_slot(parent: &mut ChildSpawnerCommands, holder: Holder, index: usize, size: f32) {
    parent
        .spawn((
            Slot(SlotRef { holder, index }),
            Button,
            Node {
                width: Val::Px(size),
                height: Val::Px(size),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.2, 0.2, 0.22)),
            children![(Text::new(""), TextFont::from_font_size(14.0))],
        ))
        .observe(slot_clicked)
        .observe(slot_drag_start)
        .observe(slot_drag_drop)
        .observe(slot_drag_end)
        .observe(slot_over)
        .observe(slot_out);
}

/// The slot-indexed holders on screen: the player's inventory and
/// hotbar, and the open crate if there is one. Every gesture borrows
/// them through here as one modforge `Holders`.
#[derive(SystemParam)]
pub struct OnScreenHolders<'w, 's> {
    players: Query<'w, 's, (&'static mut Inventory, &'static mut Hotbar), With<Player>>,
    containers: Query<'w, 's, &'static mut Container>,
    open: Res<'w, OpenContainer>,
}

impl OnScreenHolders<'_, '_> {
    fn get(&mut self) -> Option<Holders<'_>> {
        let (inventory, hotbar) = self.players.single_mut().ok()?;
        let container = self
            .open
            .0
            .and_then(|entity| self.containers.get_mut(entity).ok());
        Some(Holders {
            inventory: &mut inventory.into_inner().0,
            hotbar: &mut hotbar.into_inner().0,
            container: container.map(|c| &mut c.into_inner().0),
        })
    }

    /// One holder by slot, for single-holder actions (use, drop).
    fn one(&mut self, slot: SlotRef) -> Option<&mut modforge::item::Inventory> {
        let holders = self.get()?;
        match slot.holder {
            Holder::Inventory => Some(holders.inventory),
            Holder::Hotbar => Some(holders.hotbar),
            Holder::Container => holders.container,
        }
    }
}

/// The one engine-side follow-up to the panel opening or closing:
/// the cursor is free while a panel is open and grabbed when none
/// is; closing drops any drag, hover, and open crate with it.
pub fn panel_changed(
    hud: &mut HudState,
    hovered: &mut HoveredSlot,
    open_container: &mut OpenContainer,
    cursors: &mut Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    let open = hud.open_panel == OpenPanel::Inventory;
    hud.dragging = None;
    hovered.0 = None;
    if !open {
        open_container.0 = None;
    }
    if let Ok(mut cursor) = cursors.single_mut() {
        cursor.grab_mode = if open {
            CursorGrabMode::None
        } else {
            CursorGrabMode::Locked
        };
        cursor.visible = open;
    }
}

/// I (Atlas) or Tab opens and closes the panel; Escape closes it.
pub fn toggle_inventory(
    keys: Res<ButtonInput<KeyCode>>,
    mut hud: ResMut<HudRes>,
    mut hovered: ResMut<HoveredSlot>,
    mut open_container: ResMut<OpenContainer>,
    mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    let open = hud.0.open_panel == OpenPanel::Inventory;
    if keys.any_just_pressed([KeyCode::KeyI, KeyCode::Tab])
        || (open && keys.just_pressed(KeyCode::Escape))
    {
        toggle_panel(&mut hud.0, OpenPanel::Inventory);
        panel_changed(&mut hud.0, &mut hovered, &mut open_container, &mut cursors);
    }
}

pub fn inventory_open(hud: Res<HudRes>) -> bool {
    hud.0.open_panel == OpenPanel::Inventory
}

/// Digit keys use the hotbar slot (modforge `hotbar_slot` order),
/// panel open or not.
pub fn hotbar_keys(
    keys: Res<ButtonInput<KeyCode>>,
    registry: Res<ItemRegistryRes>,
    mut players: Query<(&mut Hotbar, &mut Survival), With<Player>>,
) {
    const DIGITS: [(KeyCode, u8); 10] = [
        (KeyCode::Digit1, 1),
        (KeyCode::Digit2, 2),
        (KeyCode::Digit3, 3),
        (KeyCode::Digit4, 4),
        (KeyCode::Digit5, 5),
        (KeyCode::Digit6, 6),
        (KeyCode::Digit7, 7),
        (KeyCode::Digit8, 8),
        (KeyCode::Digit9, 9),
        (KeyCode::Digit0, 0),
    ];
    let Some(slot) = DIGITS
        .iter()
        .find(|(key, _)| keys.just_pressed(*key))
        .and_then(|(_, digit)| hotbar_slot(*digit))
    else {
        return;
    };
    let Ok((mut hotbar, mut survival)) = players.single_mut() else {
        return;
    };
    let _ = use_slot(&mut survival.0, &mut hotbar.0, slot, &registry.0);
}

/// Double left click uses the slot (modforge `use_slot`; food is
/// eaten). A single click does nothing, as in Atlas.
fn slot_clicked(
    click: On<Pointer<Click>>,
    slots: Query<&Slot>,
    registry: Res<ItemRegistryRes>,
    mut holders: OnScreenHolders,
    mut players: Query<&mut Survival, With<Player>>,
) {
    if click.button != PointerButton::Primary || click.count < 2 {
        return;
    }
    let Ok(slot) = slots.get(click.entity) else {
        return;
    };
    let (Some(holder), Ok(mut survival)) = (holders.one(slot.0), players.single_mut()) else {
        return;
    };
    // A refused use (not food, empty) changes nothing.
    let _ = use_slot(&mut survival.0, holder, slot.0.index, &registry.0);
}

fn slot_drag_start(
    drag: On<Pointer<DragStart>>,
    slots: Query<&Slot>,
    mut hud: ResMut<HudRes>,
    mut holders: OnScreenHolders,
) {
    if drag.button != PointerButton::Primary {
        return;
    }
    let (Ok(slot), Some(holders)) = (slots.get(drag.entity), holders.get()) else {
        return;
    };
    start_drag(&mut hud.0, &holders, slot.0);
}

/// The dragged stack lands on the slot under the cursor.
fn slot_drag_drop(
    drop: On<Pointer<DragDrop>>,
    slots: Query<&Slot>,
    mut hud: ResMut<HudRes>,
    registry: Res<ItemRegistryRes>,
    mut holders: OnScreenHolders,
) {
    let (Ok(slot), Some(mut holders)) = (slots.get(drop.entity), holders.get()) else {
        return;
    };
    end_drag(&mut hud.0, &mut holders, slot.0, &registry.0);
}

/// Hover plus T (Atlas) sends the stack under the cursor to the
/// other side, Shift+T half of it (modforge `transfer_slot`).
pub fn transfer_hovered(
    keys: Res<ButtonInput<KeyCode>>,
    hovered: Res<HoveredSlot>,
    registry: Res<ItemRegistryRes>,
    mut holders: OnScreenHolders,
) {
    if !keys.just_pressed(KeyCode::KeyT) {
        return;
    }
    let (Some(slot), Some(mut holders)) = (hovered.0, holders.get()) else {
        return;
    };
    let half = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    transfer_slot(&mut holders, slot, half, &registry.0);
}

/// A drag released anywhere else is cancelled.
fn slot_drag_end(_end: On<Pointer<DragEnd>>, mut hud: ResMut<HudRes>) {
    hud.0.dragging = None;
}

fn slot_over(over: On<Pointer<Over>>, slots: Query<&Slot>, mut hovered: ResMut<HoveredSlot>) {
    if let Ok(slot) = slots.get(over.entity) {
        hovered.0 = Some(slot.0);
    }
}

fn slot_out(out: On<Pointer<Out>>, slots: Query<&Slot>, mut hovered: ResMut<HoveredSlot>) {
    if let Ok(slot) = slots.get(out.entity)
        && hovered.0 == Some(slot.0)
    {
        hovered.0 = None;
    }
}

/// Hover plus O drops the stack under the cursor at the player's
/// feet (modforge `drop_slot`, then the one spawn path).
pub fn drop_hovered(
    keys: Res<ButtonInput<KeyCode>>,
    hovered: Res<HoveredSlot>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<CelMaterial>>,
    mut holders: OnScreenHolders,
    players: Query<&Transform, With<Player>>,
) {
    if !keys.just_pressed(KeyCode::KeyO) {
        return;
    }
    let (Some(slot), Ok(transform)) = (hovered.0, players.single()) else {
        return;
    };
    let Some(stack) = holders.one(slot).and_then(|holder| drop_slot(holder, slot.index)) else {
        return;
    };
    let feet = transform.translation.with_y(0.3) + transform.forward().with_y(0.0) * 1.0;
    spawn_item(
        &mut commands,
        &mut meshes,
        &mut materials,
        stack,
        feet,
        [0.6, 0.6, 0.6],
    );
}

/// Paint the panels and the hotbar from the holders in place: the
/// inventory panel visible while open, the container panel while a
/// crate is open, one label per slot, the dragged slot lit.
pub fn render_inventory(
    hud: Res<HudRes>,
    open_container: Res<OpenContainer>,
    players: Query<(&Inventory, &Hotbar), With<Player>>,
    containers: Query<&Container>,
    mut panels: Query<&mut Visibility, (With<InventoryPanel>, Without<ContainerPanel>)>,
    mut container_panels: Query<&mut Visibility, With<ContainerPanel>>,
    mut slots: Query<(&Slot, &mut BackgroundColor, &Children)>,
    mut texts: Query<&mut Text>,
) {
    let shown = |on: bool| {
        if on {
            Visibility::Visible
        } else {
            Visibility::Hidden
        }
    };
    let open = hud.0.open_panel == OpenPanel::Inventory;
    let container = open_container
        .0
        .and_then(|entity| containers.get(entity).ok());
    for mut visibility in &mut panels {
        *visibility = shown(open);
    }
    for mut visibility in &mut container_panels {
        *visibility = shown(open && container.is_some());
    }
    let Ok((inventory, hotbar)) = players.single() else {
        return;
    };
    for (slot, mut color, children) in &mut slots {
        let dragging = hud.0.dragging == Some(slot.0);
        *color = BackgroundColor(if dragging {
            Color::srgb(0.5, 0.45, 0.2)
        } else {
            Color::srgb(0.2, 0.2, 0.22)
        });
        let holder = match slot.0.holder {
            Holder::Inventory => Some(&inventory.0),
            Holder::Hotbar => Some(&hotbar.0),
            Holder::Container => container.map(|c| &c.0),
        };
        let label = slot_label(
            holder
                .and_then(|h| h.slots.get(slot.0.index))
                .and_then(|s| s.as_ref()),
        );
        for child in children.iter() {
            if let Ok(mut text) = texts.get_mut(child) {
                text.0 = label.clone();
            }
        }
    }
}

pub fn write_prompt(
    looking_at: Res<LookingAt>,
    items: Query<&WorldItem>,
    mut state: ResMut<HudRes>,
) {
    if let Some(kind) = looking_at.kind {
        let (name, count) = looking_at
            .entity
            .and_then(|e| items.get(e).ok())
            .map(|w| (Some(w.stack.item.as_str()), Some(w.stack.count)))
            .unwrap_or((None, None));
        state.0.prompt = prompt_for(kind, name, count);
    } else {
        state.0.prompt = Default::default();
    }
}

/// Paint the HUD. Reads the player's survival stats in place; the
/// player entity is the one owner, the HUD never copies them.
pub fn render(
    state: Res<HudRes>,
    players: Query<&Survival, With<Player>>,
    mut huds: Query<&mut Text, With<Hud>>,
) {
    let Ok(mut text) = huds.single_mut() else {
        return;
    };
    let Ok(survival) = players.single() else {
        return;
    };
    let mut line = format!(
        "Hunger: {:.0}  Thirst: {:.0}",
        survival.0.hunger, survival.0.thirst
    );
    if state.0.prompt.can_interact {
        line.push('\n');
        line.push_str(&state.0.prompt.text);
    }
    text.0 = line;
}
