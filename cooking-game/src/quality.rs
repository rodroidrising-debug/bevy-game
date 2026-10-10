//! Serve quality grades: how fast you serve decides the grade bonus.
//!
//! Pipeline: `score_serve(recipe, age, expire)` (freshness points in
//! `config.rs`) produces the base-with-freshness value, then
//! [`graded_points`] multiplies it by the [`Grade`] multiplier.
//!
//! Thresholds are on `age / expire`:
//! - `<= 0.25` => [`Grade::S`] (PERFECT, x1.5)
//! - `<= 0.55` => [`Grade::A`] (Great, x1.2)
//! - else      => [`Grade::B`] (Served, x1.0)

use bevy::prelude::*;

/// Serve-speed grade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Grade {
    /// Served within the first quarter of the ticket lifetime.
    S,
    /// Served within the first ~half of the ticket lifetime.
    A,
    /// Anything slower (but still before expiry).
    #[default]
    B,
}

impl Grade {
    /// Convenience wrapper so call sites can write `Grade::for_serve(age, expire)`.
    pub fn for_serve(age: f32, expire: f32) -> Grade {
        grade_for(age, expire)
    }

    /// Numeric multiplier for this grade.
    pub fn multiplier(self) -> f32 {
        grade_multiplier(self)
    }

    /// HUD label for this grade.
    pub fn label(self) -> &'static str {
        grade_label(self)
    }

    /// HUD / floating-text color for this grade.
    pub fn color(self) -> Color {
        grade_color(self)
    }
}

/// Grade a serve by ticket age.
///
/// `age` is seconds since the ticket spawned, `expire` is the ticket
/// lifetime (`LevelConfig::order_expire_s`). Negative ages are clamped to
/// `S`; non-positive lifetimes defensively grade `B` (no meaningful ratio).
pub fn grade_for(age: f32, expire: f32) -> Grade {
    if expire <= 0.0 {
        return Grade::B;
    }
    let ratio = (age / expire).max(0.0);
    if ratio <= 0.25 {
        Grade::S
    } else if ratio <= 0.55 {
        Grade::A
    } else {
        Grade::B
    }
}

/// Score multiplier per grade: S = 1.5, A = 1.2, B = 1.0.
pub fn grade_multiplier(g: Grade) -> f32 {
    match g {
        Grade::S => 1.5,
        Grade::A => 1.2,
        Grade::B => 1.0,
    }
}

/// Apply the grade multiplier to a freshness-adjusted base score.
///
/// Returns `(total, grade)` where `total = (base as f32 * mult).round()`.
pub fn graded_points(base_with_freshness: u32, age: f32, expire: f32) -> (u32, Grade) {
    let grade = grade_for(age, expire);
    let total = (base_with_freshness as f32 * grade_multiplier(grade)).round() as u32;
    (total, grade)
}

/// Short HUD label per grade.
pub fn grade_label(g: Grade) -> &'static str {
    match g {
        Grade::S => "PERFECT! x1.5",
        Grade::A => "Great x1.2",
        Grade::B => "Served",
    }
}

/// HUD / floating-text color per grade: gold / green / white.
pub fn grade_color(g: Grade) -> Color {
    match g {
        Grade::S => Color::srgb(1.0, 0.84, 0.0),
        Grade::A => Color::srgb(0.35, 0.9, 0.35),
        Grade::B => Color::WHITE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn s_boundary_at_quarter() {
        assert_eq!(grade_for(0.0, 80.0), Grade::S);
        assert_eq!(grade_for(20.0, 80.0), Grade::S); // exactly 0.25
        assert_eq!(grade_for(20.01, 80.0), Grade::A); // just over
    }

    #[test]
    fn a_boundary_at_55_percent() {
        assert_eq!(grade_for(44.0, 80.0), Grade::A); // exactly 0.55
        assert_eq!(grade_for(44.01, 80.0), Grade::B); // just over
        assert_eq!(grade_for(21.0, 80.0), Grade::A);
    }

    #[test]
    fn b_for_slow_and_expired() {
        assert_eq!(grade_for(60.0, 80.0), Grade::B);
        assert_eq!(grade_for(79.9, 80.0), Grade::B);
        assert_eq!(grade_for(200.0, 80.0), Grade::B);
    }

    #[test]
    fn multipliers_match_spec() {
        assert!(approx(grade_multiplier(Grade::S), 1.5));
        assert!(approx(grade_multiplier(Grade::A), 1.2));
        assert!(approx(grade_multiplier(Grade::B), 1.0));
    }

    #[test]
    fn s_bonus_math_fresh_hundred() {
        // 100 base with full freshness served instantly => 150.
        let (total, grade) = graded_points(100, 0.0, 80.0);
        assert_eq!(grade, Grade::S);
        assert_eq!(total, 150);
    }

    #[test]
    fn b_has_no_bonus() {
        let (total, grade) = graded_points(137, 70.0, 80.0);
        assert_eq!(grade, Grade::B);
        assert_eq!(total, 137);
    }

    #[test]
    fn a_bonus_rounds() {
        // 100 * 1.2 = 120 exactly; 99 * 1.2 = 118.8 => 119.
        assert_eq!(graded_points(100, 30.0, 80.0), (120, Grade::A));
        assert_eq!(graded_points(99, 30.0, 80.0), (119, Grade::A));
    }

    #[test]
    fn labels_and_colors() {
        assert_eq!(grade_label(Grade::S), "PERFECT! x1.5");
        assert_eq!(grade_label(Grade::A), "Great x1.2");
        assert_eq!(grade_label(Grade::B), "Served");
        assert_eq!(grade_color(Grade::B), Color::WHITE);
        assert_ne!(grade_color(Grade::S), grade_color(Grade::A));
    }

    #[test]
    fn degenerate_expire_is_b() {
        assert_eq!(grade_for(0.0, 0.0), Grade::B);
        assert_eq!(grade_for(5.0, -10.0), Grade::B);
        assert_eq!(grade_for(-5.0, 80.0), Grade::S);
    }
}
