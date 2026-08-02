# TokenPress Showcase

Measured token reduction on nine commit-pinned open-source repositories, at
the *aggressive* (lossy) settings. Every number on this page is taken from
[`RESULTS.md`](RESULTS.md), which holds the full methodology, the platform
notes, and the default-setting numbers. Nothing here is extrapolated.

Two tokenizers only: `o200k_base` (OpenAI GPT-4o / GPT-4.1 / o-series,
including Codex models) and `cl100k_base` (GPT-4 / GPT-3.5-turbo). Both are
embedded in the binary and were measured locally.

---

## Headline: tokio, -50.5%

The only corpus of the nine that clears a 40% reduction *on the two embedded
OpenAI tokenizers measured here*. Per-model candidate lists are still pending
(see caveats).

| | |
|---|---|
| Project | [tokio-rs/tokio](https://github.com/tokio-rs/tokio) |
| Language | Rust |
| Pinned commit | `adc2ae7af2caaea83985fbdfbc7884c159c486f2` (main snapshot) |
| Files formatted | 790 `.rs` |
| Flags | `--rs-strip-doc-comments` |

| Tokenizer | Before | After | Saved |
|---|---|---|---|
| `o200k_base` | 1,394,248 | 690,397 | **-50.5%** |
| `cl100k_base` | 1,394,142 | 684,818 | **-50.9%** |

Absolute saving at `o200k_base`: **703,851 tokens** per full-repo prompt.

tokio is doc-comment-dense, and Rust additionally loses all `//` and `/* */`
comments through the `syn` token stream (see caveats), so more than half of a
full-repo prompt disappears.

```bash
target/release/tokenpress stats benchmarks/corpus/tokio \
    --tokenizer o200k_base --rs-strip-doc-comments
```

---

## Full corpus table (aggressive settings)

Measured 2026-08-01 on a Linux LF checkout of the pinned commits (express
2026-08-02, same way). `Files` is the number of files successfully formatted.

| Project | Lang | Pinned commit | Files | `o200k_base` | `cl100k_base` |
|---|---|---|---|---|---|
| [tokio-rs/tokio](https://github.com/tokio-rs/tokio) | Rust | `adc2ae7af2caaea83985fbdfbc7884c159c486f2` | 790 | **-50.5%** | **-50.9%** |
| [langchain-ai/langchain](https://github.com/langchain-ai/langchain) | Python | `a1a1ad3bb3eb6cf7680b39ff0fb37f7150393a25` | 2,530 | -38.8% | -39.2% |
| [BurntSushi/ripgrep](https://github.com/BurntSushi/ripgrep) | Rust | `4649aa97` (tag 14.1.1) | 98 | -37.4% | -37.6% |
| [fastapi/fastapi](https://github.com/fastapi/fastapi) | Python | `95f8322ee1dcda7ceace7b1c4f6c9915b36d748f` | 1,136 | -36.2% | -36.9% |
| [psf/requests](https://github.com/psf/requests) | Python | `0e322af8` (tag v2.32.3) | 36 | -36.0% | -36.3% |
| [huggingface/transformers](https://github.com/huggingface/transformers) | Python | `71c6f699ac9b3f8fc42a6a3e9dc59034c349a678` | 4,700 | -34.9% | -35.6% |
| [expressjs/express](https://github.com/expressjs/express) | JavaScript | `dbac741a49a5a64336b70c06e85c2e2706e36336` (tag v5.2.1) | 142 | -25.4% | -25.9% |
| [django/django](https://github.com/django/django) | Python | `50d706d0aebcc2d073c8d034b6e22fc98fad49f2` | 2,924 | -23.1% | -23.8% |
| [astral-sh/uv](https://github.com/astral-sh/uv) | Rust + Python | `be765050837d81badb20e1f70eec62146c586902` | 718 | -21.5% | -21.8% |

Raw token counts for the same runs:

| Project | `o200k` before | `o200k` after | `cl100k` before | `cl100k` after |
|---|---|---|---|---|
| tokio | 1,394,248 | 690,397 | 1,394,142 | 684,818 |
| langchain | 2,956,442 | 1,810,164 | 2,942,240 | 1,787,452 |
| ripgrep | 415,590 | 259,984 | 415,698 | 259,335 |
| fastapi | 731,846 | 466,724 | 728,430 | 459,582 |
| requests | 86,331 | 55,265 | 86,014 | 54,827 |
| transformers | 17,030,922 | 11,086,759 | 16,956,696 | 10,927,850 |
| express | 135,740 | 101,253 | 135,206 | 100,122 |
| django | 4,191,951 | 3,221,811 | 4,122,515 | 3,141,905 |
| uv | 4,806,817 | 3,773,907 | 4,779,673 | 3,739,347 |

### Flags used

| Corpus type | Flags |
|---|---|
| Python (requests, django, fastapi, langchain, transformers) | `--py-strip-comments --py-strip-annotations --py-strip-docstrings` |
| Rust (ripgrep, tokio) | `--rs-strip-doc-comments` |
| JavaScript (express) | `--js-strip-comments` |
| Mixed (uv: 624 `.rs` + 94 `.py`) | all four Python/Rust flags — per-language flags are language-scoped, so passing a Rust flag to a Python tree is a verified no-op |

### Why express is mid-table

express is the only JavaScript corpus, and `--js-strip-comments` is a weaker
lever than its Python and Rust counterparts: the JS/TS backend already drops
trailing and expression-position comments unconditionally (see caveats), so
the flag only buys the leading statement-level comments, jsdoc, annotation
and legal comments on top. At default settings express is already at -17.3%
on `o200k_base`; the flag adds +8.1pp. No JavaScript or TypeScript project
has been hunted for a ≥40% result yet — one corpus is not a search, so this
page makes **no ≥40% claim for JS/TS in either direction**.

### Why django and uv are low

Django's bulk is test fixtures and data tables rather than prose. uv is 87%
Rust by file count, and its doc comments are a small fraction of the tree —
`--py-strip-docstrings` moves it by only +0.1pp.

### What docstring stripping contributes

Same corpus, same checkout, `o200k_base`, the added flag being the only
difference:

| Corpus | Without | With `--py-strip-docstrings` | Delta |
|---|---|---|---|
| requests | -20.1% | -36.0% | +15.9pp |
| django | -17.1% | -23.1% | +6.0pp |
| fastapi | -34.5% | -36.2% | +1.7pp |
| langchain | -22.4% | -38.8% | +16.4pp |
| transformers | -21.6% | -34.9% | +13.3pp |
| uv | -21.4% | -21.5% | +0.1pp |

---

## Verification

* **Every file counted above passed TokenPress verification** (re-parse plus
  token/AST equivalence). The runs reported here have **0 verification
  refusals**. Two files across the corpus are excluded because they are
  intentionally-broken fixtures — Django's `tests_syntax_error.py` (invalid
  Python by design) and LangChain's `non-utf8-encoding.py` (invalid UTF-8 by
  design) — i.e. unreadable/invalid *input*, reported per file, not refused
  output.
* An earlier run of the same settings surfaced 19 files that TokenPress
  refused to format, caused by two defects in `--py-strip-annotations`. No
  file was written; both defects were fixed and the langchain and transformers
  rows were re-measured with the recovered files included. `RESULTS.md` keeps
  the full triage.

### Behavioral verification against upstream test suites

Structural equivalence is not behavioral equivalence, so
`benchmarks/verify-upstream.sh` runs two projects' own upstream test suites
against a TokenPress-formatted copy and diffs the outcome **per test id**
against an unformatted baseline copy.

| Target | Files | Rewritten | Refused | Result |
|---|---|---|---|---|
| requests v2.32.3 (`pytest`) | 36 `.py` | 35 | 0 | **IDENTICAL** — 585 passed / 5 failed / 15 skipped / 1 xfailed on both copies |
| ripgrep 14.1.1 (`cargo test --workspace`) | 98 `.rs` | 98 | 0 | **IDENTICAL after a fix** — 1106 ok / 3 ignored, exit 0 |
| express v5.2.1 (`mocha`, express's own `npm test` arguments) | 142 `.js` | 141 | 0 | **IDENTICAL** — 1238 passed / 0 failed on both copies, exit 0 |

All caveats matter:

* The requests suite's 5 failures are sandbox network artifacts and fail
  identically on the unformatted copy. The claim is *identical outcomes*, not
  *all tests pass*.
* The first ripgrep run **diverged** on exactly 1 of 1109 tests and found a
  real formatter bug (a mixed sugared/raw doc block changed what a doc example
  asserted), which token/AST equivalence is structurally blind to. It was
  fixed test-first in commit `b1572d3`; the re-run is IDENTICAL.
* express is the only target green on both sides — its suite drives ephemeral
  localhost servers and needs no outbound network. That is a property of the
  suite, not a stronger claim: what is verified is still *identical outcomes*.
  The express target needs `node`, `npm` and npm registry access to run at all.
* These runs cover **default settings only**. The aggressive flags on this
  page are not covered by that harness — stripping doc comments would delete
  the doc tests being compared.
* No test suite can detect the Rust or JS/TS comment loss below, because
  comments do not run. An IDENTICAL verdict is behavioral equivalence, not
  context equivalence.

---

## Caveats

**The aggressive settings are lossy by design.** They are opt-in flags, and
each removes information from the source:

| Flag | What it removes |
|---|---|
| `--py-strip-comments` | Python `#` comments |
| `--py-strip-docstrings` | the leading string literal of a module, class or function body — this empties `__doc__`, breaking `help()` and doctests |
| `--py-strip-annotations` | Python type hints — breaks `__annotations__`-based introspection (dataclass, pydantic) |
| `--rs-strip-doc-comments` | Rust `///` and `//!` doc comments, and with them rustdoc and doctests |
| `--js-strip-comments` | the JS/TS comments that survive re-emission at all — leading statement-level comments, jsdoc, annotation comments (`#__PURE__`) and legal comments (`//!`, `/*!`, `@license`, `@preserve`) |

**Token savings are not free context.** Every comment and docstring stripped
above is prose the model can no longer read. Whether that degrades the quality
of a model's answers on real code tasks — and by how much — is **unmeasured**;
no experiment on this project has tested it. The numbers on this page quantify
the cost saving only. They say nothing about the quality trade-off on the other
side of it.

**Rust loses regular comments even at default settings.** The Rust backend
re-emits from the `syn` token stream, which does not carry `//` or `/* */`
comments; they are always dropped. Only doc comments survive, and only without
`--rs-strip-doc-comments`. Part of the savings for every Rust corpus above
therefore comes from discarded comments, not from syntactic noise alone. If a
file's `//` comments matter to you, keep the original — TokenPress cannot
round-trip them.

**JavaScript/TypeScript loses some comments even at default settings.** The
JS/TS backend re-emits from its own code generator, and **trailing comments
and comments in expression position are always dropped, with or without
`--js-strip-comments`.** Only leading statement-level comments, jsdoc
(`/** */`), annotation comments and legal comments survive. Verification
cannot detect this — its canonical form is comment-free by construction — so
the express default-settings number (-17.3% on `o200k_base`) is already a
lossy one, and `--js-strip-comments` is the difference between "some comments
kept" and "none", not between "all" and "none". The CLI prints the caveat on
stderr once per run that touches a JS/TS file.

**JSX text is never compressed, and this page does not measure it.**
Whitespace inside JSX element children is semantically significant, so it is
re-emitted verbatim; a `.jsx`/`.tsx` file saves tokens only on the
JavaScript/TypeScript around its markup. express v5.2.1 is 142 `.js` files
with no TypeScript and no JSX, so **nothing on this page measures the
`.ts`/`.tsx`/`.jsx` paths**, and a JSX-heavy tree should be expected to save
less than -25.4%.

**Rust macro-body whitespace is minimized.** The tokens inside a macro
invocation are preserved exactly; the whitespace between them is not. For
whitespace-sensitive macros — `stringify!` is the common case — this changes
the string produced at runtime, and a re-spaced macro body is token-identical
to the original, so verification does not detect it.

**Public tokenizers only.** All numbers here are `o200k_base` and
`cl100k_base`. Claude's vocabulary is private and has not been measured; no
number on this page is given or extrapolated for any private or closed
tokenizer, and none should be. `RESULTS.md` additionally reports three
open-model tokenizers (Qwen3.6, GLM-5.2, Kimi K3) for the default and
historical-aggressive runs; those columns are absent from the full-aggressive
run above because the measurement environment could not fetch the
revision-pinned tokenizer files.

**Which projects clear 40% depends on the tokenizer.** Candidate selection on
this page used `o200k_base` and `cl100k_base` only, so the ≥40% claim is a
proxy — judge savings, and ≥40% membership, on the tokenizer of the model you
actually use. The gap is not cosmetic: ripgrep aggressive is -38.2% on
`o200k_base` but -42.7% on Qwen3.6 in `RESULTS.md`'s historical table, so it
already clears 40% on Qwen while missing it here. Separate candidate lists for
Qwen3.6, GLM-5.2, Kimi K3 and Gemma are pending an environment with
huggingface.co access; Gemma is not in the benchmark tokenizer set yet.

**Line endings shift the baseline slightly.** These runs are a Linux LF
checkout; the earlier tables in `RESULTS.md` were measured on Windows with
CRLF, which raises the before-counts (requests: 86,331 LF vs 86,922 CRLF at
`o200k`). This is why ripgrep reads -37.4% here and -38.2% in the historical
aggressive table — the flag set is unchanged. Converting the LF checkout back
to CRLF reproduces the historical numbers exactly.

**Nine corpora is not a population.** These are the repositories measured,
not a sample chosen to be representative. Savings depend heavily on how much
of a tree is prose documentation. JavaScript is represented by exactly one
project, so `-25.4%` is a data point, not a language-level expectation.

---

## Reproduce

```bash
./benchmarks/fetch.sh          # corpus at the pinned commits above
cargo build --release -p tokenpress-cli

# Rust corpora
for c in ripgrep tokio; do
    target/release/tokenpress stats "benchmarks/corpus/$c" --tokenizer o200k_base \
        --rs-strip-doc-comments
done

# Python corpora
for c in requests django fastapi langchain transformers; do
    target/release/tokenpress stats "benchmarks/corpus/$c" --tokenizer o200k_base \
        --py-strip-comments --py-strip-annotations --py-strip-docstrings
done

# JavaScript corpus
target/release/tokenpress stats benchmarks/corpus/express --tokenizer o200k_base \
    --js-strip-comments

# Mixed tree
target/release/tokenpress stats benchmarks/corpus/uv --tokenizer o200k_base \
    --py-strip-comments --py-strip-annotations --py-strip-docstrings \
    --rs-strip-doc-comments

# repeat any of the above with --tokenizer cl100k_base for the second column

# upstream behavioral check (default settings)
# the express target additionally needs node, npm and npm registry access
./benchmarks/verify-upstream.sh all   # 0 = identical, 1 = diverged, 2 = never ran
```

`fetch.sh` exits non-zero at the tokenizer-download step when huggingface.co
is unreachable; the corpus clones above it have already completed, and the two
tokenizers used on this page are embedded in the binary and need no download.
