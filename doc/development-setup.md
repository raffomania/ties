## 🛠️ Development Setup

Install the dependencies:

- [Latest stable version of Rust](https://www.rust-lang.org/learn/get-started) (An older version might work as well, but is not tested)
- [mkcert](https://github.com/FiloSottile/mkcert#installation)
  - Don't forget to run `mkcert -install`
- [podman](http://podman.io/docs/installation), for conveniently running postgres for development and tests

Install dependencies available via cargo:

```sh
cargo install cargo-run-bin
```

Copy `.env.example` to `.env` and edit it to your liking.

Optional: run `cargo bin just install-git-hooks` to automatically run checks before committing.

In the root of the repository, launch the server:

```sh
cargo bin just watch
```

Then, open [http://localhost:4040] in your browser.
