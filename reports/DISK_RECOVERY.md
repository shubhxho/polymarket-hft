# Local disk recovery

The workspace build failed while writing Rust archives/metadata because the disk was full. Failed workspace artifacts, duplicate release artifacts and unused MLX C examples were removed. Source, fixtures, journals and credentials were preserved.

Both local checkouts now use the ignored `.cargo/config.toml` shared cache at `/private/tmp/polymarket-hft-target`, with one build job. The source manifest disables development debug/incremental output and includes a compact correctness profile. MLX toolchain configuration disables unused C/C++ demo executables in future builds.

`make compact-check` passed in `/Users/shubh/Documents/polymarket-hft`: 42 tests, formatting and Clippy. Reuse that cache with:

```sh
cargo build --profile compact --locked --offline
```

Validated native MLX CPU executables are also installed under `.local/bin`; no rebuild is needed to use them. The generated release cache was removed after its 42 standard/44 native CPU tests, benchmark and codegen checks, while retaining installed binaries and report evidence. Avoid building multiple profiles concurrently on this nearly-full host. Exact free storage changes as other applications write; no indefinite capacity guarantee is implied.
