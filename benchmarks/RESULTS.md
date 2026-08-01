# TokenPress Benchmark Results

Measured: 2026-07-31 (open-model tokenizers added 2026-08-01)
Binary: `cargo build --release -p tokenpress-cli` at the commit containing this file
Command: `tokenpress stats <corpus> --tokenizer <name> [options]`
Platform: Windows 11, rustc 1.95.0

## Corpus (commit-pinned via `benchmarks/fetch.ps1`)

| Project | Version | Commit | Files |
|---|---|---|---|
| psf/requests | v2.32.3 | `0e322af8` | 36 `.py` (src + tests + docs scripts) |
| BurntSushi/ripgrep | 14.1.1 | `4649aa97` | 98 `.rs` (all crates) |
| django/django | main snapshot | `50d706d0` | 2,924 `.py` |
| fastapi/fastapi | main snapshot | `95f8322e` | 1,136 `.py` |
| tokio-rs/tokio | main snapshot | `adc2ae7a` | 790 `.rs` |
| langchain-ai/langchain | main snapshot | `a1a1ad3b` | 2,530 `.py` |
| huggingface/transformers | main snapshot | `71c6f699` | 4,700 `.py` |
| astral-sh/uv | main snapshot | `be765050` | 718 `.rs` + `.py` |

**Every parseable file passed verification** (re-parse + token/AST
equivalence). The only skipped files across ~13,000 are two intentionally
broken test fixtures: Django's `tests_syntax_error.py` (invalid Python by
design) and LangChain's `non-utf8-encoding.py` (invalid UTF-8 by design) —
both correctly rejected with per-file errors.

## Which LLM does each number apply to? (tokenizer ↔ model mapping)

| Tokenizer | Models | Source | Notes |
|---|---|---|---|
| `o200k_base` | OpenAI GPT-4o, GPT-4.1, o1/o3/o4 series (incl. Codex models) | tiktoken built-in | measured locally |
| `cl100k_base` | OpenAI GPT-4, GPT-3.5-turbo | tiktoken built-in | measured locally |
| Qwen3.6 | Qwen/Qwen3.6-35B-A3B (rev `995ad96e`) | `--tokenizer hf:` + tokenizer.json | measured locally |
| GLM-5.2 | zai-org/GLM-5.2 (rev `b4734de4`) | `--tokenizer hf:` + tokenizer.json | measured locally |
| Kimi K3 | moonshotai/Kimi-K3 (rev `9f62e4e9`) | `--tokenizer kimi:` + tiktoken.model | tiktoken ranks + Kimi pat_str loader |

Claude is not yet measured — its vocabulary is private, so numbers require
the `count_tokens` API. **Never extrapolate private-tokenizer savings from
the numbers below** (project rule). Tokenizer files are downloaded
revision-pinned by `benchmarks/fetch.ps1`.

## Results

### Default settings (Python: comments/docstrings/annotations kept; Rust: doc comments kept, regular comments dropped; adjacent imports merged)

Context-lossless for Python — whitespace/blank-line/indent minimization plus
PY09 import merging only.

**Rust is not context-lossless, even at default settings.** The Rust backend
re-emits from the `syn` token stream, which does not carry regular comments:
`//` and `/* */` comments are always dropped, and only doc comments (`///`,
`//!`) survive. Part of the default savings for every Rust corpus below —
ripgrep in this table, tokio and uv in the next — therefore comes from
discarded comments, not from syntactic noise alone.

