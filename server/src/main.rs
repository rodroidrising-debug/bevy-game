//! Short-Order Showdown backend (Axum) — Step 9.
//!
//! Public:
//! - GET  /health, GET /, GET /api/scores?level=&limit=, GET /api/items
//! Accounts + authed economy:
//! - POST /api/accounts {player, password} -> {token, ...snapshot}
//! - POST /api/auth     {player, password} -> {token, ...snapshot}
//! - GET  /api/me                          (Bearer) -> snapshot
//! - POST /api/scores   (Bearer) {level, score, stars, grade} -> {rank, total, coins, gems, perfects}
//! - POST /api/shop/buy (Bearer) {kind: gear|meal|diner, id} -> snapshot or error
//!
//! Storage: JSON file (DATA_DIR/sod_db.json), RwLock-guarded. Legacy scores.json
//! (bare array) is migrated on first load.

use std::{
    collections::HashMap,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::get,
    Router,
};
use axum::http::HeaderMap;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

const MAX_ENTRIES: usize = 2000;
const MAX_PLAYER_LEN: usize = 16;
const NEW_ACCOUNT_COINS: u32 = 25;
const PERFECTS_PER_GEM: u32 = 8;

// ---------------------------------------------------------------------------
// types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScoreEntry {
    pub player: String,
    pub level: u8,
    pub score: u32,
    pub stars: u8,
    pub at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct User {
    pub pass_hash: String,
    pub salt: String,
    pub token: String,
    pub coins: u32,
    pub gems: u32,
    pub perfects: u32,
    pub gear: [bool; 6],
    pub meals: [u8; 8],
    pub diner: [u8; 3],
    pub best_scores: [u32; 3],
    pub best_stars: [u8; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Db {
    #[serde(default)]
    pub users: HashMap<String, User>,
    #[serde(default)]
    pub scores: Vec<ScoreEntry>,
}

#[derive(Debug, Deserialize)]
pub struct SubmitScore {
    pub player: String,
    pub level: u8,
    pub score: u32,
    pub stars: u8,
}

#[derive(Debug, Deserialize)]
pub struct AuthedScore {
    pub level: u8,
    pub score: u32,
    pub stars: u8,
    pub grade: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Credentials {
    pub player: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct BuyOrder {
    /// "gear" | "meal" | "diner"
    pub kind: String,
    pub id: u8,
}

#[derive(Debug, Deserialize)]
pub struct TopQuery {
    pub level: Option<u8>,
    pub limit: Option<usize>,
}

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<RwLock<Db>>,
    pub file: PathBuf,
}

// ---------------------------------------------------------------------------
// pure logic (unit-tested)
// ---------------------------------------------------------------------------

pub fn clean_name(raw: &str) -> Result<String, &'static str> {
    let mut name: String = raw
        .trim()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if name.is_empty() {
        return Err("player name required");
    }
    name.truncate(MAX_PLAYER_LEN);
    Ok(name)
}

/// Validate a submission. Returns the cleaned entry or a reason.
pub fn clean_submission(sub: &SubmitScore, now_utc: &str) -> Result<ScoreEntry, &'static str> {
    let name = clean_name(&sub.player)?;
    if sub.level > 2 {
        return Err("level must be 0, 1 or 2");
    }
    if sub.stars > 3 {
        return Err("stars must be 0..=3");
    }
    if sub.score > 99_999 {
        return Err("score out of range");
    }
    Ok(ScoreEntry {
        player: name,
        level: sub.level,
        score: sub.score,
        stars: sub.stars,
        at: now_utc.to_string(),
    })
}

/// Rank of `entry` within `entries` for its level (1-based).
pub fn rank_of(entries: &[ScoreEntry], entry: &ScoreEntry) -> usize {
    let mut better = 0;
    for e in entries {
        if e.level == entry.level && e.score > entry.score {
            better += 1;
        }
    }
    better + 1
}

pub fn top_for(entries: &[ScoreEntry], level: Option<u8>, limit: usize) -> Vec<ScoreEntry> {
    let mut v: Vec<ScoreEntry> = entries
        .iter()
        .filter(|e| level.map_or(true, |l| e.level == l))
        .cloned()
        .collect();
    v.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.at.cmp(&b.at)));
    v.truncate(limit.clamp(1, 100));
    v
}

pub fn hash_password(salt_hex: &str, password: &str) -> String {
    let mut h = Sha256::new();
    h.update(salt_hex.as_bytes());
    h.update(password.as_bytes());
    hex_of(h.finalize())
}

