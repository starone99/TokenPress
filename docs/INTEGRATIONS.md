# Integrations

Running TokenPress as a gate rather than by hand: the pre-commit hook, the
GitHub Action, the config file, and the cargo features that decide which
backends get built.

**Before wiring any of these into CI, read the gate in the
[README](../README.md#does-a-human-read-this-code) first.** `format` mode
rewrites your source, and there is no un-format.


Three adoption surfaces, in the shape ruff/clippy/eslint users already expect:
a pre-commit hook, a GitHub Action, and a project config file. All three drive
the same CLI and share its exit codes.

### pre-commit

TokenPress ships hook definitions for the [pre-commit](https://pre-commit.com)
framework (`.pre-commit-hooks.yaml`). Add them to the consuming repository's
`.pre-commit-config.yaml`:

```yaml
repos:
  - repo: https://github.com/starone99/TokenPress
    rev: v0.1.0                  # pin a real tag or a full commit SHA
    hooks:
      - id: tokenpress-check
      # - id: tokenpress-format  # …or rewrite instead of only reporting
```

`rev: v0.1.0` above is a placeholder: pin an actual released tag or commit SHA,
never a branch — pre-commit clones the repository at that ref, and the hook
obtains the CLI from that clone.

**A tag and a SHA are not equally cheap.** On a tag the hook downloads that
release's binary, checks it against the release's `SHA256SUMS` and caches it
under the clone's `target/prebuilt/<tag>/`: seconds, and none of the
prerequisites below apply. On a SHA there is no release binary corresponding to
the pin, so the CLI is compiled from the clone instead — correct, and the
reason the prerequisites exist. Pinning a tag is therefore the recommendation
rather than merely one of two equal options.

| Hook | Runs | Result |
|---|---|---|
| `tokenpress-check` | `tokenpress check <staged files>` — writes nothing | Fails when any file is not in normalized form (CLI exit 1) |
| `tokenpress-format` | `tokenpress format <staged files>` — rewrites in place | pre-commit reports the run as failed whenever it had something to rewrite; re-stage and commit again |

Pick one. Enabling both is redundant: `tokenpress-check` fails the run for
exactly the files `tokenpress-format` then rewrites.

Exit semantics: `check` exits 1 exactly when something would change, which is
what fails the hook. `format` itself exits 0 either way — the run is reported
as failed because files changed on disk, the same "a gate cannot pass silently
on rewritten files" rule as the Action's `mode: format`. Exit 2 (a parse or
verification failure, or an unsupported path) fails the hook too, and nothing
that fails verification is ever written.

Both hooks declare
`files: (\.(py|rs|js|mjs|cjs|jsx|ts|mts|cts|tsx|rb|rake|gemspec|ru|go|java|cs)$|(^|/)(Gemfile|Rakefile)$)`
alongside
`types_or: [python, rust, javascript, jsx, ts, tsx, ruby, go, java, "c#"]`,
so only files the CLI accepts ever reach it. pre-commit ANDs the two: a path has to
match the regex *and* carry one of the tags. The regex is the authority;
`types_or` is a coarse pre-filter, so a tag an older `identify` release does not
emit only means a skipped file, never a wrong rewrite. The second regex branch
is what picks up Ruby's extensionless `Gemfile`/`Rakefile`; `.go`, `.java` and
`.cs` need no such branch (`identify` tags them `go`, `java` and `c#` — that
last tag keeps the language's real name even though the CLI's language key,
cargo feature and config table are all spelled `csharp`, and it is quoted in
the YAML because a bare `c#` would start a comment — and the build descriptors
beside them, `go.mod`/`go.sum`, `pom.xml`/`build.gradle`, `.csproj`/`.sln`,
are not source of any of the three).
Extension-less scripts with a Python shebang are excluded on purpose: an
explicitly named unsupported path makes the CLI exit 2. Both are
`require_serial: true` — the first invocation populates the CLI, by download or
by `cargo build`, and parallel copies would only race to write the same path or
contend for the same cargo lock.
`minimum_pre_commit_version` is 2.9.0.

Prerequisites for the consumer. **All of them are prerequisites of the source
build**, so pinning a tag on a host the releases cover removes every one of
them; what stays is `curl` or `wget`, and `sh` on Windows:

- **A working `cargo` on `PATH`** — rustup is the easiest route. The hooks are
  `language: script`; the entry script builds `tokenpress-cli` inside
  pre-commit's own clone of this repository, with the working directory there
  so `rust-toolchain.toml` pins the compiler (rustup then installs it on first
  use). The first hook run therefore pays one release build; later runs reuse
  that clone's `target/`.
- **When the source build is what runs.** Four cases, and only the last is a
  choice: the pin is not a tag; the host has no release archive (Windows, and
  every non-x86_64 Linux — the release ships Linux x86_64, macOS on both
  architectures, and Windows x86_64, but `install.sh` unpacks `tar.gz` only);
  the download or its checksum failed, which is reported on stderr and never
  turned into a refusal to commit; or `TOKENPRESS_NO_PREBUILT=1` is set. The
  `TOKENPRESS_NO_*` backend switches below also force it, because a release
  binary has all four backends linked in.
- **libclang and a C compiler** — the CLI now builds four native backends.
  The Ruby one's `ruby-prism-sys` dependency compiles vendored prism C sources
  and generates its bindings with bindgen (libclang *and* a C compiler); the
  Go, Java and C# ones compile the tree-sitter runtime and their own grammar
  with `cc` (a C compiler, no bindgen — the C# grammar is the one that
  compiles a second C file, its external scanner, which changes nothing about
  the prerequisite). Without libclang the build fails with
  `Unable to find libclang`. On Linux `apt install libclang-dev` (plus `gcc` or
  `clang`); on macOS `xcode-select --install`; on Windows install LLVM
  (`choco install llvm`) and set `LIBCLANG_PATH=C:\Program Files\LLVM\bin`.
  **Neither `ruby` nor the Go toolchain nor a JDK nor a .NET SDK is needed to
  build** — nothing in the build shells out to any of them. They are needed at
  *run* time only if you pass `--verify external`, which runs `ruby -c` over
  Ruby output, `gofmt -e` over Go output, `javac`'s parse gate over Java output
  and Roslyn's `csc` over C# output (for C# that means `dotnet` on PATH: the
  compiler ships inside the SDK, and its version is discovered at run time);
  the hooks do not by default. **Opt out with
  `TOKENPRESS_NO_RUBY=1`, `TOKENPRESS_NO_GO=1`, `TOKENPRESS_NO_JAVA=1` and/or
  `TOKENPRESS_NO_CSHARP=1`**: the hook then builds the CLI without that
  backend's default-on cargo feature, dropping it from the dependency graph.
  Setting just `TOKENPRESS_NO_RUBY=1` removes the libclang requirement;
  setting all four removes the C compiler too. The dropped backend's paths
  become unsupported paths — the hooks' file filter still offers them, so
  exclude them from `files:` if your repository has any.
