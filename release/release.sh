#!/bin/sh

# release/release.sh builds the dbgen binaries for release to Linux x86_64 via Docker.

set -ex

P=$(dirname "$(realpath "$(dirname "$0")")")

sudo docker run --rm \
    --volume "$P":/dbgen \
    --volume "$HOME/.cargo/git":/root/.cargo/git:ro \
    --volume "$HOME/.cargo/registry":/root/.cargo/registry:ro \
    --workdir /dbgen \
    --network host \
    kennytm/dbgen-build-env \
        cargo build --release --locked \
        -p dbgen \
        -p dbdbgen \
        --target x86_64-unknown-linux-gnu
