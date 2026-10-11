//! All UI screens: loading, menus, HUD, pause, results.

use crate::assets::{all_critical_loaded, loading_fraction, GameAssets};
use crate::config::{LEVELS, RecipeId};
use crate::world::*;
use crate::GameState;
use bevy::prelude::*;

// ---------- markers ----------

fn click(commands: &mut Commands, assets: &GameAssets, muted: &Muted) {
    if !muted.0 {
        crate::assets::play_one(commands, assets.snd_plate.clone(), 0.35);
    }
}

/// Where the Shop's Back button returns to.
#[derive(Resource)]
pub struct ShopReturn(pub GameState);

impl Default for ShopReturn {
    fn default() -> Self {
        Self(GameState::RestaurantSelect)
    }
}

/// Last shop message (bought / can't afford).
#[derive(Resource, Default)]
pub struct ShopMsg(pub String);

#[derive(Component)]
pub struct LoadingBarFill;
#[derive(Component)]
pub struct LoadingText;

#[derive(Component)]
pub enum MenuBtn {
    Start,
    Settings,
    Quit,
}

#[derive(Component)]
pub enum SelectBtn {
    Level(usize),
    Chef,
    Shop,
    Back,
}

#[derive(Component)]
pub struct ChefText;

#[derive(Component)]
pub enum BriefBtn {
    Cook,
    Back,
}

#[derive(Component)]
pub enum PauseBtn {
    Resume,
    Restart,
    Menu,
}

#[derive(Component)]
pub enum CompleteBtn {
    Next,
    Replay,
    Menu,
    Shop,
}

#[derive(Component)]
pub struct HudTimer;
#[derive(Component)]
pub struct HudScore;
#[derive(Component)]
pub struct HudOrders;
#[derive(Component)]
pub struct HudCarried;
#[derive(Component)]
pub struct HudPrompt;
#[derive(Component)]
pub struct HudHint;
#[derive(Component)]
pub struct HudMini;
#[derive(Component)]
pub struct TopBoardText;

pub fn initial_board_text(top: &crate::leaderboard::TopScores) -> String {
    if top.entries.is_empty() {
        "Leaderboard: connecting...".to_string()
    } else {
        crate::leaderboard::render_top(&top.entries)
    }
}

pub fn update_top_board(
    top: Res<crate::leaderboard::TopScores>,
    cfg: Res<crate::leaderboard::BoardConfig>,
    mut q: Query<&mut Text, With<TopBoardText>>,
) {
    let Ok(mut t) = q.single_mut() else {
        return;
    };
    if top.pending {
        t.0 = format!("Leaderboard ({}): connecting...", cfg.player);
    } else if top.entries.is_empty() {
        t.0 = format!("Leaderboard: offline (playing as {}). Scores save locally.", cfg.player);
    } else {
        let mut s = format!("Online top ({}):\n", cfg.player);
        for (i, (name, pts, stars)) in top.entries.iter().enumerate() {
            s.push_str(&format!("{}. {} — {} ({}*)\n", i + 1, name, pts, stars));
        }
        t.0 = s;
    }
}
#[derive(Component)]
pub struct HudTutorial;
#[derive(Component)]
pub struct HudCombo;

// ---------- helpers ----------

fn root_full(color: Color) -> (Node, BackgroundColor, StateScoped<GameState>) {
    (
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(14.0),
            ..default()
        },
        BackgroundColor(color),
        // caller replaces state
        StateScoped(GameState::Loading),
    )
}

fn title_text(s: &str, size: f32, color: Color) -> (Text, TextFont, TextColor) {
    (
        Text::new(s),
        TextFont { font_size: size, ..default() },
        TextColor(color),
    )
}

/// Full-screen textured backdrop (first child = painted behind content).
/// `dim` multiplies the image so text stays readable.
fn backdrop(asset_server: &AssetServer, file: &str, dim: f32) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        ImageNode {
            image: asset_server.load(format!("images/{file}")),
            color: Color::srgb(dim, dim, dim),
            ..default()
        },
    )
}

fn menu_button(label: &str) -> (Button, Node, BackgroundColor, BorderColor) {
    (
        Button,
        Node {
            width: Val::Px(320.0),
            height: Val::Px(58.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.16, 0.5, 0.26)),
        BorderColor(Color::WHITE),
    )
}

// ---------- loading ----------

pub fn setup_loading(mut commands: Commands) {
    let (node, bg, _) = root_full(Color::srgb(0.08, 0.09, 0.12));
    commands.spawn((
        node,
        bg,
        StateScoped(GameState::Loading),
        children![
            title_text(
                "SHORT-ORDER SHOWDOWN",
                44.0,
                Color::srgb(1.0, 0.82, 0.35)
            ),
            title_text("Loading kitchen... (KayKit models + sounds)", 20.0, Color::WHITE),
            (
                Node {
                    width: Val::Px(420.0),
                    height: Val::Px(26.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.2, 0.2, 0.25)),
                children![(
                    LoadingBarFill,
                    Node {
                        width: Val::Percent(0.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.2, 0.8, 0.3)),
                )],
            ),
            (
                LoadingText,
                title_text("0%", 20.0, Color::WHITE).0,
                title_text("0%", 20.0, Color::WHITE).1,
                title_text("0%", 20.0, Color::WHITE).2,
            ),
        ],
    ));
}

pub fn update_loading(
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    assets: Option<Res<GameAssets>>,
    mut fill: Query<&mut Node, With<LoadingBarFill>>,
    mut txt: Query<&mut Text, With<LoadingText>>,
    mut next: ResMut<NextState<GameState>>,
    mut waited: Local<f32>,
) {
    let Some(a) = assets else { return };
    let f = loading_fraction(&asset_server, &a);
    if let Ok(mut n) = fill.single_mut() {
        n.width = Val::Percent(f * 100.0);
    }
    if let Ok(mut t) = txt.single_mut() {
        t.0 = format!("{}% - models + 6 kitchen sounds", (f * 100.0) as i32);
    }
    if all_critical_loaded(&asset_server, &a) {
        next.set(GameState::MainMenu);
        return;
    }
    // Failsafe: a missing decoder or file must never trap the player here.
    // (This is what bricked Windows builds before the `wav` feature existed.)
    *waited += time.delta_secs();
    if *waited > 25.0 {
        eprintln!("[loading] timeout after 25s at {:.0}% — continuing with fallbacks", f * 100.0);
        next.set(GameState::MainMenu);
    }
}

// ---------- main menu ----------

