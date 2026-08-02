# TokenPress

> A token-aware formatter for Python, Rust and JavaScript/TypeScript that
> minimizes LLM token usage while preserving behavior.

TokenPress is a token-aware source code formatter for LLMs. Unlike a minifier that
shrinks characters, TokenPress optimizes against an actual LLM tokenizer —
the output is the equivalent program that costs the fewest input tokens.

```text
minimize  tokenizer.encode(transformed_code)
s.t.      the transformed code parses, compiles, and behaves identically
```

Output that fails verification (re-parse + AST/token equivalence) is never
written — that is the project's core invariant.

**The intended reader of TokenPress output is a model, not a person.** The use
case is the machine-consumed copy of a codebase: the repo or file you paste
into a prompt, hand to an agent's context window, or feed into a RAG index. For
a human reader, formatting and comments are value; in a machine-only copy they
are billed tokens. Run TokenPress on the copy bound for the model and keep the
original for humans — and note that TokenPress never renames an identifier
(variable, function, type) and never edits the contents of a string; only
whitespace, newlines, comments, docstrings and annotations are ever touched.

## Measured savings

Full corpora, every file passing verification. See
[benchmarks/RESULTS.md](benchmarks/RESULTS.md) for methodology, corpus pins,
and all tokenizers.

| Corpus | Setting | GPT-4o/o-series (`o200k_base`) | Qwen3.6 | GLM-5.2 | Kimi K3 |
|---|---|---|---|---|---|
| requests v2.32.3 | default | -9.0% | -9.8% | -9.0% | -8.7% |
| requests | aggressive¹ | -20.6% | -21.0% | -20.7% | -20.2% |
| ripgrep 14.1.1 | default | -18.9% | **-23.2%** | -18.3% | -18.2% |
| ripgrep | aggressive² | -38.2% | **-42.7%** | -38.1% | -37.9% |
| express v5.2.1 | default | -17.3% | *pending* | *pending* | *pending* |
| express | aggressive³ | -25.4% | *pending* | *pending* | *pending* |

¹ `--py-strip-comments --py-strip-annotations` ² `--rs-strip-doc-comments`
³ `--js-strip-comments`

*pending* means not measured yet: the open-model tokenizer numbers
(Qwen3.6/GLM-5.2/Kimi K3) for express need Hugging Face tokenizer downloads
that have not been run for this corpus. See
[benchmarks/RESULTS.md](benchmarks/RESULTS.md), which uses the same *pending*
convention.

The default setting is context-lossless for Python: comments, docstrings and
type annotations are all kept; only syntactic noise (whitespace, blank lines,
indentation width) is minimized and adjacent imports are merged.

**Rust is not context-lossless, even at default settings.** The Rust backend
re-emits from the `syn` token stream, which does not carry regular comments:
`//` and `/* */` comments are **always** dropped. Only doc comments (`///`,
`//!`) survive — they are `#[doc = "..."]` attributes — and only unless
`--rs-strip-doc-comments` is passed. Part of the measured Rust savings above
therefore comes from discarded comments, not from syntactic noise alone.

**JavaScript/TypeScript is not context-lossless at default settings either,
and the loss is partial rather than total.** The JS/TS backend re-emits through
oxc's code generator: trailing comments and comments in expression position are
always dropped, with or without `--js-strip-comments`; only leading
statement-level comments, jsdoc (`/** */`), annotation comments (such as
`#__PURE__`) and legal comments (`//!`, `/*!`, `@license`, `@preserve`)
survive. Part of express's default savings above therefore comes from
discarded comments.

Savings differ per tokenizer — the reason this is a token-aware formatter,
not a character minifier.

## Language support

| Language | Extensions | Status |
|---|---|---|
| Python | `.py` | Supported |
| Rust | `.rs` | Supported (with the comment/macro caveats below) |
| JavaScript / TypeScript | `.js` `.mjs` `.cjs` `.jsx` `.ts` `.mts` `.cts` `.tsx` | Supported (with the comment/JSX caveats below) |

**`--verify external` is real for JavaScript/TypeScript**, which is what
"Supported" is gated on here: the output is handed to the language's own
toolchain, on top of the built-in AST-equivalence check. It runs
`tsc --noEmit --noCheck --skipLibCheck --allowJs --jsx preserve` over the
candidate — one syntax-only command covering all eight extensions — and falls
back to `node --check` when no `tsc` is on PATH. The fallback reads only
`.js`, `.mjs` and `.cjs`; for a `.jsx`/`.ts`/`.mts`/`.cts`/`.tsx` file with no
`tsc` available the run **fails naming the missing tool** rather than quietly
checking less than you asked for, so `--verify external` requires `tsc` (or,
for plain JavaScript, `node`) on PATH. The candidate is checked in a private
temp file that carries the target's extension; nothing is written to your file
until every check has passed. Python and Rust do not implement the level yet
and still treat it as `--verify ast`; the CLI says so on stderr when a `.py` or
`.rs` file is in the run.

