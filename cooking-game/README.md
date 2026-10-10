# Short-Order Showdown (Bevy cooking game) — Step 9 done

Real cooking game, Bevy 0.16, ~9500 lines, 55 game tests + 6 server tests.
KayKit glTF + kitchen SFX + Kevin MacLeod music + procedural menu art.

## Run

```sh
sudo apt-get install -y libasound2-dev libudev-dev pkg-config
cargo run -p cooking-game
cargo test -p cooking-game   # 55 tests
```

## Step 9: server accounts + economy + Android/Windows CI (this step)

- **Server accounts + economy** (`/server`, Axum): `POST /api/accounts`
  (new accounts start with 25 coins), `POST /api/auth` (token rotation),
  `GET /api/me`, authed `POST /api/scores` (server-side coin/gem ledger +
  bests), authed `POST /api/shop/buy` (gear/meal/diner, same prices as the
  client), public `GET /api/items` catalog + leaderboard. Legacy scores.json
  migrates to `sod_db.json`. Passwords: salted SHA-256, tokens 24-byte hex.
- **Client sync**: auto device account (tag + secret in save.txt), menu-time
  balance sync, authed submits adopt server balances/bests, shop buys go
  through the server when online with instant local fallback offline.
- **Android**: `manifest.yaml` (package id, icon, INTERNET permission —
  format verified against the xbuild/apk crate sources), `icon.png`,
  `--features embed-assets` (`bevy_embedded_assets=0.13.0`, the release built
  for Bevy 0.16 — 0.16.0 targets Bevy 0.19), loading screen treats failed
  assets as done so packaging gaps can never softlock.
- **CI** (`.github/workflows/`): `windows.yml` (test + release exe + assets
  artifact), `android.yml` (xbuild 0.2.0 debug APK + artifact).
- **Deploy**: `render.yaml` + `server/Dockerfile` ready; needs the repo public
  (or GitHub connected) so Render can fetch it, then one `render services
  create` + bake the URL into the client default.

## Step 8: server + textures

Axum leaderboard (now extended — see above), textured menu backdrops,
procedural PNGs via `gen_textures.py`.

## Controls (defaults, all rebindable in Settings)

WASD/arrows move | E/Space interact-chop-stir | Q/X trash | R rescue | Esc/P pause | M mute.

## Credits

- Models: KayKit Restaurant Bits 1.0 FREE.
- SFX: free-kitchen-sounds (itch.io, royalty-free).
- Music: Kevin MacLeod — Lobby Time / Monkeys Spinning Monkeys / Happy Boy End Theme (incompetech.com, CC-BY 4.0).
- Menu art + icon: generated (`gen_textures.py`, project-owned).
