# TokenPress Showcase

Measured token reduction on eleven commit-pinned open-source repositories, at
the *aggressive* (lossy) settings. Every number on this page is taken from
[`RESULTS.md`](RESULTS.md), which holds the full methodology, the platform
notes, and the default-setting numbers. Nothing here is extrapolated.

Five public tokenizers: `o200k_base` (OpenAI GPT-4o / GPT-4.1 / o-series,
including Codex models) and `cl100k_base` (GPT-4 / GPT-3.5-turbo), both
embedded in the binary, plus three open-model tokenizers measured
2026-08-02 from revision-pinned files — Qwen3.6, GLM-5.2 and Kimi K3.
Candidate lists are per-tokenizer (see below); Gemma is not measured.

---

## Headline: tokio, -50.5%

The only corpus of the eleven that clears a 40% reduction on every tokenizer
measured — -50.5% on `o200k_base`, and **-55.2% on Qwen3.6** (1,542,030 →
690,601). On Qwen3.6 specifically, three more corpora clear the bar; see
"Per-tokenizer ≥40% candidates" below.

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

Measured 2026-08-01 on a Linux LF checkout of the pinned commits (express and
rack 2026-08-02, gin 2026-08-04, same way). `Files` is the number of files
successfully formatted.

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
| [rack/rack](https://github.com/rack/rack) | Ruby | `e1f22fdbe99afd2126b6fbf05bb12399359574b7` (tag v3.2.6) | 104 | -20.8% | -20.5% |
| [astral-sh/uv](https://github.com/astral-sh/uv) | Rust + Python | `be765050837d81badb20e1f70eec62146c586902` | 718 | -21.5% | -21.8% |
| [gin-gonic/gin](https://github.com/gin-gonic/gin) | Go | `6ad6205e9c94a4b8a320219e28c37c29d22a7a2c` (tag v1.11.0) | 95 | -19.4% | -19.9% |

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
| rack | 187,175 | 148,273 | 186,474 | 148,157 |
| uv | 4,806,817 | 3,773,907 | 4,779,673 | 3,739,347 |
| gin | 173,337 | 139,758 | 172,761 | 138,297 |

### Flags used

| Corpus type | Flags |
|---|---|
| Python (requests, django, fastapi, langchain, transformers) | `--py-strip-comments --py-strip-annotations --py-strip-docstrings` |
| Rust (ripgrep, tokio) | `--rs-strip-doc-comments` |
| JavaScript (express) | `--js-strip-comments` |
| Ruby (rack: 93 `.rb`, 9 `.ru`, `rack.gemspec`, `Gemfile`, `Rakefile`) | `--ruby-strip-comments` |
| Go (gin: 95 `.go`) | `--go-strip-comments` |
| Mixed (uv: 624 `.rs` + 94 `.py`) | all four Python/Rust flags — per-language flags are language-scoped, so passing a Rust flag to a Python tree is a verified no-op |

### Why rack is low, and what its default number is

rack is the only Ruby corpus, and Ruby is one of the two backends (with Go)
whose *default* settings discard nothing at all: every `#` comment and every
`=begin`/`=end` embdoc survives, so the default-settings figure — **-9.2% on `o200k_base`,
-8.9% on `cl100k_base`** — is genuinely context-lossless, unlike the Rust and
JavaScript defaults. `--ruby-strip-comments` is the entire
comments-kept-vs-dropped difference and adds +11.6pp on top — the second
largest of the three measured comment-stripping deltas, behind Go's +13.0pp,
which is the same structural case. rack is also
comment-dense rather than prose-dense, which is why the total lands near
django's. **No Ruby project has been hunted for a ≥40% result**, so this page
makes **no ≥40% claim for Ruby in either direction**.

rack was picked over sinatra, the first candidate: sinatra's suite installs
and runs, but its test count varied run to run (1,114 vs 1,120 tests), one of
its integration tests rewrites the repository's own `Gemfile.lock` mid-run,
and it randomizes test order, so runs were not comparable to each other — let
alone across two copies. rack's 1,177 tests are bit-for-bit reproducible.

### Why gin is last, and what its default number is

gin is the only Go corpus, and at **-19.4%** it is the smallest total on this
page. The reason is `gofmt`. Go source in the wild arrives already
whitespace-canonical — one tab per nesting level rather than a run of spaces
(14,008 of gin's 21,590 lines start with a tab; exactly 11 start with a
space), no trailing whitespace, and consecutive blank lines already collapsed
to one. On top of that, Go's automatic semicolon insertion makes a line break
a syntactic token, so the backend removes indentation and blank lines but
never joins two lines the way the Rust backend does. There is simply less
whitespace to take.

Its **default-settings figure is -6.4% on `o200k_base`, -6.2% on
`cl100k_base`** — the lowest on this page, and, like Ruby's and unlike Rust's
and JavaScript's, genuinely context-lossless: every comment survives, including
the ones the Go toolchain reads as directives. `--go-strip-comments` is the
entire comments-kept-vs-dropped difference and adds **+13.0pp** — the largest
of the three comment-stripping deltas measured (Ruby +11.6pp, JS/TS +8.1pp;
Rust's `--rs-strip-doc-comments` has no equivalent measurement). Go convention
puts a doc comment on every exported identifier, so the prose is where gin's
compressible bulk actually is. **No Go project has been hunted for a ≥40% result**, so this page
makes **no ≥40% claim for Go in either direction**.

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

## Per-tokenizer ≥40% candidates

Added 2026-08-02. Savings differ enough by tokenizer that candidate
selection is done per tokenizer, not on an OpenAI proxy. Measured on the
maintainer's Windows machine from an LF checkout of the same pins; the
`o200k_base` sanity runs reproduced the Linux totals exactly. The run used
the current CLI (JS/TS backend included), so `.js` files inside the Python
and mixed corpora are formatted too — file counts and the django
percentages shift slightly against the 2026-08-01 table; `RESULTS.md`
("Open-model tokenizers") has the full grid, raw counts and the coverage
notes.

| Tokenizer | Models | ≥40% candidates (aggressive flags) |
|---|---|---|
| `o200k_base` | GPT-4o / GPT-4.1 / o-series | tokio **-50.5%** |
| `cl100k_base` | GPT-4 / GPT-3.5-turbo | tokio **-50.9%** |
| Qwen3.6 | Qwen3.6 family | tokio **-55.2%**, ripgrep **-42.7%**, langchain **-41.1%**, fastapi **-40.1%** |
| GLM-5.2 | GLM-5.2 | tokio **-50.9%** |
| Kimi K3 | Kimi K3 | tokio **-50.6%** |

The Qwen3.6 list is four deep because Qwen prices whitespace runs
comparatively expensively — the same property that makes it save +7.9pp
more than `o200k_base` on express. Selecting candidates on `o200k_base`
would have missed three of Qwen's four. No list exists for Gemma-tokenizer
models: the official Gemma repos are gated, so its tokenizer is not in the
benchmark set (maintainer decision, 2026-08-02).

This run covers the nine corpora that existed when it was made. Two are
missing from it: rack (Ruby), added later the same day, and gin (Go), added
2026-08-04. **Neither has any open-model figures.** They reach -20.8% and
-19.4% on `o200k_base`, far below the bar, so neither can change any list
above — but no Qwen3.6 / GLM-5.2 / Kimi K3 number should be quoted for Ruby
or for Go until they are re-measured, and the `o200k_base` figure is not a
substitute: the table above is the evidence that it is not.

---

## Verification

* **Every file counted above passed TokenPress verification** (re-parse plus
  token/AST equivalence). The runs reported here have **1 verification
  refusal in total**, in rack: `lib/rack/utils.rb`, at default settings and
  with `--ruby-strip-comments` alike, so it is excluded from rack's 104-file
  count and from its token totals. It is a documented Ruby over-refusal class
  — a location slice that spans a multi-line index call — and it is the safe
  direction: the file is left byte-for-byte alone and no output that failed
  the check is ever written. `RESULTS.md` has the minimal reproducer. Two
  further files across the corpus are excluded because they are
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
`benchmarks/verify-upstream.sh` runs five projects' own upstream test suites
against a TokenPress-formatted copy and diffs the outcome **per test id**
against an unformatted baseline copy.

| Target | Files | Rewritten | Refused | Result |
|---|---|---|---|---|
| requests v2.32.3 (`pytest`) | 36 `.py` | 35 | 0 | **IDENTICAL** — 585 passed / 5 failed / 15 skipped / 1 xfailed on both copies |
| ripgrep 14.1.1 (`cargo test --workspace`) | 98 `.rs` | 98 | 0 | **IDENTICAL after a fix** — 1106 ok / 3 ignored, exit 0 |
| express v5.2.1 (`mocha`, express's own `npm test` arguments) | 142 `.js` | 141 | 0 | **IDENTICAL** — 1238 passed / 0 failed on both copies, exit 0 |
| rack v3.2.6 (`rake test:regular` + `rake test:separate`) | 105 Ruby | 103 | 1 | **DIVERGED** — 1 of 2,354 outcomes; 2,348→2,346 passed, 2→4 failed, exit 1 |
| gin v1.11.0 (`go test -json -count=1 ./...`) | 95 `.go` | 94 | 0 | **IDENTICAL** — 592 passed / 0 failed / 3 skipped on both copies, exit 0 |

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
* **The rack run diverged, and the divergence is real** — reproduced twice,
  byte-identically. One test, `Rack::Builder::parse_file` "sets `__LINE__`
  correctly", passes on the unformatted copy and fails on the formatted one:
  its fixture asserts that a line number is `3`, and deleting the blank line
  above the code makes it `2`. Nothing else in rack's 1,177 tests noticed the
  reformatting. See the line-number caveat below; `RESULTS.md` has the full
  triage. rack's other 2 failures are a root-user filesystem-permission
  artifact and fail identically on both copies. The rack target needs `ruby`,
  `bundler` and rubygems.org access to run at all.
* **gin's IDENTICAL verdict is not evidence that Go preserves line numbers.**
  It does not, and no backend does. Go's `testing` prints `file:line`, and
  gin's own `recovery.go` builds a stack trace from `runtime.Caller` — the
  ingredient for exactly rack's divergence is present. What is absent is any
  gin test that *asserts* on a line number; its recovery tests match the log
  against the panic message and the test name instead. The class is untested
  by this corpus, not disproved by it. Note also that 5 of gin's 95 files sit
  behind build constraints (`appengine`, `nomsgpack`, `jsoniter`, `go_json`,
  `sonic`) and are formatted but never compiled by the default-tag test run;
  checked by hand, `go build -tags <t> ./...` exits 0 on both copies for each
  of the five. The go target needs the Go toolchain and Go module proxy
  access to run at all.
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
| `--ruby-strip-comments` | Ruby `#` comments and `=begin`/`=end` embdocs — all of them, this being the only Ruby lever. The shebang and the leading magic-comment window (`# frozen_string_literal: true` and friends) survive, being semantic rather than prose |
| `--go-strip-comments` | Go `//` and `/* */` comments — all of them, this being the only Go lever. The comments the toolchain reads as directives (`//go:` lines, `/*line*/`, build constraints and the cgo preamble) survive, being semantic rather than prose |

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

**Ruby keeps every comment at default settings — but no backend keeps line
numbers.** Ruby is one of the two languages on this page (with Go) whose
default settings discard nothing: comments and embdocs all survive, so rack's default `-9.2%`
is context-lossless and `--ruby-strip-comments` is a pure opt-in. What Ruby
shares with every other backend is that removing blank lines and indentation
**moves every line below the removal**, so `__LINE__`, `caller` and backtraces
change. Token/AST equivalence cannot see this — the location-independent
comparison that stands in for AST equality is blind to it by construction —
and the rack upstream run above is the measured proof that it is observable:
one rack test asserts a `__LINE__` value and fails on the formatted copy. On
that particular file the deleted blank line saved **zero tokens** (35 before,
35 after at `o200k_base`), because a blank line and a plain newline cost the
same one token. If your code, your tests or your tooling depend on line
numbers, TokenPress output is not a drop-in replacement for the original.
Go is in exactly the same position, and its upstream run does *not* clear it:
gin's suite is IDENTICAL only because nothing in it asserts on a line number,
not because Go output keeps them.

**Go keeps every comment at default settings, and the number is low because
`gofmt` got there first.** Go is the second language on this page whose
default settings discard nothing. Its -6.4% default is the smallest on the
page, and that is a fact about Go's culture rather than about the backend:
`gofmt` has already reduced the file to one tab per nesting level, no trailing
whitespace and no runs of blank lines before TokenPress ever sees it, and Go's
automatic semicolon insertion means the line breaks can never be joined away
the way Rust's are. What remains compressible is the prose, which is why
`--go-strip-comments` moves gin further (+13.0pp) than
`--ruby-strip-comments` moves rack (+11.6pp) or `--js-strip-comments` moves
express (+8.1pp).

**Ruby has known over-refusal classes, and one of them fires on rack.** Two
shapes make the Ruby verifier reject a rewrite that was in fact harmless — a
`<<~` squiggly heredoc whose body gets re-indented, and a location slice
spanning a multi-line index call. rack hits the second one in
`lib/rack/utils.rb`, which is therefore left untouched and excluded from
rack's counts above (**1 refusal**, at both settings). Refusals are the safe
direction and cost only savings, never correctness, but they mean a Ruby tree
may come back with a few files unchanged.

**Rust macro-body whitespace is minimized.** The tokens inside a macro
invocation are preserved exactly; the whitespace between them is not. For
whitespace-sensitive macros — `stringify!` is the common case — this changes
the string produced at runtime, and a re-spaced macro body is token-identical
to the original, so verification does not detect it.

**Public tokenizers only.** All numbers here are the two embedded OpenAI
tokenizers plus the three revision-pinned open-model tokenizers (Qwen3.6,
GLM-5.2, Kimi K3). Claude's vocabulary is private and has not been
measured; no number on this page is given or extrapolated for any private
or closed tokenizer, and none should be.

**Which projects clear 40% depends on the tokenizer.** This is now measured,
not hypothesized: the per-tokenizer lists above show three of Qwen3.6's four
candidates are invisible on `o200k_base`. Judge savings, and ≥40%
membership, on the tokenizer of the model you actually use. Each list is
valid only for models using that tokenizer; Gemma-tokenizer models have no
list, because Gemma is not in the benchmark tokenizer set (gated repos).

**Line endings shift the baseline slightly.** These runs are a Linux LF
checkout; the earlier tables in `RESULTS.md` were measured on Windows with
CRLF, which raises the before-counts (requests: 86,331 LF vs 86,922 CRLF at
`o200k`). This is why ripgrep reads -37.4% here and -38.2% in the historical
aggressive table — the flag set is unchanged. Converting the LF checkout back
to CRLF reproduces the historical numbers exactly.

**Eleven corpora is not a population.** These are the repositories measured,
not a sample chosen to be representative. Savings depend heavily on how much
of a tree is prose documentation. JavaScript, Ruby and Go are each represented
by exactly one project, so `-25.4%`, `-20.8%` and `-19.4%` are data points,
not language-level expectations.

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

# Ruby corpus - the paths are listed explicitly, because rack ships two
# `*.js` placeholder fixtures that are not JavaScript
find benchmarks/corpus/rack -not -path '*/.git/*' \
    \( -name '*.rb' -o -name '*.rake' -o -name '*.gemspec' -o -name '*.ru' \
    -o -name 'Gemfile' -o -name 'Rakefile' \) -print0 |
    xargs -0 target/release/tokenpress stats --tokenizer o200k_base \
        --ruby-strip-comments

# Go corpus - the pin holds no file of any other supported language, so the
# tree can be passed directly
target/release/tokenpress stats benchmarks/corpus/gin --tokenizer o200k_base \
    --go-strip-comments

# Mixed tree
target/release/tokenpress stats benchmarks/corpus/uv --tokenizer o200k_base \
    --py-strip-comments --py-strip-annotations --py-strip-docstrings \
    --rs-strip-doc-comments

# repeat any of the above with --tokenizer cl100k_base for the second column

# upstream behavioral check (default settings)
# express additionally needs node, npm and npm registry access; rack needs
# ruby, bundler and rubygems.org access; go needs the Go toolchain and Go
# module proxy access
./benchmarks/verify-upstream.sh all   # 0 = identical, 1 = diverged, 2 = never ran
# this currently exits 1: the rack target diverges on the __LINE__ test above.
# the go target on its own exits 0.
```

`fetch.sh` exits non-zero at the tokenizer-download step when huggingface.co
is unreachable; the corpus clones above it have already completed, and the two
tokenizers used on this page are embedded in the binary and need no download.