pub fn hex_of(bytes: impl AsRef<[u8]>) -> String {
    bytes.as_ref().iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn random_hex(n_bytes: usize) -> String {
    let mut buf = vec![0u8; n_bytes];
    rand::thread_rng().fill_bytes(&mut buf);
    hex_of(buf)
}

pub fn check_password(user: &User, password: &str) -> bool {
    if password.len() < 4 || password.len() > 64 {
        return false;
    }
    hash_password(&user.salt, password) == user.pass_hash
}

/// Server-side serve payout (mirrors the client): coins + gem cadence.
/// Returns (coins_earned, gem_earned).
pub fn pay_serve(user: &mut User, score: u32, grade: &str) -> (u32, bool) {
    let mut coins = score / 10;
    coins += match grade {
        "S" => 2,
        "A" => 1,
        _ => 0,
    };
    user.coins = user.coins.saturating_add(coins);
    let mut gem = false;
    if grade == "S" {
        user.perfects += 1;
        if user.perfects % PERFECTS_PER_GEM == 0 {
            user.gems = user.gems.saturating_add(1);
            gem = true;
        }
    }
    (coins, gem)
}

// --- shop catalog (source of truth; client mirrors these numbers) ---

pub fn gear_cost(id: u8) -> Option<(u32, u32)> {
    match id {
        0 => Some((60, 0)),
        1 => Some((80, 0)),
        2 => Some((50, 0)),
        3 => Some((70, 0)),
        4 => Some((90, 0)),
        5 => Some((40, 1)),
        _ => None,
    }
}

pub fn meal_cost(level: u8) -> Option<u32> {
    match level {
        0 => Some(40),
        1 => Some(80),
        2 => Some(120),
        _ => None,
    }
}

pub fn diner_cost(level: u8) -> Option<u32> {
    match level {
        0 => Some(100),
        1 => Some(200),
        2 => Some(300),
        _ => None,
    }
}

/// Apply a purchase to a user. Ok(()) or a reason string.
pub fn apply_buy(user: &mut User, kind: &str, id: u8) -> Result<(), &'static str> {
    match kind {
        "gear" => {
            let i = id as usize;
            if i >= 6 {
                return Err("unknown gear id");
            }
            if user.gear[i] {
                return Err("already owned");
            }
            let (c, g) = gear_cost(id).ok_or("unknown gear id")?;
            if user.coins < c {
                return Err("not enough coins");
            }
            if user.gems < g {
                return Err("not enough gems");
            }
            user.coins -= c;
            user.gems -= g;
            user.gear[i] = true;
            Ok(())
        }
        "meal" => {
            let i = (id as usize).min(7);
            if id as usize >= 8 {
                return Err("unknown recipe id");
            }
            if user.meals[i] >= 3 {
                return Err("max level");
            }
            let cost = meal_cost(user.meals[i]).ok_or("max level")?;
            if user.coins < cost {
                return Err("not enough coins");
            }
            user.coins -= cost;
            user.meals[i] += 1;
            Ok(())
        }
        "diner" => {
            let i = (id as usize).min(2);
            if id as usize >= 3 {
                return Err("unknown restaurant id");
            }
            if user.diner[i] >= 3 {
                return Err("max level");
            }
            let cost = diner_cost(user.diner[i]).ok_or("max level")?;
            if user.coins < cost {
                return Err("not enough coins");
            }
            user.coins -= cost;
            user.diner[i] += 1;
            Ok(())
        }
        _ => Err("kind must be gear|meal|diner"),
    }
}

fn snapshot(player: &str, user: &User) -> serde_json::Value {
    serde_json::json!({
        "player": player,
        "token": user.token,
        "coins": user.coins,
        "gems": user.gems,
        "perfects": user.perfects,
        "gear": user.gear,
        "meals": user.meals,
        "diner": user.diner,
        "best_scores": user.best_scores,
        "best_stars": user.best_stars,
    })
}

fn bearer(headers: &HeaderMap) -> Option<String> {
    let v = headers.get(axum::http::header::AUTHORIZATION)?.to_str().ok()?;
    v.strip_prefix("Bearer ").map(|s| s.trim().to_string())
}

// ---------------------------------------------------------------------------
// persistence
// ---------------------------------------------------------------------------

fn now_utc() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