If your *input* does not pass the external checker (a file the toolchain
already rejects — ESM syntax in a `.cjs`, a syntax newer than your `tsc`), the
output is not checked against it and the file is accepted on the built-in
equivalence check alone: TokenPress does not fail a run over a file that was
already broken before it ran. Expect the level to be substantially slower than
`--verify ast` — it spawns a probe and two checker processes per file.

`.jsx` and `.tsx` are accepted, with one caveat that limits what they can save:
**JSX text is never compressed.** Whitespace inside element children is
semantically significant, so it is re-emitted verbatim — a JSX file saves
tokens only on the JavaScript/TypeScript around its markup. A round trip that
did squeeze that whitespace out would fail the equivalence check and never be
written. `.d.ts` is covered by `.ts`.

**JS/TS output is not comment-preserving, even at default settings** — the
same honesty class as the Rust `//` comment loss above. Trailing comments and
comments in expression position are **always** dropped when re-emitting, with
or without `--js-strip-comments`. Only leading statement-level comments, jsdoc
(`/** */`), annotation comments (such as `#__PURE__`) and legal comments
(`//!`, `/*!`, `@license`, `@preserve`) survive. That is a property of the code
generator, not an option, and verification cannot detect it because its
canonical form is comment-free by construction. In JSX the one comment
construct the strip flag reaches is a comment-only expression container:
`{/* c */}` becomes `{}` under `--js-strip-comments`, which is valid JSX and
renders identically. The CLI prints these caveats on stderr once per run that
touches a JS/TS file.

## Usage

```bash
tokenpress format <PATH>...        # rewrite in place (dirs walk recursively)
tokenpress check  <PATH>...        # CI gate: exit 1 if anything would change
tokenpress diff   <PATH>...        # unified diff, writes nothing
tokenpress stats  <PATH>... [--json]

# tokenizer selection
tokenpress stats . --tokenizer o200k_base          # default (GPT-4o/o-series)
tokenpress stats . --tokenizer cl100k_base         # GPT-4 / GPT-3.5
tokenpress stats . --tokenizer hf:tokenizer.json   # any HF tokenizer (Qwen, GLM, ...)
tokenpress stats . --tokenizer kimi:tiktoken.model # Kimi K2/K3 ranks format

# context/behavior trade-offs (opt-in flags — except the Rust and JS/TS
# comment loss, which is unconditional; see below)
tokenpress format . --py-strip-comments      # drop # comments
tokenpress format . --py-strip-docstrings    # drop docstrings (empties __doc__: breaks help() and doctests!)
tokenpress format . --py-strip-annotations   # drop type hints (breaks dataclass/pydantic introspection!)
tokenpress format . --py-no-merge-imports    # keep adjacent imports separate
tokenpress format . --rs-strip-doc-comments  # drop ///+//! doc comments (and doctests)
tokenpress format . --js-strip-comments      # drop the JS/TS comments that survive at all
```

Exit codes: `0` ok · `1` check found changes · `2` error (parse/verification
failures are reported per file; nothing corrupt is ever written).

**Stripped prose is context the model no longer sees.** Comments, docstrings
and annotations are information an LLM could have used to answer questions
about the code, and every strip flag deletes some of it. Whether — and how
much — that degrades the quality of a model's answers has **not been measured
yet**. Until it is, treat the aggressive flags as a cost/quality trade-off you
are choosing, not as free savings.

## Integrations

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
never a branch — pre-commit clones the repository at that ref and builds the
CLI from it.

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

Both hooks declare `files: \.(py|rs|js|mjs|cjs|jsx|ts|mts|cts|tsx)$` alongside
`types_or: [python, rust, javascript, jsx, ts, tsx]`, so only files the CLI
accepts ever reach it. The regex is the authority; `types_or` is a coarse
pre-filter.
Extension-less scripts with a Python shebang are excluded on purpose: an
explicitly named unsupported path makes the CLI exit 2. Both are
`require_serial: true` — every invocation runs a `cargo build` first, and
parallel copies would only contend for the same cargo lock.
`minimum_pre_commit_version` is 2.9.0.

Prerequisites for the consumer:

- **A working `cargo` on `PATH`** — rustup is the easiest route. The hooks are
  `language: script`; the entry script builds `tokenpress-cli` inside
  pre-commit's own clone of this repository, with the working directory there
  so `rust-toolchain.toml` pins the compiler (rustup then installs it on first
  use). The first hook run therefore pays one release build; later runs reuse
  that clone's `target/`.
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
builds the CLI from its own pinned checkout and caches it, so nothing has to be
installed first:

```yaml
- uses: starone99/TokenPress@v0.1.0
  with:
    paths: src tests          # default `.`; directories are walked, .gitignore-aware
    mode: check               # default; `format` rewrites in place
    extra-args: --rs-strip-doc-comments   # optional, passed through verbatim
```

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
| `extra-args` | *(empty)* | Extra `tokenpress` flags, passed through verbatim (whitespace-separated), e.g. `--rs-strip-doc-comments --py-strip-comments --js-strip-comments`. |

Output:

| Output | Meaning |
|---|---|
| `changed` | `'true'` if any file was rewritten (`format`) or would be rewritten (`check`), otherwise `'false'`. Set even when the step fails, so a `continue-on-error` step can gate a follow-up on it. It is `'false'` when the run errored out or had no supported path to process. |

**Directories and explicitly named files are treated differently.** A directory
is handed to the CLI as-is: its walk is `.gitignore`-aware and picks up only
the supported extensions (`.py`, `.rs`, `.js`, `.mjs`, `.cjs`, `.jsx`, `.ts`,
`.mts`, `.cts`, `.tsx`), so pointing `paths` at a mixed tree is safe. An
explicitly named file
is *not* filtered by the CLI — an unsupported one is an error (exit 2) — so the
action drops files with any other extension from the argument list itself and
logs which ones it skipped. A glob over a mixed tree therefore does not abort
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
```

That is the complete schema — there are no other keys. `verify = "external"`
runs the JavaScript/TypeScript toolchain over JS/TS output (see **Language
support**); for `.py` and `.rs` files it still behaves exactly like `"ast"` and
says so on stderr.

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
The same holds for `[javascript] strip_comments` and `--js-strip-comments`.

**Config problems fail loudly**, like every other linter-style tool: an unknown
key, a wrong value type, malformed TOML, or an unknown `tokenizer`/`verify`
value is an error naming the offending key, reported before any file is read —
exit 2, nothing written. A discovered config that does not parse fails exactly
as hard as an explicit one.

## What it never touches

Identifiers, string/number literals, decorators/attributes, the token sequence
inside macro invocations, import order — anything that carries meaning for an
LLM or affects behavior. In Python, comments, docstrings and annotations are
kept by default and only removed by explicit opt-in — and every strip flag
loses information: `--py-strip-docstrings` removes the leading string literal
of a module, class or function body (other string expressions are untouched),
which empties `__doc__`.

Three documented exceptions — two in Rust, one in JavaScript/TypeScript. These
are the scope limits on the "preserving behavior" claim at the top of this
page.

**Regular comments are dropped.** `//` and `/* */` comments are always lost,
because the `syn` token stream the emitter works from does not preserve them.
Doc comments (`///`, `//!`) are preserved unless `--rs-strip-doc-comments` is
passed. If a Rust file's `//` comments matter to you, keep the original —
TokenPress cannot round-trip them.

**Macro body whitespace is minimized.** The *tokens* inside a macro invocation
are preserved exactly, but the whitespace between them is not. For
whitespace-sensitive macros — `stringify!` is the common case — this changes
the string produced at runtime. TokenPress's verification is token-canonical
(re-parse + token-stream equivalence), and a re-spaced macro body is
token-identical to the original, so this class of behavior change is **not**
detected by the verifier. If your code depends on the exact text
`stringify!` renders, review the diff before accepting it.

**Trailing and expression-position JS/TS comments are dropped.** Regardless of
`--js-strip-comments`, the JS/TS emitter keeps only leading statement-level
comments plus jsdoc, annotation (`#__PURE__`) and legal (`//!`, `/*!`,
`@license`, `@preserve`) comments. Everything else — a `// tail` after a
statement, a comment between arguments — is lost, and the verifier cannot see
it because its canonical form is comment-free. If a JS/TS file's inline
comments matter to you, keep the original.

## Layout

Cargo workspace with a single distributed binary:

| Crate | Role |
|---|---|
| `tokenpress-core` | Formatter/Tokenizer traits, tokenizer backends (tiktoken, HF, Kimi ranks) |
| `tokenpress-python` | Python: token-stream re-render + transform passes + verification |
| `tokenpress-rust` | Rust: syn token-stream re-render + verification |
| `tokenpress-js` | JavaScript/TypeScript: oxc parse + whitespace-minimal re-emit + verification (built-in and `tsc`/`node`) |
| `tokenpress-cli` | The `tokenpress` binary: discovery, language detection, commands |
| `tokenpress-wasm` | `wasm-bindgen` boundary for the browser demo (Python + Rust, per-tokenizer token stats) |

## Development

TDD with a hard gate: `scripts/coverage.ps1` (Windows) / `scripts/coverage.sh`
fails the build under 100% line coverage. CI runs fmt, clippy `-D warnings`,
tests (Linux/Windows), and the coverage gate. See `CLAUDE.md` for the rules.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  <https://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
