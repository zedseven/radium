1. Install & set up Rust
   1. Install rust with [rustup](https://rustup.rs/)
   2. Update rust to use the nightly channel `rustup update nightly`
   3. Set rust to use the nightly toolchain by default `rustup default nightly`
2. Install 3rd party dependencies
   1. install `build-essentials` or similar
   2. openssl
      1. As rust instructs, `libssl-dev` or `openssl-dev` 
   3. libsqlite3
      1. `libsqlite3-dev` seemed to do the trick
3. Build!
   1. `cargo build --release`