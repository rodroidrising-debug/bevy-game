//! 3D patience bars floating above waiting customers.
//!
//! One thin unlit box per visible customer ticket. The bar hovers above the
//! customer's head (`slot_position(slot) + y 2.0`), shrinks along x as the
//! ticket nears expiry, and shifts green -> yellow -> red.
//!
//! Wiring (in `main.rs`):
//! ```ignore
//! mod patience;
//! // Startup:
//! app.add_systems(Startup, patience::setup_patience);
//! // Playing:
//! app.add_systems(Update, patience::sync_patience.run_if(in_state(GameState::Playing)));
//! ```

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use crate::customers::{slot_position, Customer, MAX_CUSTOMERS};
use crate::world::{InKitchen, LevelClock, OrderTicket};
use crate::GameState;

/// Height above the customer slot origin where the bar floats.
pub const BAR_HEIGHT_ABOVE_SLOT: f32 = 2.0;
/// Bar dimensions: thin box, 1 unit wide at full patience.
pub const BAR_SIZE: Vec3 = Vec3::new(1.0, 0.12, 0.05);
/// Minimum x-scale so an almost-expired bar is still visible.
pub const BAR_MIN_SCALE_X: f32 = 0.02;

/// Marker for a single patience bar; tracks the ticket it visualizes.
#[derive(Component, Debug)]
pub struct PatienceBar {
    /// The [`OrderTicket`] entity this bar visualizes.
    pub order: Entity,
}

/// Shared mesh + per-stage unlit materials for patience bars.
#[derive(Resource, Debug)]
pub struct PatienceAssets {
    pub mesh: Handle<Mesh>,
    pub green: Handle<StandardMaterial>,
    pub yellow: Handle<StandardMaterial>,
    pub red: Handle<StandardMaterial>,
}

/// Build the shared bar mesh and the three unlit stage materials.
///
/// Startup system; safe to run once. Inserted as [`PatienceAssets`].
pub fn setup_patience(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(Cuboid::new(BAR_SIZE.x, BAR_SIZE.y, BAR_SIZE.z));
    let mut unlit = |r: f32, g: f32, b: f32| {
        materials.add(StandardMaterial {
            base_color: Color::srgb(r, g, b),
            unlit: true,
            ..default()
        })
    };
    commands.insert_resource(PatienceAssets {
        mesh,
        green: unlit(0.25, 0.85, 0.30),
        yellow: unlit(1.0, 0.85, 0.15),
        red: unlit(0.95, 0.25, 0.20),
    });
}

/// Fraction of patience remaining, clamped to `0..=1`.
///
/// `remaining` is seconds left (`expires_at - elapsed`), `total` is the full
/// ticket lifetime (`expires_at - spawned_at`). A non-positive total yields
/// `0.0` (nothing left) instead of dividing by zero.
pub fn patience_fraction(remaining: f32, total: f32) -> f32 {
    if total <= 0.0 {
        return 0.0;
    }
    (remaining / total).clamp(0.0, 1.0)
}

/// Discrete color stage for a patience fraction.
///
/// - `0` green  — fraction `> 0.5`
/// - `1` yellow — fraction `> 0.25`
/// - `2` red    — anything else (including `0.0`)
pub fn patience_stage(frac: f32) -> u8 {
    if frac > 0.5 {
        0
    } else if frac > 0.25 {
        1
    } else {
        2
    }
}

/// Pick the material handle for a stage from [`PatienceAssets`].
fn material_for_stage(assets: &PatienceAssets, stage: u8) -> Handle<StandardMaterial> {
    match stage {
        0 => assets.green.clone(),
        1 => assets.yellow.clone(),
        _ => assets.red.clone(),
    }
}

/// Bar world position for a customer slot: above the head.
pub fn bar_position_for_slot(slot: usize) -> Vec3 {
    slot_position(slot) + Vec3::new(0.0, BAR_HEIGHT_ABOVE_SLOT, 0.0)
}

