# TokenPress Benchmark Results

Measured: 2026-07-31 (open-model tokenizers added 2026-08-01; the
JavaScript/TypeScript corpus added 2026-08-02; the Ruby corpus added
2026-08-02; per-tokenizer aggressive run and the express open-model rows
added 2026-08-02; the Go corpus added 2026-08-04)
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
| expressjs/express | v5.2.1 | `dbac741a` | 142 `.js` |
| rack/rack | v3.2.6 | `e1f22fdb` | 105 Ruby (93 `.rb`, 9 `.ru`, `rack.gemspec`, `Gemfile`, `Rakefile`) |
| gin-gonic/gin | v1.11.0 | `6ad6205e` | 95 `.go` (whole tree) |

**Every parseable file passed verification, with one measured exception**
(re-parse + token/AST equivalence). The only skipped files across ~13,200 are
two intentionally broken test fixtures — Django's `tests_syntax_error.py`
(invalid Python by design) and LangChain's `non-utf8-encoding.py` (invalid
UTF-8 by design), both correctly rejected with per-file errors — plus one
genuine refusal in rack, `lib/rack/utils.rb`, which is a known Ruby
over-refusal class documented in the Ruby section below. The 95 Go files
added 2026-08-04 add **no** refusals and no rejected inputs, at either
setting and either tokenizer. A refusal writes
nothing and leaves the file untouched; it is the core invariant working, not
a corruption. Runs made with the JS/TS-enabled CLI (2026-08-02 onward)
additionally reject three django `.js` files as invalid input —
template/fixture files, listed in the open-model subsection below.

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

### Default settings (Python: comments/docstrings/annotations kept; Rust: doc comments kept, regular comments dropped; JS/TS: only some comments kept; Ruby: everything kept; Go: everything kept; adjacent imports merged)

Context-lossless for Python — whitespace/blank-line/indent minimization plus
PY09 import merging only.

**Rust is not context-lossless, even at default settings.** The Rust backend
re-emits from the `syn` token stream, which does not carry regular comments:
`//` and `/* */` comments are always dropped, and only doc comments (`///`,
`//!`) survive. Part of the default savings for every Rust corpus below —
ripgrep in this table, tokio and uv in the next — therefore comes from
discarded comments, not from syntactic noise alone.

**JavaScript/TypeScript is not context-lossless at default settings either,
and the loss is partial rather than total.** The JS/TS backend re-emits from
its own code generator: **trailing comments and comments in expression
position are always dropped, with or without `--js-strip-comments`.** Only
leading statement-level comments, jsdoc (`/** */`), annotation comments (such
as `#__PURE__`) and legal comments (`//!`, `/*!`, `@license`, `@preserve`)
survive. That is a property of the generator, not an option, and verification
cannot detect it because its canonical form is comment-free by construction.
Part of express's default savings below therefore comes from discarded
comments. The CLI prints this caveat on stderr once per run that touches a
JS/TS file.

**Ruby, unlike Rust and JavaScript/TypeScript, drops nothing at default
settings.** The Ruby backend rewrites the source in place rather than
re-emitting from a token stream, so every `#` comment and every `=begin`/`=end`
embdoc survives, along with the shebang and the magic-comment window. Ruby's
default savings below are therefore **pure whitespace** — the
context-lossless case, like Python — and comment removal is entirely opt-in
behind `--ruby-strip-comments`. The CLI prints no caveat warning for a
Ruby-only run, because there is nothing to warn about.

**Go behaves like Ruby: nothing is discarded at default settings.** The Go
backend rewrites whitespace in place, so every `//` and `/* */` comment
survives, including the ones the toolchain reads as directives (`//go:`
lines, `/*line*/`, build constraints and the cgo preamble) — those survive
`--go-strip-comments` too, being semantic rather than prose. Newlines are
kept as well, deliberately: Go's automatic semicolon insertion makes a line
break load-bearing, so the emitter removes indentation and blank lines but
never joins two lines. Go's default savings below are therefore **pure
whitespace**, and the CLI prints no caveat warning for a Go-only run.

**What no backend preserves is line numbers.** Removing blank lines and
indentation moves every line below the removal, in Ruby and Go exactly as in
Rust and JS/TS. That is invisible to token/AST equivalence, and the rack run below is
the first corpus in this file whose own test suite asserts on a line number
directly; see the behavioral-verification section.

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

### JavaScript/TypeScript — expressjs/express (default settings, added 2026-08-02)

Measured 2026-08-02, Linux LF checkout, rustc 1.95.0, on the commit pinned in
`fetch.sh`/`fetch.ps1` (`dbac741a49a5a64336b70c06e85c2e2706e36336`, tag
`v5.2.1`). This is the first corpus for the `tokenpress-js` backend, so it is
reported on its own rather than folded into the tables above: those were
measured on Windows with a CRLF checkout, and the line-ending caveat further
down applies to any comparison across the two.

The two embedded-tokenizer rows were measured in that environment, which
cannot reach huggingface.co. The Qwen3.6 / GLM-5.2 / Kimi K3 rows were
filled in on 2026-08-02 from the maintainer's Windows machine, using an LF
checkout of the same pin; the `o200k_base` totals reproduced exactly
(135,740 → 112,307), so the two platforms are directly comparable.

The whole tree is measured, matching every other corpus: `index.js`, `lib/`,
`test/`, `examples/` and `benchmarks/`. All 142 files are `.js`; there is no
TypeScript and no JSX in this pin, so the **JSX-text caveat (JSX children are
re-emitted verbatim and save nothing) is untested by these numbers** and a
`.jsx`/`.tsx`-heavy corpus would be expected to save less.

| Corpus | Tokenizer | Files | Before | After | Saved |
|---|---|---|---|---|---|
| express | o200k_base | 142 | 135,740 | 112,307 | **-17.3%** |
| express | cl100k_base | 142 | 135,206 | 111,338 | **-17.7%** |
| express | Qwen3.6 | 142 | 156,175 | 117,208 | **-25.0%** |
| express | GLM-5.2 | 142 | 135,795 | 111,924 | **-17.6%** |
| express | Kimi K3 | 142 | 136,412 | 112,724 | **-17.4%** |

Qwen3.6 stands out on JavaScript the same way it does on Rust: it prices
whitespace runs comparatively high (its before-count is 156k where the
others sit at ~136k), so whitespace-minimal re-emission saves +7.7pp more
than on `o200k_base`.

