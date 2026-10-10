//! Kitchen ambience: per-restaurant lighting rigs, stove fire flicker,
//! player walk animation, station progress bars, and seated diners.
//!
//! Wiring (already applied in `main.rs`):
//! ```ignore
//! mod ambience;
//! // Startup:
//! ambience::setup_progress,
//! // OnEnter(Playing) (order matters: lights -> kitchen -> diners -> HUD):
//! //   setup_lights_camera, ambience::restaurant_lighting_system,
//! //   spawn_kitchen, ambience::spawn_diners_system, ui::setup_hud
//! // Update, run_if(in_state(GameState::Playing)) — register AFTER
//! // player_movement so player_anim wins the per-frame y write:
//! //   ambience::sync_stove_glows, ambience::flicker_glows,
//! //   ambience::player_anim, ambience::sync_progress_bars, ambience::tick_diners
//! ```
//!
//! Only `bevy::prelude::*` plus local crate items are used, and only
//! `Cuboid` / `Sphere` / `Capsule3d` primitives (no `Torus`).

use bevy::prelude::*;

use crate::world::{InKitchen, Player, SelectedLevel, Station, StationKind};

// ===========================================================================
// 1. Per-restaurant lighting rigs
// ===========================================================================

/// Warm bistro sun tint (L0).
const L0_SUN: (f32, f32, f32) = (1.0, 0.95, 0.87);
/// Cozy amber sun tint (L1).
const L1_SUN: (f32, f32, f32) = (1.0, 0.84, 0.62);
/// Bright fresh daylight sun tint (L2).
const L2_SUN: (f32, f32, f32) = (0.96, 0.98, 1.0);

/// Per-restaurant sun rig: `(sun color, sun illuminance in lux, sun position)`.
///
/// * L0 — warm bistro: slightly orange sun, `9000` lux.
/// * L1 — cozy & dimmer: amber sun, `7500` lux, lower sun angle.
/// * L2 — bright fresh daylight: near-white sun, `11000` lux, high sun.
///
/// Levels above 2 reuse the L2 rig.
pub fn restaurant_lighting(level: usize) -> (Color, f32, Vec3) {
    match level {
        0 => (
            Color::srgb(L0_SUN.0, L0_SUN.1, L0_SUN.2),
            9000.0,
            Vec3::new(6.0, 12.0, 4.0),
        ),
        1 => (
            Color::srgb(L1_SUN.0, L1_SUN.1, L1_SUN.2),
            7500.0,
            Vec3::new(4.0, 10.0, 6.0),
        ),
        _ => (
            Color::srgb(L2_SUN.0, L2_SUN.1, L2_SUN.2),
            11000.0,
            Vec3::new(2.0, 14.0, 3.0),
        ),
    }
}

/// Ambient part of the rig: `(tint, brightness)`.
///
/// `AmbientLight::brightness` is in cd/m^2 with a Bevy default of `80.0`,
/// so values stay in that neighbourhood: L0 warm `95`, L1 dim amber `55`,
/// L2 bright neutral `135`.
fn ambient_rig(level: usize) -> (Color, f32) {
    match level {
        0 => (Color::srgb(1.0, 0.92, 0.80), 95.0),
        1 => (Color::srgb(1.0, 0.80, 0.58), 55.0),
        _ => (Color::srgb(0.90, 0.95, 1.0), 135.0),
    }
}

