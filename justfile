_default:
    @just --list

# Open FILE in yap. With no FILE, yap shows its file picker.
run FILE="": build
    #!/usr/bin/env bash
    set -euo pipefail
    npm run dev > /tmp/yap-vite.log 2>&1 &
    vite_pid=$!
    # Only ever kill the server this recipe started.
    trap 'kill "$vite_pid" 2>/dev/null || true' EXIT

    ready=0
    for _ in $(seq 100); do
        if curl -sf -o /dev/null http://localhost:1420/; then ready=1; break; fi
        sleep 0.1
    done
    if [[ "$ready" -ne 1 ]]; then
        echo "vite never came up; see /tmp/yap-vite.log" >&2
        exit 1
    fi

    if [[ -n "{{ FILE }}" ]]; then
        ./src-tauri/target/debug/yap "{{ FILE }}"
    else
        ./src-tauri/target/debug/yap
    fi

# Compile the Rust binary that `run` launches.
build:
    cargo build --manifest-path src-tauri/Cargo.toml

# Everything that can be checked without a display.
check: test typecheck

test:
    npx vitest run
    cargo test --manifest-path src-tauri/Cargo.toml

typecheck:
    npx svelte-check --tsconfig ./tsconfig.json

# Release bundle (.rpm, AppImage).
bundle:
    npm run tauri build
