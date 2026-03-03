b:
    cargo build

t:
    cargo test

l:
    cargo clippy -- -D warnings

fmt:
    cargo fmt

check:
    cargo fmt --check && cargo clippy -- -D warnings && cargo test
