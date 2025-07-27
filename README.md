# Diffodil 🌼

Git diffs in your browser.

Work in progress! 🚧

# Use

Using [uv](https://github.com/astral-sh/uv):

```sh
uvx git+https://github.com/buntec/diffodil --help

```

(Should work similarly with `pipx`.)

# Dev

Prerequisites:

- [bun](https://bun.com/)
- [uv](https://github.com/astral-sh/uv)

Optional but recommended:

- [just](https://github.com/casey/just)
- [direnv](https://direnv.net/)
- [pixi](https://pixi.sh/latest/)

Install frontend dependencies like this:

```sh
just init
```

To work on frontend and backend, open two shells and do:

```sh
just run-frontend-dev
just run-backend-dev /path/to/root
```

Don't forget to rebuild and commit the production frontend assets when you are done:

```sh
just build-frontend
```

To build the Python wheel:

```sh
just build
```
