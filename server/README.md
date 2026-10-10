# Server — Short-Order Showdown leaderboard (Axum)

Axum JSON API + tiny HTML page. Deploy to Render with Docker (repo-root `render.yaml`).

## Local run

```sh
cargo run -p sod-server
# or: cd server && cargo run
# env: PORT=3000 DATA_DIR=./data RUST_LOG=info
curl localhost:3000/health
curl -X POST localhost:3000/api/scores \
  -H 'Content-Type: application/json' \
  -d '{"player":"Chef-AB12","level":0,"score":450,"stars":3}'
curl 'localhost:3000/api/scores?level=0&limit=5'
```

## Deploy on Render (LIVE)

Running at **https://short-order-showdown-scores.onrender.com**
(`render.yaml` blueprint + `server/Dockerfile`, free plan).

```sh
# from the repo root:
render blueprint launch --file render.yaml
# then note the public URL, e.g. https://short-order-showdown-scores.onrender.com
```

Point the game at it (any one of):

- run the game with `LEADERBOARD_URL=https://<your-app>.onrender.com`, or
- add a line to your save file: `server=https://<your-app>.onrender.com`

```sh
LEADERBOARD_URL=https://short-order-showdown-scores.onrender.com cargo run -p cooking-game
```

## Notes

- Storage is `scores.json` under `$DATA_DIR` (default `./data`). Free Render
  instances have ephemeral disks: scores reset on redeploy/restart unless you
  attach a persistent disk (paid) and mount it at `/data` with `DATA_DIR=/data`.
- CORS is open (`*`) so the game client can POST from anywhere.
- Validation: names alnum/`-`/`_` max 16 chars, level 0-2, stars 0-3,
  score ≤ 99999. Rank is computed per level before insert.
