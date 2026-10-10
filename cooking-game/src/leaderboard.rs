//! Online backend client (Step 9): accounts, authed submits, balance sync,
//! online shop buys. All blocking I/O runs in worker threads; the game polls
//! a channel and stays playable offline.

use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use bevy::prelude::*;

use crate::world::{RunScore, SelectedLevel};

pub const DEFAULT_URL: &str = "https://short-order-showdown-scores.onrender.com";

#[derive(Resource, Debug, Clone)]
pub struct BoardConfig {
    pub base_url: String,
    pub player: String,
    pub secret: String,
}

impl BoardConfig {
    pub fn from_settings(settings: &crate::settings::Settings) -> Self {
        let url = if !settings.server.trim().is_empty() {
            settings.server.trim().trim_end_matches('/').to_string()
        } else {
            std::env::var("LEADERBOARD_URL").unwrap_or_else(|_| DEFAULT_URL.into())
        };
        Self { base_url: url, player: settings.player.clone(), secret: settings.secret.clone() }
    }

    pub fn scores_url(&self) -> String {
        format!("{}/api/scores", self.base_url.trim_end_matches('/'))
    }
    pub fn auth_url(&self) -> String {
        format!("{}/api/auth", self.base_url.trim_end_matches('/'))
    }
    pub fn accounts_url(&self) -> String {
        format!("{}/api/accounts", self.base_url.trim_end_matches('/'))
    }
    pub fn me_url(&self) -> String {
        format!("{}/api/me", self.base_url.trim_end_matches('/'))
    }
    pub fn buy_url(&self) -> String {
        format!("{}/api/shop/buy", self.base_url.trim_end_matches('/'))
    }
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(Duration::from_secs(5)).build()
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Authoritative balance snapshot from the server.
#[derive(Debug, Clone, Default)]
pub struct SnapDelta {
    pub coins: u32,
    pub gems: u32,
    pub perfects: u32,
    pub gear: [bool; 6],
    pub meals: [u8; 8],
    pub diner: [u8; 3],
    pub best_scores: [u32; 3],
    pub best_stars: [u8; 3],
}

pub enum NetMsg {
    /// Adopt server balances + show top list.
    SubmitDone { snap: SnapDelta, rank: u32, total: u32, top: Vec<(String, u32, u8)> },
    /// Adopted /api/me snapshot on the menu.
    MeSynced { snap: SnapDelta },
    /// Online shop result (ok + message) or fall back to local buy.
    ShopDone { ok: bool, text: String, snap: Option<SnapDelta> },
    BuyLocal { kind: String, id: u8 },
}

#[derive(Resource, Default)]
pub struct NetInbox {
    rx: Option<std::sync::Mutex<Receiver<NetMsg>>>,
    /// Set true once any server round-trip succeeds.
    pub online: bool,
}

impl NetInbox {
    pub fn post(&mut self, rx: Receiver<NetMsg>) {
        self.rx = Some(std::sync::Mutex::new(rx));
    }
    pub fn busy(&self) -> bool {
        self.rx.is_some()
    }
}

// --- tiny JSON readers for our own API shapes ---

fn num_after(s: &str, key: &str) -> Option<u32> {
    let i = s.find(key)? + key.len();
    s[i..].chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok()
}

fn str_after(s: &str, key: &str) -> Option<String> {
    let i = s.find(key)? + key.len();
    let rest = &s[i..];
    let rest = rest.trim_start_matches([' ', '"'].as_slice());
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn bool_array(s: &str, key: &str, n: usize) -> Vec<bool> {
    let Some(i) = s.find(key) else {
        return vec![false; n];
    };
    let seg_end = s[i..].find(']').unwrap_or(s.len() - i);
    let seg = &s[i..i + seg_end];
    let mut out: Vec<bool> = seg
        .split(|c| c == '[' || c == ',' || c == ']')
        .filter_map(|t| match t.trim() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        })
        .collect();
    out.resize(n, false);
    out.truncate(n);
    out
}

fn num_array(s: &str, key: &str, n: usize) -> Vec<u32> {
    let Some(i) = s.find(key) else {
        return vec![0; n];
    };
    let seg_end = s[i..].find(']').unwrap_or(s.len() - i);
    let seg = &s[i..i + seg_end];
    let mut out: Vec<u32> = seg
        .split(|c| c == '[' || c == ',' || c == ']')
        .filter_map(|t| t.trim().parse::<u32>().ok())
        .collect();
    out.resize(n, 0);
    out.truncate(n);
    out
}

pub fn parse_snap(body: &str) -> Option<SnapDelta> {
    let coins = num_after(body, "\"coins\":")?;
    let gems = num_after(body, "\"gems\":")?;
    let perfects = num_after(body, "\"perfects\":").unwrap_or(0);
    let gear_v = bool_array(body, "\"gear\":", 6);
    let meals_v = num_array(body, "\"meals\":", 8);
    let diner_v = num_array(body, "\"diner\":", 3);
    let bs_v = num_array(body, "\"best_scores\":", 3);
    let bst_v = num_array(body, "\"best_stars\":", 3);
    let mut snap = SnapDelta {
        coins,
        gems,
        perfects,
        gear: [false; 6],
        meals: [0; 8],
        diner: [0; 3],
        best_scores: [0; 3],
        best_stars: [0; 3],
    };
    snap.gear.copy_from_slice(&gear_v);
    for (i, v) in meals_v.into_iter().enumerate() {
        snap.meals[i] = v.min(3) as u8;
    }
    for (i, v) in diner_v.into_iter().enumerate() {
        snap.diner[i] = v.min(3) as u8;
    }
    snap.best_scores.copy_from_slice(&bs_v);
    for (i, v) in bst_v.into_iter().enumerate() {
        snap.best_stars[i] = v.min(3) as u8;
    }
    Some(snap)
}

/// Minimal parser for our own top-list JSON: [{player,score,stars}...].
pub fn parse_top(body: &str) -> Vec<(String, u32, u8)> {
    let mut out = Vec::new();
    for chunk in body.split("{\"player\":\"").skip(1) {
        let Some(end) = chunk.find('"') else {
            continue;
        };
        let player = chunk[..end].to_string();
        let rest = &chunk[end..];
        let score = number_after(rest, "\"score\":").unwrap_or(0);
        let stars = number_after(rest, "\"stars\":").unwrap_or(0) as u8;
        out.push((player, score, stars.min(3)));
        if out.len() >= 5 {
            break;
        }
    }
    out
}

fn number_after(s: &str, key: &str) -> Option<u32> {
    num_after(s, key)
}

/// Render stored top entries for the results screen.
pub fn render_top(entries: &[(String, u32, u8)]) -> String {
    let mut s = "Online top:\n".to_string();
    for (i, (name, pts, stars)) in entries.iter().enumerate() {
        s.push_str(&format!("{}. {} — {} ({}*)\n", i + 1, name, pts, stars));
    }
    s
}

// --- worker ops (blocking, run in threads) ---

/// Login; on unknown player, auto-register with the device secret.
fn ensure_token(cfg: &BoardConfig) -> Option<String> {
    let ag = agent();
    let login_body = format!(
        "{{\"player\":\"{}\",\"password\":\"{}\"}}",
        esc(&cfg.player),
        esc(&cfg.secret)
    );
    if let Ok(resp) = ag.post(&cfg.auth_url()).set("Content-Type", "application/json").send_string(&login_body) {
        if let Ok(body) = resp.into_string() {
            if let Some(t) = str_after(&body, "\"token\":\"") {
                return Some(t);
            }
        }
    }
    // try register (409 name-taken without our secret -> None)
    if let Ok(resp) = ag
        .post(&cfg.accounts_url())
        .set("Content-Type", "application/json")
        .send_string(&login_body)
    {
        if resp.status() == 201 {
            if let Ok(body) = resp.into_string() {
                return str_after(&body, "\"token\":\"");
            }
        }
    }
    None
}

fn authed_post(url: &str, token: &str, body: &str) -> Option<String> {
    let resp = agent()
        .post(url)
        .set("Content-Type", "application/json")
        .set("Authorization", &format!("Bearer {}", token))
        .send_string(body)
        .ok()?;
    if resp.status() == 200 || resp.status() == 201 {
        resp.into_string().ok()
    } else {
        None
    }
}

fn authed_get(url: &str, token: &str) -> Option<String> {
    let resp = agent()
        .get(url)
        .set("Authorization", &format!("Bearer {}", token))
        .call()
        .ok()?;
    if resp.status() == 200 {
        resp.into_string().ok()
    } else {
        None
    }
}

// --- systems ---

/// On entering results: ensure account, submit score, fetch top-5. One thread.
pub fn board_refresh(
    mut inbox: ResMut<NetInbox>,
    cfg: Res<BoardConfig>,
    score: Res<RunScore>,
    selected: Res<SelectedLevel>,
    mut top: ResMut<TopScores>,
) {
    if inbox.busy() {
        return;
    }
    top.pending = true;
    top.entries.clear();
    let cfg = cfg.clone();
    let (points, level, grade) = (score.points, selected.0.min(2) as u8, score.last_grade.clone());
    let (tx, rx) = mpsc::channel();
    inbox.post(rx);
    std::thread::spawn(move || {
        let grade = if grade.is_empty() { "B".to_string() } else { grade };
        let Some(token) = ensure_token(&cfg) else {
            return; // offline: run stays local, board shows offline note
        };
        let stars = crate::config::stars_for(points, crate::config::LEVELS[level as usize].star_thresholds);
        let body = format!(
            "{{\"level\":{},\"score\":{},\"stars\":{},\"grade\":\"{}\",\"tip\":0}}",
            level, points, stars, grade
        );
        let resp = authed_post(&cfg.scores_url(), &token, &body);
        let Some(b) = resp else {
            return;
        };
        let snap = parse_snap(&b);
        let rank = num_after(&b, "\"rank\":").unwrap_or(0);
        let total = num_after(&b, "\"total\":").unwrap_or(0);
        let list = authed_get(&format!("{}?level={}&limit=5", cfg.scores_url(), level), &token)
            .map(|t| parse_top(&t))
            .unwrap_or_default();
        if let Some(s) = snap {
            let _ = tx.send(NetMsg::SubmitDone { snap: s, rank, total, top: list });
        }
    });
}

/// On entering the menu: sync balances from the server if reachable.
pub fn menu_sync(mut inbox: ResMut<NetInbox>, cfg: Res<BoardConfig>) {
    if inbox.busy() {
        return;
    }
    let cfg = cfg.clone();
    let (tx, rx) = mpsc::channel();
    inbox.post(rx);
    std::thread::spawn(move || {
        let Some(token) = ensure_token(&cfg) else {
            return;
        };
        if let Some(body) = authed_get(&cfg.me_url(), &token) {
            if let Some(snap) = parse_snap(&body) {
                let _ = tx.send(NetMsg::MeSynced { snap });
            }
        }
    });
}

/// Fire an online shop purchase; result arrives via [`NetMsg::ShopDone`],
/// or [`NetMsg::BuyLocal`] when the server is unreachable.
pub fn shop_buy_async(inbox: &mut NetInbox, cfg: &BoardConfig, kind: &str, id: u8) {
    if inbox.busy() {
        return;
    }
    let cfg = cfg.clone();
    let kind = kind.to_string();
    let (tx, rx) = mpsc::channel();
    inbox.post(rx);
    std::thread::spawn(move || {
        let Some(token) = ensure_token(&cfg) else {
            let _ = tx.send(NetMsg::BuyLocal { kind, id });
            return;
        };
        let body = format!("{{\"kind\":\"{}\",\"id\":{}}}", kind, id);
        match authed_post(&cfg.buy_url(), &token, &body) {
            Some(resp) => match parse_snap(&resp) {
                Some(snap) => {
                    let _ = tx.send(NetMsg::ShopDone {
                        ok: true,
                        text: "Bought! (synced with server)".to_string(),
                        snap: Some(snap),
                    });
                }
                None => {
                    // error payload: {"error": "..."}
                    let why = str_after(&resp, "\"error\":\"").unwrap_or_else(|| "rejected".to_string());
                    let _ = tx.send(NetMsg::ShopDone { ok: false, text: why, snap: None });
                }
            },
            None => {
                let _ = tx.send(NetMsg::BuyLocal { kind, id });
            }
        }
    });
}

/// Drain worker results: adopt balances, top list, shop outcomes. Runs always.
pub fn net_poll(
    mut inbox: ResMut<NetInbox>,
    mut economy: ResMut<crate::economy::Economy>,
    mut best: ResMut<crate::world::Best>,
    settings: Res<crate::settings::Settings>,
    mut top: ResMut<TopScores>,
    mut shop_msg: ResMut<crate::ui::ShopMsg>,
) {
    let mut dead = false;
    let mut msgs = Vec::new();
    if let Some(lock) = inbox.rx.as_ref() {
        let rx = lock.lock().unwrap();
        loop {
            match rx.try_recv() {
                Ok(m) => msgs.push(m),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    dead = true;
                    break;
                }
            }
        }
    } else {
        return;
    }
    if dead {
        inbox.rx = None;
        top.pending = false; // worker died offline: show offline note
    }
    for msg in msgs {
        match msg {
            NetMsg::SubmitDone { snap, rank, total, top: list } => {
                adopt(&mut economy, &mut best, &snap, &settings);
                top.entries = list;
                top.pending = false;
                inbox.online = true;
                if rank > 0 {
                    shop_msg.0 = format!("Score posted! Rank {}/{}", rank, total);
                }
            }
            NetMsg::MeSynced { snap } => {
                adopt(&mut economy, &mut best, &snap, &settings);
                inbox.online = true;
            }
            NetMsg::ShopDone { ok, text, snap } => {
                if let Some(s) = snap.as_ref() {
                    adopt(&mut economy, &mut best, s, &settings);
                }
                shop_msg.0 = text;
                if ok {
                    inbox.online = true;
                }
            }
            NetMsg::BuyLocal { kind, id } => {
                let res = match kind.as_str() {
                    "gear" => economy.buy(crate::economy::UpgradeId::ALL[(id as usize).min(5)]).map(|_| "bought"),
                    "meal" => economy.buy_meal(id as usize).map(|_| "meal upgraded"),
                    "diner" => economy.buy_diner(id as usize).map(|_| "restaurant upgraded"),
                    _ => Err("bad item"),
                };
                shop_msg.0 = match res {
                    Ok(what) => format!("{}! (offline — will sync on next login)", what),
                    Err(why) => why.to_string(),
                };
                crate::save::save_full(&best, &economy, &settings);
            }
        }
    }
}