/// Apply the restaurant rig for `level`: sun color + illuminance and ambient
/// color + brightness.
///
/// Plain helper (not a system: `level: usize` is not a `SystemParam`).
/// Call it from [`restaurant_lighting_system`], or from your own system by
/// moving the params through:
///
/// ```ignore
/// fn my_wrap(level: Res<SelectedLevel>, sun_q: Query<&mut DirectionalLight>, amb: Option<ResMut<AmbientLight>>) {
///     apply_restaurant_lighting(level.0, sun_q, amb);
/// }
/// ```
///
/// The sun *position* from [`restaurant_lighting`] is informational here
/// (the query carries no `Transform`); use [`restaurant_lighting_system`]
/// if the sun should also be repositioned.
pub fn apply_restaurant_lighting(
    level: usize,
    mut sun_q: Query<&mut DirectionalLight>,
    ambient: Option<ResMut<AmbientLight>>,
) {
    let (sun_color, sun_lux, _sun_pos) = restaurant_lighting(level);
    let (amb_color, amb_bright) = ambient_rig(level);
    for mut sun in sun_q.iter_mut() {
        sun.color = sun_color;
        sun.illuminance = sun_lux;
    }
    if let Some(mut amb) = ambient {
        amb.color = amb_color;
        amb.brightness = amb_bright;
    }
}

/// System wrapper: applies the rig for the [`SelectedLevel`], including sun
/// repositioning. Register on `OnEnter(GameState::Playing)` (after
/// `setup_lights_camera` so the sun exists).
pub fn restaurant_lighting_system(
    selected: Res<SelectedLevel>,
    mut sun_q: Query<(&mut DirectionalLight, &mut Transform)>,
    ambient: Option<ResMut<AmbientLight>>,
) {
    let level = selected.0;
    let (sun_color, sun_lux, sun_pos) = restaurant_lighting(level);
    let (amb_color, amb_bright) = ambient_rig(level);
    for (mut sun, mut tf) in sun_q.iter_mut() {
        sun.color = sun_color;
        sun.illuminance = sun_lux;
        tf.translation = sun_pos;
        tf.look_at(Vec3::ZERO, Vec3::Y);
    }
    if let Some(mut amb) = ambient {
        amb.color = amb_color;
        amb.brightness = amb_bright;
    }
}

// ===========================================================================
// 2. Stove fire flicker
// ===========================================================================

/// Height above a station's origin where the fire glow hovers.
pub const STOVE_GLOW_Y: f32 = 1.2;
/// A glow belongs to a station anchor within this radius.
const GLOW_MATCH_RADIUS: f32 = 0.9;

/// Marker for fire-glow lights hovering over cooking stations.
#[derive(Component)]
pub struct StoveGlow;

/// Ensure exactly one glow light per actively-processing `Stove`/`Oven`.
///
/// Matching is positional (glow anchor = station pos + `STOVE_GLOW_Y`):
/// glows with no processing station nearby are despawned (finished, picked
/// up, or kitchen torn down), and missing glows are spawned. The extra
/// `glow_xf` query is what makes the mapping exact while keeping the
/// requested `(commands, stations, glows)` params intact.
pub fn sync_stove_glows(
    mut commands: Commands,
    stations: Query<(Entity, &Transform, &Station)>,
    glows: Query<Entity, With<StoveGlow>>,
    glow_xf: Query<&Transform, With<StoveGlow>>,
) {
    // Anchors that currently need a glow.
    let mut need: Vec<Vec3> = Vec::new();
    for (_e, tf, st) in stations.iter() {
        if st.processing.is_some()
            && matches!(st.kind, StationKind::Stove | StationKind::Oven)
        {
            need.push(tf.translation + Vec3::new(0.0, STOVE_GLOW_Y, 0.0));
        }
    }

    // Despawn glows whose station finished / was picked up / is gone.
    for e in glows.iter() {
        let pos = glow_xf
            .get(e)
            .map(|t| t.translation)
            .unwrap_or(Vec3::splat(f32::MAX));
        if !need.iter().any(|a| a.distance(pos) < GLOW_MATCH_RADIUS) {
            commands.entity(e).despawn();
        }
    }

    // Spawn one glow per uncovered anchor.
    for anchor in need.iter() {
        let covered = glow_xf
            .iter()
            .any(|t| t.translation.distance(*anchor) < GLOW_MATCH_RADIUS);
        if !covered {
            commands.spawn((
                PointLight {
                    color: Color::srgb(1.0, 0.52, 0.13),
                    intensity: 8.0,
                    range: 4.0,
                    ..default()
                },
                Transform::from_translation(*anchor),
                StoveGlow,
                InKitchen,
            ));
        }
    }
}

