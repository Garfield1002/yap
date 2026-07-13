_default:
    @just --list

# Open FILE in the native POC. With no FILE, it starts an untitled document.
run FILE="":
    cargo run --release {{ if FILE != "" { "-- " + FILE } else { "" } }}

# Compile the debug binary.
build:
    cargo build

# Everything that can be checked without a display.
check: test typecheck

test:
    cargo test

typecheck:
    cargo check --all-targets

# Release build of the app.
release:
    cargo build --release

# Display-independent retained-model benchmark over a fixture.
bench FILE="fixtures/sample.md":
    cargo build --release --bin model_bench
    ./target/release/model_bench "{{ FILE }}"

# Install the native POC binary into ~/.cargo/bin.
install:
    cargo install --path . --locked