**0 verification refusals** across all 142 files, at both tokenizers. Absolute
saving at o200k: 23,433 tokens per full-repo prompt.

Read `-17.3%` with the comment caveat above: at default settings this run has
already lost express's trailing and expression-position comments, so it is not
the context-lossless number that the Python corpora report.

```bash
target/release/tokenpress stats benchmarks/corpus/express --tokenizer o200k_base
```

### Ruby — rack/rack (default settings, added 2026-08-02)

Measured 2026-08-02, Linux LF checkout, rustc 1.95.0, ruby 3.3.6, on the
commit pinned in `fetch.sh`/`fetch.ps1`
(`e1f22fdbe99afd2126b6fbf05bb12399359574b7`, tag `v3.2.6`). This is the first
corpus for the `tokenpress-ruby` backend, and it is reported on its own for
the same reason express is: the tables above were measured on Windows with a
CRLF checkout, and the line-ending caveat further down applies to any
comparison across the two.

**Only the two embedded tokenizers were measured** — the measurement
environment cannot reach huggingface.co, so the Qwen3.6 / GLM-5.2 / Kimi K3
columns are pending here exactly as they are for express and for the
full-aggressive run below.

The whole tree is measured: 49 files under `lib/`, 53 under `test/` and the 3
build files at the root (rack's other directories — `contrib/`, `config/`,
`docs/` — hold no Ruby).
**Every path class the backend claims is exercised except `.rake`**, which
rack has none of — 93 `.rb`, 9 `.ru` (rack's `config.ru` fixtures),
`rack.gemspec`, `Gemfile` and `Rakefile`, 105 files in total. The two
extensionless files are matched by exact name, which is what makes rack a
useful first Ruby corpus rather than just a `.rb` count.

The Ruby paths are passed explicitly rather than handing the tree to the
formatter: rack ships two files named `*.js` under `test/cgi/assets/` whose
entire content is `### TestFile ###`. They are placeholders for the CGI asset
tests and are not JavaScript, so a whole-tree run hands them to the JS backend
and reports two parse errors — noise from another language in a Ruby
measurement. They contribute nothing to the totals either way.

| Corpus | Tokenizer | Files | Before | After | Saved |
|---|---|---|---|---|---|
| rack | o200k_base | 104 | 187,175 | 170,012 | **-9.2%** |
| rack | cl100k_base | 104 | 186,474 | 169,907 | **-8.9%** |
| rack | Qwen3.6 | — | — | — | *pending* |
| rack | GLM-5.2 | — | — | — | *pending* |
| rack | Kimi K3 | — | — | — | *pending* |

`Files` is 104 of the 105, because of **1 verification refusal**:
`lib/rack/utils.rb`. It refuses at both settings and both tokenizers, and it
is a known over-refusal class, not a new defect — see the subsection below.
Absolute saving at o200k: 17,163 tokens per full-repo prompt.

Unlike express's `-17.3%`, this `-9.2%` **is** context-lossless: nothing but
whitespace was removed, every comment and embdoc is still in the output. It
sits in the same range as the Python default-settings numbers (requests
`-9.0%`) for the same reason — the honest, nothing-discarded case costs the
most.

```bash
find benchmarks/corpus/rack -not -path '*/.git/*' \
    \( -name '*.rb' -o -name '*.rake' -o -name '*.gemspec' -o -name '*.ru' \
    -o -name 'Gemfile' -o -name 'Rakefile' \) -print0 |
    xargs -0 target/release/tokenpress stats --tokenizer o200k_base
```

#### The one Ruby refusal, identified

`lib/rack/utils.rb` fails verification with `output AST differs from input` at
default settings. It is over-refusal **class 1** from
`crates/tokenpress-ruby/src/verify.rs`: a location slice that spans more than
one token, here a multi-line index call, whose `message_loc` covers the whole
bracket pair, so joining the lines inside it moves the slice even though
nothing semantic changed.

Delta-debugged down to this shape, which rack has in
`SYMBOL_TO_STATUS_CODE`:

```ruby
X = Hash[*Y.map { |a, b|
  [a, b]
}.flatten]
```

`tokenpress stats` on that file alone reproduces the refusal; putting the same
call on one line formats fine. The refusal is in the safe direction — a
formattable file is left alone, and no output that fails the check is ever
written.

### Go — gin-gonic/gin (default settings, added 2026-08-04)

Measured 2026-08-04, Linux LF checkout, rustc 1.95.0, go 1.24.7, on the
commit pinned in `fetch.sh`/`fetch.ps1`
(`6ad6205e9c94a4b8a320219e28c37c29d22a7a2c`, tag `v1.11.0`). This is the
first corpus for the `tokenpress-go` backend, and it is reported on its own
for the same reason express and rack are: the tables above were measured on
Windows with a CRLF checkout, and the line-ending caveat further down applies
to any comparison across the two.

**Only the two embedded tokenizers were measured** — the measurement
environment cannot reach huggingface.co, so the Qwen3.6 / GLM-5.2 / Kimi K3
columns are pending here exactly as they are for rack. **No open-model number
exists for Go yet, and none may be quoted until one is measured on the
maintainer's machine**; in particular the `o200k_base` figure is not a usable
proxy for them, as the Qwen3.6 column elsewhere in this file makes plain.

The whole tree is measured, all 95 files being `.go`. This pin holds no file
of any other language TokenPress claims — no `.js`, `.py`, `.rs` or Ruby path
anywhere — so unlike rack it can simply be handed to the formatter as a
directory with nothing misrouted to another backend. The count is taken with
`find -type f`: Go's own source tree contains a *directory* named
`not_a_file.go`, so a bare `-name '*.go'` test is not a file count anywhere
in this project.

| Corpus | Tokenizer | Files | Before | After | Saved |
|---|---|---|---|---|---|
| gin | o200k_base | 95 | 173,337 | 162,297 | **-6.4%** |
| gin | cl100k_base | 95 | 172,761 | 162,002 | **-6.2%** |
| gin | Qwen3.6 | — | — | — | *pending* |
| gin | GLM-5.2 | — | — | — | *pending* |
| gin | Kimi K3 | — | — | — | *pending* |

**0 verification refusals** across all 95 files, at both tokenizers and at
both settings. Absolute saving at o200k: 11,040 tokens per full-repo prompt.

Like rack's `-9.2%` and unlike express's `-17.3%`, this `-6.4%` **is**
context-lossless: every comment survives and only whitespace was removed.

**-6.4% is the lowest default-settings figure of any corpus in this file, and
the reason is `gofmt`.** Go source in the wild is already whitespace-canonical
before TokenPress sees it, which leaves far less to remove than in any other
language here:

* **Indentation is one tab per nesting level, never a run of spaces.** Of
  gin's 21,590 lines, 14,008 begin with a tab and exactly 11 begin with a
  space (all inside string literals or comments). A four-space Python indent
  costs more tokens than a single tab does at every level of nesting, so
  deleting Go's indentation recovers less than deleting Python's or Ruby's.
* **`gofmt` already collapses consecutive blank lines to one** and strips
  trailing whitespace, so there are no runs of blank lines to squeeze — 3,282
  of the 21,590 lines are blank, and each is worth about one token.
* **Newlines stay.** Go's automatic semicolon insertion makes a line break a
  syntactic token, so the emitter never joins two lines; every one of those
  21,590 line breaks is still in the output. Rust, whose newlines carry no
  syntax, deletes them outright — and additionally loses every `//` and
  `/* */` comment through the `syn` token stream. Those two together are why
  ripgrep's default run reads -18.9% where gin's reads -6.4% (across the
  CRLF/LF platform boundary described below, which is worth well under a
  point and does not change the comparison).
* **Comments are all kept at this setting**, unlike Rust and JS/TS.

In other words, the number is low because Go's ecosystem already did most of
the whitespace work — which is a fact about the language's culture, not a
weakness in the backend. The comment lever, when it is pulled, is the largest
of the three comment-stripping deltas measured in this file (+13.0pp against
Ruby's +11.6pp and JS/TS's +8.1pp; see `--go-strip-comments` below).

```bash
target/release/tokenpress stats benchmarks/corpus/gin --tokenizer o200k_base
```

#### External verification over the whole Go corpus

Separately from the numbers above, all 95 files were re-run at
`--verify external`, which additionally hands every output to `gofmt -e`:

```bash
target/release/tokenpress stats benchmarks/corpus/gin \
    --tokenizer o200k_base --verify external
```

**All 95 files pass**, with token totals identical to the `--verify ast` run
(173,337 → 162,297), 0 refusals and exit 0.

The timing figures in this file stay on `--verify ast` on purpose. External
verification is one `gofmt` probe plus two `gofmt` spawns per file, so its
wall time measures process startup, not TokenPress: the whole corpus takes
**0.60–0.67 s** at `--verify ast` and **1.43 s** at `--verify external` on
this machine — a 2.3× difference that is entirely `gofmt`.

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
(`benchmarks/fetch.sh`); the express and rack rows were measured the same way
on 2026-08-02, when the JS/TS and Ruby backends and their corpora were added,
and the gin row on 2026-08-04 when the Go corpus was added.
**Only the two embedded tokenizers were measured in this run** — the
measurement environment cannot reach huggingface.co. The Qwen3.6 / GLM-5.2 /
Kimi K3 columns were measured separately on 2026-08-02 from the maintainer's
Windows machine and are reported under "Open-model tokenizers" below; that
run predates the rack corpus, so rack has no open-model figures.

Exact flags:

* Python corpora (requests, django, fastapi, langchain, transformers):
  `--py-strip-comments --py-strip-annotations --py-strip-docstrings`
* Rust corpora (ripgrep, tokio): `--rs-strip-doc-comments`
* Mixed corpus (uv, 624 `.rs` + 94 `.py`): all four flags. Per-language
  flags are language-scoped — passing the Rust flag to a pure-Python tree
  (or vice versa) is a verified no-op, so a mixed tree can safely take all
  four at once.
* JavaScript corpus (express): `--js-strip-comments`. It only reaches the
  comments the backend keeps at all — the trailing and expression-position
  comments are gone either way (see the caveat above), so this flag is a
  smaller lever for JS/TS than `--rs-strip-doc-comments` is for Rust.
* Ruby corpus (rack): `--ruby-strip-comments`. This is the opposite case from
  JS/TS — the Ruby default keeps *every* comment and embdoc, so the flag is
  the full difference between "all comments kept" and "none", and it is the
  single largest lever the Ruby backend has. The shebang and the leading
  magic-comment window survive it (`# frozen_string_literal: true` and
  friends are semantic, not prose).
* Go corpus (gin): `--go-strip-comments`. Structurally the same case as Ruby
  — the Go default keeps every comment, so the flag is the whole
  comments-kept-vs-dropped difference and the only lossy lever the Go backend
  has. The directive comments survive it (`//go:` lines, `/*line*/`, build
  constraints and the cgo preamble are semantic, not prose).

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
| express | o200k_base | 142 | 135,740 | 101,253 | **-25.4%** |
| express | cl100k_base | 142 | 135,206 | 100,122 | **-25.9%** |
| rack | o200k_base | 104 | 187,175 | 148,273 | **-20.8%** |
| rack | cl100k_base | 104 | 186,474 | 148,157 | **-20.5%** |
| gin | o200k_base | 95 | 173,337 | 139,758 | **-19.4%** |
| gin | cl100k_base | 95 | 172,761 | 138,297 | **-19.9%** |

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

##### What `--js-strip-comments` adds

Same corpus, same LF checkout, same tokenizer (o200k_base), the added flag
being the only difference:

| Corpus | Without comment stripping | With `--js-strip-comments` | Delta |
|---|---|---|---|
| express | -17.3% | **-25.4%** | +8.1pp |

+8.1pp is the *whole* opt-in cost of comment stripping for JS/TS, and it is
smaller than it looks: the trailing and expression-position comments were
already gone from the -17.3% baseline, so the flag only buys the leading
statement-level comments, jsdoc, annotation and legal comments on top. Unlike
`--rs-strip-doc-comments`, it is not the difference between "all comments
kept" and "no comments kept" — no JS/TS setting keeps all of them.

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

##### What `--ruby-strip-comments` adds

Same corpus, same LF checkout, the added flag being the only difference:

| Corpus | Tokenizer | Without comment stripping | With `--ruby-strip-comments` | Delta |
|---|---|---|---|---|
| rack | o200k_base | -9.2% | **-20.8%** | +11.6pp |
| rack | cl100k_base | -8.9% | **-20.5%** | +11.7pp |

+11.6pp is a bigger jump than express's +8.1pp from `--js-strip-comments`, and
for a structural reason rather than a corpus one: the JS/TS flag starts from a
baseline that has already lost the trailing and expression-position comments,
whereas the Ruby flag starts from a baseline that has lost nothing. Ruby and
Go are the two backends where the strip flag really is the whole of the
comments-kept-vs-dropped difference; the Go figures are in the next
subsection, and they are larger still.

The **1 refusal is the same file at both settings** (`lib/rack/utils.rb`, the
multi-line index call above), so nothing new is refused by adding the flag.
The second known Ruby over-refusal class — a whitelisted magic-comment key
appearing lexically *after* the first code token, which `--ruby-strip-comments`
deletes and the verifier then rejects — **did not fire anywhere in rack**.

##### What `--go-strip-comments` adds

Same corpus, same LF checkout, the added flag being the only difference:

| Corpus | Tokenizer | Without comment stripping | With `--go-strip-comments` | Delta |
|---|---|---|---|---|
| gin | o200k_base | -6.4% | **-19.4%** | +13.0pp |
| gin | cl100k_base | -6.2% | **-19.9%** | +13.7pp |

**+13.0pp is the largest comment-stripping delta measured in this file**,
ahead of rack's +11.6pp and express's +8.1pp, and it is the same structural
reason as Ruby's only more so: the Go default keeps *every* comment, so the
flag is the whole difference, and Go's convention of a doc comment on each
exported identifier makes gin comment-dense to begin with. It flips the two
Go figures' ordering against each other — `cl100k_base` saves *more* than
`o200k_base` under the flag (-19.9% vs -19.4%) where at default settings it
saved less (-6.2% vs -6.4%) — because the comment prose the flag removes
tokenizes differently from the whitespace the default run removes.

**0 refusals with the flag**, at both tokenizers, exactly as without it, and
the file count is unchanged at 95. Absolute saving at o200k: 33,579 tokens
per full-repo prompt.

`--go-strip-comments` is the Go backend's only lossy flag. Note what it does
*not* remove: the comments the Go toolchain reads as directives — `//go:`
lines, `/*line*/`, build constraints and the cgo preamble — survive it, so
stripping comments cannot change how a package builds.

##### Open-model tokenizers (Qwen3.6 / GLM-5.2 / Kimi K3, added 2026-08-02)

Measured 2026-08-02 on the maintainer's Windows machine, rustc 1.95.0, from
an **LF checkout** of the same pinned commits (clones made with
`core.autocrlf=false`), so these rows extend the Linux LF table above, not
the historical CRLF tables. Two sanity checks anchor the platforms:
requests aggressive and express default at `o200k_base` reproduced the
Linux totals exactly (86,331 → 55,265 and 135,740 → 112,307).

One deliberate difference: this run used the current CLI, which includes
the JS/TS backend added 2026-08-02, so `.js` files inside the Python and
mixed corpora are formatted too. The flag matrix is unchanged —
`--js-strip-comments` was still applied only to express, so those embedded
`.js` files are formatted at default JS settings. The two
embedded-tokenizer columns were re-measured with the same binary so all
five columns of the table below share one file set. File-count changes
against the 2026-08-01 table: fastapi 1,136 → 1,140 (+4 `.js`), langchain
2,530 → 2,531 (+1 — its only other `.js` lives under `.github/`, which the
gitignore-aware walk skips as hidden), uv 718 → 719 (+1), django
2,924 → 3,032 (+108 `.js` formatted; 3 more rejected as invalid input,
below). Only django's percentage moves visibly: its admin/static JS adds
~423k `o200k` tokens that compress less than Python, so its o200k saving
reads -22.2% here vs -23.1% in the Python-only table. That is a coverage
change, not a formatter change.

Savings per corpus, aggressive flags as above (bold = clears the ≥40%
showcase bar):

| Corpus | Files | o200k_base | cl100k_base | Qwen3.6 | GLM-5.2 | Kimi K3 |
|---|---|---|---|---|---|---|
| tokio | 790 | **-50.5%** | **-50.9%** | **-55.2%** | **-50.9%** | **-50.6%** |
| ripgrep | 98 | -37.4% | -37.6% | **-42.7%** | -37.6% | -37.3% |
| langchain | 2,531 | -38.8% | -39.2% | **-41.1%** | -39.3% | -38.6% |
| fastapi | 1,140 | -36.1% | -36.8% | **-40.1%** | -36.7% | -36.3% |
| requests | 36 | -36.0% | -36.3% | -36.5% | -36.2% | -35.4% |
| transformers | 4,700 | -34.9% | -35.6% | -36.1% | -35.5% | -34.8% |
| express | 142 | -25.4% | -25.9% | -33.3% | -25.9% | -25.5% |
| django | 3,032 | -22.2% | -22.7% | -24.8% | -22.8% | -21.9% |
| uv | 719 | -21.5% | -21.8% | -24.7% | -21.4% | -21.7% |

Raw counts for the three open-model tokenizers:

| Corpus | Qwen3.6 before | after | GLM-5.2 before | after | Kimi K3 before | after |
|---|---|---|---|---|---|---|
| tokio | 1,542,030 | 690,601 | 1,394,952 | 684,919 | 1,390,718 | 687,497 |
| ripgrep | 458,041 | 262,670 | 415,968 | 259,500 | 416,120 | 260,957 |
| langchain | 3,240,913 | 1,908,563 | 2,945,096 | 1,788,970 | 2,946,274 | 1,808,322 |
| fastapi | 830,093 | 497,465 | 734,771 | 465,058 | 731,460 | 465,816 |
| requests | 94,786 | 60,211 | 86,278 | 55,037 | 86,409 | 55,858 |
| transformers | 18,431,508 | 11,784,285 | 17,020,980 | 10,986,756 | 16,936,419 | 11,039,030 |
| express | 156,175 | 104,226 | 135,795 | 100,685 | 136,412 | 101,572 |
| django | 5,050,621 | 3,800,285 | 4,550,506 | 3,513,998 | 4,625,412 | 3,610,917 |
| uv | 5,559,284 | 4,185,455 | 4,862,547 | 3,821,025 | 4,742,042 | 3,715,145 |

**0 verification refusals** at every tokenizer. Three django `.js` files
are rejected as invalid *input* — beyond the two broken fixtures documented
at the top of this file — all at parse, with nothing written:
`django/views/templates/i18n_catalog.js` (a Django template containing
`{% %}` tags, not JavaScript), `tests/i18n/commands/javascript.js` (an
i18n-scanner fixture), and
`tests/staticfiles_tests/project/documents/cached/module.js` (duplicate
export). The langchain run still reports only the known non-UTF-8 fixture.

**Coverage limit:** this run covers the nine corpora that existed on
2026-08-02 when it was made. Two corpora are missing from it. The rack (Ruby)
corpus was added later the same day; the gin (Go) corpus was added
2026-08-04. **Neither has any open-model figures.** Their embedded-tokenizer
aggressive results (-20.8% / -20.5% and -19.4% / -19.9%) are far below the
≥40% bar, so their absence cannot change any candidate list below — but no
Qwen3.6 / GLM-5.2 / Kimi K3 number should be quoted for Ruby or for Go until
they are re-measured on the maintainer's machine. The `o200k_base` figure is
explicitly *not* a proxy for them: the table above shows Qwen3.6 diverging
from `o200k_base` by up to +7.9pp on a single corpus.

#### Showcase candidates (≥40% aggressive reduction)

Recorded for the ROADMAP P3 task ("hunt for well-known projects per
supported language where the aggressive setting clears ≥40% token
reduction"). The hunt is defined **per target-model tokenizer** — savings
differ enough by tokenizer that a single list would be wrong for most
models. Embedded-tokenizer columns measured 2026-08-01 (re-measured
2026-08-02 with the JS-enabled CLI — unchanged except django, see the
open-model subsection); open-model columns measured 2026-08-02. rack was
added 2026-08-02 and gin 2026-08-04, both measured on the embedded tokenizers
only (-20.8% / -20.5% and -19.4% / -19.9%, below the bar on all four); they
are absent from the open-model columns. **Neither is a ≥40% candidate, so the
lists below are unchanged by their addition.**

**Per-tokenizer candidate lists:**

| Tokenizer | ≥40% candidates (aggressive flags) |
|---|---|
| o200k_base | tokio **-50.5%** |
| cl100k_base | tokio **-50.9%** |
| Qwen3.6 | tokio **-55.2%**, ripgrep **-42.7%**, langchain **-41.1%**, fastapi **-40.1%** |
| GLM-5.2 | tokio **-50.9%** |
| Kimi K3 | tokio **-50.6%** |

tokio clears the bar on every tokenizer measured and stays the headline
everywhere; at Qwen3.6 it reaches **-55.2%** (1,542,030 → 690,601, an
absolute saving of 851,429 tokens per full-repo prompt). The Qwen3.6 list
is four deep because Qwen prices whitespace runs comparatively expensively;
selecting candidates on the `o200k_base` proxy would have missed three of
its four — which is exactly why this hunt is per-tokenizer. Gemma is not
measured: the official `google/gemma-*` repos are gated (license acceptance
plus auth token), so pinning its `tokenizer.json` needs an
authenticated-download story nothing else needs; it was excluded from this
run by maintainer decision (2026-08-02).

**Clears ≥40% on the embedded tokenizers — 1 of 11 corpora:**

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

On GLM-5.2 and Kimi K3 the same projects stay near misses (langchain -39.3%
/ -38.6%, ripgrep -37.6% / -37.3%, fastapi -36.7% / -36.3%); the full grid
is in the open-model subsection above.

Flags: Python projects `--py-strip-comments --py-strip-annotations
--py-strip-docstrings`; ripgrep `--rs-strip-doc-comments`.

**Well under 40%:** expressjs/express `dbac741a` (-25.4% / -25.9%,
`--js-strip-comments`), django/django `50d706d0` (-23.1% / -23.8%),
astral-sh/uv `be765050` (-21.5% / -21.8%, all four flags), rack/rack
`e1f22fdb` (-20.8% / -20.5%, `--ruby-strip-comments`) and gin-gonic/gin
`6ad6205e` (-19.4% / -19.9%, `--go-strip-comments`). Django's bulk is
test fixtures and data tables rather than prose; uv is dominated by Rust
source whose doc comments are a small fraction of the tree; gin starts from
`gofmt`-canonical whitespace, so its default run has the least of any corpus
here left to remove. express is the
only JavaScript corpus and the JS/TS backend has no lever comparable to
`--rs-strip-doc-comments` or `--py-strip-docstrings` — no JS/TS showcase
candidate has been hunted for yet, so **no ≥40% claim is made for JavaScript
or TypeScript in either direction**; one corpus is not a search. The same
holds for Ruby: rack is the only Ruby corpus, `--ruby-strip-comments` is the
backend's only lossy flag, and **no ≥40% claim is made for Ruby in either
direction** either. And it holds for Go: gin is the only Go corpus,
`--go-strip-comments` is that backend's only lossy flag, and **no ≥40% claim
is made for Go in either direction** either.

Two caveats on this list:

* **ripgrep's number is not directly comparable to the historical aggressive
  table** (-37.4% here vs -38.2% there). The Rust flag set did not change —
  `--rs-strip-doc-comments` is the same single flag in both runs — and the
  difference is entirely the CRLF→LF checkout described above.
* **Each list is valid only for models using that tokenizer.** The per-
  tokenizer measurement (2026-08-02) confirmed the gap is decisive: three of
  Qwen3.6's four candidates are invisible on the `o200k_base` proxy. Judge
  ≥40% membership on the tokenizer of the model you actually run. Gemma
  remains unmeasured (gated repos — see the note above), so no candidate
  list exists for Gemma-tokenizer models.

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
* JavaScript lands between the two: express saves -17.3% at default settings
  and -25.4% with `--js-strip-comments`. Like Rust, JS/TS newlines carry
  little syntax and the backend already discards some comments; unlike Rust,
  it has no doc-comment-sized lever to pull on top, because the comments the
  strip flag reaches are the smaller half to begin with.
* Ruby behaves like Python, not like Rust or JS/TS: rack's default `-9.2%` is
  the context-lossless number (nothing but whitespace removed), and it lands
  next to requests' `-9.0%` rather than next to express's `-17.3%`. The whole
  of Ruby's comment cost is in one opt-in flag, which is why
  `--ruby-strip-comments` moves it further (+11.6pp) than
  `--js-strip-comments` moves express (+8.1pp).
* Go is the extreme of the same axis, in both directions. Its default `-6.4%`
  is the **lowest** default-settings figure in this file, and its
  `--go-strip-comments` delta (+13.0pp) is the **largest of the three
  comment-stripping deltas measured here** (Ruby +11.6pp, JS/TS +8.1pp; Rust
  has no equivalent measurement in this file). Both follow from the same two
  facts:
  `gofmt` has already normalised the whitespace before TokenPress sees the
  file (one tab per level, no runs of blank lines), and Go's automatic
  semicolon insertion forbids joining lines, so the newlines Rust deletes
  outright all stay. What is left to remove at default settings is small;
  what is left in the comments is not.
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
.\target\release\tokenpress.exe stats benchmarks\corpus\express --tokenizer o200k_base --js-strip-comments
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
target/release/tokenpress stats benchmarks/corpus/express --tokenizer o200k_base

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

# full aggressive - JavaScript corpus
target/release/tokenpress stats benchmarks/corpus/express --tokenizer o200k_base \
    --js-strip-comments

# Ruby corpus - the paths are listed explicitly so that rack's two non-JS
# `*.js` placeholder fixtures are not handed to the JS backend
find benchmarks/corpus/rack -not -path '*/.git/*' \
    \( -name '*.rb' -o -name '*.rake' -o -name '*.gemspec' -o -name '*.ru' \
    -o -name 'Gemfile' -o -name 'Rakefile' \) -print0 >/tmp/rack-files.nul
xargs -0 target/release/tokenpress stats --tokenizer o200k_base \
    </tmp/rack-files.nul                       # default settings
xargs -0 target/release/tokenpress stats --tokenizer o200k_base \
    --ruby-strip-comments </tmp/rack-files.nul # full aggressive

# Go corpus - the pin holds no file of any other supported language, so the
# tree can be passed directly
target/release/tokenpress stats benchmarks/corpus/gin --tokenizer o200k_base
target/release/tokenpress stats benchmarks/corpus/gin --tokenizer o200k_base \
    --go-strip-comments                        # full aggressive

# separate external-verification pass over the same corpus (needs gofmt on
# PATH); this is a verification run, not a timing run - see the note above
target/release/tokenpress stats benchmarks/corpus/gin --tokenizer o200k_base \
    --verify external

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

The script
(`benchmarks/verify-upstream.sh <requests|ripgrep|express|rack|go|all>`) does
the same thing for every target:

1. **Pinned corpus, SHA-asserted.** The corpus is cloned at the same tag as
   `fetch.ps1`/`fetch.sh` and its `HEAD` is asserted against a hard-coded
   commit SHA, so a retagged upstream cannot silently change what is verified
   (requests `0e322af8`, ripgrep `4649aa97`, express `dbac741a`, rack
   `e1f22fdb`, gin `6ad6205e`).
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
| express | `mocha --require test/support/env --check-leaks test/ test/acceptance/` — the exact arguments of express's own `npm test`, only the reporter differs | one `npm install`, run in the baseline copy and then copied to the formatted copy, so both runs execute against byte-identical dependencies; private `TMPDIR` per run |
| rack | `bundle exec rake test:regular` and `bundle exec rake test:separate` — both of rack's test tasks, invoked separately | one `bundle install`, run in the baseline copy, its `Gemfile.lock` copied to the formatted copy and one `BUNDLE_PATH` vendored inside the work directory shared by both; private `TMPDIR` per run; proxy env vars stripped for both runs |
| go | `go test -json -count=1 ./...` | one `go mod download`, warmed from the baseline copy; `-count=1` disables Go's test result cache; a `GOCACHE` inside the work directory, shared by both runs, so the user's build cache is never written to; private `TMPDIR` per run; proxy env vars stripped for both runs |

The shared `node_modules` is not a convenience. express sets
`package-lock=false` in its `.npmrc` and ships no lockfile, so two independent
`npm install` runs may resolve different versions inside the declared semver
ranges — installing once and copying the tree is the node equivalent of the
one shared venv and the one shared `cargo fetch`. It also means the express
target needs npm registry access; the script checks for `node` and `npm` up
front and fails with exit 2 (infrastructure, not a result) if the install
cannot complete.

The rack target has the same problem and solves it the bundler way: rack ships
no lockfile either, so the resolution happens once in the baseline copy and
the resulting `Gemfile.lock` is copied to the formatted copy, with both
resolving through it against one `BUNDLE_PATH` vendored inside the work
directory. Nothing is installed into the user's gem home. As a side effect the
formatted `Gemfile` — TokenPress rewrites it, it being a Ruby file — has to
still satisfy the lock resolved from the unformatted one.

Three rack-specific choices worth stating:

* **Both test tasks are run, each on its own.** rack's own `rake test` chains
  `spec` → `test:regular` → `test:separate`, but `spec` only regenerates
  `SPEC.rdoc` from the `##` comments in `lib/rack/lint.rb` and defines no
  tests, and rake stops at the first failing task — so one failing test in
  `test:regular` would skip `test:separate` entirely, which is exactly what
  happens in this sandbox. Invoking the two test tasks separately reaches
  *more* of the suite than a bare `rake test` does, not less. The phase name
  is part of the test id, because `test:separate` re-runs the same ids one
  process per file.
* **A minitest reporter plugin, not `--verbose`.** minitest ships no
  machine-readable reporter, and its verbose console output prints the test id
  and the result marker separately, so a suite that runs tests in threads
  interleaves them and lines are lost (measured on another Ruby candidate:
  1,095 parsable lines for 1,114 tests). The script writes a tiny plugin into
  the work directory and puts it on the load path; minitest discovers it by
  globbing `minitest/*_plugin.rb` over `$LOAD_PATH` and requires it itself.
  It cannot be injected with `RUBYOPT=-r` instead — that runs before bundler
  has set the load path up, so `require "minitest"` would pick the
  interpreter's default-gem copy rather than the bundled one.
* **The Ruby paths are formatted explicitly**, for the same reason the
  measurement passes them explicitly: rack's two `*.js` placeholder fixtures
  are not JavaScript and would otherwise be reported as parse errors by
  another backend.

Three go-specific choices, likewise:

* **`go test -json`, not the console output.** `go test` prints
  `--- PASS: Name` lines, but a package running tests in parallel interleaves
  several tests' output between a test's start and its result line, and
  subtest results are indented under their parent. That is the fragile text
  reconstruction the ripgrep target is forced into, because stable libtest has
  no machine-readable mode. `go test` *has* one, and it emits exactly one
  terminal event per test whatever the concurrency, so the go target uses it
  and inherits none of the ripgrep normalizations.
* **Package-level verdicts are rows too**, recorded under the id
  `<package>` — an import path can never be that string, so the two cannot
  collide. This matters because a package that fails to build, or whose test
  binary panics before any test reports, produces a package verdict and *no*
  test rows at all; without the package row that would read as silence rather
  than as a difference. gin's 595 rows are 588 test and subtest outcomes plus
  7 package verdicts.
* **The reducer is a Go program, run with `go run`.** The go target already
  requires the Go toolchain, so parsing the event stream with it adds no
  prerequisite the target did not have — the same reasoning the express target
  uses for parsing its mocha report with node. It imports only the standard
  library, so it needs no module and no network.

Two things the go target gets for free that the express and rack targets have
to engineer. gin ships a `go.sum`, which pins every dependency by content
hash, so the two runs provably resolve identical dependencies without the
copied `node_modules` or the copied `Gemfile.lock`. And Go's build cache is
content-addressed — a cache hit implies byte-identical inputs — so unlike
cargo's target directory it is *shared* between the two runs on purpose,
rather than kept private, and still cannot carry one run's artifacts into the
other.

Normalizations are needed to make the comparison meaningful, and each is
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
* **express**: mocha records each test's defining file as an absolute path, so
  the run directory is rewritten to a `<tree>` placeholder. The file is kept
  in the id rather than dropped, because 8 of express's `fullTitle`s are
  shared by two tests each; rows that are still identical after that are
  compared as a multiset, like the ripgrep doc-test ids.
* **rack**: none. The ids minitest reports are `Class#test_name` and carry no
  path and no line number, so nothing has to be rewritten. In particular the
  ripgrep line-number normalization is deliberately *not* generalized to this
  target: there a line number was part of a test's *identity*, here the one
  test that mentions a line number asserts it as a *value*, and normalizing
  that away would delete the finding below instead of measuring it.
* **go**: none, for the same reason. `go test -json` reports a package import
  path and a test name, neither of which carries a run path or a line number.
  Nothing is rewritten and nothing is stripped.

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

### express v5.2.1 — IDENTICAL

Run 2026-08-02, the first behavioral verification of the `tokenpress-js`
backend.

| | Value |
|---|---|
| `.js` files | 142 |
| Rewritten | 141 |
| Refused by verification | 0 |
| Verdict | **IDENTICAL** |

| Outcome | Baseline | Formatted |
|---|---|---|
| passed | 1238 | 1238 |
| failed | 0 | 0 |
| pending | 0 | 0 |

Every one of express's 1,238 tests reached the same outcome on both copies,
and the script exited 0. Unlike the requests run, this suite is green on both
sides — it drives its own ephemeral localhost servers through supertest and
needs no outbound network — so here *identical outcomes* and *all tests pass*
happen to coincide. The claim being made is still only the first one.

The single unrewritten file is `examples/static-files/public/js/app.js`, a
one-line stub whose entire content is `// foo` — a leading statement-level
comment, which the backend keeps, so the file is already in TokenPress's
canonical form. It is not a refusal, and the formatter reported no error for
it.

Two limits on what this run proves, both specific to the pin:

* **express v5.2.1 is 142 `.js` files and nothing else** — no TypeScript, no
  JSX. The `.ts`/`.mts`/`.cts`/`.tsx`/`.jsx` paths of the backend are covered
  by the crate's own tests, not by this corpus.
* **No test can observe the comment loss.** Comments do not run, so a suite
  that is IDENTICAL says nothing about the trailing and expression-position
  comments the backend drops unconditionally. Behavioral equivalence is not
  context equivalence — the same caveat already recorded for Rust.

### rack v3.2.6 — DIVERGED on 1 test, and it is a real behavior change

Run 2026-08-02, the first behavioral verification of the `tokenpress-ruby`
backend. **Reproduced twice, byte-identically** — this is not a flake.

| | Value |
|---|---|
| Ruby files | 105 |
| Rewritten | 103 |
| Refused by verification | 1 (`lib/rack/utils.rb`) |
| Unchanged | 1 (`test/builder/bom.ru`) |
| Verdict | **DIVERGED** — 1 test id, in both phases |

| Outcome | Baseline | Formatted |
|---|---|---|
| passed | 2,348 | 2,346 |
| failed | 2 | 4 |
| skipped | 4 | 4 |

2,354 rows is rack's 1,177 tests counted once per phase (`test:regular` and
`test:separate` run the same suite, in one process and in one process per
file).

**The diverging test is `Rack::Builder::parse_file` "sets `__LINE__`
correctly", and it fails on the formatted copy in both phases.** The fixture
it loads, `test/builder/line.ru`, is three lines:

```ruby
# frozen_string_literal: true

run lambda{ |env| [200, { 'content-type' => 'text/plain' }, [__LINE__.to_s]] }
```

and the test asserts that the app's body is `'3'`. TokenPress deletes the
blank line — its whitespace-minimization policy, working as designed — so
`__LINE__` evaluates to `2` and the assertion fails.

Three things make this worth stating plainly rather than filing as a bug:

1. **It is not Ruby-specific.** Every backend moves line numbers: Rust and
   JS/TS delete newlines outright, Python collapses blank lines. Ruby is
   simply the first corpus in this file whose own suite asserts on a line
   number as a *value*. ripgrep's doc-test ids also shift, but there the line
   number is part of an id and the harness normalizes it; no normalization can
   legitimately hide an assertion.
2. **Verification is structurally blind to it, by construction.** The
   comparable artifact prism produces is location-independent — that is what
   makes it usable as an AST-equality stand-in at all — so `__LINE__` before
   and after are the same token in the same position, and format-time
   verification passes. This is the second time the upstream-suite harness has
   caught something the formatter's own check cannot see (the first was
   ripgrep's mixed doc block).
3. **The rewrite that broke it saved nothing.** `line.ru` is 35 tokens before
   and 35 tokens after at `o200k_base`: a blank line and a following newline
   cost the same one token, so deleting the blank line bought zero tokens and
   changed observable behavior. That is a strictly bad trade on this file, and
   it is the concrete argument for the caveat rather than a hypothetical one.

The two failures present on **both** copies are
`Rack::Directory` "return 404 for unreadable directories", once per phase. The
test writes a file, `chmod`s it to `0`, and expects the server to refuse it;
the run environment is root, for whom mode `0` is still readable, so it gets
`200`. That is a sandbox artifact, it fails identically on the unformatted
copy, and it is exactly why the comparison is against a baseline rather than
against "all green". The 4 skips (2 per phase) are rack's own unconditional
skips in `spec_request.rb`.

What this run does *not* show: no other rack test noticed the reformatting.
2,346 of 2,354 outcomes are `passed` on both sides, including rack's own
fixtures for `__END__`, `=begin`/`=end` embdocs, a leading byte-order mark and
the `frozen_string_literal` magic comment — the four shapes most likely to
break a whitespace rewriter, all of which survive.

### gin v1.11.0 — IDENTICAL

Run 2026-08-04, the first behavioral verification of the `tokenpress-go`
backend. **Reproduced twice, byte-identically.**

| | Value |
|---|---|
| `.go` files | 95 |
| Rewritten | 94 |
| Refused by verification | 0 |
| Unchanged | 1 (`doc.go`) |
| Verdict | **IDENTICAL** — exit 0 |

| Outcome | Baseline | Formatted |
|---|---|---|
| pass | 592 | 592 |
| fail | 0 | 0 |
| skip | 3 | 3 |

595 rows on each side: 588 test and subtest outcomes (587 pass, 1 skip —
gin's own `TestPathCleanMallocs`) plus 7 package verdicts (5 pass, 2 skip —
`codec/json` and `ginS` have no test files). Every one reached the same
outcome on both copies. Like express and unlike requests and rack, this suite
is green on both sides — it drives its own ephemeral localhost servers and
needs no outbound network once the modules are downloaded — so here
*identical outcomes* and *all tests pass* happen to coincide. The claim being
made is still only the first one.

The single unrewritten file is `doc.go`, whose entire content is one `/* */`
package comment and the line `package gin // import "…"`. The Go backend
keeps comments at default settings and there is no indentation and no blank
line outside the comment, so the file is already in TokenPress's canonical
form. It is not a refusal, and the formatter reported no error for it.

**The expected divergence class did not fire, and it is worth saying exactly
why.** Go's `testing` package prints `file:line`, and any test that
golden-compares a panic trace, `runtime.Caller` output or a `%+v` stack would
diverge legitimately once formatting moves the lines — the same shape as
rack's `__LINE__` finding. gin *does* contain the ingredient:
`recovery.go:123` calls `runtime.Caller` and writes `file:line` into its
panic log. What gin does not do is assert on it. Its recovery tests check the
log with `assert.Contains` against the panic message, the test name and the
request line; no assertion in the suite mentions a line number. So the
divergence class is live in this corpus and simply untested by it. **An
IDENTICAL verdict here is not evidence that Go output preserves line
numbers — it does not, and no backend does.**

Two further limits on what this run proves:

* **Five of the 95 files are formatted but never compiled by the run.** gin
  uses build constraints, and under the default tag set `go build` ignores
  `context_appengine.go` (`//go:build appengine`),
  `binding/binding_nomsgpack.go` (`nomsgpack`) and
  `codec/json/{go_json,jsoniter,sonic}.go`. TokenPress rewrites them like any
  other file — it does not evaluate build tags — so the test comparison says
  nothing about them. Checked separately, by hand rather than by the script:
  `go build -tags <t> ./...` for each of `appengine`, `nomsgpack`,
  `jsoniter`, `go_json` and `sonic` exits 0 on **both** copies, so all five
  still compile after formatting. The build constraints themselves survive
  intact, blank line included — a `//go:build` line only counts as a
  constraint when it is followed by a blank line and precedes the package
  clause, and the Go backend's comment hazard policy keeps that window;
  `context_appengine.go` was inspected byte by byte to confirm it.
* **No test can observe a comment.** Comments do not run, so an IDENTICAL
  verdict says nothing about comment preservation either way. For Go that
  happens to be moot at default settings — nothing is dropped — but the
  general caveat is the same one recorded for Rust and JS/TS. There is no
  cgo in this pin and no nested module, so those paths are covered by the
  crate's own tests rather than by this corpus.

### Scope and caveats

* These runs verify **default settings only**. The aggressive settings above
  (`--py-strip-comments`, `--py-strip-annotations`, `--rs-strip-doc-comments`,
  `--js-strip-comments`, `--ruby-strip-comments`, `--go-strip-comments`) are
  knowingly lossy and are not covered by this harness — stripping doc comments would delete the doc
  tests being compared.
* The Rust and JS/TS comment-loss caveats still stand: Rust `//` and `/* */`
  comments, and JS/TS trailing and expression-position comments, are dropped
  unconditionally, and no test suite can detect that, because comments do not
  run. Behavioral equivalence is not context equivalence. Ruby and Go are the
  exceptions — they drop no comments at default settings — but they share the
  line-number caveat below.
* **Line numbers are not preserved by any backend**, and the rack run above is
  the measured proof that this is observable: `__LINE__`, `caller`, backtraces
  and anything derived from them move when blank lines and indentation go.
  Only rack's suite happens to assert on one. gin's IDENTICAL verdict is
  emphatically not a counter-example: Go's `testing` prints `file:line` and
  gin's own `recovery.go` builds a stack trace from `runtime.Caller`, but no
  gin test asserts on a line number, so the class is untested there rather
  than absent.
* Only five corpora are covered (requests, ripgrep, express, rack, gin). The
  larger corpora in the table above are verified structurally, not
  behaviorally.
* The 5 requests failures and the 2 rack failures are environment artifacts,
  not upstream-green results; the claim is *identical outcomes*, not *all
  tests pass*.
* The express target additionally requires `node`, `npm` and npm registry
  access; the rack target requires `ruby`, `bundler` and rubygems.org access;
  the go target requires the Go toolchain and Go module proxy access. They are
  the only targets with a network prerequisite beyond the git clone; without
  it the run exits 2 (never ran) rather than reporting a verdict.

### Reproduce

```bash
./benchmarks/verify-upstream.sh requests   # pytest, junit per-test-id diff
./benchmarks/verify-upstream.sh ripgrep    # cargo test, multiset diff
./benchmarks/verify-upstream.sh express    # mocha, JSON reporter per-test-id diff
./benchmarks/verify-upstream.sh rack       # minitest, reporter-plugin per-test-id diff
./benchmarks/verify-upstream.sh go         # go test -json, per-test-id diff
./benchmarks/verify-upstream.sh all
# exit 0 = identical, 1 = diverged, 2 = the comparison never ran
# the rack target currently exits 1 - see the __LINE__ finding above
# the go target exits 0
```
