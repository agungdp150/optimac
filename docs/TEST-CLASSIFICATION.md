# Test classification

`TEST-CLASSIFICATION.json` names every test discovered in the macOS fixture-only library, binary and CLI integration harnesses. `make test-classification` builds with the normal Cargo environment, lists the resulting harnesses without running test bodies, and rejects missing, stale or duplicate entries. `make test` requires this check.

Filesystem tests that passed under Seatbelt are recorded as `fixture_fs` with `review: passed_under_os_sandbox`; this is not manual certification of every call path. Two fake-executable diagnostic tests are `subprocess_audited`. Five recovery tests require production policy and are explicitly ignored in the normal suite; `make test-recovery-isolated` runs their unchanged assertions under the same sandbox. The manifest is macOS-specific because platform-gated tests differ elsewhere.

`make test` runs all normal harnesses under the OS sandbox (85 pass, 4 explicitly ignored). `make test-fast` retains the old selected subset. `make test-concurrent-isolated` verifies separate process lock identities. The sandbox runner fails closed when sandbox-exec cannot be applied; it does not fall back to HOME-only isolation.
