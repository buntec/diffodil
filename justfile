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

run-frontend-dev *args:
    bun run dev --host {{ args }}

run-backend-dev *args:
    cargo run -- {{ args }}

format:
    treefmt
