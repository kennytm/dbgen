#!/bin/sh

set -eux

if [ $(uname) != 'Linux' ]; then
    P=$(dirname "$(realpath "$(dirname "$0")")")
    docker run \
        --volume "$P:/dbgen" \
        --volume "$HOME/.cargo/git:/root/.cargo/git:ro" \
        --volume "$HOME/.cargo/registry:/root/.cargo/registry:ro" \
        --workdir '/dbgen' \
        --rm \
        --network=host \
        kennytm/dbgen-build-env \
        /dbgen/release/release.sh
    exit 0
fi

rustc -vV
cargo build --release --target x86_64-unknown-linux-gnu
strip -s target/x86_64-unknown-linux-gnu/release/dbgen
strip -s target/x86_64-unknown-linux-gnu/release/dbschemagen
