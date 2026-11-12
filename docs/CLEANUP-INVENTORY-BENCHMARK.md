# Inventory benchmark

Run `cargo run --release --locked --example inventory_benchmark`.
The executable creates four temporary roots with 1,000 files each, 256 bytes per
file. Five Quick and five Deep runs verify completeness and exactly 1,024,000
logical bytes. Fixture setup is excluded from timings. Fixtures are removed
afterward; user caches are never scanned.

Measured on macOS aarch64 (Apple Silicon), release build, 2026-11-11:

| Mode | Five elapsed times (ms) |
| --- | --- |
| Quick | 93, 93, 90, 86, 89 |
| Deep | 93, 94, 94, 108, 94 |

These small warm filesystem fixtures validate reproducibility and accounting,
not performance on large real caches. They do not establish UI input-to-frame
p95 or cancellation latency. Intel execution, denied-access/privacy scenarios,
large cold-cache workloads and UI latency measurements remain release-validation
work. OS filesystem calls are blocking and may exceed cooperative budgets.
