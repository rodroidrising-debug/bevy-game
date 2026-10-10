//! Short-Order Showdown - full Bevy cooking game (Step 4).
//!
//! Flow: Loading -> MainMenu -> RestaurantSelect -> LevelBrief
//!   -> Playing <-> Paused -> LevelComplete -> GameComplete
//!
//! Step 3: Veggie Cafe rush (4 counters, full crates), tutorial coach,
//! combo streaks, highlight ring, quality grades, patience bars, lighting rigs, progress bars, diners.

mod assets;
mod ambience;
mod combo;
mod config;
mod customers;
mod economy;
mod effects;
mod highlight;
mod leaderboard;
mod minigames;
mod music;
mod patience;
mod quality;
mod save;
mod settings;
mod stew;
mod tutorial;
mod ui;
mod world;

use assets::start_loading;
use bevy::prelude::*;
use world::*;

fn main() {
    let (best, economy, mut settings) = save::load_full();
    if settings.player == "Chef" {
        // First run: stable install tag so leaderboard identity persists.
        settings.player = format!("Chef-{:04X}", install_bits());
        save::save_full(&best, &economy, &settings);
    }
    if settings.secret.is_empty() {
        settings.secret = format!(
            "{:04x}{:04x}{:04x}{:04x}{:04x}{:04x}{:04x}{:04x}",
            install_bits(),
            install_bits(),
            install_bits(),
            install_bits(),
            install_bits(),
            install_bits(),
            install_bits(),
            install_bits()
        );
        save::save_full(&best, &economy, &settings);
    }
    let board = leaderboard::BoardConfig::from_settings(&settings);
    let mut app = App::new();
    // Android builds use --features embed-assets so the APK needs no
    // separate asset packaging (xbuild does not bundle assets/).
    #[cfg(feature = "embed-assets")]
    let _ = app.add_plugins(bevy_embedded_assets::EmbeddedAssetPlugin {
        mode: bevy_embedded_assets::PluginMode::ReplaceDefault,
    });
    app.add_plugins(DefaultPlugins)
        .init_state::<GameState>()
        .enable_state_scoped_entities::<GameState>()
        .insert_resource(SelectedLevel(0))
        .insert_resource(LevelClock::default())
        .insert_resource(RunScore::default())
        .insert_resource(NextOrderIndex(0))
        .insert_resource(best)
        .insert_resource(Hint::default())
        .insert_resource(Muted(false))
        .insert_resource(NewRunQueued(false))
        .insert_resource(economy)
        .insert_resource(settings)
        .insert_resource(board)
        .insert_resource(leaderboard::TopScores::default())
        .insert_resource(leaderboard::NetInbox::default())
        .insert_resource(music::MusicState::default())
        .insert_resource(tutorial::reset_tutorial())
        .insert_resource(combo::Combo::default())
        .add_plugins(stew::StewPlugin)
        .add_event::<effects::OrderServed>()
        .add_systems(Startup, (effects::setup_effects, highlight::setup_highlight, ambience::setup_progress, patience::setup_patience))
        .insert_resource(ui::ShopReturn::default())
        .insert_resource(ui::ShopMsg::default())
        .insert_resource(ui::ShopTab::default())
        .insert_resource(ui::Listening::default())
        .add_systems(Update, music::music_tick)
        .add_systems(Update, music::global_volume_tick)
        .add_systems(
            Update,
            (
                effects::steam_spawner,
                effects::tick_steam,
                effects::chop_bounce,
                effects::serve_confetti_spawner,
                effects::tick_confetti,
                highlight::highlight_tick,
            )
                .run_if(in_state(GameState::Playing)),
        )
        .add_systems(
            Update,
            (customers::sync_customers, customers::tick_customers, customers::tick_clients)
                .run_if(in_state(GameState::Playing)),
        )
        // ---- Loading ----
        .add_systems(OnEnter(GameState::Loading), (start_loading, ui::setup_loading))
        .add_systems(Update, ui::update_loading.run_if(in_state(GameState::Loading)))
        // ---- Main menu ----
        .add_systems(OnEnter(GameState::MainMenu), (despawn_kitchen, leaderboard::menu_sync, ui::setup_menu))
        .add_systems(Update, ui::menu_buttons.run_if(in_state(GameState::MainMenu)))
        .add_systems(Update, leaderboard::net_poll)
        // ---- Restaurant select ----
        .add_systems(OnEnter(GameState::RestaurantSelect), (despawn_kitchen, ui::setup_select))
        .add_systems(Update, ui::select_buttons.run_if(in_state(GameState::RestaurantSelect)))
        .add_systems(Update, ui::update_chef_row.run_if(in_state(GameState::RestaurantSelect)))
        // ---- Settings ----
        .add_systems(OnEnter(GameState::Settings), ui::setup_settings)
        .add_systems(Update, ui::settings_buttons.run_if(in_state(GameState::Settings)))
        .add_systems(Update, ui::capture_rebind.run_if(in_state(GameState::Settings)))
        .add_systems(Update, ui::update_settings_text.run_if(in_state(GameState::Settings)))
        // ---- Level brief (new-run entry cleans old kitchen) ----
        .add_systems(OnEnter(GameState::LevelBrief), (despawn_kitchen, ui::setup_brief))
        .add_systems(Update, ui::brief_buttons.run_if(in_state(GameState::LevelBrief)))
        // ---- Playing (spawn guarded: resume-from-pause skips) ----
        .add_systems(
            OnEnter(GameState::Playing),
            (
                setup_lights_camera,
                ambience::restaurant_lighting_system,
                spawn_kitchen,
                ambience::spawn_diners_system,
                ui::setup_hud,
            ),
        )
        .add_systems(Update, player_movement.run_if(in_state(GameState::Playing)))
        .add_systems(Update, camera_follow.run_if(in_state(GameState::Playing)))
        .add_systems(Update, interact_system.run_if(in_state(GameState::Playing)))
        .add_systems(Update, processing_tick.run_if(in_state(GameState::Playing)))
        .add_systems(Update, order_spawner.run_if(in_state(GameState::Playing)))
        .add_systems(Update, order_tick.run_if(in_state(GameState::Playing)))
        .add_systems(Update, carried_visual_sync.run_if(in_state(GameState::Playing)))
        .add_systems(Update, carried_follow.run_if(in_state(GameState::Playing)))
        .add_systems(Update, hint_tick.run_if(in_state(GameState::Playing)))
        .add_systems(Update, combo::combo_tick.run_if(in_state(GameState::Playing)))
        .add_systems(Update, tutorial::tutorial_tick.run_if(in_state(GameState::Playing)))
        .add_systems(Update, toggle_mute.run_if(in_state(GameState::Playing)))
        .add_systems(Update, pause_input.run_if(in_state(GameState::Playing)))
        .add_systems(Update, ui::update_hud.run_if(in_state(GameState::Playing)))
        .add_systems(Update, ui::update_tutorial_line.run_if(in_state(GameState::Playing)))
        .add_systems(Update, ui::update_combo_line.run_if(in_state(GameState::Playing)))
        .add_systems(Update, minigames::chop_marker_tick.run_if(in_state(GameState::Playing)))
        .add_systems(Update, minigames::stir_heat_tick.run_if(in_state(GameState::Playing)))
        .add_systems(Update, minigames::update_mini_hud.run_if(in_state(GameState::Playing)))
        .add_systems(Update, patience::sync_patience.run_if(in_state(GameState::Playing)))
        .add_systems(
            Update,
            (
                ambience::sync_stove_glows,
                ambience::flicker_glows,
                ambience::player_anim,
                ambience::sync_progress_bars,
                ambience::tick_diners,
            )
                .run_if(in_state(GameState::Playing)),
        )
        // ---- Paused (overlay: kitchen + HUD preserved) ----
        .add_systems(OnEnter(GameState::Paused), ui::setup_paused)
        .add_systems(
            Update,
            (resume_input, ui::paused_buttons).run_if(in_state(GameState::Paused)),
        )
        // ---- Results (clean kitchen on enter) ----
        .add_systems(
            OnEnter(GameState::LevelComplete),
            (despawn_kitchen, leaderboard::board_refresh, ui::setup_complete),
        )
        .add_systems(Update, ui::update_top_board.run_if(in_state(GameState::LevelComplete)))
        .add_systems(
            Update,
            ui::complete_buttons.run_if(in_state(GameState::LevelComplete)),
        )
        .add_systems(OnEnter(GameState::GameComplete), (despawn_kitchen, ui::setup_gamecomplete))
        .add_systems(
            Update,
            ui::gamecomplete_buttons.run_if(in_state(GameState::GameComplete)),
        )
        // ---- Shop ----
        .add_systems(OnEnter(GameState::Shop), ui::setup_shop)
        .add_systems(Update, ui::shop_buttons.run_if(in_state(GameState::Shop)))
        .add_systems(Update, ui::update_shop_text.run_if(in_state(GameState::Shop)))
        .run();
}

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    #[default]
    Loading,
    MainMenu,
    RestaurantSelect,
    LevelBrief,
    Playing,
    Paused,
    LevelComplete,
    GameComplete,
    Shop,
    Settings,
}

