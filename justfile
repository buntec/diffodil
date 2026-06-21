default:
    @just --list

init:
    bun install

build-frontend:
    bun run build

build-backend: build-frontend
    cargo build --release

build: build-backend

run *args:
    cargo run --release -- {{ args }}

dev *args:
    just run-frontend-dev & just run-backend-dev {{ args }}; kill %1

run-frontend-dev *args:
    bun run dev --host {{ args }}

run-backend-dev *args:
    cargo run -- {{ args }}

kill-dev:
    -pkill -f "cargo run --"
    -pkill -f "bun run dev"

install: build
    cargo install --path .

# Bump version (patch by default; `just bump minor` or `just bump major`)
# Requires cargo-edit: `cargo install cargo-edit`
bump level="patch":
    cargo set-version --bump {{level}}
    @echo "bumped to $(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')"

format:
    cargo fmt
    bunx prettier --write frontend/src

# Install as a background service (launchd on macOS)
service-install root *args: install
    diffodil install {{ root }} {{ args }}

# Uninstall the background service
service-uninstall:
    diffodil uninstall

# Restart the background service
service-restart:
    diffodil restart

# Print the generated plist without installing
service-print root *args:
    cargo run --release -- install {{ root }} --print {{ args }}
