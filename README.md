# TokenPress

> A token-aware formatter for Python, Rust, JavaScript/TypeScript, Ruby, Go,
> Java and C# that minimizes LLM token usage while preserving behavior.

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

| Corpus | Setting | GPT-4o/o-series (`o200k_base`) | Qwen3.6 | GLM-5.2 | Kimi K3 | Gemma 4 |
|---|---|---|---|---|---|---|
| requests v2.32.3 | default | -9.0% | -9.8% | -9.0% | -8.7% | -16.4%⁷ |
| requests | aggressive¹ | -20.6% | -21.0% | -20.7% | -20.2% | -25.7%⁷ |
| ripgrep 14.1.1 | default | -19.2%⁸ | **-23.2%**⁸ | -18.3%⁸ | -18.2%⁸ | -28.0%⁷⁸ |
| ripgrep | aggressive² | -38.2% | **-42.7%** | -38.1% | -37.9% | -45.1%⁷ |
| express v5.2.1 | default | -17.3% | -25.0% | -17.6% | -17.4% | -21.7% |
| express | aggressive³ | -25.4% | **-33.3%** | -25.9% | -25.5% | -29.6% |
| rack v3.2.6 | default | -9.2% | -8.2% | -8.9% | -8.9% | -7.6% |
| rack | aggressive⁴ | -20.8% | -19.8% | -20.5% | -20.5% | -18.2% |
| gin v1.11.0 | default | -6.4% | -5.6% | -6.2% | -6.4% | -6.8% |
| gin | aggressive⁵ | -19.4% | -18.7% | -20.0% | -20.0% | -18.8% |
| commons-lang 3.17.0 | default | -6.1% | -5.6% | -6.2% | -6.0% | -5.0% |
| commons-lang | aggressive⁶ | **-45.5%** | **-45.3%** | **-46.6%** | **-45.5%** | **-42.9%** |

¹ `--py-strip-comments --py-strip-annotations` ² `--rs-strip-doc-comments`
³ `--js-strip-comments` ⁴ `--ruby-strip-comments` ⁵ `--go-strip-comments`
⁶ `--java-strip-comments`

⁷ **Read these four Gemma cells with the line-ending caveat, not without
it.** The requests and ripgrep rows are CRLF measurements — that is what
their other four columns are too, so the rows are internally consistent —
but Gemma 4 is an order of magnitude more CRLF-sensitive than any other
tokenizer here (a CRLF checkout inflates its before-count ~11-13%, where
`o200k_base` moves ~1% and Qwen3.6 not at all). On an LF checkout the same
runs read -7.3% / -25.7% / -20.9% / **-39.6%**. In particular **ripgrep's
-45.1% is not a ≥40% showcase candidate**: at LF it is -39.6% and misses the
bar, and `benchmarks/SHOWCASE.md`'s per-tokenizer lists are LF throughout.
The other rows in this table are LF and need no such adjustment.

⁸ **This row mixes three emitter revisions and only the first cell is
current.** How `tokenpress-rust` re-emits doc comments changed twice on
2026-08-06, in opposite directions. First a correctness fix: a doc block in
which one line needed the raw `#[doc = …]` fallback was being emitted
*mixed*, which misindents a doc example, so the whole block was forced to the
raw form — and that form costs more tokens than `///`, taking this cell from
the published -18.9% to -17.9%. Then the fallback itself was narrowed: a line
comment carries no escape sequences, so a doc line holding quotes or
backslashes can be sugared back to `///` after all, and only values no line
comment can carry (a `/*! … */` module doc, say) still fall back. Blocks are
still emitted in one form, so the misindent cannot return. `o200k_base` is
**-19.2%** on that build, re-measured 2026-08-06 — past the -18.9% originally
published rather than back to it. The Qwen3.6, GLM-5.2 and Kimi K3 cells date
from 2026-07-31 and the Gemma cell from the intermediate build; all four are
left exactly as measured rather than adjusted — no tokenizer's saving is ever
estimated from another's here, and none of the four can be re-run without the
open-model tokenizer files. Expect each to move in the same direction and by
a similar scale when they are. The aggressive row below is unaffected by
either change (`--rs-strip-doc-comments` deletes the attributes before
emission). Full account, including the single-file mechanism, the rustdoc
behaviour that was measured and the tokio row both changes also affect, in
[benchmarks/RESULTS.md](benchmarks/RESULTS.md) under "Correction
(2026-08-06)" and "Update (2026-08-06)".

