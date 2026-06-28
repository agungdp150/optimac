BINARY := opti-mac
VERSION ?= $(shell cat VERSION 2>/dev/null || echo dev)
GOCACHE ?= $(CURDIR)/.cache/go-build
GOPATH ?= $(CURDIR)/.cache/go-mod

.PHONY: build test security release-check clean install

build:
	GOCACHE="$(GOCACHE)" GOPATH="$(GOPATH)" asdf exec go build -ldflags "-X main.version=$(VERSION)" -o bin/$(BINARY) ./cmd/opti-mac

test:
	GOCACHE="$(GOCACHE)" GOPATH="$(GOPATH)" asdf exec go test ./...

security:
	./scripts/security-scan.sh

release-check:
	./scripts/release-check.sh

clean:
	rm -rf bin

install: build
	install -d "$(HOME)/.local/bin"
	install bin/$(BINARY) "$(HOME)/.local/bin/$(BINARY)"