fn adopt(
    economy: &mut crate::economy::Economy,
    best: &mut crate::world::Best,
    snap: &SnapDelta,
    settings: &crate::settings::Settings,
) {
    economy.coins = snap.coins;
    economy.gems = snap.gems;
    economy.perfect_serves = snap.perfects;
    economy.upgrades.owned = snap.gear;
    economy.upgrades.meals = snap.meals;
    economy.upgrades.diner_lv = snap.diner;
    for i in 0..3 {
        best.scores[i] = best.scores[i].max(snap.best_scores[i]);
        best.stars[i] = best.stars[i].max(snap.best_stars[i]);
    }
    crate::save::save_full(best, economy, settings);
}

#[derive(Resource)]
pub struct TopScores {
    pub entries: Vec<(String, u32, u8)>,
    pub pending: bool,
}

impl Default for TopScores {
    fn default() -> Self {
        Self { entries: Vec::new(), pending: false }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_top_list() {
        let body = r#"[{"player":"Amy","level":0,"score":450,"stars":3,"at":"t"},{"player":"Bo","level":0,"score":300,"stars":2,"at":"t"}]"#;
        let v = parse_top(body);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0], ("Amy".to_string(), 450, 3));
        assert_eq!(v[1].1, 300);
    }

    #[test]
    fn parses_garbage_as_empty() {
        assert!(parse_top("not json").is_empty());
        assert!(parse_top("").is_empty());
    }

    #[test]
    fn parses_snapshot() {
        let body = r#"{"player":"P","token":"t","coins":77,"gems":2,"perfects":9,"gear":[true,false,false,false,false,false],"meals":[1,0,0,0,0,0,0,0],"diner":[0,1,0],"best_scores":[1,2,3],"best_stars":[0,1,2]}"#;
        let s = parse_snap(body).unwrap();
        assert_eq!((s.coins, s.gems, s.perfects), (77, 2, 9));
        assert!(s.gear[0] && !s.gear[1]);
        assert_eq!(s.meals[0], 1);
        assert_eq!(s.diner[1], 1);
        assert_eq!(s.best_scores, [1, 2, 3]);
    }

    #[test]
    fn scores_url_trims_slash() {
        let c = BoardConfig { base_url: "https://x.example/".into(), player: "P".into(), secret: "s".into() };
        assert_eq!(c.scores_url(), "https://x.example/api/scores");
    }
}