pub fn setup_menu(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(12.0),
            ..default()
        },
        BackgroundColor(Color::srgb(0.09, 0.11, 0.17)),
        StateScoped(GameState::MainMenu),
        children![
            backdrop(&assets, "menu_bg.png", 0.5),
            title_text(
                "SHORT-ORDER SHOWDOWN",
                54.0,
                Color::srgb(1.0, 0.82, 0.35)
            ),
            title_text(
                "A Bevy cooking game - KayKit Restaurant Bits + kitchen sounds",
                20.0,
                Color::WHITE
            ),
            title_text(
                "Move: your keys (rebind in Settings, arrows always work) | E interact/chop/stir | Q trash | R rescue | Esc pause | M mute",
                17.0,
                Color::srgb(0.7, 0.9, 0.7)
            ),
            title_text(
                "HOW TO PLAY: plate [E] -> ingredient -> CHOP in rhythm (E on ###) / STIR fry with E + R flip-rescue / bake (oven) -> assemble on Counter -> serve at Window. Wash at Sink. Stew: veg in pot, boil, bowl ladles. Fast serves grade up to PERFECT x1.5 + combo!",
                16.0,
                Color::srgb(0.8, 0.85, 0.9)
            ),
            (
                MenuBtn::Start,
                menu_button("Start").0,
                menu_button("Start").1,
                menu_button("Start").2,
                menu_button("Start").3,
                children![title_text("Start Cooking", 24.0, Color::WHITE)],
            ),
            (
                MenuBtn::Settings,
                Button,
                Node {
                    width: Val::Px(320.0),
                    height: Val::Px(52.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.16, 0.4, 0.5)),
                BorderColor(Color::WHITE),
                children![title_text("Settings + Keys", 22.0, Color::WHITE)],
            ),
            (
                MenuBtn::Quit,
                Button,
                Node {
                    width: Val::Px(320.0),
                    height: Val::Px(52.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.45, 0.15, 0.15)),
                BorderColor(Color::WHITE),
                children![title_text("Quit", 22.0, Color::WHITE)],
            ),
        ],
    ));
}

pub fn menu_buttons(
    mut commands: Commands,
    assets: Res<GameAssets>,
    muted: Res<Muted>,
    mut q: Query<(&Interaction, &MenuBtn, &mut BackgroundColor), (Changed<Interaction>, With<Button>)>,
    mut next: ResMut<NextState<GameState>>,
    mut exit: EventWriter<AppExit>,
) {
    for (inter, btn, mut color) in q.iter_mut() {
        match *inter {
            Interaction::Pressed => {
                click(&mut commands, &assets, &muted);
                match btn {
                MenuBtn::Start => next.set(GameState::RestaurantSelect),
                MenuBtn::Settings => next.set(GameState::Settings),
                MenuBtn::Quit => {
                    exit.write(AppExit::Success);
                }
                }
            }
            Interaction::Hovered => color.0 = Color::srgb(0.25, 0.62, 0.32),
            Interaction::None => {
                color.0 = match btn {
                    MenuBtn::Start => Color::srgb(0.16, 0.5, 0.26),
                    MenuBtn::Settings => Color::srgb(0.16, 0.4, 0.5),
                    MenuBtn::Quit => Color::srgb(0.45, 0.15, 0.15),
                }
            }
        }
    }
}

// ---------- restaurant select ----------

pub fn setup_select(mut commands: Commands, assets: Res<AssetServer>, best: Res<Best>) {
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(10.0),
            ..default()
        },
        BackgroundColor(Color::srgb(0.1, 0.12, 0.18)),
        StateScoped(GameState::RestaurantSelect),
        children![
            backdrop(&assets, "menu_bg.png", 0.45),
            title_text("Choose your restaurant (Step-by-step)", 36.0, Color::srgb(1.0, 0.82, 0.35)),
            (
                SelectBtn::Chef,
                Button,
                Node {
                    width: Val::Px(720.0),
                    height: Val::Px(54.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.3, 0.2, 0.35)),
                BorderColor(Color::WHITE),
                children![(
                    ChefText,
                    Text::new("Chef"),
                    TextFont { font_size: 19.0, ..default() },
                    TextColor(Color::WHITE),
                )],
            ),
            select_card(0, best.stars[0], best.scores[0], true),
            select_card(1, best.stars[1], best.scores[1], best.stars[0] >= 1),
            select_card(2, best.stars[2], best.scores[2], best.stars[1] >= 1),
            (
                SelectBtn::Shop,
                Button,
                Node {
                    width: Val::Px(720.0),
                    height: Val::Px(54.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.5, 0.38, 0.1)),
                BorderColor(Color::WHITE),
                children![title_text("Upgrade Shop (coins + gems)", 20.0, Color::WHITE)],
            ),
            (
                SelectBtn::Back,
                Button,
                Node {
                    width: Val::Px(320.0),
                    height: Val::Px(48.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.3, 0.3, 0.35)),
                BorderColor(Color::WHITE),
                children![title_text("< Back", 20.0, Color::WHITE)],
            ),
        ],
    ));
}

pub fn update_chef_row(
    settings: Res<crate::settings::Settings>,
    mut q: Query<&mut Text, With<ChefText>>,
) {
    if let Ok(mut t) = q.single_mut() {
        t.0 = format!("Chef: {} — {} (click to switch)", settings.chef.name(), settings.chef.blurb());
    }
}

