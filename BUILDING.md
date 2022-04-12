# Building Radium
1. Install & set up Rust:
   1. Install [Rustup](https://rustup.rs/)
   2. Install the *nightly* toolchain: `rustup install nightly`
   3. Set Rust to use *nightly* for the project: `rustup override set nightly` in the project directory
2. Install 3rd-party dependencies (Linux-only):
   1. Install `build-essentials` or similar (dependent on your platform)
   2. OpenSSL
      - As Rust instructs, `libssl-dev` or `openssl-dev` 
   3. libsqlite3
      - `libsqlite3-dev` seemed to do the trick
3. Build!
   - `cargo build --release`

You will also need to set up and configure LavaLink to use any audio-related commands.
