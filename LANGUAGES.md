# Language support

Per-language detail for TokenPress: what each backend does, what it keeps,
and what it cannot. The short version — which languages are supported and
whether the default setting keeps your comments — is the table in the
[README](README.md).

**Python and Rust are the primary targets.** They are what the project was
built for, what the benchmarks cover most deeply — six of the eight corpora in
the README's chart — and where the work goes first. The other five are supported,
on the same invariant and the same verification, but each rests on a single
corpus and none is the reason this exists.

| Language | Extensions | Default keeps comments | External check |
|---|---|---|---|
| **Python** | `.py` | ✅ | ❌ built-in check only |
| **Rust** | `.rs` | ❌ `//` and `/* */` always dropped | ❌ built-in check only |
| JavaScript / TypeScript | `.js` `.mjs` `.cjs` `.jsx` `.ts` `.mts` `.cts` `.tsx` | ⚠️ partial — trailing and expression-position dropped | ✅ `tsc --noEmit` |
| Ruby | `.rb` `.rake` `.gemspec` `.ru`, plus `Gemfile` and `Rakefile` | ✅ | ✅ `ruby -c` |
| Go | `.go` | ✅ | ✅ `gofmt -e` |
| Java | `.java` | ✅ | ✅ `javac`, stopped after parse |
| C# | `.cs` | ✅ | ✅ Roslyn `csc` |

That last column is worth reading before the rest of this section, because it
cuts against the paragraph above it. "Supported" is gated on handing the
output to the language's own toolchain, on top of the built-in AST-equivalence
check — and **the two primary languages are the two that do not do it yet.**
Python and Rust have the internal check and nothing else. That is the weakest
spot in the project, it is stated here rather than buried, and closing it is
the first item on the [roadmap](ROADMAP.md).

## How each external checker is invoked, and why that one

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

## Ruby, Go, Java and C#

**Ruby is supported.** The backend parses with prism, re-emits, verifies, and
refuses to write anything that fails; `--verify external` hands the output to
`ruby -c` as described above, which is what the label was gated on. Default
settings are whitespace-only and keep every comment;
`--ruby-strip-comments` is the lossy opt-in. Ruby is the one backend that also
claims file names without an extension: `Gemfile` and `Rakefile` are matched
exactly and **case-sensitively** (`gemfile` is not Ruby, and `Gemfile.lock` is
not Ruby at all). Measured savings are published for one Ruby corpus,
rack v3.2.6: -9.2% at default settings and -20.8% with `--ruby-strip-comments`,
both on `o200k_base` — see the [README](README.md#how-much-it-saves) and
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
included (see [VERIFICATION.md](VERIFICATION.md)).

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

