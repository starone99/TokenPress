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

TODO: run the upstream test suites (pytest / cargo test) on formatted
corpora as public proof of behavior preservation.
