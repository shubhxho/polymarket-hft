# Publication

The benchmarks are public at:

https://gist.github.com/shubhxho/1b15d2cd4c0dc4568bb311f627e6db0b

The Gist includes the report, raw CPU measurements, validation log, reproduction instructions and encoded Rust source archive. The decoded archive has SHA-256 `d017f200aa7fb1f09fccf71256beca792c217c1cc81823e34bac71c3b9621b70`.

The full project, including the archived Python prototype, is delivered to the private repository:

https://github.com/shubhxho/polymarket-hft

An isolated checkout at `/private/tmp/polymarket-hft-publish` is used for the commit and push because this workspace does not permit creating `.git`. Build caches, virtual environments, signing secrets and installed binaries are excluded from source control.

There are no submissions to HRT or an external leaderboard. No compatible external benchmark protocol or historical data has been supplied. The publication explicitly identifies the local synthetic methodology and the unvalidated live/MLX paths.