- **On Windows, `sh` on `PATH`** — the entry point is a `#!/usr/bin/env sh`
  script. Git for Windows provides one.

pre-commit runs hooks from the root of the consuming repository, so a
`tokenpress.toml` there is picked up automatically (see below). Try the hooks
across a whole tree before wiring them into commits:

```bash
pre-commit run --all-files
```

### GitHub Action

Add the gate to an existing workflow with one step — the composite action
obtains the CLI from its own pinned checkout, so nothing has to be installed
first:

```yaml
- uses: starone99/TokenPress@v0.1.0
  with:
    paths: src tests          # default `.`; directories are walked, .gitignore-aware
    mode: check               # default; `format` rewrites in place
    extra-args: --rs-strip-doc-comments   # optional, passed through verbatim
```

**Pinned to a `v…` tag on a Linux or macOS runner, the action downloads that
release's binary** — verified against the release's `SHA256SUMS`, no compiler
involved — **and the whole paragraph below does not apply.** It applies when
the action compiles instead: a branch or SHA in `uses:`, a Windows runner
(no `tar.gz` archive), or any of the four backend inputs switched off, which
asks for a binary no release ships. A download that fails here fails the step
rather than falling back to a build, because a workflow that pinned a release
and cannot have it should say so instead of quietly spending minutes
compiling.

