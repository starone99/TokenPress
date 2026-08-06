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

The C compiler half of that is **no longer prism's alone**:
`tokenpress-treesitter` (and every tree-sitter grammar crate a per-language
backend pins) compiles C sources with `cc` too — tree-sitter's `src/lib.c` and
a grammar's generated `parser.c`. That half needs *only* a C compiler: no C++,
and no bindgen, so no libclang. `tokenpress-go`, `tokenpress-java` and
`tokenpress-csharp` are all in the CLI's default build, so removing the Ruby
backend removes the libclang prerequisite but *not* the C-compiler one — only
removing all four does that. (The C# grammar is the first here with a live
external scanner, so `cc` compiles two C files for it rather than one; it is
the same prerequisite, not a new one.)

- **Linux**: `apt install libclang-dev` (or `clang`); the C compiler is `gcc`
  or `clang`.
- **macOS**: `xcode-select --install` — the Command Line Tools ship both, or
  `brew install llvm`.
- **Windows**: install LLVM (`choco install llvm`, or the llvm.org installer)
  and point bindgen at it with `LIBCLANG_PATH=C:\Program Files\LLVM\bin`; the
  C compiler comes from the MSVC build tools. Both of those want
  administrator rights. Without them, the llvm.org release is an NSIS
  installer that takes a target directory and installs per-user quite
  happily — `LLVM-22.1.8-win64.exe /S /D=C:\Users\<you>\LLVM`, then
  `LIBCLANG_PATH=C:\Users\<you>\LLVM\bin` (measured: exit 0, no UAC prompt,
  `bin\libclang.dll` present, `cargo build -p tokenpress-cli` green). Note
  `winget install LLVM.LLVM --scope user` does **not** work — there is no
  user-scope installer in the manifest.

If bindgen cannot find the library, the build fails with
`Unable to find libclang`. Both hosted CI images ship LLVM already, but the
workflows do not rely on that silently: every job that builds the workspace
runs `.github/actions/libclang`, which checks and installs only if missing —
except the `no-native-backends` job, which exists to prove the build below
needs none of them.

#### Windows: `/std:c11` against a pre-10.0.20348 SDK

On Windows the C-compiler half has a second failure mode that looks like a
tree-sitter bug and is not one. `tree-sitter` 0.26 compiles its C with
`-std:c11`, and that flag implicitly turns on MSVC's **conforming
preprocessor** (`/Zc:preprocessor`). Windows SDK headers older than
10.0.20348 do not parse under it, so `cargo build` dies inside the SDK rather
than inside the crate:

```
...\10.0.17763.0\um\oaidl.h(487): error C2059: syntax error: '/'
...\10.0.17763.0\um\propidlbase.h(378): error C2371: 'pvarVal': redefinition
error occurred in cc-rs: command did not execute successfully
```

The clue is the path: every error is in `um\*.h`, none in `tree-sitter-*`.
This blocks `tokenpress-treesitter` and therefore the `go`, `java` and
`csharp` backends; the pure-Rust backends and `tokenpress-ruby` are
unaffected.

The real fix is to install a newer Windows SDK. Until then, switch the
conforming preprocessor back off for the C dependencies only — the vendored C
does not need it:

```powershell
$env:CFLAGS_x86_64_pc_windows_msvc = "-Zc:preprocessor-"
cargo build --workspace
```

Set it in the shell (or per-machine `.cargo/config.toml`) rather than in the
repository: it is a property of one SDK installation, not of the project, and
committing it would impose the traditional preprocessor on everyone else.

This is **not** confined to the native crates: `tokenpress-cli` depends on
`tokenpress-ruby`, `tokenpress-go`, `tokenpress-java` and `tokenpress-csharp`
in its default build,
so even a narrow `cargo build -p tokenpress-cli` — which is exactly what the
pre-commit hook (`scripts/pre-commit-hook.sh`) and the GitHub Action
(`action.yml`) run on a *consumer's* machine — needs both a C compiler and
libclang. The
way out is the CLI's four default-on cargo features, `ruby`, `go`, `java` and
`csharp`, which are independent:

```bash
cargo build -p tokenpress-cli --no-default-features                              # no libclang, no cc
cargo test -p tokenpress-cli --no-default-features                               # suite must pass here too
cargo build -p tokenpress-cli --no-default-features --features go,java,csharp    # no libclang, still cc
cargo build -p tokenpress-cli --no-default-features --features ruby              # no tree-sitter
```

