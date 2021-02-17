Release procedure
=================

## Pre-check

1. Push master branch to GitHub.
2. Ensure the GitHub Action check passes.
3. Verify the version number in `/Cargo.toml`.

## Build package

4. Update `Dockerfile` and the Docker image if necessary (always use latest stable rustc).
5. Run `release/release.py` to build the Linux binaries.
6. Run `release/package.sh` to package into `*.tar.xz`.

## Publish playground

7. Run `release/playground.sh` to build the WASM module.
8. Run `release/publish-playground.sh` to commit playground into gh-pages branch and push to GitHub pages.

## Publish package

9. `cargo publish`.
10. Create GitHub release.