**The runner needs libclang and a C compiler** *for that build.* The action
builds the CLI from its own checkout, and that build includes all four
native backends: the
Ruby one (`ruby-prism-sys`: vendored C + bindgen) and the Go, Java and C# ones
(the tree-sitter runtime and their grammars: C via `cc`). GitHub-hosted Ubuntu
runners generally ship both; Windows runners may need LLVM installed
(`choco install llvm`, with `LIBCLANG_PATH` pointing at it). A composite action
cannot install toolchain prerequisites into the job that uses it, so this
action does not try to — provide your own step if your runner lacks them (this
repository's own CI uses a local `.github/actions/libclang` composite action for
exactly that, and consumers need their own equivalent). None of Ruby, the Go
toolchain, a JDK or a .NET SDK is needed to build; they are needed at run time
only if `extra-args` selects `--verify external`, which runs `ruby -c` over
Ruby output, `gofmt -e` over Go output, `javac`'s parse gate over Java output
and Roslyn's `csc` — reached through `dotnet` — over C# output
(GitHub-hosted runners preinstall all four). **Or drop the requirement with
`ruby: 'false'`, `go: 'false'`, `java: 'false'` and/or `csharp: 'false'`**: the
action then builds the CLI without that backend's default-on cargo feature, so
it is not in the dependency graph. `ruby: 'false'` alone drops the libclang
requirement; all four together drop the C compiler as well, leaving a
pure-Rust build. The
dropped backend's paths are unsupported paths in that build — skipped by the
directory walk, an error (exit 2) when named explicitly.

As a standalone gate workflow:

```yaml
name: TokenPress

on:
  push:
    branches: [main]
  pull_request:

jobs:
  tokenpress:
    name: Token-normalized form
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: starone99/TokenPress@v0.1.0
        with:
          paths: src tests
```

`check` fails the step when anything would change and writes nothing. `format`
rewrites files and then *also* fails, so a gate cannot pass silently on
rewritten files. That makes an autocommit flow explicit: run the step with
`continue-on-error: true` and branch on its `changed` output.

```yaml
jobs:
  tokenpress:
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v4
      - id: tokenpress
        uses: starone99/TokenPress@v0.1.0
        continue-on-error: true
        with:
          mode: format
      - if: steps.tokenpress.outputs.changed == 'true'
        run: |
          git config user.name 'github-actions[bot]'
          git config user.email '41898282+github-actions[bot]@users.noreply.github.com'
          git commit -am 'style: tokenpress format'
          git push
```

Inputs:

| Input | Default | Meaning |
|---|---|---|
| `mode` | `check` | `check` reports and fails, writing nothing. `format` rewrites in place and then fails if it had something to rewrite. Any other value fails the step with exit 2. |
| `paths` | `.` | Whitespace-separated files and/or directories, relative to the workspace. Subject to the shell's word splitting and globbing, so `src/*.py` works. |
| `extra-args` | *(empty)* | Extra `tokenpress` flags, passed through verbatim (whitespace-separated), e.g. `--rs-strip-doc-comments --py-strip-comments --js-strip-comments --ruby-strip-comments --go-strip-comments --java-strip-comments --csharp-strip-comments`. |
| `ruby` | `true` | `false` builds the CLI without its default-on `ruby` cargo feature, dropping the Ruby backend and with it the libclang build prerequisite. Ruby paths are then unsupported paths. Any other value fails the step with exit 2. |
| `go` | `true` | `false` builds the CLI without its default-on `go` cargo feature, dropping the Go backend (tree-sitter runtime + grammar, both compiled with `cc`). `.go` paths are then unsupported paths. Any other value fails the step with exit 2. |
| `java` | `true` | `false` builds the CLI without its default-on `java` cargo feature, dropping the Java backend (the `tree-sitter-java` grammar, compiled with `cc`). `.java` paths are then unsupported paths. Independent of `ruby`, `go` and `csharp`: only with all four `false` does the build need no C toolchain at all. Any other value fails the step with exit 2. |
| `csharp` | `true` | `false` builds the CLI without its default-on `csharp` cargo feature, dropping the C# backend (the `tree-sitter-c-sharp` grammar, whose `parser.c` and external `scanner.c` are compiled with `cc`). `.cs` paths are then unsupported paths. Independent of `ruby`, `go` and `java`: only with all four `false` does the build need no C toolchain at all. Any other value fails the step with exit 2. |

