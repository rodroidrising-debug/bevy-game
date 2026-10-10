//! Juice & polish: steam, chop pulse, serve confetti (Bevy 0.16).

use bevy::prelude::*;

use crate::world::{InKitchen, Station, StationKind};
use crate::GameState;

#[derive(Event, Debug, Clone)]
pub struct OrderServed {
    pub recipe_name: String,
}

impl OrderServed {
    pub fn new(name: impl Into<String>) -> Self {
        Self { recipe_name: name.into() }
    }
}

#[derive(Component, Debug)]
pub struct SteamParticle {
    pub vel: Vec3,
    pub life: f32,
    pub max_life: f32,
}

#[derive(Resource, Debug, Clone)]
pub struct SteamFx {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

pub fn setup_effects(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Startup runs once; resource persists for the app lifetime.
    let mesh = meshes.add(Sphere::new(0.14));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.45),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.insert_resource(SteamFx { mesh, material });
}

pub fn spawn_steam(commands: &mut Commands, fx: &SteamFx, pos: Vec3, vel: Vec3, life: f32, scale: f32) -> Entity {
    commands.spawn((
        SteamParticle { vel, life, max_life: life.max(0.0001) },
        Mesh3d(fx.mesh.clone()),
        MeshMaterial3d(fx.material.clone()),
        Transform { translation: pos, scale: Vec3::splat(scale), ..default() },
        InKitchen,
    )).id()
}

fn next_rand(state: &mut u32) -> f32 {
    let mut x = *state;
    if x == 0 {
        x = 0x9E37_79B9;
    }
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *state = x;
    ((x >> 8) as f32) / 16_777_216.0
}

pub fn steam_spawner(
    mut commands: Commands,
    time: Res<Time>,
    fx: Option<Res<SteamFx>>,
    stations: Query<(&Transform, &Station)>,
    mut acc: Local<f32>,
    mut seed: Local<u32>,
) {
    let Some(fx) = fx.as_ref() else { return };
    if *seed == 0 {
        *seed = 0x1234_5678;
    }
    *acc += time.delta_secs();
    let mut emitters: Vec<Vec3> = Vec::new();
    for (xf, st) in stations.iter() {
        if st.processing.is_some() {
            emitters.push(xf.translation);
        }
    }
    if emitters.is_empty() || *acc < 0.08 {
        return;
    }
    *acc = 0.0;
    for base in emitters {
        let rx = (next_rand(&mut seed) - 0.5) * 0.5;
        let rz = (next_rand(&mut seed) - 0.5) * 0.5;
        spawn_steam(&mut commands, fx, base + Vec3::new(rx, 1.15, rz), Vec3::new(rx * 0.4, 1.1, rz * 0.4), 0.5, 0.7 + next_rand(&mut seed) * 0.8);
    }
}

pub fn tick_steam(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Transform, &mut SteamParticle)>) {
    let dt = time.delta_secs();
    for (e, mut xf, mut p) in q.iter_mut() {
        p.life -= dt;
        if p.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        xf.translation += p.vel * dt;
        xf.scale *= 1.0 + dt * 1.6;
        xf.translation.x += dt * 0.15 * (p.life * 10.0).sin();
    }
}

/// Simple scale pulse on cutting-board anchors while chopping.
pub fn chop_bounce(time: Res<Time>, stations: Query<(Entity, &Station)>, mut anchors: Query<&mut Transform>) {
    let t = time.elapsed_secs();
    let s = 1.0 + 0.05 * (t * 20.0).sin();
    for (e, st) in stations.iter() {
        if st.kind != StationKind::CuttingBoard {
            continue;
        }
        if let Ok(mut tr) = anchors.get_mut(e) {
            tr.scale = if st.processing.is_some() { Vec3::splat(s) } else { Vec3::ONE };
        }
    }
}

#[derive(Component, Debug)]
pub struct Confetti {
    pub vel: Vec3,
    pub life: f32,
    pub spin: Vec3,
}

