# TokenPress demo page

A static, dependency-free page that runs the Python, Rust and
JavaScript/TypeScript formatters in the browser through
`crates/tokenpress-wasm`. Paste source in, pick the language and flags, and see
the formatted output plus the token savings under both embedded tokenizers
(`o200k_base`, `cl100k_base`).

The language selector has six entries: Python, Rust, and the four
JavaScript/TypeScript dialects (JavaScript, JSX, TypeScript, TSX). The last
four all call `formatJs`, which takes the dialect in its options object —
there is no file behind the boundary for the formatter to read an extension
from. Verification in the browser is internal only (re-parse plus canonical
re-emit equivalence): WebAssembly cannot spawn processes, so the CLI's
`--verify external` (`tsc --noEmit` / `node --check`) has no counterpart here.

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

`.github/workflows/pages.yml` builds this directory on every push to `master`
that touches `site/`, `crates/`, `Cargo.toml`, `Cargo.lock` or
`rust-toolchain.toml` (and on manual dispatch). The build job always runs — it
is the only place CI exercises `site/build.sh`, and it fails outright if the
bundle is missing `pkg/tokenpress_wasm.js` or `pkg/tokenpress_wasm_bg.wasm`. It
then stages a clean document root (`index.html` and the generated `pkg/`, minus
`build.sh`, `.tools/` and this README) and uploads it as a Pages artifact.

The deploy job is gated on `!github.event.repository.private`, so nothing is
published while the repository is private — Pages on a private repository needs
a paid plan, and going public is a human decision. Flipping the repository to
public opens the gate on its own, with no change to the workflow.
