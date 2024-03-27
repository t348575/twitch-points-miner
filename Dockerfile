FROM clux/muslrust:stable AS chef
USER root
RUN cargo install cargo-chef
RUN apt-get update && apt-get install clang lld -y
WORKDIR /app

FROM chef as planner
WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
WORKDIR /app
ENV RUSTFLAGS='-C target-feature=+crt-static -C link-arg=-s -C strip=symbols -C linker=clang -C link-arg=-fuse-ld=lld'
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json --features api
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl --features api

FROM scratch AS runtime
WORKDIR /
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/twitch-points-miner /app
EXPOSE 3000
ENTRYPOINT ["/app"]