Every corpus in this table is measured on every tokenizer listed; nothing
here is estimated from one tokenizer's number. `cl100k_base` (GPT-4 /
GPT-3.5-turbo) is omitted for width and is in
[benchmarks/RESULTS.md](benchmarks/RESULTS.md) along with raw token counts,
seven further corpora, and the line-ending note that explains why two
platforms' before-counts differ slightly.

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
| Ruby | `.rb` `.rake` `.gemspec` `.ru`, plus the exact file names `Gemfile` and `Rakefile` | Supported (context-lossless at default settings; see below) |
| Go | `.go` | Supported (context-lossless at default settings; see below) |
| Java | `.java` | Supported (context-lossless at default settings; see below) |
| C# | `.cs` | Supported (context-lossless at default settings; see below) |

**`--verify external` is real for every language in the table except Python and
Rust**, which is what "Supported" is gated on here: the output is handed to the
language's own toolchain, on top of the built-in AST-equivalence check.

For JavaScript/TypeScript it runs
`tsc --noEmit --noCheck --skipLibCheck --allowJs --jsx preserve` over the
candidate — one syntax-only command covering all eight extensions — and falls
back to `node --check` when no `tsc` is on PATH. The fallback reads only
`.js`, `.mjs` and `.cjs`; for a `.jsx`/`.ts`/`.mts`/`.cts`/`.tsx` file with no
`tsc` available the run **fails naming the missing tool** rather than quietly
checking less than you asked for, so `--verify external` requires `tsc` (or,
for plain JavaScript, `node`) on PATH.

For Ruby it runs `ruby -c` — the interpreter's own syntax check, which
compiles the file and stops, so nothing in your code runs (`BEGIN`/`END`
blocks included). The verdict is the exit status alone: `ruby -c` prints
warnings for files that are perfectly valid, and those never fail a run.
`--verify external` therefore requires `ruby` on PATH for Ruby paths, and
**fails naming it** when it is missing rather than quietly checking less.

For Go it runs `gofmt -e` — the toolchain's own front end, which parses the
file with `go/parser` and reports every parse error rather than the first ten.
Nothing is compiled, linked or run, and no package outside the file is loaded,
so no module context is needed. The verdict is the exit status alone: `gofmt`
prints the *reformatted* source on success and TokenPress's output is not
`gofmt`-shaped, so that output is discarded rather than compared —
`--verify external` is a parse check, never a style check. It requires `gofmt`
on PATH (it ships with every Go distribution) for Go paths, and **fails naming
it** when it is missing. The probe is `gofmt -h`: `gofmt` has no `--version`,
and a bare `gofmt` reads standard input.

For Java it runs `javac -XDshould-stop.ifNoError=PARSE` — the compiler's own
front end, stopped after the parse phase. Nothing is resolved, attributed,
generated or run, and no class file is written, so no classpath, module path
or build is needed: a file whose imports name types nothing provides still
exits 0, which is what makes checking a single file out of a real project
meaningful at all. The verdict is the exit status alone, and deliberately so —
the JVM prints `Picked up JAVA_TOOL_OPTIONS: ...` to stderr on *every*
invocation wherever that variable is set, so a stderr-based gate would reject
everything on those machines. `--verify external` requires `javac` (any JDK)
on PATH for Java paths and **fails naming it** when it is missing. Because
`-XD` is javac's internal namespace and an unrecognised key is *silently
ignored* rather than rejected, TokenPress runs the gate over a built-in
valid-but-unresolvable fixture before trusting it, and fails loudly naming the
option if a JDK ever stops honouring it — otherwise the check would quietly
become a whole-program compile and start blaming TokenPress for your
classpath.