fn select_card(
    level: usize,
    stars: u8,
    score: u32,
    unlocked: bool,
) -> impl Bundle {
    let cfg = &LEVELS[level];
    let label = if unlocked {
        format!(
            "Step {}: {} - {}  (best: {} stars, {} pts) [E> click]",
            level + 1,
            cfg.restaurant,
            cfg.subtitle,
            stars,
            score
        )
    } else {
        format!("Step {}: {} - LOCKED (earn 1 star before)", level + 1, cfg.restaurant)
    };
    let bg = if unlocked {
        Color::srgb(0.16, 0.45, 0.3)
    } else {
        Color::srgb(0.25, 0.25, 0.28)
    };
    (
        SelectBtn::Level(level),
        Button,
        Node {
            width: Val::Px(720.0),
            height: Val::Px(64.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(2.0)),
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(bg),
        BorderColor(Color::WHITE),
        children![(Text::new(label), TextFont { font_size: 18.0, ..default() }, TextColor(Color::WHITE))],
    )
}

pub fn select_buttons(
    mut commands: Commands,
    assets: Res<GameAssets>,
    muted: Res<Muted>,
    mut q: Query<(&Interaction, &SelectBtn), (Changed<Interaction>, With<Button>)>,
    mut next: ResMut<NextState<GameState>>,
    mut selected: ResMut<SelectedLevel>,
    best: Res<Best>,
    economy: Res<crate::economy::Economy>,
    mut settings: ResMut<crate::settings::Settings>,
    mut shop_return: ResMut<ShopReturn>,
) {
    for (inter, btn) in q.iter_mut() {
        if *inter != Interaction::Pressed {
            continue;
        }
        click(&mut commands, &assets, &muted);
        match btn {
            SelectBtn::Back => next.set(GameState::MainMenu),
            SelectBtn::Chef => {
                settings.chef = settings.chef.next();
                crate::save::save_full(&best, &economy, &settings);
            }
            SelectBtn::Shop => {
                shop_return.0 = GameState::RestaurantSelect;
                next.set(GameState::Shop);
            }
            SelectBtn::Level(i) => {
                let unlocked = *i == 0 || (*i == 1 && best.stars[0] >= 1) || (*i == 2 && best.stars[1] >= 1);
                if unlocked {
                    selected.0 = *i;
                    next.set(GameState::LevelBrief);
                }
            }
        }
    }
}

// ---------- level brief ----------
pub fn setup_brief(mut commands: Commands, assets: Res<AssetServer>, selected: Res<SelectedLevel>) {
    let cfg = &LEVELS[selected.0.min(LEVELS.len() - 1)];
    let mut recipe_lines = String::new();
    for r in cfg.recipes {
        recipe_lines.push_str(&format!("- {}: {}\n", r.name(), r.hint()));
    }
    let info = format!(
        "{} ({})\nTime: {}s | Orders expire after {}s | Max active: {}\nStars: 1>{}  2>{}  3>{}\n\nRecipes:\n{}",
        cfg.restaurant,
        cfg.subtitle,
        cfg.time_limit_s as i32,
        cfg.order_expire_s as i32,
        cfg.max_active_orders,
        cfg.star_thresholds.0,
        cfg.star_thresholds.1,
        cfg.star_thresholds.2,
        recipe_lines
    );
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(10.0),
            padding: UiRect::all(Val::Px(20.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.1, 0.12, 0.18)),
        StateScoped(GameState::LevelBrief),
        children![
            backdrop(&assets, "menu_bg.png", 0.42),
            title_text("Level Brief", 40.0, Color::srgb(1.0, 0.82, 0.35)),
            (
                Text::new(info),
                TextFont { font_size: 19.0, ..default() },
                TextColor(Color::WHITE),
            ),
            title_text(
                "Flow: grab crate -> chop (board) / fry (stove) / bake (oven) -> plate on Counter -> serve at Window. Wash dirty plates at Sink!",
                17.0,
                Color::srgb(0.7, 0.9, 0.7)
            ),
            (
                BriefBtn::Cook,
                Button,
                Node {
                    width: Val::Px(320.0),
                    height: Val::Px(58.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.16, 0.5, 0.26)),
                BorderColor(Color::WHITE),
                children![title_text("Start Cooking!", 24.0, Color::WHITE)],
            ),
            (
                BriefBtn::Back,
                Button,
                Node {
                    width: Val::Px(320.0),
                    height: Val::Px(48.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.3, 0.3, 0.35)),
                BorderColor(Color::WHITE),
                children![title_text("< Back", 20.0, Color::WHITE)],
            ),
        ],
    ));
}

pub fn brief_buttons(
    mut commands: Commands,
    assets: Res<GameAssets>,
    muted: Res<Muted>,
    mut q: Query<(&Interaction, &BriefBtn), (Changed<Interaction>, With<Button>)>,
    mut next: ResMut<NextState<GameState>>,
    mut selected: ResMut<SelectedLevel>,
    mut new_run: ResMut<NewRunQueued>,
) {
    for (inter, btn) in q.iter_mut() {
        if *inter != Interaction::Pressed {
            continue;
        }
        click(&mut commands, &assets, &muted);
        match btn {
            BriefBtn::Cook => {
                new_run.0 = true; // fresh run: spawn kitchen + reset score/clock/tutorial/combo
                next.set(GameState::Playing);
            }
            BriefBtn::Back => {
                let _ = &mut selected;
                next.set(GameState::RestaurantSelect);
            }
        }
    }
}

// ---------- settings ----------

#[derive(Component)]
pub enum SettingsBtn {
    Vol(u8, f32),
    Bind(usize),
    Reset,
    Back,
}

#[derive(Component)]
pub struct SettingsVolText;

#[derive(Component)]
pub struct BindLabel(pub usize);

#[derive(Component)]
pub struct SettingsMsgText;

/// Which binding row is waiting for a keypress.
#[derive(Resource, Default)]
pub struct Listening(pub Option<usize>);

pub fn setup_settings(mut commands: Commands, assets: Res<AssetServer>, settings: Res<crate::settings::Settings>) {
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            padding: UiRect::all(Val::Px(14.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.09, 0.1, 0.14)),
        StateScoped(GameState::Settings),
        children![
            backdrop(&assets, "menu_bg.png", 0.45),
            title_text("Settings", 40.0, Color::srgb(1.0, 0.82, 0.35)),
            (
                SettingsVolText,
                Text::new(vol_line(&settings)),
                TextFont { font_size: 20.0, ..default() },
                TextColor(Color::WHITE),
            ),
            vol_all_row(),
            title_text("Keys (click, then press a key)", 20.0, Color::srgb(0.7, 0.9, 0.7)),
            bind_group(&settings, 0),
            bind_group(&settings, 3),
            bind_group(&settings, 6),
            (
                SettingsMsgText,
                Text::new("Arrows / Space / X / P always work as alternates."),
                TextFont { font_size: 16.0, ..default() },
                TextColor(Color::srgb(0.75, 0.75, 0.8)),
            ),
            (
                SettingsBtn::Reset,
                Button,
                Node {
                    width: Val::Px(320.0),
                    height: Val::Px(44.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.5, 0.4, 0.12)),
                BorderColor(Color::WHITE),
                children![title_text("Reset defaults", 18.0, Color::WHITE)],
            ),
            (
                SettingsBtn::Back,
                Button,
                Node {
                    width: Val::Px(320.0),
                    height: Val::Px(48.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.3, 0.3, 0.35)),
                BorderColor(Color::WHITE),
                children![title_text("< Back", 20.0, Color::WHITE)],
            ),
            (
                Text::new(format!("Online tag: {} | server: {} (set server= URL in save.txt for your Render deploy)\nMusic: Kevin MacLeod (incompetech.com), CC-BY 4.0: Lobby Time / Monkeys Spinning Monkeys / Happy Boy End Theme. SFX: free-kitchen-sounds. Models: KayKit Restaurant Bits.", settings.player, if settings.server.is_empty() { "local" } else { settings.server.as_str() })),
                TextFont { font_size: 13.0, ..default() },
                TextColor(Color::srgb(0.6, 0.6, 0.65)),
            ),
        ],
    ));
}