fn load_db(path: &Path) -> Db {
    // new format
    if let Ok(s) = std::fs::read_to_string(path) {
        if let Ok(db) = serde_json::from_str::<Db>(&s) {
            return db;
        }
        // legacy: bare score array
        if let Ok(scores) = serde_json::from_str::<Vec<ScoreEntry>>(&s) {
            tracing::info!("migrating legacy scores.json ({} entries)", scores.len());
            return Db { users: HashMap::new(), scores };
        }
        if !s.trim().is_empty() {
            tracing::warn!("db file corrupt, starting empty");
        }
    }
    // legacy sidecar
    if let Some(parent) = path.parent() {
        let legacy = parent.join("scores.json");
        if legacy != path {
            if let Ok(s) = std::fs::read_to_string(&legacy) {
                if let Ok(scores) = serde_json::from_str::<Vec<ScoreEntry>>(&s) {
                    tracing::info!("migrating legacy scores.json ({} entries)", scores.len());
                    return Db { users: HashMap::new(), scores };
                }
            }
        }
    }
    Db::default()
}

fn save_db(path: &Path, db: &Db) {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    match serde_json::to_string_pretty(db) {
        Ok(s) => {
            if let Err(e) = std::fs::write(path, s) {
                tracing::warn!("failed to persist db: {e}");
            }
        }
        Err(e) => tracing::warn!("failed to serialize db: {e}"),
    }
}

// ---------------------------------------------------------------------------
// handlers
// ---------------------------------------------------------------------------

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "game": "short-order-showdown",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn index(State(state): State<AppState>) -> Html<String> {
    let db = state.store.read().unwrap();
    let mut rows = String::new();
    for e in top_for(&db.scores, None, 25) {
        rows.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            html_escape(&e.player),
            e.level,
            e.score,
            e.stars,
            html_escape(&e.at)
        ));
    }
    Html(format!(
        "<!doctype html><html><head><meta charset=utf-8><title>Short-Order Showdown scores</title>\
        <style>body{{font-family:sans-serif;background:#141118;color:#f5e9d0;max-width:720px;margin:2em auto;padding:0 1em}}\
        table{{border-collapse:collapse;width:100%}}td,th{{border:1px solid #554;padding:.4em}}</style></head>\
        <body><h1>Short-Order Showdown — leaderboard</h1>\
        <p>{} chefs, {} scores. Create an account: <code>POST /api/accounts</code> \
        (<code>{{\"player\":\"Chef-AB12\",\"password\":\"....\"}}</code>), then submit with \
        <code>Authorization: Bearer &lt;token&gt;</code>.</p>\
        <table><tr><th>player</th><th>level</th><th>score</th><th>stars</th><th>at</th></tr>{}</table></body></html>",
        db.users.len(),
        db.scores.len(),
        rows
    ))
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

async fn get_scores(State(state): State<AppState>, Query(q): Query<TopQuery>) -> impl IntoResponse {
    let db = state.store.read().unwrap();
    Json(top_for(&db.scores, q.level, q.limit.unwrap_or(10)))
}

async fn get_items() -> impl IntoResponse {
    Json(serde_json::json!({
        "gear": [
            {"id": 0, "name": "Sharp Knife", "coins": 60, "gems": 0},
            {"id": 1, "name": "Non-stick Pan", "coins": 80, "gems": 0},
            {"id": 2, "name": "Fast Shoes", "coins": 50, "gems": 0},
            {"id": 3, "name": "Long Fuse", "coins": 70, "gems": 0},
            {"id": 4, "name": "Lucky Ladle", "coins": 90, "gems": 0},
            {"id": 5, "name": "Mulligan Charm", "coins": 40, "gems": 1},
        ],
        "meals": {"levels": 3, "bonus_per_level_pct": 10, "costs": [40, 80, 120]},
        "diner": {"levels": 3, "patience_bonus_s": 5, "costs": [100, 200, 300]},
        "vip_multiplier": 1.5,
        "new_account_coins": NEW_ACCOUNT_COINS,
    }))
}

