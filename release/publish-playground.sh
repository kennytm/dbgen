#!/bin/sh

# release/publish-playground.sh copies the playground WASM to the gh-pages branch.

set -eux

git worktree add gh-pages gh-pages
COMMIT="$(git rev-parse HEAD)"
cp dbgen-playground/index.html dbgen-playground/playground_bg.wasm dbgen-playground/playground.js gh-pages
cd gh-pages
git add index.html playground_bg.wasm playground.js
git commit -m "Publish v$(cargo read-manifest | jq -r .version) ($COMMIT)"
git push origin gh-pages
cd ..
git worktree remove gh-pages
