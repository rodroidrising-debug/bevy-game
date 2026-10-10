//! Settings (Step 7): volumes, remappable keys, chef character. Persisted in save.txt.

use bevy::prelude::*;

/// Playable chefs. Cosmetic color + small speed trait.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CharacterId {
    #[default]
    Tomato,
    Basil,
    Berry,
}

impl CharacterId {
    pub const ALL: [CharacterId; 3] = [CharacterId::Tomato, CharacterId::Basil, CharacterId::Berry];

    pub fn name(&self) -> &'static str {
        match self {
            CharacterId::Tomato => "Chef Tomato",
            CharacterId::Basil => "Chef Basil",
            CharacterId::Berry => "Chef Berry",
        }
    }

    pub fn blurb(&self) -> &'static str {
        match self {
            CharacterId::Tomato => "Balanced classic. Speed x1.00",
            CharacterId::Basil => "Speedy sous-chef. Speed x1.06",
            CharacterId::Berry => "Steady saucier. Speed x0.95, +5% tips",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            CharacterId::Tomato => Color::srgb(0.95, 0.35, 0.25),
            CharacterId::Basil => Color::srgb(0.3, 0.7, 0.3),
            CharacterId::Berry => Color::srgb(0.35, 0.45, 0.95),
        }
    }

    pub fn speed_mult(&self) -> f32 {
        match self {
            CharacterId::Tomato => 1.0,
            CharacterId::Basil => 1.06,
            CharacterId::Berry => 0.95,
        }
    }

    /// Berry's steady hands earn slightly better tips.
    pub fn tip_bonus(&self) -> u32 {
        match self {
            CharacterId::Berry => 1,
            _ => 0,
        }
    }

    pub fn index(&self) -> usize {
        match self {
            CharacterId::Tomato => 0,
            CharacterId::Basil => 1,
            CharacterId::Berry => 2,
        }
    }

    pub fn next(&self) -> CharacterId {
        CharacterId::ALL[(self.index() + 1) % CharacterId::ALL.len()]
    }
}

/// Remappable primary keys. Arrows (move), Space (interact), X (trash) and
/// P (pause) are always hard-coded alternates and need no rebinding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bindings {
    pub up: KeyCode,
    pub down: KeyCode,
    pub left: KeyCode,
    pub right: KeyCode,
    pub interact: KeyCode,
    pub trash: KeyCode,
    pub rescue: KeyCode,
    pub pause: KeyCode,
    pub mute: KeyCode,
}

impl Default for Bindings {
    fn default() -> Self {
        Self {
            up: KeyCode::KeyW,
            down: KeyCode::KeyS,
            left: KeyCode::KeyA,
            right: KeyCode::KeyD,
            interact: KeyCode::KeyE,
            trash: KeyCode::KeyQ,
            rescue: KeyCode::KeyR,
            pause: KeyCode::Escape,
            mute: KeyCode::KeyM,
        }
    }
}

