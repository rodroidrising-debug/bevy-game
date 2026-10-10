//! Gameplay world: kitchen, player, stations, processing, orders, scoring.

use crate::assets::{play_one, spawn_model, GameAssets};
use crate::config::*;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

// ---------- states are defined in main.rs ----------
use crate::GameState;

// ---------- components ----------

#[derive(Component)]
pub struct Player {
    pub speed: f32,
}

#[derive(Component)]
pub struct FollowCamera;

#[derive(Debug, Clone)]
pub enum Held {
    Single(Item),
    Plate(Vec<Item>),
    Dish(RecipeId),
}

impl Held {
    pub fn label(&self) -> String {
        match self {
            Held::Single(i) => i.display_name().to_string(),
            Held::Plate(v) if v.is_empty() => "Empty Plate".to_string(),
            Held::Plate(v) => {
                let parts: Vec<&str> = v.iter().map(|i| i.display_name()).collect();
                format!("Plate [{}]", parts.join(" + "))
            }
            Held::Dish(r) => format!("{} (ready!)", r.name()),
        }
    }
}

#[derive(Component, Default)]
pub struct Hands {
    pub held: Option<Held>,
}

#[derive(Component)]
pub struct CarriedProp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StationKind {
    Crate(Item),
    CuttingBoard,
    Stove,
    Oven,
    PlateStack,
    BowlStack,
    Sink,
    Counter,
    OrderWindow,
    Trash,
    DishRack,
}

