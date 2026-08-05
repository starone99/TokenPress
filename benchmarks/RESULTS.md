# TokenPress Benchmark Results

Measured: 2026-07-31 (open-model tokenizers added 2026-08-01; the
JavaScript/TypeScript corpus added 2026-08-02; the Ruby corpus added
2026-08-02; per-tokenizer aggressive run and the express open-model rows
added 2026-08-02; the Go corpus added 2026-08-04; the gin and rack open-model
rows, default and aggressive, added 2026-08-04 — with that, every corpus in
this file had all five tokenizers; the Java corpus added 2026-08-04 and the C#
corpus 2026-08-05, both on the two embedded tokenizers only, so they are the
two corpora here still missing their three open-model columns)
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
| apache/commons-lang | 3.17.0 | `29ccc766` | 500 `.java` (whole tree, all under `src/`) |
| JoshClose/CsvHelper | 33.1.0 | `5dad8b8b` | 461 `.cs` (whole tree; its 4 `.js` are generated-site assets and are excluded) |

**Every parseable file passed verification, with one measured exception**
(re-parse + token/AST equivalence). The only skipped files across ~13,800 are
two intentionally broken test fixtures — Django's `tests_syntax_error.py`
(invalid Python by design) and LangChain's `non-utf8-encoding.py` (invalid
UTF-8 by design), both correctly rejected with per-file errors — plus one
genuine refusal in rack, `lib/rack/utils.rb`, which is a known Ruby
over-refusal class documented in the Ruby section below. The 95 Go files
added 2026-08-04 add **no** refusals and no rejected inputs, at either
setting and either tokenizer, and neither do the 500 Java files added the
same day. **The 461 C# files added 2026-08-05 do**: 2, at every setting, both
over-refusals of valid input, triaged in the C# section below. (A third one
appeared under `--csharp-strip-comments` when this corpus was first measured;
it was a shared tree-sitter-backend defect, fixed 2026-08-05, and the
`--csharp-strip-comments` rows below are re-measured after the fix.) A refusal
writes
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
| Gemma 4 | google/gemma-4-31B (rev `5bbc2fb1`) | `--tokenizer hf:` + tokenizer.json | measured locally; added 2026-08-06 |

Claude is not yet measured — its vocabulary is private, so numbers require
the `count_tokens` API. **Never extrapolate private-tokenizer savings from
the numbers below** (project rule). Tokenizer files are downloaded
revision-pinned by `benchmarks/fetch.ps1`.

**Gemma needed no authentication, contrary to what this file said until
2026-08-06.** Gemma was excluded from the 2026-08-02 tokenizer hunt as gated,
and that was correct for the generations that existed then — `google/gemma-2-*`
and `google/gemma-3-*` still report `gated: manual` (license acceptance plus an
auth token). **Gemma 4 does not**: `google/gemma-4-31B` reports `gated: false`
and its `tokenizer.json` downloads over plain HTTP with no `HF_TOKEN`, which is
how `fetch.sh` now takes it. No community mirror is involved, so the provenance
objection that rejected one earlier does not apply. Only the 31B base repo is
pinned; the other Gemma 4 base repos (12B, 26B-A4B, E4B) serve a byte-identical
`tokenizer.json` — same 32,170,070 bytes, same LFS oid — so the column is a
Gemma 4 figure and not a 31B-only one. `google/gemma-4-31B-it` differs and is
not used.

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

**All five tokenizers are measured.** The two embedded ones were measured with
the corpus on 2026-08-02, when the environment could not reach huggingface.co;
the three open-model columns were filled in on 2026-08-04 on the maintainer's
machine, which can.

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
| rack | Qwen3.6 | 104 | 199,847 | 183,370 | **-8.2%** |
| rack | GLM-5.2 | 104 | 186,913 | 170,346 | **-8.9%** |
| rack | Kimi K3 | 104 | 186,528 | 169,974 | **-8.9%** |

The three open-model rows were measured on 2026-08-04 on the maintainer's
machine, a day after the embedded pair, so **both embedded rows were re-run
first as a control and reproduced exactly** — 187,175 → 170,012 and
186,474 → 169,907, matching to the token. The corpus was checked out with
`core.autocrlf false` to get the same LF tree, so all five rows describe one
identical set of files.

The five agree within **1.0pp** (-8.2% to -9.2%), and Qwen3.6 is the outlier
in both directions at once: it counts rack **6.8% larger** before compression
than `o200k_base` (199,847 vs 187,175) and reports the smallest saving. That
is the same direction it takes on gin and the *opposite* of the one it takes
on requests, where it reports the largest — which is exactly why the project
rule forbids reading one tokenizer's percentage off another's in either
direction.

`Files` is 104 of the 105, because of **1 verification refusal**:
`lib/rack/utils.rb`. It refuses at both settings and all five tokenizers, and
it is a known over-refusal class, not a new defect — see the subsection below.
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

#### One Windows obstacle worth recording, because it silently changes the file count

`test/spec_multipart.rb` is **quarantined by Windows Defender on sight**,
which is how this corpus first got measured at 103 files instead of 104. The
detection is `Backdoor:PHP/Remoteshell.B`, and it is a signature match on real
content rather than a vague heuristic: rack embeds a PHP web-shell payload as
multipart *upload test data*. It is inert — a string in a Ruby test file that
is never executed as PHP — and it is the pinned upstream content of tag
`v3.2.6`, but it is a genuine match, not a false positive on innocuous text.

The failure mode is what makes it worth writing down: the file does not error,
it **ceases to exist**, so a run silently measures a smaller corpus and still
prints a plausible percentage. The 103-file totals differ from the 104-file
ones by more than rounding at aggressive settings (-21.6% against -20.8%),
because the removed file saves only -10.5% there against the tree's -21.6%.

Check the file count before trusting any rack number on Windows. To suppress
the quarantine, an administrator adds an exclusion for the corpus directory
only:

```powershell
Add-MpPreference -ExclusionPath "<repo>\benchmarks\corpus\rack"
git -C "<repo>\benchmarks\corpus\rack" checkout -f FETCH_HEAD
```

and removes it again afterwards. Excluding `benchmarks/corpus` as a whole
would leave every future corpus download unscanned; do not widen it.

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

**All five tokenizers are measured.** The two embedded ones were measured with
the rest of the corpus on 2026-08-04; the three open-model columns were filled
in on 2026-08-04 on the maintainer's machine, which can reach huggingface.co —
the measurement container cannot, which is why they were `*pending*` for one day.
rack's three were filled in the same day; no corpus in this file is missing a
tokenizer any more.

Because the open-model rows were measured separately, the embedded rows were
**re-run first as a control and reproduced exactly** — 95 files, 173,337 →
162,297 at `o200k_base` and 173,337 → 139,758 with `--go-strip-comments`,
matching the recorded figures to the token. The corpus was re-checked out with
`core.autocrlf false` to get the same LF tree the original run measured, so
the open-model rows sit on the same side of the line-ending boundary as the
embedded ones and the five are directly comparable.

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
| gin | Qwen3.6 | 95 | 191,866 | 181,174 | **-5.6%** |
| gin | GLM-5.2 | 95 | 172,972 | 162,213 | **-6.2%** |
| gin | Kimi K3 | 95 | 174,919 | 163,740 | **-6.4%** |

**0 verification refusals** across all 95 files, at all five tokenizers and at
both settings. Absolute saving at o200k: 11,040 tokens per full-repo prompt.