async fn register(State(state): State<AppState>, Json(creds): Json<Credentials>) -> impl IntoResponse {
    let name = match clean_name(&creds.player) {
        Ok(n) => n,
        Err(why) => return err(StatusCode::UNPROCESSABLE_ENTITY, why),
    };
    if creds.password.len() < 4 || creds.password.len() > 64 {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "password must be 4..64 chars");
    }
    let mut db = state.store.write().unwrap();
    if db.users.contains_key(&name) {
        return err(StatusCode::CONFLICT, "name taken");
    }
    let salt = random_hex(16);
    let user = User {
        pass_hash: hash_password(&salt, &creds.password),
        salt,
        token: random_hex(24),
        coins: NEW_ACCOUNT_COINS,
        ..Default::default()
    };
    let snap = snapshot(&name, &user);
    db.users.insert(name, user);
    save_db(&state.file, &db);
    (StatusCode::CREATED, Json(snap)).into_response()
}

async fn login(State(state): State<AppState>, Json(creds): Json<Credentials>) -> impl IntoResponse {
    let name = match clean_name(&creds.player) {
        Ok(n) => n,
        Err(why) => return err(StatusCode::UNPROCESSABLE_ENTITY, why),
    };
    let mut db = state.store.write().unwrap();
    let Some(user) = db.users.get_mut(&name) else {
        return err(StatusCode::UNAUTHORIZED, "unknown player (register first)");
    };
    if !check_password(user, &creds.password) {
        return err(StatusCode::UNAUTHORIZED, "wrong password");
    }
    user.token = random_hex(24); // rotate on each login
    let snap = snapshot(&name, user);
    save_db(&state.file, &db);
    (StatusCode::OK, Json(snap)).into_response()
}

fn authed<'a>(db: &'a mut Db, headers: &HeaderMap) -> Result<(String, &'a mut User), axum::response::Response> {
    let Some(tok) = bearer(headers) else {
        return Err(err(StatusCode::UNAUTHORIZED, "missing Bearer token"));
    };
    let name = db.users.iter().find_map(|(n, u)| (u.token == tok).then(|| n.clone()));
    match name {
        Some(n) => Ok((n.clone(), db.users.get_mut(&n).unwrap())),
        None => Err(err(StatusCode::UNAUTHORIZED, "bad token (login again)")),
    }
}

async fn me(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let mut db = state.store.write().unwrap();
    match authed(&mut db, &headers) {
        Ok((name, user)) => (StatusCode::OK, Json(snapshot(&name, user))).into_response(),
        Err(r) => r,
    }
}

async fn post_scores(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(sub): Json<AuthedScore>,
) -> impl IntoResponse {
    if sub.level > 2 || sub.stars > 3 || sub.score > 99_999 {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "score out of range");
    }
    let grade = sub.grade.as_deref().unwrap_or("B");
    if !matches!(grade, "S" | "A" | "B") {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "grade must be S|A|B");
    }
    let mut db = state.store.write().unwrap();
    let Some(tok) = bearer(&headers) else {
        return err(StatusCode::UNAUTHORIZED, "missing Bearer token");
    };
    let Some(name) = db.users.iter().find_map(|(n, u)| (u.token == tok).then(|| n.clone())) else {
        return err(StatusCode::UNAUTHORIZED, "bad token (login again)");
    };
    let entry = ScoreEntry {
        player: name.clone(),
        level: sub.level,
        score: sub.score,
        stars: sub.stars,
        at: now_utc(),
    };
    let rank = rank_of(&db.scores, &entry);
    let total = db.scores.iter().filter(|e| e.level == entry.level).count() + 1;
    db.scores.push(entry);
    if db.scores.len() > MAX_ENTRIES {
        db.scores.sort_by(|a, b| b.score.cmp(&a.score));
        db.scores.truncate(MAX_ENTRIES);
    }
    let user = db.users.get_mut(&name).unwrap();
    let (coins, gem) = pay_serve(user, sub.score, grade);
    let i = (sub.level as usize).min(2);
    if sub.score > user.best_scores[i] {
        user.best_scores[i] = sub.score;
    }
    if sub.stars > user.best_stars[i] {
        user.best_stars[i] = sub.stars;
    }
    if sub.stars >= 3 {
        user.gems = user.gems.saturating_add(1);
    }
    let out = serde_json::json!({
        "rank": rank, "total": total,
        "coins_earned": coins, "gem_earned": gem,
        "coins": user.coins, "gems": user.gems, "perfects": user.perfects,
        "best_scores": user.best_scores, "best_stars": user.best_stars,
    });
    save_db(&state.file, &db);
    info!("{name} scored {} (rank {rank}/{total})", sub.score);
    (StatusCode::CREATED, Json(out)).into_response()
}