Output:

| Output | Meaning |
|---|---|
| `changed` | `'true'` if any file was rewritten (`format`) or would be rewritten (`check`), otherwise `'false'`. Set even when the step fails, so a `continue-on-error` step can gate a follow-up on it. It is `'false'` when the run errored out or had no supported path to process. |

**Directories and explicitly named files are treated differently.** A directory
is handed to the CLI as-is: its walk is `.gitignore`-aware and picks up only
the supported paths (`.py`, `.rs`, `.js`, `.mjs`, `.cjs`, `.jsx`, `.ts`,
`.mts`, `.cts`, `.tsx`, `.rb`, `.rake`, `.gemspec`, `.ru`, `.go`, `.java`,
`.cs`, and files named `Gemfile` or `Rakefile`), so pointing `paths` at a mixed tree is
safe. An
explicitly named file
is *not* filtered by the CLI — an unsupported one is an error (exit 2) — so the
action drops every other path from the argument list itself and logs which ones
it skipped. Extensions are matched with globs; the two extensionless Ruby names
are matched on the basename exactly, so `myGemfile` is skipped while
`lib/Gemfile` is not. A glob over a mixed tree therefore does not abort
the run, and if nothing supported is left the step succeeds with
`changed=false`. A path that is neither a file nor a directory is passed
through, so a typo is reported rather than silently swallowed.

### Configuration file

Project-wide defaults live in a `tokenpress.toml`. Every key is optional; a
missing key means "not configured", never an explicit default.

