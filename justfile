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

format:
    treefmt