fn vol_line(settings: &crate::settings::Settings) -> String {
    format!(
        "Master {}% | Music {}% | SFX {}%",
        (settings.master * 100.0) as i32,
        (settings.music * 100.0) as i32,
        (settings.sfx * 100.0) as i32
    )
}

fn vol_btn(which: u8, dir: f32, label: &str) -> impl Bundle {
    (
        SettingsBtn::Vol(which, dir),
        Button,
        Node {
            width: Val::Px(48.0),
            height: Val::Px(36.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.3, 0.3, 0.35)),
        BorderColor(Color::WHITE),
        children![title_text(label, 20.0, Color::WHITE)],
    )
}

fn vol_all_row() -> impl Bundle {
    (
        Node {
            width: Val::Px(640.0),
            height: Val::Px(42.0),
            justify_content: JustifyContent::SpaceEvenly,
            align_items: AlignItems::Center,
            ..default()
        },
        children![
            (Text::new("Master"), TextFont { font_size: 17.0, ..default() }, TextColor(Color::WHITE)),
            vol_btn(0, -1.0, "-"),
            vol_btn(0, 1.0, "+"),
            (Text::new("Music"), TextFont { font_size: 17.0, ..default() }, TextColor(Color::WHITE)),
            vol_btn(1, -1.0, "-"),
            vol_btn(1, 1.0, "+"),
            (Text::new("SFX"), TextFont { font_size: 17.0, ..default() }, TextColor(Color::WHITE)),
            vol_btn(2, -1.0, "-"),
            vol_btn(2, 1.0, "+"),
        ],
    )
}


fn bind_group(settings: &crate::settings::Settings, start: usize) -> impl Bundle {
    (
        Node {
            width: Val::Px(520.0),
            justify_content: JustifyContent::FlexStart,
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        },
        children![
            bind_row(start, settings),
            bind_row(start + 1, settings),
            bind_row(start + 2, settings),
        ],
    )
}

fn bind_row(i: usize, settings: &crate::settings::Settings) -> impl Bundle {
    (
        Node {
            width: Val::Px(520.0),
            height: Val::Px(40.0),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        },
        children![
            (
                Text::new(crate::settings::Bindings::ACTIONS[i.min(8)]),
                TextFont { font_size: 18.0, ..default() },
                TextColor(Color::WHITE),
            ),
            (
                SettingsBtn::Bind(i),
                Button,
                Node {
                    width: Val::Px(140.0),
                    height: Val::Px(36.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.16, 0.4, 0.5)),
                BorderColor(Color::WHITE),
                children![(
                    BindLabel(i),
                    Text::new(crate::settings::key_name(settings.bindings.get(i))),
                    TextFont { font_size: 17.0, ..default() },
                    TextColor(Color::WHITE),
                )],
            ),
        ],
    )
}

pub fn settings_buttons(
    mut commands: Commands,
    assets: Res<GameAssets>,
    muted: Res<Muted>,
    mut q: Query<(&Interaction, &SettingsBtn), (Changed<Interaction>, With<Button>)>,
    mut settings: ResMut<crate::settings::Settings>,
    mut listening: ResMut<Listening>,
    best: Res<Best>,
    economy: Res<crate::economy::Economy>,
    mut next: ResMut<NextState<GameState>>,
) {
    for (inter, btn) in q.iter_mut() {
        if *inter != Interaction::Pressed {
            continue;
        }
        match btn {
            SettingsBtn::Vol(which, dir) => {
                settings.bump(*which, *dir);
                click(&mut commands, &assets, &muted);
                crate::save::save_full(&best, &economy, &settings);
            }
            SettingsBtn::Bind(i) => {
                listening.0 = Some(*i);
                click(&mut commands, &assets, &muted);
            }
            SettingsBtn::Reset => {
                let (chef, player, server, secret) = (settings.chef, settings.player.clone(), settings.server.clone(), settings.secret.clone());
                *settings = crate::settings::Settings::default();
                settings.chef = chef;
                settings.player = player;
                settings.server = server;
                settings.secret = secret;
                listening.0 = None;
                click(&mut commands, &assets, &muted);
                crate::save::save_full(&best, &economy, &settings);
            }
            SettingsBtn::Back => {
                listening.0 = None;
                click(&mut commands, &assets, &muted);
                crate::save::save_full(&best, &economy, &settings);
                next.set(GameState::MainMenu);
            }
        }
    }
}

/// Capture the next keyboard press for the armed binding row.
pub fn capture_rebind(
    keys: Res<ButtonInput<KeyCode>>,
    mut listening: ResMut<Listening>,
    mut settings: ResMut<crate::settings::Settings>,
    best: Res<Best>,
    economy: Res<crate::economy::Economy>,
) {
    let Some(i) = listening.0 else {
        return;
    };
    for k in keys.get_just_pressed() {
        let k = *k;
        if crate::settings::key_from_name(crate::settings::key_name(k)).is_some() {
            settings.bindings.set(i, k);
            listening.0 = None;
            crate::save::save_full(&best, &economy, &settings);
        }
        break;
    }
}

pub fn update_settings_text(
    settings: Res<crate::settings::Settings>,
    listening: Res<Listening>,
    mut vol_q: Query<&mut Text, (With<SettingsVolText>, Without<BindLabel>, Without<SettingsMsgText>)>,
    mut binds: Query<(&mut Text, &BindLabel), (Without<SettingsVolText>, Without<SettingsMsgText>)>,
    mut msg_q: Query<&mut Text, (With<SettingsMsgText>, Without<SettingsVolText>, Without<BindLabel>)>,
) {
    if let Ok(mut t) = vol_q.single_mut() {
        t.0 = vol_line(&settings);
    }
    for (mut t, label) in binds.iter_mut() {
        t.0 = crate::settings::key_name(settings.bindings.get(label.0)).to_string();
    }
    if let Ok(mut t) = msg_q.single_mut() {
        t.0 = match listening.0 {
            Some(i) => format!("Press a key for {}...", crate::settings::Bindings::ACTIONS[i.min(8)]),
            None => "Arrows / Space / X / P always work as alternates.".to_string(),
        };
    }
}

// ---------- shop ----------

#[derive(Component)]
pub enum ShopBtn {
    Tab(u8),
    Buy(usize),
    BuyMeal(usize),
    BuyDiner(usize),
    Back,
}

#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
pub enum ShopTab {
    #[default]
    Gear,
    Meals,
    Diner,
}

impl ShopTab {
    pub fn name(&self) -> &'static str {
        match self {
            ShopTab::Gear => "Gear",
            ShopTab::Meals => "Meals",
            ShopTab::Diner => "Diners",
        }
    }
}

#[derive(Component)]
pub struct ShopRoot;

#[derive(Component)]
pub struct ShopWallet;

#[derive(Component)]
pub struct ShopMsgText;