Because there are four of them, `--no-default-features` is the *all-off*
build rather than the Ruby opt-out it used to be: keeping a backend means
naming it. Dropping one drops its crate from the dependency graph entirely;
its paths become unsupported paths, its `--ruby-strip-comments`/
`--go-strip-comments`/`--java-strip-comments`/`--csharp-strip-comments` flag
does not exist, and its `[ruby]`/`[go]`/`[java]`/`[csharp]` table in
`tokenpress.toml` is a config error naming the missing feature. `go`, `java`
and `csharp` share `tokenpress-treesitter` but none implies another, so the C
compiler goes only when all three are off. The consumer-facing escape hatches
are `TOKENPRESS_NO_RUBY=1`, `TOKENPRESS_NO_GO=1`, `TOKENPRESS_NO_JAVA=1` and
`TOKENPRESS_NO_CSHARP=1` for the pre-commit hook and `ruby: 'false'` /
`go: 'false'` / `java: 'false'` / `csharp: 'false'` for the action, both
documented in the README's **Integrations** and **Cargo features** sections —
neither integration can install a toolchain for the consumer: a composite
action cannot add one to the job that uses it. Note the coverage gate measures
the default build only.

The `no-native-backends` job builds and tests only the fully-off configuration;
the single-feature builds are asserted on `cargo tree` alone (each has to be
exactly its own slice of the graph — its own grammar in, the others out),
which is what the features are actually about. The CLI's own suite is what
covers the combinations: it has to pass in all sixteen
`ruby × go × java × csharp` configurations, since each
`#[cfg(feature = ...)]` in `cli.rs` and `config.rs` is a separate code path.
A fourth independent feature doubles that matrix again, so drive it with a
loop over the subsets rather than by hand.

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

**`gofmt` must be on PATH to run the suite too**, on the same terms:
`tokenpress-go` implements `--verify external` by running `gofmt -e`, and its
tests exercise that against real processes. `gofmt` ships inside every Go
distribution (`$(go env GOROOT)/bin/gofmt`, normally already on PATH next to
`go`), and **both CI runners preinstall Go**, so nothing has to be set up for
it. What cannot be arranged on demand (a machine with no `gofmt`, a process
that fails to spawn) goes through the same injectable `Tools` seam.

Note that `gofmt` is probed with `gofmt -h`. It has no `--version` flag, and a
**bare** `gofmt` reads standard input — a probe without an argument would
block rather than fail, so never write one.

**`javac` must be on PATH to run the suite too**, on the same terms:
`tokenpress-java` implements `--verify external` by running
`javac -XDshould-stop.ifNoError=PARSE`, and its tests exercise that against
real processes. Any JDK provides it — CI installs one explicitly with
`actions/setup-java` (Temurin 21, the JDK the gate was measured against)
rather than relying on whatever a runner image happens to ship. What cannot be
arranged on demand (a machine with no `javac`, a process that fails to spawn,
a JDK that stopped honouring the flag) goes through the same injectable
`Tools` seam.

Note that `javac` is probed with `javac -version`. `javac -h` exits 2 (it
wants a native-header output directory), and a **bare** `javac` exits 2 with
`no source files` — either would make a perfectly good JDK look absent. Note
too that `-XD` is javac's internal option namespace and an unrecognised key is
*silently ignored*: `-XDbogus.key=PARSE` runs a full compile and exits 1
without a word about the flag. That is why the checker self-tests the gate
over a built-in valid-but-unresolvable fixture before trusting it, and why
that self-test is not optional.

**`dotnet` must be on PATH to run the suite too**, on the same terms:
`tokenpress-csharp` implements `--verify external` by running Roslyn's `csc`,
and its tests exercise that against real processes. `csc` is not a program on
PATH — it ships inside the SDK as a managed assembly the `dotnet` host runs —
so the checker asks `dotnet --list-sdks` where it is and builds the path to
`<sdk>/Roslyn/bincore/csc.dll` itself. **The SDK version is discovered at run
time and must never be hardcoded**: the directory shape differs between Linux
(`/usr/lib/dotnet/sdk`) and Windows (`C:\Program Files\dotnet\sdk`), and CI
runs both. Any .NET SDK provides it — CI installs one explicitly with
`actions/setup-dotnet` (pinned to 8.0.129, the SDK the gate was measured
against, into an install directory of its own so the newest SDK on the machine
*is* the pinned one) rather than relying on whatever a runner image ships.
What cannot be arranged on demand (a machine with no `dotnet`, a process that
fails to spawn, an SDK whose diagnostics can no longer be read) goes through
the same injectable `Tools` seam. On a local Linux checkout,
`apt-get update && apt-get install -y dotnet-sdk-8.0` is enough; the
`apt-get update` is not optional, since a stale package index 404s.

