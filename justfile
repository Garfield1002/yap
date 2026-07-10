_default:
    @just --list

# Open FILE in bulletmd. With no FILE, bulletmd starts an untitled document.
run FILE="": build
    #!/usr/bin/env bash
    set -euo pipefail
    npm run dev > /tmp/bulletmd-vite.log 2>&1 &
    vite_pid=$!
    # Only ever kill the server this recipe started.
    trap 'kill "$vite_pid" 2>/dev/null || true' EXIT

    ready=0
    for _ in $(seq 100); do
        if curl -sf -o /dev/null http://localhost:1420/; then ready=1; break; fi
        sleep 0.1
    done
    if [[ "$ready" -ne 1 ]]; then
        echo "vite never came up; see /tmp/bulletmd-vite.log" >&2
        exit 1
    fi

    if [[ -n "{{ FILE }}" ]]; then
        ./src-tauri/target/debug/bulletmd "{{ FILE }}"
    else
        ./src-tauri/target/debug/bulletmd
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

# Release bundles for the current platform.
bundle:
    npm run tauri build

# Linux packages. Installing the .deb or .rpm registers bulletmd with the desktop.
# NO_STRIP: linuxdeploy's bundled `strip` is too old for DT_RELR (.relr.dyn)
# relocations in modern distro libs (e.g. Fedora 43), which breaks the AppImage.
bundle-linux:
    NO_STRIP=true npm run tauri build -- --bundles deb,rpm,appimage

# macOS application bundle and drag-to-Applications installer (run on a Mac).
bundle-macos:
    npm run tauri build -- --bundles app,dmg

# Install the bulletmd binary into ~/.cargo/bin.
install:
    # tauri-build embeds dist/ into the binary, so the frontend has to exist first.
    npm run build
    # Without custom-protocol, tauri loads the frontend from devUrl instead of dist/.
    cargo install --path src-tauri --locked --features tauri/custom-protocol
