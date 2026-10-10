//! Waiting customers at the order window (no new assets).

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use crate::config::RecipeId;
use crate::world::{InKitchen, OrderTicket};
use crate::GameState;

#[derive(Component, Debug)]
pub struct Customer {
    pub order_entity: Entity,
    pub bob_phase: f32,
    pub base_y: f32,
    pub slot: usize,
    pub state: ClientState,
    /// For Leaving clients whose ticket is gone: door exit target.
    pub leaving: bool,
    /// Golden VIP client.
    pub vip: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientState {
    WalkIn,
    Waiting,
    Leaving,
}

/// Door spawn position (south entrance).
pub fn door_position() -> Vec3 {
    Vec3::new(0.0, CUSTOMER_Y, 6.5)
}

pub const WALK_SPEED: f32 = 2.6;

#[derive(Component, Debug, Default)]
pub struct CustomerHead;

#[derive(Component, Debug, Default)]
pub struct CustomerBody;

pub const CUSTOMER_Z: f32 = -5.2;
pub const CUSTOMER_Y: f32 = 0.85;
pub const MAX_CUSTOMERS: usize = 4;

pub fn slot_position(slot: usize) -> Vec3 {
    let i = slot.min(MAX_CUSTOMERS.saturating_sub(1)) as f32;
    Vec3::new(-3.0 + i * 2.0, CUSTOMER_Y, CUSTOMER_Z)
}

pub fn recipe_color(recipe: RecipeId) -> Color {
    match recipe {
        RecipeId::ClassicBurger => Color::srgb(0.95, 0.55, 0.20),
        RecipeId::Cheeseburger => Color::srgb(1.00, 0.80, 0.15),
        RecipeId::DeluxeBurger => Color::srgb(0.80, 0.25, 0.20),
        RecipeId::HeartyStew => Color::srgb(0.55, 0.30, 0.15),
        RecipeId::SundayDinner => Color::srgb(0.75, 0.55, 0.85),
        RecipeId::VeggieBurger => Color::srgb(0.30, 0.75, 0.30),
        RecipeId::VeggieDeluxe => Color::srgb(0.15, 0.60, 0.55),
        RecipeId::NoodleBowl => Color::srgb(0.95, 0.75, 0.25),
    }
}

pub fn skin_color(slot: usize) -> Color {
    const SKINS: [Color; 4] = [
        Color::srgb(1.00, 0.85, 0.70),
        Color::srgb(0.90, 0.70, 0.55),
        Color::srgb(0.65, 0.48, 0.35),
        Color::srgb(0.45, 0.32, 0.24),
    ];
    SKINS[slot % SKINS.len()]
}

fn spawn_customer(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    ticket_entity: Entity,
    recipe: RecipeId,
    slot: usize,
    phase: f32,
    vip: bool,
) -> Entity {
    let pos = slot_position(slot);
    let body_color = if vip { Color::srgb(1.0, 0.8, 0.15) } else { recipe_color(recipe) };
    let body_mesh = meshes.add(Capsule3d::new(0.32, 0.75));
    let head_mesh = meshes.add(Sphere::new(0.26));
    commands.spawn((
        Customer { order_entity: ticket_entity, bob_phase: phase, base_y: pos.y, slot, state: ClientState::WalkIn, leaving: false, vip },
        Mesh3d(body_mesh.clone()),
        MeshMaterial3d(materials.add(StandardMaterial { base_color: body_color, perceptual_roughness: 0.8, ..default() })),
        Transform::from_translation(door_position()),
        Visibility::default(),
        InKitchen,
        CustomerBody,
    )).with_children(|parent| {
        parent.spawn((
            CustomerHead,
            Mesh3d(head_mesh),
            MeshMaterial3d(materials.add(StandardMaterial { base_color: skin_color(slot), perceptual_roughness: 0.6, ..default() })),
            Transform::from_xyz(0.0, 0.85, 0.0),
        ));
    }).id()
}

pub fn sync_customers(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    tickets: Query<(Entity, &OrderTicket)>,
    customers: Query<(Entity, &Customer, &Transform)>,
    time: Res<Time>,
) {
    let mut ordered: Vec<(Entity, RecipeId, f32, bool)> =
        tickets.iter().map(|(e, t)| (e, t.recipe, t.spawned_at, t.vip)).collect();
    ordered.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
    let visible: Vec<(Entity, RecipeId, bool)> =
        ordered.into_iter().take(MAX_CUSTOMERS).map(|(e, r, _, v)| (e, r, v)).collect();
    let visible_set: HashSet<Entity> = visible.iter().map(|(e, _, _)| *e).collect();
    let visible_index: HashMap<Entity, usize> =
        visible.iter().enumerate().map(|(i, (e, _, _))| (*e, i)).collect();

    // Tickets gone (served/expired): clients storm out instead of vanishing.
    for (cust_entity, cust, _) in customers.iter() {
        if !visible_set.contains(&cust.order_entity) && !cust.leaving {
            commands.entity(cust_entity).insert(LeavingNow);
        }
    }
    let mut covered: HashSet<Entity> = HashSet::new();
    for (_, cust, _) in customers.iter() {
        covered.insert(cust.order_entity);
    }
    for (ticket_entity, recipe, vip) in visible.iter() {
        if covered.contains(ticket_entity) {
            continue;
        }
        let slot = *visible_index.get(ticket_entity).unwrap_or(&0);
        let phase = time.elapsed_secs() + (ticket_entity.index() as f32 * 1.37) % 6.28;
        spawn_customer(&mut commands, &mut meshes, &mut materials, *ticket_entity, *recipe, slot, phase, *vip);
    }

    // Re-slot waiting clients so the line stays compact after serves.
    // (Movement itself happens in tick_clients.)
    let _ = visible_index;
}

/// Marker: client's ticket is gone, walk to the door then despawn.
#[derive(Component)]
pub struct LeavingNow;

/// Walk clients: WalkIn -> Waiting (slot), Leaving -> door -> despawn.
pub fn tick_clients(
    mut commands: Commands,
    time: Res<Time>,
    mut bodies: Query<(Entity, &mut Customer, &mut Transform), (With<CustomerBody>, Without<CustomerHead>)>,
    tickets: Query<(Entity, &OrderTicket)>,
    leaving: Query<(), With<LeavingNow>>,
) {
    let dt = time.delta_secs();
    // Current slot per ticket (oldest-first, like sync).
    let mut ordered: Vec<(Entity, f32)> = tickets.iter().map(|(e, t)| (e, t.spawned_at)).collect();
    ordered.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    let slot_of: HashMap<Entity, usize> =
        ordered.into_iter().take(MAX_CUSTOMERS).enumerate().map(|(i, (e, _))| (e, i)).collect();

    for (e, mut cust, mut xf) in bodies.iter_mut() {
        let is_leaving = leaving.get(e).is_ok() || !slot_of.contains_key(&cust.order_entity);
        let target = if is_leaving {
            if cust.state != ClientState::Leaving {
                cust.state = ClientState::Leaving;
                cust.leaving = true;
            }
            door_position()
        } else {
            let slot = slot_of.get(&cust.order_entity).copied().unwrap_or(cust.slot);
            cust.slot = slot;
            if cust.state == ClientState::WalkIn {
                // still walking in; target the slot
            }
            slot_position(slot)
        };
        let to = Vec3::new(target.x - xf.translation.x, 0.0, target.z - xf.translation.z);
        let dist = to.length();
        if is_leaving && dist < 0.35 {
            commands.entity(e).despawn();
            continue;
        }
        if !is_leaving && dist < 0.15 {
            cust.state = ClientState::Waiting;
            // Face the kitchen.
            let face = xf.translation + Vec3::new(0.0, 0.0, 1.0);
            xf.look_at(face, Vec3::Y);
            continue;
        }
        if dist > 0.02 {
            let step = to.normalize() * (WALK_SPEED * dt).min(dist);
            xf.translation.x += step.x;
            xf.translation.z += step.z;
            // Face travel direction.
            let yaw = step.x.atan2(step.z);
            xf.rotation = Quat::from_rotation_y(yaw);
        }
    }
}

pub fn tick_customers(
    time: Res<Time>,
    mut bodies: Query<(Entity, &Customer, &mut Transform), (With<CustomerBody>, Without<CustomerHead>)>,
    mut heads: Query<&mut Transform, (With<CustomerHead>, Without<CustomerBody>)>,
    tickets: Query<&crate::world::OrderTicket>,
    clock: Res<crate::world::LevelClock>,
) {
    let t = time.elapsed_secs();
    for (_e, cust, mut xf) in bodies.iter_mut() {
        // Impatience shake: red-stage tickets (<25% left) tremble.
        let mut shake = 0.0;
        if let Ok(ticket) = tickets.get(cust.order_entity) {
            let total = (ticket.expires_at - ticket.spawned_at).max(0.01);
            let frac = ((ticket.expires_at - clock.elapsed) / total).clamp(0.0, 1.0);
            if frac < 0.25 {
                shake = (1.0 - frac / 0.25) * 0.09;
            }
        }
        xf.translation.y = cust.base_y + (t * 2.2 + cust.bob_phase).sin() * 0.06;
        if shake > 0.0 {
            xf.translation.x += (t * 31.0 + cust.bob_phase).sin() * shake;
        }
    }
    for mut h in heads.iter_mut() {
        h.translation.y = 0.85 + (t * 3.0).sin() * 0.02;
        h.rotation = Quat::from_rotation_y((t * 0.8).sin() * 0.25);
    }
}

pub fn customers_plugin(app: &mut App) {
    app.add_systems(Update, (sync_customers, tick_customers).run_if(in_state(GameState::Playing)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_spread_across_window() {
        let p0 = slot_position(0);
        let p3 = slot_position(3);
        assert!((p0.z - CUSTOMER_Z).abs() < 0.001);
        assert!(p0.x < p3.x);
        assert!((p0.x - (-3.0)).abs() < 0.001);
        assert!((p3.x - 3.0).abs() < 0.001);
    }

    #[test]
    fn recipe_colors_distinct() {
        assert_ne!(recipe_color(RecipeId::ClassicBurger), recipe_color(RecipeId::VeggieBurger));
    }
}
