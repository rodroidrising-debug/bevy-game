//! Economy (Step 6): coins, gems, and permanent kitchen upgrades.
//!
//! - Coins: 1 per 10 pts served + grade tips (S +2, A +1). Spend in shop.
//! - Gems: 1 per 3-star level finish + 1 per 8 PERFECT serves. Premium buys.
//! - Upgrades persist in save.txt alongside scores/stars.

use bevy::prelude::*;

/// How many PERFECT serves earn a bonus gem.
pub const PERFECTS_PER_GEM: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UpgradeId {
    SharpKnife,
    NonstickPan,
    FastShoes,
    LongFuse,
    LuckyLadle,
    Mulligan,
}

impl UpgradeId {
    pub const ALL: [UpgradeId; 6] = [
        UpgradeId::SharpKnife,
        UpgradeId::NonstickPan,
        UpgradeId::FastShoes,
        UpgradeId::LongFuse,
        UpgradeId::LuckyLadle,
        UpgradeId::Mulligan,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            UpgradeId::SharpKnife => "Sharp Knife",
            UpgradeId::NonstickPan => "Non-stick Pan",
            UpgradeId::FastShoes => "Fast Shoes",
            UpgradeId::LongFuse => "Long Fuse",
            UpgradeId::LuckyLadle => "Lucky Ladle",
            UpgradeId::Mulligan => "Mulligan Charm",
        }
    }

    pub fn blurb(&self) -> &'static str {
        match self {
            UpgradeId::SharpKnife => "Chop in 2 hits instead of 3.",
            UpgradeId::NonstickPan => "Pan heat rises 35% slower, stirs cool more.",
            UpgradeId::FastShoes => "Move ~25% faster.",
            UpgradeId::LongFuse => "+3s before fried food burns.",
            UpgradeId::LuckyLadle => "Stew boils in 6s instead of 8s.",
            UpgradeId::Mulligan => "First expired order each run is forgiven.",
        }
    }

    /// (coin cost, gem cost). Gems only where marked.
    pub fn cost(&self) -> (u32, u32) {
        match self {
            UpgradeId::SharpKnife => (60, 0),
            UpgradeId::NonstickPan => (80, 0),
            UpgradeId::FastShoes => (50, 0),
            UpgradeId::LongFuse => (70, 0),
            UpgradeId::LuckyLadle => (90, 0),
            UpgradeId::Mulligan => (40, 1),
        }
    }

    pub fn index(&self) -> usize {
        match self {
            UpgradeId::SharpKnife => 0,
            UpgradeId::NonstickPan => 1,
            UpgradeId::FastShoes => 2,
            UpgradeId::LongFuse => 3,
            UpgradeId::LuckyLadle => 4,
            UpgradeId::Mulligan => 5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Upgrades {
    pub owned: [bool; 6],
    /// Meal levels per RecipeId index (0-3). +10% points per level.
    pub meals: [u8; 8],
    /// Restaurant levels (0-3). +5s ticket time per level.
    pub diner_lv: [u8; 3],
}

impl Upgrades {
    pub fn has(&self, id: UpgradeId) -> bool {
        self.owned[id.index()]
    }
    pub fn give(&mut self, id: UpgradeId) {
        self.owned[id.index()] = true;
    }
    pub fn to_bits(&self) -> String {
        self.owned.iter().map(|b| if *b { '1' } else { '0' }).collect()
    }
    pub fn from_bits(s: &str) -> Self {
        let mut o = [false; 6];
        for (i, c) in s.chars().take(6).enumerate() {
            o[i] = c == '1';
        }
        Self { owned: o, meals: [0; 8], diner_lv: [0; 3] }
    }

    pub fn meals_code(&self) -> String {
        self.meals.iter().map(|v| char::from_digit((*v).min(3) as u32, 10).unwrap_or('0')).collect()
    }
    pub fn diner_code(&self) -> String {
        self.diner_lv.iter().map(|v| char::from_digit((*v).min(3) as u32, 10).unwrap_or('0')).collect()
    }
    pub fn apply_codes(&mut self, meals: &str, diner: &str) {
        for (i, c) in meals.chars().take(8).enumerate() {
            self.meals[i] = c.to_digit(10).unwrap_or(0).min(3) as u8;
        }
        for (i, c) in diner.chars().take(3).enumerate() {
            self.diner_lv[i] = c.to_digit(10).unwrap_or(0).min(3) as u8;
        }
    }

    /// +10% dish points per meal level.
    pub fn meal_mult(&self, recipe_idx: usize) -> f32 {
        1.0 + 0.1 * self.meals[recipe_idx.min(7)] as f32
    }

    /// +5s ticket patience per restaurant level.
    pub fn diner_bonus(&self, level: usize) -> f32 {
        5.0 * self.diner_lv[level.min(2)] as f32
    }

    pub fn meal_cost(&self, recipe_idx: usize) -> u32 {
        let lv = self.meals[recipe_idx.min(7)];
        if lv >= 3 {
            return u32::MAX; // maxed
        }
        40 * (lv as u32 + 1)
    }

    pub fn diner_cost(&self, level: usize) -> u32 {
        let lv = self.diner_lv[level.min(2)];
        if lv >= 3 {
            return u32::MAX;
        }
        100 * (lv as u32 + 1)
    }

    // ---- gameplay effects ----
    pub fn chop_needed(&self) -> u32 {
        if self.has(UpgradeId::SharpKnife) {
            2
        } else {
            3
        }
    }
    pub fn heat_rate_mult(&self) -> f32 {
        if self.has(UpgradeId::NonstickPan) {
            0.65
        } else {
            1.0
        }
    }
    pub fn stir_cool_bonus(&self) -> f32 {
        if self.has(UpgradeId::NonstickPan) {
            0.08
        } else {
            0.0
        }
    }
    pub fn player_speed(&self) -> f32 {
        if self.has(UpgradeId::FastShoes) {
            6.2
        } else {
            5.0
        }
    }
    pub fn burn_bonus(&self) -> f32 {
        if self.has(UpgradeId::LongFuse) {
            3.0
        } else {
            0.0
        }
    }
    pub fn boil_total(&self) -> f32 {
        if self.has(UpgradeId::LuckyLadle) {
            6.0
        } else {
            8.0
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct Economy {
    pub coins: u32,
    pub gems: u32,
    pub upgrades: Upgrades,
    pub perfect_serves: u32,
    pub mulligan_used_this_run: bool,
}

impl Default for Economy {
    fn default() -> Self {
        Self { coins: 0, gems: 0, upgrades: Upgrades::default(), perfect_serves: 0, mulligan_used_this_run: false }
    }
}

impl Economy {
    /// Pay out a serve: coins from points + tips, gem every 8 PERFECTs.
    /// Returns (coins_earned, gem_earned).
    pub fn pay_serve(&mut self, points: u32, grade: crate::quality::Grade) -> (u32, bool) {
        let mut coins = points / 10;
        coins += match grade {
            crate::quality::Grade::S => 2,
            crate::quality::Grade::A => 1,
            crate::quality::Grade::B => 0,
        };
        self.coins = self.coins.saturating_add(coins);
        let mut gem = false;
        if grade == crate::quality::Grade::S {
            self.perfect_serves += 1;
            if self.perfect_serves % PERFECTS_PER_GEM == 0 {
                self.gems = self.gems.saturating_add(1);
                gem = true;
            }
        }
        (coins, gem)
    }

    /// Try to buy; returns Ok(()) or a reason string.
    pub fn buy(&mut self, id: UpgradeId) -> Result<(), &'static str> {
        if self.upgrades.has(id) {
            return Err("Already owned");
        }
        let (c, g) = id.cost();
        if self.coins < c {
            return Err("Not enough coins");
        }
        if self.gems < g {
            return Err("Not enough gems");
        }
        self.coins -= c;
        self.gems -= g;
        self.upgrades.give(id);
        Ok(())
    }

    pub fn reset_run_flags(&mut self) {
        self.mulligan_used_this_run = false;
    }

    /// Buy a meal level (coins only). Err reason or maxed.
    pub fn buy_meal(&mut self, recipe_idx: usize) -> Result<(), &'static str> {
        let i = recipe_idx.min(7);
        if self.upgrades.meals[i] >= 3 {
            return Err("Max level");
        }
        let cost = self.upgrades.meal_cost(i);
        if self.coins < cost {
            return Err("Not enough coins");
        }
        self.coins -= cost;
        self.upgrades.meals[i] += 1;
        Ok(())
    }

    /// Buy a restaurant level (coins only).
    pub fn buy_diner(&mut self, level: usize) -> Result<(), &'static str> {
        let i = level.min(2);
        if self.upgrades.diner_lv[i] >= 3 {
            return Err("Max level");
        }
        let cost = self.upgrades.diner_cost(i);
        if self.coins < cost {
            return Err("Not enough coins");
        }
        self.coins -= cost;
        self.upgrades.diner_lv[i] += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn economy_pays_and_gems() {
        let mut e = Economy::default();
        let (c, g) = e.pay_serve(130, crate::quality::Grade::S);
        assert_eq!(c, 13 + 2);
        assert!(!g);
        assert_eq!(e.coins, 15);
        for _ in 0..7 {
            e.pay_serve(100, crate::quality::Grade::S);
        }
        assert_eq!(e.gems, 1); // 8th perfect
    }

    #[test]
    fn buy_checks_funds() {
        let mut e = Economy::default();
        assert_eq!(e.buy(UpgradeId::SharpKnife), Err("Not enough coins"));
        e.coins = 100;
        assert!(e.buy(UpgradeId::SharpKnife).is_ok());
        assert_eq!(e.buy(UpgradeId::SharpKnife), Err("Already owned"));
        assert_eq!(e.buy(UpgradeId::Mulligan), Err("Not enough gems"));
        e.coins = 100;
        e.gems = 1;
        assert!(e.buy(UpgradeId::Mulligan).is_ok());
    }

    #[test]
    fn upgrade_effects() {
        let mut u = Upgrades::default();
        assert_eq!(u.chop_needed(), 3);
        assert_eq!(u.player_speed(), 5.0);
        u.give(UpgradeId::SharpKnife);
        u.give(UpgradeId::FastShoes);
        u.give(UpgradeId::LuckyLadle);
        assert_eq!(u.chop_needed(), 2);
        assert_eq!(u.player_speed(), 6.2);
        assert_eq!(u.boil_total(), 6.0);
        assert_eq!(Upgrades::from_bits(&u.to_bits()).owned, u.owned);
    }

    #[test]
    fn meal_and_diner_levels() {
        let mut e = Economy::default();
        e.coins = 1000;
        assert!((e.upgrades.meal_mult(0) - 1.0).abs() < 1e-6);
        assert!(e.buy_meal(0).is_ok());
        assert!((e.upgrades.meal_mult(0) - 1.1).abs() < 1e-6);
        assert_eq!(e.buy_meal(0), Ok(()));
        assert_eq!(e.buy_meal(0), Ok(()));
        assert_eq!(e.buy_meal(0), Err("Max level"));
        assert_eq!(e.upgrades.diner_bonus(1), 0.0);
        assert!(e.buy_diner(1).is_ok());
        assert_eq!(e.upgrades.diner_bonus(1), 5.0);
        let mut u2 = Upgrades::default();
        u2.apply_codes(&e.upgrades.meals_code(), &e.upgrades.diner_code());
        assert_eq!(u2.meals, e.upgrades.meals);
        assert_eq!(u2.diner_lv, e.upgrades.diner_lv);
    }
}