| Corpus | Tokenizer | Before | After | Saved |
|---|---|---|---|---|
| requests | o200k_base | 86,922 | 79,093 | **-9.0%** |
| requests | cl100k_base | 86,531 | 78,726 | **-9.0%** |
| requests | Qwen3.6 | 94,786 | 85,548 | **-9.7%** |
| requests | GLM-5.2 | 86,791 | 78,984 | **-9.0%** |
| requests | Kimi K3 | 87,235 | 79,645 | **-8.7%** |
| ripgrep | o200k_base | 420,944 | 341,242 | **-18.9%** |
| ripgrep | cl100k_base | 419,272 | 342,663 | **-18.3%** |
| ripgrep | Qwen3.6 | 458,041 | 351,962 | **-23.2%** |
| ripgrep | GLM-5.2 | 419,526 | 342,819 | **-18.3%** |
| ripgrep | Kimi K3 | 420,393 | 343,904 | **-18.2%** |

### Well-known projects (default settings, savings %)

| Project | o200k_base | cl100k_base | Qwen3.6 | GLM-5.2 | Kimi K3 |
|---|---|---|---|---|---|
| Django (4.22M tok) | -10.3% | -10.6% | **-12.1%** | -10.6% | -10.3% |
| FastAPI (738k tok) | -22.2% | -22.7% | **-26.7%** | -22.6% | -22.6% |
| tokio (1.42M tok) | -20.4% | -19.6% | **-23.1%** | -19.6% | -19.5% |
| LangChain (2.96M tok) | -12.5% | -12.9% | **-15.6%** | -12.9% | -12.2% |
| transformers (17.0M tok) | -8.6% | -8.9% | **-10.3%** | -8.9% | -8.5% |
| uv (4.81M tok) | -13.3% | -13.5% | **-16.9%** | -13.3% | -13.5% |

Token totals in parentheses are the o200k before-counts. Absolute savings at
o200k: Django 435k, FastAPI 163k, tokio 290k, LangChain 370k, transformers
1.46M, uv 641k tokens per full-repo prompt. Qwen3.6 consistently benefits
the most (its tokenizer prices whitespace runs highest).

### Aggressive settings (accepting context loss)

#### Historical run — predates `--py-strip-docstrings`

**This table was measured on 2026-07-31, before the `--py-strip-docstrings`
flag existed**, so its Python numbers still keep every docstring. It is
retained unchanged as the historical record; the full-aggressive numbers are
in the next subsection.

* Python: `--py-strip-comments --py-strip-annotations`
  (warning: breaks `__annotations__`-based introspection — treat these
  numbers as an upper bound)
* Rust: `--rs-strip-doc-comments` (loses rustdoc & doctests)

| Corpus | Tokenizer | Before | After | Saved |
|---|---|---|---|---|
| requests | o200k_base | 86,922 | 69,035 | **-20.6%** |
| requests | cl100k_base | 86,531 | 68,642 | **-20.7%** |
| requests | Qwen3.6 | 94,786 | 74,905 | **-21.0%** |
| requests | GLM-5.2 | 86,791 | 68,862 | **-20.7%** |
| requests | Kimi K3 | 87,235 | 69,622 | **-20.2%** |
| ripgrep | o200k_base | 420,944 | 260,047 | **-38.2%** |
| ripgrep | cl100k_base | 419,272 | 259,429 | **-38.1%** |
| ripgrep | Qwen3.6 | 458,041 | 262,670 | **-42.7%** |
| ripgrep | GLM-5.2 | 419,526 | 259,578 | **-38.1%** |
| ripgrep | Kimi K3 | 420,393 | 261,072 | **-37.9%** |

#### Aggressive + strip_docstrings (full aggressive, added 2026-08-01)

Measured: 2026-08-01, Linux, rustc 1.95.0, same commit-pinned corpus
(`benchmarks/fetch.sh`). **Only the two embedded tokenizers were measured**
— the measurement environment cannot reach huggingface.co, so the Qwen3.6 /
GLM-5.2 / Kimi K3 columns are absent here and are to be filled in from an
environment that can fetch those revision-pinned tokenizer files.

Exact flags:

* Python corpora (requests, django, fastapi, langchain, transformers):
  `--py-strip-comments --py-strip-annotations --py-strip-docstrings`
