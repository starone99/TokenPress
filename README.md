<p align="center">
  <strong>TokenPress</strong>
</p>

<p align="center">
  <strong>A formatter for agentic coding — optimized for the tokenizer, not the human reader.</strong>
</p>

<p align="center">
  <a href="https://github.com/starone99/TokenPress/actions"><img src="https://github.com/starone99/TokenPress/workflows/CI/badge.svg" alt="CI"></a>
  <a href="LICENSE-APACHE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue.svg" alt="License"></a>
  <img src="https://img.shields.io/badge/coverage-100%25-brightgreen.svg" alt="Coverage">
</p>

---

If you are doing agentic coding, why are you still running a formatter built
for a human reader? Black, gofmt, rustfmt and Prettier all optimize for a
person's eyes — line width, alignment, blank lines between things. When the
reader is a model, none of that is value. It is billed tokens.

TokenPress emits the equivalent program that costs the fewest input tokens:

```text
minimize  tokenizer.encode(transformed_code)
s.t.      the transformed code parses, compiles, and behaves identically
```

It is not a minifier — character count and token count disagree, so the
transforms are chosen against a real tokenizer. **Output that fails
verification is never written**, and identifiers and string contents are never
touched.

## How much it saves

Aggressive settings, full corpora at pinned commits, every file passing
verification. The solid bar is what *every* tokenizer saves; the shaded tail
is how much further the most favourable one goes.

```text
                      0%   10%  20%  30%  40%  50%  60%
                      ├────┼────┼────┼────┼────┼────┤
tokio (Rust)          █████████████████████████░░░      -50.5 … -55.2%
ripgrep (Rust)        ███████████████████░░             -37.3 … -42.7%
langchain (Python)    ███████████████████░░             -37.1 … -41.1%
fastapi (Python)      ██████████████████░░              -36.1 … -40.1%
requests (Python)     ████████████████░░                -31.9 … -36.5%
transformers (Python) ███████████████░░░                -30.3 … -36.1%
uv (Rust + Python)    ███████████░                      -21.4 … -24.7%
django (Python)       ██████████░░                      -20.7 … -24.8%
```

The spread is the point: savings are per tokenizer, which is why the benchmark
measures six of them — GLM-5.2, Kimi K3, Gemma 4, Qwen3.6, `o200k_base` and
`cl100k_base`. The other five supported languages, one corpus each, same run:

```text
commons-lang (Java)   █████████████████████░░           -42.9 … -46.6%
express (JS)          █████████████░░░░                 -25.4 … -33.3%
rack (Ruby)           █████████░                        -18.2 … -20.8%
gin (Go)              █████████░                        -18.7 … -20.0%
```

**No private or closed tokenizer has been measured, and no number here is
extrapolated to one.** Savings track how much of a tree is prose, not which
language it is written in; one corpus per language is a data point, never a
language-level expectation. Thirteen corpora, raw token counts, per-tokenizer
tables and the line-ending caveats are in
[benchmarks/RESULTS.md](benchmarks/RESULTS.md), summarized in
[benchmarks/SHOWCASE.md](benchmarks/SHOWCASE.md).

**Stripping comments and docstrings deletes context a model could have used,
and whether that degrades its answers has not been measured.** The savings are
measured; the quality trade-off is not. Treat the aggressive flags as a choice,
not as free.

## Does a human read this code?

One question decides how to use this.

**Yes — format the copy you hand the model and leave your source alone.**
Paste it into a prompt, hand it to an agent's context window, feed it to a RAG
index. This holds even at default settings: the default run still removes
blank lines and squeezes indentation. "Context-lossless" here is a claim about
what a *model* can recover, never about what a person enjoys reading.

**No — nobody reads it, the repository is written and maintained by agents —
then normalizing the source itself is coherent**, and the pre-commit hook and
GitHub Action exist for that. Two things to know first, neither about human
readers:

- **Rust joins every line.** At default settings the Rust backend re-emits a
  whole file as one line, so line-addressed edit tools, `git diff`, merge
  conflicts and stack traces all degrade. The other backends keep newlines.
- **Rust and JS/TS lose comments at default**, and there is no un-format.
  Under a hook that is not a one-time conversion — every comment written
  afterwards is removed on the next run.

There is no reverse mapping: no source map, no patch-back. A model can read
formatted code and answer about it, but a diff against the formatted copy will
not apply to an unformatted original. **A file a model is going to edit should
be given to it unformatted.**

## Installation

```bash
cargo install --git https://github.com/starone99/TokenPress tokenpress-cli
```

