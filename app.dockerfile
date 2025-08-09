FROM oven/bun:slim AS frontend
WORKDIR /frontend
COPY frontend /frontend
RUN bun install
RUN bun run build

FROM rust:1.85.0 AS chef
RUN cargo install cargo-chef
RUN apt update && apt install clang wget pkg-config libglib2.0-dev curl -y
RUN wget https://github.com/rui314/mold/releases/download/v2.33.0/mold-2.33.0-x86_64-linux.tar.gz -O mold.tar.gz
RUN mkdir mold && tar -xvzf mold.tar.gz -C mold --strip 1
RUN cp mold/bin/mold /usr/local/bin
WORKDIR /tpm

FROM chef AS planner
ADD app app
ADD common common
COPY ["Cargo.toml", "Cargo.lock", "."]
RUN perl -0777 -i -pe 's/members = \[[^\]]+\]/members = ["app", "common"]/igs' Cargo.toml
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /tpm/recipe.json recipe.json
ARG RUSTFLAGS='-C strip=symbols -C linker=clang -C link-arg=-fuse-ld=/usr/local/bin/mold'
RUN RUSTFLAGS="$RUSTFLAGS" cargo chef cook --release --recipe-path recipe.json
ADD app app
ADD common common
COPY ["Cargo.toml", "Cargo.lock", "."]
RUN perl -0777 -i -pe 's/members = \[[^\]]+\]/members = ["app", "common"]/igs' Cargo.toml
RUN RUSTFLAGS="$RUSTFLAGS" cargo build --release --target x86_64-unknown-linux-gnu

FROM ubuntu AS runtime
COPY --from=frontend /dist /dist
WORKDIR /
ENV LOG=info
COPY --from=builder /tpm/target/x86_64-unknown-linux-gnu/release/twitch-points-miner /app
EXPOSE 3000
ENTRYPOINT ["/app"]
