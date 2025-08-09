FROM rust:1.85.0 AS chef
RUN cargo install cargo-chef
RUN apt update && apt install clang wget pkg-config libglib2.0-dev curl -y
RUN wget https://github.com/rui314/mold/releases/download/v2.33.0/mold-2.33.0-x86_64-linux.tar.gz -O mold.tar.gz
RUN mkdir mold && tar -xvzf mold.tar.gz -C mold --strip 1
RUN cp mold/bin/mold /usr/local/bin
WORKDIR /tpm

FROM chef AS planner
ADD mock mock
ADD common common
COPY ["Cargo.toml", "Cargo.lock", "."]
RUN perl -0777 -i -pe 's/members = \[[^\]]+\]/members = ["mock", "common"]/igs' Cargo.toml
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /tpm/recipe.json recipe.json
ARG RUSTFLAGS='-C strip=symbols -C linker=clang -C link-arg=-fuse-ld=/usr/local/bin/mold'
RUN RUSTFLAGS="$RUSTFLAGS" cargo chef cook --recipe-path recipe.json
ADD mock mock
ADD common common
COPY ["Cargo.toml", "Cargo.lock", "."]
RUN perl -0777 -i -pe 's/members = \[[^\]]+\]/members = ["mock", "common"]/igs' Cargo.toml
RUN RUSTFLAGS="$RUSTFLAGS" cargo build --target x86_64-unknown-linux-gnu

FROM ubuntu AS runtime
WORKDIR /
COPY --from=builder /tpm/target/x86_64-unknown-linux-gnu/debug/mock /app
EXPOSE 3000
ENTRYPOINT ["/app"]