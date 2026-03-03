SHELL := /bin/bash

HIDE ?= @

.PHONY: gen build build-release dev run test test-cov check fix lint clean help

name := "ferroclaw"
tag := $(shell git rev-parse --short HEAD 2>/dev/null || echo "dev")

# Initialize Rust development environment
gen:
	@echo "🦀 Initializing ferroclaw development environment..."
	$(HIDE)rustup update
	$(HIDE)rustup component add rustfmt clippy
	$(HIDE)cargo install cargo-watch 2>/dev/null || true
	$(HIDE)cargo fetch
	-$(HIDE)cp .env.example .env 2>/dev/null || echo "No .env.example found, skipping."
	@echo ""
	@echo "✅ Environment ready."
	@echo "   Edit .env and set FERROCLAW_OPENAI_API_KEY, then run: make dev"

# Build for development
build:
	$(HIDE)cargo build
	@echo "✅ Development build completed."

# Build for production (single binary)
build-release:
	$(HIDE)cargo build --release
	@echo "✅ Release binary: target/release/ferroclaw"

# Development mode — auto-reload on file changes
dev:
	$(HIDE)cargo watch -x run

# Run production binary
run:
	$(HIDE)cargo run --release

# Run tests
test:
	$(HIDE)cargo test
	@echo "✅ Tests passed."

# Run tests with coverage report
test-cov:
	$(HIDE)cargo tarpaulin --out Html --output-dir coverage
	@echo "✅ Coverage report: coverage/tarpaulin-report.html"

# Type check + clippy
check:
	$(HIDE)cargo check
	$(HIDE)cargo clippy -- -D warnings
	@echo "✅ Check and lint completed."

# Format code (auto-fix)
fix:
	$(HIDE)cargo fmt
	@echo "✅ Code formatted."

# Lint check (CI mode, no fixing)
lint:
	$(HIDE)cargo fmt -- --check
	$(HIDE)cargo clippy -- -D warnings
	@echo "✅ Lint passed."

# Clean build artifacts
clean:
	$(HIDE)cargo clean
	@echo "✅ Cleaned."

# Help
help:
	@echo "ferroclaw — Personal AI Assistant in Rust"
	@echo ""
	@echo "Setup:"
	@echo "  make gen            - Initialize Rust environment (first-time setup)"
	@echo ""
	@echo "Development:"
	@echo "  make dev            - Start dev server with auto-reload (cargo watch)"
	@echo "  make build          - Build (debug)"
	@echo "  make build-release  - Build (release, single binary)"
	@echo "  make run            - Run release binary"
	@echo ""
	@echo "Testing & Lint:"
	@echo "  make test           - Run all tests"
	@echo "  make test-cov       - Run tests with HTML coverage report"
	@echo "  make check          - cargo check + clippy"
	@echo "  make fix            - cargo fmt (auto-fix)"
	@echo "  make lint           - fmt + clippy (CI mode)"
	@echo ""
	@echo "Utilities:"
	@echo "  make clean          - Remove build artifacts"