* Rust corpora (ripgrep, tokio): `--rs-strip-doc-comments`
* Mixed corpus (uv, 624 `.rs` + 94 `.py`): all four flags. Per-language
  flags are language-scoped — passing the Rust flag to a pure-Python tree
  (or vice versa) is a verified no-op, so a mixed tree can safely take all
  four at once.

**Line-ending caveat — read before comparing against the tables above.**
The earlier tables were measured on Windows, where git checks the corpus out
with CRLF; this run is a Linux LF checkout of the *same* pinned commits. LF
before-counts are therefore slightly lower (requests: 86,331 vs 86,922 at
o200k, -0.7%). This was confirmed, not assumed: converting the LF checkout to
CRLF reproduces the historical numbers exactly (86,922 → 79,093 at default
settings, byte-identical to the default-settings table). **Differences
between this subsection and the tables above are a platform artifact, not a
formatter change.**

| Corpus | Tokenizer | Files | Before | After | Saved |
|---|---|---|---|---|---|
| requests | o200k_base | 36 | 86,331 | 55,265 | **-36.0%** |
| requests | cl100k_base | 36 | 86,014 | 54,827 | **-36.3%** |
| ripgrep | o200k_base | 98 | 415,590 | 259,984 | **-37.4%** |
| ripgrep | cl100k_base | 98 | 415,698 | 259,335 | **-37.6%** |
| django | o200k_base | 2,924 | 4,191,951 | 3,221,811 | **-23.1%** |
| django | cl100k_base | 2,924 | 4,122,515 | 3,141,905 | **-23.8%** |
| fastapi | o200k_base | 1,136 | 731,846 | 466,724 | **-36.2%** |
| fastapi | cl100k_base | 1,136 | 728,430 | 459,582 | **-36.9%** |
| tokio | o200k_base | 790 | 1,394,248 | 690,397 | **-50.5%** |
| tokio | cl100k_base | 790 | 1,394,142 | 684,818 | **-50.9%** |
| langchain | o200k_base | 2,530 | 2,956,442 | 1,810,164 | **-38.8%** |
| langchain | cl100k_base | 2,530 | 2,942,240 | 1,787,452 | **-39.2%** |
| transformers | o200k_base | 4,700 | 17,030,922 | 11,086,759 | **-34.9%** |
| transformers | cl100k_base | 4,700 | 16,956,696 | 10,927,850 | **-35.6%** |
| uv | o200k_base | 718 | 4,806,817 | 3,773,907 | **-21.5%** |
| uv | cl100k_base | 718 | 4,779,673 | 3,739,347 | **-21.8%** |

`Files` counts files that were successfully formatted; refused files (next
subsection) are excluded from both the file count and the token totals. The
langchain and transformers rows were re-measured on 2026-08-01 after the
PYO3 refusals below were fixed, so they now cover the previously-excluded 19
files (langchain 2,512 → 2,530, transformers 4,699 → 4,700); every other row
is the original measurement. Only langchain's percentages moved (-38.5% →
-38.8% at o200k, -39.0% → -39.2% at cl100k); transformers' are unchanged to
one decimal, the recovered file being one of 4,700.

##### What `--py-strip-docstrings` adds

Same corpus, same LF checkout, same tokenizer (o200k_base) — the only
difference is the added flag, so these deltas are apples-to-apples:

| Corpus | Without docstring stripping | With `--py-strip-docstrings` | Delta |
|---|---|---|---|
| requests | -20.1% | **-36.0%** | +15.9pp |
| django | -17.1% | **-23.1%** | +6.0pp |
| fastapi | -34.5% | **-36.2%** | +1.7pp |
| langchain | -22.4% | **-38.8%** | +16.4pp |
| transformers | -21.6% | **-34.9%** | +13.3pp |
| uv | -21.4% | **-21.5%** | +0.1pp |

Docstring stripping is the single largest aggressive lever for
documentation-heavy Python (langchain +16.4pp, requests +15.9pp). It barely
moves uv (+0.1pp), which is 87% Rust by file count, or FastAPI (+1.7pp),
whose bulk is annotations and inline comments rather than prose docstrings.

