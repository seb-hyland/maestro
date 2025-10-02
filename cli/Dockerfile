FROM messense/cargo-zigbuild:latest

RUN apt-get update && \
    apt-get install -y shellcheck rsync && \
    rm -rf /var/lib/apt/lists/*

ENV HOME="/tmp"

RUN rustup default stable && \
    rustup install stable && \
    rustup update && \
    rustup target add \
    x86_64-unknown-linux-musl \
    aarch64-unknown-linux-musl \
    x86_64-apple-darwin \
    aarch64-apple-darwin