pub use config::{match_recipe, score_serve, stars_for, Item, RecipeId};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classic_burger_matches() {
        let plate = vec![Item::Bun, Item::BurgerCooked, Item::LettuceSlice, Item::TomatoSlice];
        assert_eq!(match_recipe(&plate), Some(RecipeId::ClassicBurger));
    }

    #[test]
    fn wrong_combo_matches_nothing() {
        let plate = vec![Item::Bun, Item::CarrotChopped];
        assert_eq!(match_recipe(&plate), None);
    }

    #[test]
    fn scoring_and_stars() {
        let pts = score_serve(RecipeId::ClassicBurger, 10.0, 75.0);
        assert_eq!(pts, 130);
        assert_eq!(stars_for(600, (200, 400, 600)), 3);
        assert_eq!(stars_for(199, (200, 400, 600)), 0);
    }

    #[test]
    fn chop_and_cook_tables() {
        assert!(config::chop_result(Item::Tomato).is_some());
        assert!(config::chop_result(Item::BurgerRaw).is_none());
        assert!(config::cook_result(Item::BurgerRaw).is_some());
        assert!(config::cook_result(Item::Tomato).is_none());
    }

    #[test]
    fn stew_plate_matches() {
        let plate = stew::ladled_stew_plate();
        assert_eq!(match_recipe(&plate), Some(RecipeId::HeartyStew));
    }

    #[test]
    fn save_roundtrip() {
        let b = Best { scores: [10, 20, 30], stars: [1, 2, 3] };
        let s = save::serialize_best(&b);
        let back = save::parse_best(&s);
        assert_eq!(back.scores, b.scores);
        assert_eq!(back.stars, b.stars);
    }

    #[test]
    fn combo_pays_streaks() {
        assert_eq!(combo::combo_bonus(1), 0);
        assert_eq!(combo::combo_bonus(3), 20);
        assert_eq!(combo::combo_bonus(99), 50);
    }

    #[test]
    fn veggie_deluxe_matches() {
        let plate = vec![
            Item::Bun, Item::VegPattyCooked, Item::CheeseSlice,
            Item::OnionChopped, Item::CarrotChopped, Item::LettuceChopped,
        ];
        assert_eq!(match_recipe(&plate), Some(RecipeId::VeggieDeluxe));
    }

    #[test]
    fn noodle_bowl_matches_and_fries() {
        let plate = vec![Item::NoodlesCooked, Item::OnionChopped, Item::CarrotChopped];
        assert_eq!(match_recipe(&plate), Some(RecipeId::NoodleBowl));
        let (out, t, burn) = config::cook_result(Item::NoodleBrick).expect("noodles fry");
        assert_eq!(out, Item::NoodlesCooked);
        assert_eq!(t, 3.0);
        assert_eq!(burn, 6.0);
        assert_eq!(config::burned_version(Item::PotatoMashed), Item::Charred);
    }

    #[test]
    fn save_all_roundtrips_economy() {
        let b = Best { scores: [10, 20, 30], stars: [1, 2, 3] };
        let mut e = economy::Economy::default();
        e.coins = 77;
        e.gems = 2;
        e.upgrades.give(economy::UpgradeId::SharpKnife);
        e.perfect_serves = 9;
        let s = save::serialize_all(&b, &e);
        let (bb, ee) = save::parse_all(&s);
        assert_eq!(bb.scores, b.scores);
        assert_eq!(ee.coins, 77);
        assert_eq!(ee.gems, 2);
        assert!(ee.upgrades.has(economy::UpgradeId::SharpKnife));
        // old files without economy still load
        let (old_b, old_e) = save::parse_all("scores=1,2,3 stars=0,1,0");
        assert_eq!(old_b.scores, [1, 2, 3]);
        assert_eq!(old_e.coins, 0);
    }
}

fn install_bits() -> u16 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() ^ (d.as_secs() as u32))
        .unwrap_or(0xBEEF);
    let mut x = t.max(1);
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    (x & 0xFFFF) as u16
}