##### Verification refusals under `--py-strip-annotations` (FIXED 2026-08-01)

> **Resolved.** All 19 refusals below are fixed; langchain and transformers
> now format with **0 verification refusals** (langchain still reports the
> one known non-UTF-8 fixture, which is unreadable input rather than a
> refusal). The full-aggressive table above has been re-measured with the
> recovered files included. The triage that follows is kept as the
> historical record of how the bug was found and what it was.

This was the first time the aggressive settings were run over the six large
corpora, and it surfaced **19 files that TokenPress refused to format** —
beyond the two intentionally-broken fixtures documented above. No file was
written; the formatter reported per-file errors and excluded them, which is
the core invariant working as designed.

| Corpus | Refusals | Breakdown |
|---|---|---|
| django | 1 | known fixture `tests_syntax_error.py` (invalid input, not a refusal) |
| langchain | 19 | 1 known fixture `non-utf8-encoding.py` + **18 new** |
| transformers | 1 | **1 new** (`models/esm/openfold_utils/protein.py`) |
| requests, ripgrep, fastapi, tokio, uv | 0 | — |

Error classes across the 19 new refusals: 11 × `output token stream differs
from input`, 6 × `output failed to re-parse: Expected a statement`, 1 ×
`output failed to re-parse: Expected ':', found '='`, plus 1 × the known
non-UTF-8 fixture.

**The cause is `--py-strip-annotations` (PYO3), not the new
`--py-strip-docstrings` flag.** Isolated by re-running each flag alone on
langchain at o200k: `--py-strip-annotations` alone reproduces all 19;
`--py-strip-docstrings` alone and `--py-strip-comments` alone each produce
only the 1 known non-UTF-8 fixture error, as does the default setting. The
same holds for transformers (annotations 1, docstrings 0, comments 0). These
refusals are therefore pre-existing behavior of a flag that shipped earlier
— newly *discovered* here, not newly *introduced*.

Minimal reproducer (delta-debugged from langchain's
`output_parsers/regex.py`) — a value-less annotated declaration that both
follows a nested block and is the last statement of its suite:

```python
class C:
    def m(self):
        return True
    output_keys: list[str]
```

`tokenpress stats <file> --py-strip-annotations` →
`verification failed: output token stream differs from input`. Giving the
declaration a value (`output_keys: list[str] = []`) or removing the
preceding nested block makes it pass. The same shape reproduces inside a
function body, so it is not class-specific.

A second, independent trigger found while reducing: a file whose final
statement is a value-less annotated declaration **and** which lacks a
trailing newline fails the same way even without a preceding block.

Both are refusals, not corruptions — nothing was written.

**Root cause and fix (2026-08-01).** Two independent defects in PYO3's
AST-span-to-token mapping, both fixed test-first in
`crates/tokenpress-python/src/passes.rs`:

1. *Zero-width block markers inside a statement span.* The lexer emits
   `Dedent` — and, for a file with no trailing newline, the closing
   `Newline` — at zero width positioned exactly on a statement boundary, so
   they fall inside the AST span of the `x: T` declaration they abut.
   Replacing that declaration with `pass` swallowed them: eating a `Dedent`
   left the `pass` inside the block that had just closed, and eating the
   final `Newline` dropped the statement terminator. This is the same class
   of shape problem `strip_docstrings` hit with statement separators and
   `settle_indents`. Fixed by stepping the whole-statement replacement over
   block markers. Accounts for the 11 × `output token stream differs from
   input`.
