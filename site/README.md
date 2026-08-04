# TokenPress demo page

A static, dependency-free page that runs the Python, Rust,
JavaScript/TypeScript, Go and Java formatters in the browser through
`crates/tokenpress-wasm`. Paste source in, pick the language and flags, and see
the formatted output plus the token savings under both embedded tokenizers
(`o200k_base`, `cl100k_base`).

The language selector has eight entries: Python, Rust, the four
JavaScript/TypeScript dialects (JavaScript, JSX, TypeScript, TSX), Go and
Java. The four JS/TS entries all call `formatJs`, which takes the dialect in
its options object — there is no file behind the boundary for the formatter to
read an extension from. Verification in the browser is internal only (re-parse
plus equivalence): WebAssembly cannot spawn processes, so the CLI's
`--verify external` (`tsc --noEmit` / `node --check` for JS/TS, `gofmt -e` for
Go, `javac` for Java) has no counterpart here.

Ruby is deliberately absent, and there is no `formatRuby`: `tokenpress-wasm`
does not depend on `tokenpress-ruby` at all. The Ruby parser (prism) is a
vendored C library whose sources do not build for `wasm32-unknown-unknown`
(`fatal error: 'ctype.h' file not found` — the target ships no libc headers).
The only wasm targets that build script handles are the `wasm32-*wasi*` family,
and `wasm-bindgen` generates no browser bindings for those: run over a
`wasm32-wasip1` build it exits 0 but emits glue with every `#[wasm_bindgen]`
export missing. Ruby is CLI-only; see the
`(b) wasm + demo site` note under `### tokenpress-ruby` in `ROADMAP.md` for the
full investigation.

Go and Java, by contrast, *are* here. Their grammars are C too, but
`tree-sitter-language` already ships a libc shim for `wasm32-unknown-unknown`
(headers under `wasm/include`, `stdio.c`/`stdlib.c`/`string.c` under
`wasm/src`) and advertises it as `links` metadata. The `tree-sitter` runtime's
build script reads that metadata and compiles the shim's sources into the
link; the upstream grammar build scripts do not, so their `src/parser.c`
cannot find `<stdlib.h>`. `build.sh` closes that gap by exporting
`CFLAGS_wasm32_unknown_unknown` with the shim's include directory — see the
comment there, and do not delete it. One export covers every such grammar:
Java needed no build-script change and no second export, which is what makes
the difference from Ruby a property of the C library, not of the language.

No framework, no bundler, no CDN: `index.html`, `style.css` and `app.js` are
served as-is, and everything else is the wasm-bindgen output. Once built, the
page works fully offline — the source you paste never leaves the browser.

## Build

```bash
./site/build.sh
```

Prerequisites: bash, curl, tar, a rustup toolchain, and `jq`.

The script is script-relative (run it from anywhere) and idempotent. It:

1. reads the exact `wasm-bindgen` version from the workspace `Cargo.lock`, so
   the CLI can never disagree with the crate the wasm blob was compiled
   against;
2. installs that version of `wasm-bindgen-cli` into `site/.tools/` — the
   prebuilt GitHub release binary when one exists for the host, otherwise
   `cargo install` at the same pinned version;
3. locates the `tree-sitter-language` wasm libc shim with `cargo metadata` and
   `jq` and exports its headers as `CFLAGS_wasm32_unknown_unknown`, without
   which the C grammars fail to compile (`'stdlib.h' file not found`); the
   registry path is never hardcoded, and a missing package or directory is a
   hard error rather than a silent fallback;
4. builds `tokenpress-wasm` for `wasm32-unknown-unknown` in release mode;
5. runs `wasm-bindgen --target web` and writes the bundle to `site/pkg/`.

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
