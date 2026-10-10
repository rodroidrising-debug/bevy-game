//! Real cooking minigames (Step 5): rhythm chopping, active stirring, burn rescue.
//!
//! - Chop: timing bar ping-pongs 0..1; press E when the marker is centered.
//!   Perfect (|pos-0.5|<0.08) counts double, Good (<0.18) single. 3 hits done.
//! - Stir: heat 0..1 rises ~0.22/s while frying; E stirs (-0.28). Heat 1.0 = instant burn.
//! - Rescue: R near a burning-done pan (once per pan) restarts the burn clock.

use bevy::prelude::*;

use crate::assets::{play_one, GameAssets};
use crate::ui::HudMini;
use crate::world::{Hint, Player, RunScore, Station};

/// Rhythm chopping state on a CuttingBoard station entity.
#[derive(Component, Debug)]
pub struct ChopGame {
    pub pos: f32,
    pub dir: f32,
    pub speed: f32,
    pub hits: u32,
    pub needed: u32,
    pub perfects: u32,
}

impl ChopGame {
    pub fn new(needed: u32) -> Self {
        Self { pos: 0.0, dir: 1.0, speed: 1.1, hits: 0, needed: needed.max(1), perfects: 0 }
    }
    pub fn finished(&self) -> bool {
        self.hits >= self.needed
    }
}

/// Active-fry state on a Stove station entity.
#[derive(Component, Debug)]
pub struct StirGame {
    pub heat: f32,
    pub rescues_used: u8,
}

impl StirGame {
    pub fn new() -> Self {
        Self { heat: 0.25, rescues_used: 0 }
    }
}

pub const CHOP_PERFECT_R: f32 = 0.08;
pub const CHOP_GOOD_R: f32 = 0.18;
pub const STIR_HEAT_RATE: f32 = 0.22;
pub const STIR_COOL: f32 = 0.28;

/// 2 = perfect, 1 = good, 0 = miss.
pub fn chop_hit_quality(pos: f32) -> u8 {
    let d = (pos - 0.5).abs();
    if d < CHOP_PERFECT_R {
        2
    } else if d < CHOP_GOOD_R {
        1
    } else {
        0
    }
}

/// Text timing bar, 21 cells, center zone marked.
pub fn render_chop_bar(pos: f32) -> String {
    let n = 21usize;
    let mut s = String::with_capacity(n + 16);
    s.push('[');
    let idx = (pos.clamp(0.0, 1.0) * (n as f32 - 1.0)).round() as usize;
    for i in 0..n {
        let c = if i == idx {
            'O'
        } else if (i as i32 - 10).abs() <= 1 {
            '#'
        } else if (i as i32 - 10).abs() <= 3 {
            '-'
        } else {
            '.'
        };
        s.push(c);
    }
    s.push_str("] E=HIT");
    s
}

pub fn render_heat_bar(heat: f32) -> String {
    let n = 16usize;
    let filled = (heat.clamp(0.0, 1.0) * n as f32).round() as usize;
    let mut s = String::from("HEAT [");
    for i in 0..n {
        s.push(if i < filled { '!' } else { '.' });
    }
    s.push_str("] E=STIR");
    s
}

/// Advance all chop markers (ping-pong 0..1).
pub fn chop_marker_tick(time: Res<Time>, mut q: Query<&mut ChopGame>) {
    let dt = time.delta_secs();
    for mut g in q.iter_mut() {
        g.pos += g.dir * g.speed * dt;
        if g.pos >= 1.0 {
            g.pos = 1.0;
            g.dir = -1.0;
        } else if g.pos <= 0.0 {
            g.pos = 0.0;
            g.dir = 1.0;
        }
    }
}

/// Heat rises on every active fry or bake; at 1.0 it flash-burns.
pub fn stir_heat_tick(
    time: Res<Time>,
    economy: Res<crate::economy::Economy>,
    mut stations: Query<(Entity, &mut Station)>,
    mut stirs: Query<&mut StirGame>,
) {
    let dt = time.delta_secs();
    let rate = STIR_HEAT_RATE * economy.upgrades.heat_rate_mult();
    for (e, mut st) in stations.iter_mut() {
        let Ok(mut g) = stirs.get_mut(e) else {
            continue;
        };
        let Some(pr) = st.processing.as_mut() else {
            continue;
        };
        if pr.kind != crate::world::ProcKind::Cook && pr.kind != crate::world::ProcKind::Bake {
            continue;
        }
        // Only heat while raw cooking (not the burn-watch phase).
        if pr.elapsed >= pr.total {
            continue;
        }
        g.heat += rate * dt;
        if g.heat >= 1.0 {
            g.heat = 1.0;
            // Flash-burn: jump past cook + burn window.
            let burn = pr.burn_after.unwrap_or(5.0);
            pr.elapsed = pr.total + burn + 0.01;
        }
    }
}

