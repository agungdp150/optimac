# Cleanup comparison baseline

The reported 20 GB versus 2 GB difference is unverified. The two figures cannot be compared directly without the other tool's version, selected categories, and whether each reports logical size, estimated reclaimable storage, or a change in available space. The existing OptiMac recovery action moves files to the same volume and does not reclaim their data blocks.

A read-only inventory can be collected with `optimac clean scan --json`; its results are default-location hints and may be partial. They do not identify what another tool would delete. A fair comparison needs inventories captured before either tool modifies data, or independent equivalent fixtures, plus the other tool's reviewed category report and per-volume before/after measurements. This baseline deliberately makes no parity or reclaimed-space claim.

Existing disposable tests verify inventory is read-only, excludes Apple cache roots, deduplicates hard links, and reports limits. They are functional checks, not a benchmark on representative Intel and Apple Silicon machines. The versioned removal plans, durable deletion journal, reconciliation, and permanent deletion required for Phase 2 are not yet implemented.
