# TokenPress Development Rules

## TDD (mandatory)

1. Write a **failing test first** for any new behavior (red).
2. Write the minimal implementation that passes (green).
3. A test that has passed must keep passing — when behavior changes, change
   the test first.

## Coverage gate (mandatory)

- Run before every commit: `.\scripts\coverage.ps1` (Windows) /
  `./scripts/coverage.sh`
- **The gate fails under 100% line coverage.** New code does not merge
  without tests.
- Sole exception: `crates/tokenpress-cli/src/main.rs` (uninstrumentable thin
  entry point; no logic allowed — everything lives in the `cli.rs` library
  and is tested there).
- Do not write unreachable defensive code (`unreachable!` etc.) — redesign
  so the branch cannot exist. If truly unavoidable, comment why and raise it
  in review.

## Build / test

- `cargo build --workspace` / `cargo test --workspace`
- Toolchain: rustc 1.95.0 (`rust-toolchain.toml`). Local Windows uses an
  MSVC host override (`rustup override` — avoids the gnu-host dlltool issue).
- The Python parser is ruff's internal crates (pinned exactly at `=0.0.6`,
  no semver guarantees). All parser API access stays in
  `tokenpress-python/src/parser.rs`.

## Language & docs

- Everything committed to the repository (docs, comments, commit messages)
  is written in English.
- `docs/` — technical design docs, local-only (gitignored).
- `docs/transforms/{python,rust}.md` — per-language transform rule reference
  (cite rule IDs like PY01/RSO1).
- Core invariant: **output that fails verification (re-parse + equivalence)
  is never written.**