/// E at a chopping board: judge timing. Returns true (always consumes E).
/// On completion sets processing.elapsed = total so the normal finish path fires.
pub fn attempt_chop(
    commands: &mut Commands,
    assets: &GameAssets,
    muted: bool,
    game: &mut ChopGame,
    station_entity: Entity,
    st: &mut Station,
    hint: &mut Hint,
    score: &mut RunScore,
    sfx: f32,
) -> bool {
    let q = chop_hit_quality(game.pos);
    match q {
        2 => {
            game.hits += 2;
            game.perfects += 1;
            hint.set("PERFECT cut! (2)");
            if !muted {
                play_one(commands, assets.snd_chop.clone(), 1.0 * sfx);
                play_one(commands, assets.snd_ding.clone(), 0.4 * sfx);
            }
        }
        1 => {
            game.hits += 1;
            hint.set(format!("Good chop ({}/{})", game.hits.min(game.needed), game.needed));
            if !muted {
                play_one(commands, assets.snd_chop.clone(), 0.9 * sfx);
            }
        }
        _ => {
            hint.set("Miss! Watch the marker — hit when O is centered on ###.");
            if !muted {
                play_one(commands, assets.snd_plate.clone(), 0.4 * sfx);
            }
        }
    }
    if game.finished() {
        if let Some(pr) = st.processing.as_mut() {
            pr.elapsed = pr.total; // normal finish path dings + slots result
        }
        if game.perfects >= 2 {
            score.points += 5;
            hint.set("Dish prepped with PERFECT cuts! +5");
        }
        commands.entity(station_entity).remove::<ChopGame>();
    }
    true
}

/// E at a frying pan: stir (cool heat). Always consumes E.
pub fn attempt_stir(
    commands: &mut Commands,
    assets: &GameAssets,
    muted: bool,
    game: &mut StirGame,
    cool: f32,
    hint: &mut Hint,
    sfx: f32,
) -> bool {
    game.heat = (game.heat - cool).max(0.0);
    hint.set(format!("Stirred! {}", render_heat_bar(game.heat)));
    if !muted {
        play_one(commands, assets.snd_sizzle.clone(), 0.5 * sfx);
    }
    true
}

/// R near a done-but-burning pan: one rescue per pan restarts burn clock.
pub fn attempt_rescue(
    game: &mut StirGame,
    st: &mut Station,
    hint: &mut Hint,
) -> bool {
    let Some(pr) = st.processing.as_mut() else {
        return false;
    };
    if pr.burn_after.is_none() || pr.elapsed < pr.total {
        return false; // nothing to rescue yet
    }
    if game.rescues_used >= 1 {
        hint.set("Already flipped once — serve it or lose it!");
        return true;
    }
    game.rescues_used += 1;
    game.heat = 0.4;
    pr.elapsed = pr.total + 0.001; // restart burn countdown
    // Clear the "dinged" flag so a re-ding doesn't spam: mark watcher fresh.
    pr.chop_sfx_timer = -1.0;
    hint.set("FLIPPED! Burn clock restarted (one save per pan).");
    true
}

/// HUD line for the nearest active minigame, or empty.
pub fn minigame_line(
    player_pos: Vec3,
    stations: &Query<(Entity, &Transform, &Station)>,
    chops: &Query<&ChopGame>,
    stirs: &Query<&StirGame>,
) -> String {
    let mut best: Option<(Entity, f32)> = None;
    for (e, t, st) in stations.iter() {
        if st.processing.is_none() {
            continue;
        }
        let d = player_pos.distance(t.translation);
        if d <= 2.6 && best.map_or(true, |(_, bd)| d < bd) {
            best = Some((e, d));
        }
    }
    let Some((e, _)) = best else {
        return String::new();
    };
    if let Ok(g) = chops.get(e) {
        return format!("CHOP {}/{} {}", g.hits.min(g.needed), g.needed, render_chop_bar(g.pos));
    }
    if let Ok(g) = stirs.get(e) {
        if g.heat > 0.75 {
            return format!("{} !!STIR NOW!! [R=flip if burning]", render_heat_bar(g.heat));
        }
        return render_heat_bar(g.heat);
    }
    String::new()
}

pub fn update_mini_hud(
    player_q: Query<&Transform, With<Player>>,
    stations: Query<(Entity, &Transform, &Station)>,
    chops: Query<&ChopGame>,
    stirs: Query<&StirGame>,
    mut mini_q: Query<&mut Text, With<HudMini>>,
) {
    if let (Ok(p), Ok(mut t)) = (player_q.single(), mini_q.single_mut()) {
        t.0 = minigame_line(p.translation, &stations, &chops, &stirs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timing_grades() {
        assert_eq!(chop_hit_quality(0.5), 2);
        assert_eq!(chop_hit_quality(0.56), 2);
        assert_eq!(chop_hit_quality(0.60), 1);
        assert_eq!(chop_hit_quality(0.1), 0);
        assert_eq!(chop_hit_quality(0.9), 0);
    }

    #[test]
    fn bar_renders_marker_and_zone() {
        let s = render_chop_bar(0.5);
        assert!(s.contains('O'));
        assert!(s.contains("###") || s.contains('#'));
        assert!(s.contains("E=HIT"));
        let h = render_heat_bar(0.5);
        assert!(h.contains("HEAT"));
        assert!(h.contains("STIR"));
    }

    #[test]
    fn chop_game_finishes() {
        let mut g = ChopGame::new(3);
        assert!(!g.finished());
        g.hits = 2;
        assert!(!g.finished());
        g.hits = 3;
        assert!(g.finished());
    }
}
