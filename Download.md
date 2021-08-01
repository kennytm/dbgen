Download and Install
====================

Pre-built binaries (Linux x86_64)
---------------------------------

Pre-built binaries are provided for Linux (with glibc 2.14+) in x86_64 (amd64) architecture.

1. Go to the GitHub releases page <https://github.com/kennytm/dbgen/releases>.

2. Scroll to the **Assets** section.

3. Download the file with name like `dbgen-vX.Y.Z-x86_64-unknown-linux-gnu.tar.xz`.

4. Extract the archive. The executables inside can be run immediately.

    ```sh
    tar xf dbgen-*.tar.xz

    chmod a+x bin/*

    bin/dbgen --help
    ```

Install via cargo
-----------------

On other platforms, `dbgen` can be built from source via `cargo install`.

1. Install a C compiler (e.g. `gcc` or `clang`) and `pkg-config` tool.
    These can typically be found in the system package manager.

2. Install the latest stable
    [Rust compiler with Cargo package manager](https://www.rust-lang.org/tools/install).

3. Once `cargo` is installed, run

    ```sh
    cargo install dbgen
    ```

    to build and install `dbgen` into `~/.cargo/bin/`.
