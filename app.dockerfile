FROM node:22-alpine AS frontend
WORKDIR /ui
COPY ui/package.json ui/package-lock.json ./
RUN npm ci
COPY ui .
# vite.config.ts writes to ../dist, so the output lands at /dist
RUN npm run build

FROM alpine:latest AS tz
RUN apk --no-cache add tzdata

FROM t348575/muslrust-chef:1.92.0-stable AS chef
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
RUN RUSTFLAGS="$RUSTFLAGS" cargo build --release --target x86_64-unknown-linux-musl

FROM scratch AS runtime
COPY --from=frontend /dist /dist
COPY --from=tz /usr/share/zoneinfo /usr/share/zoneinfo
WORKDIR /
ENV LOG=info
COPY --from=builder /tpm/target/x86_64-unknown-linux-musl/release/twitch-points-miner /app
EXPOSE 3000
ENTRYPOINT ["/app"]