impl StationKind {
    pub fn name(&self) -> String {
        match self {
            StationKind::Crate(i) => format!("Crate: {}", i.display_name()),
            StationKind::CuttingBoard => "Cutting Board".to_string(),
            StationKind::Stove => "Stove (+ stew pot L2+)".to_string(),
            StationKind::Oven => "Oven".to_string(),
            StationKind::PlateStack => "Plate Stack".to_string(),
            StationKind::BowlStack => "Bowl Stack (stew)".to_string(),
            StationKind::Sink => "Sink (wash)".to_string(),
            StationKind::Counter => "Counter (assemble)".to_string(),
            StationKind::OrderWindow => "Order Window (serve)".to_string(),
            StationKind::Trash => "Trash".to_string(),
            StationKind::DishRack => "Dish Rack".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcKind {
    Chop,
    Cook,
    Bake,
    Wash,
}

#[derive(Debug, Clone)]
pub struct Processing {
    pub input: Item,
    pub output: Item,
    pub total: f32,
    pub elapsed: f32,
    pub burn_after: Option<f32>,
    pub kind: ProcKind,
    pub chop_sfx_timer: f32,
}

#[derive(Component)]
pub struct Station {
    pub kind: StationKind,
    pub slot: Option<Held>,
    pub processing: Option<Processing>,
}

#[derive(Component)]
pub struct OrderTicket {
    pub recipe: RecipeId,
    pub spawned_at: f32,
    pub expires_at: f32,
    /// Golden client: 1.5x points, same patience bar, gold customer.
    pub vip: bool,
}

/// Scripted order indices that are VIPs per level (golden clients).
pub fn is_vip_order(level: usize, script_index: usize) -> bool {
    match level {
        1 => script_index == 4 || script_index == 7,
        2 => script_index == 3 || script_index == 7 || script_index == 10,
        _ => false,
    }
}

/// Marker for all run-scoped entities (kitchen, player, tickets, props).
/// Replaces StateScoped(Playing): the Paused state must NOT despawn the run.
#[derive(Component, Default)]
pub struct InKitchen;

#[derive(Resource, Default)]
pub struct NewRunQueued(pub bool);

// ---------- resources ----------

#[derive(Resource, Default)]
pub struct SelectedLevel(pub usize);

#[derive(Resource)]
pub struct LevelClock {
    pub elapsed: f32,
    pub active: bool,
}

impl Default for LevelClock {
    fn default() -> Self {
        Self { elapsed: 0.0, active: false }
    }
}

#[derive(Resource, Default)]
pub struct RunScore {
    pub points: u32,
    pub served: u32,
    pub expired: u32,
    pub trashed: u32,
    /// Grade of the most recent serve (for the online submit).
    pub last_grade: String,
}

#[derive(Resource, Default)]
pub struct NextOrderIndex(pub usize);

#[derive(Resource)]
pub struct Best {
    pub scores: [u32; 3],
    pub stars: [u8; 3],
}

impl Default for Best {
    fn default() -> Self {
        Self { scores: [0; 3], stars: [0; 3] }
    }
}

#[derive(Resource, Default)]
pub struct Hint {
    pub text: String,
    pub timer: f32,
}

impl Hint {
    pub fn set(&mut self, s: impl Into<String>) {
        self.text = s.into();
        self.timer = 3.0;
    }
}

#[derive(Resource, Default)]
pub struct Muted(pub bool);

/// Bundled run-mutable context (keeps system params under Bevy's limit).
#[derive(SystemParam)]
pub struct RunCtx<'w, 's> {
    pub combo: ResMut<'w, crate::combo::Combo>,
    pub economy: ResMut<'w, crate::economy::Economy>,
    pub chop_q: Query<'w, 's, &'static mut crate::minigames::ChopGame>,
    pub stir_q: Query<'w, 's, &'static mut crate::minigames::StirGame>,
    pub served: EventWriter<'w, crate::effects::OrderServed>,
}

// ---------- setup ----------

pub fn setup_lights_camera(mut commands: Commands, cam_q: Query<(), With<FollowCamera>>) {
    if !cam_q.is_empty() {
        return; // already spawned (fixes duplicates on restart)
    }
    commands.spawn((
        DirectionalLight {
            illuminance: 9000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(6.0, 12.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Camera3d::default(),
        FollowCamera,
        Transform::from_xyz(0.0, 11.0, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn station_model_file(kind: &StationKind) -> &'static str {
    match kind {
        StationKind::Crate(Item::Bun) => "crate_buns.gltf",
        StationKind::Crate(Item::BurgerRaw) => "crate_steak.gltf",
        StationKind::Crate(Item::Carrot) => "crate_carrots.gltf",
        StationKind::Crate(Item::Cheese) => "crate_cheese.gltf",
        StationKind::Crate(Item::Ham) => "crate_ham.gltf",
        StationKind::Crate(Item::Lettuce) => "crate_lettuce.gltf",
        StationKind::Crate(Item::Onion) => "crate_onions.gltf",
        StationKind::Crate(Item::Potato) => "crate_potatoes.gltf",
        StationKind::Crate(Item::Tomato) => "crate_tomatoes.gltf",
        StationKind::Crate(Item::Steak) => "crate_steak.gltf",
        StationKind::Crate(Item::NoodleBrick) => "crate_carrots.gltf",
        StationKind::Crate(Item::VegPattyRaw) => "crate_lettuce.gltf",
        StationKind::Crate(_) => "crate.gltf",
        StationKind::CuttingBoard => "cuttingboard.gltf",
        StationKind::Stove => "stove_single.gltf",
        StationKind::Oven => "oven.gltf",
        StationKind::PlateStack => "dishrack_plates.gltf",
        StationKind::BowlStack => "bowl_small.gltf",
        StationKind::Sink => "kitchencounter_sink.gltf",
        StationKind::Counter => "kitchentable_A_large.gltf",
        StationKind::OrderWindow => "wall_orderwindow.gltf",
        StationKind::Trash => "crate.gltf",
        StationKind::DishRack => "dishrack_plates.gltf",
    }
}

fn crates_for_level(level: usize) -> Vec<Item> {
    match level {
        0 => vec![
            Item::Bun,
            Item::BurgerRaw,
            Item::Lettuce,
            Item::Tomato,
            Item::Cheese,
        ],
        1 => vec![
            Item::Carrot,
            Item::Potato,
            Item::Onion,
            Item::Ham,
            Item::NoodleBrick,
            Item::Bun,
            Item::BurgerRaw,
            Item::Lettuce,
            Item::Tomato,
        ],
        _ => vec![
            Item::Bun,
            Item::BurgerRaw,
            Item::VegPattyRaw,
            Item::Lettuce,
            Item::Tomato,
            Item::Cheese,
            Item::Onion,
            Item::Carrot,
            Item::Potato,
            Item::NoodleBrick,
            Item::Ham,
            Item::Steak,
        ],
    }
}

/// Spawn the kitchen for the selected level. All gameplay entities get
/// `InKitchen` so level transitions are clean (Paused must NOT despawn).
/// Guarded: skips entirely on resume-from-pause (kitchen present, no new run).
pub fn spawn_kitchen(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    selected: Res<SelectedLevel>,
    player_q: Query<(), With<Player>>,
    mut new_run: ResMut<NewRunQueued>,
    mut tutorial: ResMut<crate::tutorial::Tutorial>,
    mut combo: ResMut<crate::combo::Combo>,
    mut economy: ResMut<crate::economy::Economy>,
    settings: Res<crate::settings::Settings>,
) {
    if !new_run.0 && !player_q.is_empty() {
        return; // resume from pause: keep existing run, don't reset
    }
    new_run.0 = false;
    *tutorial = crate::tutorial::reset_tutorial();
    *combo = crate::combo::Combo::default();
    economy.reset_run_flags();
    let level = selected.0.min(LEVELS.len() - 1);

    // floor
    let floor = spawn_model(
        &mut commands,
        &assets,
        &mut meshes,
        &mut materials,
        "floor_kitchen.gltf",
        Vec3::new(0.0, -0.55, 0.0),
        4.0,
        Color::srgb(0.35, 0.36, 0.4),
    );
    commands.entity(floor).insert(InKitchen);

    // Order window (north)
    spawn_station(
        &mut commands,
        &assets,
        &mut meshes,
        &mut materials,
        StationKind::OrderWindow,
        Vec3::new(0.0, 0.0, -4.2),
        1.2,
    );

    // Counters row (assembly) — 3 normally, 4th for Veggie Cafe rush.
    let counter_xs: &[f32] = if level >= 2 { &[-3.2, -1.1, 1.1, 3.2] } else { &[-2.5, 0.0, 2.5] };
    for x in counter_xs {
        spawn_station(
            &mut commands,
            &assets,
            &mut meshes,
            &mut materials,
            StationKind::Counter,
            Vec3::new(*x, 0.0, -1.2),
            1.0,
        );
    }

    // Cutting boards (west)
    for (i, z) in [0.6, 2.0].iter().enumerate() {
        let _ = i;
        spawn_station(
            &mut commands,
            &assets,
            &mut meshes,
            &mut materials,
            StationKind::CuttingBoard,
            Vec3::new(-4.0, 0.0, *z),
            1.0,
        );
    }
    if level >= 1 {
        spawn_station(
            &mut commands,
            &assets,
            &mut meshes,
            &mut materials,
            StationKind::CuttingBoard,
            Vec3::new(-4.0, 0.0, -1.0),
            1.0,
        );
    }

    // Stoves (east)
    spawn_station(
        &mut commands,
        &assets,
        &mut meshes,
        &mut materials,
        StationKind::Stove,
        Vec3::new(4.0, 0.0, 0.6),
        1.0,
    );
    spawn_station(
        &mut commands,
        &assets,
        &mut meshes,
        &mut materials,
        StationKind::Stove,
        Vec3::new(4.0, 0.0, 2.0),
        1.0,
    );

    // Oven for levels 1+
    if level >= 1 {
        spawn_station(
            &mut commands,
            &assets,
            &mut meshes,
            &mut materials,
            StationKind::Oven,
            Vec3::new(4.0, 0.0, -1.2),
            1.0,
        );
    }

    // Plate stack + sink + dish rack (west wall)
    spawn_station(
        &mut commands,
        &assets,
        &mut meshes,
        &mut materials,
        StationKind::PlateStack,
        Vec3::new(-2.0, 0.0, -3.4),
        1.0,
    );
    spawn_station(
        &mut commands,
        &assets,
        &mut meshes,
        &mut materials,
        StationKind::Sink,
        Vec3::new(2.0, 0.0, -3.4),
        1.0,
    );
    spawn_station(
        &mut commands,
        &assets,
        &mut meshes,
        &mut materials,
        StationKind::DishRack,
        Vec3::new(3.2, 0.0, -3.4),
        0.9,
    );

    // Trash (south-east)
    spawn_station(
        &mut commands,
        &assets,
        &mut meshes,
        &mut materials,
        StationKind::Trash,
        Vec3::new(4.6, 0.0, 3.4),
        0.9,
    );

    // Crates row (south)
    let crates = crates_for_level(level);
    let n = crates.len() as f32;
    for (i, item) in crates.into_iter().enumerate() {
        let x = -4.5 + (i as f32) * (9.0 / (n - 1.0).max(1.0));
        spawn_station(
            &mut commands,
            &assets,
            &mut meshes,
            &mut materials,
            StationKind::Crate(item),
            Vec3::new(x, 0.0, 4.2),
            0.9,
        );
    }

    // Fridge decor
    let fridge = spawn_model(
        &mut commands,
        &assets,
        &mut meshes,
        &mut materials,
        "fridge_A.gltf",
        Vec3::new(-5.2, 0.0, -3.4),
        1.0,
        Color::srgb(0.7, 0.72, 0.75),
    );
    commands.entity(fridge).insert(InKitchen);

    // Dining corner decor (non-interactive, Step 2 atmosphere)
    for (file, pos, s) in [
        ("table_round_A.gltf", Vec3::new(-7.5, 0.0, -6.5), 1.0),
        ("chair_A.gltf", Vec3::new(-6.3, 0.0, -6.5), 1.0),
        ("chair_A.gltf", Vec3::new(-8.7, 0.0, -6.5), 1.0),
        ("table_round_A.gltf", Vec3::new(7.5, 0.0, -6.5), 1.0),
        ("wall.gltf", Vec3::new(-4.0, 0.0, -5.6), 1.5),
        ("wall.gltf", Vec3::new(4.0, 0.0, -5.6), 1.5),
        ("menu.gltf", Vec3::new(0.0, 2.2, -4.0), 1.0),
    ] {
        let e = spawn_model(&mut commands, &assets, &mut meshes, &mut materials, file, pos, s, Color::WHITE);
        commands.entity(e).insert(InKitchen);
    }

    // Bowl stack for stew ladling (levels 1+)
    if level >= 1 {
        spawn_station(
            &mut commands,
            &assets,
            &mut meshes,
            &mut materials,
            StationKind::BowlStack,
            Vec3::new(-3.2, 0.0, -3.4),
            1.0,
        );
        // Stew pots sit on the two stoves.
        crate::stew::spawn_stew_pots(
            &mut commands,
            &assets,
            &mut meshes,
            &mut materials,
            level,
            &[Vec3::new(4.0, 0.0, 0.6), Vec3::new(4.0, 0.0, 2.0)],
        );
    }

    // Player: capsule body. Speed honors Fast Shoes x chef trait.
    let speed = economy.upgrades.player_speed() * settings.chef.speed_mult();
    let chef_color = settings.chef.color();
    commands.spawn((
        Player { speed },
        Hands::default(),
        InKitchen,
        Mesh3d(meshes.add(Capsule3d::new(0.35, 0.9))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: chef_color,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.8, 2.5),
    ));

    // Reset run resources
    commands.insert_resource(LevelClock { elapsed: 0.0, active: true });
    commands.insert_resource(RunScore::default());
    commands.insert_resource(NextOrderIndex(0));
    commands.insert_resource(Hint::default());
}

fn spawn_station(
    commands: &mut Commands,
    assets: &GameAssets,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    kind: StationKind,
    pos: Vec3,
    scale: f32,
) {
    let file = station_model_file(&kind);
    let model = if let Some(h) = assets.scenes.get(file) {
        commands
            .spawn((
                SceneRoot(h.clone()),
                Transform {
                    translation: pos,
                    scale: Vec3::splat(scale),
                    ..default()
                },
                InKitchen,
            ))
            .id()
    } else {
        commands
            .spawn((
                Mesh3d(meshes.add(Cuboid::new(1.2, 0.9, 1.2))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.5, 0.5, 0.55),
                    ..default()
                })),
                Transform::from_translation(pos),
                InKitchen,
            ))
            .id()
    };
    let _ = model;

    // Invisible interaction anchor (slightly above floor).
    commands.spawn((
        Station { kind, slot: None, processing: None },
        Transform::from_translation(pos),
        Visibility::default(),
        InKitchen,
    ));
}

/// Despawn the whole run (kitchen, player, tickets, props, customers, fx).
/// Runs when entering menu-ish states. Pause must NOT trigger this.
pub fn despawn_kitchen(mut commands: Commands, q: Query<Entity, With<InKitchen>>) {
    for e in q.iter() {
        commands.entity(e).despawn();
    }
}

// ---------- per-frame systems (Playing only) ----------

pub fn player_movement(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<crate::settings::Settings>,
    mut player: Query<(&Player, &mut Transform), Without<FollowCamera>>,
    stations: Query<&Transform, (With<Station>, Without<Player>, Without<FollowCamera>)>,
) {
    let Ok((p, mut xf)) = player.single_mut() else {
        return;
    };
    let b = &settings.bindings;
    let mut dir = Vec3::ZERO;
    if keys.pressed(b.up) || keys.pressed(KeyCode::ArrowUp) {
        dir.z -= 1.0;
    }
    if keys.pressed(b.down) || keys.pressed(KeyCode::ArrowDown) {
        dir.z += 1.0;
    }
    if keys.pressed(b.left) || keys.pressed(KeyCode::ArrowLeft) {
        dir.x -= 1.0;
    }
    if keys.pressed(b.right) || keys.pressed(KeyCode::ArrowRight) {
        dir.x += 1.0;
    }
    if dir.length_squared() > 0.0 {
        let dir = dir.normalize();
        let mut next = xf.translation + dir * p.speed * time.delta_secs();
        next.x = next.x.clamp(-5.4, 5.4);
        next.z = next.z.clamp(-4.6, 5.0);
        next.y = 0.8;
        // push out of station footprints
        for s in stations.iter() {
            let d = Vec3::new(next.x - s.translation.x, 0.0, next.z - s.translation.z);
            let dist = d.length();
            if dist < 0.95 && dist > 0.0001 {
                let push = d.normalize() * (0.95 - dist);
                next.x += push.x;
                next.z += push.z;
            }
        }
        xf.translation = next;
    }
}

pub fn camera_follow(
    time: Res<Time>,
    player: Query<&Transform, (With<Player>, Without<FollowCamera>)>,
    mut cam: Query<&mut Transform, (With<FollowCamera>, Without<Player>)>,
) {
    let Ok(p) = player.single() else { return };
    let Ok(mut c) = cam.single_mut() else { return };
    let target = p.translation + Vec3::new(0.0, 10.2, 7.2);
    c.translation = c.translation.lerp(target, (6.0 * time.delta_secs()).clamp(0.0, 1.0));
    c.look_at(p.translation + Vec3::new(0.0, 0.8, 0.0), Vec3::Y);
}

fn nearest_station(
    player_pos: Vec3,
    stations: &Query<(Entity, &Transform, &Station)>,
) -> Option<Entity> {
    let mut best: Option<(Entity, f32)> = None;
    for (e, t, _) in stations.iter() {
        let d = player_pos.distance(t.translation);
        if d <= 2.4 && best.map_or(true, |(_, bd)| d < bd) {
            best = Some((e, d));
        }
    }
    best.map(|(e, _)| e)
}

pub fn interact_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut player_q: Query<(Entity, &Transform, &mut Hands), With<Player>>,
    mut stations: Query<(Entity, &Transform, &mut Station)>,
    mut pots: Query<(Entity, &Transform, &mut crate::stew::PotComponent)>,
    mut hint: ResMut<Hint>,
    mut score: ResMut<RunScore>,
    selected: Res<SelectedLevel>,
    tickets: Query<(Entity, &OrderTicket)>,
    clock: Res<LevelClock>,
    muted: Res<Muted>,
    settings: Res<crate::settings::Settings>,
    mut ctx: RunCtx,
) {
    let Ok((player_entity, p_xf, mut hands)) = player_q.single_mut() else {
        return;
    };
    let _ = player_entity;
    let _ = selected;
    let player_pos = p_xf.translation;
    let b = &settings.bindings;
    let sfx = settings.sfx;

    let do_interact = keys.just_pressed(b.interact) || keys.just_pressed(KeyCode::Space);
    let do_trash = keys.just_pressed(b.trash) || keys.just_pressed(KeyCode::KeyX);

    if do_trash {
        // Stew pots claim Q first (dump/wash flow); fall back to hands.
        if crate::stew::handle_pot_trash_key(&mut commands, &assets, muted.0, sfx, &mut pots, player_pos, &mut score, &mut hint) {
            return;
        }
        if let Some(held) = hands.held.take() {
            score.trashed += 1;
            if matches!(held, Held::Single(i) if i.is_food()) || matches!(held, Held::Dish(_)) {
                score.points = score.points.saturating_sub(10);
            }
            hint.set("Trashed. (-10 if food)");
            if !muted.0 {
                play_one(&mut commands, assets.snd_plate.clone(), 0.5 * sfx);
            }
        }
        return;
    }

    // R: burn rescue (flip the pan once per fry).
    if keys.just_pressed(b.rescue) {
        let mut best: Option<Entity> = None;
        let mut best_d = f32::MAX;
        for (e, t, _) in stations.iter() {
            let d = player_pos.distance(t.translation);
            if d <= 2.4 && d < best_d {
                best_d = d;
                best = Some(e);
            }
        }
        if let Some(re) = best {
            if let (Ok(mut game), Ok((_, _, mut rst))) =
                (ctx.stir_q.get_mut(re), stations.get_mut(re))
            {
                crate::minigames::attempt_rescue(&mut game, &mut rst, &mut hint);
            } else {
                hint.set("No frying pan nearby to rescue (R).");
            }
        }
        return;
    }

    if !do_interact {
        return;
    }

    // Stew pots (level 1+) claim E first — but NOT when holding fryable
    // food (patty/ham/steak go to the generic Stove below). Pots sit on the
    // same tiles as stoves, so route by intent.
    let holding_fryable = matches!(&hands.held, Some(Held::Single(i)) if crate::config::cook_result(*i).is_some());
    if !holding_fryable {
        if crate::stew::handle_stove_interact(&mut commands, &assets, muted.0, sfx, &mut hands, &mut pots, player_pos, &mut hint, ctx.economy.upgrades.boil_total()) {
            return;
        }
    }

    // find nearest
    let mut best: Option<Entity> = None;
    let mut best_d = f32::MAX;
    for (e, t, _) in stations.iter() {
        let d = p_xf.translation.distance(t.translation);
        if d <= 2.4 && d < best_d {
            best_d = d;
            best = Some(e);
        }
    }
    let Some(se) = best else {
        hint.set("No station in range");
        return;
    };
    let Ok((_, _, mut st)) = stations.get_mut(se) else {
        return;
    };

    let vol = if muted.0 { 0.0 } else { 0.9 * sfx };
    let silent = muted.0;

    match st.kind {
        StationKind::Crate(item) => {
            if hands.held.is_some() {
                hint.set("Hands full! Press Q to trash first.");
            } else if item == Item::PlateClean || item == Item::PlateDirty {
                hands.held = Some(Held::Single(item));
                if !silent {
                    play_one(&mut commands, assets.snd_plate.clone(), vol);
                }
            } else {
                hands.held = Some(Held::Single(item));
                hint.set(format!("Grabbed {}", item.display_name()));
                if !silent {
                    play_one(&mut commands, assets.snd_plate.clone(), vol * 0.7);
                }
            }
        }
        StationKind::PlateStack => {
            if hands.held.is_some() {
                hint.set("Hands full!");
            } else {
                hands.held = Some(Held::Plate(vec![]));
                hint.set("Took a clean plate. Add ingredients at a Counter.");
                if !silent {
                    play_one(&mut commands, assets.snd_plate.clone(), vol);
                }
            }
        }
        StationKind::BowlStack => {
            if hands.held.is_some() {
                hint.set("Hands full!");
            } else {
                hands.held = Some(Held::Single(Item::BowlClean));
                hint.set("Took a clean bowl. Ladle stew at a done pot!");
                if !silent {
                    play_one(&mut commands, assets.snd_plate.clone(), vol);
                }
            }
        }
        StationKind::CuttingBoard => {
            // Rhythm minigame owns E while chopping.
            if st.processing.is_some() {
                // Take finished result if ready (minigame completed it).
                let done = st.processing.as_ref().map_or(false, |p| p.elapsed >= p.total);
                if done {
                    if let Some(slot) = st.slot.take() {
                        hands.held = Some(slot);
                        st.processing = None;
                        if !silent {
                            play_one(&mut commands, assets.snd_plate.clone(), vol * 0.7);
                        }
                    } else {
                        hint.set("Chopping... wait for the ding!");
                    }
                } else if let Ok(mut game) = ctx.chop_q.get_mut(se) {
                    crate::minigames::attempt_chop(&mut commands, &assets, silent, &mut game, se, &mut st, &mut hint, &mut score, sfx);
                } else {
                    hint.set("Chopping... wait for the ding!");
                }
            } else if let Some(held) = hands.held.take() {
                match held {
                    Held::Single(item) => {
                        if let Some((out, t)) = chop_result(item) {
                            st.processing = Some(Processing {
                                input: item,
                                output: out,
                                total: t,
                                elapsed: 0.0,
                                burn_after: None,
                                kind: ProcKind::Chop,
                                chop_sfx_timer: 0.0,
                            });
                            commands.entity(se).insert(crate::minigames::ChopGame::new(ctx.economy.upgrades.chop_needed()));
                            hint.set(format!("Chopping {}... hit E when O is centered!", item.display_name()));
                            if !silent {
                                play_one(&mut commands, assets.snd_chop.clone(), vol);
                            }
                        } else {
                            hands.held = Some(Held::Single(item));
                            hint.set(format!("{} can't be chopped here.", item.display_name()));
                        }
                    }
                    other => {
                        hands.held = Some(other);
                        hint.set("Only single ingredients go on the board.");
                    }
                }
            } else if let Some(slot) = st.slot.take() {
                hands.held = Some(slot);
                if !silent {
                    play_one(&mut commands, assets.snd_plate.clone(), vol * 0.7);
                }
            } else {
                hint.set("Bring something choppable (tomato, lettuce, onion, carrot, cheese, potato, steak).");
            }
        }
        StationKind::Stove => {
            if st.processing.is_some() {
                let done = st.processing.as_ref().map_or(false, |p| p.elapsed >= p.total);
                if done {
                    // Finished (or burning-watch): E takes the result.
                    if let Some(slot) = st.slot.take() {
                        hands.held = Some(slot);
                        st.processing = None;
                        commands.entity(se).remove::<crate::minigames::StirGame>();
                        if !silent {
                            play_one(&mut commands, assets.snd_plate.clone(), vol * 0.7);
                        }
                    } else {
                        hint.set("Done! ...but the pan is empty? Take it before it burns!");
                    }
                } else if ctx.stir_q.get_mut(se).is_ok() {
                    if let Ok(mut game) = ctx.stir_q.get_mut(se) {
                        let cool = crate::minigames::STIR_COOL + ctx.economy.upgrades.stir_cool_bonus();
                        crate::minigames::attempt_stir(&mut commands, &assets, silent, &mut game, cool, &mut hint, sfx);
                    }
                } else {
                    hint.set("Cooking... E to stir, don't let HEAT fill!");
                }
            } else if let Some(held) = hands.held.take() {
                match held {
                    Held::Single(item) => {
                        if let Some((out, t, burn)) = cook_result(item) {
                            let burn = burn + ctx.economy.upgrades.burn_bonus();
                            st.processing = Some(Processing {
                                input: item,
                                output: out,
                                total: t,
                                elapsed: 0.0,
                                burn_after: Some(burn),
                                kind: ProcKind::Cook,
                                chop_sfx_timer: 0.0,
                            });
                            commands.entity(se).insert(crate::minigames::StirGame::new());
                            hint.set(format!("Frying {} ({}s)... E = stir, keep HEAT down!", item.display_name(), t as i32));
                            if !silent {
                                play_one(&mut commands, assets.snd_sizzle.clone(), vol);
                            }
                        } else {
                            hands.held = Some(Held::Single(item));
                            hint.set(format!("{} can't be fried. Try patty / ham / steak / noodles.", item.display_name()));
                        }
                    }
                    other => {
                        hands.held = Some(other);
                        hint.set("Only single ingredients go in the pan.");
                    }
                }
            } else if let Some(slot) = st.slot.take() {
                hands.held = Some(slot);
                if !silent {
                    play_one(&mut commands, assets.snd_plate.clone(), vol * 0.7);
                }
            } else {
                hint.set("Bring something fryable (raw patty, ham, steak pieces).");
            }
        }
        StationKind::Oven => {
            if st.processing.is_some() {
                let done = st.processing.as_ref().map_or(false, |p| p.elapsed >= p.total);
                if done {
                    if let Some(slot) = st.slot.take() {
                        hands.held = Some(slot);
                        st.processing = None;
                        commands.entity(se).remove::<crate::minigames::StirGame>();
                        if !silent {
                            play_one(&mut commands, assets.snd_plate.clone(), vol * 0.7);
                        }
                    } else {
                        hint.set("Baked! Take it before it chars!");
                    }
                } else if let Ok(mut game) = ctx.stir_q.get_mut(se) {
                    game.heat = (game.heat - (crate::minigames::STIR_COOL + ctx.economy.upgrades.stir_cool_bonus())).max(0.0);
                    hint.set(format!("Vented! {}", crate::minigames::render_heat_bar(game.heat)));
                    if !silent {
                        play_one(&mut commands, assets.snd_boil.clone(), vol * 0.4);
                    }
                } else {
                    hint.set("Baking... E to vent, don't let HEAT fill!");
                }
            } else if let Some(held) = hands.held.take() {
                match held {
                    Held::Single(Item::PotatoChopped) => {
                        st.processing = Some(Processing {
                            input: Item::PotatoChopped,
                            output: Item::PotatoMashed,
                            total: 5.0,
                            elapsed: 0.0,
                            burn_after: Some(6.0 + ctx.economy.upgrades.burn_bonus()),
                            kind: ProcKind::Bake,
                            chop_sfx_timer: 0.0,
                        });
                        commands.entity(se).insert(crate::minigames::StirGame::new());
                        hint.set("Baking potatoes (5s)... E = vent, HEAT can char them!");
                        if !silent {
                            play_one(&mut commands, assets.snd_boil.clone(), vol * 0.6);
                        }
                    }
                    Held::Single(item) => {
                        hands.held = Some(Held::Single(item));
                        hint.set("Oven only bakes Chopped Potato -> Mashed.");
                    }
                    other => {
                        hands.held = Some(other);
                        hint.set("Oven only takes chopped potato.");
                    }
                }
            } else if let Some(slot) = st.slot.take() {
                hands.held = Some(slot);
                if !silent {
                    play_one(&mut commands, assets.snd_plate.clone(), vol * 0.7);
                }
            } else {
                hint.set("Bring Chopped Potato to bake.");
            }
        }
        StationKind::Sink => {
            if st.processing.is_some() {
                hint.set("Washing... wait!");
            } else if let Some(held) = hands.held.take() {
                match held {
                    Held::Single(Item::PlateDirty) => {
                        st.processing = Some(Processing {
                            input: Item::PlateDirty,
                            output: Item::PlateClean,
                            total: 2.0,
                            elapsed: 0.0,
                            burn_after: None,
                            kind: ProcKind::Wash,
                            chop_sfx_timer: 0.0,
                        });
                        hint.set("Washing plate...");
                        if !silent {
                            play_one(&mut commands, assets.snd_boil.clone(), vol * 0.5);
                        }
                    }
                    Held::Single(item) => {
                        hands.held = Some(Held::Single(item));
                        hint.set(format!("Only dirty plates go in the sink (not {}).", item.display_name()));
                    }
                    other => {
                        hands.held = Some(other);
                        hint.set("Sink only washes dirty plates.");
                    }
                }
            } else if let Some(slot) = st.slot.take() {
                hands.held = Some(slot);
                if !silent {
                    play_one(&mut commands, assets.snd_plate.clone(), vol * 0.7);
                }
            } else {
                hint.set("Bring a Dirty Plate (from the Dish Rack).");
            }
        }
        StationKind::Counter => {
            // Assembly logic.
            if let Some(held) = hands.held.take() {
                match held {
                    Held::Single(item) => {
                        match st.slot.take() {
                            None => {
                                st.slot = Some(Held::Single(item));
                                if !silent {
                                    play_one(&mut commands, assets.snd_plate.clone(), vol * 0.6);
                                }
                            }
                            Some(Held::Plate(mut v)) => {
                                if item == Item::PlateClean || item == Item::PlateDirty || item == Item::BowlClean {
                                    hint.set("That doesn't go on a plate — bowls ladle at the pot!");
                                    st.slot = Some(Held::Plate(v));
                                    hands.held = Some(Held::Single(item));
                                } else if item.is_burned() {
                                    hint.set("Burned! Trash it (Q) - it can't go on a plate.");
                                    st.slot = Some(Held::Plate(v));
                                    hands.held = Some(Held::Single(item));
                                } else {
                                    v.push(item);
                                    if let Some(recipe) = match_recipe(&v) {
                                        st.slot = Some(Held::Dish(recipe));
                                        hint.set(format!("{} complete! Serve at the Order Window!", recipe.name()));
                                        if !silent {
                                            play_one(&mut commands, assets.snd_ding.clone(), vol);
                                        }
                                    } else {
                                        // Check if still a subset of some recipe
                                        if is_subset_of_any_recipe(&v) {
                                            st.slot = Some(Held::Plate(v));
                                            hint.set("Added! Keep building...");
                                        } else {
                                            // invalid combo: keep but warn
                                            st.slot = Some(Held::Plate(v));
                                            hint.set("Hmm, that combo matches no recipe. Q near trash to dump, or keep trying.");
                                        }
                                        if !silent {
                                            play_one(&mut commands, assets.snd_plate.clone(), vol * 0.6);
                                        }
                                    }
                                }
                            }
                            Some(Held::Single(other)) => {
                                // try to make a plate if one of them is a plate
                                if other == Item::PlateClean && item.is_food() && !item.is_burned() {
                                    let v = vec![item];
                                    st.slot = Some(Held::Plate(v));
                                    hint.set("Started plating! Add more at this counter.");
                                    if !silent {
                                        play_one(&mut commands, assets.snd_plate.clone(), vol * 0.6);
                                    }
                                } else if item == Item::PlateClean && other.is_food() {
                                    let v = vec![other];
                                    st.slot = Some(Held::Plate(v));
                                    hint.set("Started plating! Add more at this counter.");
                                    if !silent {
                                        play_one(&mut commands, assets.snd_plate.clone(), vol * 0.6);
                                    }
                                } else {
                                    hint.set("Counter occupied. Take it first (E with empty hands).");
                                    st.slot = Some(Held::Single(other));
                                    hands.held = Some(Held::Single(item));
                                }
                            }
                            Some(Held::Dish(r)) => {
                                st.slot = Some(Held::Dish(r));
                                hands.held = Some(Held::Single(item));
                                hint.set("Finished dish here - take it to serve!");
                            }
                        }
                    }
                    Held::Plate(v) => {
                        match st.slot.take() {
                            None => {
                                st.slot = Some(Held::Plate(v));
                                if !silent {
                                    play_one(&mut commands, assets.snd_plate.clone(), vol * 0.6);
                                }
                            }
                            Some(Held::Single(item)) => {
                                if item.is_food() && !item.is_burned() {
                                    let mut nv = v;
                                    nv.push(item);
                                    if let Some(recipe) = match_recipe(&nv) {
                                        st.slot = Some(Held::Dish(recipe));
                                        hint.set(format!("{} complete! Serve it!", recipe.name()));
                                        if !silent {
                                            play_one(&mut commands, assets.snd_ding.clone(), vol);
                                        }
                                    } else if is_subset_of_any_recipe(&nv) {
                                        st.slot = Some(Held::Plate(nv));
                                        hint.set("Added! Keep building...");
                                        if !silent {
                                            play_one(&mut commands, assets.snd_plate.clone(), vol * 0.6);
                                        }
                                    } else {
                                        st.slot = Some(Held::Plate(nv));
                                        hint.set("No recipe matches this combo...");
                                        if !silent {
                                            play_one(&mut commands, assets.snd_plate.clone(), vol * 0.6);
                                        }
                                    }
                                } else {
                                    st.slot = Some(Held::Single(item));
                                    hands.held = Some(Held::Plate(v));
                                    hint.set("Can't add that to a plate.");
                                }
                            }
                            Some(other) => {
                                st.slot = Some(other);
                                hands.held = Some(Held::Plate(v));
                                hint.set("Counter occupied.");
                            }
                        }
                    }
                    Held::Dish(r) => {
                        if st.slot.is_none() {
                            st.slot = Some(Held::Dish(r));
                            if !silent {
                                play_one(&mut commands, assets.snd_plate.clone(), vol * 0.6);
                            }
                        } else {
                            hands.held = Some(Held::Dish(r));
                            hint.set("Counter occupied.");
                        }
                    }
                }
            } else if let Some(slot) = st.slot.take() {
                hands.held = Some(slot);
                if !silent {
                    play_one(&mut commands, assets.snd_plate.clone(), vol * 0.6);
                }
            } else {
                hint.set("Empty counter. Bring a plate + ingredients.");
            }
        }
        StationKind::OrderWindow => {
            if let Some(held) = hands.held.take() {
                // Ladled stew arrives as a complete Plate — promote it to a Dish.
                let held = match held {
                    Held::Plate(v) => match match_recipe(&v) {
                        Some(r) => Held::Dish(r),
                        None => {
                            hands.held = Some(Held::Plate(v));
                            hint.set("That plate matches no recipe — keep building at a Counter.");
                            return;
                        }
                    },
                    other => other,
                };
                match held {
                    Held::Dish(recipe) => {
                        // oldest matching ticket first (fair + fun)
                        let mut best_ticket: Option<(Entity, f32, bool)> = None;
                        for (e, t) in tickets.iter() {
                            if t.recipe == recipe {
                                let age = clock.elapsed - t.spawned_at;
                                if best_ticket.map_or(true, |(_, ba, _)| age > ba) {
                                    best_ticket = Some((e, age, t.vip));
                                }
                            }
                        }
                        if let Some((te, age, vip)) = best_ticket {
                            let cfg = &LEVELS[selected.0.min(LEVELS.len() - 1)];
                            let base = score_serve(recipe, age, cfg.order_expire_s);
                            let (graded, grade) = crate::quality::graded_points(base, age, cfg.order_expire_s);
                            let meal_mult = ctx.economy.upgrades.meal_mult(recipe.index());
                            let mut total = (graded as f32 * meal_mult).round() as u32;
                            if vip {
                                total = (total as f32 * 1.5).round() as u32;
                            }
                            let bonus = crate::combo::note_serve(&mut ctx.combo);
                            score.points += total + bonus;
                            score.served += 1;
                            score.last_grade = match grade {
                                crate::quality::Grade::S => "S",
                                crate::quality::Grade::A => "A",
                                _ => "B",
                            }
                            .to_string();
                            commands.entity(te).despawn();
                            // dirty plate spawns on dish rack
                            spawn_dirty_on_rack(&mut stations, &mut hint);
                            let (mut coins, gem) = ctx.economy.pay_serve(total + bonus, grade);
                            coins += settings.chef.tip_bonus();
                            ctx.economy.coins = ctx.economy.coins.saturating_add(settings.chef.tip_bonus());
                            hint.set(format!(
                                "Served {}{}! {} +{}{} +{} coins{}",
                                if vip { "VIP " } else { "" },
                                recipe.name(),
                                crate::quality::grade_label(grade),
                                total,
                                if bonus > 0 { format!(" +{} combo", bonus) } else { String::new() },
                                coins,
                                if gem { " +1 GEM!" } else { "" }
                            ));
                            ctx.served.write(crate::effects::OrderServed::new(recipe.name()));
                            if !silent {
                                play_one(&mut commands, assets.snd_plate.clone(), vol);
                                play_one(&mut commands, assets.snd_ding.clone(), vol * 0.7);
                            }
                        } else {
                            hands.held = Some(Held::Dish(recipe));
                            hint.set(format!("No ticket wants {} right now!", recipe.name()));
                        }
                    }
                    other => {
                        hands.held = Some(other);
                        hint.set("Only finished dishes go to the window. Assemble on a Counter.");
                    }
                }
            } else {
                hint.set("Bring a finished dish here to serve!");
            }
        }
        StationKind::Trash => {
            if let Some(held) = hands.held.take() {
                score.trashed += 1;
                if matches!(held, Held::Single(i) if i.is_food()) || matches!(held, Held::Dish(_)) {
                    score.points = score.points.saturating_sub(10);
                }
                hint.set("Trashed.");
                if !silent {
                    play_one(&mut commands, assets.snd_plate.clone(), vol * 0.5);
                }
            } else if let Some(slot) = st.slot.take() {
                let _ = slot;
                hint.set("Trashed counter item. (Counter cleared)");
                score.trashed += 1;
            } else {
                hint.set("Nothing to trash. (Hold Q anywhere to dump hands.)");
            }
        }
        StationKind::DishRack => {
            if hands.held.is_some() {
                hint.set("Hands full!");
            } else if let Some(slot) = st.slot.take() {
                hands.held = Some(slot);
                hint.set("Took dirty plate - wash at Sink!");
                if !silent {
                    play_one(&mut commands, assets.snd_plate.clone(), vol * 0.6);
                }
            } else {
                hint.set("No dirty plates. (They appear here after serving.)");
            }
        }
    }
}

fn spawn_dirty_on_rack(
    stations: &mut Query<(Entity, &Transform, &mut Station)>,
    _hint: &mut Hint,
) {
    for (_, _, mut st) in stations.iter_mut() {
        if st.kind == StationKind::DishRack && st.slot.is_none() {
            st.slot = Some(Held::Single(Item::PlateDirty));
            return;
        }
    }
    // rack full: drop silently (still counts as generated)
}

fn all_recipe_sets() -> Vec<&'static [Item]> {
    vec![
        RecipeId::ClassicBurger.components(),
        RecipeId::Cheeseburger.components(),
        RecipeId::DeluxeBurger.components(),
        RecipeId::HeartyStew.components(),
        RecipeId::SundayDinner.components(),
        RecipeId::VeggieBurger.components(),
        RecipeId::VeggieDeluxe.components(),
        RecipeId::NoodleBowl.components(),
    ]
}

fn is_subset_of_any_recipe(v: &[Item]) -> bool {
    for set in all_recipe_sets() {
        let mut remaining: Vec<Item> = set.to_vec();
        let mut ok = true;
        for x in v {
            if let Some(pos) = remaining.iter().position(|y| y == x) {
                remaining.remove(pos);
            } else {
                ok = false;
                break;
            }
        }
        if ok {
            return true;
        }
    }
    false
}

/// Tick chopping / cooking / baking / washing + burn.
/// Chop stations with an active ChopGame are frozen here: hits drive completion.
pub fn processing_tick(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    muted: Res<Muted>,
    mut stations: Query<(Entity, &mut Station)>,
    chop_q: Query<&crate::minigames::ChopGame>,
    settings: Res<crate::settings::Settings>,
    mut hint: ResMut<Hint>,
) {
    let psfx = settings.sfx;
    for (e, mut st) in stations.iter_mut() {
        let has_chop_game = chop_q.get(e).is_ok();
        // take processing to avoid borrow issues
        if let Some(mut pr) = st.processing.take() {
            // Rhythm chopping: timer frozen until hits complete it.
            if !(pr.kind == ProcKind::Chop && has_chop_game) {
                pr.elapsed += time.delta_secs();
            }
            // chop sfx ticks (passive only; minigame plays per-hit instead)
            if pr.kind == ProcKind::Chop && !has_chop_game {
                pr.chop_sfx_timer -= time.delta_secs();
                if pr.chop_sfx_timer <= 0.0 && pr.elapsed < pr.total {
                    pr.chop_sfx_timer = 0.6;
                    if !muted.0 {
                        play_one(&mut commands, assets.snd_chop.clone(), 0.9 * psfx);
                    }
                }
            }
            let finished = pr.elapsed >= pr.total;
            let burned = match pr.burn_after {
                Some(b) => pr.elapsed >= pr.total + b,
                None => false,
            };
            if burned {
                let b = burned_version(pr.output);
                st.slot = Some(Held::Single(b));
                hint.set(format!("BURNED! Trash it (Q)."));
                if !muted.0 {
                    play_one(&mut commands, assets.snd_whistle.clone(), 0.9 * psfx);
                }
                // processing consumed (burned result sits in slot)
            } else if finished && pr.burn_after.is_some() {
                // cooked but can still burn if left: keep processing running as "burn countdown"
                // move result to slot but keep a burn watcher? Simpler: keep in processing until burn or pickup.
                // Put result in slot AND keep watcher.
                st.slot = Some(Held::Single(pr.output));
                // keep watcher with remaining burn time
                pr.elapsed = pr.total; // clamp
                st.processing = Some(pr);
                // ding once when first finished: detect by slot just filled
                // (play ding every frame would spam; only ding if slot was empty before —
                // we just filled it, and watcher remains, so gate with a flag via elapsed exactness)
                // Simplest: ding only on the exact frame it finishes.
                // We track: if elapsed just crossed total this frame, ding.
                // Since we clamped, use a small epsilon: ding if elapsed <= total + dt.
                // To avoid spam, only ding when slot was None before... it was (processing active).
                // But watcher persists, so next frames would re-ding. Use: ding only if pr.elapsed == pr.total
                // AND slot newly set — we need one-shot. Workaround: set elapsed slightly above total
                // after ding so subsequent frames skip.
                // Actually simplest: ding now, then bump elapsed by epsilon so we know we dinged.
                if !muted.0 {
                    // ding only first time: check burn watcher not yet dinged via chop_sfx_timer marker
                    // reuse chop_sfx_timer = -1.0 as "dinged" flag for cook
                }
                // Mark dinged:
                if let Some(w) = st.processing.as_mut() {
                    if w.chop_sfx_timer != -1.0 {
                        if !muted.0 {
                            play_one(&mut commands, assets.snd_ding.clone(), 0.9 * psfx);
                        }
                        w.chop_sfx_timer = -1.0;
                        w.elapsed = w.total + 0.001;
                    }
                }
            } else if finished {
                st.slot = Some(Held::Single(pr.output));
                let msg = match pr.kind {
                    ProcKind::Chop => format!("Chopped -> {}!", pr.output.display_name()),
                    ProcKind::Cook => format!("Cooked -> {}!", pr.output.display_name()),
                    ProcKind::Bake => format!("Baked -> {}!", pr.output.display_name()),
                    ProcKind::Wash => "Plate clean!".to_string(),
                };
                hint.set(msg);
                if !muted.0 {
                    play_one(&mut commands, assets.snd_ding.clone(), 0.9 * psfx);
                    play_one(&mut commands, assets.snd_plate.clone(), 0.6 * psfx);
                }
            } else {
                st.processing = Some(pr);
            }
        } else {
            // If a cooked item sits in slot with an active burn watcher... handled above.
            // If slot has cooked item but watcher exists, watcher ticks (above).
            // Nothing to do.
        }
    }
    // Handle burn watcher pickup clearing: no-op scan (kept for clarity).
    for (_e, mut st) in stations.iter_mut() {
        if st.slot.is_none() {
            continue;
        }
        let _ = &mut st;
    }
    // Clear stale watchers whose slot was picked up (slot None + watcher in burn phase)
    for (_e, mut st) in stations.iter_mut() {
        if st.slot.is_none() && st.processing.is_some() {
            let in_burn_phase = st.processing.as_ref().map_or(false, |p| p.elapsed >= p.total);
            if in_burn_phase {
                st.processing = None;
            }
        }
    }
}

pub fn hint_tick(time: Res<Time>, mut hint: ResMut<Hint>) {
    if hint.timer > 0.0 {
        hint.timer -= time.delta_secs();
        if hint.timer <= 0.0 {
            hint.text.clear();
        }
    }
}

/// Show carried item mesh floating above player + station progress rings (simple).
pub fn carried_visual_sync(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    player_q: Query<(&Transform, &Hands), With<Player>>,
    prop_q: Query<Entity, With<CarriedProp>>,
    stations: Query<(&Transform, &Station)>,
) {
    let Ok((p_xf, hands)) = player_q.single() else {
        return;
    };
    // desired model
    let desired: Option<(&str, Color)> = match &hands.held {
        None => None,
        Some(Held::Single(i)) => Some((i.model_file(), Color::WHITE)),
        Some(Held::Plate(_)) => Some(("plate.gltf", Color::WHITE)),
        Some(Held::Dish(r)) => Some((r.dish_model(), Color::WHITE)),
    };
    let has_prop = prop_q.single().is_ok();
    let need_respawn = match (&desired, has_prop) {
        (None, false) => false,
        (None, true) => true, // need despawn
        (Some(_), false) => true,
        (Some(_), true) => {
            // check if model matches: store in entity? simplify: respawn at most 4x/sec via frame counter?
            // To avoid churn, only respawn when count changes. Use a cheap approach: keep as-is unless hands changed.
            // We track via resource? Simplest: despawn+respawn every frame is wasteful but works for small scenes.
            // Instead respawn only every 0.25s. Use time-based throttle via local.
            false
        }
    };
    // Throttle: use a frame-skipping static via system param? Keep simple: only handle None<->Some transitions here,
    // and refresh Some->Some in a slower system below. For now handle transitions:
    if desired.is_none() && has_prop {
        for e in prop_q.iter() {
            commands.entity(e).despawn();
        }
        return;
    }
    if desired.is_some() && !has_prop {
        let (file, color) = desired.unwrap();
        let pos = p_xf.translation + Vec3::new(0.0, 1.6, 0.0);
        let e = spawn_model(&mut commands, &assets, &mut meshes, &mut materials, file, pos, 0.55, color);
        commands.entity(e).insert(CarriedProp).insert(InKitchen);
        return;
    }
    let _ = need_respawn;
    // prop movement handled in carried_follow; station progress shown in HUD prompt.
    let _ = stations;
}

pub fn carried_follow(
    player_q: Query<&Transform, With<Player>>,
    mut prop_q: Query<&mut Transform, (With<CarriedProp>, Without<Player>)>,
) {
    let Ok(p) = player_q.single() else { return };
    for mut t in prop_q.iter_mut() {
        t.translation = p.translation + Vec3::new(0.0, 1.7, 0.0);
    }
}

// ---------- orders ----------

pub fn order_spawner(
    mut commands: Commands,
    clock: Res<LevelClock>,
    selected: Res<SelectedLevel>,
    mut next: ResMut<NextOrderIndex>,
    active: Query<&OrderTicket>,
    economy: Res<crate::economy::Economy>,
) {
    if !clock.active {
        return;
    }
    let level = selected.0.min(LEVELS.len() - 1);
    let cfg = &LEVELS[level];
    if next.0 >= cfg.script.len() {
        return;
    }
    if active.iter().len() >= cfg.max_active_orders {
        return;
    }
    let s = &cfg.script[next.0];
    if clock.elapsed >= s.at_s {
        commands.spawn((
            OrderTicket {
                recipe: s.recipe,
                spawned_at: clock.elapsed,
                expires_at: clock.elapsed + cfg.order_expire_s + economy.upgrades.diner_bonus(level),
                vip: is_vip_order(level, next.0),
            },
            InKitchen,
        ));
        next.0 += 1;
    }
}

pub fn order_tick(
    mut commands: Commands,
    time: Res<Time>,
    mut clock: ResMut<LevelClock>,
    tickets: Query<(Entity, &OrderTicket)>,
    mut score: ResMut<RunScore>,
    assets: Res<GameAssets>,
    muted: Res<Muted>,
    mut hint: ResMut<Hint>,
    selected: Res<SelectedLevel>,
    mut next_state: ResMut<NextState<GameState>>,
    mut best: ResMut<Best>,
    mut combo: ResMut<crate::combo::Combo>,
    mut economy: ResMut<crate::economy::Economy>,
    settings: Res<crate::settings::Settings>,
) {
    if clock.active {
        clock.elapsed += time.delta_secs();
    }
    let psfx = settings.sfx;
    // expire
    let mut expired_ids = vec![];
    for (e, t) in tickets.iter() {
        if clock.elapsed >= t.expires_at {
            expired_ids.push(e);
        }
    }
    for e in expired_ids {
        commands.entity(e).despawn();
        // Mulligan Charm forgives the first expiry each run.
        if economy.upgrades.has(crate::economy::UpgradeId::Mulligan) && !economy.mulligan_used_this_run {
            economy.mulligan_used_this_run = true;
            hint.set("Mulligan Charm! Expired order forgiven (no penalty).");
            if !muted.0 {
                play_one(&mut commands, assets.snd_plate.clone(), 0.5 * psfx);
            }
            continue;
        }
        score.expired += 1;
        score.points = score.points.saturating_sub(50);
        crate::combo::note_expire(&mut combo);
        hint.set("Order expired! -50 pts, combo lost");
        if !muted.0 {
            play_one(&mut commands, assets.snd_whistle.clone(), 0.9 * psfx);
        }
    }
    // level end
    let cfg = &LEVELS[selected.0.min(LEVELS.len() - 1)];
    if clock.active && clock.elapsed >= cfg.time_limit_s {
        clock.active = false;
        // unserved count as expired for honesty? Just finish.
        let stars = crate::config::stars_for(score.points, cfg.star_thresholds);
        let i = selected.0.min(2);
        if score.points > best.scores[i] {
            best.scores[i] = score.points;
        }
        if stars > best.stars[i] {
            best.stars[i] = stars;
        }
        if stars >= 3 {
            economy.gems = economy.gems.saturating_add(1);
            hint.set("3 STARS! +1 gem. Spend it in the Shop.");
        }
        crate::save::save_all(&best, &economy);
        if !muted.0 {
            play_one(&mut commands, assets.snd_ding.clone(), 1.0 * psfx);
        }
        next_state.set(GameState::LevelComplete);
    }
}

pub fn toggle_mute(keys: Res<ButtonInput<KeyCode>>, settings: Res<crate::settings::Settings>, mut muted: ResMut<Muted>) {
    if keys.just_pressed(settings.bindings.mute) {
        muted.0 = !muted.0;
    }
}

pub fn pause_input(
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<crate::settings::Settings>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
) {
    if *state.get() != GameState::Playing {
        return;
    }
    if keys.just_pressed(settings.bindings.pause) || keys.just_pressed(KeyCode::KeyP) {
        next.set(GameState::Paused);
    }
}

pub fn resume_input(
    keys: Res<ButtonInput<KeyCode>>,
    settings: Res<crate::settings::Settings>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
) {
    if *state.get() != GameState::Paused {
        return;
    }
    if keys.just_pressed(settings.bindings.pause) || keys.just_pressed(KeyCode::KeyP) {
        next.set(GameState::Playing);
    }
}

/// Prompt text for HUD: nearest station + hands summary.
pub fn current_prompt(
    player_q: &Query<(&Transform, &Hands), With<Player>>,
    stations: &Query<(&Transform, &Station)>,
) -> String {
    let Ok((p_xf, hands)) = player_q.single() else {
        return String::new();
    };
    let mut best: Option<(&Station, f32)> = None;
    for (t, s) in stations.iter() {
        let d = p_xf.translation.distance(t.translation);
        if d <= 2.4 && best.map_or(true, |(_, bd)| d < bd) {
            best = Some((s, d));
        }
    }
    match best {
        None => String::new(),
        Some((s, _)) => {
            if s.processing.is_some() {
                if let Some(pr) = &s.processing {
                    let pct = ((pr.elapsed / pr.total).min(1.0) * 100.0) as i32;
                    return format!("{}: {}% ...", s.kind.name(), pct);
                }
                return format!("{}: working...", s.kind.name());
            }
            match &s.kind {
                StationKind::Crate(_) => "[E] Grab".to_string(),
                StationKind::PlateStack => "[E] Take plate".to_string(),
                StationKind::BowlStack => "[E] Take bowl (stew)".to_string(),
                StationKind::CuttingBoard | StationKind::Stove | StationKind::Oven | StationKind::Sink => {
                    if hands.held.is_some() {
                        "[E] Place / start".to_string()
                    } else if s.slot.is_some() {
                        "[E] Take result".to_string()
                    } else {
                        "[E] (bring ingredient)".to_string()
                    }
                }
                StationKind::Counter => {
                    if hands.held.is_some() {
                        "[E] Add / place".to_string()
                    } else if s.slot.is_some() {
                        "[E] Take".to_string()
                    } else {
                        "[E] (bring plate + food)".to_string()
                    }
                }
                StationKind::OrderWindow => "[E] Serve dish".to_string(),
                StationKind::Trash => "[E]/[Q] Trash".to_string(),
                StationKind::DishRack => "[E] Take dirty plate".to_string(),
            }
        }
    }
}

/// Unused helper kept for API stability.
pub fn nearest_station_unused() {
    let _ = nearest_station;
}
