FROM t348575/muslrust-chef:1.77.1-stable as chef
WORKDIR /app

FROM chef as planner
ADD src src
COPY ["Cargo.toml", "Cargo.lock", "."]
RUN cargo chef prepare --recipe-path recipe.json

FROM chef as builder
COPY --from=planner /app/recipe.json recipe.json
ARG RUSTFLAGS='-C strip=symbols -C linker=clang -C link-arg=-fuse-ld=/usr/local/bin/mold'
RUN RUSTFLAGS="$RUSTFLAGS" cargo chef cook --recipe-path recipe.json --features web_api --bin twitch-mock
ADD src src
COPY ["Cargo.toml", "Cargo.lock", "."]
RUN RUSTFLAGS="$RUSTFLAGS" cargo build --target x86_64-unknown-linux-musl --features web_api --bin twitch-mock

FROM busybox AS runtime
WORKDIR /
COPY --from=builder /app/target/x86_64-unknown-linux-musl/debug/twitch-mock /app
EXPOSE 3000
ENTRYPOINT ["/app"]