pub fn setup_shop(
    mut commands: Commands,
    assets: Res<AssetServer>,
    economy: Res<crate::economy::Economy>,
    tab: Res<ShopTab>,
) {
    build_shop(&mut commands, &assets, &economy, *tab);
}

fn shop_text(s: &str, size: f32) -> (Text, TextFont, TextColor) {
    (Text::new(s), TextFont { font_size: size, ..default() }, TextColor(Color::WHITE))
}

fn shop_button(btn: ShopBtn, label: &str, w: f32) -> impl Bundle {
    (
        btn,
        Button,
        Node {
            width: Val::Px(w),
            height: Val::Px(40.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.16, 0.5, 0.26)),
        BorderColor(Color::WHITE),
        children![shop_text(label, 15.0)],
    )
}

fn attach(commands: &mut Commands, root: Entity, b: impl Bundle) {
    commands.spawn(b).insert(ChildOf(root));
}

fn meal_recipe(i: usize) -> crate::config::RecipeId {
    use crate::config::RecipeId::*;
    [ClassicBurger, Cheeseburger, DeluxeBurger, HeartyStew, SundayDinner, VeggieBurger, VeggieDeluxe, NoodleBowl][i.min(7)]
}

pub fn build_shop(commands: &mut Commands, assets: &AssetServer, economy: &crate::economy::Economy, tab: ShopTab) {
    let root = commands
        .spawn((
            ShopRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(5.0),
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.1, 0.1, 0.14)),
            StateScoped(GameState::Shop),
        ))
        .id();
    attach(commands, root, backdrop(assets, "menu_bg.png", 0.4));
    attach(commands, root, title_text("Upgrade Shop", 36.0, Color::srgb(1.0, 0.85, 0.4)));
    attach(commands, root, (
        ShopWallet,
        shop_text(&format!("Coins {} | Gems {}", economy.coins, economy.gems), 21.0).0,
        shop_text("", 21.0).1,
        TextColor(Color::srgb(1.0, 0.9, 0.5)),
    ));
    // tabs
    for (i, name) in ["Gear", "Meals", "Diners"].iter().enumerate() {
        let _ = (i, name);
    }
    attach(commands, root, (
        Node {
            width: Val::Px(760.0),
            height: Val::Px(46.0),
            justify_content: JustifyContent::SpaceEvenly,
            align_items: AlignItems::Center,
            ..default()
        },
        children![
            shop_button(ShopBtn::Tab(0), "Gear", 150.0),
            shop_button(ShopBtn::Tab(1), "Meals", 150.0),
            shop_button(ShopBtn::Tab(2), "Diners", 150.0),
        ],
    ));
    attach(commands, root, shop_text(
        match tab {
            ShopTab::Gear => "Kitchen gear (permanent). Mulligan needs 1 gem.",
            ShopTab::Meals => "Dish recipes: +10% points per level (max 3).",
            ShopTab::Diner => "Restaurants: +5s ticket patience per level (max 3).",
        },
        16.0,
    ));
    match tab {
        ShopTab::Gear => {
            for id in crate::economy::UpgradeId::ALL {
                let (c, g) = id.cost();
                let cost = if g > 0 {
                    format!("{}c + {}g", c, g)
                } else {
                    format!("{}c", c)
                };
                let label = if economy.upgrades.has(id) {
                    "OWNED".to_string()
                } else {
                    format!("Buy {}", cost)
                };
                attach(commands, root, shop_row(&format!("{} — {}", id.name(), id.blurb()), &label, ShopBtn::Buy(id.index())));
            }
        }
        ShopTab::Meals => {
            for r in [
                crate::config::RecipeId::ClassicBurger,
                crate::config::RecipeId::Cheeseburger,
                crate::config::RecipeId::DeluxeBurger,
                crate::config::RecipeId::HeartyStew,
                crate::config::RecipeId::SundayDinner,
                crate::config::RecipeId::VeggieBurger,
                crate::config::RecipeId::VeggieDeluxe,
                crate::config::RecipeId::NoodleBowl,
            ] {
                let lv = economy.upgrades.meals[r.index()];
                let label = if lv >= 3 {
                    "MAX".to_string()
                } else {
                    format!("Buy {}c", economy.upgrades.meal_cost(r.index()))
                };
                attach(commands, root, shop_row(
                    &format!("{} — Lv {} (+{}%)", r.name(), lv, lv as u32 * 10),
                    &label,
                    ShopBtn::BuyMeal(r.index()),
                ));
            }
        }
        ShopTab::Diner => {
            for (i, name) in ["Burger Bistro", "Stew House", "Veggie Cafe"].iter().enumerate() {
                let lv = economy.upgrades.diner_lv[i];
                let label = if lv >= 3 {
                    "MAX".to_string()
                } else {
                    format!("Buy {}c", economy.upgrades.diner_cost(i))
                };
                attach(commands, root, shop_row(
                    &format!("{} — Lv {} (+{}s patience)", name, lv, lv as u32 * 5),
                    &label,
                    ShopBtn::BuyDiner(i),
                ));
            }
        }
    }
    attach(commands, root, (
        ShopMsgText,
        shop_text("", 18.0).0,
        shop_text("", 18.0).1,
        TextColor(Color::srgb(0.7, 0.95, 0.7)),
    ));
    attach(commands, root, (
        ShopBtn::Back,
        Button,
        Node {
            width: Val::Px(320.0),
            height: Val::Px(44.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.3, 0.3, 0.35)),
        BorderColor(Color::WHITE),
        children![title_text("< Back", 20.0, Color::WHITE)],
    ));
}

fn shop_row(desc: &str, btn_label: &str, btn: ShopBtn) -> impl Bundle {
    (
        Node {
            width: Val::Px(760.0),
            height: Val::Px(50.0),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            padding: UiRect::all(Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.16, 0.16, 0.2)),
        children![
            (Text::new(desc), TextFont { font_size: 16.0, ..default() }, TextColor(Color::WHITE)),
            (
                btn,
                Button,
                Node {
                    width: Val::Px(190.0),
                    height: Val::Px(38.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.16, 0.5, 0.26)),
                BorderColor(Color::WHITE),
                children![(
                    Text::new(btn_label),
                    TextFont { font_size: 15.0, ..default() },
                    TextColor(Color::WHITE),
                )],
            ),
        ],
    )
}

