# MLX binding patch

`mlx-rs` is the published 0.32.0 Rust wrapper, copied from the Cargo registry with its original licenses. One manifest change sets `mlx-sys`'s `default-features` to false. The wrapper's existing `metal` and `accelerate` features still forward explicitly.

This permits an actual CPU-only MLX build; the upstream dependency otherwise enables Metal through `mlx-sys`'s defaults even when `mlx-rs` defaults are disabled. No kernels or unsafe FFI implementations are modified. The native MLX version remains pinned by mlx-sys to 0.32.2. The original Cargo checksum manifest describes upstream files and predates this local manifest patch.
