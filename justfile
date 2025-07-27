default:
    @just --list

init:
    bun install

build-frontend:
    bun run build

build-backend:
    uv build

build: build-frontend build-backend

run-frontend-dev *args:
    bun run dev --host {{ args }}

run-backend-dev *args:
    uv run diffodil -v {{ args }}

format:
    treefmt