/// Flicker every glow: warm shimmer driven by time and world x/z position,
/// plus a small vertical bob. Position phase keeps neighbouring stoves from
/// pulsing in lockstep.
pub fn flicker_glows(
    time: Res<Time>,
    mut glows: Query<(&mut PointLight, &mut Transform), With<StoveGlow>>,
) {
    let t = time.elapsed_secs();
    for (mut light, mut tf) in glows.iter_mut() {
        let x = tf.translation.x;
        let z = tf.translation.z;
        light.intensity = 7.0 + 2.5 * (t * 13.0 + x * 7.0 + z * 3.0).sin();
        tf.translation.y = STOVE_GLOW_Y + (t * 9.0 + x * 3.0 + z * 1.7).sin() * 0.05;
    }
}

// ===========================================================================
// 3. Player walk animation (non-authoritative: y bob + tilt only)
// ===========================================================================

/// Base hover height of the player capsule (matches `player_movement`).
pub const PLAYER_BASE_Y: f32 = 0.8;
/// Bob amplitude while walking.
const WALK_BOB: f32 = 0.06;
/// Roll lean per unit of strafe input.
const STRAFE_LEAN: f32 = -0.08;
/// How fast yaw/roll/y ease toward their targets.
const ANIM_EASE: f32 = 12.0;

fn key_axis(keys: &ButtonInput<KeyCode>, neg: KeyCode, pos: KeyCode) -> f32 {
    (keys.pressed(pos) as u8 as f32) - (keys.pressed(neg) as u8 as f32)
}

fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    use std::f32::consts::PI;
    let d = (b - a + PI * 3.0).rem_euclid(PI * 2.0) - PI;
    a + d * t.clamp(0.0, 1.0)
}

/// Walk bob + strafe lean + face-movement yaw.
///
/// Runs *after* `player_movement` and never touches x/z, so the movement
/// system stays authoritative for position. While any WASD/arrow key is
/// held, `y = 0.8 + |sin(t*10)| * 0.06`; roll leans into strafe
/// (`x_input * -0.08`); yaw eases toward the movement heading. When idle,
/// y/roll ease back to rest while the last facing is kept.
pub fn player_anim(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut player_q: Query<&mut Transform, With<Player>>,
) {
    let strafe = key_axis(&keys, KeyCode::KeyA, KeyCode::KeyD)
        + key_axis(&keys, KeyCode::ArrowLeft, KeyCode::ArrowRight);
    let fwd = key_axis(&keys, KeyCode::KeyW, KeyCode::KeyS)
        + key_axis(&keys, KeyCode::ArrowUp, KeyCode::ArrowDown);
    let strafe = strafe.clamp(-1.0, 1.0);
    // World-space move dir: -z is "up" on screen (W), +z is "down" (S).
    let mx = strafe;
    let mz = -fwd;
    let moving = mx.abs() > 0.01 || mz.abs() > 0.01;

    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    let k = (ANIM_EASE * dt).clamp(0.0, 1.0);

    for mut tf in player_q.iter_mut() {
        let (_, yaw, _) = tf.rotation.to_euler(EulerRot::YXZ);
        if moving {
            tf.translation.y = PLAYER_BASE_Y + (t * 10.0).sin().abs() * WALK_BOB;
            // Face the movement heading. Forward is -Z rotated by yaw:
            // (-sin(yaw), -cos(yaw)) == (mx, mz)  =>  yaw = atan2(-mx, -mz).
            let target_yaw = (-mx).atan2(-mz);
            let target_roll = strafe * STRAFE_LEAN;
            let new_yaw = lerp_angle(yaw, target_yaw, k);
            let (_, _, roll) = tf.rotation.to_euler(EulerRot::YXZ);
            tf.rotation = Quat::from_euler(
                EulerRot::YXZ,
                new_yaw,
                0.0,
                roll + (target_roll - roll) * k,
            );
        } else {
            // Ease back to rest pose, keep last facing.
            tf.translation.y += (PLAYER_BASE_Y - tf.translation.y) * k;
            let (_, _, roll) = tf.rotation.to_euler(EulerRot::YXZ);
            tf.rotation = Quat::from_euler(EulerRot::YXZ, yaw, 0.0, roll * (1.0 - k));
        }
    }
}