const CONFETTI_COLORS: [Color; 8] = [
    Color::srgb(0.95, 0.25, 0.30),
    Color::srgb(1.00, 0.65, 0.10),
    Color::srgb(1.00, 0.90, 0.20),
    Color::srgb(0.25, 0.80, 0.35),
    Color::srgb(0.25, 0.55, 1.00),
    Color::srgb(0.65, 0.35, 0.95),
    Color::srgb(1.00, 1.00, 1.00),
    Color::srgb(0.15, 0.15, 0.18),
];

pub fn serve_confetti_spawner(
    mut commands: Commands,
    mut events: EventReader<OrderServed>,
    stations: Query<(&Transform, &Station)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut seed: Local<u32>,
) {
    if *seed == 0 {
        *seed = 0xC0FF_EE42;
    }
    let n_events = events.read().count();
    if n_events == 0 {
        return;
    }
    let mut origin = Vec3::new(0.0, 1.6, -4.2);
    for (xf, st) in stations.iter() {
        if st.kind == StationKind::OrderWindow {
            origin = xf.translation + Vec3::new(0.0, 1.6, 0.6);
            break;
        }
    }
    let mesh = meshes.add(Cuboid::new(0.12, 0.12, 0.02));
    for _ in 0..n_events {
        for i in 0..12 {
            let a = next_rand(&mut seed) * std::f32::consts::TAU;
            let speed = 1.5 + next_rand(&mut seed) * 3.0;
            let color = CONFETTI_COLORS[(i + (*seed as usize)) % CONFETTI_COLORS.len()];
            commands.spawn((
                Confetti {
                    vel: Vec3::new(a.cos() * speed, 3.0 + next_rand(&mut seed) * 3.5, a.sin() * speed * 0.7 + 1.0),
                    life: 1.2,
                    spin: Vec3::new((next_rand(&mut seed) - 0.5) * 12.0, (next_rand(&mut seed) - 0.5) * 12.0, (next_rand(&mut seed) - 0.5) * 12.0),
                },
                Mesh3d(mesh.clone()),
                MeshMaterial3d(materials.add(StandardMaterial { base_color: color, unlit: true, ..default() })),
                Transform {
                    translation: origin + Vec3::new((next_rand(&mut seed) - 0.5) * 0.6, next_rand(&mut seed) * 0.4, (next_rand(&mut seed) - 0.5) * 0.6),
                    rotation: Quat::from_euler(EulerRot::XYZ, next_rand(&mut seed) * 3.0, next_rand(&mut seed) * 3.0, next_rand(&mut seed) * 3.0),
                    scale: Vec3::ONE,
                },
                InKitchen,
            ));
        }
    }
}

pub fn tick_confetti(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Transform, &mut Confetti)>) {
    let dt = time.delta_secs();
    for (e, mut xf, mut c) in q.iter_mut() {
        c.life -= dt;
        if c.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        c.vel.y -= 9.8 * dt;
        c.vel *= 1.0 - (0.6 * dt).clamp(0.0, 0.9);
        xf.translation += c.vel * dt;
        if xf.translation.y < 0.05 {
            xf.translation.y = 0.05;
            c.vel.y = c.vel.y.abs() * 0.3;
            c.vel.x *= 0.7;
            c.vel.z *= 0.7;
        }
        let (x, y, z) = xf.rotation.to_euler(EulerRot::XYZ);
        xf.rotation = Quat::from_euler(EulerRot::XYZ, x + c.spin.x * dt, y + c.spin.y * dt, z + c.spin.z * dt);
    }
}

pub fn effects_plugin(app: &mut App) {
    app.add_event::<OrderServed>();
    app.add_systems(Startup, setup_effects);
    app.add_systems(Update, (steam_spawner, tick_steam, chop_bounce, serve_confetti_spawner, tick_confetti).run_if(in_state(GameState::Playing)));
}