For C# it runs Roslyn's own `csc` — and not the way any of the others work,
because **C# has no parse-only compiler mode**. Run with `/nostdlib+` (no
reference assemblies, so no project file, no restore and no network) over a
file from a real project, `csc` reports a pile of unresolved-type errors that
say nothing about whether the file is well-formed. So the noise is not removed,
it is made to **cancel**: the compiler is run over your input *and* over the
candidate, the `error CS####` codes of each run are collected into a multiset
with their positions discarded, and the output is accepted only when the two
multisets are equal. Unresolvable references produce the same complaints on
both sides; a syntax error introduced by formatting appears on one side only,
in **any** code range — a top-level statement after a type declaration is
`CS8803`, not `CS1xxx`, which is why there is no code-range shortcut here. The
verdict is therefore the diagnostics and **never the exit status**, since `csc`
exits non-zero for exactly the noise this design tolerates. `--verify external`
requires `dotnet` (any .NET SDK; the SDK's own version is discovered at run
time, never hardcoded) on PATH for C# paths and **fails naming it** when it is
missing. Because the whole verdict is parsed text, TokenPress runs the gate
over a built-in valid-but-unresolvable fixture before trusting it and fails
loudly if the codes can no longer be read — otherwise a reworded diagnostic
format would leave two empty multisets comparing equal and a check that passes
everything.

The candidate is checked in a private temp file — carrying the target's
extension for JS/TS, always `.rb` for Ruby, which is what makes the
extensionless `Gemfile` and `Rakefile` checkable at all, always `.go` for Go,
always `.java` for Java (safe because Java's public-class/filename rule is a
semantic check the parse gate stops before reaching), always `.cs` for C#;
nothing is written to your file until every check has passed. Python and Rust
do not implement the level yet and still treat it as `--verify ast`; the CLI
says so on stderr when a `.py` or `.rs` path is in the run.

If your *input* does not pass the external checker (a file the toolchain
already rejects — ESM syntax in a `.cjs`, a syntax newer than your `tsc`, a
regexp literal prism parses and MRI refuses to compile, a `.go` file with no
package clause, a Java `long x = 99999999999;` with no `L` suffix, a C#
`long x = 99999999999999999999999;`), the output is not checked against it and
the file is accepted on the built-in equivalence check alone: TokenPress does
not fail a run over a file that was already broken before it ran. For C# that
is not even a special case — the identical complaint appears on both sides of
the comparison and cancels like any other noise. Expect the level to be
substantially slower than `--verify ast` — it spawns a probe and two checker
processes per file, three for Java, where the third is the gate's own
self-test, and four for C#, whose self-test is a before/after pair.

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

**Ruby is supported.** The backend parses with prism, re-emits, verifies, and
refuses to write anything that fails; `--verify external` hands the output to
`ruby -c` as described above, which is what the label was gated on. Default
settings are whitespace-only and keep every comment;
`--ruby-strip-comments` is the lossy opt-in. Ruby is the one backend that also
claims file names without an extension: `Gemfile` and `Rakefile` are matched
exactly and **case-sensitively** (`gemfile` is not Ruby, and `Gemfile.lock` is
not Ruby at all). Measured savings are published for one Ruby corpus,
rack v3.2.6: -9.2% at default settings and -20.8% with `--ruby-strip-comments`,
both on `o200k_base` — see the table above and
[benchmarks/RESULTS.md](benchmarks/RESULTS.md), which also reports
`cl100k_base` (-8.9% / -20.5%). All five tokenizers are measured — the
open-model three land at -8.2% / -8.9% / -8.9% at default settings — but one
corpus is not a language-wide claim. That run has one verification refusal,
`lib/rack/utils.rb` — a documented over-refusal class, in the safe direction:
the file is left unchanged and nothing that fails the check is written. Ruby is
also the one supported language the browser demo does not offer: prism is a
vendored C library whose sources do not build for the
`wasm32-unknown-unknown` target the demo bundle is compiled to, so Ruby is
CLI-only.

