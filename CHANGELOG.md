# Changelog

Notable changes. Entries that change the **bytes a file formats to** are
marked **[output]** — those are the ones that can make `tokenpress check`
start failing on an unchanged source tree after an upgrade — see the policy
below.

This project is pre-1.0. It follows [Keep a Changelog](https://keepachangelog.com/)
loosely and does not yet promise semantic versioning.

## Stability: will the output change between versions?

**Yes, and you should expect it to.** TokenPress is pre-1.0 and the emitters
are still being improved, which means a version bump can change the bytes a
file formats to even though nothing about your code changed.

This is not hypothetical. On 2026-08-06 the Rust emitter's handling of doc
comments changed twice in one day — first a correctness fix that forced a
whole doc block to the raw `#[doc = …]` form when any one line needed it
(which costs more tokens than `///`), then a narrowing of that fallback so a
line holding quotes or backslashes could be sugared back. ripgrep's default
run read -18.9%, then -17.9%, then -19.2%. Same input, same flags, three
different outputs.

What that means in practice:

- **`tokenpress check` can start failing after an upgrade** with no change to
  your source. That is the gate doing its job — the canonical form moved. Run
  `format` once and commit the result.
- **Pin the version.** The `rev:` in a pre-commit config and the tag in the
  Action exist for this. Upgrade deliberately, in a commit of its own, so the
  reformatting diff is not mixed into a change you actually made.
- **Savings figures are version-specific.** Every number in
  [benchmarks/RESULTS.md](benchmarks/RESULTS.md) carries the date it was
  measured, and rows that predate an emitter change say so rather than being
  quietly restated.

What does *not* change with a version bump is the invariant: output that fails
re-parse and equivalence is never written, at any version. Improvements move
the token count and the exact bytes; they do not move what is guaranteed about
them.

After 1.0 this section will say something stronger. It does not yet, because
that would not be true.

## Unreleased

Everything so far. There has been no release.

### Added

- Seven language backends: Python, Rust, JavaScript/TypeScript, Ruby, Go, Java
  and C#. A language is called *Supported* only once `--verify external` hands
  its output to that language's own toolchain — `ruby -c`, `gofmt -e`, `javac`
  stopped after the parse phase, Roslyn `csc` under `/nostdlib+`, `tsc
  --noEmit`. Python and Rust have the built-in check only.
- `format`, `check`, `diff` and `stats` commands, with `--json` output for
  `stats`.
- Tokenizer selection: `o200k_base` and `cl100k_base` embedded, plus
  `hf:<tokenizer.json>` and `kimi:<tiktoken.model>` for any downloaded
  vocabulary.
- `tokenpress.toml`, discovered by walking up from the working directory.
- A pre-commit hook and a GitHub Action, both defaulting to `check`.
- A WebAssembly build and an interactive demo page covering every backend
  except Ruby, whose parser does not build for `wasm32-unknown-unknown`.
- `install.sh` and `install.ps1`, which verify the release archive against its
  `SHA256SUMS` before extracting anything.
- Benchmarks over thirteen commit-pinned corpora against six tokenizers, with
  an upstream-test harness that runs a corpus's own suite against a formatted
  copy.
- **The pre-commit hook and the GitHub Action install a release binary when
  the pin is a release tag**, instead of compiling the CLI on the consumer's
  machine every time. Both go through `install.sh`, so the archive is checked
  against the release's `SHA256SUMS` and an unverifiable download installs
  nothing. This removes `cargo`, a C compiler and libclang from the consumer's
  prerequisites in the case that should be the common one.

  A source build still happens, by design, when the pin is a branch or a bare
  commit (no release binary corresponds to it, and a formatter's verdict has
  to come from the revision that was pinned), when the host has no release
  archive (Windows, and every non-x86_64 Linux), or when a smaller binary than
  a release ships is requested through `TOKENPRESS_NO_RUBY` and friends or the
  Action's `ruby`/`go`/`java`/`csharp` inputs. `TOKENPRESS_NO_PREBUILT=1`
  forces it outright. The hook treats a failed download as a reason to build;
  the Action treats it as a reason to fail the step.
- `SECURITY.md`: private reporting route, the threat model for a tool that
  rewrites source files in place, and what release integrity does and does not
  cover.
- **Releases ship Linux x86_64, Apple Silicon macOS and Windows x86_64.**
  There is no Intel macOS archive: building it natively needs a `macos-13`
  runner and GitHub is retiring the Intel images — the first v0.1.0 attempt
  sat queued on that label for over an hour while every other target finished
  in minutes. Intel Macs build from source with
  `cargo install --git https://github.com/starone99/TokenPress tokenpress-cli`,
  which is what `install.sh` now tells them by name rather than through its
  generic unsupported-host message. Cross-compiling was rejected: it would
  publish an artifact no job has ever run, and the C dependencies here are
  where that fails quietly.

### Changed

- **[output]** The Rust emitter's doc-comment handling changed twice on
  2026-08-06. A doc block in which one line needed the raw `#[doc = …]`
  fallback was being emitted mixed, which misindents a doc example; the whole
  block is now emitted in one form. The fallback was then narrowed, so a doc
  line holding quotes or backslashes is sugared back to `///`. ripgrep's
  default run went -18.9% → -17.9% → -19.2% across the three revisions.
- **[output]** Comment-only files are no longer refused under
  `--go-strip-comments`, `--java-strip-comments` and `--csharp-strip-comments`.
  A file whose entire content is comments used to be emptied and then rejected
  by the equivalence check; it now formats. csvhelper's aggressive run moves
  458 → 459 files.
- The C# external checker now passes `/preferreduilang:en-US`, so a diagnostic
  quoted into an error message reads the same regardless of the machine's UI
  language.
- Licensing moved from `MIT OR Apache-2.0` to **Apache-2.0 only**. Apache-2.0
  is incompatible with GPLv2, so a GPLv2 project can no longer take this code;
  the explicit patent grant is unaffected.

### Fixed

- A mixed sugared/raw Rust doc block changed what a doc example asserted —
  found by running ripgrep's own test suite against a formatted copy (1108 of
  1109 tests matched; the one divergence was this). Token/AST equivalence is
  structurally blind to it.
- The release workflow packaged `tar.gz` only on Linux, which would have made
  the macOS jobs upload nothing and fail the build.

### Known limitations

- Line numbers are not preserved by any backend, at any setting.
- Rust drops `//` and `/* */` comments even at default settings; JS/TS drops
  trailing and expression-position comments.
- Whether stripping comments and docstrings degrades a model's answers has not
  been measured. The protocol is pre-registered in
  `benchmarks/quality-eval/DESIGN.md` and has not been run.
