//! The HUD: writer systems fill HudRes from game state, one
//! renderer reads only HudRes to paint the screen text. The
//! inventory panel paints the player's slots in place and forwards
//! clicks to modforge's panel decisions (click_slot, use_slot).

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use modforge::hud::{
    HudState, OpenPanel, drop_slot, end_drag, prompt_for, slot_label, start_drag, toggle_panel,
    use_slot,
};

use crate::cel::CelMaterial;
use crate::content::ItemRegistryRes;
use crate::interact::{LookingAt, WorldItem, spawn_item};
use crate::player::{INVENTORY_SLOTS, Player, PlayerInventory};
use crate::survival::Survival;

#[derive(Resource)]
pub struct HudRes(pub HudState);

/// The inventory slot under the cursor, for the hover keys (O drops,
/// T transfers). Atlas: shortcuts act on what the mouse is over.
#[derive(Resource, Default)]
pub struct HoveredSlot(pub Option<usize>);

#[derive(Component)]
pub struct Hud;

/// The inventory panel root; its children are one `Slot` each.
#[derive(Component)]
pub struct InventoryPanel;

/// One slot button in the panel, by inventory index.
#[derive(Component)]
pub struct Slot(pub usize);

const SLOT_COLUMNS: usize = 5;
const SLOT_SIZE: f32 = 96.0;

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
                panel
                    .spawn((
                        Slot(index),
                        Button,
                        Node {
                            width: Val::Px(SLOT_SIZE),
                            height: Val::Px(SLOT_SIZE),
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
        });
}

/// I (Atlas) or Tab opens and closes the panel. Open releases the
/// cursor so the slots can be used; close grabs it back.
pub fn toggle_inventory(
    keys: Res<ButtonInput<KeyCode>>,
    mut hud: ResMut<HudRes>,
    mut hovered: ResMut<HoveredSlot>,
    mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if !keys.any_just_pressed([KeyCode::KeyI, KeyCode::Tab]) {
        return;
    }
    toggle_panel(&mut hud.0, OpenPanel::Inventory);
    hud.0.dragging_slot = None;
    hovered.0 = None;
    if let Ok(mut cursor) = cursors.single_mut() {
        let open = hud.0.open_panel == OpenPanel::Inventory;
        cursor.grab_mode = if open {
            CursorGrabMode::None
        } else {
            CursorGrabMode::Locked
        };
        cursor.visible = open;
    }
}

pub fn inventory_open(hud: Res<HudRes>) -> bool {
    hud.0.open_panel == OpenPanel::Inventory
}

/// Double left click uses the slot (modforge `use_slot`; food is
/// eaten). A single click does nothing, as in Atlas.
fn slot_clicked(
    click: On<Pointer<Click>>,
    slots: Query<&Slot>,
    registry: Res<ItemRegistryRes>,
    mut players: Query<(&mut PlayerInventory, &mut Survival), With<Player>>,
) {
    if click.button != PointerButton::Primary || click.count < 2 {
        return;
    }
    let Ok(slot) = slots.get(click.entity) else {
        return;
    };
    let Ok((mut inventory, mut survival)) = players.single_mut() else {
        return;
    };
    // A refused use (not food, empty) changes nothing.
    let _ = use_slot(&mut survival.0, &mut inventory.0, slot.0, &registry.0);
}

fn slot_drag_start(
    drag: On<Pointer<DragStart>>,
    slots: Query<&Slot>,
    mut hud: ResMut<HudRes>,
    players: Query<&PlayerInventory, With<Player>>,
) {
    if drag.button != PointerButton::Primary {
        return;
    }
    let (Ok(slot), Ok(inventory)) = (slots.get(drag.entity), players.single()) else {
        return;
    };
    start_drag(&mut hud.0, &inventory.0, slot.0);
}

/// The dragged stack lands on the slot under the cursor.
fn slot_drag_drop(
    drop: On<Pointer<DragDrop>>,
    slots: Query<&Slot>,
    mut hud: ResMut<HudRes>,
    registry: Res<ItemRegistryRes>,
    mut players: Query<&mut PlayerInventory, With<Player>>,
) {
    let (Ok(slot), Ok(mut inventory)) = (slots.get(drop.entity), players.single_mut()) else {
        return;
    };
    end_drag(&mut hud.0, &mut inventory.0, slot.0, &registry.0);
}

/// A drag released anywhere else is cancelled.
fn slot_drag_end(_end: On<Pointer<DragEnd>>, mut hud: ResMut<HudRes>) {
    hud.0.dragging_slot = None;
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
    mut players: Query<(&Transform, &mut PlayerInventory), With<Player>>,
) {
    if !keys.just_pressed(KeyCode::KeyO) {
        return;
    }
    let Some(slot) = hovered.0 else {
        return;
    };
    let Ok((transform, mut inventory)) = players.single_mut() else {
        return;
    };
    let Some(stack) = drop_slot(&mut inventory.0, slot) else {
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

/// Paint the panel from the player's inventory in place: visible
/// only while open, one label per slot, the selected slot lit.
pub fn render_inventory(
    hud: Res<HudRes>,
    players: Query<&PlayerInventory, With<Player>>,
    mut panels: Query<&mut Visibility, With<InventoryPanel>>,
    mut slots: Query<(&Slot, &mut BackgroundColor, &Children)>,
    mut texts: Query<&mut Text>,
) {
    let open = hud.0.open_panel == OpenPanel::Inventory;
    for mut visibility in &mut panels {
        *visibility = if open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if !open {
        return;
    }
    let Ok(inventory) = players.single() else {
        return;
    };
    for (slot, mut color, children) in &mut slots {
        let dragging = hud.0.dragging_slot == Some(slot.0);
        *color = BackgroundColor(if dragging {
            Color::srgb(0.5, 0.45, 0.2)
        } else {
            Color::srgb(0.2, 0.2, 0.22)
        });
        let label = slot_label(inventory.0.slots.get(slot.0).and_then(|s| s.as_ref()));
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
