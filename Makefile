BINARY := optimac
.PHONY: build test security release-check clean install rust-build rust-test
build:
	cargo build --release --locked
	mkdir -p bin
	cp target/release/$(BINARY) bin/$(BINARY)
test:
	cargo fmt --check
	cargo test --locked
	cargo clippy --locked --all-targets -- -D warnings
security:
	./scripts/security-scan.sh
release-check:
	./scripts/release-check.sh
clean:
	cargo clean
	rm -f bin/$(BINARY)
install: build
	install -d "$(HOME)/.local/bin"
	install bin/$(BINARY) "$(HOME)/.local/bin/$(BINARY)"
rust-build: build
rust-test: test
