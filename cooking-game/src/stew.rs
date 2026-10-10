//! Stew-pot subsystem (Step 2: Stew House).
//! Lifecycle: Empty -> add veg (boil 8s, boil sfx) -> Done (ding)
//! -> ladle with BowlClean -> Dirty -> wash (E empty) -> Empty.
//! Neglect Done 6s -> Burned (whistle) -> dump (Q) -> Dirty -> wash.

use bevy::prelude::*;

use crate::assets::{play_one, spawn_model, GameAssets};
use crate::config::{match_recipe, Item, RecipeId};
use crate::world::{Hands, Held, Hint, InKitchen, RunScore};
use crate::GameState;

pub const BOIL_TOTAL_S: f32 = 8.0;
pub const BURN_EXTRA_S: f32 = 6.0;
pub const POT_EMPTY_MODEL: &str = "pot_A.gltf";
pub const POT_STEW_MODEL: &str = "pot_A_stew.gltf";
pub const POT_VISUAL_OFFSET: Vec3 = Vec3::new(0.0, 0.95, 0.0);
pub const HEARTY_STEW_CONTENTS: [Item; 3] = [
    Item::CarrotChopped,
    Item::PotatoChopped,
    Item::OnionChopped,
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PotBoil {
    pub elapsed: f32,
    pub total: f32,
}

impl PotBoil {
    pub fn new() -> Self {
        Self { elapsed: 0.0, total: BOIL_TOTAL_S }
    }
    pub fn new_with(total: f32) -> Self {
        Self { elapsed: 0.0, total: total.max(1.0) }
    }
    pub fn fraction(&self) -> f32 {
        if self.total <= 0.0 {
            1.0
        } else {
            (self.elapsed / self.total).clamp(0.0, 1.0)
        }
    }
    pub fn is_done(&self) -> bool {
        self.elapsed >= self.total
    }
}

impl Default for PotBoil {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Component, Debug, Clone)]
pub struct PotComponent {
    pub contents: Vec<Item>,
    pub boiling: Option<PotBoil>,
    pub burned: bool,
    pub burn_elapsed: f32,
    pub dirty: bool,
    pub visual: Option<Entity>,
    pub visual_dirty: bool,
    pub boil_sfx_played: bool,
    pub done_ding_played: bool,
}

