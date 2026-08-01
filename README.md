# TokenPress

> A token-aware formatter for Python and Rust that minimizes LLM token usage while preserving behavior.

TokenPress is a token-aware source code formatter for LLMs. Unlike a minifier that
shrinks characters, TokenPress optimizes against an actual LLM tokenizer —
the output is the equivalent program that costs the fewest input tokens.

```text
minimize  tokenizer.encode(transformed_code)
s.t.      the transformed code parses, compiles, and behaves identically
```

Output that fails verification (re-parse + AST/token equivalence) is never
written — that is the project's core invariant.

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

¹ `--py-strip-comments --py-strip-annotations` ² `--rs-strip-doc-comments`

The default setting is context-lossless for Python: comments, docstrings and
type annotations are all kept; only syntactic noise (whitespace, blank lines,
indentation width) is minimized and adjacent imports are merged.

**Rust is not context-lossless, even at default settings.** The Rust backend
re-emits from the `syn` token stream, which does not carry regular comments:
`//` and `/* */` comments are **always** dropped. Only doc comments (`///`,
`//!`) survive — they are `#[doc = "..."]` attributes — and only unless
`--rs-strip-doc-comments` is passed. Part of the measured Rust savings above
therefore comes from discarded comments, not from syntactic noise alone.

Savings differ per tokenizer — the reason this is a token-aware formatter,
not a character minifier.

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

# context/behavior trade-offs (opt-in flags — except Rust //-comment loss, see below)
tokenpress format . --py-strip-comments      # drop # comments
tokenpress format . --py-strip-docstrings    # drop docstrings (empties __doc__: breaks help() and doctests!)
tokenpress format . --py-strip-annotations   # drop type hints (breaks dataclass/pydantic introspection!)
tokenpress format . --py-no-merge-imports    # keep adjacent imports separate
tokenpress format . --rs-strip-doc-comments  # drop ///+//! doc comments (and doctests)
```

Exit codes: `0` ok · `1` check found changes · `2` error (parse/verification
failures are reported per file; nothing corrupt is ever written).

## What it never touches

Identifiers, string/number literals, decorators/attributes, the token sequence
inside macro invocations, import order — anything that carries meaning for an
LLM or affects behavior. In Python, comments, docstrings and annotations are
kept by default and only removed by explicit opt-in — and every strip flag
loses information: `--py-strip-docstrings` removes the leading string literal
of a module, class or function body (other string expressions are untouched),
which empties `__doc__`.

Two documented exceptions, both in Rust — these are the scope limits on the
"preserving behavior" claim at the top of this page.

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

## Layout

Cargo workspace with a single distributed binary:

| Crate | Role |
|---|---|
| `tokenpress-core` | Formatter/Tokenizer traits, tokenizer backends (tiktoken, HF, Kimi ranks) |
| `tokenpress-python` | Python: token-stream re-render + transform passes + verification |
| `tokenpress-rust` | Rust: syn token-stream re-render + verification |
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