**Ruby, unlike Rust and JS/TS, is context-lossless at default settings.** The
Ruby emitter rewrites the whitespace *between* protected source spans and
copies everything else verbatim, so **every comment survives byte for byte** —
leading, trailing, inline and `=begin`/`=end` embdocs alike.
`--ruby-strip-comments` is the opt-in that removes them, and even then the
shebang and the leading magic-comment window (everything before the first code
token, e.g. `# frozen_string_literal: true`) are kept. Whitespace minimization
removes indentation, trailing whitespace and blank lines and collapses other
runs of spaces and tabs to exactly one space — never to zero, because `a - b`
is a subtraction while `a -b` is a call with a unary-minus argument — and
newlines are statement terminators in Ruby, so they stay. There is therefore no
Ruby caveat warning on stderr: there is nothing to warn about. Context-lossless
here is about *comments*, not line numbers — those no backend preserves, Ruby
included (see **What it never touches** below).

**Go is supported.** The backend parses with tree-sitter, re-emits
whitespace-minimally over the source bytes, verifies with re-parse plus
AST-equivalence, and refuses to write anything that fails — the same core
invariant as every other backend. `--verify external` hands the output to
`gofmt -e` as described above, which is what the label was gated on. `.go` is
the whole path set (`go.mod` and `go.sum` are not Go source), and measured
over the Go 1.24.7 standard library (7,117 files, 79 MB) the savings are
**-7.2%** at default settings and **-23.6%** with `--go-strip-comments`, both
on `o200k_base`, with zero verification refusals — and `gofmt -e` accepted
every one of the 7,060 outputs, in both comment configurations.

**Go, like Ruby, is context-lossless at default settings** — and it defends
what Go puts *in* comments. Every comment survives byte for byte unless
`--go-strip-comments` is passed, and even then the comments the toolchain
reads as instructions are kept: `//go:` directives, `/*line*/`, and build
constraints (`//go:build` and the legacy `// +build`). Three further rules
apply at **both** settings, because they are whitespace rules rather than
deletion rules: an indented directive-shaped comment is never moved to
column 0 (where `go generate` or the compiler would start obeying it), a
build-constraint prologue is reproduced verbatim including the blank line a
legacy `// +build` needs to keep working, and a file that imports `"C"` is
left byte for byte identical — cgo preambles are C source that is compiled,
so a cgo file reports no savings at all. There is therefore no Go caveat
warning on stderr, for the same reason there is no Ruby one: nothing is
dropped behind your back. Go **is** in the browser demo, unlike Ruby — the
tree-sitter grammar compiles for `wasm32-unknown-unknown` once the libc shim
`tree-sitter-language` already ships is on the C include path. The demo runs
Go at `--verify ast` (in-process re-parse plus AST equivalence) and not at
`--verify external`, because a WebAssembly module cannot spawn `gofmt`.

**Java is supported.** The backend rides the same tree-sitter engine as Go:
it parses with `tree-sitter-java`, re-emits whitespace-minimally over the
source bytes, verifies with re-parse plus AST-equivalence, and refuses to
write anything that fails — the same core invariant as every other backend.
`--verify external` hands the output to `javac`'s own parse gate on top of
that, which is what the "Supported" label is gated on. `.java` is the whole
path set (`module-info.java` and `package-info.java` are ordinary Java
sources needing no special case, while `pom.xml` and `build.gradle` are not
Java source at all), and measured over apache/commons-lang 3.17.0 (500 files,
7.4 MB) the savings are **-6.2%** at default settings and **-45.5%** with
`--java-strip-comments`, both on `o200k_base`, with zero verification
refusals and every one of the 1,000 written outputs accepted by `javac`.