pub fn shop_buttons(
    mut commands: Commands,
    assets: Res<GameAssets>,
    asset_server: Res<AssetServer>,
    muted: Res<Muted>,
    mut q: Query<(Entity, &Interaction, &ShopBtn, &mut BackgroundColor), (Changed<Interaction>, With<Button>)>,
    mut economy: ResMut<crate::economy::Economy>,
    best: Res<Best>,
    mut tab: ResMut<ShopTab>,
    roots: Query<Entity, With<ShopRoot>>,
    mut msg: ResMut<ShopMsg>,
    mut next: ResMut<NextState<GameState>>,
    shop_return: Res<ShopReturn>,
    mut inbox: ResMut<crate::leaderboard::NetInbox>,
    cfg: Res<crate::leaderboard::BoardConfig>,
) {
    for (e, inter, btn, mut color) in q.iter_mut() {
        if *inter != Interaction::Pressed {
            continue;
        }
        match btn {
            ShopBtn::Back => {
                click(&mut commands, &assets, &muted);
                msg.0.clear();
                next.set(shop_return.0.clone());
            }
            ShopBtn::Tab(i) => {
                let nt = match i {
                    1 => ShopTab::Meals,
                    2 => ShopTab::Diner,
                    _ => ShopTab::Gear,
                };
                *tab = nt;
                click(&mut commands, &assets, &muted);
                for r in roots.iter() {
                    commands.entity(r).despawn();
                }
                build_shop(&mut commands, &asset_server, &economy, nt);
            }
            ShopBtn::Buy(i) => {
                let id = crate::economy::UpgradeId::ALL[(*i).min(5)];
                if inbox.online && !inbox.busy() {
                    crate::leaderboard::shop_buy_async(&mut inbox, &cfg, "gear", id.index() as u8);
                    msg.0 = format!("Buying {}...", id.name());
                    click(&mut commands, &assets, &muted);
                    let _ = e;
                } else {
                    match economy.buy(id) {
                        Ok(()) => {
                            msg.0 = format!("Bought {}! (offline)", id.name());
                            color.0 = Color::srgb(0.55, 0.45, 0.12);
                            crate::assets::play_one(&mut commands, assets.snd_ding.clone(), 0.8);
                            crate::save::save_all(&best, &economy);
                            let _ = e;
                        }
                        Err(why) => {
                            msg.0 = format!("{}: {}", id.name(), why);
                            click(&mut commands, &assets, &muted);
                        }
                    }
                }
            }
            ShopBtn::BuyMeal(i) => {
                let r = meal_recipe(*i);
                if inbox.online && !inbox.busy() {
                    crate::leaderboard::shop_buy_async(&mut inbox, &cfg, "meal", (*i).min(7) as u8);
                    msg.0 = format!("Upgrading {}...", r.name());
                    click(&mut commands, &assets, &muted);
                    let _ = e;
                } else {
                    match economy.buy_meal(*i) {
                        Ok(()) => {
                            msg.0 = format!("{} upgraded to Lv {}! (offline)", r.name(), economy.upgrades.meals[(*i).min(7)]);
                            color.0 = Color::srgb(0.55, 0.45, 0.12);
                            crate::assets::play_one(&mut commands, assets.snd_ding.clone(), 0.8);
                            crate::save::save_all(&best, &economy);
                            let _ = e;
                        }
                        Err(why) => {
                            msg.0 = format!("{}: {}", r.name(), why);
                            click(&mut commands, &assets, &muted);
                        }
                    }
                }
            }
            ShopBtn::BuyDiner(i) => {
                let names = ["Burger Bistro", "Stew House", "Veggie Cafe"];
                if inbox.online && !inbox.busy() {
                    crate::leaderboard::shop_buy_async(&mut inbox, &cfg, "diner", (*i).min(2) as u8);
                    msg.0 = format!("Upgrading {}...", names[(*i).min(2)]);
                    click(&mut commands, &assets, &muted);
                    let _ = e;
                } else {
                    match economy.buy_diner(*i) {
                        Ok(()) => {
                            msg.0 = format!("{} upgraded to Lv {}! (offline)", names[(*i).min(2)], economy.upgrades.diner_lv[(*i).min(2)]);
                            color.0 = Color::srgb(0.55, 0.45, 0.12);
                            crate::assets::play_one(&mut commands, assets.snd_ding.clone(), 0.8);
                            crate::save::save_all(&best, &economy);
                            let _ = e;
                        }
                        Err(why) => {
                            msg.0 = format!("{}: {}", names[(*i).min(2)], why);
                            click(&mut commands, &assets, &muted);
                        }
                    }
                }
            }
        }
    }
}

pub fn update_shop_text(
    economy: Res<crate::economy::Economy>,
    msg: Res<ShopMsg>,
    mut wallet_q: Query<&mut Text, (With<ShopWallet>, Without<ShopMsgText>)>,
    mut msg_q: Query<&mut Text, (With<ShopMsgText>, Without<ShopWallet>)>,
) {
    if let Ok(mut t) = wallet_q.single_mut() {
        t.0 = format!("Coins {} | Gems {}  (serves + tips, PERFECT streaks, 3-star gems)", economy.coins, economy.gems);
    }
    if let Ok(mut t) = msg_q.single_mut() {
        t.0 = msg.0.clone();
    }
}


// ---------- HUD ----------

pub fn setup_hud(mut commands: Commands, hud_q: Query<(), With<HudTimer>>) {
    if !hud_q.is_empty() {
        return; // resume from pause: HUD already present
    }
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::FlexStart,
            align_items: AlignItems::FlexStart,
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(12.0)),
            row_gap: Val::Px(6.0),
            ..default()
        },
        InKitchen,
        children![
            (
                HudTimer,
                Text::new("Time 0:00"),
                TextFont { font_size: 26.0, ..default() },
                TextColor(Color::srgb(1.0, 0.85, 0.4)),
            ),
            (
                HudScore,
                Text::new("Score 0"),
                TextFont { font_size: 24.0, ..default() },
                TextColor(Color::WHITE),
            ),
            (
                HudOrders,
                Text::new("No orders"),
                TextFont { font_size: 20.0, ..default() },
                TextColor(Color::srgb(1.0, 0.9, 0.6)),
            ),
            (
                HudCarried,
                Text::new("Hands: empty"),
                TextFont { font_size: 20.0, ..default() },
                TextColor(Color::srgb(0.6, 1.0, 0.6)),
            ),
            (
                HudPrompt,
                Text::new(""),
                TextFont { font_size: 20.0, ..default() },
                TextColor(Color::srgb(0.7, 0.85, 1.0)),
            ),
            (
                HudHint,
                Text::new(""),
                TextFont { font_size: 20.0, ..default() },
                TextColor(Color::srgb(1.0, 0.75, 0.5)),
            ),
            (
                HudTutorial,
                Text::new(""),
                TextFont { font_size: 19.0, ..default() },
                TextColor(Color::srgb(0.55, 0.95, 1.0)),
            ),
            (
                HudCombo,
                Text::new(""),
                TextFont { font_size: 19.0, ..default() },
                TextColor(Color::srgb(1.0, 0.9, 0.3)),
            ),
            (
                HudMini,
                Text::new(""),
                TextFont { font_size: 20.0, ..default() },
                TextColor(Color::srgb(1.0, 0.95, 0.5)),
            ),
            (
                Text::new("Move: WASD/arrows | E interact/stir/chop | R rescue | Q trash | Esc pause | M mute (all rebindable in Settings)"),
                TextFont { font_size: 16.0, ..default() },
                TextColor(Color::srgb(0.75, 0.75, 0.8)),
            ),
        ],
    ));
}

