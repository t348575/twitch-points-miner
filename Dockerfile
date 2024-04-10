FROM node:21-bookworm-slim as frontend
RUN mkdir /dist
COPY frontend /
RUN npm i
RUN npm run build

FROM t348575/muslrust-chef:1.77.1-stable as planner
WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM planner AS builder
WORKDIR /app
ARG RUSTFLAGS='-C target-feature=+crt-static -C link-arg=-s -C strip=symbols -C linker=clang -C link-arg=-fuse-ld=lld'
COPY --from=planner /app/recipe.json recipe.json
RUN RUSTFLAGS="$RUSTFLAGS" cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json --features web_api,analytics
COPY . .
RUN RUSTFLAGS="$RUSTFLAGS" cargo build --release --target x86_64-unknown-linux-musl --features web_api,analytics

FROM scratch AS runtime
COPY --from=frontend /dist /dist
WORKDIR /
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/twitch-points-miner /app
EXPOSE 3000
ENTRYPOINT ["/app"]