impl Bindings {
    pub const ACTIONS: [&'static str; 9] =
        ["Move Up", "Move Down", "Move Left", "Move Right", "Interact", "Trash", "Rescue Flip", "Pause", "Mute"];

    pub fn get(&self, i: usize) -> KeyCode {
        match i {
            0 => self.up,
            1 => self.down,
            2 => self.left,
            3 => self.right,
            4 => self.interact,
            5 => self.trash,
            6 => self.rescue,
            7 => self.pause,
            _ => self.mute,
        }
    }

    pub fn set(&mut self, i: usize, key: KeyCode) {
        match i {
            0 => self.up = key,
            1 => self.down = key,
            2 => self.left = key,
            3 => self.right = key,
            4 => self.interact = key,
            5 => self.trash = key,
            6 => self.rescue = key,
            7 => self.pause = key,
            _ => self.mute = key,
        }
    }

    pub fn to_csv(&self) -> String {
        [self.up, self.down, self.left, self.right, self.interact, self.trash, self.rescue, self.pause, self.mute]
            .iter()
            .map(|k| key_name(*k))
            .collect::<Vec<_>>()
            .join(",")
    }

    pub fn from_csv(s: &str) -> Self {
        let mut b = Bindings::default();
        for (i, part) in s.split(',').take(9).enumerate() {
            if let Some(k) = key_from_name(part.trim()) {
                b.set(i, k);
            }
        }
        b
    }
}

/// Short display name for supported keys.
pub fn key_name(key: KeyCode) -> &'static str {
    match key {
        KeyCode::KeyW => "W",
        KeyCode::KeyA => "A",
        KeyCode::KeyS => "S",
        KeyCode::KeyD => "D",
        KeyCode::KeyE => "E",
        KeyCode::KeyQ => "Q",
        KeyCode::KeyR => "R",
        KeyCode::KeyM => "M",
        KeyCode::KeyP => "P",
        KeyCode::KeyX => "X",
        KeyCode::KeyF => "F",
        KeyCode::KeyC => "C",
        KeyCode::KeyB => "B",
        KeyCode::KeyG => "G",
        KeyCode::KeyH => "H",
        KeyCode::KeyJ => "J",
        KeyCode::KeyK => "K",
        KeyCode::KeyL => "L",
        KeyCode::KeyT => "T",
        KeyCode::KeyU => "U",
        KeyCode::KeyI => "I",
        KeyCode::KeyO => "O",
        KeyCode::KeyZ => "Z",
        KeyCode::ArrowUp => "Up",
        KeyCode::ArrowDown => "Down",
        KeyCode::ArrowLeft => "Left",
        KeyCode::ArrowRight => "Right",
        KeyCode::Space => "Space",
        KeyCode::Escape => "Esc",
        KeyCode::Enter => "Enter",
        KeyCode::Tab => "Tab",
        KeyCode::ShiftLeft => "LShift",
        _ => "?",
    }
}

/// Parse a name produced by key_name (plus single letters). None if unsupported.
pub fn key_from_name(s: &str) -> Option<KeyCode> {
    match s {
        "W" => Some(KeyCode::KeyW),
        "A" => Some(KeyCode::KeyA),
        "S" => Some(KeyCode::KeyS),
        "D" => Some(KeyCode::KeyD),
        "E" => Some(KeyCode::KeyE),
        "Q" => Some(KeyCode::KeyQ),
        "R" => Some(KeyCode::KeyR),
        "M" => Some(KeyCode::KeyM),
        "P" => Some(KeyCode::KeyP),
        "X" => Some(KeyCode::KeyX),
        "F" => Some(KeyCode::KeyF),
        "C" => Some(KeyCode::KeyC),
        "B" => Some(KeyCode::KeyB),
        "G" => Some(KeyCode::KeyG),
        "H" => Some(KeyCode::KeyH),
        "J" => Some(KeyCode::KeyJ),
        "K" => Some(KeyCode::KeyK),
        "L" => Some(KeyCode::KeyL),
        "T" => Some(KeyCode::KeyT),
        "U" => Some(KeyCode::KeyU),
        "I" => Some(KeyCode::KeyI),
        "O" => Some(KeyCode::KeyO),
        "Z" => Some(KeyCode::KeyZ),
        "Up" => Some(KeyCode::ArrowUp),
        "Down" => Some(KeyCode::ArrowDown),
        "Left" => Some(KeyCode::ArrowLeft),
        "Right" => Some(KeyCode::ArrowRight),
        "Space" => Some(KeyCode::Space),
        "Esc" => Some(KeyCode::Escape),
        "Enter" => Some(KeyCode::Enter),
        "Tab" => Some(KeyCode::Tab),
        "LShift" => Some(KeyCode::ShiftLeft),
        _ => None,
    }
}

#[derive(Resource, Debug, Clone)]
pub struct Settings {
    pub master: f32,
    pub music: f32,
    pub sfx: f32,
    pub bindings: Bindings,
    pub chef: CharacterId,
    /// Persistent online tag (auto-generated) + custom server URL (empty = default).
    pub player: String,
    pub server: String,
    /// Device secret for the server account (auto-generated, never shown).
    pub secret: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self { master: 1.0, music: 0.7, sfx: 0.9, bindings: Bindings::default(), chef: CharacterId::default(), player: "Chef".to_string(), server: String::new(), secret: String::new() }
    }
}