pub fn update_hud(
    clock: Res<LevelClock>,
    score: Res<RunScore>,
    tickets: Query<&OrderTicket>,
    player_q: Query<(&Transform, &Hands), With<Player>>,
    stations: Query<(&Transform, &Station)>,
    hint: Res<Hint>,
    selected: Res<SelectedLevel>,
    muted: Res<Muted>,
    economy: Res<crate::economy::Economy>,
    mut timer_q: Query<&mut Text, (With<HudTimer>, Without<HudScore>, Without<HudOrders>, Without<HudCarried>, Without<HudPrompt>, Without<HudHint>)>,
    mut score_q: Query<&mut Text, (With<HudScore>, Without<HudTimer>, Without<HudOrders>, Without<HudCarried>, Without<HudPrompt>, Without<HudHint>)>,
    mut orders_q: Query<&mut Text, (With<HudOrders>, Without<HudTimer>, Without<HudScore>, Without<HudCarried>, Without<HudPrompt>, Without<HudHint>)>,
    mut carried_q: Query<&mut Text, (With<HudCarried>, Without<HudTimer>, Without<HudScore>, Without<HudOrders>, Without<HudPrompt>, Without<HudHint>)>,
    mut prompt_q: Query<&mut Text, (With<HudPrompt>, Without<HudTimer>, Without<HudScore>, Without<HudOrders>, Without<HudCarried>, Without<HudHint>)>,
    mut hint_q: Query<&mut Text, (With<HudHint>, Without<HudTimer>, Without<HudScore>, Without<HudOrders>, Without<HudCarried>, Without<HudPrompt>)>,
) {
    let cfg = &LEVELS[selected.0.min(LEVELS.len() - 1)];
    if let Ok(mut t) = timer_q.single_mut() {
        let remain = (cfg.time_limit_s - clock.elapsed).max(0.0);
        let m = (remain / 60.0).floor() as i32;
        let s = (remain % 60.0).floor() as i32;
        t.0 = format!("{} - Time {}:{:02} {}", cfg.restaurant, m, s, if muted.0 { "[MUTED]" } else { "" });
    }
    if let Ok(mut t) = score_q.single_mut() {
        t.0 = format!("Score {} | Served {} | Expired {} | Trashed {} | Coins {} | Gems {}", score.points, score.served, score.expired, score.trashed, economy.coins, economy.gems);
    }
    if let Ok(mut t) = orders_q.single_mut() {
        if tickets.iter().len() == 0 {
            t.0 = "Orders: waiting for tickets...".to_string();
        } else {
            let mut lines = vec!["Orders:".to_string()];
            for o in tickets.iter() {
                let left = (o.expires_at - clock.elapsed).max(0.0);
                lines.push(format!("- {}{} ({:.0}s left)", if o.vip { "VIP " } else { "" }, o.recipe.name(), left));
            }
            t.0 = lines.join("\n");
        }
    }
    if let Ok(mut t) = carried_q.single_mut() {
        if let Ok((_, hands)) = player_q.single() {
            t.0 = match &hands.held {
                None => "Hands: empty".to_string(),
                Some(h) => format!("Hands: {}", h.label()),
            };
        }
    }
    if let Ok(mut t) = prompt_q.single_mut() {
        t.0 = current_prompt(&player_q, &stations);
    }
    if let Ok(mut t) = hint_q.single_mut() {
        t.0 = hint.text.clone();
    }
}

pub fn update_tutorial_line(
    selected: Res<SelectedLevel>,
    tutorial: Res<crate::tutorial::Tutorial>,
    mut tut_q: Query<&mut Text, With<HudTutorial>>,
) {
    if let Ok(mut t) = tut_q.single_mut() {
        if selected.0 == 0 && !tutorial.done {
            t.0 = format!("Tutorial: {}", crate::tutorial::tutorial_line(&tutorial));
        } else {
            t.0 = String::new();
        }
    }
}

pub fn update_combo_line(
    combo: Res<crate::combo::Combo>,
    mut combo_q: Query<&mut Text, With<HudCombo>>,
) {
    if let Ok(mut t) = combo_q.single_mut() {
        if combo.active() {
            t.0 = format!("COMBO x{} (keep serving <15s apart! up to +50)", combo.streak);
        } else {
            t.0 = String::new();
        }
    }
}

// ---------- pause ----------

pub fn setup_paused(mut commands: Commands) {
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(10.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.05, 0.05, 0.08, 0.85)),
        StateScoped(GameState::Paused),
        children![
            title_text("Paused", 44.0, Color::WHITE),
            (
                PauseBtn::Resume,
                Button,
                Node {
                    width: Val::Px(300.0),
                    height: Val::Px(54.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.16, 0.5, 0.26)),
                BorderColor(Color::WHITE),
                children![title_text("Resume (Esc)", 22.0, Color::WHITE)],
            ),
            (
                PauseBtn::Restart,
                Button,
                Node {
                    width: Val::Px(300.0),
                    height: Val::Px(54.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.5, 0.4, 0.12)),
                BorderColor(Color::WHITE),
                children![title_text("Restart level", 22.0, Color::WHITE)],
            ),
            (
                PauseBtn::Menu,
                Button,
                Node {
                    width: Val::Px(300.0),
                    height: Val::Px(54.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.4, 0.15, 0.15)),
                BorderColor(Color::WHITE),
                children![title_text("Quit to menu", 22.0, Color::WHITE)],
            ),
        ],
    ));
}

pub fn paused_buttons(
    mut commands: Commands,
    assets: Res<GameAssets>,
    muted: Res<Muted>,
    mut q: Query<(&Interaction, &PauseBtn), (Changed<Interaction>, With<Button>)>,
    mut next: ResMut<NextState<GameState>>,
) {
    for (inter, btn) in q.iter_mut() {
        if *inter != Interaction::Pressed {
            continue;
        }
        click(&mut commands, &assets, &muted);
        match btn {
            PauseBtn::Resume => next.set(GameState::Playing),
            PauseBtn::Restart => next.set(GameState::LevelBrief),
            PauseBtn::Menu => next.set(GameState::MainMenu),
        }
    }
}

// ---------- level complete ----------

