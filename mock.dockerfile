FROM t348575/rust-chef:1-stable as chef
RUN apt install pkg-config libglib2.0-dev -y
WORKDIR /tpm

FROM chef as planner
ADD mock mock
ADD common common
COPY ["Cargo.toml", "Cargo.lock", "."]
RUN perl -0777 -i -pe 's/members = \[[^\]]+\]/members = ["mock", "common"]/igs' Cargo.toml
RUN cargo chef prepare --recipe-path recipe.json

FROM chef as builder
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