// ===========================================================================
// 4. Progress bars above processing stations
// ===========================================================================

/// Height above a station's origin where progress bars hover.
pub const PROGRESS_Y: f32 = 1.9;
/// Outer (background) bar size.
const BAR_BACK_SIZE: Vec3 = Vec3::new(1.1, 0.09, 0.09);
/// Inner (fill) bar thickness; x scale is the fraction.
const BAR_FILL_YZ: (f32, f32) = (0.11, 0.11);
/// Full inner width (fill grows left-aligned inside this).
const BAR_INNER_W: f32 = 1.0;

/// Link from a bar entity back to the station it visualises.
#[derive(Component)]
pub struct ProgressBar {
    pub station: Entity,
}

/// Marker distinguishing the fill quad from the background quad.
#[derive(Component)]
pub struct ProgressFill;

/// Shared meshes/materials for progress bars.
#[derive(Resource)]
pub struct ProgressAssets {
    pub mesh: Handle<Mesh>,
    pub fill_mat: Handle<StandardMaterial>,
    pub back_mat: Handle<StandardMaterial>,
}

/// Fraction of a timed task that has elapsed, clamped to `[0, 1]`.
/// A non-positive `total` means "instant", i.e. already complete (`1.0`).
pub fn progress_fraction(elapsed: f32, total: f32) -> f32 {
    if total <= 0.0 {
        return 1.0;
    }
    (elapsed / total).clamp(0.0, 1.0)
}

/// Fill color ramp: fresh green (`0.0`) → hot orange (`1.0`).
pub fn bar_color(frac: f32) -> Color {
    let f = frac.clamp(0.0, 1.0);
    Color::srgb(
        0.25 + (1.0 - 0.25) * f,
        0.85 + (0.55 - 0.85) * f,
        0.30 + (0.10 - 0.30) * f,
    )
}

/// Flashing red used when food is done but can still burn.
pub fn danger_color(time_s: f32) -> Color {
    let pulse = 0.5 + 0.5 * (time_s * 8.0).sin();
    Color::srgb(1.0, 0.12 + 0.20 * pulse, 0.08)
}

/// Startup: build the shared thin-box assets. Idempotent (skips if the
/// resource already exists, e.g. after a hot reload of schedules).
pub fn setup_progress(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing: Option<Res<ProgressAssets>>,
) {
    if existing.is_some() {
        return;
    }
    commands.insert_resource(ProgressAssets {
        mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        fill_mat: materials.add(StandardMaterial {
            base_color: bar_color(0.0),
            unlit: true,
            ..default()
        }),
        back_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.08, 0.08, 0.10),
            unlit: true,
            ..default()
        }),
    });
}