pub fn plural(n: u32, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

pub fn setup_complete(
    mut commands: Commands,
    assets: Res<AssetServer>,
    selected: Res<SelectedLevel>,
    score: Res<RunScore>,
    best: Res<Best>,
    top: Res<crate::leaderboard::TopScores>,
) {
    let i = selected.0.min(LEVELS.len() - 1);
    let cfg = &LEVELS[i];
    let stars = crate::config::stars_for(score.points, cfg.star_thresholds);
    let star_str = match stars {
        3 => "***",
        2 => "**.",
        1 => "*..",
        _ => "... (failed - need 1 star to unlock next)",
    };
    let info = format!(
        "{} complete!\nStars: {}  Score: {} (best {})\nServed: {}  Expired: {}  Trashed: {}\nThresholds: 1>{} 2>{} 3>{}",
        cfg.restaurant,
        star_str,
        score.points,
        best.scores[i],
        score.served,
        score.expired,
        score.trashed,
        cfg.star_thresholds.0,
        cfg.star_thresholds.1,
        cfg.star_thresholds.2
    );
    let has_next = i + 1 < LEVELS.len() && stars >= 1;
    let next_label = if has_next {
        format!("Next: {} >", LEVELS[i + 1].restaurant)
    } else if i + 1 < LEVELS.len() {
        "Next (locked: earn 1 star)".to_string()
    } else {
        "Finish! (see results)".to_string()
    };
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(10.0),
            ..default()
        },
        BackgroundColor(Color::srgb(0.09, 0.12, 0.16)),
        StateScoped(GameState::LevelComplete),
        children![
            backdrop(&assets, "results_bg.png", 0.5),
            title_text("Level Complete", 44.0, Color::srgb(1.0, 0.82, 0.35)),
            (
                Text::new(info),
                TextFont { font_size: 22.0, ..default() },
                TextColor(Color::WHITE),
            ),
            (
                TopBoardText,
                Text::new(initial_board_text(&top)),
                TextFont { font_size: 18.0, ..default() },
                TextColor(Color::srgb(1.0, 0.9, 0.55)),
            ),
            (
                CompleteBtn::Next,
                Button,
                Node {
                    width: Val::Px(360.0),
                    height: Val::Px(54.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.16, 0.5, 0.26)),
                BorderColor(Color::WHITE),
                children![(Text::new(next_label), TextFont { font_size: 20.0, ..default() }, TextColor(Color::WHITE))],
            ),
            (
                CompleteBtn::Replay,
                Button,
                Node {
                    width: Val::Px(360.0),
                    height: Val::Px(50.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.5, 0.4, 0.12)),
                BorderColor(Color::WHITE),
                children![title_text("Replay", 20.0, Color::WHITE)],
            ),
            (
                CompleteBtn::Shop,
                Button,
                Node {
                    width: Val::Px(360.0),
                    height: Val::Px(50.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.5, 0.38, 0.1)),
                BorderColor(Color::WHITE),
                children![title_text("Shop (spend coins)", 20.0, Color::WHITE)],
            ),
            (
                CompleteBtn::Menu,
                Button,
                Node {
                    width: Val::Px(360.0),
                    height: Val::Px(50.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.35, 0.2, 0.2)),
                BorderColor(Color::WHITE),
                children![title_text("Menu", 20.0, Color::WHITE)],
            ),
        ],
    ));
}

pub fn complete_buttons(
    mut commands: Commands,
    assets: Res<GameAssets>,
    muted: Res<Muted>,
    mut q: Query<(&Interaction, &CompleteBtn), (Changed<Interaction>, With<Button>)>,
    mut next: ResMut<NextState<GameState>>,
    mut selected: ResMut<SelectedLevel>,
    score: Res<RunScore>,
    mut shop_return: ResMut<ShopReturn>,
) {
    for (inter, btn) in q.iter_mut() {
        if *inter != Interaction::Pressed {
            continue;
        }
        click(&mut commands, &assets, &muted);
        match btn {
            CompleteBtn::Menu => next.set(GameState::MainMenu),
            CompleteBtn::Replay => next.set(GameState::LevelBrief),
            CompleteBtn::Shop => {
                shop_return.0 = GameState::LevelComplete;
                next.set(GameState::Shop);
            }
            CompleteBtn::Next => {
                let i = selected.0.min(LEVELS.len() - 1);
                let cfg = &LEVELS[i];
                let stars = crate::config::stars_for(score.points, cfg.star_thresholds);
                if i + 1 >= LEVELS.len() {
                    next.set(GameState::GameComplete);
                } else if stars >= 1 {
                    selected.0 = i + 1;
                    next.set(GameState::LevelBrief);
                }
            }
        }
    }
}

// ---------- game complete ----------

pub fn setup_gamecomplete(mut commands: Commands, assets: Res<AssetServer>, best: Res<Best>) {
    let total_stars: u32 = best.stars.iter().map(|s| *s as u32).sum();
    let total_score: u32 = best.scores.iter().sum();
    let mut rows = String::new();
    for (i, cfg) in LEVELS.iter().enumerate() {
        rows.push_str(&format!("- {}: {} stars, {} pts\n", cfg.restaurant, best.stars[i], best.scores[i]));
    }
    let info = format!("Total: {total_stars}/9 stars, {total_score} pts\n{rows}");
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(10.0),
            ..default()
        },
        BackgroundColor(Color::srgb(0.09, 0.12, 0.16)),
        StateScoped(GameState::GameComplete),
        children![
            backdrop(&assets, "results_bg.png", 0.5),
            title_text("You finished all restaurants!", 40.0, Color::srgb(1.0, 0.82, 0.35)),
            (
                Text::new(info),
                TextFont { font_size: 22.0, ..default() },
                TextColor(Color::WHITE),
            ),
            (
                CompleteBtn::Menu,
                Button,
                Node {
                    width: Val::Px(360.0),
                    height: Val::Px(54.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.16, 0.5, 0.26)),
                BorderColor(Color::WHITE),
                children![title_text("Back to menu", 22.0, Color::WHITE)],
            ),
        ],
    ));
}

pub fn gamecomplete_buttons(
    mut commands: Commands,
    assets: Res<GameAssets>,
    muted: Res<Muted>,
    mut q: Query<(&Interaction, &CompleteBtn), (Changed<Interaction>, With<Button>)>,
    mut next: ResMut<NextState<GameState>>,
) {
    for (inter, btn) in q.iter_mut() {
        if *inter != Interaction::Pressed {
            continue;
        }
        click(&mut commands, &assets, &muted);
        if matches!(btn, CompleteBtn::Menu) {
            next.set(GameState::MainMenu);
        }
    }
}

pub fn _use_recipe_id() {
    let _ = RecipeId::ClassicBurger;
}
