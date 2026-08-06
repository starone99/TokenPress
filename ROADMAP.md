# Roadmap

What is done, what is next, and the questions that are open. Anything not
listed here is not planned.

## Supported today

Seven languages. A language is declared **Supported** only once
`--verify external` hands its output to that language's own toolchain, on top
of the built-in re-parse and AST/token equivalence check:

| Language | External checker |
|---|---|
| Ruby | `ruby -c` |
| Go | `gofmt -e` |
| Java | `javac`, stopped after the parse phase |
| C# | Roslyn `csc` under `/nostdlib+`, diagnostics compared before and after |
| JavaScript / TypeScript | `tsc --noEmit`, falling back to `node --check` |
| Python | built-in check only — see *Next* |
| Rust | built-in check only — see *Next* |

Also shipped: a `tokenpress.toml` config file, a pre-commit hook, a GitHub
Action, a WebAssembly build with an interactive demo page, and a benchmark
suite covering thirteen commit-pinned corpora against six tokenizers
(`o200k_base`, `cl100k_base`, Qwen3.6, GLM-5.2, Kimi K3, Gemma 4). See
[`benchmarks/RESULTS.md`](benchmarks/RESULTS.md) for the methodology and
[`benchmarks/SHOWCASE.md`](benchmarks/SHOWCASE.md) for the summary.

## Next

**External verification for Python and Rust.** These two have the weakest
verification story in the project — the internal AST/token equivalence check
and nothing else — and they are the two most people reach for first. Closing
that gap matters more than an eighth language does.

**PHP backend**, reusing the tree-sitter engine Go, Java and C# already
share. The grammar is the easy part. PHP files are literal output outside
`<?php … ?>`, so the boundary has to survive byte for byte, and no existing
backend has that problem to solve.

## Open questions

Decisions rather than work, and open on purpose. Input welcome in an issue.

**Should a comment stripper carve out comments that only tooling reads?**
`javac`, `csc` and PHP's parser all ignore `NOSONAR`, `//CHECKSTYLE:OFF`,
`@formatter:off`, `$NON-NLS-1$` and their neighbours — so no measurement can
settle this — but a project's checkstyle, spotless or sonar configuration does
not ignore them. The same question covers C# `///` XML documentation and PHP
PHPDoc: ordinary comments to the grammar, while `csc /doc` and every
documentation generator read them. Answering the three differently would be
hard to justify, so they are being decided together.

**Line joining for the free-form languages.** The engine already has the
branch. Java was measured and came out *worse*: byte-identical output once
comments are stripped, and 0.10pp worse on `o200k_base`, because a newline
tokenizes marginally better than a space. Go, C# and PHP are unmeasured, and
the Java precedent argues against all three. If it is ever scheduled, the
first deliverable is a refusal count at default settings, not a savings
figure.

## Deliberately not planned

**A quality evaluation of the aggressive flags.** Whether deleting comments
and docstrings degrades a model's answers is **not measured**, and the README
and SHOWCASE say so wherever those numbers appear. The protocol is fully
pre-registered in
[`benchmarks/quality-eval/DESIGN.md`](benchmarks/quality-eval/DESIGN.md) —
paired variants, contamination-resistant task families, a blinded judge, and
equivalence testing so that "no harm" would be an affirmative finding rather
than a null result. It has not been run. Anyone who wants to run it has
everything they need, and the caveats stay until somebody does.

**Gemma 2 and Gemma 3 tokenizers.** Those repositories are gated — license
acceptance plus an auth token — which nothing else in the benchmark set
requires. Gemma 4 is not gated, and is measured.

**C, C++, Kotlin, Swift.** The README documents why: the preprocessor for the
first two, parser maturity for the others.

## Standing constraints

- Savings are never extrapolated to a tokenizer that has not been measured,
  and no private or closed vocabulary has been measured.
- Downloaded corpora and tokenizer files are never committed.
- Output that fails verification is never written. That is the project's core
  invariant, and it applies to the tooling built around it too.
