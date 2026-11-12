# Test classification

`TEST-CLASSIFICATION.json` names every test discovered in the macOS fixture-only library, binary and CLI integration harnesses. `make test-classification` builds with the normal Cargo environment, lists the resulting harnesses without running test bodies, and rejects missing, stale or duplicate entries. `make test` requires this check.

Uncertain tests are assigned `fixture_fs` with `review: unreviewed`. This classification is a requirement for isolation, not evidence that their current access is safe. No subprocess is yet certified `subprocess_audited`. Four existing recovery tests remain blocked; this manifest neither ignores nor disables them. The manifest is macOS-specific because platform-gated tests differ elsewhere.