/// Keep patience bars in sync with tickets + customers.
///
/// - Spawns one bar per visible customer ticket (customer `order_entity`
///   set, capped at [`MAX_CUSTOMERS`]).
/// - Despawns bars whose ticket is gone (served / expired) or no longer has
///   a visible customer.
/// - Every frame updates each bar's position (above its customer slot),
///   `scale.x` (remaining fraction clamped to `0.02..=1.0`), and material
///   (`> 0.5` green, `> 0.25` yellow, else red).
///
/// `assets` is `Option` so the system silently no-ops before
/// [`setup_patience`] has run (e.g. in unit tests or early startup frames).
pub fn sync_patience(
    mut commands: Commands,
    assets: Option<Res<PatienceAssets>>,
    tickets: Query<(Entity, &OrderTicket)>,
    customers: Query<&Customer>,
    mut bars: Query<(Entity, &PatienceBar, &mut Transform, &mut MeshMaterial3d<StandardMaterial>)>,
    clock: Res<LevelClock>,
) {
    let Some(assets) = assets else {
        return;
    };

    // Ticket lookup: entity -> ticket times.
    let mut ticket_map: HashMap<Entity, (f32, f32)> = HashMap::new();
    for (e, t) in tickets.iter() {
        ticket_map.insert(e, (t.spawned_at, t.expires_at));
    }

    // Customer lookup: ticket entity -> slot. Only tickets that still exist
    // count as visible; cap at MAX_CUSTOMERS for safety.
    let mut order_slot: HashMap<Entity, usize> = HashMap::new();
    for cust in customers.iter() {
        if ticket_map.contains_key(&cust.order_entity) && !order_slot.contains_key(&cust.order_entity)
        {
            order_slot.insert(cust.order_entity, cust.slot);
        }
        if order_slot.len() >= MAX_CUSTOMERS {
            break;
        }
    }
    let visible: HashSet<Entity> = order_slot.keys().copied().collect();

    // Despawn bars whose order vanished or lost its customer.
    for (bar_entity, bar, _, _) in bars.iter() {
        if !visible.contains(&bar.order) {
            commands.entity(bar_entity).despawn();
        }
    }

    // Which orders already have a bar?
    let mut covered: HashSet<Entity> = HashSet::new();
    for (_, bar, _, _) in bars.iter() {
        covered.insert(bar.order);
    }

    // Spawn missing bars at their current position / scale / color.
    for order in visible.iter() {
        if covered.contains(order) {
            continue;
        }
        let Some(&(spawned_at, expires_at)) = ticket_map.get(order) else {
            continue;
        };
        let slot = order_slot.get(order).copied().unwrap_or(0);
        let total = expires_at - spawned_at;
        let remaining = expires_at - clock.elapsed;
        let frac = patience_fraction(remaining, total);
        let stage = patience_stage(frac);
        let scale_x = frac.clamp(BAR_MIN_SCALE_X, 1.0);
        let mut pos = bar_position_for_slot(slot);
        // Left-anchor the shrink so the bar drains right-to-left.
        pos.x -= (1.0 - scale_x) * 0.5;
        commands.spawn((
            PatienceBar { order: *order },
            Mesh3d(assets.mesh.clone()),
            MeshMaterial3d(material_for_stage(&assets, stage)),
            Transform {
                translation: pos,
                scale: Vec3::new(scale_x, 1.0, 1.0),
                ..default()
            },
            Visibility::default(),
            InKitchen,
        ));
    }

    // Per-frame refresh of every surviving bar.
    for (_, bar, mut xf, mut mat) in bars.iter_mut() {
        let Some(slot) = order_slot.get(&bar.order).copied() else {
            continue;
        };
        let Some(&(spawned_at, expires_at)) = ticket_map.get(&bar.order) else {
            continue;
        };
        let total = expires_at - spawned_at;
        let remaining = expires_at - clock.elapsed;
        let frac = patience_fraction(remaining, total);
        let stage = patience_stage(frac);
        let scale_x = frac.clamp(BAR_MIN_SCALE_X, 1.0);
        let mut pos = bar_position_for_slot(slot);
        pos.x -= (1.0 - scale_x) * 0.5;
        xf.translation = pos;
        xf.scale = Vec3::new(scale_x, 1.0, 1.0);
        mat.0 = material_for_stage(&assets, stage);
    }
}

/// Register [`setup_patience`] (Startup) + [`sync_patience`] (Playing).
pub fn patience_plugin(app: &mut App) {
    app.add_systems(Startup, setup_patience);
    app.add_systems(Update, sync_patience.run_if(in_state(GameState::Playing)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fraction_clamps_and_guards() {
        assert!((patience_fraction(75.0, 75.0) - 1.0).abs() < 1e-6);
        assert!((patience_fraction(37.5, 75.0) - 0.5).abs() < 1e-6);
        assert!((patience_fraction(0.0, 75.0) - 0.0).abs() < 1e-6);
        // Over-full and negative remaining clamp.
        assert!((patience_fraction(100.0, 75.0) - 1.0).abs() < 1e-6);
        assert!((patience_fraction(-5.0, 75.0) - 0.0).abs() < 1e-6);
        // Degenerate totals never divide by zero.
        assert_eq!(patience_fraction(5.0, 0.0), 0.0);
        assert_eq!(patience_fraction(5.0, -10.0), 0.0);
    }

    #[test]
    fn stage_thresholds() {
        assert_eq!(patience_stage(1.0), 0); // full => green
        assert_eq!(patience_stage(0.51), 0);
        assert_eq!(patience_stage(0.5), 1); // boundary => yellow
        assert_eq!(patience_stage(0.26), 1);
        assert_eq!(patience_stage(0.25), 2); // boundary => red
        assert_eq!(patience_stage(0.0), 2);
        assert_eq!(patience_stage(-1.0), 2);
    }

    #[test]
    fn green_yellow_red_ordering() {
        // Later fractions map to hotter stages.
        assert!(patience_stage(0.9) < patience_stage(0.4));
        assert!(patience_stage(0.4) < patience_stage(0.1));
    }

    #[test]
    fn bar_floats_above_slot() {
        let pos = bar_position_for_slot(0);
        let slot = slot_position(0);
        assert!((pos.x - slot.x).abs() < 1e-6);
        assert!((pos.z - slot.z).abs() < 1e-6);
        assert!((pos.y - (slot.y + BAR_HEIGHT_ABOVE_SLOT)).abs() < 1e-6);
    }

    #[test]
    fn bar_size_is_thin_box() {
        assert!((BAR_SIZE.x - 1.0).abs() < 1e-6);
        assert!((BAR_SIZE.y - 0.12).abs() < 1e-6);
        assert!((BAR_SIZE.z - 0.05).abs() < 1e-6);
    }
}
