# Publication

The benchmarks are public at:

https://gist.github.com/shubhxho/1b15d2cd4c0dc4568bb311f627e6db0b

The Gist includes measured default release and native MLX CPU release/debug results, benchmark chart, validation evidence, documentation and an encoded Rust source archive. REPRODUCE.md in the Gist records the archive SHA-256. Native CPU debug timings are not directly comparable to default release timings.

The full project, including the archived Python prototype, is delivered to the private repository:

https://github.com/shubhxho/polymarket-hft

An isolated checkout at `/private/tmp/polymarket-hft-publish` is used for the commit and push because this workspace does not permit creating `.git`. Build caches, virtual environments, signing secrets and installed binaries are excluded from source control.

There are no submissions to HRT or an external leaderboard. No compatible protocol or historical data has been supplied. Publication identifies the synthetic methodology, native CPU validation and outstanding Metal/live gates.
