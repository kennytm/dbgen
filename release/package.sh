#!/bin/sh

set -eux

P=$(dirname "$(realpath "$(dirname "$0")")")

mkdir bin
cp $P/target/x86_64-unknown-linux-gnu/release/{dbgen,dbschemagen} bin/
tar cfvJ dbgen-v$(cargo read-manifest | jq -r .version)-x86_64-unknown-linux-gnu.tar.xz bin
rm -r bin
