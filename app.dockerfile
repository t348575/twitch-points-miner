FROM oven/bun:slim as frontend
WORKDIR /frontend
COPY frontend /frontend
RUN bun install
RUN bun run build

FROM t348575/muslrust-chef:1.77.1-stable as chef
WORKDIR /app

FROM chef as planner
ADD src src
ADD migrations migrations 
COPY ["Cargo.toml", "Cargo.lock", "diesel.toml", "."]
RUN cargo chef prepare --recipe-path recipe.json

FROM chef as builder
COPY --from=planner /app/recipe.json recipe.json
ARG RUSTFLAGS='-C strip=symbols -C linker=clang -C link-arg=-fuse-ld=/usr/local/bin/mold'
RUN RUSTFLAGS="$RUSTFLAGS" cargo chef cook --release --recipe-path recipe.json --features web_api,analytics --bin twitch-points-miner
ADD src src
ADD migrations migrations 
COPY ["Cargo.toml", "Cargo.lock", "diesel.toml", "."]
RUN RUSTFLAGS="$RUSTFLAGS" cargo build --release --target x86_64-unknown-linux-musl --features web_api,analytics --bin twitch-points-miner

FROM busybox AS runtime
COPY --from=frontend /dist /dist
WORKDIR /
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/twitch-points-miner /app
EXPOSE 3000
ENTRYPOINT ["/app"]