# Diffodil 🌼

Git diffs in your browser.

A single-binary application that serves a web UI for browsing git diffs across your repositories.

## Install

Download the binary from the releases page, or build from source:

```sh
cargo install --path .
```

## Use

```sh
diffodil /path/to/repos
```

This starts a server on port 8765 (configurable with `--port`) and serves the UI at `http://localhost:8765`. All git repositories found recursively under the given root path will be available for browsing.

```
Usage: diffodil [OPTIONS] <ROOT>

Arguments:
  <ROOT>  Only git repos below the root will be considered

Options:
  -p, --port <PORT>  The port on which the server will listen [default: 8765]
  -v, --verbose...   Increase verbosity
  -h, --help         Print help
```

## Dev

Prerequisites:

- [Rust](https://rustup.rs/)
- [bun](https://bun.com/)
- [just](https://github.com/casey/just) (optional)

Install frontend dependencies:

```sh
just init
```

For development, run the frontend dev server and the Rust backend in two shells:

```sh
just run-frontend-dev
just run-backend-dev /path/to/root
```

Build the release binary (includes frontend):

```sh
just build
```
