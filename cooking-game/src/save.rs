//! Persistence for best scores / stars without new dependencies.
//! Format: `scores=1,2,3 stars=1,2,3`. Missing/corrupt -> zeros. Never panics.

use std::fs;
use std::path::PathBuf;

use crate::economy::{Economy, Upgrades};
use crate::world::Best;

pub const SAVE_FILE_NAME: &str = "save.txt";

pub fn save_path() -> PathBuf {
    match std::env::current_exe() {
        Ok(exe) => {
            if let Some(dir) = exe.parent() {
                return dir.join(SAVE_FILE_NAME);
            }
            PathBuf::from(SAVE_FILE_NAME)
        }
        Err(e) => {
            eprintln!("[save] current_exe failed ({e}); using ./{SAVE_FILE_NAME}");
            PathBuf::from(SAVE_FILE_NAME)
        }
    }
}

pub fn serialize_best(best: &Best) -> String {
    format!(
        "scores={},{},{} stars={},{},{}",
        best.scores[0], best.scores[1], best.scores[2],
        best.stars[0], best.stars[1], best.stars[2]
    )
}

/// Full line: scores + stars + economy. Old files (scores/stars only) still load.
pub fn serialize_all(best: &Best, econ: &Economy) -> String {
    format!(
        "{} coins={} gems={} upgrades={} meals={} diner={} perfects={}",
        serialize_best(best),
        econ.coins,
        econ.gems,
        econ.upgrades.to_bits(),
        econ.upgrades.meals_code(),
        econ.upgrades.diner_code(),
        econ.perfect_serves
    )
}

pub fn serialize_full(best: &Best, econ: &Economy, settings: &crate::settings::Settings) -> String {
    format!("{} {}", serialize_all(best, econ), settings.to_tokens())
}

fn parse_u32(s: &str) -> u32 {
    s.trim().parse::<u32>().unwrap_or(0)
}

fn parse_star(s: &str) -> u8 {
    let v: i32 = s.trim().parse::<i32>().unwrap_or(0);
    v.clamp(0, 3) as u8
}

fn parse_csv_u32(s: &str) -> [u32; 3] {
    let mut out = [0u32; 3];
    for (i, part) in s.split(',').take(3).enumerate() {
        out[i] = parse_u32(part);
    }
    out
}

fn parse_csv_star(s: &str) -> [u8; 3] {
    let mut out = [0u8; 3];
    for (i, part) in s.split(',').take(3).enumerate() {
        out[i] = parse_star(part);
    }
    out
}

pub fn parse_best(content: &str) -> Best {
    let (best, _econ) = parse_all(content);
    best
}

/// Parse full line; unknown/missing tokens default (backward compatible).
/// Settings are applied into `settings` when provided.
pub fn parse_all(content: &str) -> (Best, Economy) {
    let (best, econ, _s) = parse_full(content);
    (best, econ)
}

pub fn parse_full(content: &str) -> (Best, Economy, crate::settings::Settings) {
    let mut scores = [0u32; 3];
    let mut stars = [0u8; 3];
    let mut econ = Economy::default();
    let mut upgrades_bits: Option<String> = None;
    let mut meals_code = String::new();
    let mut diner_code = String::new();
    for token in content.split_whitespace() {
        if let Some(rest) = token.strip_prefix("scores=") {
            scores = parse_csv_u32(rest);
        } else if let Some(rest) = token.strip_prefix("stars=") {
            stars = parse_csv_star(rest);
        } else if let Some(rest) = token.strip_prefix("coins=") {
            econ.coins = parse_u32(rest);
        } else if let Some(rest) = token.strip_prefix("gems=") {
            econ.gems = parse_u32(rest);
        } else if let Some(rest) = token.strip_prefix("upgrades=") {
            upgrades_bits = Some(rest.to_string());
        } else if let Some(rest) = token.strip_prefix("meals=") {
            meals_code = rest.to_string();
        } else if let Some(rest) = token.strip_prefix("diner=") {
            diner_code = rest.to_string();
        } else if let Some(rest) = token.strip_prefix("perfects=") {
            econ.perfect_serves = parse_u32(rest);
        }
    }
    if let Some(bits) = upgrades_bits {
        econ.upgrades = Upgrades::from_bits(&bits);
        econ.upgrades.apply_codes(&meals_code, &diner_code);
    }
    let mut settings = crate::settings::Settings::default();
    settings.apply_tokens(content);
    (Best { scores, stars }, econ, settings)
}

pub fn load_best() -> Best {
    let (best, _econ) = load_all();
    best
}

/// Load scores + economy + settings. Missing/corrupt -> defaults, never panics.
pub fn load_all() -> (Best, Economy) {
    let (b, e, _s) = load_full();
    (b, e)
}

pub fn load_full() -> (Best, Economy, crate::settings::Settings) {
    let path = save_path();
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[save] no save at {} ({e}); using defaults", path.display());
            return (Best::default(), Economy::default(), crate::settings::Settings::default());
        }
    };
    if content.trim().is_empty() {
        return (Best::default(), Economy::default(), crate::settings::Settings::default());
    }
    parse_full(&content)
}

pub fn save_best(best: &Best) {
    // Backward-compatible single-arg form: preserves economy + settings on disk.
    let ( _b, econ, settings) = load_full_quiet();
    let _ = _b;
    save_full(best, &econ, &settings);
}

fn load_full_quiet() -> (Best, Economy, crate::settings::Settings) {
    let path = save_path();
    match fs::read_to_string(&path) {
        Ok(c) if !c.trim().is_empty() => parse_full(&c),
        _ => (Best::default(), Economy::default(), crate::settings::Settings::default()),
    }
}

/// Write scores + economy, preserving settings tokens already on disk.
pub fn save_all(best: &Best, econ: &Economy) {
    let (_, _, settings) = load_full_quiet();
    save_full(best, econ, &settings);
}

/// Write everything. Errors logged, never panics.
pub fn save_full(best: &Best, econ: &Economy, settings: &crate::settings::Settings) {
    let path = save_path();
    let text = serialize_full(best, econ, settings);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = fs::create_dir_all(parent);
        }
    }
    if let Err(e) = fs::write(&path, format!("{text}\n")) {
        eprintln!("[save] failed to write {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_canonical() {
        let b = Best { scores: [120, 450, 900], stars: [1, 2, 3] };
        let s = serialize_best(&b);
        assert_eq!(s, "scores=120,450,900 stars=1,2,3");
        let back = parse_best(&s);
        assert_eq!(back.scores, b.scores);
        assert_eq!(back.stars, b.stars);
    }

    #[test]
    fn parse_tolerates_whitespace_and_order() {
        let b = parse_best("  stars=3,2,1   scores=10,20,30\n");
        assert_eq!(b.scores, [10, 20, 30]);
        assert_eq!(b.stars, [3, 2, 1]);
    }

    #[test]
    fn parse_defaults_on_garbage() {
        let b = parse_best("hello world !!!");
        assert_eq!(b.scores, [0, 0, 0]);
        assert_eq!(b.stars, [0, 0, 0]);
        let b3 = parse_best("scores=abc,,-5 stars=99,-1,xyz");
        assert_eq!(b3.scores, [0, 0, 0]);
        assert_eq!(b3.stars, [3, 0, 0]);
    }
}
