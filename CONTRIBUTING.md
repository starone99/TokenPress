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
