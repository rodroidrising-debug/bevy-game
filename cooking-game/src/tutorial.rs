//! Step-by-step coach for level 0 (Burger Bistro).

use bevy::prelude::*;

use crate::config::Item;
use crate::world::{Hands, Held, Hint, Player, RunScore, SelectedLevel, Station};

#[derive(Resource, Debug, Clone)]
pub struct Tutorial {
    pub step: usize,
    pub done: bool,
}

impl Default for Tutorial {
    fn default() -> Self {
        Self { step: 0, done: false }
    }
}

pub fn reset_tutorial() -> Tutorial {
    Tutorial { step: 0, done: false }
}

pub const STEP_LINES: [&str; 8] = [
    "Take a clean plate [E] at Plate Stack",
    "Grab a Bun [E] at its crate",
    "Chop Lettuce: grab Lettuce, [E] at board, then TAP E in rhythm (O on ###) x3",
    "Fry patty: grab Raw Patty, [E] at Stove, stir with E, R rescues burns, take when done",
    "Assemble on Counter: plate + ingredients until dish completes",
    "Serve [E] at Order Window",
    "Wash: take dirty plate at Dish Rack, [E] at Sink",
    "Tutorial complete! Finish the rush!",
];

pub fn tutorial_line(tutorial: &Tutorial) -> String {
    STEP_LINES[tutorial.step.min(STEP_LINES.len() - 1)].to_string()
}

pub fn step_complete(
    step: usize,
    has_plate: bool,
    has_bun: bool,
    has_lettuce: bool,
    has_cooked: bool,
    has_dish: bool,
    served: u32,
    has_clean_plate: bool,
) -> bool {
    match step {
        0 => has_plate,
        1 => has_bun,
        2 => has_lettuce,
        3 => has_cooked,
        4 => has_dish,
        5 => served >= 1,
        6 => served >= 1 && has_clean_plate,
        _ => false,
    }
}

fn plate_contains(items: &[Item], pred: impl Fn(Item) -> bool) -> bool {
    items.iter().copied().any(pred)
}

fn is_lettuce_done(i: Item) -> bool {
    matches!(i, Item::LettuceSlice | Item::LettuceChopped)
}

pub fn tutorial_tick(
    player_q: Query<(&Transform, &Hands), With<Player>>,
    stations_q: Query<&Station>,
    score: Res<RunScore>,
    selected: Res<SelectedLevel>,
    mut hint: ResMut<Hint>,
    mut tutorial: ResMut<Tutorial>,
) {
    if selected.0 != 0 || tutorial.done {
        return;
    }
    if tutorial.step >= STEP_LINES.len() {
        tutorial.step = STEP_LINES.len() - 1;
        tutorial.done = true;
        return;
    }
    if tutorial.step >= 7 {
        tutorial.done = true;
        return;
    }
    let Ok((_xf, hands)) = player_q.single() else {
        return;
    };
    let mut hands_is_plate = false;
    let mut hands_single_bun = false;
    let mut hands_plate_has_bun = false;
    let mut hands_single_lettuce = false;
    let mut hands_plate_has_lettuce = false;
    let mut hands_single_cooked = false;
    let mut hands_plate_has_cooked = false;
    let mut hands_is_dish = false;
    let mut hands_is_clean_plate = false;
    match &hands.held {
        Some(Held::Plate(v)) => {
            hands_is_plate = true;
            hands_plate_has_bun = plate_contains(v, |i| i == Item::Bun);
            hands_plate_has_lettuce = plate_contains(v, is_lettuce_done);
            hands_plate_has_cooked = plate_contains(v, |i| i == Item::BurgerCooked);
        }
        Some(Held::Single(Item::Bun)) => hands_single_bun = true,
        Some(Held::Single(i)) if is_lettuce_done(*i) => hands_single_lettuce = true,
        Some(Held::Single(Item::BurgerCooked)) => hands_single_cooked = true,
        Some(Held::Single(Item::PlateClean)) => hands_is_clean_plate = true,
        Some(Held::Dish(_)) => hands_is_dish = true,
        _ => {}
    }
    let mut station_plate_has_bun = false;
    let mut station_plate_has_lettuce = false;
    let mut station_plate_has_cooked = false;
    let mut station_single_cooked = false;
    let mut station_has_dish = false;
    for st in stations_q.iter() {
        match &st.slot {
            Some(Held::Plate(v)) => {
                if plate_contains(v, |i| i == Item::Bun) {
                    station_plate_has_bun = true;
                }
                if plate_contains(v, is_lettuce_done) {
                    station_plate_has_lettuce = true;
                }
                if plate_contains(v, |i| i == Item::BurgerCooked) {
                    station_plate_has_cooked = true;
                }
            }
            Some(Held::Single(Item::BurgerCooked)) => station_single_cooked = true,
            Some(Held::Dish(_)) => station_has_dish = true,
            _ => {}
        }
    }
    let has_plate = hands_is_plate || hands_is_clean_plate;
    let has_bun = hands_single_bun || hands_plate_has_bun || station_plate_has_bun;
    let has_lettuce = hands_single_lettuce || hands_plate_has_lettuce || station_plate_has_lettuce;
    let has_cooked = hands_single_cooked || hands_plate_has_cooked || station_plate_has_cooked || station_single_cooked;
    let has_dish = hands_is_dish || station_has_dish;
    if step_complete(tutorial.step, has_plate, has_bun, has_lettuce, has_cooked, has_dish, score.served, hands_is_clean_plate) {
        tutorial.step += 1;
        if tutorial.step >= STEP_LINES.len() - 1 {
            tutorial.step = STEP_LINES.len() - 1;
            tutorial.done = true;
        }
        hint.set(tutorial_line(&tutorial));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_advance_in_order() {
        assert!(step_complete(0, true, false, false, false, false, 0, false));
        assert!(!step_complete(0, false, false, false, false, false, 0, false));
        assert!(step_complete(1, false, true, false, false, false, 0, false));
        assert!(step_complete(5, false, false, false, false, false, 1, false));
        assert!(!step_complete(5, false, false, false, false, true, 0, false));
        assert!(step_complete(6, false, false, false, false, false, 1, true));
        assert!(!step_complete(7, true, true, true, true, true, 99, true));
    }

    #[test]
    fn lines_cover_all_steps() {
        for i in 0..8 {
            assert!(!tutorial_line(&Tutorial { step: i, done: i == 7 }).is_empty());
        }
        assert_eq!(tutorial_line(&Tutorial { step: 999, done: true }), "Tutorial complete! Finish the rush!");
    }
}