Building the Ruby, Go, Java and C# backends needs a C compiler, and libclang
for Ruby. Drop them with `--no-default-features` plus the features you want —
see [INTEGRATIONS.md](INTEGRATIONS.md#cargo-features).

## Quick start

```bash
tokenpress stats  <PATH>...        # what it would save — writes nothing
tokenpress diff   <PATH>...        # unified diff — writes nothing
tokenpress format <PATH>...        # rewrite in place (dirs walk recursively)
tokenpress check  <PATH>...        # CI gate: exit 1 if anything would change
```

Start with `stats`. It touches nothing and tells you whether this is worth it
for your tree:

```bash
tokenpress stats . --tokenizer o200k_base            # GPT-4o / o-series (default)
tokenpress stats . --tokenizer cl100k_base           # GPT-4 / GPT-3.5
tokenpress stats . --tokenizer hf:tokenizer.json     # any HF tokenizer (Qwen, GLM, Gemma…)
tokenpress stats . --tokenizer kimi:tiktoken.model   # Kimi ranks format
```

Everything lossy is an opt-in flag, and each says what it breaks:

```bash
--py-strip-comments        # drop # comments
--py-strip-docstrings      # empties __doc__ — breaks help() and doctests
--py-strip-annotations     # breaks dataclass/pydantic introspection
--py-no-merge-imports      # keep adjacent imports separate
--rs-strip-doc-comments    # drops /// and //! — rustdoc and doctests with them
--js-strip-comments        # drops the JS/TS comments that survive at all
--ruby-strip-comments      # shebang and magic comments kept
--go-strip-comments        # //go: directives, build constraints, cgo preamble kept
--java-strip-comments      # Javadoc included
--csharp-strip-comments    # /// XML documentation included
```

Exit codes: `0` ok · `1` check found changes · `2` error. Parse and
verification failures are reported per file, and nothing corrupt is ever
written.

## How it works

```text
  source ──▶ parse ──▶ re-emit at minimum token cost ──▶ verify ──▶ write
                                                            │
                                              ┌─────────────┴─────────────┐
                                              │ re-parse                  │
                                              │ AST / token equivalence   │
                                              │ the language's own tool   │  ← --verify external
                                              └─────────────┬─────────────┘
                                                            │
                                                     fails ─┴─▶ file left untouched
```

The last step is the whole design. A transform that cannot be proven
equivalent is not written, so the worst case is that a file is left alone —
never that it is corrupted.

## Language support

**Python and Rust are the primary targets** — what the project was built for,
what the benchmarks cover most deeply, and where the work goes first. The
other five are supported on the same invariant and the same verification, but
each rests on a single corpus.

| Language | Extensions | Default keeps comments | External check |
|---|---|---|---|
| **Python** | `.py` | ✅ | ❌ built-in check only |
| **Rust** | `.rs` | ❌ `//` and `/* */` always dropped | ❌ built-in check only |
| JavaScript / TypeScript | `.js` `.mjs` `.cjs` `.jsx` `.ts` `.mts` `.cts` `.tsx` | ⚠️ partial | ✅ `tsc --noEmit` |
| Ruby | `.rb` `.rake` `.gemspec` `.ru`, `Gemfile`, `Rakefile` | ✅ | ✅ `ruby -c` |
| Go | `.go` | ✅ | ✅ `gofmt -e` |
| Java | `.java` | ✅ | ✅ `javac`, stopped after parse |
| C# | `.cs` | ✅ | ✅ Roslyn `csc` |

That last column cuts against the paragraph above it, and is stated here
rather than buried: **the two primary languages are the two without external
verification.** Closing that is the first item on the [roadmap](ROADMAP.md).

Per-language detail — what each backend keeps, what it cannot, and how each
external checker is invoked — is in [LANGUAGES.md](LANGUAGES.md).

## Integrations

A pre-commit hook, a GitHub Action and a `tokenpress.toml` config file, all
with a `check` mode that writes nothing. Setup and options in
[INTEGRATIONS.md](INTEGRATIONS.md).

```yaml
# .pre-commit-config.yaml
repos:
  - repo: https://github.com/starone99/TokenPress
    rev: v0.1.0
    hooks:
      - id: tokenpress-check   # writes nothing; fails if anything would change
```

## Documentation

| | |
|---|---|
| [LANGUAGES.md](LANGUAGES.md) | Per-language support, caveats and external checkers |
| [VERIFICATION.md](VERIFICATION.md) | What the check proves, and what is never touched |
| [INTEGRATIONS.md](INTEGRATIONS.md) | pre-commit, GitHub Action, config file, cargo features |
| [benchmarks/RESULTS.md](benchmarks/RESULTS.md) | Full methodology, thirteen corpora, six tokenizers |
| [benchmarks/SHOWCASE.md](benchmarks/SHOWCASE.md) | The summary, and the ≥40% candidates per tokenizer |
| [ROADMAP.md](ROADMAP.md) | What is next, and the questions that are open |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Building, testing, and the toolchains each backend needs |

## Development

TDD with a hard gate: `scripts/coverage.ps1` (Windows) / `scripts/coverage.sh`
fails the build under 100% line coverage. CI runs fmt, clippy `-D warnings`,
tests on Linux and Windows, and the coverage gate. Rules in
[CLAUDE.md](CLAUDE.md).

## License

Licensed under the Apache License, Version 2.0
([LICENSE-APACHE](LICENSE-APACHE) or
<https://www.apache.org/licenses/LICENSE-2.0>).

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
licensed as above, without any additional terms or conditions.