2. *Parenthesized annotations.* Ruff's expression ranges exclude enclosing
   parentheses, so `x: (int) = 1` reports a span covering only `int` and the
   `)` was left behind with no opener — `x)=1`. Fixed by anchoring each
   annotation's span on the `:` / `->` that introduces it and extending it
   forward until brackets balance, which also handles a range that starts or
   ends inside a bracket (`(a) | b`, `a | (b)`) and leaves a parenthesized
   *target* (`(x): int = 1` → `(x)=1`) alone. Accounts for the 6 ×
   `Expected a statement` and the 1 × `Expected ':', found '='`.

Re-measured after the fix: langchain 2,530 files and transformers 4,700
files, **0 verification refusals** in both, at both embedded tokenizers.

#### Showcase candidates (≥40% aggressive reduction)

Recorded for the ROADMAP P3 task ("hunt for well-known projects per
supported language where the aggressive setting clears ≥40% token
reduction"). Measured 2026-08-01 on the embedded tokenizers only.

**Clears ≥40% — 1 of 8 corpora:**

| Project | Language | Commit | o200k_base | cl100k_base | Absolute saving (o200k) |
|---|---|---|---|---|---|
| tokio-rs/tokio | Rust | `adc2ae7a` | **-50.5%** | **-50.9%** | 703,851 tokens |

```bash
target/release/tokenpress stats benchmarks/corpus/tokio \
    --tokenizer o200k_base --rs-strip-doc-comments
```

tokio is the strongest showcase in the corpus: it is doc-comment-dense, and
Rust additionally loses all `//` / `/* */` comments through the `syn` token
stream, so more than half of a full-repo prompt disappears.

**Near misses (35–40% on o200k_base):**

| Project | Language | Commit | o200k_base | cl100k_base |
|---|---|---|---|---|
| langchain-ai/langchain | Python | `a1a1ad3b` | -38.8% | -39.2% |
| BurntSushi/ripgrep | Rust | `4649aa97` | -37.4% | -37.6% |
| fastapi/fastapi | Python | `95f8322e` | -36.2% | -36.9% |
| psf/requests | Python | `0e322af8` | -36.0% | -36.3% |
| huggingface/transformers | Python | `71c6f699` | -34.9% | -35.6% |

Flags: Python projects `--py-strip-comments --py-strip-annotations
--py-strip-docstrings`; ripgrep `--rs-strip-doc-comments`.

**Well under 40%:** django/django `50d706d0` (-23.1% / -23.8%) and
astral-sh/uv `be765050` (-21.5% / -21.8%, all four flags). Django's bulk is
test fixtures and data tables rather than prose; uv is dominated by Rust
source whose doc comments are a small fraction of the tree.

Two caveats on this list:

* **ripgrep's number is not directly comparable to the historical aggressive
  table** (-37.4% here vs -38.2% there). The Rust flag set did not change —
  `--rs-strip-doc-comments` is the same single flag in both runs — and the
  difference is entirely the CRLF→LF checkout described above.
* **The open-model tokenizers would likely add candidates.** In the
  historical table Qwen3.6 beat o200k by +4.5pp on ripgrep aggressive
  (-42.7% vs -38.2%), i.e. ripgrep already cleared 40% on Qwen and did not
  on o200k. Several of the near misses above may clear ≥40% on Qwen3.6 once
  those tokenizer files can be fetched. Re-run this section in an
  environment with huggingface access before treating the list as final.

## Interpretation

* Rust saves more than Python: newlines carry no syntax in Rust, so all
  newlines/indentation can go, and `syn` always drops regular comments (an
  MVP constraint). ripgrep is doc-comment-heavy, hence the large aggressive
  delta.
* requests' -9.0% default keeps every comment and docstring; requests is a
  densely documented project, so preservation costs a lot.
* Tokenizer differences are real — notably **Qwen3.6 saves -23.2% on
  ripgrep vs o200k's -18.9%** (+4.3pp). Qwen's tokenizer encodes
  indentation/whitespace runs comparatively expensively (Before is 458k vs
  o200k's 421k on the same corpus), so whitespace removal pays off more.
  This is the strongest evidence for the premise that TokenPress optimizes
  tokens, not characters.
* Context math: with default settings alone, ripgrep's full source drops
  from ~3.3 to ~2.7 fills of a 128k context (o200k).

## Reproduce

```powershell
.\benchmarks\fetch.ps1     # corpus + tokenizer files (revision-pinned)
cargo build --release -p tokenpress-cli
.\target\release\tokenpress.exe stats benchmarks\corpus\requests --tokenizer o200k_base
.\target\release\tokenpress.exe stats benchmarks\corpus\ripgrep --tokenizer o200k_base
# open-model tokenizers
.\target\release\tokenpress.exe stats benchmarks\corpus\ripgrep --tokenizer hf:benchmarks\tokenizers\qwen3.6.json
.\target\release\tokenpress.exe stats benchmarks\corpus\ripgrep --tokenizer hf:benchmarks\tokenizers\glm-5.2.json
.\target\release\tokenpress.exe stats benchmarks\corpus\ripgrep --tokenizer kimi:benchmarks\tokenizers\kimi-k3.tiktoken
# aggressive settings
.\target\release\tokenpress.exe stats benchmarks\corpus\requests --tokenizer o200k_base --py-strip-comments --py-strip-annotations
.\target\release\tokenpress.exe stats benchmarks\corpus\ripgrep --tokenizer o200k_base --rs-strip-doc-comments
```

Bash equivalent, including the full-aggressive runs added 2026-08-01. Note
that `fetch.sh` exits non-zero at the tokenizer-download step when
huggingface.co is unreachable; the corpus clones above it have already
completed at that point, and the two embedded tokenizers need no download.

```bash
./benchmarks/fetch.sh     # corpus + tokenizer files (revision-pinned)
cargo build --release -p tokenpress-cli

# default settings
target/release/tokenpress stats benchmarks/corpus/requests --tokenizer o200k_base
target/release/tokenpress stats benchmarks/corpus/ripgrep --tokenizer o200k_base

# full aggressive — Python corpora
for c in requests django fastapi langchain transformers; do
    target/release/tokenpress stats "benchmarks/corpus/$c" --tokenizer o200k_base \
        --py-strip-comments --py-strip-annotations --py-strip-docstrings
done

# full aggressive — Rust corpora
for c in ripgrep tokio; do
    target/release/tokenpress stats "benchmarks/corpus/$c" --tokenizer o200k_base \
        --rs-strip-doc-comments
done

# full aggressive — mixed tree (per-language flags are language-scoped)
target/release/tokenpress stats benchmarks/corpus/uv --tokenizer o200k_base \
    --py-strip-comments --py-strip-annotations --py-strip-docstrings \
    --rs-strip-doc-comments

# repeat any of the above with --tokenizer cl100k_base for the second column
```

## Bugs this measurement caught (all fixed)

The first run excluded 8 requests files and 10 ripgrep files as verification
failures. The verification layer refused to write corrupted output and
reported errors instead — exactly as designed — which surfaced three real
formatter bugs:

1. **Python trailing comments inside brackets**: `f(a,  # note` swallowed
   the continuation line → inline comments now force a line break.
2. **Rust open range patterns**: `128.. =>` glued into `..=` + `>` → added
   the `('.','=')` glue pair.
3. **Rust macro-body spacing metadata**: `vec![1, -2]` → `vec![1,-2]` flips
   the comma's Joint/Alone flag, failing the overly strict string
   comparison → switched to structural token comparison (token content and
   structure still must match exactly).

A later exploratory run over Django, FastAPI and tokio (~6.4M tokens total)
caught two more adjacency bugs, both fixed (the requests numbers above moved
by +23 tokens as a result):

4. **Python implicit string concatenation**: `"" "x"` glued into `"""x`,
   opening a triple-quoted string → same-quote string tokens keep a space.
5. **Rust literal suffixes**: `extern "system" fn` glued into
   `"system"fn`, which lexes as a single suffixed literal → a literal
   ending in `"`/`'`/`#` followed by a word keeps a space.

The transformers corpus caught one more — a genuine behavior-change bug:

6. **f-string debug specifiers**: minimizing `f"{x = }"` to `f"{x=}"`
   changes the runtime output string (the whitespace is echoed verbatim).
   The AST-equivalence check caught it in 12 transformers files. Fix:
   f-string interiors are now emitted verbatim, matching the design contract
   that string content is never touched.

Also hardened: a single unreadable file (e.g. LangChain's intentional
non-UTF-8 fixture) no longer aborts the whole run — it is reported per-file
like any other error.

## Behavioral verification against upstream test suites

Everything above is a *structural* claim: TokenPress re-parses its own output
and requires token/AST equivalence, and never writes a file that fails that
check. Structural equivalence is not the same as behavioral equivalence, so
`benchmarks/verify-upstream.sh` checks the claim from the outside — it runs
each corpus's own upstream test suite against a TokenPress-formatted copy and
compares the result, per test id, against an unformatted baseline copy.

### Methodology

The script (`benchmarks/verify-upstream.sh <requests|ripgrep|all>`) does the
same thing for both targets:

1. **Pinned corpus, SHA-asserted.** The corpus is cloned at the same tag as
   `fetch.ps1`/`fetch.sh` and its `HEAD` is asserted against a hard-coded
   commit SHA, so a retagged upstream cannot silently change what is verified
   (requests `0e322af8`, ripgrep `4649aa97`).
2. **Two pristine copies** of that corpus in a private work directory:
   `*-baseline` (untouched) and `*-formatted`.
3. **Format at default settings only** — no aggressive flags. Files that fail
   TokenPress's own verification are reported and left untouched; they still
   take part in the test run, unformatted. The copy is a git checkout, so
   `git status --porcelain` reports exactly how many files were rewritten.
4. **Run the project's real test suite on both copies**, each in its own
   isolated environment (details below).
5. **Diff per-test-id outcomes, not summary counts.** Both runs are reduced to
   a sorted `outcome<TAB>test id` listing and compared with `diff`. Equal
   pass/fail *totals* with a swapped pass and fail would not slip through.

Exit codes are result-vs-infrastructure separated: **0** = outcomes identical,
**1** = outcomes diverged (a result), **2** = usage or infrastructure error,
i.e. the comparison never ran. On a non-zero exit the work directory is kept
for inspection instead of being deleted.

Isolation, per target:

| Target | Runner | Isolation |
|---|---|---|
| requests | `pytest tests -q -p no:cacheprovider --junitxml=…` | one shared venv from `requirements-dev.txt` (byte-identical dependencies), editable install repointed at each copy and asserted via `requests.__file__`; private `TMPDIR` per run; proxy env vars stripped for both runs |
| ripgrep | `cargo test --workspace --offline --no-fail-fast` | private `CARGO_TARGET_DIR` and `TMPDIR` per run; one shared `cargo fetch` warmed before both runs so `--offline` resolves identical dependencies; `--workspace` without `--features pcre2`, matching ripgrep's own non-pcre2 CI job |

Two normalizations are needed to make the comparison meaningful, and both are
deliberately narrow:

* **requests**: parametrized test ids can embed the tree path (the suite
  parametrizes over `__file__`), so the run directory is rewritten to a
  `<tree>` placeholder.
* **ripgrep**: doc-test ids embed the line number of their code block
  (`lib.rs - f (line 42)`). Dropping `//` comments legitimately moves every
  doc comment below them, so ` (line N)` is stripped and the doc tests stay
  in the comparison — doc comments themselves survive formatting, so the doc
  tests must still run and still reach the same outcome. Ids that collide
  after stripping are kept as duplicate rows and compared as a multiset, so a
  change in any single code block still shows up.

**Methodology gotcha worth recording**: the first requests attempt reported a
false divergence. requests' own `extract_zipped_paths()` caches its output
under `tempfile.gettempdir()` and skips the extraction when that file already
exists — with a shared temp directory the second run was effectively testing
against the first run's extracted sources. Hence the private `TMPDIR` per run.
Similarly, proxy environment variables are dropped for both runs because parts
of the suite assert on proxy handling and an inherited `HTTPS_PROXY` fails
tests for reasons that have nothing to do with formatting.

### requests v2.32.3 — IDENTICAL

| | Value |
|---|---|
| `.py` files | 36 |
| Rewritten | 35 |
| Refused by verification | 0 |
| Verdict | **IDENTICAL** |

| Outcome | Baseline | Formatted |
|---|---|---|
| passed | 585 | 585 |
| failed | 5 | 5 |
| skipped | 15 | 15 |
| xfailed | 1 | 1 |

Every test reached the same outcome on both copies. The 5 failures are
sandbox network artifacts (no outbound network in the run environment); they
fail identically on the unformatted copy, which is exactly why the comparison
is against a baseline rather than against "all green".

### ripgrep 14.1.1 — DIVERGED, then IDENTICAL after a fix

| | Value |
|---|---|
| `.rs` files | 98 |
| Rewritten | 98 |
| Refused by verification | 0 |
| Verdict (first run) | **DIVERGED** — 1 of 1109 tests |
| Verdict (after fix `b1572d3`) | **IDENTICAL** — 1106 ok / 3 ignored, exit 0 |

**The first ripgrep run found a real bug, and this is the most important
result in this file.** Across 22 test binaries and 1109 tests, exactly one
test — `grep_cli`'s `patterns_from_reader` doc test — failed on the
formatted copy and passed on the baseline.

Root cause, in `crates/tokenpress-rust/src/emit.rs`: doc attributes were
re-emitted line by line, so a doc line whose literal contains `\` or `"` fell
back to the raw `#[doc = "…"]` form while its neighbours in the *same*
contiguous doc block stayed sugared (`///`). rustdoc's fragment-unindent step
treats the two forms differently — it strips the conventional leading space
from sugared fragments but keeps it on raw ones — so a mixed block
reconstructs the doc example with a stray leading space on exactly the raw
lines. In this case that space landed inside a multi-line string literal in
the example, changing what the example asserts.

Token/AST equivalence is structurally blind to this class of change: `/// x`
and `#[doc = " x"]` are the same token stream, so format-time verification
passed and only the upstream suite caught it. Fix (commit `b1572d3`,
written test-first): a contiguous same-kind doc block is now emitted in one
consistent form — fully sugared, or fully raw `#[doc = …]` if any line needs
the escape fallback — so rustdoc unindents the whole block uniformly. The
re-run after the fix is IDENTICAL.

This is the whole argument for running the upstream suites: a 1-in-1109
behavioral divergence that the formatter's own verification could not see, in
a corpus that had already passed every structural check reported earlier in
this file.

### Scope and caveats

* These runs verify **default settings only**. The aggressive settings above
  (`--py-strip-comments`, `--py-strip-annotations`, `--rs-strip-doc-comments`)
  are knowingly lossy and are not covered by this harness — stripping doc
  comments would delete the doc tests being compared.
* The Rust comment-loss caveat still stands: `//` and `/* */` comments are
  dropped, and no test suite can detect that, because comments do not run.
  Behavioral equivalence is not context equivalence.
* Only two corpora are covered (requests, ripgrep). The larger corpora in the
  table above are verified structurally, not behaviorally.
* The 5 requests failures are environment artifacts, not upstream-green
  results; the claim is *identical outcomes*, not *all tests pass*.

### Reproduce

```bash
./benchmarks/verify-upstream.sh requests   # pytest, junit per-test-id diff
./benchmarks/verify-upstream.sh ripgrep    # cargo test, multiset diff
./benchmarks/verify-upstream.sh all
# exit 0 = identical, 1 = diverged, 2 = the comparison never ran
```
