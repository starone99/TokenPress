# Contributing to TokenPress

Thanks for your interest. This document describes the rules that actually gate
a change here. They are enforced by CI (`.github/workflows/ci.yml`), not by
convention.

## The core invariant

**Output that fails verification (re-parse + equivalence) is never written.**

Every language backend must uphold this. A transform that cannot be verified
does not ship, however large the token saving.

## TDD is mandatory

1. Write a **failing test first** for any new behavior (red).
2. Write the minimal implementation that makes it pass (green).
3. A test that has passed must keep passing. When behavior changes
   intentionally, **change the test first**, then the implementation.

Tests live next to the code they cover, in `#[cfg(test)] mod tests` blocks
inside each source file. There is no separate `tests/` directory.

## Coverage gate

Run before every commit:

```bash
./scripts/coverage.sh      # Linux / macOS
```

```powershell
.\scripts\coverage.ps1     # Windows
```

Both wrap `cargo llvm-cov --workspace --fail-under-lines 100`, so **the gate
fails under 100% line coverage**. Install the tool with
`cargo install cargo-llvm-cov` if it is missing; CI installs it via
`taiki-e/install-action`.

The **sole exception** is `crates/tokenpress-cli/src/main.rs` — an
uninstrumentable thin entry point, excluded by the scripts'
`--ignore-filename-regex`. No logic is allowed there: everything lives in the
`tokenpress_cli` library (`crates/tokenpress-cli/src/cli.rs`) and is tested
there.

Do **not** write unreachable defensive code (`unreachable!`, `panic!` on
"impossible" states, and similar) to satisfy the compiler — redesign so the
branch cannot exist. If it is genuinely unavoidable, comment why and raise it
in review.

## Building and testing

```bash
cargo build --workspace
cargo test --workspace
```

The toolchain is pinned by `rust-toolchain.toml` (rustc **1.95.0**, with the
`llvm-tools-preview`, `clippy` and `rustfmt` components); rustup installs it
automatically. The 1.95 floor comes from `ruff_python_parser` 0.0.6.

CI runs the build and test suite on both `ubuntu-latest` and `windows-latest`,
so keep changes platform-neutral — in particular path handling and any
path-string assertions in tests.

### Build prerequisite: a C compiler and libclang

`tokenpress-ruby` depends on `ruby-prism`, whose `-sys` crate compiles the
vendored prism C sources with `cc` and generates its bindings with **bindgen**.
Building the workspace therefore needs a **C compiler** and **libclang**.
**Ruby itself is not needed at build time** — nothing in the build shells out
to `ruby`. (`llvm-tools-preview` from `rust-toolchain.toml` does *not* provide
libclang; it is a different component.)

- **Linux**: `apt install libclang-dev` (or `clang`); the C compiler is `gcc`
  or `clang`.
- **macOS**: `xcode-select --install` — the Command Line Tools ship both, or
  `brew install llvm`.
- **Windows**: install LLVM (`choco install llvm`, or the llvm.org installer)
  and point bindgen at it with `LIBCLANG_PATH=C:\Program Files\LLVM\bin`; the
  C compiler comes from the MSVC build tools.

If bindgen cannot find the library, the build fails with
`Unable to find libclang`. Both hosted CI images ship LLVM already, but the
workflows do not rely on that silently: every job that builds the workspace
runs `.github/actions/libclang`, which checks and installs only if missing —
except the `no-ruby` job, which exists to prove the build below needs neither.

This is **not** confined to the Ruby crate: `tokenpress-cli` depends on
`tokenpress-ruby` in its default build, so even a narrow `cargo build -p
tokenpress-cli` — which is exactly what the pre-commit hook
(`scripts/pre-commit-hook.sh`) and the GitHub Action (`action.yml`) run on a
*consumer's* machine — needs both. The way out is the CLI's default-on `ruby`
cargo feature, its only feature, so nothing else has to be re-enabled by hand:

```bash
cargo build -p tokenpress-cli --no-default-features  # no libclang, no cc
cargo test -p tokenpress-cli --no-default-features   # suite must pass here too
```