async fn shop_buy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(order): Json<BuyOrder>,
) -> impl IntoResponse {
    let mut db = state.store.write().unwrap();
    let (name, user) = match authed(&mut db, &headers) {
        Ok(v) => v,
        Err(r) => return r,
    };
    match apply_buy(user, &order.kind, order.id) {
        Ok(()) => {
            let snap = snapshot(&name, user);
            save_db(&state.file, &db);
            (StatusCode::OK, Json(snap)).into_response()
        }
        Err(why) => err(StatusCode::PAYMENT_REQUIRED, why),
    }
}

fn err(status: StatusCode, why: &str) -> axum::response::Response {
    (status, Json(serde_json::json!({ "error": why }))).into_response()
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/health", get(health))
        .route("/api/scores", get(get_scores).post(post_scores))
        .route("/api/items", get(get_items))
        .route("/api/accounts", axum::routing::post(register))
        .route("/api/auth", axum::routing::post(login))
        .route("/api/me", get(me))
        .route("/api/shop/buy", axum::routing::post(shop_buy))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "data".to_string());
    let file = PathBuf::from(data_dir).join("sod_db.json");
    let state = AppState {
        store: Arc::new(RwLock::new(load_db(&file))),
        file,
    };

    let app = router(state);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(player: &str, level: u8, score: u32) -> ScoreEntry {
        ScoreEntry {
            player: player.into(),
            level,
            score,
            stars: 3,
            at: "2026-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn rejects_bad_submissions() {
        assert!(clean_submission(&SubmitScore { player: "  ".into(), level: 0, score: 10, stars: 1 }, "t").is_err());
        assert!(clean_submission(&SubmitScore { player: "ok".into(), level: 5, score: 10, stars: 1 }, "t").is_err());
        assert!(clean_submission(&SubmitScore { player: "ok".into(), level: 0, score: 10, stars: 9 }, "t").is_err());
        assert!(clean_submission(&SubmitScore { player: "!!! ...".into(), level: 1, score: 100, stars: 2 }, "t").is_err());
    }

    #[test]
    fn cleans_names() {
        let e = clean_submission(
            &SubmitScore { player: "  Chef <Bob>!  ".into(), level: 2, score: 500, stars: 3 },
            "2026-05-01T00:00:00Z",
        )
        .unwrap();
        assert_eq!(e.player, "ChefBob");
        assert_eq!(e.level, 2);
    }

    #[test]
    fn ranks_and_tops() {
        let v = vec![entry("a", 0, 100), entry("b", 0, 300), entry("c", 1, 200)];
        assert_eq!(rank_of(&v, &entry("me", 0, 250)), 2);
        let top = top_for(&v, Some(0), 10);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].player, "b");
        let all = top_for(&v, None, 2);
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn password_roundtrip() {
        let salt = random_hex(16);
        let h = hash_password(&salt, "secret123");
        let user = User { pass_hash: h, salt, ..Default::default() };
        assert!(check_password(&user, "secret123"));
        assert!(!check_password(&user, "wrong"));
        assert!(!check_password(&user, "abc")); // too short
    }

    #[test]
    fn economy_payouts_and_gems() {
        let mut u = User::default();
        let (c, g) = pay_serve(&mut u, 130, "S");
        assert_eq!((c, g), (15, false));
        for _ in 0..7 {
            pay_serve(&mut u, 100, "S");
        }
        assert_eq!(u.gems, 1);
        assert_eq!(pay_serve(&mut u, 100, "B").0, 10);
    }

    #[test]
    fn shop_rules() {
        let mut u = User { coins: 500, gems: 1, ..Default::default() };
        assert!(apply_buy(&mut u, "gear", 0).is_ok());
        assert_eq!(apply_buy(&mut u, "gear", 0), Err("already owned"));
        assert_eq!(apply_buy(&mut u, "gear", 9), Err("unknown gear id"));
        assert!(apply_buy(&mut u, "meal", 0).is_ok());
        assert!(apply_buy(&mut u, "meal", 0).is_ok());
        assert!(apply_buy(&mut u, "meal", 0).is_ok());
        assert_eq!(apply_buy(&mut u, "meal", 0), Err("max level"));
        assert_eq!(apply_buy(&mut u, "diner", 7), Err("unknown restaurant id"));
        // mulligan needs the gem
        assert!(apply_buy(&mut u, "gear", 5).is_ok());
        assert_eq!(u.gems, 0);
        assert_eq!(apply_buy(&mut u, "weapons", 0), Err("kind must be gear|meal|diner"));
    }
}