```toml
# tokenpress.toml

# Tokenizer to optimize for — same spellings as `--tokenizer`:
# o200k_base | cl100k_base | hf:<tokenizer.json> | kimi:<tiktoken.model>
tokenizer = "o200k_base"      # built-in default: o200k_base

# Verification level applied to every output: "reparse" | "ast" | "external"
verify = "ast"                # built-in default: ast

[python]
strip_comments    = false     # --py-strip-comments
strip_docstrings  = false     # --py-strip-docstrings
strip_annotations = false     # --py-strip-annotations
merge_imports     = true      # `false` is the config spelling of --py-no-merge-imports

[rust]
strip_doc_comments = false    # --rs-strip-doc-comments

# Covers TypeScript too — one backend, one option set.
[javascript]
strip_comments = false        # --js-strip-comments

# Covers every Ruby path: .rb/.rake/.gemspec/.ru plus Gemfile and Rakefile.
[ruby]
strip_comments = false        # --ruby-strip-comments

# Covers .go, which is the Go backend's whole path set.
[go]
strip_comments = false        # --go-strip-comments

# Covers .java, which is the Java backend's whole path set. Distinct from
# [javascript] above: the names are close, the backends share nothing.
[java]
strip_comments = false        # --java-strip-comments

# Covers .cs, which is the C# backend's whole path set. The table is spelled
# `csharp`, not `c#`: the language key has to be a bare TOML key and a
# command-line flag, and `#` starts a TOML comment.
[csharp]
strip_comments = false        # --csharp-strip-comments
```

That is the complete schema — there are no other keys. `verify = "external"`
runs the JavaScript/TypeScript toolchain over JS/TS output, `ruby -c` over
Ruby output, `gofmt -e` over Go output, `javac`'s parse gate over Java output
and Roslyn's `csc` over C# output (see **Language support**); for `.py` and
`.rs` it still behaves exactly like `"ast"` and says so on stderr.

A `[ruby]`, `[go]`, `[java]` or `[csharp]` table is a config error naming the
missing feature in a build that switched that backend off (see **Cargo features**
below), rather than being silently ignored.

**Discovery.** Without `--config`, the nearest `tokenpress.toml` found walking
up from the current directory is used; the first one found wins, and having
none at all is not an error. Discovery starts from the working directory, not
from the paths given on the command line. `--config <path>` is accepted by
`format`, `check`, `diff` and `stats`; passing it disables discovery entirely
and the file must exist — a missing one is a hard error.

**Precedence: explicit CLI flag > config file > built-in default.**
`--tokenizer` and `--verify` override their config counterparts. The strip
flags are presence-only booleans, so the command line can only turn them *on*:
the config file is the project baseline, and `strip_comments = false` there
cannot cancel a `--py-strip-comments` passed on the command line (nor can the
command line re-enable import merging that `merge_imports = false` turned off).
The same holds for `[javascript] strip_comments`/`--js-strip-comments`,
`[ruby] strip_comments`/`--ruby-strip-comments`,
`[go] strip_comments`/`--go-strip-comments`,
`[java] strip_comments`/`--java-strip-comments` and
`[csharp] strip_comments`/`--csharp-strip-comments`.

**Config problems fail loudly**, like every other linter-style tool: an unknown
key, a wrong value type, malformed TOML, or an unknown `tokenizer`/`verify`
value is an error naming the offending key, reported before any file is read —
exit 2, nothing written. A discovered config that does not parse fails exactly
as hard as an explicit one.

### Cargo features

Four of the seven backends are native and carry a build prerequisite the
others do not, so each is a default-on cargo feature of `tokenpress-cli` that
can be switched off on its own:

| Feature | Default | Drops | Prerequisite removed |
|---|---|---|---|
| `ruby` | on | `tokenpress-ruby` → `ruby-prism-sys` (vendored prism C + bindgen) | libclang (and its C compiler) |
| `go` | on | `tokenpress-go` → `tokenpress-treesitter` + `tree-sitter-go` (C via `cc`) | the C compiler for the Go grammar |
| `java` | on | `tokenpress-java` → `tokenpress-treesitter` + `tree-sitter-java` (C via `cc`) | the C compiler for the Java grammar |
| `csharp` | on | `tokenpress-csharp` → `tokenpress-treesitter` + `tree-sitter-c-sharp` (C via `cc`: `parser.c` **and** an external `scanner.c`) | the C compiler for the C# grammar |

`go`, `java` and `csharp` share the `tokenpress-treesitter` engine but none
implies another, so a build with only one of them compiles only that grammar —
and the C compiler prerequisite goes only once *all three* are off.

```bash
cargo build -p tokenpress-cli                                                    # all four (default)
cargo build -p tokenpress-cli --no-default-features --features go,java,csharp    # no Ruby
cargo build -p tokenpress-cli --no-default-features --features ruby              # no tree-sitter backend
cargo build -p tokenpress-cli --no-default-features                              # none: pure Rust
```

Because the four features are independent, `--no-default-features` is the
*all-off* build, not the Ruby opt-out — whichever backends you want to keep
have to be named. Dropping a backend removes it completely: its paths become
unsupported paths (skipped by the directory walk, exit 2 when named
explicitly), its `--ruby-strip-comments`/`--go-strip-comments`/
`--java-strip-comments`/`--csharp-strip-comments` flag stops existing, and its
`[ruby]`/`[go]`/`[java]`/`[csharp]` config table becomes an error naming the
missing feature rather than a silently ignored setting.

The consumer-facing equivalents are `TOKENPRESS_NO_RUBY=1` /
`TOKENPRESS_NO_GO=1` / `TOKENPRESS_NO_JAVA=1` / `TOKENPRESS_NO_CSHARP=1` for
the pre-commit hook and `ruby: 'false'` / `go: 'false'` / `java: 'false'` /
`csharp: 'false'` for the Action, both described above; each maps onto exactly
the cargo invocation in the table.