That drops `tokenpress-ruby` from the dependency graph entirely; Ruby paths
become unsupported paths, the `--ruby-strip-comments` flag does not exist, and
a `[ruby]` table in `tokenpress.toml` is a config error naming the missing
feature. The consumer-facing escape hatches are `TOKENPRESS_NO_RUBY=1` for the
pre-commit hook and `ruby: 'false'` for the action, both documented in the
README's **Integrations** section — neither integration can install a toolchain
for the consumer: a composite action cannot add one to the job that uses it.
Note the coverage gate measures the default build only.

**`node` must be on PATH to run the suite.** `tokenpress-js` implements
`--verify external` by running the real toolchain (`tsc --noEmit`, falling back
to `node --check`), and its tests exercise that against real processes. Only
`node` is assumed — it is present on both CI runners — so the orchestration
around it (probe order, the missing-tool error, the Windows `tsc.cmd` shim) is
tested through an injectable seam rather than against an installed `tsc`.

**`ruby` must be on PATH to run the suite too** — for the *tests*, not the
build: `tokenpress-ruby` implements `--verify external` by running `ruby -c`,
and its tests exercise that against real processes, exactly as the JavaScript
backend's do. `ruby` is present on both CI runners. Everything the installed
interpreter cannot be made to do on demand (a machine with no `ruby` at all, a
process that fails to spawn) is tested through the same injectable `Tools`
seam.

## Integration surfaces

`.pre-commit-hooks.yaml` (with `scripts/pre-commit-hook.sh`), `action.yml` and
the `tokenpress.toml` schema in `crates/tokenpress-cli/src/config.rs` are
consumer-facing contracts: hook ids, action inputs/outputs, config keys and
exit codes are what other repositories pin against. They are documented in the
README's **Integrations** section — change one and update that section in the
same commit.

## Code style

CI enforces exactly two things, and they must both pass locally:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Clippy warnings are errors. Run `cargo fmt` before committing; no custom
rustfmt configuration is used.

## Adding a language backend

The shape is fixed by the existing backends, `tokenpress-python` and
`tokenpress-rust`:

1. **New crate** `crates/tokenpress-<lang>`, added to the root `Cargo.toml`
   `[workspace] members` and `[workspace.dependencies]`. Inherit
   `version`/`edition`/`license`/`repository` via `.workspace = true`.
2. **Implement `tokenpress_core::Formatter`** (`language()`, `supports()`,
   `format()`) and expose a `<Lang>Options` struct for language-level
   trade-offs.
3. **Isolate parser access in one module.** All ruff parser API access in the
   Python backend stays in `crates/tokenpress-python/src/parser.rs`, because
   the ruff crates are internal components with no semver guarantees and are
   therefore pinned exactly (`=0.0.6`). Do the same for any new parser
   dependency: one module owns it, the rest of the crate sees your own types.
   Pin exactly whenever upstream gives no semver guarantee.
4. **Follow the pipeline**: parse → transform passes → token-stream re-render
   (`emit`) → verification (`verify`) → token accounting. Verification
   re-parses the output and compares it for equivalence against the intended
   token stream/AST; the caller discards anything that fails.
5. **Document the transform rules per language** in
   `docs/transforms/<lang>.md`, with stable rule IDs (`PY01`, `PYO1`, `RS01`,
   `RSO1`, …) that source comments and option doc-comments cite. Note that
   `docs/` is gitignored (local-only), but the rule IDs referenced from code
   are part of the committed surface — keep them consistent.
6. **Register the formatter** in `crates/tokenpress-cli/src/cli.rs`
   (`formatters()`) and add the corresponding CLI flags.
7. **Document any behavior the backend cannot preserve** in `README.md`. The
   Rust backend's dropped `//` comments and re-spaced macro bodies are the
   precedent: known limits are stated, not hidden.

## Language

Everything committed to this repository — code comments, docs, commit
messages, PR descriptions — is written in **English**.

## Pull requests

Before opening one, make sure locally that:

- `cargo fmt --all -- --check` is clean
- `cargo clippy --workspace --all-targets -- -D warnings` is clean
- `cargo test --workspace` passes
- `./scripts/coverage.sh` reports 100%

Keep commits focused and describe *why* in the message, not just *what*.

## Licensing

TokenPress is dual-licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
