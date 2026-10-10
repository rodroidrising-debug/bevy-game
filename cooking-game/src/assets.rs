//! Asset preloading: KayKit glTF models + kitchen sounds.

use bevy::asset::{LoadedFolder, LoadState};
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Resource)]
pub struct GameAssets {
    pub models_folder: Handle<LoadedFolder>,
    pub sounds_folder: Handle<LoadedFolder>,
    /// Preloaded scene handles by file name, e.g. "plate.gltf".
    pub scenes: HashMap<String, Handle<Scene>>,
    // Sounds (wav, shipped in the zip).
    pub snd_chop: Handle<AudioSource>,
    pub snd_sizzle: Handle<AudioSource>,
    pub snd_boil: Handle<AudioSource>,
    pub snd_whistle: Handle<AudioSource>,
    pub snd_plate: Handle<AudioSource>,
    pub snd_ding: Handle<AudioSource>,
    pub mus_menu: Handle<AudioSource>,
    pub mus_game: Handle<AudioSource>,
    pub mus_win: Handle<AudioSource>,
    pub loading_done: bool,
}

/// All model files the game may spawn. Kept in one list so the loading
/// screen can gate on them.
pub const MODEL_FILES: &[&str] = &[
    // crates
    "crate_buns.gltf",
    "crate_steak.gltf",
    "crate_carrots.gltf",
    "crate_cheese.gltf",
    "crate_ham.gltf",
    "crate_lettuce.gltf",
    "crate_onions.gltf",
    "crate_potatoes.gltf",
    "crate_tomatoes.gltf",
    "crate.gltf",
    // ingredients
    "food_ingredient_bun.gltf",
    "food_ingredient_burger_uncooked.gltf",
    "food_ingredient_burger_cooked.gltf",
    "food_ingredient_burger_trash.gltf",
    "food_ingredient_vegetableburger_uncooked.gltf",
    "food_ingredient_vegetableburger_cooked.gltf",
    "food_ingredient_carrot.gltf",
    "food_ingredient_carrot_pieces.gltf",
    "food_ingredient_carrot_chopped.gltf",
    "food_ingredient_cheese.gltf",
    "food_ingredient_cheese_slice.gltf",
    "food_ingredient_ham.gltf",
    "food_ingredient_ham_cooked.gltf",
    "food_ingredient_ham_trash.gltf",
    "food_ingredient_lettuce.gltf",
    "food_ingredient_lettuce_slice.gltf",
    "food_ingredient_lettuce_chopped.gltf",
    "food_ingredient_onion.gltf",
    "food_ingredient_onion_rings.gltf",
    "food_ingredient_onion_chopped.gltf",
    "food_ingredient_potato.gltf",
    "food_ingredient_potato_chopped.gltf",
    "food_ingredient_potato_mashed.gltf",
    "food_ingredient_steak.gltf",
    "food_ingredient_steak_pieces.gltf",
    "food_ingredient_tomato.gltf",
    "food_ingredient_tomato_slice.gltf",
    // dishes
    "food_burger.gltf",
    "food_dinner.gltf",
    "food_stew.gltf",
    "food_vegetableburger.gltf",
    "plate.gltf",
    "plate_dirty.gltf",
    "bowl_small.gltf",
    "pot_A.gltf",
    "pot_A_stew.gltf",
    // stations / furniture
    "cuttingboard.gltf",
    "knife.gltf",
    "stove_single.gltf",
    "stove_multi.gltf",
    "oven.gltf",
    "pan_A.gltf",
    "kitchencounter_straight_A.gltf",
    "kitchencounter_sink.gltf",
    "kitchentable_A_large.gltf",
    "dishrack_plates.gltf",
    "floor_kitchen.gltf",
    "wall.gltf",
    "wall_orderwindow.gltf",
    "fridge_A.gltf",
    "menu.gltf",
    "table_round_A.gltf",
    "chair_A.gltf",
];

