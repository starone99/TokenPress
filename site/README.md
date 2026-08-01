# TokenPress demo page

A static, dependency-free page that runs the Python and Rust formatters in the
browser through `crates/tokenpress-wasm`. Paste source in, pick the language
and flags, and see the formatted output plus the token savings under both
embedded tokenizers (`o200k_base`, `cl100k_base`).

No framework, no bundler, no CDN: `index.html`, `style.css` and `app.js` are
served as-is, and everything else is the wasm-bindgen output. Once built, the
page works fully offline — the source you paste never leaves the browser.

## Build

```bash
./site/build.sh
```

The script is script-relative (run it from anywhere) and idempotent. It:

1. reads the exact `wasm-bindgen` version from the workspace `Cargo.lock`, so
   the CLI can never disagree with the crate the wasm blob was compiled
   against;
2. installs that version of `wasm-bindgen-cli` into `site/.tools/` — the
   prebuilt GitHub release binary when one exists for the host, otherwise
   `cargo install` at the same pinned version;
3. builds `tokenpress-wasm` for `wasm32-unknown-unknown` in release mode;
4. runs `wasm-bindgen --target web` and writes the bundle to `site/pkg/`.

## Serve locally

The page is an ES module and fetches the `.wasm` file, so it needs a real HTTP
server — opening `index.html` over `file://` will not work.

```bash
cd site
python3 -m http.server 8000
# then open http://localhost:8000/
```

## Build artifacts

`site/pkg/` (the generated JS glue and `.wasm` blob) and `site/.tools/` (the
downloaded `wasm-bindgen` CLI) are build outputs. Both are gitignored and must
never be committed — rebuild them with `./site/build.sh`.

## Deployment

Publishing this page (GitHub Pages or otherwise) is a separate, later task;
this directory only covers building and running it locally.