impl Default for PotComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl PotComponent {
    pub fn new() -> Self {
        Self {
            contents: Vec::new(),
            boiling: None,
            burned: false,
            burn_elapsed: 0.0,
            dirty: false,
            visual: None,
            visual_dirty: true,
            boil_sfx_played: false,
            done_ding_played: false,
        }
    }
    pub fn is_empty_clean(&self) -> bool {
        self.contents.is_empty() && self.boiling.is_none() && !self.burned && !self.dirty
    }
    pub fn is_cooking(&self) -> bool {
        matches!(&self.boiling, Some(b) if !b.is_done()) && !self.burned && !self.dirty
    }
    pub fn is_done(&self) -> bool {
        matches!(&self.boiling, Some(b) if b.is_done()) && !self.burned && !self.dirty
    }
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
    pub fn boil_remaining(&self) -> f32 {
        match &self.boiling {
            None => 0.0,
            Some(b) => (b.total - b.elapsed).max(0.0),
        }
    }
    pub fn burn_remaining(&self) -> f32 {
        if !self.is_done() {
            return BURN_EXTRA_S;
        }
        (BURN_EXTRA_S - self.burn_elapsed).max(0.0)
    }
    pub fn has_complete_stew_mix(&self) -> bool {
        match_recipe(&self.contents) == Some(RecipeId::HeartyStew)
    }
    fn mark_visual_dirty(&mut self) {
        self.visual_dirty = true;
    }
    fn reset_boil_state(&mut self) {
        self.boiling = None;
        self.burn_elapsed = 0.0;
        self.boil_sfx_played = false;
        self.done_ding_played = false;
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PotVisual {
    pub pot: Entity,
}

pub fn is_stew_veg(item: Item) -> bool {
    matches!(
        item,
        Item::CarrotChopped | Item::PotatoChopped | Item::OnionChopped
    )
}

pub fn is_clean_bowl(item: Item) -> bool {
    item == Item::BowlClean
}

pub fn ladled_stew_plate() -> Vec<Item> {
    HEARTY_STEW_CONTENTS.to_vec()
}

pub fn contents_label(contents: &[Item]) -> String {
    if contents.is_empty() {
        "empty".to_string()
    } else {
        contents.iter().map(|i| i.display_name()).collect::<Vec<_>>().join(" + ")
    }
}

pub fn pot_prompt(pot: &PotComponent) -> String {
    if pot.burned {
        return format!("Pot: BURNED! [{}] — Q to dump, then E empty to wash.", contents_label(&pot.contents));
    }
    if pot.dirty {
        return "Pot: dirty — E empty-handed to wash.".to_string();
    }
    match &pot.boiling {
        None => {
            if pot.contents.is_empty() {
                "Pot: empty — bring Chopped Carrot / Potato / Onion [E].".to_string()
            } else {
                format!("Pot: waiting [{}] — add more [E] or dump [Q].", contents_label(&pot.contents))
            }
        }
        Some(b) if !b.is_done() => {
            format!("Pot: boiling {}% ({}s) [{}].", (b.fraction() * 100.0) as i32, pot.boil_remaining().ceil() as i32, contents_label(&pot.contents))
        }
        Some(_) => {
            if pot.has_complete_stew_mix() {
                format!("Pot: STEW READY! [{:.0}s to burn] — Clean Bowl [E] to ladle.", pot.burn_remaining())
            } else {
                format!("Pot: boiled [{}] incomplete — add missing veg or Q dumps.", contents_label(&pot.contents))
            }
        }
    }
}

pub fn spawn_pot_at(
    commands: &mut Commands,
    assets: &GameAssets,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    pos: Vec3,
) -> Entity {
    // Mesh first so the logic entity can store its handle directly (no deferred queue).
    let mesh = spawn_model(
        commands,
        assets,
        meshes,
        materials,
        POT_EMPTY_MODEL,
        pos + POT_VISUAL_OFFSET,
        0.9,
        Color::srgb(0.55, 0.55, 0.6),
    );
    let pot = commands
        .spawn((
            PotComponent { visual: Some(mesh), visual_dirty: false, ..PotComponent::new() },
            Transform::from_translation(pos),
            Visibility::default(),
            InKitchen,
        ))
        .id();
    commands.entity(mesh).insert(PotVisual { pot }).insert(InKitchen);
    pot
}

pub fn spawn_stew_pots(
    commands: &mut Commands,
    assets: &GameAssets,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    level: usize,
    stove_positions: &[Vec3],
) -> Vec<Entity> {
    if level < 1 {
        return Vec::new();
    }
    stove_positions.iter().take(2).map(|pos| spawn_pot_at(commands, assets, meshes, materials, *pos)).collect()
}

pub fn add_ingredient_to_pot(
    commands: &mut Commands,
    assets: &GameAssets,
    muted: bool,
    sfx: f32,
    hands: &mut Hands,
    pot: &mut PotComponent,
    hint: &mut Hint,
    boil_total: f32,
) -> bool {
    if pot.dirty {
        hint.set("Pot is dirty — E empty-handed to wash first.");
        return true;
    }
    if pot.burned {
        hint.set("Pot burned! Q to dump, then E empty to wash.");
        return true;
    }
    if let Some(b) = &pot.boiling {
        if b.is_done() && pot.has_complete_stew_mix() {
            hint.set("Stew done! Bring a Clean Bowl (E) — don't add more.");
            return true;
        }
    }
    let Some(held) = hands.held.take() else {
        return false;
    };
    match held {
        Held::Single(item) if is_stew_veg(item) => {
            if pot.contents.len() >= 6 {
                hands.held = Some(Held::Single(item));
                hint.set("Pot full! Ladle (bowl) or dump (Q).");
                return true;
            }
            let was_done = matches!(&pot.boiling, Some(b) if b.is_done());
            pot.contents.push(item);
            pot.mark_visual_dirty();
            if pot.boiling.is_none() {
                pot.boiling = Some(PotBoil::new_with(boil_total));
            }
            if was_done {
                pot.burn_elapsed = (pot.burn_elapsed - 2.0).max(0.0);
                pot.done_ding_played = true;
            }
            if !pot.boil_sfx_played {
                pot.boil_sfx_played = true;
                if !muted {
                    play_one(commands, assets.snd_boil.clone(), 0.9 * sfx);
                }
            }
            if !muted {
                play_one(commands, assets.snd_plate.clone(), 0.6 * sfx);
            }
            if pot.has_complete_stew_mix() {
                hint.set(format!("Added {} — mix complete! Boil {:.0}s left.", item.display_name(), pot.boil_remaining()));
            } else {
                hint.set(format!("Added {} [{}]. Need carrot+potato+onion.", item.display_name(), contents_label(&pot.contents)));
            }
            true
        }
        Held::Single(item) => {
            hands.held = Some(Held::Single(item));
            hint.set(format!("{} doesn't go in stew. Chop carrot/potato/onion.", item.display_name()));
            true
        }
        other => {
            hands.held = Some(other);
            hint.set("Pot takes single chopped veg only.");
            true
        }
    }
}

pub fn try_ladle(
    commands: &mut Commands,
    assets: &GameAssets,
    muted: bool,
    sfx: f32,
    hands: &mut Hands,
    pot: &mut PotComponent,
    hint: &mut Hint,
) -> bool {
    if !matches!(hands.held, Some(Held::Single(i)) if is_clean_bowl(i)) {
        return false;
    }
    if pot.dirty {
        hint.set("Pot dirty — wash (E empty) first.");
        return true;
    }
    if pot.burned {
        hint.set("Burned! Dump (Q), then wash (E).");
        return true;
    }
    let Some(boil) = &pot.boiling else {
        hint.set("Pot empty — add chopped carrot+potato+onion.");
        return true;
    };
    if !boil.is_done() {
        hint.set(format!("Still boiling {:.0}s. Wait for ding!", pot.boil_remaining().ceil()));
        return true;
    }
    if !pot.has_complete_stew_mix() {
        hint.set(format!("Incomplete [{}] — add missing veg or Q dumps.", contents_label(&pot.contents)));
        return true;
    }
    hands.held = None;
    let plate = ladled_stew_plate();
    debug_assert_eq!(match_recipe(&plate), Some(RecipeId::HeartyStew));
    hands.held = Some(Held::Plate(plate));
    pot.contents.clear();
    pot.reset_boil_state();
    pot.burned = false;
    pot.dirty = true;
    pot.mark_visual_dirty();
    hint.set("Ladled Hearty Stew! Serve at Window. Pot dirty — E empty to wash.");
    if !muted {
        play_one(commands, assets.snd_plate.clone(), 0.9 * sfx);
        play_one(commands, assets.snd_ding.clone(), 0.6 * sfx);
    }
    true
}

pub fn empty_pot(
    commands: &mut Commands,
    assets: &GameAssets,
    muted: bool,
    sfx: f32,
    pot: &mut PotComponent,
    score: &mut RunScore,
    hint: &mut Hint,
) -> bool {
    let had_something = !pot.contents.is_empty() || pot.boiling.is_some() || pot.burned || pot.dirty;
    if !had_something {
        hint.set("Pot already empty.");
        return true;
    }
    if pot.dirty && pot.contents.is_empty() && pot.boiling.is_none() && !pot.burned {
        hint.set("Pot dirty — wash (E empty), nothing to dump.");
        return true;
    }
    let had_food = !pot.contents.is_empty();
    let was_done_or_burned = pot.burned || matches!(&pot.boiling, Some(b) if b.is_done());
    pot.contents.clear();
    pot.reset_boil_state();
    pot.burned = false;
    pot.dirty = was_done_or_burned;
    pot.mark_visual_dirty();
    score.trashed += 1;
    if had_food {
        score.points = score.points.saturating_sub(10);
    }
    hint.set(if pot.dirty { "Dumped. (-10) Dirty — E empty to wash." } else { "Dumped. (-10 if food)" });
    if !muted {
        play_one(commands, assets.snd_plate.clone(), 0.5 * sfx);
    }
    true
}

pub fn try_wash_pot(
    commands: &mut Commands,
    assets: &GameAssets,
    muted: bool,
    sfx: f32,
    hands: &Hands,
    pot: &mut PotComponent,
    hint: &mut Hint,
) -> bool {
    if hands.held.is_some() {
        return false;
    }
    if pot.burned && (!pot.contents.is_empty() || pot.boiling.is_some()) {
        hint.set("Burned! Dump (Q) first, then wash (E).");
        return true;
    }
    if !pot.dirty {
        return false;
    }
    pot.contents.clear();
    pot.reset_boil_state();
    pot.burned = false;
    pot.dirty = false;
    pot.mark_visual_dirty();
    hint.set("Scrubbed pot clean! Ready for veg.");
    if !muted {
        play_one(commands, assets.snd_boil.clone(), 0.4 * sfx);
        play_one(commands, assets.snd_plate.clone(), 0.4 * sfx);
    }
    true
}

/// Nearest pot within 2.4m claims E. Returns true if consumed.
pub fn handle_stove_interact(
    commands: &mut Commands,
    assets: &GameAssets,
    muted: bool,
    sfx: f32,
    hands: &mut Hands,
    pots: &mut Query<(Entity, &Transform, &mut PotComponent)>,
    player_pos: Vec3,
    hint: &mut Hint,
    boil_total: f32,
) -> bool {
    let mut best: Option<Entity> = None;
    let mut best_d = f32::MAX;
    for (e, t, _) in pots.iter() {
        let d = player_pos.distance(t.translation);
        if d <= 2.4 && d < best_d {
            best_d = d;
            best = Some(e);
        }
    }
    let Some(pe) = best else { return false };
    let Ok((_e, _t, mut pot)) = pots.get_mut(pe) else { return false };
    if hands.held.is_none() {
        if try_wash_pot(commands, assets, muted, sfx, hands, &mut pot, hint) {
            return true;
        }
        hint.set(pot_prompt(&pot));
        return true;
    }
    if matches!(hands.held, Some(Held::Single(i)) if is_clean_bowl(i)) {
        try_ladle(commands, assets, muted, sfx, hands, &mut pot, hint);
        return true;
    }
    if matches!(hands.held, Some(Held::Single(i)) if is_stew_veg(i)) {
        add_ingredient_to_pot(commands, assets, muted, sfx, hands, &mut pot, hint, boil_total);
        return true;
    }
    hint.set(format!("Stew pot [{}]: chopped carrot/potato/onion, or bowl when ready.", contents_label(&pot.contents)));
    true
}

/// Q near pot dumps it. Returns true if consumed.
pub fn handle_pot_trash_key(
    commands: &mut Commands,
    assets: &GameAssets,
    muted: bool,
    sfx: f32,
    pots: &mut Query<(Entity, &Transform, &mut PotComponent)>,
    player_pos: Vec3,
    score: &mut RunScore,
    hint: &mut Hint,
) -> bool {
    let mut best: Option<Entity> = None;
    let mut best_d = f32::MAX;
    for (e, t, _) in pots.iter() {
        let d = player_pos.distance(t.translation);
        if d <= 2.4 && d < best_d {
            best_d = d;
            best = Some(e);
        }
    }
    let Some(pe) = best else { return false };
    let Ok((_e, _t, mut pot)) = pots.get_mut(pe) else { return false };
    // Only claim Q if the pot actually has something to dump/wash.
    if pot.is_empty_clean() {
        return false;
    }
    empty_pot(commands, assets, muted, sfx, &mut pot, score, hint)
}

pub fn tick_pots(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<GameAssets>,
    muted: Res<crate::world::Muted>,
    settings: Res<crate::settings::Settings>,
    mut pots: Query<&mut PotComponent>,
    mut hint: ResMut<Hint>,
) {
    let dt = time.delta_secs();
    let sfx = settings.sfx;
    for mut pot in pots.iter_mut() {
        if pot.dirty || pot.burned {
            continue;
        }
        let Some(boil) = pot.boiling.as_mut() else { continue };
        if boil.is_done() {
            pot.burn_elapsed += dt;
            if pot.burn_elapsed >= BURN_EXTRA_S {
                pot.burned = true;
                pot.mark_visual_dirty();
                hint.set("Stew BURNED! Q dumps, E empty washes.");
                if !muted.0 {
                    play_one(&mut commands, assets.snd_whistle.clone(), 0.9 * sfx);
                }
            }
            continue;
        }
        boil.elapsed += dt;
        if boil.is_done() {
            boil.elapsed = boil.total;
            pot.mark_visual_dirty();
            if !pot.done_ding_played {
                pot.done_ding_played = true;
                if pot.has_complete_stew_mix() {
                    hint.set("Stew READY! Clean Bowl + E to ladle.");
                } else {
                    hint.set(format!("Boiled [{}] incomplete — add missing veg or burns!", contents_label(&pot.contents)));
                }
                if !muted.0 {
                    play_one(&mut commands, assets.snd_ding.clone(), 0.9 * sfx);
                }
            }
        }
    }
}

pub fn sync_pot_visuals(
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    pot_q: Query<(Entity, &Transform, &mut PotComponent), Without<PotVisual>>,
    visual_q: Query<(Entity, &PotVisual)>,
) {
    use std::collections::{HashMap, HashSet};
    let mut visual_of: HashMap<Entity, Entity> = HashMap::new();
    let mut live_visuals: HashSet<Entity> = HashSet::new();
    for (ve, pv) in visual_q.iter() {
        visual_of.insert(pv.pot, ve);
        live_visuals.insert(ve);
    }
    for (pot_e, pot_t, mut pot) in pot_q.into_iter() {
        if !pot.visual_dirty {
            // Repair stale handle if visual was cleaned up.
            match pot.visual {
                Some(ve) if live_visuals.contains(&ve) => continue,
                Some(_) => { pot.visual = visual_of.get(&pot_e).copied(); }
                None => { pot.visual = visual_of.get(&pot_e).copied(); }
            }
            if pot.visual.is_some() && !pot.visual_dirty {
                continue;
            }
        }
        if let Some(old) = pot.visual.take() {
            if live_visuals.contains(&old) {
                commands.entity(old).despawn();
            }
        } else if let Some(mapped) = visual_of.get(&pot_e) {
            if live_visuals.contains(mapped) {
                commands.entity(*mapped).despawn();
            }
        }
        let done_ready = pot.is_done() && pot.has_complete_stew_mix();
        let (file, scale, tint) = if pot.burned {
            (POT_EMPTY_MODEL, 0.9, Color::srgb(0.45, 0.2, 0.15))
        } else if pot.dirty {
            (POT_EMPTY_MODEL, 0.85, Color::srgb(0.5, 0.5, 0.52))
        } else if done_ready {
            (POT_STEW_MODEL, 0.95, Color::WHITE)
        } else {
            (POT_EMPTY_MODEL, 0.9, Color::WHITE)
        };
        let mesh = spawn_model(&mut commands, &assets, &mut meshes, &mut materials, file, pot_t.translation + POT_VISUAL_OFFSET, scale, tint);
        commands.entity(mesh).insert(PotVisual { pot: pot_e }).insert(InKitchen);
        pot.visual = Some(mesh);
        pot.visual_dirty = false;
    }
}

pub struct StewPlugin;

impl Plugin for StewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (tick_pots, sync_pot_visuals).chain().run_if(in_state(GameState::Playing)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ladled_plate_matches_hearty_stew() {
        let plate = ladled_stew_plate();
        assert_eq!(match_recipe(&plate), Some(RecipeId::HeartyStew));
        let mut rev = plate.clone();
        rev.reverse();
        assert_eq!(match_recipe(&rev), Some(RecipeId::HeartyStew));
    }

    #[test]
    fn only_chopped_veg_counts() {
        assert!(is_stew_veg(Item::CarrotChopped));
        assert!(!is_stew_veg(Item::Carrot));
        assert!(!is_stew_veg(Item::BowlClean));
    }

    #[test]
    fn pot_transitions() {
        let mut pot = PotComponent::new();
        assert!(pot.is_empty_clean());
        pot.contents = ladled_stew_plate();
        pot.boiling = Some(PotBoil { elapsed: 3.0, total: BOIL_TOTAL_S });
        assert!(pot.is_cooking());
        pot.boiling = Some(PotBoil { elapsed: BOIL_TOTAL_S, total: BOIL_TOTAL_S });
        assert!(pot.is_done());
        pot.burned = true;
        assert!(pot.burned);
    }
}
