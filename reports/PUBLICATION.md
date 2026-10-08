# Publication

The benchmarks are public at:

https://gist.github.com/shubhxho/1b15d2cd4c0dc4568bb311f627e6db0b

The Gist includes measured default release and native MLX CPU release/debug results, benchmark chart, validation evidence, documentation and an encoded Rust source archive. REPRODUCE.md in the Gist records the archive SHA-256. Native CPU debug timings are not directly comparable to default release timings.

The measured comparison report adds full-scan and BTree baselines with identical outputs, five rounds, raw batch samples and a chart. Reported lookup wins do not imply that this project beats HRT or another complete trading engine.

The Hugging Face benchmark dataset is published at:

https://huggingface.co/datasets/shubhxho/polymarket-hft-benchmarks

It includes 22,500 viewer-ready raw batches, the measured comparisons, native CPU results, validation evidence, documentation and the source bundle. make package prepares these files. Only synthetic research data is published.

The full project, including the archived Python prototype, is delivered to the private repository:

https://github.com/shubhxho/polymarket-hft

An isolated checkout at `/private/tmp/polymarket-hft-publish` is used for the commit and push because this workspace does not permit creating `.git`. Build caches, virtual environments, signing secrets and installed binaries are excluded from source control.

There are no submissions to HRT or an external leaderboard. No compatible protocol or historical data has been supplied. Publication identifies the synthetic methodology, native CPU validation and outstanding Metal/live gates.