impl Settings {
    pub fn bump(&mut self, which: u8, dir: f32) {
        let v = match which {
            0 => &mut self.master,
            1 => &mut self.music,
            _ => &mut self.sfx,
        };
        *v = (*v + dir * 0.1).clamp(0.0, 1.0);
    }

    pub fn to_tokens(&self) -> String {
        format!(
            "vol={:.2},{:.2},{:.2} chef={} keys={} player={} server={} secret={}",
            self.master,
            self.music,
            self.sfx,
            self.chef.index(),
            self.bindings.to_csv(),
            sanitize_tag(&self.player),
            self.server.trim(),
            self.secret.trim(),
        )
    }

    pub fn apply_tokens(&mut self, content: &str) {
        for token in content.split_whitespace() {
            if let Some(rest) = token.strip_prefix("vol=") {
                let parts: Vec<&str> = rest.split(',').collect();
                if parts.len() >= 3 {
                    self.master = parts[0].parse().map(|v: f32| v.clamp(0.0, 1.0)).unwrap_or(1.0);
                    self.music = parts[1].parse().map(|v: f32| v.clamp(0.0, 1.0)).unwrap_or(0.7);
                    self.sfx = parts[2].parse().map(|v: f32| v.clamp(0.0, 1.0)).unwrap_or(0.9);
                }
            } else if let Some(rest) = token.strip_prefix("chef=") {
                let i: usize = rest.parse().unwrap_or(0);
                self.chef = CharacterId::ALL[i.min(2)];
            } else if let Some(rest) = token.strip_prefix("keys=") {
                self.bindings = Bindings::from_csv(rest);
            } else if let Some(rest) = token.strip_prefix("player=") {
                let p = sanitize_tag(rest);
                if !p.is_empty() {
                    self.player = p;
                }
            } else if let Some(rest) = token.strip_prefix("server=") {
                self.server = rest.trim().to_string();
            } else if let Some(rest) = token.strip_prefix("secret=") {
                let p: String = rest.chars().filter(|c| c.is_alphanumeric()).take(64).collect();
                if !p.is_empty() {
                    self.secret = p;
                }
            }
        }
    }
}

/// Keep only leaderboard-safe characters, max 16.
pub fn sanitize_tag(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .take(16)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_roundtrip() {
        for k in [KeyCode::KeyW, KeyCode::KeyE, KeyCode::Space, KeyCode::Escape, KeyCode::ArrowUp] {
            assert_eq!(key_from_name(key_name(k)), Some(k));
        }
        assert_eq!(key_from_name("Nope"), None);
    }

    #[test]
    fn bindings_csv_roundtrip() {
        let mut b = Bindings::default();
        b.set(4, KeyCode::KeyF);
        let back = Bindings::from_csv(&b.to_csv());
        assert_eq!(back.interact, KeyCode::KeyF);
        assert_eq!(back.up, KeyCode::KeyW);
        // garbage keeps defaults
        let g = Bindings::from_csv("E,Q,garbage");
        assert_eq!(g.up, KeyCode::KeyE);
        assert_eq!(g.right, KeyCode::KeyD);
    }

    #[test]
    fn settings_tokens_roundtrip() {
        let mut s = Settings::default();
        s.master = 0.5;
        s.chef = CharacterId::Berry;
        s.bindings.set(4, KeyCode::Space);
        let t = s.to_tokens();
        let mut back = Settings::default();
        back.apply_tokens(&t);
        assert!((back.master - 0.5).abs() < 0.01);
        assert_eq!(back.chef, CharacterId::Berry);
        assert_eq!(back.bindings.interact, KeyCode::Space);
    }

    #[test]
    fn chef_cycle_and_traits() {
        assert_eq!(CharacterId::Tomato.next(), CharacterId::Basil);
        assert_eq!(CharacterId::Berry.next(), CharacterId::Tomato);
        assert!(CharacterId::Basil.speed_mult() > 1.0);
        assert_eq!(CharacterId::Berry.tip_bonus(), 1);
    }
}
