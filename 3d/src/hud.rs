//! The HUD: writer systems fill HudRes from game state, one
//! renderer reads only HudRes to paint the screen text. The
//! inventory panel and the hotbar paint the player's holders in
//! place and forward gestures to modforge's decisions (start_drag,
//! end_drag, use_slot, drop_slot). Atlas controls (design.md).

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use modforge::hud::{
    HOTBAR_SLOTS, Holder, Holders, HudState, OpenPanel, SlotRef, drop_slot, end_drag, hotbar_key,
    prompt_for, slot_label, start_drag, toggle_panel, transfer_slot, use_slot,
};

use crate::actor::{Equipment, Hotbar, INVENTORY_SLOTS, Inventory};
use crate::cel::CelMaterial;
use crate::content::ItemRegistryRes;
use crate::input::{Action, ActionQueue, ThisTick};
use crate::interact::{Container, LookingAt, OpenContainer, WorldItem, spawn_item};
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
/// Slots the container panel paints: the largest storage any
/// deployable has. Smaller boxes leave the rest blank.
const CONTAINER_PANEL_SLOTS: usize = 12;

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
            for index in 0..CONTAINER_PANEL_SLOTS {
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
            hotbar: &mut hotbar.into_inner().0.slots,
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

/// `Inventory` (I or Tab) opens and closes the panel; `Cancel`
/// (Escape) closes it.
pub fn toggle_inventory(
    mut hud: ResMut<HudRes>,
    mut hovered: ResMut<HoveredSlot>,
    mut open_container: ResMut<OpenContainer>,
    mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>,
    players: Query<&ThisTick, With<Player>>,
) {
    let Ok(this_tick) = players.single() else {
        return;
    };
    // Escape closes whatever panel is open; a note closes without
    // touching the cursor, as opening it never released it.
    if hud.0.open_panel == OpenPanel::Note && ActionQueue::has(&this_tick.0, Action::Cancel) {
        toggle_panel(&mut hud.0, OpenPanel::Note);
        hud.0.reading = None;
        return;
    }
    let open = hud.0.open_panel == OpenPanel::Inventory;
    let toggle = ActionQueue::has(&this_tick.0, Action::Inventory);
    let cancel = open && ActionQueue::has(&this_tick.0, Action::Cancel);
    if toggle || cancel {
        toggle_panel(&mut hud.0, OpenPanel::Inventory);
        hud.0.reading = None;
        panel_changed(&mut hud.0, &mut hovered, &mut open_container, &mut cursors);
    }
}

pub fn inventory_open(hud: Res<HudRes>) -> bool {
    hud.0.open_panel == OpenPanel::Inventory
}

/// `Hotbar { index }` uses that bar slot, panel open or not. The
/// key-to-slot order is modforge's `hotbar_slot`, applied by the
/// bindings.
pub fn hotbar_keys(
    registry: Res<ItemRegistryRes>,
    mut players: Query<(&mut Hotbar, &mut Survival, &mut Equipment, &ThisTick), With<Player>>,
) {
    let Ok((mut hotbar, mut survival, mut equipment, this_tick)) = players.single_mut() else {
        return;
    };
    for stamped in &this_tick.0 {
        if let Action::Hotbar { index } = stamped.action {
            let _ = hotbar_key(&mut survival.0, &mut hotbar.0, index, &registry.0, &mut equipment.0);
        }
    }
}

/// Double left click uses the slot (modforge `use_slot`; food is
/// eaten). A single click does nothing, as in Atlas.
fn slot_clicked(
    click: On<Pointer<Click>>,
    slots: Query<&Slot>,
    registry: Res<ItemRegistryRes>,
    mut holders: OnScreenHolders,
    mut players: Query<(&mut Survival, &mut Equipment), With<Player>>,
) {
    if click.button != PointerButton::Primary || click.count < 2 {
        return;
    }
    let Ok(slot) = slots.get(click.entity) else {
        return;
    };
    let (Some(holder), Ok((mut survival, mut equipment))) =
        (holders.one(slot.0), players.single_mut())
    else {
        return;
    };
    // A refused use (not food, empty) changes nothing.
    let _ = use_slot(&mut survival.0, holder, slot.0.index, &registry.0, &mut equipment.0);
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
    hovered: Res<HoveredSlot>,
    registry: Res<ItemRegistryRes>,
    mut holders: OnScreenHolders,
    players: Query<&ThisTick, With<Player>>,
) {
    let Ok(this_tick) = players.single() else {
        return;
    };
    let Some(half) = this_tick.0.iter().find_map(|s| match s.action {
        Action::Transfer { half } => Some(half),
        _ => None,
    }) else {
        return;
    };
    let (Some(slot), Some(mut holders)) = (hovered.0, holders.get()) else {
        return;
    };
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
    hovered: Res<HoveredSlot>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<CelMaterial>>,
    registry: Res<ItemRegistryRes>,
    mut holders: OnScreenHolders,
    players: Query<(&Transform, &ThisTick), With<Player>>,
) {
    let (Some(slot), Ok((transform, this_tick))) = (hovered.0, players.single()) else {
        return;
    };
    if !ActionQueue::has(&this_tick.0, Action::Drop) {
        return;
    }
    let Some(stack) = holders.one(slot).and_then(|holder| drop_slot(holder, slot.index)) else {
        return;
    };
    let Some(def) = registry.0.def(&stack.item) else {
        return;
    };
    let feet = transform.translation.with_y(0.3) + transform.forward().with_y(0.0) * 1.0;
    spawn_item(
        &mut commands,
        &mut meshes,
        &mut materials,
        def,
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
        // The held bar slot is lit (Rust's selected slot).
        let held = slot.0.holder == Holder::Hotbar && hotbar.0.held == Some(slot.0.index);
        *color = BackgroundColor(if dragging {
            Color::srgb(0.5, 0.45, 0.2)
        } else if held {
            Color::srgb(0.35, 0.35, 0.4)
        } else {
            Color::srgb(0.2, 0.2, 0.22)
        });
        let holder = match slot.0.holder {
            Holder::Inventory => Some(&inventory.0),
            Holder::Hotbar => Some(&hotbar.0.slots),
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
    diagnostics: Res<DiagnosticsStore>,
    registry: Res<ItemRegistryRes>,
    players: Query<(&Survival, &crate::combat::Health, &Inventory, &Hotbar), With<Player>>,
    mut huds: Query<&mut Text, With<Hud>>,
) {
    let Ok(mut text) = huds.single_mut() else {
        return;
    };
    let Ok((survival, health, inventory, hotbar)) = players.single() else {
        return;
    };
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
        .unwrap_or(0.0);
    let mut line = format!(
        "FPS: {fps:.0}  Health: {:.0}  Hunger: {:.0}  Thirst: {:.0}  Rest: {:.0}  Safety: {:.0}",
        health.0.current, survival.0.hunger, survival.0.thirst, survival.0.rest, survival.0.safety
    );
    // What is in the hands and, for a gun, the rounds left anywhere
    // on the player.
    if let Some(held) = hotbar.0.held_stack() {
        line.push_str(&format!("  Held: {}", held.item));
        if let Some(ammo) = registry
            .0
            .def(&held.item)
            .and_then(|d| d.combat.as_ref())
            .and_then(|c| c.ammo.as_deref())
        {
            let rounds = hotbar.0.slots.count_of(ammo) + inventory.0.count_of(ammo);
            line.push_str(&format!(" ({ammo} x{rounds})"));
        }
    }
    if state.0.prompt.can_interact {
        line.push('\n');
        line.push_str(&state.0.prompt.text);
    }
    if let Some((name, current, max)) = &state.0.target {
        line.push_str(&format!("\n{name}: {current:.0}/{max:.0}"));
    }
    // The note panel: its words, then who signed them.
    if state.0.open_panel == OpenPanel::Note
        && let Some(note) = &state.0.reading
    {
        line.push_str(&format!("\n\n{}\n    {}", note.text, note.signed));
    }
    text.0 = line;
}

/// The actor under the crosshair, for the HUD: a ray from the eye
/// against the bodies, the health read off the body's root.
pub fn write_target(
    spatial: avian3d::prelude::SpatialQuery,
    players: Query<(Entity, &Transform, Option<&Children>), With<Player>>,
    bodies: Query<&ChildOf, With<crate::actor::Body>>,
    actors: Query<(&crate::actor::Npc, &crate::combat::Health)>,
    sensors: Query<(), With<avian3d::prelude::Sensor>>,
    mut state: ResMut<HudRes>,
) {
    state.0.target = None;
    let Ok((player, transform, children)) = players.single() else {
        return;
    };
    let own: Vec<Entity> = std::iter::once(player)
        .chain(children.into_iter().flat_map(|c| c.iter()))
        .collect();
    let filter = avian3d::prelude::SpatialQueryFilter::from_excluded_entities(own);
    // Small items are sensors; the target line looks past them.
    let solid = |entity: Entity| sensors.get(entity).is_err();
    let Some(hit) = spatial.cast_ray_predicate(
        transform.translation,
        transform.forward(),
        50.0,
        true,
        &filter,
        &solid,
    ) else {
        return;
    };
    let Ok(child_of) = bodies.get(hit.entity) else {
        return;
    };
    if let Ok((npc, health)) = actors.get(child_of.parent()) {
        state.0.target = Some((npc.def.clone(), health.0.current, health.0.max));
    }
}
