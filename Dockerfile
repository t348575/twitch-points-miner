FROM node:21-bookworm-slim as frontend
RUN mkdir /dist
COPY frontend /
RUN npm i
RUN npm run build

FROM clux/muslrust as builder
WORKDIR /app
RUN apt update && apt install clang wget -y
RUN wget https://github.com/rui314/mold/releases/download/v2.30.0/mold-2.30.0-x86_64-linux.tar.gz
RUN tar -xvzf mold-2.30.0-x86_64-linux.tar.gz
RUN cp mold-2.30.0-x86_64-linux/bin/mold /usr/local/bin
ARG RUSTFLAGS='-C strip=symbols -C linker=clang -C link-arg=-fuse-ld=/usr/local/bin/mold'
COPY . .
RUN RUSTFLAGS="$RUSTFLAGS" cargo build --release --target x86_64-unknown-linux-musl --features web_api,analytics

FROM busybox AS runtime
COPY --from=frontend /dist /dist
WORKDIR /
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/twitch-points-miner /app
EXPOSE 3000
ENTRYPOINT ["/app"]