The five agree closely here — **-5.6% to -6.4%**, a 0.8pp spread. Qwen3.6 is
the outlier in both columns: it counts gin **10.7% larger** before compression
than `o200k_base` does (191,866 vs 173,337), the same vocabulary difference it
shows on the Python corpora (requests: 94,786 vs 86,922, 9.0% larger), and it
reports the **smallest** saving of the five. That second half is specific to
Go and does not generalize — on requests, Qwen3.6 reports the *largest* saving
(-9.7% against o200k's -9.0%). Which direction it falls is a property of the
corpus, which is exactly why the project rule forbids quoting one tokenizer's
percentage for another model family in either direction.

What does hold across all five: gin is the lowest-saving corpus in this file
at **every** tokenizer measured, not just at `o200k_base`. (Superseded in
part on 2026-08-04 by the Java corpus below, and the picture completed when
commons-lang's open-model columns were measured on 2026-08-05: commons-lang is
lower at four of the five tokenizers, and gin keeps the title at **Qwen3.6
only**, where the two are a hair apart — -5.57% against -5.65%, both of which
this file prints as -5.6%. Read the raw counts in the Java table below rather
than the rounded column if that margin matters to you.)

Like rack's `-9.2%` and unlike express's `-17.3%`, this `-6.4%` **is**
context-lossless: every comment survives and only whitespace was removed.

**-6.4% was the lowest default-settings figure of any corpus in this file
until commons-lang's -6.1% arrived on 2026-08-04, and the reason is `gofmt`.**
Go source in the wild is already whitespace-canonical
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
weakness in the backend. The comment lever, when it is pulled, adds +13.0pp
(see `--go-strip-comments` below) — the largest of the comment-stripping
deltas in this file until Java's `--java-strip-comments` was measured at
+39.4pp on 2026-08-04 and C#'s at +24.0pp the next day, and still ahead of
Ruby's +11.6pp and JS/TS's +8.1pp.

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

### Java — apache/commons-lang (default settings, added 2026-08-04)

Measured 2026-08-04, Linux LF checkout, rustc 1.95.0, Maven 3.9.11,
JDK 21.0.10, on the commit pinned in `fetch.sh`/`fetch.ps1`
(`29ccc7665f3bc5d84155a3092ab2209a053324e6`, annotated tag
`rel/commons-lang-3.17.0`). This is the first corpus for the
`tokenpress-java` backend, and it is reported on its own for the same reason
express, rack and gin are: the tables further up were measured on Windows with
a CRLF checkout, and the line-ending caveat below applies to any comparison
across the two.

**All five tokenizers are measured.** The two embedded ones were measured with
the backend on 2026-08-04; the three open-model columns were filled in on
2026-08-05 on the maintainer's machine, which can reach huggingface.co — the
measurement container cannot, which is why they were `*pending*` for a day, the same
way gin's and rack's were. No corpus in this file is missing a tokenizer.

Because the open-model rows were measured separately, the embedded rows were
**re-run first as a control and reproduced exactly** — 500 files,
1,736,204 → 1,629,486 at `o200k_base` and 1,736,204 → 945,989 with
`--java-strip-comments`, matching the recorded figures to the token, and
cl100k likewise. The corpus was re-checked out with `core.autocrlf false`
**and `core.eol lf`** to get the same LF tree the original run measured, so
the open-model rows sit on the same side of the line-ending boundary as the
embedded ones and the five are directly comparable. Both settings are needed
here and `core.autocrlf false` alone is not enough, unlike on gin: commons-lang
ships a `.gitattributes` that marks `*.java` as `text`, and a `text` path is
checked out in the platform-native ending whenever `core.eol` is left at
`native`, whatever `core.autocrlf` says. Verified with `git ls-files --eol`
(575 paths `i/lf w/lf`) rather than assumed.

The whole tree is measured, all 500 files being `.java` and all of them under
`src/`. This pin holds no file of any other language TokenPress claims — no
`.py`, `.rs`, `.js`/`.ts`, Ruby path or `.go` anywhere — so like gin and
unlike rack it can be handed to the formatter as a directory with nothing
misroutable to another backend. The count is taken with `find -type f` and
excludes `target/`, so it means the same thing whether or not Maven has
already built in the tree; a fresh clone has no `target/` at all.

| Corpus | Tokenizer | Files | Before | After | Saved |
|---|---|---|---|---|---|
| commons-lang | o200k_base | 500 | 1,736,204 | 1,629,486 | **-6.1%** |
| commons-lang | cl100k_base | 500 | 1,675,227 | 1,571,424 | **-6.2%** |
| commons-lang | Qwen3.6 | 500 | 1,835,291 | 1,731,602 | **-5.6%** |
| commons-lang | GLM-5.2 | 500 | 1,677,273 | 1,573,470 | **-6.2%** |
| commons-lang | Kimi K3 | 500 | 1,737,751 | 1,632,931 | **-6.0%** |

**0 verification refusals** across all 500 files, at all five tokenizers and at
both settings; `tokenpress format` over the tree exits 0 and rewrites
500 of 500 files. Absolute saving at o200k: 106,718 tokens per full-repo
prompt.

The five agree more closely on this corpus than on any other in this file —
**-5.6% to -6.2%, a 0.6pp spread**, narrower than gin's 0.8pp. Qwen3.6 is the
outlier in the same direction it is everywhere: it counts commons-lang **5.7%
larger** before compression than `o200k_base` does (1,835,291 vs 1,736,204)
and reports the smallest saving of the five. GLM-5.2 tracks `cl100k_base` to
within 0.03pp here (1,677,273 vs 1,675,227 before), which is its habit across
this file and not a Java-specific result.

#### What Java loses at default settings: nothing

This was checked rather than assumed, because it does not follow from the
other backends. For every one of the 500 files, the original and the formatted
output are **identical once all whitespace characters are deleted from both**.
That is a stronger statement than "comments are kept": nothing was dropped,
added or reordered — Javadoc blocks and ordinary `//` and `/* */` comments all
survive, in place, with their text intact. (It does not by itself speak for
whitespace *inside* string literals and text blocks, which the test deletes on
both sides; that is covered instead by the backend contract that string
content is never touched and by the token/AST equivalence check that gates
every write.) The CLI accordingly prints **no caveat warning** for a Java-only run — there is
deliberately no `JAVA_CAVEAT_WARNING`, for the same reason there is no Ruby or
Go one, and `crates/tokenpress-cli/src/cli.rs` has a test asserting its
absence. So this `-6.1%`, like rack's `-9.2%` and gin's `-6.4%` and unlike
express's `-17.3%` and any Rust figure, is genuinely context-lossless.

What the whitespace-only rewrite actually removes on this corpus is
indentation and blank lines. Interior lines of a block comment keep their
original leading spaces, because they are part of the comment token rather
than whitespace between tokens — visible in any diff as Javadoc bodies that
stay indented under a `/**` that has moved to column 0.

**-6.1% is now the lowest default-settings figure in this file**, taking that
place from gin's -6.4% — the two are a rounding step apart and low for related
reasons. commons-lang is Apache-style Java under a checkstyle configuration,
so like `gofmt`-formatted Go it arrives without trailing whitespace and
without runs of blank lines. It differs from Go in that its indentation *is*
runs of spaces (4 per level, deeply nested), which is the more expensive kind
to carry and should push the figure the other way. What outweighs that is how
much of the file is comment: commons-lang is documentation-dense even by Java
standards, and comments are exactly what the default setting does not touch,
so a large share of the tree is untouchable at this setting.

```bash
target/release/tokenpress stats benchmarks/corpus/commons-lang \
    --tokenizer o200k_base
```

**`--java-strip-comments` is the largest single lever measured anywhere in
this file: +39.4pp**, taking commons-lang from -6.1% to **-45.5%** at o200k
(C#'s `--csharp-strip-comments` adds +24.0pp, Go's +13.0pp, Ruby's +11.6pp,
JS/TS's +8.1pp).
Javadoc is a block comment as far as the grammar is concerned, so the flag
takes the entire API documentation of a library whose documentation is most of
its text. The aggressive rows are in the aggressive section below.

#### Upstream verification of this corpus

`benchmarks/verify-upstream.sh java` runs commons-lang's own Maven surefire
suite over an unformatted and a formatted copy and diffs the outcomes per test
id; the full write-up is under "Behavioral verification against upstream test
suites" below. The pristine baseline is `BUILD SUCCESS`, exit 0,
**11,720 tests, 0 failures, 0 errors** — and either 13 or 19 skipped,
depending on the run.

**The skip count is not stable and no target may assert it.** Six of
commons-lang's tests are `assumeTrue`-gated on a time-zone parse that
succeeds or fails depending on JVM state the suite itself perturbs; six
pristine runs here produced 19, 13, 19, 13, 19 and 13, including two
unformatted copies run back to back as a control. An earlier ROADMAP note
recorded 13 for this same pin. Nothing is wrong with any of those figures. It
is the reason the harness compares a baseline run against a formatted run made
in the same environment and the same invocation rather than against a recorded
constant — and, for that one test class, folds `pass` and `skip` together so
the coin flip cannot masquerade as a formatter finding. The full account is
under "Behavioral verification against upstream test suites".

Timing note, and the reason this target stays on `--verify ast`: Java's
`--verify external` is one `javac` probe plus **three** ~0.4 s `javac` spawns
per file, so over 500 files it would measure JVM startup rather than
TokenPress. The external gate is exercised by the crate's own tests instead.

### C# — JoshClose/CsvHelper (default settings, added 2026-08-05)

Measured 2026-08-05, Linux LF checkout, rustc 1.95.0, .NET SDK 8.0.129, on the
commit pinned in `fetch.sh`/`fetch.ps1`
(`5dad8b8b1d8b074f8353cfd482e939db788a8927`, tag `33.1.0`). This is the first
corpus for the `tokenpress-csharp` backend, and it is reported on its own for
the same reason express, rack, gin and commons-lang are: the tables further up
were measured on Windows with a CRLF checkout, and the line-ending caveat
below applies to any comparison across the two.

**Only the two embedded tokenizers are measured.** The Qwen3.6, GLM-5.2 and
Kimi K3 rows are `*pending*`: this measurement container cannot reach
huggingface.co, so the revision-pinned tokenizer files `fetch.sh` downloads
are unavailable here, and the project rule is that an unmeasured column is
written as pending rather than estimated from `o200k_base`. gin, rack and
commons-lang all sat pending for a day for the same reason and were filled in
on the maintainer's machine; this corpus is waiting on the same run.

**Why CsvHelper, and what was rejected.** Nothing about a C# corpus was
pre-validated, and the constraint that decided it was the toolchain rather
than the language: CI pins .NET SDK 8.0.129, and a `global.json` that asks for
a newer SDK is a hard failure, not a warning. Seven candidates were cloned and
checked before this one was pinned.

| Candidate | License | Verdict |
|---|---|---|
| JamesNK/Newtonsoft.Json | MIT | rejected — `Src/global.json` asks for SDK `8.0.300` with `rollForward: latestFeature`, which 8.0.129 does not satisfy (a *lower* feature band, not a lower patch). Every commit whose test project can target `net8.0` carries that pin or the later `9.0.300` one; the last commit with an older pin, tag `13.0.3`, asks for `6.0.400` and tests only up to `net6.0`. |
| serilog/serilog | Apache-2.0 | rejected — `global.json` pins SDK `10.0.100`. |
| App-vNext/Polly | BSD-3-Clause | rejected — `global.json` pins SDK `10.0.302`. |
| FluentValidation | Apache-2.0 | rejected — `global.json` pins SDK `10.0.0`. |
| Humanizr/Humanizer | MIT | rejected — `global.json` pins an SDK 11 preview; the newest tags pin 10. |
| nodatime/nodatime | Apache-2.0 | rejected — `global.json` pins SDK `10.0.101`; the last tag without a 10.x pin tests on `netcoreapp3.1`/`net6.0`, runtimes SDK 8 does not ship. |
| jbogard/MediatR | Apache-2.0 | rejected — no `global.json`, but every test project targets `net10.0` only. |
| **JoshClose/CsvHelper** | **MS-PL / Apache-2.0** | **pinned** — no `global.json`, the test project lists `net8.0`, and its suite is 1,063 tests in ~2 s. |

This is J6(a)'s gson lesson applied in advance: a corpus is only usable if its
own build accepts the available toolchain, and for .NET that question is
settled by one file. It is worth stating what the survivor costs. CsvHelper's
projects list `net9.0` first and `net48`/`net47`/`net462` last, so every
`dotnet` invocation here passes `-p:TargetFrameworks=net8.0` — without it the
*restore* fails for all frameworks, not just the unsupported one
(`NETSDK1045`), and the `net4x` ones could not run on Linux in any case. That
is a build-invocation choice, not a source filter: all 461 `.cs` files are
measured either way.

The whole tree's `.cs` files are measured, 461 of them, spread over five
projects (191 library, 244 tests, 22 docs generator, 2 benchmarks, 2 website).
Unlike gin and commons-lang, and like rack, this pin does hold files of
another language TokenPress claims — four `.js` files under `docs/scripts/`
and `src/CsvHelper.Website/input/scripts/`, assets of a generated
documentation site that no project compiles. The `.cs` paths are therefore
passed explicitly rather than handing the tree to the formatter, so no other
backend's output lands inside a C# measurement.

| Corpus | Tokenizer | Files | Before | After | Saved |
|---|---|---|---|---|---|
| csvhelper | o200k_base | 459 | 376,214 | 322,419 | **-14.3%** |
| csvhelper | cl100k_base | 459 | 369,519 | 319,090 | **-13.6%** |
| csvhelper | Qwen3.6 | 459 | *pending* | *pending* | *pending* |
| csvhelper | GLM-5.2 | 459 | *pending* | *pending* | *pending* |
| csvhelper | Kimi K3 | 459 | *pending* | *pending* | *pending* |

`Files` is 459 of the 461, because of **2 verification refusals**, identical
on both embedded tokenizers and at every setting — including
`--csharp-strip-comments`, which used to add a third and no longer does. Both
are triaged below. `tokenpress format`
over the explicit `.cs` list rewrites 459 of 459 formattable files and leaves
the two refused ones untouched. Absolute saving at o200k: 53,795 tokens per
full-repo prompt.

#### What C# loses at default settings: nothing

Checked rather than assumed, the same way the Java section checks it. For
every one of the 461 files — the 459 rewritten ones and the 2 refused ones —
the original and the formatted output are **identical once all whitespace
characters are deleted from both**. Nothing was dropped, added or reordered:
`///` documentation comments and ordinary `//` and `/* */` comments all
survive, in place, with their text intact, and so do the preprocessor
directives, which are not comments at all. (As with Java, that test deletes
whitespace *inside* string literals on both sides, so it does not by itself
speak for them; those are covered by the backend contract that string content
is never touched and by the token/AST equivalence check that gates every
write.) The CLI accordingly prints **no caveat warning** for a C#-only run,
for the same reason there is no Ruby, Go or Java one. So this `-14.3%`, like
rack's `-9.2%` and gin's `-6.4%` and commons-lang's `-6.1%`, and unlike
express's `-17.3%` and any Rust figure, is genuinely context-lossless.

#### -14.3% is the highest context-lossless figure here, and braces are why

The four backends that discard nothing at default settings had, before this
corpus, produced -9.2% (rack), -6.4% (gin) and -6.1% (commons-lang). C# more
than doubles the best of them, and it does so from an *unfavourable* starting
point: CsvHelper is tab-indented, 38,532 of its 51,745 `.cs` lines beginning
with a tab against 240 beginning with a space, so the cheap-indentation
advantage that holds gin's number down applies here too.

What outweighs it is brace style. C# convention puts `{` and `}` on lines of
their own, and **11,307 of the corpus's 51,745 lines (21.8%) contain a single
brace and nothing else**; a further 6,971 (13.5%) are blank. Roughly a third
of the file is therefore lines the emitter can join away without touching a
comment or a literal. Java shares the language family but not the layout —
commons-lang is checkstyle-formatted K&R, where the opening brace rides the
statement line it belongs to — which is most of the gap between -6.1% and
-14.3%.

```bash
find benchmarks/corpus/csvhelper -not -path '*/.git/*' -name '*.cs' -print0 |
    xargs -0 target/release/tokenpress stats --tokenizer o200k_base
```

`--csharp-strip-comments` adds a further **+24.0pp**, taking the corpus to
-38.3% at o200k — the second-largest comment-stripping lever measured in this
file, behind commons-lang's +39.4pp. The aggressive rows are in the aggressive
section below.

#### The C# refusals, identified — and none is the expected defect

The C6 scoping predicted two sources of refusal on a real corpus: a slice
pattern with a designation, and `#if` blocks whose arms are not each a
complete syntactic unit — the latter measured at 65 of 945 files on the
Newtonsoft.Json tree and called "the real-world ceiling on C# coverage".
**Neither fires here.** CsvHelper has 11 `#if` directives across 11 files, 1
`#region` and 10 `#pragma`s, and every one of them parses clean. The refusals
this corpus does produce were two different classes, both previously
unrecorded, and both over-refusals of valid input rather than corruptions —
nothing was written in either case. **Class 2 has since been fixed**, so the
count as measured today is 2, at every setting; it is kept below because the
diagnosis is what the fix was built on.

**Class 1, 2 files, default settings and every other setting: an escaped
brace immediately followed by an interpolation hole that contains a string
literal.** `docs-src/CsvHelper.DocsGenerator/Extensions.cs` and
`.../Formatters/XmlDocFormatter.cs` are reported as
`parse error: syntax error`, and the byte offsets in both point at the same
shape:

```csharp
return $"{{{string.Join(",", parts)}}}";
```

Delta-debugged, the trigger needs all three parts. `$"{{{g}}}"` parses, and so
does `$"{{{g.Trim()}}}"`. `$"{g.Trim(",")}"` parses. `$"{{ {g.Trim(",")} }}"`
parses — one space between the escaped brace and the hole is enough. Only `{{`
written directly against a hole whose body contains a `"` fails. That the
input is valid C# was confirmed by compiling and running it: Roslyn accepts
the expression and it prints `{a,b}`. This is a grammar limit in
`tree-sitter-c-sharp`'s scanner, reached at parse time, so no setting avoids
it.

**Class 2 — FIXED 2026-08-05 — was 1 file, `--csharp-strip-comments` only: a
file whose entire content is comments.**
`tests/CsvHelper.Tests/Mappings/ConstructorParameter/HeaderPrefixMapTests.cs`
is six lines, all of them `//`, explaining why the test class does not exist.
Stripping the comments left nothing, and the empty result was rejected with
`verification failed: output AST differs from input`. Minimal reproducer:
a file containing only `// only a comment`.

**Class 2 was not C#-specific, and that is worth recording where someone will
find it.** The same one-line file refused identically as `.java` under
`--java-strip-comments` and as `.go` under `--go-strip-comments`. commons-lang
and gin simply contain no comment-only file, so the class went unseen until a
corpus had one. It was a pre-existing over-refusal shared by three backends,
not something the C# work introduced, and it was left as a finding here rather
than fixed inside a benchmarks task.

The cause was **one** bug in the shared engine, not three: in
`tokenpress-treesitter`'s comparable artifact, the leaf-vs-branch decision
read the *pre-skip* child count, so a comment-only root took the branch path
and rendered `(source_file)` while the empty root took the leaf path and
rendered `(source_file )` — a separator with nothing after it. The leaf path
now emits nothing after the kind for a zero-width, non-missing node. The
re-measured `--csharp-strip-comments` rows below cover 459 files instead of
458, and `HeaderPrefixMapTests.cs` now reports `108 → 1 tokens (-99.1%)`: the
file carries a UTF-8 BOM, which is not a comment and correctly survives.

**Ruby and Python do not behave the same way on this shape, and the earlier
write-up wrongly lumped them together.** Re-checked on `# only a comment`:
`--py-strip-comments` reports `-100.0%` and writes the file empty, which is
the same verdict the three tree-sitter backends now give; `--ruby-strip-comments`
reports `5 → 5 tokens (-0.0%)` and leaves the file **byte-identical on disk**.
Ruby therefore never refused this input, but it never emptied it either — a
separate finding about the Ruby comment policy, recorded here and deliberately
not changed by the class-2 fix.

#### Upstream verification of this corpus

`benchmarks/verify-upstream.sh csharp` runs CsvHelper's own xunit suite over an
unformatted and a formatted copy and diffs the outcomes per test id; the full
write-up is under "Behavioral verification against upstream test suites"
below. The verdict is **IDENTICAL**, exit 0, reproduced twice.

Timing note, and the reason this target stays on `--verify ast`: C#'s
`--verify external` is one `csc` probe plus **three** ~0.358 s `dotnet csc.dll`
spawns per file, so over 461 files it would measure toolchain startup rather
than TokenPress. The external gate is exercised by the crate's own tests
instead.

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
run predates both the rack and gin corpora, which were measured separately on
2026-08-04 and appear in their own sections above.

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
* Java corpus (commons-lang): `--java-strip-comments`. Structurally the same
  case again — the Java default keeps every comment, so the flag is the whole
  comments-kept-vs-dropped difference and the only lossy lever the Java
  backend has. Nothing survives it: Javadoc is an ordinary block comment to
  the grammar, so `--java-strip-comments` takes the library's entire API
  documentation with the rest. That is why it is the largest lever in this
  file (+39.4pp).
* C# corpus (csvhelper): `--csharp-strip-comments`. The same case once more —
  the C# default keeps every comment, so the flag is the whole
  comments-kept-vs-dropped difference and the only lossy lever the C# backend
  has, and `///` documentation is an ordinary comment to the grammar and goes
  with the rest. Preprocessor directives are *not* comments and survive it, so
  stripping cannot change which arm of an `#if` compiles.

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
| rack | Qwen3.6 | 104 | 199,847 | 160,297 | **-19.8%** |
| rack | GLM-5.2 | 104 | 186,913 | 148,548 | **-20.5%** |
| rack | Kimi K3 | 104 | 186,528 | 148,307 | **-20.5%** |
| gin | o200k_base | 95 | 173,337 | 139,758 | **-19.4%** |
| gin | cl100k_base | 95 | 172,761 | 138,297 | **-19.9%** |
| gin | Qwen3.6 | 95 | 191,866 | 155,999 | **-18.7%** |
| gin | GLM-5.2 | 95 | 172,972 | 138,461 | **-20.0%** |
| gin | Kimi K3 | 95 | 174,919 | 139,936 | **-20.0%** |
| commons-lang | o200k_base | 500 | 1,736,204 | 945,989 | **-45.5%** |
| commons-lang | cl100k_base | 500 | 1,675,227 | 894,692 | **-46.6%** |
| commons-lang | Qwen3.6 | 500 | 1,835,291 | 1,004,068 | **-45.3%** |
| commons-lang | GLM-5.2 | 500 | 1,677,273 | 896,297 | **-46.6%** |
| commons-lang | Kimi K3 | 500 | 1,737,751 | 947,047 | **-45.5%** |
| csvhelper | o200k_base | 459 | 376,214 | 231,961 | **-38.3%** |
| csvhelper | cl100k_base | 459 | 369,519 | 223,727 | **-39.5%** |
| csvhelper | Qwen3.6 | 459 | *pending* | *pending* | *pending* |
| csvhelper | GLM-5.2 | 459 | *pending* | *pending* | *pending* |
| csvhelper | Kimi K3 | 459 | *pending* | *pending* | *pending* |

`Files` counts files that were successfully formatted; refused files (next
subsection) are excluded from both the file count and the token totals. The
langchain and transformers rows were re-measured on 2026-08-01 after the
PYO3 refusals below were fixed, so they now cover the previously-excluded 19
files (langchain 2,512 → 2,530, transformers 4,699 → 4,700); every other row
is the original measurement, with one later exception: the csvhelper rows were
re-measured on 2026-08-05 after the comment-only over-refusal (class 2 in the
C# section above) was fixed, so they now cover 459 files instead of 458
(376,106 → 376,214 before, 231,960 → 231,961 after at o200k). Only cl100k's
percentage moved, -39.4% → -39.5%. Only langchain's percentages moved among
the 2026-08-01 re-measurements (-38.5% → -38.8% at o200k, -39.0% → -39.2% at
cl100k); transformers' are unchanged to one decimal, the recovered file being
one of 4,700.

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
| rack | Qwen3.6 | -8.2% | **-19.8%** | +11.5pp |
| rack | GLM-5.2 | -8.9% | **-20.5%** | +11.7pp |
| rack | Kimi K3 | -8.9% | **-20.5%** | +11.6pp |

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
| gin | Qwen3.6 | -5.6% | **-18.7%** | +13.1pp |
| gin | GLM-5.2 | -6.2% | **-20.0%** | +13.8pp |
| gin | Kimi K3 | -6.4% | **-20.0%** | +13.6pp |

The three open-model rows were added on 2026-08-04 and **the lever is the same
size for all five**: +13.0pp to +13.8pp, a 0.8pp spread. This is the first
corpus in the file where a comment-stripping delta has been checked against
more than the two embedded tokenizers, and the answer is that the delta is a
property of the source, not of the vocabulary — worth stating because the
per-tokenizer rule elsewhere in this file is a warning about *totals*, and it
would have been just as easy for the deltas to scatter.

**+13.0pp was the largest comment-stripping delta measured in this file** when
it was taken, ahead of rack's +11.6pp and express's +8.1pp; commons-lang's
+39.4pp on the same day and csvhelper's +24.0pp the next have since put it
third. It is the same structural
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

##### What `--java-strip-comments` adds

Same corpus, same LF checkout, the added flag being the only difference:

| Corpus | Tokenizer | Without comment stripping | With `--java-strip-comments` | Delta |
|---|---|---|---|---|
| commons-lang | o200k_base | -6.1% | **-45.5%** | +39.4pp |
| commons-lang | cl100k_base | -6.2% | **-46.6%** | +40.4pp |
| commons-lang | Qwen3.6 | -5.6% | **-45.3%** | +39.6pp |
| commons-lang | GLM-5.2 | -6.2% | **-46.6%** | +40.4pp |
| commons-lang | Kimi K3 | -6.0% | **-45.5%** | +39.5pp |

The three open-model rows were added on 2026-08-05, and the finding gin's
table reported holds here too and more strongly: **the lever is the same size
for all five**, +39.4pp to +40.4pp, a 1.0pp spread on a delta an order of
magnitude larger than gin's. Two corpora in two languages now say the same
thing — the size of a comment-stripping delta is a property of how much of the
source is comment, not of the tokenizer's vocabulary. That is worth stating
precisely because the per-tokenizer rule elsewhere in this file warns about
*totals*: those genuinely do scatter (Qwen3.6 counts this tree 5.7% larger
than `o200k_base` before compression), and the deltas do not.

**+39.4pp is the largest comment-stripping delta measured in this file** —
three times gin's +13.0pp, and the reason is that Javadoc is an ordinary block
comment to the grammar. The flag deletes the whole published API documentation
of a library whose product *is* its documented API. There is no Javadoc-only
lever and no directive-comment carve-out of the kind Go has: it takes every
comment or none.

**0 refusals with the flag**, at all five tokenizers, exactly as without it,
and the file count is unchanged at 500. Absolute saving at o200k: 790,215
tokens per full-repo prompt.

##### What `--csharp-strip-comments` adds

Same corpus, same LF checkout, the added flag being the only difference. Both
columns are measured over the same **459** files — the flag no longer changes
which files format, so the delta is apples-to-apples and these are exactly the
percentages in the C# section above and the aggressive table:

| Corpus | Tokenizer | Without comment stripping | With `--csharp-strip-comments` | Delta |
|---|---|---|---|---|
| csvhelper | o200k_base | -14.3% | **-38.3%** | +24.0pp |
| csvhelper | cl100k_base | -13.6% | **-39.5%** | +25.8pp |

+24.0pp is the second-largest comment-stripping delta in this file, behind
commons-lang's +39.4pp and ahead of gin's +13.0pp, and it is the same
structural reason as Java's: the C# default keeps every comment, so the flag
is the whole difference, and `///` documentation on every public member makes
a library corpus comment-dense — 5,387 of CsvHelper's 51,745 lines are `///`
lines before any `//` or `/* */` is counted. It is smaller than Java's because
CsvHelper's documentation is one-line summaries where commons-lang's is
multi-paragraph Javadoc, and because C#'s brace lines give the *default* run a
much larger base to start from.

**The refusal count stays at 2 with the flag.** It used to go from 2 to 3 —
the one place in this file where a strip flag added a refusal — the extra file
being `HeaderPrefixMapTests.cs`, whose entire content is comments; stripping
them leaves an empty file and the equivalence check rejected it. That was
class 2, fixed 2026-08-05 in the shared tree-sitter engine and re-measured
here; both classes are triaged in the C# section above. No strip flag in this
file adds a refusal any more. Absolute saving at o200k: 144,253 tokens per
full-repo prompt.

`--csharp-strip-comments` is the C# backend's only lossy flag. Note what it
does *not* remove: preprocessor directives are not comments, so `#if`,
`#region`, `#pragma` and `#nullable` all survive it and stripping cannot
change which arm of a conditional compiles.

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
2026-08-04.

**Both were measured on all three open-model tokenizers on 2026-08-04**, so
the gap is closed and every corpus in this file now has all five. Aggressive
figures — gin -18.7% / -20.0% / -20.0% against -19.4% / -19.9% embedded, and
rack -19.8% / -20.5% / -20.5% against -20.8% / -20.5%. All ten are far below
the ≥40% bar, so **no candidate list below changes** — now on evidence rather
than on the assumption that a missing figure could not have cleared it.

That assumption is worth retiring explicitly, because it was doing real work
for two days: the `o200k_base` figure was never a proxy for the other three
(the table above shows Qwen3.6 diverging from it by up to +7.9pp on a single
corpus), so "far below the bar on `o200k_base`" was not by itself a reason to
believe the open-model figures were too. They are, and now it is measured.

##### Gemma 4 (added 2026-08-06)

Measured 2026-08-06 on the maintainer's Windows machine, rustc 1.95.0, from
the same **LF checkout** as the table above, with
`--tokenizer hf:benchmarks/tokenizers/gemma-4.json`. Two `o200k_base` sanity
checks anchor it to that run before any Gemma number was taken: requests
aggressive and express default reproduced 86,331 → 55,265 and
135,740 → 112,307 exactly. Unlike the 2026-08-02 run this one covers **every**
corpus in the file — all thirteen, at both settings — because the corpus set
stopped growing after csvhelper.

| Corpus | Files | default | aggressive | before (Gemma 4) | after (aggr.) |
|---|---|---|---|---|---|
| tokio | 790 | -17.4% | **-51.9%** | 1,659,757 | 798,170 |
| commons-lang | 500 | -5.0% | **-42.9%** | 1,975,634 | 1,127,349 |
| ripgrep | 99 | -20.9% | -39.6% | 494,063 | 298,643 |
| langchain | 2,531 | -12.7% | -37.1% | 3,575,462 | 2,248,729 |
| fastapi | 1,140 | -23.9% | -37.0% | 917,394 | 578,406 |
| csvhelper | 459 / 458 | -12.4% | -36.2% | 456,124 / 455,994 | 291,052 |
| requests | 36 | -7.3% | -31.9% | 105,590 | 71,871 |
| transformers | 4,700 | -7.0% | -30.3% | 21,303,227 | 14,852,082 |
| express | 142 | -21.7% | -29.6% | 168,589 | 118,742 |
| uv | 719 | -15.3% | -22.8% | 5,948,169 | 4,590,330 |
| django | 3,032 | -9.8% | -20.7% | 5,791,716 | 4,591,172 |
| gin | 95 | -6.8% | -18.8% | 216,156 | 175,474 |
| rack | 104 | -7.6% | -18.2% | 224,100 | 183,226 |

**ripgrep reads 99 files here, not the 98 of the table above.** The extra file
is `pkg/brew/ripgrep-bin.rb`, a Ruby file inside the Rust corpus that the
`tokenpress-ruby` backend now covers; it accounts for exactly the 268-token
before-count difference. This is a coverage change of the same kind as the
`.js` additions noted above, not a formatter change, and it is worth 0.1pp at
most: re-measuring ripgrep's whole row at 99 files gives -37.4% / -37.6% /
-42.6% / -37.6% / -37.3%, against the published -37.4% / -37.6% / **-42.7%** /
-37.6% / -37.3%. Only Qwen3.6 moves, by 0.1pp, so the table above is left
as measured and this is recorded rather than propagated.

csvhelper is the one corpus whose file count differs between the two settings
(459 default, 458 aggressive), for the comment-only-file reason documented in
its own section. **0 verification refusals** at either setting on every other
corpus, matching what the other five tokenizers report.

**Two corpora clear the ≥40% bar on Gemma 4 — tokio and commons-lang — which
is the same pair `o200k_base` gives and three fewer than Qwen3.6.** Gemma is
therefore the *strictest* tokenizer in the set for this hunt, not the most
generous, and ripgrep misses by 0.4pp (-39.6%) where Qwen3.6 clears it at
-42.6%. This is one more instance of the rule the per-tokenizer lists exist to
enforce, and the first where a newly added tokenizer *shrank* no list but
confirmed the OpenAI pair's.

**Gemma 4 counts source substantially larger than every other tokenizer
here** — before-compression, and at LF: transformers 21.30M against
`o200k_base`'s 17.03M (+25.1%), ripgrep 494,063 against 415,858 (+18.8%),
commons-lang 1,975,634 against 1,736,204 (+13.8%). Qwen3.6 was the previous
outlier at +5.7% on that Java tree; Gemma is a much larger one. The totals
rule at the top of this file applies with more force than before: **a token
budget computed for a GPT-4o context is not a Gemma budget.**

**There is no simple story about which setting Gemma likes.** Its default
saving beats `o200k_base`'s on some corpora and trails it on others —
express -21.7% vs -17.3% and gin -6.8% vs -6.4%, but rack -7.6% vs -9.2%,
commons-lang -5.0% vs -6.1% and csvhelper -12.4% vs -14.3%. Nothing here
supports a per-language or per-setting generalisation, and none is made.

**Gemma 4 is an order of magnitude more sensitive to CRLF than anything else
measured.** Re-running ripgrep and requests from a CRLF clone of the same
pins moves the before-counts like this:

| Tokenizer | ripgrep LF → CRLF | requests LF → CRLF |
|---|---|---|
| `o200k_base` | +1.3% | +0.7% |
| `cl100k_base` | +0.9% | — |
| Qwen3.6 | +0.0% | — |
| GLM-5.2 | +0.9% | — |
| Kimi K3 | +1.0% | — |
| **Gemma 4** | **+11.2%** | **+12.6%** |

Qwen3.6 is exactly unchanged; Gemma inflates by more than a tenth. The
practical consequence is a trap: on the CRLF clone ripgrep's aggressive run
reads **-45.1%** on Gemma and would look like a ≥40% candidate, where the LF
figure is -39.6% and is not. **The candidate lists below are LF figures and
ripgrep is not on Gemma's list.** The flip side is a real and previously
unstated saving — for a Gemma-tokenizer model, converting a CRLF tree to LF
is worth about 11-13% on its own, before TokenPress runs at all.

#### Showcase candidates (≥40% aggressive reduction)

Recorded for the ROADMAP P3 task ("hunt for well-known projects per
supported language where the aggressive setting clears ≥40% token
reduction"). The hunt is defined **per target-model tokenizer** — savings
differ enough by tokenizer that a single list would be wrong for most
models. Embedded-tokenizer columns measured 2026-08-01 (re-measured
2026-08-02 with the JS-enabled CLI — unchanged except django, see the
open-model subsection); open-model columns measured 2026-08-02. rack was
added 2026-08-02 and gin 2026-08-04, both measured on the embedded tokenizers
only at first and completed on the maintainer's machine (-20.8% / -20.5% and
-19.4% / -19.9% on the embedded pair, and below the bar on all five once the
open-model columns arrived). **Neither is a ≥40% candidate, so the lists below
are unchanged by their addition.** commons-lang was added 2026-08-04 on the
embedded tokenizers and completed on 2026-08-05, and it **is** a candidate on
all five (-45.5% / -46.6% / -45.3% / -46.6% / -45.5%), so it now joins every
list rather than the two it sat in while its open-model columns were pending.
That it clears the bar on the OpenAI pair was never evidence that it would
clear it on the other three — the lists stay per-tokenizer for the reason
spelled out below, and Qwen3.6 is exactly where commons-lang came closest to
falling short.

**Per-tokenizer candidate lists:**

| Tokenizer | ≥40% candidates (aggressive flags) |
|---|---|
| o200k_base | tokio **-50.5%**, commons-lang **-45.5%** |
| cl100k_base | tokio **-50.9%**, commons-lang **-46.6%** |
| Qwen3.6 | tokio **-55.2%**, commons-lang **-45.3%**, ripgrep **-42.7%**, langchain **-41.1%**, fastapi **-40.1%** |
| GLM-5.2 | tokio **-50.9%**, commons-lang **-46.6%** |
| Kimi K3 | tokio **-50.6%**, commons-lang **-45.5%** |
| Gemma 4 | tokio **-51.9%**, commons-lang **-42.9%** |

**Two corpora now clear the bar on all six tokenizers, not one** — tokio and
commons-lang, since 2026-08-05, and Gemma 4 joined the grid on 2026-08-06
without changing that. tokio stays the headline because it is ahead
by percentage at every one of the six; at Qwen3.6 tokio
reaches **-55.2%** (1,542,030 → 690,601, an
absolute saving of 851,429 tokens per full-repo prompt). The Qwen3.6 list
is five deep because Qwen prices whitespace runs comparatively expensively;
selecting candidates on the `o200k_base` proxy would have missed three of
its five — which is exactly why this hunt is per-tokenizer. Gemma 4 makes the
same point from the other end: it is the strictest of the six, agreeing with
the OpenAI pair on a two-corpus list while ripgrep misses by 0.4pp. Gemma's
figures here are LF; its CRLF ripgrep run reads -45.1% and must not be read
as a candidate (see the Gemma 4 subsection above).

**Clears ≥40% on all six tokenizers — 2 of 13 corpora:**

| Project | Language | Commit | o200k_base | cl100k_base | Absolute saving (o200k) |
|---|---|---|---|---|---|
| tokio-rs/tokio | Rust | `adc2ae7a` | **-50.5%** | **-50.9%** | 703,851 tokens |
| apache/commons-lang | Java | `29ccc766` | **-45.5%** | **-46.6%** | 790,215 tokens |

Open-model figures for both are in the per-tokenizer lists above; the two
embedded columns are kept here because the absolute-saving comparison below
is quoted at `o200k_base`.

```bash
target/release/tokenpress stats benchmarks/corpus/tokio \
    --tokenizer o200k_base --rs-strip-doc-comments
target/release/tokenpress stats benchmarks/corpus/commons-lang \
    --tokenizer o200k_base --java-strip-comments
```

tokio is the strongest showcase in the corpus by percentage: it is
doc-comment-dense, and Rust additionally loses all `//` / `/* */` comments
through the `syn` token stream, so more than half of a full-repo prompt
disappears. commons-lang saves **more tokens in absolute terms** off a smaller
tree — 790,215 against tokio's 703,851 — because `--java-strip-comments`
deletes Javadoc, and a library whose whole product is its documented API is
mostly Javadoc by weight. It is the first non-Rust corpus to clear the bar on
either embedded tokenizer (Python clears it only on Qwen3.6), and since
2026-08-05 it is measured on all five, clearing the bar on every one of them.

The absolute-saving comparison holds at four of the five tokenizers, and the
exception is instructive. Tokens saved per full-repo prompt, commons-lang
against tokio: 790,215 vs 703,851 at `o200k_base`, 780,535 vs 709,324 at
`cl100k_base`, 780,976 vs 710,033 at GLM-5.2, 790,704 vs 703,221 at Kimi K3 —
and **831,223 vs 851,429 at Qwen3.6, the one tokenizer where tokio saves more
in absolute terms.** It is the same property behind gin losing its
lowest-saving title everywhere except Qwen3.6: Qwen counts these trees larger
to begin with, and it inflates tokio's Rust more than commons-lang's Java
(1,542,030 against o200k's 1,394,248, +10.6%; commons-lang 1,835,291 against
1,736,204, +5.7%), so the same percentage buys tokio more tokens there. Which
corpus "saves most" is a per-tokenizer question in absolute terms as well as
relative ones.

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
is made for Go in either direction** either. Java is the one language where a
single corpus did clear the bar — but one corpus is still not a search, so
**no general ≥40% claim is made for Java** beyond this pin, and none at all
for Java on the three open-model tokenizers, which are unmeasured.

Two caveats on this list:

* **ripgrep's number is not directly comparable to the historical aggressive
  table** (-37.4% here vs -38.2% there). The Rust flag set did not change —
  `--rs-strip-doc-comments` is the same single flag in both runs — and the
  difference is entirely the CRLF→LF checkout described above.
* **Each list is valid only for models using that tokenizer.** The per-
  tokenizer measurement (2026-08-02) confirmed the gap is decisive: three of
  Qwen3.6's four candidates are invisible on the `o200k_base` proxy. Judge
  ≥40% membership on the tokenizer of the model you actually run. Gemma 4
  was added 2026-08-06 and has its own list; Gemma 2 and Gemma 3 remain
  unmeasured, and their repos really are gated.

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
* C# is the other extreme of that axis, and it settles that the axis is
  *layout*, not language family. csvhelper's default `-14.3%` is the highest
  context-lossless figure in this file — more than double rack's `-9.2%`,
  gin's `-6.4%` and commons-lang's `-6.1%` — while its
  `--csharp-strip-comments` delta (+24.0pp) is the second-largest. C# and Java
  are the same kind of language with the same comment conventions, so the gap
  between -14.3% and -6.1% is not about the grammar: CsvHelper puts braces on
  lines of their own (21.8% of its lines hold one brace and nothing else) and
  commons-lang, checkstyle-formatted, does not. Tab indentation, which is what
  holds gin down, does not save a corpus that spends a fifth of its lines on
  punctuation.
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

# Java corpus - like the Go pin, it holds no file of any other supported
# language, so the tree can be passed directly
target/release/tokenpress stats benchmarks/corpus/commons-lang \
    --tokenizer o200k_base
target/release/tokenpress stats benchmarks/corpus/commons-lang \
    --tokenizer o200k_base --java-strip-comments   # full aggressive
# no --verify external run is recorded for this corpus: it is one probe plus
# three ~0.4 s javac spawns per file, which over 500 files measures JVM
# startup rather than TokenPress. The external Java gate is covered by the
# crate's own tests.

# C# corpus - like rack and unlike the Go and Java pins, this one does hold
# files of another supported language (4 generated-site `.js`), so the `.cs`
# paths are listed explicitly
find benchmarks/corpus/csvhelper -not -path '*/.git/*' -name '*.cs' -print0 \
    >/tmp/csvhelper-files.nul
xargs -0 target/release/tokenpress stats --tokenizer o200k_base \
    </tmp/csvhelper-files.nul                      # default settings
xargs -0 target/release/tokenpress stats --tokenizer o200k_base \
    --csharp-strip-comments </tmp/csvhelper-files.nul   # full aggressive
# no --verify external run is recorded for this corpus either: it is one probe
# plus three ~0.358 s `dotnet csc.dll` spawns per file, which over 461 files
# measures toolchain startup rather than TokenPress. The external C# gate is
# covered by the crate's own tests.

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
(`benchmarks/verify-upstream.sh <requests|ripgrep|express|rack|go|java|csharp|all>`)
does the same thing for every target:

1. **Pinned corpus, SHA-asserted.** The corpus is cloned at the same tag as
   `fetch.ps1`/`fetch.sh` and its `HEAD` is asserted against a hard-coded
   commit SHA, so a retagged upstream cannot silently change what is verified
   (requests `0e322af8`, ripgrep `4649aa97`, express `dbac741a`, rack
   `e1f22fdb`, gin `6ad6205e`, commons-lang `29ccc766`, csvhelper
   `5dad8b8b`).
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
| java | `mvn -B -Dmaven.test.failure.ignore=true test` | each copy builds into its own `target/`; the baseline runs first and warms the Maven local repository, so the formatted run resolves the same artifacts from cache (formatting never touches `pom.xml`); private `TMPDIR` per run; proxy env vars **kept** for both runs, because Maven needs Maven Central and this suite drives no localhost servers |
| csharp | `dotnet build` then `dotnet test --no-build --logger trx`, both with `-p:TargetFrameworks=net8.0` | each copy builds into its own `bin/` and `obj/` and writes its trx to its own results directory; the baseline runs first and warms the NuGet package folder, so the formatted run restores the same packages from cache (formatting never touches a `.csproj`); private `TMPDIR` per run; proxy env vars **kept** for both runs, for the same reason as java |

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

Three java-specific choices, finally:

* **`-Dmaven.test.failure.ignore=true`.** A failing test is the thing being
  measured, so it must not fail the build; a compile error or an unresolvable
  dependency must. With that flag the two cases separate cleanly: surefire
  records failures in its reports and Maven still exits 0, so a **non-zero
  exit means Maven never produced a comparable result** and the target either
  dies (baseline) or reports the strongest possible divergence — the formatted
  copy no longer builds (formatted). None of the other targets get this for
  free; the go target has to detect a build failure by grepping its event
  stream for `[build failed]`.
* **The per-test data comes from `target/surefire-reports/TEST-*.xml`, and it
  is parsed rather than pattern-matched.** Surefire's console output prints
  only a per-class summary line, which is exactly the aggregate this harness
  refuses to compare. The XML is reduced by a small Java program run through
  the JDK's single-file source launcher (`java Reduce.java`), which uses the
  JDK's own XML parser — the same move the go target makes with `go run` and
  the express target with node, and it adds no prerequisite the target did not
  already have. It deliberately does not reach for `xmllint` or `python`,
  neither of which this script requires anywhere else.
* **Nothing is gated on stderr for any `mvn`, `java` or `javac` invocation.**
  A JVM can write to stderr on a completely successful run — a container that
  exports `JAVA_TOOL_OPTIONS` makes every JVM start announce it there, which
  is exactly what the measurement container does — so a stderr-based check
  would report failure 100% of the time. Exit code and report files are the
  signals.

Three csharp-specific choices, which are the java ones re-derived against a
different toolchain:

* **Build and test are two invocations.** `dotnet test` has no equivalent of
  `-Dmaven.test.failure.ignore=true`: it exits 1 both for a failing test and
  for a project that does not compile, and those are precisely the two cases
  this harness must keep apart. Building first and then running
  `dotnet test --no-build` separates them — the build must exit 0, the test
  run may exit 1, and anything above 1 means VSTest never produced a
  comparable result.
* **`-p:TargetFrameworks=net8.0` on every invocation, and it is not a
  preference.** CsvHelper's projects list `net9.0` first and `net48`, `net47`
  and `net462` last. An SDK that cannot target .NET 9 fails the *restore* of
  every framework rather than just that one (`NETSDK1045`), so without the
  override the pinned SDK 8 produces no run at all; and the three `net4x`
  frameworks are not runnable on Linux under any SDK. It is a global MSBuild
  property, so it reaches the referenced library project too and both copies
  are built identically. All 461 `.cs` files are formatted and measured
  either way — this narrows what is *executed*, not what is rewritten.
* **The reducer is a C# program, built with the SDK the target already
  requires.** Its per-test data comes from the `trx` logger's XML, because
  VSTest's console output prints only a per-assembly summary — the aggregate
  this harness refuses to compare. Unlike the java reducer it cannot be run
  from source: `dotnet run app.cs` first exists in SDK 10, and this target is
  built for SDK 8, so it is compiled once into its own output directory and
  the assembly invoked directly. `dotnet run` is avoided even where it would
  work, because it writes build output to stdout and that would land in the
  reduced listing. It uses only the BCL's XML reader — no package, no network.

Unlike the go and rack targets, whose dependencies can be warmed once and then
worked from, **the java target needs Maven Central reachable at run time**:
Maven resolves commons-lang's test dependencies *and* its build plugins from
there into the Maven local repository, and there is no offline path that does
not presume that repository is already populated. A run that dies resolving
dependencies is a network problem, not a flake, and the script says so. **The
csharp target is in the same position**, for the same reason: NuGet resolves
CsvHelper's test dependencies into the machine-wide package folder, and there
is no offline path that does not assume that folder is already populated.

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
* **java**: the ids need none — surefire's per-test XML records a `classname`
  and a `name`, and the `name` of a parameterized test carries its argument
  types and its invocation index (`testNext(int)[3]`) but never a path and
  never a line number. The *outcomes* need one, and it is narrow: inside
  `FastDateParser_TimeZoneStrategyTest`, `pass` and `skip` are folded into a
  single `pass-or-skip` value. `fail` and `error` are untouched, there and
  everywhere else. That class is parameterized over every locale the JDK
  offers and deliberately converts an environment-dependent time-zone parse
  failure into an assumption abort — "Mark as an assumption failure instead of
  a hard fail", in commons-lang's own comment — and its Javadoc says outright
  that it "Breaks randomly on GitHub for Locale pt_PT". Measured here: four
  pristine runs of the *unformatted* tree produced 19, 13, 19 and 13 skips,
  the difference being six `testTimeZoneStrategy_DateFormatSymbols(Locale)`
  invocations on Portuguese locales. Without the fold the target reports
  DIVERGED at random on a difference formatting did not cause. The cost is
  stated rather than hidden: a formatting change that moved a test in this one
  class between passing and being assumption-aborted would not be seen. Every
  other outcome in the 11,720 is compared exactly, and the printed
  baseline/formatted tallies show the unfolded counts.
* **csharp**: none, in either direction. The `testName` the trx logger records
  is the fully-qualified method name, with a theory's arguments appended and
  neither a path nor a line number anywhere in it — checked against the run
  directory, which appears 1,071 times in the report and never once inside a
  `testName`. And unlike commons-lang's, none of CsvHelper's outcomes needed
  folding: two pristine copies of the *unformatted* tree, built and tested
  back to back exactly as the harness does, produced byte-identical listings.
  Every one of the 1,063 outcomes is compared exactly.

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

### commons-lang 3.17.0 — IDENTICAL

Run 2026-08-04, the first behavioral verification of the `tokenpress-java`
backend. Linux, Maven 3.9.11, JDK 21.0.10.

| | Value |
|---|---|
| `.java` files | 500 |
| Rewritten | 500 |
| Refused by verification | 0 |
| Unchanged | 0 |
| Verdict | **IDENTICAL** — exit 0 |

| Outcome | Baseline | Formatted |
|---|---|---|
| pass | 11,701 | 11,707 |
| fail | 0 | 0 |
| error | 0 | 0 |
| skip | 19 | 13 |

11,720 rows on each side — every test case surefire wrote a report for, from
312 test classes. Both Maven runs report `BUILD SUCCESS` and exit 0. This is
the largest suite in this file by an order of magnitude: 11,720 outcomes
against gin's 595 and rack's 2,354.

**The six-row pass/skip difference in that table is upstream flake, not a
formatter effect, and the harness is built to say so** — the next subsection
has the measurement. Those six rows are the only place the two sides differ,
they are all in one test class, and the comparison folds `pass` and `skip`
together inside that class, so the verdict is IDENTICAL and the exit code is
0. `fail` and `error` are compared exactly, there and everywhere else; every
other one of the 11,720 outcomes matched value for value.

It is also the only corpus where **every** file was rewritten: 500 of 500,
with nothing refused and nothing left in canonical form already (gin had one
such file, `doc.go`). Like express and gin and unlike requests and rack, the
suite is green on both sides — it is a pure-utility library that touches no
network — so here *identical outcomes* and *all tests pass* happen to
coincide. The claim being made is still only the first one.

**The skip count is not a constant, and this corpus proves it on one
machine.** The pristine, *unformatted* tree was run six times in the same
container on the same JDK and reported **19, 13, 19, 13, 19 and 13** skips. The
difference is six invocations of
`FastDateParser_TimeZoneStrategyTest#testTimeZoneStrategy_DateFormatSymbols(Locale)`,
a test parameterized over every locale the JDK offers, whose body converts a
time-zone parse failure into `assumeTrue(false, …)` — "mark as an assumption
failure instead of a hard fail" in commons-lang's own words — for `_`-bearing
locales on JDK 17 and later. Whether those six abort or pass depends on the
JDK's locale and time-zone data and on what the rest of the suite has left in
the JVM's defaults, and surefire's default `runOrder` is `filesystem`. So:

* **No verification target may assert an absolute skip count.** This harness
  compares a baseline run against a formatted run made in the same
  environment, in the same script invocation, and asks only whether they
  agree. An earlier ROADMAP note recorded "13 skipped" for this pin and a
  later measurement recorded 19; both are correct observations of an unstable
  number.
* **A control run settles what causes it: position, not formatting.** Two
  copies of the *unformatted* tree were built and tested back to back, exactly
  as the harness does, with nothing formatted at all. The first reported **19**
  skips and the second **13** — the same split, the same six invocations, with
  the formatter removed from the experiment entirely. It is not a simple
  first-run/second-run effect either: a later end-to-end target run had both
  sides at 19. The only claim the evidence supports is that the number is
  unstable and that formatting is not what moves it, and that is what the fold
  rests on.
* **It is not a run-order artifact, and it is not confined to the class.**
  Surefire's default `runOrder` is `filesystem`, so that was the first
  suspicion; it is wrong. In the run where the two sides disagreed, the 312
  classes executed in an *identical* order on both copies. Run on its own the
  class is stable — three consecutive `-Dtest=FastDateParser_TimeZoneStrategyTest`
  runs each reported 4 skips — so what moves the six invocations is JVM state
  the rest of the suite perturbs, not anything inside the class.
* **The harness folds that one axis and says so.** Inside
  `FastDateParser_TimeZoneStrategyTest`, `pass` and `skip` are compared as a
  single `pass-or-skip` outcome; `fail` and `error` are compared exactly
  there, and every outcome is compared exactly everywhere else. This was not a
  precaution: an early end-to-end run of the target reported **DIVERGED** with
  a diff consisting of exactly those six invocations and nothing else — a
  false finding, since two unformatted copies produce the same split. With the
  fold, three further end-to-end runs — two that produced the same 19-vs-13
  split and one where both sides landed on 19 — all reported IDENTICAL and
  exited 0. What the fold costs is stated rather than hidden: a formatting
  change that moved a test in this one class between passing and being
  assumption-aborted would not be seen. The printed tallies stay unfolded, so
  the raw counts remain visible.

Two limits on what the run proves, the same two the gin section records:

* **Line numbers are not preserved by any backend, and this suite does not
  test them.** Java stack traces carry `file:line`, so a test that
  golden-compared a stack trace would diverge legitimately once formatting
  deletes blank lines — the shape of rack's `__LINE__` finding. commons-lang
  raises and inspects exceptions throughout, but no assertion in its 11,720
  tests reads a line number out of one. The class is untested by this corpus,
  not disproved by it.
* **No test can observe a comment.** Comments do not run, so an IDENTICAL
  verdict says nothing about comment preservation either way. For Java that
  is moot at default settings — the rewrite is whitespace-only, verified
  file-by-file above — but the caveat is the same one recorded for Rust and
  JS/TS.

One thing this target does *not* share with gin: there are no build-constraint
files here. Every one of the 500 `.java` files is under `src/main/java` or
`src/test/java` and is compiled by the run, so unlike gin's five tag-gated
files, nothing was formatted without also being compiled.

### csvhelper 33.1.0 — IDENTICAL

Run 2026-08-05, the first behavioral verification of the `tokenpress-csharp`
backend. Linux, .NET SDK 8.0.129, target framework `net8.0`.

| | Value |
|---|---|
| `.cs` files | 461 |
| Rewritten | 459 |
| Refused by verification | 2 (`docs-src/…/Extensions.cs`, `docs-src/…/Formatters/XmlDocFormatter.cs`) |
| Unchanged | 0 |
| Verdict | **IDENTICAL** — exit 0 |

| Outcome | Baseline | Formatted |
|---|---|---|
| pass | 1,059 | 1,059 |
| fail | 4 | 4 |
| skip | 0 | 0 |

1,063 rows on each side, the whole of `CsvHelper.Tests` on `net8.0`. Both
`dotnet build` invocations exit 0 and both `dotnet test` invocations exit 1,
that 1 being the four failures below rather than anything about the build. The
whole target takes ~42 s end to end, the fastest of the seven.

**The 4 failures are environment artifacts and fail identically on the
unformatted copy** — the requests case, not the rack case. Two of them
(`TypeConverterFactoryTests.WriteTypeConverterFactory` and
`…WriteTypeConverterGenericInt`) compare CsvHelper's RFC-4180 `\r\n` output
against a C# **raw string literal**, whose content is whatever line endings
the checkout has: LF here, so the expectation is `\n` and the test fails;
on a CRLF checkout the same two tests pass. That makes the baseline failure
count itself checkout-dependent, which is the sharpest argument in this file
for comparing a baseline run against a formatted run made in the same
environment rather than against a recorded constant. One
(`MultipleFieldsFromOnePropertyTests.WriteMultipleFieldsFromSinglePropertyTest`)
compares a formatted `DateTime` against a literal that predates the CLDR
change to the space before `AM`/`PM`; one
(`CultureInfoAttributeTests.CsvConfiguration_FromType_InvalidAttribute_ThrowsCultureNotFoundException`)
expects an invalid culture name to throw, which ICU on Linux does not do.
None involves formatting, and the claim being made is *identical outcomes*,
not *all tests pass*.

**Reproduced twice, byte-identically, plus a control.** Two consecutive
end-to-end runs of the target both reported IDENTICAL and exited 0, and a
separate control — two copies of the *unformatted* tree, built and tested back
to back exactly as the harness does — produced listings that diff clean. There
is no commons-lang-style unstable axis in this suite and therefore no fold:
all 1,063 outcomes are compared exactly.

**`--csharp-strip-comments` was checked separately, by hand, and is also
IDENTICAL.** The target itself formats at default settings only, like every
other target in this harness, so this was run outside it: a third copy of the
tree formatted with the flag (3 refusals rather than 2, the extra being the
comment-only file), built and tested the same way, produced a listing that
diffs clean against the baseline — 1,059 pass / 4 fail again. Comments do not
run, so this is a weaker statement than the default-settings one and is
recorded as a check rather than promoted into the script; what it rules out is
the whitespace *around* a deleted comment being mis-joined.

**The two refused files never reach the compiler anyway**, which is worth
saying so the verdict is not read as stronger than it is. Both live in
`docs-src/CsvHelper.DocsGenerator`, a `netcoreapp2.2` documentation generator
that this target does not build; they are unformatted in the formatted copy
and identical to the baseline, so they contribute to neither side. What the
suite covers is `src/CsvHelper` and `tests/CsvHelper.Tests` — 435 of the 461
files, all of them rewritten.

Three limits on what the run proves:

* **`net8.0` only.** The pin's projects also list `net9.0`, `net48`, `net47`
  and `net462`; the first cannot be targeted by the pinned SDK and the other
  three cannot be executed on Linux. Formatting is applied to all 461 files
  regardless — this narrows what is *run*, not what is *rewritten* — but a
  framework-conditional code path behind `#if NET462` is compiled by neither
  copy. CsvHelper has 11 `#if` directives in total.
* **Line numbers are not preserved by any backend, and this suite does not
  test them.** .NET stack traces carry `file:line` and CsvHelper's own
  exceptions carry rich context, but no assertion in its 1,063 tests reads a
  line number out of one. The class is untested by this corpus, not disproved
  by it — the same position gin and commons-lang are in, and the opposite of
  rack's.
* **No test can observe a comment.** Comments do not run, so an IDENTICAL
  verdict says nothing about comment preservation either way. For C# that is
  moot at default settings — the rewrite is whitespace-only, verified
  file-by-file above — but the caveat is the same one recorded for Rust,
  JS/TS, Go and Java.

### Scope and caveats

* These runs verify **default settings only**. The aggressive settings above
  (`--py-strip-comments`, `--py-strip-annotations`, `--rs-strip-doc-comments`,
  `--js-strip-comments`, `--ruby-strip-comments`, `--go-strip-comments`,
  `--java-strip-comments`, `--csharp-strip-comments`) are
  knowingly lossy and are not covered by this harness — stripping doc comments would delete the doc
  tests being compared.
* The Rust and JS/TS comment-loss caveats still stand: Rust `//` and `/* */`
  comments, and JS/TS trailing and expression-position comments, are dropped
  unconditionally, and no test suite can detect that, because comments do not
  run. Behavioral equivalence is not context equivalence. Ruby, Go, Java and
  C# are the exceptions — they drop no comments at default settings — but
  they share the line-number caveat below.
* **Line numbers are not preserved by any backend**, and the rack run above is
  the measured proof that this is observable: `__LINE__`, `caller`, backtraces
  and anything derived from them move when blank lines and indentation go.
  Only rack's suite happens to assert on one. gin's IDENTICAL verdict is
  emphatically not a counter-example: Go's `testing` prints `file:line` and
  gin's own `recovery.go` builds a stack trace from `runtime.Caller`, but no
  gin test asserts on a line number, so the class is untested there rather
  than absent, and commons-lang's is the same case — Java stack traces carry
  `file:line`, but nothing in its 11,720 tests asserts on one. csvhelper is
  the same case a third time.
* Only seven corpora are covered (requests, ripgrep, express, rack, gin,
  commons-lang, csvhelper). The larger corpora in the table above are verified
  structurally, not behaviorally.
* The 5 requests failures, the 2 rack failures and the 4 csvhelper failures
  are environment artifacts, not upstream-green results; the claim is
  *identical outcomes*, not *all tests pass*. Two of csvhelper's four are
  artifacts of the checkout's *line endings* and would pass on Windows, which
  is the clearest case in this file for why the baseline is re-measured on
  every run instead of recorded.
* The express target additionally requires `node`, `npm` and npm registry
  access; the rack target requires `ruby`, `bundler` and rubygems.org access;
  the go target requires the Go toolchain and Go module proxy access; the java
  target requires `mvn`, a JDK and Maven Central access; the csharp target
  requires the .NET SDK and nuget.org access. They are the only targets with a
  network prerequisite beyond the git clone; without it the run exits 2 (never
  ran) rather than reporting a verdict. java and csharp are the strictest of
  the five: unlike the go and rack dependencies, neither Maven's nor NuGet's
  can be warmed once and then worked from offline.
* **The csharp target additionally pins a single target framework**
  (`-p:TargetFrameworks=net8.0`), because the corpus lists one framework the
  pinned SDK cannot target and three that Linux cannot execute. Everything is
  formatted; only `net8.0` is run.

### Reproduce

```bash
./benchmarks/verify-upstream.sh requests   # pytest, junit per-test-id diff
./benchmarks/verify-upstream.sh ripgrep    # cargo test, multiset diff
./benchmarks/verify-upstream.sh express    # mocha, JSON reporter per-test-id diff
./benchmarks/verify-upstream.sh rack       # minitest, reporter-plugin per-test-id diff
./benchmarks/verify-upstream.sh go         # go test -json, per-test-id diff
./benchmarks/verify-upstream.sh java       # mvn surefire, XML per-test-id diff
./benchmarks/verify-upstream.sh csharp     # dotnet test, trx per-test-id diff
./benchmarks/verify-upstream.sh all
# exit 0 = identical, 1 = diverged, 2 = the comparison never ran
# the rack target currently exits 1 - see the __LINE__ finding above
# the go, java and csharp targets exit 0
```