/// One background + fill bar per station with `processing.is_some()`.
///
/// * Bars hover at station pos + `PROGRESS_Y` and follow the station.
/// * Fill `scale.x = elapsed / total` (left-aligned), colored green→orange,
///   flashing red while food sits done-but-burnable.
/// * Each fill gets its own cloned material so per-bar colors are possible
///   despite the shared base in [`ProgressAssets`].
/// * Bars whose station finished / was removed are despawned.
#[allow(clippy::type_complexity)]
pub fn sync_progress_bars(
    mut commands: Commands,
    stations: Query<(Entity, &Transform, &Station)>,
    mut bars: Query<(
        Entity,
        &ProgressBar,
        &mut Transform,
        Option<&ProgressFill>,
        Option<&MeshMaterial3d<StandardMaterial>>,
    )>,
    progress: Option<Res<ProgressAssets>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
) {
    let Some(progress) = progress else { return };
    let t = time.elapsed_secs();

    // Active stations: entity -> (anchor pos, fraction, burn-danger).
    use std::collections::{HashMap, HashSet};
    let mut active: HashMap<Entity, (Vec3, f32, bool)> = HashMap::new();
    for (e, tf, st) in stations.iter() {
        if let Some(pr) = st.processing.as_ref() {
            let frac = progress_fraction(pr.elapsed, pr.total);
            let danger = pr.burn_after.is_some() && pr.elapsed >= pr.total;
            active.insert(e, (tf.translation + Vec3::new(0.0, PROGRESS_Y, 0.0), frac, danger));
        }
    }

    // Index existing bars per station.
    let mut backs: HashMap<Entity, Entity> = HashMap::new();
    let mut fills: HashMap<Entity, Entity> = HashMap::new();
    for (bar_e, link, _, is_fill, _) in bars.iter() {
        if is_fill.is_some() {
            fills.insert(link.station, bar_e);
        } else {
            backs.insert(link.station, bar_e);
        }
    }

    // Despawn bars whose station is no longer processing.
    let live: HashSet<Entity> = active.keys().copied().collect();
    for (bar_e, link, _, _, _) in bars.iter() {
        if !live.contains(&link.station) {
            commands.entity(bar_e).despawn();
        }
    }

    // Spawn missing halves.
    for (st_e, (anchor, frac, _)) in active.iter() {
        if !backs.contains_key(st_e) {
            commands.spawn((
                Mesh3d(progress.mesh.clone()),
                MeshMaterial3d(progress.back_mat.clone()),
                Transform {
                    translation: *anchor,
                    scale: BAR_BACK_SIZE,
                    ..default()
                },
                ProgressBar { station: *st_e },
                InKitchen,
            ));
        }
        if !fills.contains_key(st_e) {
            // Per-fill material clone so each bar can carry its own color.
            let base = materials
                .get(&progress.fill_mat)
                .cloned()
                .unwrap_or(StandardMaterial {
                    base_color: bar_color(*frac),
                    unlit: true,
                    ..default()
                });
            let mat = materials.add(StandardMaterial {
                base_color: bar_color(*frac),
                ..base
            });
            let w = (*frac * BAR_INNER_W).max(0.03);
            commands.spawn((
                Mesh3d(progress.mesh.clone()),
                MeshMaterial3d(mat),
                Transform {
                    translation: Vec3::new(
                        anchor.x - (BAR_INNER_W - w) * 0.5,
                        anchor.y,
                        anchor.z,
                    ),
                    scale: Vec3::new(w, BAR_FILL_YZ.0, BAR_FILL_YZ.1),
                    ..default()
                },
                ProgressBar { station: *st_e },
                ProgressFill,
                InKitchen,
            ));
        }
    }

    // Update positions / scales / colors every frame.
    for (_bar_e, link, mut tf, is_fill, mat_handle) in bars.iter_mut() {
        let Some((anchor, frac, danger)) = active.get(&link.station) else {
            continue; // despawn queued above
        };
        if is_fill.is_some() {
            let w = (*frac * BAR_INNER_W).max(0.03);
            tf.translation = Vec3::new(
                anchor.x - (BAR_INNER_W - w) * 0.5,
                anchor.y,
                anchor.z,
            );
            tf.scale = Vec3::new(w, BAR_FILL_YZ.0, BAR_FILL_YZ.1);
            if let Some(handle) = mat_handle {
                if let Some(mat) = materials.get_mut(&handle.0) {
                    mat.base_color = if *danger {
                        danger_color(t)
                    } else {
                        bar_color(*frac)
                    };
                }
            }
        } else {
            tf.translation = *anchor;
            tf.scale = BAR_BACK_SIZE;
        }
    }
}

