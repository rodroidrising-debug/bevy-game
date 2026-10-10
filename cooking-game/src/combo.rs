//! Serve-streak combo bonus: fast consecutive serves pay extra.

use bevy::prelude::*;

pub const COMBO_WINDOW: f32 = 15.0;
pub const MAX_COMBO_BONUS: u32 = 50;

#[derive(Resource, Debug, Clone)]
pub struct Combo {
    pub streak: u32,
    pub timer: f32,
}

impl Default for Combo {
    fn default() -> Self {
        Self { streak: 0, timer: 0.0 }
    }
}

impl Combo {
    pub fn active(&self) -> bool {
        self.streak > 0 && self.timer > 0.0
    }
}

pub fn combo_bonus(streak: u32) -> u32 {
    if streak < 2 {
        0
    } else {
        (streak - 1).min(5) * 10
    }
}

pub fn combo_tick(time: Res<Time>, mut combo: ResMut<Combo>) {
    if combo.timer <= 0.0 {
        combo.timer = 0.0;
        combo.streak = 0;
        return;
    }
    combo.timer -= time.delta_secs();
    if combo.timer <= 0.0 {
        combo.timer = 0.0;
        combo.streak = 0;
    }
}

pub fn note_serve(combo: &mut Combo) -> u32 {
    combo.streak = combo.streak.saturating_add(1);
    combo.timer = COMBO_WINDOW;
    combo_bonus(combo.streak)
}

pub fn note_expire(combo: &mut Combo) {
    combo.streak = 0;
    combo.timer = 0.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bonus_table() {
        assert_eq!(combo_bonus(0), 0);
        assert_eq!(combo_bonus(1), 0);
        assert_eq!(combo_bonus(2), 10);
        assert_eq!(combo_bonus(3), 20);
        assert_eq!(combo_bonus(6), 50);
        assert_eq!(combo_bonus(100), 50);
    }

    #[test]
    fn serve_bumps_and_expire_resets() {
        let mut c = Combo::default();
        assert_eq!(note_serve(&mut c), 0);
        assert_eq!(c.streak, 1);
        assert_eq!(note_serve(&mut c), 10);
        assert_eq!(c.streak, 2);
        note_expire(&mut c);
        assert_eq!(c.streak, 0);
        assert_eq!(c.timer, 0.0);
    }
}
