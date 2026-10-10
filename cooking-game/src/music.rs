//! Background music (Step 7): Kevin MacLeod tracks, CC-BY 4.0, see README credits.
//! Menu = Lobby Time, gameplay = Monkeys Spinning Monkeys, results = Happy Boy End Theme.

use bevy::audio::Volume;
use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::settings::Settings;
use crate::world::Muted;
use crate::GameState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicKind {
    Menu,
    Game,
    Win,
}

#[derive(Component)]
pub struct NowPlaying(pub MusicKind);

#[derive(Resource, Default)]
pub struct MusicState {
    pub current: Option<MusicKind>,
    pub vol: f32,
}

fn desired_kind(state: &GameState) -> MusicKind {
    match state {
        GameState::Playing | GameState::Paused => MusicKind::Game,
        GameState::LevelComplete | GameState::GameComplete => MusicKind::Win,
        _ => MusicKind::Menu,
    }
}

fn handle_for(kind: MusicKind, assets: &GameAssets) -> Handle<AudioSource> {
    match kind {
        MusicKind::Menu => assets.mus_menu.clone(),
        MusicKind::Game => assets.mus_game.clone(),
        MusicKind::Win => assets.mus_win.clone(),
    }
}

/// Keep the right loop playing at the right volume. Respawn-on-change only.
pub fn music_tick(
    mut commands: Commands,
    assets: Option<Res<GameAssets>>,
    settings: Res<Settings>,
    muted: Res<Muted>,
    state: Res<State<GameState>>,
    mut music: ResMut<MusicState>,
    playing: Query<Entity, With<NowPlaying>>,
) {
    let Some(assets) = assets.as_ref() else {
        return;
    };
    let want = desired_kind(state.get());
    let eff_vol = if muted.0 { 0.0 } else { settings.master * settings.music * 0.5 };
    let vol_changed = (music.vol - eff_vol).abs() > 0.005;
    if music.current == Some(want) && !vol_changed {
        return;
    }
    for e in playing.iter() {
        commands.entity(e).despawn();
    }
    if eff_vol > 0.001 {
        commands.spawn((
            NowPlaying(want),
            AudioPlayer(handle_for(want, &assets)),
            PlaybackSettings::LOOP.with_volume(Volume::Linear(eff_vol)),
        ));
    }
    music.current = Some(want);
    music.vol = eff_vol;
}

/// Master slider + mute -> GlobalVolume (scales every sound).
pub fn global_volume_tick(
    settings: Res<Settings>,
    muted: Res<Muted>,
    mut global: ResMut<bevy::audio::GlobalVolume>,
) {
    if settings.is_changed() || muted.is_changed() {
        let v = if muted.0 { 0.0 } else { settings.master };
        global.volume = Volume::Linear(v);
    }
}
