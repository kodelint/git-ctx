# Makefile for git-ctx

.PHONY: all test build clean release-local

all: test build

test:
	cargo test

build:
	cargo build --release

build-all: build-x86_64 build-arm64

build-x86_64:
	rustup target add x86_64-apple-darwin
	cargo build --release --target x86_64-apple-darwin

build-arm64:
	rustup target add aarch64-apple-darwin
	cargo build --release --target aarch64-apple-darwin

package: build-all
	mkdir -p dist
	tar -czf dist/git-ctx-x86_64-apple-darwin.tar.gz -C target/x86_64-apple-darwin/release git-ctx
	tar -czf dist/git-ctx-aarch64-apple-darwin.tar.gz -C target/aarch64-apple-darwin/release git-ctx

clean:
	cargo clean
	rm -rf dist

changelog:
	git cliff --latest > CHANGELOG.md

release-local: test package changelog
	@echo "Local release package prepared in dist/"
