- [X] Fix local execution with no copy
- [X] Fix slurm drop impl
- [X] Make container part of Process
- [X] Support generic executor
- [X] Support TOML parsing
- [X] dtor for .maestro_running
- [X] CLI
- [X] Aliased executors
- [X] Make executor part of process definition -> dependencies.txt
- [X] Doc comments?
- [X] Simplify -> default executor always just "default"
- [X] Inheritance
- [X] Attribute macro for main
- [] Python bindings
- [] Literal OR ident as executor -> ident: var, literal: toml
- [] Cache in home dir?
- [] ssh

podman run --rm \
    -v $(pwd):/io:Z -w /io \
    docker://messense/cargo-zigbuild:latest bash -c "
        apt-get update && apt-get install shellcheck && \
        rustup target add \
            x86_64-unknown-linux-musl \
            aarch64-unknown-linux-musl \
            x86_64-apple-darwin \
            aarch64-apple-darwin && \
        mkdir -p /tmp/build && cd /tmp/build && \
        cp -r /io/* /tmp/build && \
        cargo zigbuild --release \
            --target x86_64-unknown-linux-musl \
            --target aarch64-unknown-linux-musl \
            --target x86_64-apple-darwin \
            --target aarch64-apple-darwin && \
        chown -R $(id -u):$(id -g) /io/target
"

apptainer exec --fakeroot --writable \
    --bind $(pwd):/io --workdir /io \
    docker://messense/cargo-zigbuild:latest bash -c "
        apt-get update && apt-get install shellcheck && \
        rustup target add \
            x86_64-unknown-linux-musl \
            aarch64-unknown-linux-musl \
            x86_64-apple-darwin \
            aarch64-apple-darwin && \
        cargo zigbuild --release \
            --target x86_64-unknown-linux-musl \
            --target aarch64-unknown-linux-musl \
            --target x86_64-apple-darwin \
            --target aarch64-apple-darwin && \
        chown -R $(id -u):$(id -g) /io/target
"