Note that C#'s gate is the one that does **not** work like the others. C# has
no parse-only compiler mode, so there is nothing to stop `csc` at: run over a
single file with `/nostdlib+` it reports a pile of unresolved-type errors for
any real source. The verdict is therefore the **multiset of `error CS####`
codes**, compared between the input and the output, and it is read from
`csc`'s **stdout** — never from the exit status, which is non-zero for exactly
the noise the design tolerates. Never add a code-range filter: a top-level
statement after a type declaration is `CS8803`, as much a syntax error as
`CS1026` and outside the `CS1xxx` range, and there is a test that fails if
anyone tries. Because the whole verdict is parsed text, the checker self-tests
the gate over a built-in valid-but-unresolvable fixture before trusting it —
requiring both that the fixture's known codes were extracted at all and that
they cancel between the pair — and that self-test is not optional: without it,
a reworded diagnostic format would leave every file comparing an empty
multiset against an empty multiset and passing.

**Getting `ruby` and `gofmt` onto a Windows machine without administrator
rights**, since CI preinstalls both and a local checkout does not: Go ships a
plain `.zip` that needs no installer at all (`go1.26.5.windows-amd64.zip`,
extract anywhere, `gofmt` is in `go\bin`), and RubyInstaller's `.exe` is an
Inno Setup installer that takes a per-user target —
`rubyinstaller-3.3.12-1-x64.exe /verysilent /currentuser
/dir=C:\Users\<you>\toolchains\ruby33 /tasks=noassocfiles,nomodpath,noridkinstall`.
Both were measured this way (exit 0, no UAC prompt); with those two on PATH
plus `LIBCLANG_PATH`, `scripts\coverage.ps1` runs to completion and exits 0.
Without them six `external_verify_*` tests in `tokenpress-cli` fail — a
missing toolchain, not a regression. A JDK is now a **third** local
prerequisite on the same footing, and a .NET SDK a **fourth**; no per-user
Windows install has been measured for either, and the six-test figure predates
both the Java and the C# checker. The .NET SDK is the least awkward of the
four on Windows, since `dotnet-install.ps1` takes `-InstallDir` and needs no
elevation.

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
   (`formatters()`) and add the corresponding CLI flags and
   `tokenpress.toml` table. If the backend carries a build prerequisite the
   pure-Rust ones do not — a C compiler, libclang — it gets its own default-on
   cargo feature in `crates/tokenpress-cli/Cargo.toml` too, the way `ruby`,
   `go`, `java` and `csharp` each have one, plus the matching
   `TOKENPRESS_NO_<LANG>=1`
   hook variable and `<lang>: 'false'` action input. Every such feature
   doubles the number of configurations the CLI suite has to pass in, and each
   one must be green for both `cargo test` and `clippy -D warnings`.
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

TokenPress is licensed under the Apache License, Version 2.0
([LICENSE-APACHE](LICENSE-APACHE)).

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be licensed as above, without any additional terms or
conditions.

## Workspace layout

Cargo workspace with a single distributed binary:

| Crate | Role |
|---|---|
| `tokenpress-core` | Formatter/Tokenizer traits, tokenizer backends (tiktoken, HF, Kimi ranks) |
| `tokenpress-python` | Python: token-stream re-render + transform passes + verification |
| `tokenpress-rust` | Rust: syn token-stream re-render + verification |
| `tokenpress-js` | JavaScript/TypeScript: oxc parse + whitespace-minimal re-emit + verification (built-in and `tsc`/`node`) |
| `tokenpress-ruby` | Ruby: prism parse + whitespace-minimal re-emit over the source bytes + verification |
| `tokenpress-treesitter` | The grammar-agnostic tree-sitter engine: parse gate, equivalence artifact, protected spans, whitespace rewriter |
| `tokenpress-go` | Go: the grammar configuration, path set and comment policy the engine is driven with |
| `tokenpress-java` | Java: the same, for the Java grammar |
| `tokenpress-csharp` | C#: the same, for the C# grammar |
| `tokenpress-cli` | The `tokenpress` binary: discovery, language detection, commands |
| `tokenpress-wasm` | `wasm-bindgen` boundary for the browser demo (Python, Rust, JavaScript/TypeScript, Go, Java and C# — not Ruby, per-tokenizer token stats) |