// ===========================================================================
// 5. Seated diners ambience
// ===========================================================================

/// Seated capsule height (slightly sunk into the decor chairs).
pub const DINER_SEAT_Y: f32 = 0.60;

/// Gentle idle-sway marker; `phase` decorrelates neighbours.
#[derive(Component)]
pub struct Diner {
    pub phase: f32,
}

/// Diner shirt palette (teal, mustard, plum, sage).
const DINER_SHIRTS: [(f32, f32, f32); 4] = [
    (0.16, 0.55, 0.55),
    (0.85, 0.65, 0.20),
    (0.55, 0.30, 0.55),
    (0.45, 0.65, 0.40),
];

/// Skin tones for diner heads.
const DINER_SKINS: [(f32, f32, f32); 4] = [
    (0.96, 0.80, 0.64),
    (0.78, 0.56, 0.40),
    (0.55, 0.38, 0.27),
    (0.92, 0.72, 0.55),
];

/// Spawn four capsule diners on the decor chairs flanking the two round
/// tables at `(±7.5, 0, -6.5)`. Each diner faces its table (hence roughly
/// the kitchen). Plain helper so `spawn_kitchen`-style code can call it
/// directly; see [`spawn_diners_system`] for the schedule-ready version.
pub fn spawn_diners(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    // (chair pos, table pos) pairs matching the decor in `spawn_kitchen`.
    const SPOTS: [([f32; 3], [f32; 2]); 4] = [
        ([-6.3, 0.0, -6.5], [-7.5, -6.5]),
        ([-8.7, 0.0, -6.5], [-7.5, -6.5]),
        ([6.3, 0.0, -6.5], [7.5, -6.5]),
        ([8.7, 0.0, -6.5], [7.5, -6.5]),
    ];
    let body_mesh = meshes.add(Capsule3d::new(0.32, 0.7));
    let head_mesh = meshes.add(Sphere::new(0.22));

    for (i, (pos, table)) in SPOTS.iter().enumerate() {
        let dx = table[0] - pos[0];
        let dz = table[1] - pos[2];
        // Face the table: forward (-sin yaw, -cos yaw) == normalised (dx, dz).
        let yaw = (-dx).atan2(-dz);
        let (sr, sg, sb) = DINER_SHIRTS[i % DINER_SHIRTS.len()];
        let (kr, kg, kb) = DINER_SKINS[i % DINER_SKINS.len()];
        let body_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(sr, sg, sb),
            perceptual_roughness: 0.85,
            ..default()
        });
        let head_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(kr, kg, kb),
            perceptual_roughness: 0.7,
            ..default()
        });

        commands
            .spawn((
                Mesh3d(body_mesh.clone()),
                MeshMaterial3d(body_mat),
                Transform {
                    translation: Vec3::new(pos[0], DINER_SEAT_Y, pos[2]),
                    rotation: Quat::from_rotation_y(yaw),
                    ..default()
                },
                Diner {
                    phase: i as f32 * 1.7,
                },
                InKitchen,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Mesh3d(head_mesh.clone()),
                    MeshMaterial3d(head_mat),
                    Transform::from_xyz(0.0, 0.72, 0.02),
                    // Tagged too so level teardown never orphans heads.
                    InKitchen,
                ));
            });
    }
}

/// System wrapper for [`spawn_diners`]: idempotent, safe to run on every
/// `OnEnter(Playing)` (resume-from-pause keeps the existing diners).
pub fn spawn_diners_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing: Query<(), With<Diner>>,
) {
    if !existing.is_empty() {
        return;
    }
    spawn_diners(&mut commands, &mut meshes, &mut materials);
}