pub fn start_loading(mut commands: Commands, asset_server: Res<AssetServer>) {
    let models_folder: Handle<LoadedFolder> = asset_server.load_folder("models");
    let sounds_folder: Handle<LoadedFolder> = asset_server.load_folder("sounds");

    let mut scenes = HashMap::new();
    for f in MODEL_FILES {
        let path = format!("models/{f}");
        let h: Handle<Scene> =
            asset_server.load(GltfAssetLabel::Scene(0).from_asset(path));
        scenes.insert(f.to_string(), h);
    }

    commands.insert_resource(GameAssets {
        models_folder,
        sounds_folder,
        scenes,
        snd_chop: asset_server.load("sounds/chop-vegetable-fast.wav"),
        snd_sizzle: asset_server.load("sounds/pan-sizzle-loop.wav"),
        snd_boil: asset_server.load("sounds/water-boil-rolling.wav"),
        snd_whistle: asset_server.load("sounds/kettle-whistle.wav"),
        snd_plate: asset_server.load("sounds/plate-set-down.wav"),
        snd_ding: asset_server.load("sounds/kitchen-timer-ding.wav"),
        mus_menu: asset_server.load("music/Lobby Time.mp3"),
        mus_game: asset_server.load("music/Monkeys Spinning Monkeys.mp3"),
        mus_win: asset_server.load("music/Happy Boy End Theme.mp3"),
        loading_done: false,
    });
}

/// Fraction 0..1 of critical handles loaded.
pub fn loading_fraction(asset_server: &AssetServer, assets: &GameAssets) -> f32 {
    let mut done = 0usize;
    let mut total = 0usize;
    for h in assets.scenes.values() {
        total += 1;
        // Failed counts as done: the game has colored-box fallbacks, so a
        // missing/corrupt model (e.g. stripped mobile packaging) must never
        // softlock the loading screen.
        if matches!(
            asset_server.get_load_state(h.id()),
            Some(LoadState::Loaded) | Some(LoadState::Failed(_))
        ) {
            done += 1;
        }
    }
    for h in [
        &assets.snd_chop,
        &assets.snd_sizzle,
        &assets.snd_boil,
        &assets.snd_whistle,
        &assets.snd_plate,
        &assets.snd_ding,
        &assets.mus_menu,
        &assets.mus_game,
        &assets.mus_win,
    ] {
        total += 1;
        if matches!(
            asset_server.get_load_state(h.id()),
            Some(LoadState::Loaded) | Some(LoadState::Failed(_))
        ) {
            done += 1;
        }
    }
    if total == 0 {
        1.0
    } else {
        done as f32 / total as f32
    }
}

pub fn all_critical_loaded(asset_server: &AssetServer, assets: &GameAssets) -> bool {
    loading_fraction(asset_server, assets) >= 0.999
}

/// Spawn a KayKit model by file name with fallback to a colored box
/// if the scene is not yet usable (keeps game playable no matter what).
pub fn spawn_model(
    commands: &mut Commands,
    assets: &GameAssets,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    file: &str,
    translation: Vec3,
    scale: f32,
    fallback_color: Color,
) -> Entity {
    if let Some(h) = assets.scenes.get(file) {
        commands
            .spawn((
                SceneRoot(h.clone()),
                Transform {
                    translation,
                    scale: Vec3::splat(scale),
                    ..default()
                },
            ))
            .id()
    } else {
        commands
            .spawn((
                Mesh3d(meshes.add(Cuboid::new(0.6, 0.4, 0.6))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: fallback_color,
                    ..default()
                })),
                Transform::from_translation(translation),
            ))
            .id()
    }
}

/// Play a one-shot sound (DESPAWN when finished).
pub fn play_one(commands: &mut Commands, handle: Handle<AudioSource>, volume: f32) {
    commands.spawn((
        AudioPlayer(handle),
        PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(volume)),
    ));
}
