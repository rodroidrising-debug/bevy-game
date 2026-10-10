# ---- build ----
FROM rust:1.89-bookworm AS build
WORKDIR /app
COPY Cargo.toml ./
COPY src ./src
RUN cargo build --release

# ---- run ----
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=build /app/target/release/sod-server /app/sod-server
ENV PORT=3000 DATA_DIR=/data
EXPOSE 3000
CMD ["/app/sod-server"]