**Java, like Ruby and Go, is context-lossless at default settings**, and it
has less to defend than Go does: `javac` reads nothing out of a comment, so
there is no keep-list, no column-0 promotion rule and no verbatim prologue.
Every comment survives byte for byte unless `--java-strip-comments` is passed
— and **that flag deletes Javadoc**, because a `/** … */` block is an ordinary
comment to the grammar, so a stripped file loses its public API
documentation. That is where most of the 45% comes from; it is asked for
explicitly, and at the default settings not one byte of documentation is
dropped. One rule applies at **both** settings: `javac` decodes `\uXXXX`
before it lexes, and tree-sitter does not, so a file whose comment carries an
escape that decodes to a comment terminator is left byte for byte identical
and reports no savings — the analogue of Go's cgo bail-out. There is
therefore no Java caveat warning on stderr, for the same reason there is no
Ruby or Go one: nothing is dropped behind your back. Java **is** in the
browser demo, alongside Go and unlike Ruby — the tree-sitter grammar compiles
for `wasm32-unknown-unknown` under the very libc-shim include path
`site/build.sh` already exports for Go, with no build-script change of its
own. The demo runs Java at `--verify ast` (in-process re-parse plus AST
equivalence) and not at `--verify external`, because a WebAssembly module
cannot spawn `javac`.

**Java source has to be UTF-8 to be formatted at all.** Java has no fixed
source encoding — `javac`'s is `-encoding`-configurable and defaults to the
platform charset — so a Latin-1 file is legal Java to a project configured
for it. TokenPress reads files as UTF-8 and reports a non-UTF-8 file as an
error instead of rewriting it, which is the safe direction: nothing is
written. This is Ruby's situation rather than Go's, whose source is UTF-8 by
specification.

**C# rides the same tree-sitter engine as Go and Java**: it parses with
`tree-sitter-c-sharp`, re-emits whitespace-minimally over the source bytes,
verifies with re-parse plus AST-equivalence, and refuses to write anything that
fails. `--verify external` hands the output to Roslyn's `csc` on top of that,
which is what the "Supported" label is gated on — by comparing the compiler's
diagnostics before and after rather than by an exit status, because C# has no
parse-only compiler mode (see the `--verify external` section above). `.cs` is
the whole path set: `AssemblyInfo.cs`, a generated `*.Designer.cs` and a
`*.g.cs`
are ordinary sources needing no special case, while `.csproj` and `.sln` are
project metadata rather than source, `.csx` is a scripting dialect this
backend does not claim, and `.vb`/`.fs` are other languages on the same
runtime. Measured over JamesNK/Newtonsoft.Json at `4f73e74` (945 `.cs` files,
5.2 MB in, 879 written) the savings are **-8.7%** at default settings and
**-33.6%** with `--csharp-strip-comments`, on `o200k_base`.

**C#, like Ruby, Go and Java, is context-lossless at default settings.** Every
comment survives byte for byte unless `--csharp-strip-comments` is passed —
and **that flag deletes XML documentation**, because a `///` block is an
ordinary comment to this grammar with no second kind to spare it, so a
stripped file loses its API documentation. That is where most of the 33% comes
from; it is asked for explicitly, and at the default settings not one byte of
documentation is dropped. Two rules apply at **both** settings. A preprocessor
directive (`#if`, `#region`, `#nullable`) must begin its line, so the emitter
never joins lines or invents one. And where the grammar and a real C# compiler
could disagree about *where a comment ends* — a comment spanning an
`#if`/`#endif` pair is the standard case — the file is returned **byte for
byte unchanged and reports no savings**. That is C#'s analogue of Go's cgo
bail-out and Java's unicode-escape one, and it is the first documented class
of files TokenPress reports as a successful run that saved nothing rather than
as an error. There is therefore no C# caveat warning on stderr, for the same
reason there is no Ruby, Go or Java one: nothing is dropped behind your back.
C# **is** in the browser demo, alongside Go and Java and unlike Ruby — and it
is the first grammar there with a live external `scanner.c`, which needed no
build-script change of its own either: the scanner's whole libc surface is
`iswspace`, which the tree-sitter libc shim `site/build.sh` already puts on the
include path defines `static inline`, so it adds no undefined symbol to the
link. The demo runs C# at `--verify ast` (in-process re-parse plus AST
equivalence) and not at `--verify external`, because a WebAssembly module
cannot spawn a compiler.

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
tokenpress format . --ruby-strip-comments    # drop Ruby comments/embdocs (shebang + magic comments kept)
tokenpress format . --go-strip-comments      # drop Go comments (//go: directives, build constraints and cgo preambles kept)
tokenpress format . --java-strip-comments    # drop Java comments -- Javadoc included!
tokenpress format . --csharp-strip-comments  # drop C# comments -- /// XML documentation included!
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
builds the CLI from its own pinned checkout and caches it, so nothing has to be
installed first:

```yaml
- uses: starone99/TokenPress@v0.1.0
  with:
    paths: src tests          # default `.`; directories are walked, .gitignore-aware
    mode: check               # default; `format` rewrites in place
    extra-args: --rs-strip-doc-comments   # optional, passed through verbatim
```

**The runner needs libclang and a C compiler.** The action builds the CLI from
its own checkout, and that build now includes all four native backends: the
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

## What it never touches

Identifiers, string/number literals, decorators/attributes, the token sequence
inside macro invocations, import order — anything that carries meaning for an
LLM or affects behavior. In Python, comments, docstrings and annotations are
kept by default and only removed by explicit opt-in — and every strip flag
loses information: `--py-strip-docstrings` removes the leading string literal
of a module, class or function body (other string expressions are untouched),
which empties `__doc__`.

Four documented exceptions — one that applies to every backend, two in Rust,
one in JavaScript/TypeScript. These are the scope limits on the "preserving
behavior" claim at the top of this page.

**Line numbers are never preserved, by any backend, at any settings.** Deleting
blank lines and re-flowing whitespace is the core of what TokenPress does, so
every line below a removal moves — in Python, Ruby, Go, Java and C# exactly as
in Rust and JS/TS. No flag turns this off. Code whose behavior depends on physical line
numbers can therefore change behavior after formatting: Ruby `__LINE__` and
`caller`, Rust `line!()` and `std::panic::Location`, Python `inspect` and
traceback line numbers, JavaScript `Error.stack`, Go `runtime.Caller` (a
`//line` directive is the one case that is protected, because it is a comment
the toolchain reads), Java stack-trace line numbers, C# stack-trace line
numbers and `CallerLineNumberAttribute`, and any test that asserts on a
traceback or a stack trace.

Format-time verification cannot detect this **by construction**: the canonical
forms the re-parse/equivalence check compares are location-independent, which
is what makes them usable as an equality stand-in at all, so a moved line is
the same token in the same position before and after. `--verify external` does
not help either — `tsc`, `ruby -c`, `gofmt -e`, `javac`'s parse gate and the
`csc` diagnostic comparison all stop before anything runs, and none of them
compares positions. The
layer that does catch it is running a corpus's own upstream test suite against
the formatted copy (`benchmarks/verify-upstream.sh`), and it has: on 2026-08-02
the rack v3.2.6 target came back **DIVERGED** on one test,
`Rack::Builder::parse_file` "sets `__LINE__` correctly" — TokenPress deletes the
blank line above the code in `test/builder/line.ru`, so `__LINE__` reads `2`
where the test asserts `3`. Reproduced byte-identically on repeat runs — not a
flake. That rewrite saved **zero tokens** (35 before, 35 after at
`o200k_base`: a blank line and a plain newline each cost one token). Full triage in
[benchmarks/RESULTS.md](benchmarks/RESULTS.md). The limitation is documented
rather than mitigated: if your code, your tests or your tooling depend on line
numbers, TokenPress output is not a drop-in replacement for the original —
keep it.

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
| `tokenpress-ruby` | Ruby: prism parse + whitespace-minimal re-emit over the source bytes + verification |
| `tokenpress-treesitter` | The grammar-agnostic tree-sitter engine: parse gate, equivalence artifact, protected spans, whitespace rewriter |
| `tokenpress-go` | Go: the grammar configuration, path set and comment policy the engine is driven with |
| `tokenpress-java` | Java: the same, for the Java grammar |
| `tokenpress-csharp` | C#: the same, for the C# grammar |
| `tokenpress-cli` | The `tokenpress` binary: discovery, language detection, commands |
| `tokenpress-wasm` | `wasm-bindgen` boundary for the browser demo (Python, Rust, JavaScript/TypeScript, Go, Java and C# — not Ruby, per-tokenizer token stats) |

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