/// Gentle idle motion: torso sway/roll + nod + tiny seat bob, all keyed off
/// `phase` so the four diners feel alive but unsynchronised. Yaw is
/// preserved (extracted from the current rotation each frame).
pub fn tick_diners(time: Res<Time>, mut q: Query<(&Diner, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (diner, mut tf) in q.iter_mut() {
        let (_, yaw, _) = tf.rotation.to_euler(EulerRot::YXZ);
        let sway = (t * 1.3 + diner.phase).sin() * 0.05;
        let nod = (t * 0.9 + diner.phase * 1.7).sin() * 0.04;
        tf.rotation = Quat::from_euler(EulerRot::YXZ, yaw, nod, sway);
        tf.translation.y = DINER_SEAT_Y + (t * 2.0 + diner.phase).sin() * 0.02;
    }
}

// ===========================================================================
// Unit tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn srgb(c: Color) -> (f32, f32, f32) {
        let s = c.to_srgba();
        (s.red, s.green, s.blue)
    }

    fn approx(a: (f32, f32, f32), b: (f32, f32, f32)) -> bool {
        (a.0 - b.0).abs() < 1e-5 && (a.1 - b.1).abs() < 1e-5 && (a.2 - b.2).abs() < 1e-5
    }

    #[test]
    fn restaurant_lighting_table() {
        // L0 warm bistro.
        let (c0, i0, p0) = restaurant_lighting(0);
        assert_eq!(i0, 9000.0);
        assert_eq!(p0, Vec3::new(6.0, 12.0, 4.0));
        assert!(approx(srgb(c0), L0_SUN), "L0 sun tint {:?}", srgb(c0));

        // L1 cozy dimmer amber.
        let (c1, i1, p1) = restaurant_lighting(1);
        assert_eq!(i1, 7500.0);
        assert_eq!(p1, Vec3::new(4.0, 10.0, 6.0));
        assert!(approx(srgb(c1), L1_SUN), "L1 sun tint {:?}", srgb(c1));
        assert!(i1 < i0, "L1 must be dimmer than L0");

        // L2 bright fresh daylight.
        let (c2, i2, p2) = restaurant_lighting(2);
        assert_eq!(i2, 11000.0);
        assert_eq!(p2, Vec3::new(2.0, 14.0, 3.0));
        assert!(approx(srgb(c2), L2_SUN), "L2 sun tint {:?}", srgb(c2));
        assert!(i2 > i0, "L2 must be brightest");

        // Out-of-range levels reuse the L2 rig.
        let (cn, inch, pn) = restaurant_lighting(99);
        assert_eq!(inch, 11000.0);
        assert_eq!(pn, Vec3::new(2.0, 14.0, 3.0));
        assert!(approx(srgb(cn), L2_SUN));

        // Ambient rigs: L1 dimmest, L2 brightest, warm->amber->neutral.
        let (_, b0) = ambient_rig(0);
        let (_, b1) = ambient_rig(1);
        let (_, b2) = ambient_rig(2);
        assert!(b1 < b0 && b0 < b2, "ambient {b1} < {b0} < {b2}");
    }

    #[test]
    fn progress_fraction_helper() {
        assert_eq!(progress_fraction(0.0, 2.0), 0.0);
        assert_eq!(progress_fraction(1.0, 2.0), 0.5);
        assert_eq!(progress_fraction(2.0, 2.0), 1.0);
        assert_eq!(progress_fraction(5.0, 2.0), 1.0, "clamps above");
        assert_eq!(progress_fraction(-1.0, 2.0), 0.0, "clamps below");
        assert_eq!(progress_fraction(0.0, 0.0), 1.0, "instant task");
        assert_eq!(progress_fraction(3.0, -1.0), 1.0, "degenerate total");
    }

    #[test]
    fn bar_color_ramps_green_to_orange() {
        let (r0, g0, b0) = srgb(bar_color(0.0));
        let (r1, g1, b1) = srgb(bar_color(1.0));
        assert!(g0 > r0, "starts greenish");
        assert!(r1 > g1 && g1 > b1, "ends orange");
        assert_eq!(srgb(bar_color(-2.0)), srgb(bar_color(0.0)));
        assert_eq!(srgb(bar_color(42.0)), srgb(bar_color(1.0)));
    }
}
