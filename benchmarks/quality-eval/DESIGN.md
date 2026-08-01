# Downstream Quality Evaluation — Experimental Design

**Status: design only. Nothing in this document has been executed.**
Running it requires calling paid LLM APIs, which needs explicit maintainer
approval (ROADMAP P3, "Downstream quality evaluation"; the question is
recorded in `LOOPLOG.md`). This file is the pre-registration: it is committed
*before* any data is collected, and the git SHA of this file is recorded in
the results so that anyone can check the analysis against the plan that was
written down first.

## 0. Why this exists

`benchmarks/RESULTS.md` and `benchmarks/SHOWCASE.md` measure exactly one
thing: how many tokens TokenPress removes. They measure nothing about what
those tokens were worth. Both files, and `README.md`, say so explicitly — see
§10 for the sentences this experiment is meant to replace.

The experiment measures **LLM answer quality on code-understanding tasks**
across three context variants of the *same* source unit:

| Variant | How it is produced |
|---|---|
| **V0 `original`** | The file bytes as checked out at the pinned corpus commit. No TokenPress. |
| **V1 `default`** | `tokenpress format` with no strip flags. |
| **V2 `aggressive`** | Python: `--py-strip-comments --py-strip-annotations --py-strip-docstrings`. Rust: `--rs-strip-doc-comments`. |

These are exactly the flag sets already benchmarked in
`benchmarks/RESULTS.md` ("Aggressive + strip_docstrings") and
`benchmarks/SHOWCASE.md` ("Flags used"), so the token-savings side of the
trade-off is already known per corpus and does not need re-measuring.

**Rust interpretation caveat, stated up front.** For Rust, V1 is *already*
lossy: the `syn`-based backend drops every `//` and `/* */` comment even at
default settings (README "Rust is not context-lossless, even at default
settings"). So for Rust the V0→V1 contrast measures *regular-comment loss*
and the V1→V2 contrast measures *doc-comment loss*. For Python, V0→V1 is
context-lossless (whitespace + PY09 import merging only) and should be the
null-effect control; if V0→V1 shows a real effect on Python, either the
formatter is not as lossless as claimed or the measurement is noisy — both
are findings worth publishing.

---

## 1. Research question and hypotheses

### 1.1 Primary research question

For a fixed code-understanding task and a fixed model, **does replacing the
original source in the prompt with its TokenPress-formatted equivalent change
the quality of the model's answer, and by how much?**

Two contrasts per (task family × model):

* **C1 = V1 − V0** (default settings): the "is the default really free?" question.
* **C2 = V2 − V0** (aggressive settings): the "what does stripping prose cost?" question.

C3 = V2 − V1 is reported as a derived quantity (it is C2 − C1 by
construction) but is not one of the pre-registered primary tests.

### 1.2 Hypotheses

The design does not assume a direction. All three outcomes are live and all
three get published (§6.4):

* **H0 (null / equivalence).** Quality is unchanged within a pre-registered
  equivalence margin. Plausible for V1 on Python (context-lossless by
  construction), and plausible for V2 on tasks whose answers are derivable
  from code alone.
* **H− (stripping hurts).** Quality drops. Most plausible for V2 on tasks
  whose ground truth lives in prose (a docstring stating a default value, a
  doc comment describing a side effect not visible in the body).
* **H+ (stripping helps).** Quality *improves*. This is not a joke
  hypothesis: removing prose removes (a) stale or wrong comments, (b)
  distractor text, and (c) tokens that dilute attention over a long context.
  Any of the three could raise accuracy on code-derivable questions.

### 1.3 Moderator hypotheses (pre-registered, tested as secondary analyses)

The answer is expected to differ by:

* **Task type.** Bug localization should be the *least* prose-dependent
  (the bug is in the code), function summarization the *most* (the docstring
  is close to the gold answer). API-usage QA should sit in between and split
  cleanly by where the answer lives.
* **What was stripped.** The three Python strip flags are not
  interchangeable. Docstrings carry behavioral prose; `#` comments carry
  local rationale; annotations carry machine-checkable type facts that are
  often *also* recoverable from the body. Prediction: docstring loss > comment
  loss > annotation loss in effect size. This is tested as a covariate rather
  than by running six additional variants (which would triple the budget);
  see §2.6 for the single-flag ablation that is deferred to a stretch stage.
* **Where the gold answer lives.** Every API-QA item is labeled
  `answer_locus ∈ {code, prose, both}` by the item generator (§3.4). The
  pre-registered prediction is that C2 ≈ 0 on `code` items and C2 < 0 on
  `prose` items. If that pattern holds, it is the mechanistic result: the
  aggressive flags cost quality exactly and only where the answer was in the
  prose.
* **Language.** Rust V1 already loses comments; Python V1 does not. Expect
  |C1| larger for Rust than Python.

---

## 2. Corpus and sampling

### 2.1 Which corpora

Four of the eight already-pinned corpora, kept at exactly the SHAs in
`benchmarks/fetch.sh` and `benchmarks/RESULTS.md`. No new corpora, no
re-pinning, no change to `fetch.sh`.

| Corpus | Language | Pinned commit | Why this one |
|---|---|---|---|
| [psf/requests](https://github.com/psf/requests) v2.32.3 | Python | `0e322af8` | Behaviorally verified end-to-end by `verify-upstream.sh`; densely documented (-9.0% default / -36.0% aggressive at o200k), so the aggressive treatment has a large dose. 36 `.py` files. |
| [fastapi/fastapi](https://github.com/fastapi/fastapi) | Python | `95f8322ee1dcda7ceace7b1c4f6c9915b36d748f` | Annotation-heavy rather than docstring-heavy (`--py-strip-docstrings` adds only +1.7pp there vs +15.9pp on requests), so it isolates the *annotation* half of the aggressive flag set. 1,136 `.py` files. |
| [BurntSushi/ripgrep](https://github.com/BurntSushi/ripgrep) 14.1.1 | Rust | `4649aa97` | The other behaviorally verified corpus; the one that caught the real emitter bug. 98 `.rs` files. |
| [tokio-rs/tokio](https://github.com/tokio-rs/tokio) | Rust | `adc2ae7af2caaea83985fbdfbc7884c159c486f2` | The showcase headline (-50.5% o200k aggressive): the largest possible dose of doc-comment removal in the corpus set. 790 `.rs` files. |

Excluded and why: django (bulk is fixtures/data tables, low dose), langchain
and transformers (very large, and langchain contains the known non-UTF-8
fixture), uv (mixed-language tree complicates per-language stratification).
The two intentionally-broken fixtures documented in `RESULTS.md` are excluded
by the inclusion criteria anyway.

### 2.2 Unit of sampling

**One "item" = one function (Python `def`/`async def`, including methods) or
one Rust function (`fn`, including `impl` methods), presented inside a
context window cut from its own file.**

The window is: the file's module header (module docstring / `//!` header,
`import` / `use` block) + the enclosing `class` / `impl` signature if any +
the target function + up to 60 lines of trailing sibling context, truncated
to fit the token band in §2.3. The window is cut on the **original** file and
the identical *line range* is what gets formatted into V1 and V2 — i.e. the
formatter is run on the extracted window, not on the whole file, so all three
variants cover the same source region.

Presented to the model with line numbers prefixed (`%5d| `), because bug
localization needs a line coordinate. Line numbering is applied *after*
formatting, per variant, so V1 and V2 carry their own (shorter) numbering.

### 2.3 Inclusion criteria

A candidate function is eligible iff **all** hold:

1. **Size.** The original-variant window is 400–3,500 `o200k_base` tokens
   (measured with `tokenpress stats --json --tokenizer o200k_base`). Target
   mean ≈ 2,400.
2. **Substance.** Body has ≥ 8 non-blank, non-comment lines and ≥ 5
   statements. Excludes trivial getters, `__init__` passthroughs,
   `#[derive]`-only impls, and one-line wrappers.
3. **Not test code.** Path does not match `tests/`, `test/`, `benches/`,
   `**/test_*.py`, `**/*_test.rs`, `**/conftest.py`, `examples/`.
4. **Formats cleanly.** `tokenpress format` on the window succeeds with **0
   verification refusals** for both V1 and V2. Any refusal drops the
   candidate (and is logged — a refusal here would itself be a bug report).
5. **Round-trips through the token counter.** V0/V1/V2 token counts are all
   obtainable; the item records `saving_ratio` per variant as the treatment
   *dose*.
6. **Not a duplicate.** At most 3 items per file; no function used by more
   than one item; no two items whose windows overlap by more than 25% of
   their lines.

Per-family extra criteria:

* **Summarization** additionally requires a docstring / doc comment of ≥ 20
  words on the target function (so there is prose to lose). A 20% stratum of
  items with a *short* (< 20 words) or absent docstring is retained as a
  within-family control: if the effect is really about prose, that stratum
  should show a much smaller C2.
* **API-usage QA** additionally requires the target to be public API
  (Python: name not `_`-prefixed and reachable from the package's
  `__init__.py`; Rust: `pub` and reachable from the crate root) and to have
  at least one mechanically extractable ground-truth fact (§3.4).
* **Bug localization** additionally requires ≥ 1 mutation site whose
  surrounding 7-token context is *unique within the window* (§3.3), and
  requires the mutant to re-parse and to format into V1/V2 with 0 refusals.

### 2.4 Sampling procedure (deterministic)

1. Enumerate every eligible function in a fixed order: corpus → path
   (byte-sorted) → byte offset within file.
2. Key each candidate with `k = SHA256(MASTER_SEED || corpus || path ||
   qualified_name || family)`, hex, first 16 chars.
3. Sort by `k` ascending; take the first N per (corpus × family).
4. Families draw from disjoint pools: a function used by one family is
   removed from the others' candidate lists before their draw, in the fixed
   family order `summarization → bug_localization → api_qa`.
5. If a drawn candidate later fails a criterion (e.g. a mutant refuses
   verification), it is dropped and replaced by the next key in order. Every
   drop is recorded in `items/dropped.jsonl` with the reason, so the
   selection is auditable and no silent filtering happens after data
   collection.

`MASTER_SEED = "tokenpress-quality-eval-2026-08-01"` (a string constant, fed
to SHA256; recorded in the manifest).

### 2.5 Sample size and power

**n = 80 items per task family** (20 per corpus × 4 corpora), **240 items
total**, each answered under 3 variants by 2 models → **1,440 generations**.

The design is fully paired: the same item appears under V0, V1 and V2, so
every contrast is a within-item difference and all between-item variance
(function difficulty, corpus, length) cancels. This is what buys usable power
at n = 80.

Minimum detectable effect (MDE), paired, two-sided α = 0.05, power 0.80.
Using the normal approximation `dz ≈ (z_{0.975} + z_{0.80}) / √n = 2.80/√n`;
Wilcoxon signed-rank needs ≈ 5% more n than the paired *t* under normality
(ARE 0.955), which is folded into the numbers below by rounding up:

| Analysis cell | n | MDE (dz) | MDE in rubric points (SD_Δ = 0.18 on a 0–1 scale) |
|---|---|---|---|
| Pilot, one family, one model | 24 | 0.59 | 0.106 (10.6 pp) |
| **Primary: one family × one model** | **80** | **0.32** | **0.058 (5.8 pp)** |
| One family, pooled over 2 models | 160 | 0.23 | 0.041 |
| One model, pooled over 3 families | 240 | 0.19 | 0.033 |
| Everything pooled | 480 | 0.13 | 0.024 |

`SD_Δ = 0.18` is an assumption, not a measurement; the pilot (§8.1)
re-estimates it and the MDE table is recomputed and reported before the full
run. If the pilot shows `SD_Δ > 0.25`, n is raised to 120 per family (cost
impact in §7.3) or the primary analysis is pre-declared to be the pooled
`n = 160` cell.

**Bug localization is binary**, so the primary test is exact McNemar on
discordant pairs, and it is the least-powered metric. With n = 80 and an
assumed 25% discordance rate (20 discordant pairs), McNemar detects
|b − c| ≥ 2.80·√20 ≈ 12.5, i.e. **≈ 15.6 pp** of accuracy difference.
Pooled over both models and all four corpora (n = 240, ~60 discordant pairs)
that improves to **≈ 9 pp**. The pre-registration therefore names the
*pooled* cell as the primary bug-localization test and the per-model cells as
secondary; this is fixed in advance precisely so it cannot be chosen after
seeing the data.

### 2.6 Deliberately out of scope for stage 2

* **Single-flag ablation** (comments-only vs docstrings-only vs
  annotations-only). Six variants instead of three would roughly double the
  generation and judging bill. It is specified as an optional stage 4
  (§8.4) to be run only if C2 is significant and the mechanism is unclear
  from the `answer_locus` split.
* **Whole-file and whole-repo contexts.** Real TokenPress usage is
  "paste the repo into the prompt", where the savings are larger *and*
  attention dilution is a bigger factor. Single-window items are the
  affordable version; the generalization gap is listed as a threat (§9.5).

---

## 3. Task set

Three families. Each is defined by: how items are generated, the **verbatim**
prompt template, and what the gold answer is.

All prompts are sent as a `system` string plus a single `user` turn. The
system string is identical across variants — the model is never told which
variant it is looking at, and the word "TokenPress" never appears in any
prompt.

Shared system prompt (all families):

```
You are a precise software engineer answering questions about a single
excerpt of source code. Answer only from the excerpt provided. If the
excerpt does not contain enough information to answer, say exactly:
INSUFFICIENT CONTEXT
Do not speculate about code that is not shown. Do not mention formatting,
whitespace, or the presence or absence of comments.
```

The last line matters: without it, models spontaneously remark "this code has
no comments", which would leak the variant into the answer text and defeat
the judge blinding (§5.3).

### 3.1 Family A — function-purpose summarization

**Item generation.** Draw an eligible function per §2.3/§2.4. The gold
reference is built once, from the **original** source, by a separate
gold-writing pass (see below). The docstring / doc comment is *not* stripped
from V0 — that is the whole point of the contrast — but the rubric is written
so that copying the docstring is neither necessary nor sufficient to score
well (§5.2).

**Prompt (verbatim):**

```
Below is an excerpt of {LANGUAGE} source code from a real open-source
project. Lines are numbered.

<code>
{NUMBERED_CODE}
</code>

Question: What does the function `{QUALIFIED_NAME}` do?

Answer in 40-120 words of plain prose. Cover, to the extent the excerpt
supports it:
1. its purpose in one sentence,
2. what its inputs are and how they affect behaviour,
3. what it returns or what side effects it has,
4. any error or failure condition it handles or raises.

Do not restate the code line by line. Do not use bullet points or headings.
```

**Gold answer.** A 60–150 word reference description, produced once per item
in a **separate gold-writing pass** that sees the original source *including*
the docstring, plus (where available) the function's call sites. The
gold-writer is `claude-opus-5` at `effort: "high"`, run once, and every gold
answer is **read and, where necessary, corrected by the maintainer before any
scoring run** — golds are frozen and hashed into the manifest before a single
candidate answer is graded. A gold answer is never regenerated after seeing
candidate answers.

**Metric.** Rubric-based LLM judge (§5.2), three dimensions × 0–3, normalized
to [0, 1].

### 3.2 Family B — bug localization (deterministic mutations)

**Why mutations and not real historical bugs.** Real bugs from the corpora's
git history were considered and rejected for three concrete reasons:

1. `benchmarks/fetch.sh` clones at `--depth 1` (and fetches the pinned SHA
   with `--depth 1` for the "known" list). There is **no git history in the
   pinned corpus**, so history-mined items would require changing `fetch.sh`
   — out of scope for this task and a change to a file other benchmarks
   depend on.
2. Real bugs in famous projects come with published fix commits, issue
   threads and CVE writeups, all of which are very likely in the models'
   training data. Mutation-injected bugs cannot be, because the mutant did
   not exist before this experiment generated it. This is the single
   strongest contamination defence in the design (§9.1).
3. A historical bug's "location" is often a design-level answer spread over
   several files; a mutation has one unambiguous token.

**Mutation operators.** Applied to the **original** source of the window;
the resulting mutant is then formatted into V1 and V2, so all three variants
contain the *same* bug.

| ID | Operator | Python | Rust |
|---|---|---|---|
| `CMP` | comparison boundary flip | `<`↔`<=`, `>`↔`>=` | same |
| `OFF` | off-by-one on an integer literal in an index or slice | `x[i + 1]` → `x[i]`, `n - 1` → `n` | same |
| `BOOL` | negate a branch condition | wrap in `not (...)` | wrap in `!(...)` |
| `LOGIC` | logical connective swap | `and`↔`or` | `&&`↔`\|\|` |
| `ARITH` | arithmetic operator swap | `+`↔`-` (not on string operands) | same (integer operands only) |
| `CONST` | numeric literal perturbation | `n` → `n ± 1` | same |
| `LIT` | boolean literal flip | `True`↔`False` | `true`↔`false` |

Operators are chosen to (a) keep the mutant parseable, (b) keep Rust
type-correct (`ARITH` and `CONST` restricted to integer contexts, no
`Ok`/`Err` or `Some`/`None` swaps which would break typing), and (c) produce
a bug that is *findable from the code alone* — no operator depends on prose.

**Determinism.** For an item with candidate sites `S` (enumerated in AST
traversal order, then byte offset):

```
h    = SHA256(MASTER_SEED || corpus_sha || path || qualified_name || "mutation")
op   = OPERATORS[ int(h[0:4], 16) % len(applicable_operators) ]
site = sites_for(op)[ int(h[4:8], 16) % len(sites_for(op)) ]
```

Exactly one mutation per item. The chosen site must have a **unique 7-token
context window** inside the excerpt (checked with the same lexer TokenPress
uses), so the mutated construct can be re-located unambiguously in the
formatted variants — that is how the per-variant gold line is computed. Sites
without a unique context are skipped in favour of the next candidate.

**Gold answer.** `(file, qualified_name, mutated_token, gold_line[variant])`.
`gold_line` is computed per variant by re-locating the unique token context
in the formatted output.

**Prompt (verbatim):**

```
Below is an excerpt of {LANGUAGE} source code from a real open-source
project. Lines are numbered. Exactly one line of this excerpt contains a
single-token defect that was introduced deliberately: an operator, a
comparison, a boolean, or a numeric literal has been changed to the wrong
one. Everything else is correct.

<code>
{NUMBERED_CODE}
</code>

Find the defect. Reply with exactly three lines and nothing else:

LINE: <the line number containing the defect>
FUNCTION: <the name of the function containing the defect>
SNIPPET: <the defective expression, copied verbatim from that line>
```

**Metric (primary).** Binary correct/incorrect. Correct iff
`FUNCTION` matches the gold qualified name (last path segment, case-sensitive,
whitespace-normalized) **and** `SNIPPET` contains the mutated token in its
mutated form.

**Metric (secondary).** Line accuracy: `|reported_line − gold_line| ≤ 1`.
Reported separately and *not* pooled with the primary, because it is not
comparable across variants: aggressive Rust output packs many statements onto
one line, so a ±1 line window is mechanically more generous for V2 than for
V0. The primary metric is deliberately line-free for exactly this reason.
Both are reported; the asymmetry is stated wherever the secondary number
appears.

**Response parsing.** Strict: a response that does not match the three-line
shape is scored incorrect and counted separately as a `format_violation` so
that a format-following difference between variants cannot masquerade as an
accuracy difference.

### 3.3 Family C — API-usage question answering

**Item generation.** For a public target (§2.3), the generator mechanically
extracts candidate ground-truth facts and emits one question per item:

| Fact type | Extracted from | Example question |
|---|---|---|
| `default_value` | AST default of a keyword parameter | "What is the default value of the `timeout` parameter of `Session.request`?" |
| `param_role` | parameter name + its use in the body | "Which parameter of `Session.request` controls whether redirects are followed?" |
| `raises` | `raise` statements / `Err(...)` returns / `#[doc]`-stated errors | "Which exception does `HTTPAdapter.send` raise when the connection times out?" |
| `return_shape` | `return` statements / return type | "What does `Session.resolve_redirects` return?" |
| `usage` (open form) | signature + body | "How would you call `Session.request` to issue a POST with a JSON body and a 5-second timeout? Answer in one short code snippet." |

The first four are **closed-form** (auto-gradeable). The fifth is
**open-form** (judged). Split: 40 closed-form + 40 open-form per family
(10 + 10 per corpus).

**`answer_locus` labelling.** Every item is labelled by the generator:

* `code` — the gold fact is recoverable from the AST alone (a literal
  default, a `raise` in the body, a return statement).
* `prose` — the gold fact appears *only* in the docstring / doc comment (a
  documented default of a `**kwargs` key, a documented exception raised by a
  callee, a documented invariant).
* `both` — stated in both places.

This label is the mechanistic covariate for H− (§1.3). The generator's rule
is: a fact is `code` iff the extractor found it without reading any
docstring/comment node; `prose` iff a docstring-only extractor found it and
the AST extractor did not; `both` iff both did. **30 labels are hand-checked
by the maintainer before the run**; if hand-check agreement is < 90%, the
labelling rule is fixed and re-run before any scoring.

**Prompt (verbatim), closed-form:**

```
Below is an excerpt of {LANGUAGE} source code from a real open-source
project. Lines are numbered.

<code>
{NUMBERED_CODE}
</code>

Question: {QUESTION}

Reply with exactly one line and nothing else:

ANSWER: <your answer, as short as possible - a single identifier, literal,
type name, or short phrase>
```

**Prompt (verbatim), open-form:**

```
Below is an excerpt of {LANGUAGE} source code from a real open-source
project. Lines are numbered.

<code>
{NUMBERED_CODE}
</code>

Question: {QUESTION}

Answer with a short code snippet (at most 10 lines) followed by at most two
sentences of explanation. Use only names that appear in the excerpt.
```

**Metric.** Closed-form: normalized exact match (lowercase, strip quotes,
strip trailing punctuation, unify `None`/`null`, unify numeric formats;
the normalizer is fixed in code before the run and unit-tested against a
hand-written table of 30 accept/reject cases). Open-form: rubric judge
(§5.2) on the same three dimensions.

---

## 4. Models under test

### 4.1 The tokenizer/quality distinction — read this before running anything

The project's standing prohibitions (`ROADMAP.md`) include:

> Do not extrapolate or publish savings numbers for private/closed
> tokenizers (e.g. Claude).

**That prohibition is about token counts, not about answer quality.** It
exists because a savings percentage for a private vocabulary cannot be
verified by anyone outside the vendor. Measuring *how well a model answers a
question* requires no knowledge of its tokenizer at all, and the resulting
number is reproducible by anyone with an API key. So:

* **Allowed and done here:** run any model, publish its answer-quality scores
  per variant.
* **Still forbidden, and not done here:** publish or derive any
  tokens-saved / percentage-reduction figure for a private tokenizer.

Concretely, this experiment's cost model (§7) uses `o200k_base` counts as a
*billing proxy for internal budgeting only*. Those proxy numbers must never
appear in `RESULTS.md`, `SHOWCASE.md`, `README.md` or `promo/` as savings
figures for any model measured here, and the results file must carry that
sentence verbatim. Every published savings number stays sourced from the
existing embedded public tokenizers.

### 4.2 Models

| Role | Model ID | Tier | Settings |
|---|---|---|---|
| Answering (frontier) | `claude-opus-5` | Frontier ($5 / $25 per MTok) | `thinking: {"type": "adaptive"}`, `output_config: {"effort": "low"}`, `max_tokens: 2048`. **No `temperature` / `top_p` / `top_k`** — Claude Opus 5 rejects sampling parameters with a 400. |
| Answering (small/cheap) | `claude-haiku-4-5` | Small ($1 / $5 per MTok) | `temperature: 0`, `max_tokens: 1024`, thinking not enabled. |
| Judging | `claude-opus-5` | Frontier | `output_config: {"effort": "medium"}`, structured output via `output_config.format` JSON schema, `max_tokens: 2048`. |
| Gold-writing (family A) | `claude-opus-5` | Frontier | `effort: "high"`, output reviewed and corrected by the maintainer. |

Rationale for the pair: the hypothesis that stripped context hurts *more* for
weaker models is plausible and cheap to test with this pairing — a frontier
model may reconstruct a missing docstring's content from the body where a
small model cannot. Model is a factor in the analysis (§6.2), not a nuisance.

`claude-sonnet-5` is named as an **optional third arm** (stage 4, §8.4) if
the Opus/Haiku gap turns out to be large and a mid-tier point is wanted;
adding it costs roughly +60% of the generation budget and nothing of the
judging budget.

**Sampling / determinism, stated honestly.** `temperature: 0` is *not*
available on `claude-opus-5` (sampling parameters return a 400 on that
model), so answers from the frontier arm are not deterministic. The design
does not pretend otherwise:

* Each (item, variant, model) is generated **once**; generation noise is part
  of the measured within-item variance and is absorbed by the paired design.
* A **10% subsample (24 items × 3 variants × 2 models = 144 calls)** is
  re-generated in a second, independently seeded pass to estimate
  generation-level variance directly. If the re-generation disagreement rate
  on bug localization exceeds 15%, the primary analysis switches to the
  pre-registered fallback of majority-of-3 generations on that family
  (budget impact in §7.3).
* Every call records the exact `response.model` string returned by the API
  (the resolved snapshot), the request date, and the SDK version.

---

## 5. Metrics and grading

### 5.1 Metric per family

| Family | Primary metric | Grader |
|---|---|---|
| A. Summarization | Rubric score, 3 dimensions × 0–3, normalized to [0,1] | LLM judge |
| B. Bug localization | Binary (FUNCTION + SNIPPET match) | Deterministic code |
| B. Bug localization (secondary) | Line accuracy, ±1 window | Deterministic code |
| C. API-QA, closed form (n=40) | Normalized exact match, binary | Deterministic code |
| C. API-QA, open form (n=40) | Rubric score, 3 dimensions × 0–3, normalized to [0,1] | LLM judge |

Two of the five metrics are exact-match and need no judge at all. That is
deliberate: the judge is the largest bias surface in the design, so as much
of the evidence as possible is moved off it.

### 5.2 Rubric

Three dimensions, each scored 0–3, applied identically to family A and
family C open-form:

* **Correctness (0–3).** Are the claims about the code's behaviour true of
  the code? 3 = every claim true; 2 = one minor inaccuracy that does not
  change what a reader would do; 1 = a materially wrong claim; 0 = the answer
  describes something else entirely, or is `INSUFFICIENT CONTEXT` when the
  gold shows the answer was derivable.
* **Coverage (0–3).** How much of the gold answer's substantive content is
  present? 3 = all key facts; 2 = most, one notable omission; 1 = only the
  headline; 0 = nothing substantive.
* **Groundedness (0–3).** Absence of fabrication. 3 = no claim that is not
  supported by the excerpt; 2 = one unsupported-but-harmless claim; 1 = an
  invented parameter, exception, or behaviour; 0 = largely invented.

Normalized score = (correctness + coverage + groundedness) / 9.

### 5.3 Bias controls

1. **Judge never sees the variant.** The judge prompt shows the **original
   (V0) source** as reference for *every* candidate answer, regardless of
   which variant produced it. The variant is therefore not inferable from the
   context at all, and the judge is comparing every answer against the same
   reference. This is the primary blinding mechanism.
2. **No variant labels anywhere.** Candidate answers are passed with an
   opaque id (`ans_<8 hex>`); the mapping id→variant lives only in the
   harness manifest and is not visible to the judge process.
3. **Independent absolute grading, not side-by-side.** Each answer is graded
   in its own API call. This removes order effects from the primary metric
   entirely (there is no order).
4. **Randomized presentation order in the secondary comparative pass.** A
   25% subsample (30 items × 2 models) additionally gets a 3-way blinded
   ranking where the three answers are shuffled by a per-item seed
   `SHA256(MASTER_SEED || item_id || "rank")`; each of the six orderings is
   equally likely across the subsample. If ranking and absolute grading
   disagree in *direction*, both are reported and no directional claim is
   made.
5. **Same judge model, same settings, for every variant.** Any change to the
   judge model or prompt invalidates the whole grading pass and requires a
   full re-grade — partial re-grades are forbidden by this pre-registration.
6. **Length control.** The judge is instructed to ignore length and style
   (see prompt). In addition, the analysis reports the score deltas both raw
   and adjusted for answer length (score regressed on log answer length, per
   family; the adjusted delta is reported alongside the raw one). If the
   length adjustment changes the sign of a primary contrast, that contrast is
   reported as inconclusive.
7. **Self-consistency check.** A 20% subsample (144 answers) is re-graded in
   an independently seeded second pass; exact-agreement and quadratic-weighted
   κ between the two judge passes are reported.

### 5.4 Judge prompt (verbatim)

System:

```
You are grading a short answer written by another AI system about a piece of
source code. You are given the reference source code, a gold-standard
reference answer, and one candidate answer. Grade the candidate answer
against the source code and the gold answer.

Grade only the substance. Explicitly ignore: answer length, writing style,
formatting, tone, hedging, whether the candidate used the same wording as the
gold answer, and whether the candidate quoted the code. A short answer that
is correct and complete scores exactly as high as a long one.

You must not speculate about how the candidate answer was produced or what
information the candidate was given. Grade only what is written.
```

User:

```
<reference_code>
{ORIGINAL_NUMBERED_CODE}
</reference_code>

<question>
{QUESTION}
</question>

<gold_answer>
{GOLD_ANSWER}
</gold_answer>

<candidate_answer id="{OPAQUE_ID}">
{CANDIDATE_ANSWER}
</candidate_answer>

Score the candidate answer on three dimensions, each an integer 0-3.

correctness - Are the claims the candidate makes about the code's behaviour
true of the reference code?
  3 = every claim is true
  2 = one minor inaccuracy that would not change a reader's actions
  1 = at least one materially wrong claim
  0 = describes different code, or claims the context was insufficient when
      the gold answer shows it was not

coverage - How much of the gold answer's substantive content does the
candidate contain?
  3 = all key facts present
  2 = most present, one notable omission
  1 = only the headline point
  0 = nothing substantive

groundedness - Absence of fabrication.
  3 = no claim unsupported by the reference code
  2 = one unsupported but harmless claim
  1 = invents a parameter, exception, return value or behaviour
  0 = largely invented

Also give one sentence of justification, quoting the specific candidate claim
that drove the lowest of your three scores.
```

Judge structured output schema (enforced with `output_config.format`):

```json
{
  "type": "object",
  "properties": {
    "correctness":  {"type": "integer", "enum": [0, 1, 2, 3]},
    "coverage":     {"type": "integer", "enum": [0, 1, 2, 3]},
    "groundedness": {"type": "integer", "enum": [0, 1, 2, 3]},
    "justification": {"type": "string"}
  },
  "required": ["correctness", "coverage", "groundedness", "justification"],
  "additionalProperties": false
}
```

### 5.5 Human inter-rater spot check

**Sample.** 60 candidate answers: 30 from family A and 30 from family C
open-form, stratified so that each cell (family × variant × model) is
represented and drawn by seed `SHA256(MASTER_SEED || "human-subsample")`.

**Procedure.** The maintainer grades them with the *same* rubric, from the
*same* materials the judge saw (reference code, gold, candidate, opaque id),
in randomized order, without access to the judge's scores or the variant
mapping. Grades are committed before the mapping is revealed.

**Statistics reported.** Quadratic-weighted Cohen's κ per dimension and on
the normalized total, Krippendorff's α (ordinal), Spearman ρ, and mean signed
difference (judge − human) to detect systematic leniency.

**Pre-registered gate.**

| Agreement (α on normalized total) | Consequence |
|---|---|
| α ≥ 0.60 | Judge-based metrics reported as primary evidence. |
| 0.40 ≤ α < 0.60 | Judge-based metrics reported with an explicit reliability caveat, and their confidence intervals widened by the disattenuation factor `1/√α`; the exact-match families become the headline evidence. |
| α < 0.40 | **Judge-based metrics are not reported as findings at all.** Only bug localization and closed-form API-QA are published; family A and open-form C are reported as "judge unreliable at α = …, no conclusion". |

The gate is evaluated on the **pilot** first (§8.1), so a broken judge is
caught before the full run is paid for.

---

## 6. Statistical analysis

### 6.1 Pre-registered primary tests

Unit of analysis: the per-item paired difference. For each
(task family × model × contrast) cell:

* **Continuous metrics (A, C-open):** Wilcoxon signed-rank test on the paired
  deltas (two-sided), plus a BCa bootstrap 95% CI on the **mean** paired
  delta (10,000 resamples, `numpy` `default_rng(20260801)`), plus the
  matched-pairs rank-biserial correlation as the effect size.
* **Binary metrics (B, C-closed):** exact McNemar test on discordant pairs,
  plus a bootstrap 95% CI on the paired accuracy difference.

Primary cells: 3 families × 2 models × 2 contrasts = **12 tests**
(bug localization's primary cell is the model-pooled one per §2.5, which
keeps the count at 12 by replacing its two per-model cells with two pooled
cells; the per-model cells move to the secondary set).

### 6.2 Secondary and exploratory analyses

* **Pooled mixed-effects model** (secondary, not primary):
  `score ~ variant + family + model + variant:model + (1 | item) + (1 | corpus)`,
  fitted per metric type. Reports a single pooled variant effect with its CI.
  Used for the headline sentence if and only if the per-cell results are
  directionally consistent.
* **`answer_locus` split** on family C (§3.4): C2 estimated separately on
  `code` vs `prose` items. This is the mechanism test and is pre-registered
  as *confirmatory* for the moderator hypothesis, not exploratory.
* **Dose–response.** Per-item `saving_ratio` (from `tokenpress stats --json`)
  regressed against the per-item quality delta. If stripping hurts, items
  where more was stripped should be hurt more; a flat dose–response with a
  non-zero mean effect is evidence the effect is not really about the removed
  prose.
* **Per-corpus and per-language breakdowns**, the short-docstring control
  stratum of family A, format-violation rates, `INSUFFICIENT CONTEXT` rates,
  and the memorization-probe covariate (§9.1) are all **exploratory** and
  labelled as such everywhere they appear.

### 6.3 Multiple comparisons

* **Primary family (12 tests):** Holm–Bonferroni at family-wise α = 0.05.
  Both raw and Holm-adjusted p-values are reported for every test.
* **Confirmatory moderator tests (`answer_locus`, 4 tests):** a separate
  Holm family at α = 0.05.
* **All exploratory analyses:** Benjamini–Hochberg FDR at q = 0.10, and every
  such result is printed under an "Exploratory" heading with the sentence
  "not corrected for the full analysis space".
* CIs are reported for everything, significant or not. No result is described
  in words without its interval.

### 6.4 Equivalence testing and the pre-registered decision rule

Failing to reject H0 is not evidence of no effect, so equivalence is tested
directly with **TOST** against a pre-registered margin:

| Metric | Equivalence margin Δ |
|---|---|
| Rubric score (0–1 normalized) | ±0.05 (≈ 0.45 rubric points out of 9) |
| Binary accuracy | ±0.05 (5 percentage points) |

The margin is chosen as "the smallest difference a user of TokenPress would
plausibly care about when deciding whether to pass a strip flag" and is fixed
here, before any data exists.

**What gets written where, decided in advance:**

| Outcome for a contrast | README / SHOWCASE wording |
|---|---|
| **Equivalent** (TOST passes both bounds, Holm-adjusted) | "Measured: no quality difference larger than ±5% on {families} for {models} (n = …, 95% CI …). See `benchmarks/quality-eval/RESULTS.md`." |
| **Harmful** (significantly negative after Holm) | The drop is stated with its number, family, model and CI, **in the same paragraph that states the token savings**, and the aggressive-flag table in SHOWCASE gains a "measured quality cost" column. |
| **Helpful** (significantly positive after Holm) | Published with the number *and* with the contamination caveat (§9.1) attached, because a positive result on famous corpora is the single most likely thing to be an artifact. It is not used as a marketing claim in `promo/` without a replication on an unfamiliar corpus. |
| **Inconclusive** (CI spans the margin, TOST fails, test not significant) | The "not yet measured" sentences are replaced with "measured at n = …; the result is inconclusive: 95% CI on the quality delta is […, …], which does not exclude a difference of ±5%." The honest wording is *measured and inconclusive*, never a silent revert to "not measured". |

**Honest-negative commitments.** These are the parts that make the
pre-registration worth anything:

1. A result that makes TokenPress look bad is published with the same
   prominence, in the same files, and in the same commit as one that makes it
   look good. There is no "hold it back and re-run" branch in this plan.
2. **No re-runs after seeing results**, except the ones already specified
   here (pilot gate, judge-reliability gate, majority-of-3 fallback). Any
   additional run is a new experiment with its own pre-registration and is
   reported as such, alongside the original.
3. Analyses not listed in §6.1–§6.3 are exploratory by definition and are
   labelled that way, no matter how good they look.
4. The frozen artifacts (`items/`, golds, prompts, seeds) are hashed into
   `manifest.json` before the scoring run; the results file records the git
   SHA of *this* file so any drift between plan and analysis is visible.
5. If the experiment is abandoned part-way, whatever was collected is
   published with an explicit "partial run, n = …, do not treat as
   conclusive" header rather than deleted.

---

## 7. Token and cost budget

### 7.1 Assumptions (all of them, stated so the estimate can be checked)

1. Mean presented context, V0: **2,400 `o200k_base` tokens** (window band
   400–3,500, §2.3). V1 ≈ −15%, V2 ≈ −38%, from the measured corpus deltas
   in `RESULTS.md` (default −9.0% to −22.2%; aggressive −36.0% to −50.5%).
   Mean input across the three variants ≈ 1,977 tokens.
2. Prompt scaffolding (system + question + instructions): **350 tokens**.
   Mean request input ≈ **2,330 tokens**.
3. Output budget: **500 billed output tokens** for `claude-opus-5`
   (adaptive thinking at `effort: "low"` — thinking tokens are billed as
   output), **250** for `claude-haiku-4-5`, **600** for the judge
   (`effort: "medium"`).
4. Judge input ≈ **3,250 tokens** (original code 2,400 + rubric/instructions
   450 + candidate 250 + gold 150).
5. **No prompt-cache discount is assumed.** The shared prefix is the ~350-token
   system prompt, which is below Claude Opus 5's 512-token minimum cacheable
   prefix, and every item's code block is unique. If caching does help, the
   run comes in under budget.
6. Public list prices at time of writing: `claude-opus-5` $5.00 / $25.00 per
   MTok; `claude-haiku-4-5` $1.00 / $5.00 per MTok. **Prices are re-checked
   against the pricing page at run time and the estimate re-derived before
   the maintainer is asked to approve spend.**
7. `o200k_base` is used as the token-count proxy for billing. It is not the
   models' tokenizer; the real bill may differ by roughly ±35%, which is
   inside the contingency factor in §7.3. Per §4.1, these proxy counts are a
   budgeting device and must not be published as savings figures.

### 7.2 Per-call and per-stage cost

| Call type | Input tok | Output tok | $/call |
|---|---:|---:|---:|
| Generation, `claude-opus-5` | 2,330 | 500 | $0.0242 |
| Generation, `claude-haiku-4-5` | 2,330 | 250 | $0.0036 |
| Judge, `claude-opus-5` | 3,250 | 600 | $0.0313 |
| Memorization probe, `claude-opus-5` | 2,330 | 100 | $0.0142 |
| 3-way ranking, `claude-opus-5` | 3,550 | 400 | $0.0278 |

**Full run (stage 2), n = 80 per family:**

| Stage component | Calls | Cost |
|---|---:|---:|
| Generations, Opus (240 items × 3 variants) | 720 | $17.42 |
| Generations, Haiku (240 items × 3 variants) | 720 | $2.59 |
| Judging (120 judged items × 3 variants × 2 models) | 720 | $22.54 |
| Judge self-consistency re-grade (20%) | 144 | $4.51 |
| 3-way blinded ranking (25% subsample × 2 models) | 60 | $1.67 |
| Memorization probe (240 items, V2, Opus) | 240 | $3.41 |
| Generation-variance re-run (10% subsample) | 144 | ~$2.00 |
| Gold-writing pass, family A (80 items, one-off) | 80 | $2.40 |
| **Full-run subtotal** | **2,828** | **$56.54** |

**Pilot (stage 1), n = 8 per family = 24 items:**

| Stage component | Calls | Cost |
|---|---:|---:|
| Generations, both models (24 × 3 × 2) | 144 | $2.00 |
| Judging (12 judged items × 3 × 2) | 72 | $2.25 |
| Gold-writing (8 items) | 8 | $0.24 |
| **Pilot subtotal** | **224** | **$4.49** |

**Grand total ≈ $61.** With a 1.5× contingency for retries, rate-limit
backoff waste, one round of prompt iteration after the pilot, and the
`o200k`-proxy error: **request a hard cap of $100** (pilot ≤ $10, full run
≤ $90). The harness enforces the cap itself (§8.3).

### 7.3 Budget sensitivity

| Change | Cost delta |
|---|---|
| n = 120 per family instead of 80 (if pilot shows SD_Δ > 0.25) | +50% of the full run ≈ +$28 |
| Majority-of-3 generation on family B (if generation noise > 15%) | +$4 |
| Adding `claude-sonnet-5` as a third answering arm | +$11 (generation) +$22 (judging) |
| Stage 4 single-flag ablation (6 variants instead of 3) | roughly doubles stage 2 ≈ +$57 |

Any of these that would push the total over the approved cap requires a fresh
approval; the harness stops rather than silently exceeding it.

---

## 8. Execution plan

### 8.1 Stage 1 — pilot (n = 8 per family, 24 items, ≈ $5)

Purpose is harness validation and gate checking, **not** evidence. Pilot data
are never pooled into the stage-2 analysis and are never used to pick a
direction; they are used only to re-estimate variance and to check the gates.

Checks and their pre-registered pass criteria:

| Check | Pass criterion | If it fails |
|---|---|---|
| End-to-end harness | 24 items generated, all 3 variants format with 0 refusals, all 144 calls parse | Fix harness; re-run pilot. |
| Judge ↔ human agreement (§5.5, all 72 judged pilot answers graded by hand) | Krippendorff α ≥ 0.60 | Revise rubric/judge prompt **once**, re-run pilot. Second failure → judge-based families are dropped from the design and only exact-match evidence is collected. |
| Cost calibration | Actual $/item within 1.5× of §7.2 estimate | Re-derive budget, seek fresh approval before stage 2. |
| Format-violation rate | < 10% per (family × variant) | Tighten output instructions or move to structured output; note the change here before stage 2. |
| Refusal / `INSUFFICIENT CONTEXT` rate | < 15% and not differing by more than 10pp across variants | Investigate; a variant-dependent refusal rate is itself a finding and gets reported. |
| SD of paired deltas | Recorded; MDE table (§2.5) recomputed and written into `RESULTS.md` | If SD_Δ > 0.25, raise n or re-declare the pooled cell as primary (§2.5). |

### 8.2 Stage 2 — full run (n = 80 per family, ≈ $57)

Order of operations, each step producing a frozen artifact before the next
begins:

1. `./benchmarks/fetch.sh` — corpus at the pinned SHAs; assert each SHA.
2. `cargo build --release -p tokenpress-cli`; record `git rev-parse HEAD` and
   the binary's `--version`.
3. **Item generation** → `items/items.jsonl`, `items/mutations.jsonl`,
   `items/dropped.jsonl`. Hash and freeze.
4. **Gold generation** (family A) + maintainer correction → `items/golds.jsonl`.
   Hash and freeze. *No API answer has been generated at this point.*
5. **Answer generation** → `raw/answers.jsonl` (append-only, one JSON object
   per call, including the full request parameters, `response.model`,
   `response.usage`, latency, and any retry history).
6. **Deterministic grading** (families B and C-closed) → `grades/exact.jsonl`.
7. **Judging** → `grades/judge.jsonl`; then the self-consistency re-grade and
   the 3-way ranking pass.
8. **Human spot-check** (§5.5) → `grades/human.jsonl`, committed before the
   variant mapping is revealed.
9. **Analysis** → `summary.json` + `RESULTS.md`.
10. **Publication** per the §6.4 decision table: `RESULTS.md` first, then the
    one-sentence replacements in `README.md`, `benchmarks/SHOWCASE.md`, and
    `benchmarks/RESULTS.md`.

### 8.3 Artifacts, layout, and what is committed

```
benchmarks/quality-eval/
  DESIGN.md            <- this file (committed; its SHA is recorded in RESULTS.md)
  RESULTS.md           <- committed: methodology recap, all tables, all CIs
  summary.json         <- committed: every aggregate number the prose cites
  manifest.json        <- committed: corpus SHAs, tokenpress SHA + version,
                          model IDs + resolved snapshots, SDK version, seeds,
                          prompt-template SHA256s, price table used, run dates
  harness/             <- committed: generate_items.py, run_eval.py, grade.py,
                          analyze.py, prompts/ (the verbatim templates)
  items/               <- committed: items.jsonl, mutations.jsonl, golds.jsonl,
                          dropped.jsonl  (POINTERS ONLY - see below)
  raw/                 <- gitignored: answers.jsonl (model outputs)
  grades/              <- gitignored: judge.jsonl, exact.jsonl, human.jsonl
  work/                <- gitignored: formatted variants, mutants
```

**No third-party source code is committed.** `items/*.jsonl` stores
`{corpus, corpus_sha, path, byte_start, byte_end, qualified_name,
content_sha256, variant_token_counts, ...}` — pointers and hashes, never the
excerpt text. This keeps the standing prohibition ("Do not commit
`benchmarks/corpus` downloads") intact and avoids vendoring other projects'
licensed code. Anyone can regenerate the exact excerpts from the pinned SHAs
plus the pointers.

`raw/` and `grades/` are gitignored by default because they are large and
contain model output rather than analysis; their SHA256 sums are recorded in
`manifest.json`, and the maintainer may choose to attach them to a GitHub
release. That is a publication decision, not something this plan authorizes.

**Cost enforcement.** `run_eval.py` maintains a running spend estimate from
`response.usage` and the recorded price table, writes it to
`raw/spend.jsonl` after every call, and **hard-stops** at the approved cap.
It also supports `--dry-run` (counts tokens and prints the projected bill
without calling the API) and `--resume` (skips calls already present in
`raw/answers.jsonl`, keyed by `(item_id, variant, model, pass)`), so a
rate-limit failure never costs a re-run of completed work.

### 8.4 Optional later stages (each needs its own approval)

* **Stage 3 — whole-module contexts.** Same items, but the presented context
  is the whole file rather than a window, to test whether the effect grows
  with context size. Roughly 3× the input tokens.
* **Stage 4 — single-flag ablation and/or a third model.** Only if stage 2
  finds a significant C2 whose mechanism is not resolved by the
  `answer_locus` split.
* **Stage 5 — unfamiliar-corpus replication.** The strongest possible
  contamination control: repeat the design on a small, recent, low-star
  repository that is unlikely to be in training data. This is the study that
  would let a *positive* result be quoted without the contamination caveat.

### 8.5 Reproducibility checklist

Recorded in `manifest.json` for every run, and re-checked by `analyze.py`:

- Corpus SHAs, asserted at fetch time, matching `benchmarks/fetch.sh`.
- TokenPress git SHA, `tokenpress --version`, and the exact per-variant flag
  strings.
- Exact model IDs *and* the resolved `response.model` snapshot returned by the
  API on the first call of each pass, plus the run date.
- Anthropic SDK version and API `anthropic-version`.
- All sampling settings actually sent, plus the explicit note that
  `claude-opus-5` accepts no sampling parameters, so its generations are not
  deterministic (§4.2).
- `MASTER_SEED` and every derived seed (bootstrap RNG, shuffle seeds,
  subsample seeds).
- SHA256 of every prompt template file and of the rubric.
- The price table used for the budget, with the date it was checked.

### 8.6 Repo-hygiene constraints on the harness

* The harness is Python and lives under `benchmarks/quality-eval/harness/`,
  **not** under `crates/`. The coverage gate (`./scripts/coverage.sh`) is
  `cargo llvm-cov --workspace`, so it neither covers nor is affected by these
  scripts; the gate must still be run and green for the commit that adds
  them, as it is for this docs-only commit.
* No workspace `Cargo.toml` change, no new crate, no dependency added to any
  Rust crate.
* Everything committed is in English, per `CLAUDE.md`.
* Nothing in this plan publishes anything externally, touches crates.io, or
  changes repository visibility.

---

## 9. Threats to validity

### 9.1 Training-data contamination

All four corpora are famous open-source projects. Their source, docstrings,
issue threads and blog posts are very likely in every candidate model's
training data. A model may therefore "know" what `Session.request` does
without reading the excerpt at all — which would **compress the measured
difference between variants toward zero** and make TokenPress look safer than
it is.

Mitigations, in decreasing strength:

1. **Bug localization is contamination-resistant by construction.** The
   mutant did not exist before this experiment created it, so no model can
   have memorized its answer. Family B is therefore the load-bearing family
   for any "aggressive stripping is safe" claim, and this is stated wherever
   such a claim appears.
2. **Memorization probe.** For every item, one extra call asks the model,
   given the **V2 (aggressive)** variant only:

   ```
   Below is an excerpt of source code with comments and documentation
   removed.

   <code>
   {NUMBERED_CODE}
   </code>

   Do you recognise which open-source project this code is from? Reply with
   exactly one line:

   PROJECT: <the project name, or UNKNOWN>
   ```

   The per-item recognition flag becomes a covariate; the primary contrasts
   are re-estimated on the recognized and unrecognized strata separately as a
   pre-registered secondary analysis. If effects are systematically smaller
   on recognized items, the reported effects are labelled as **lower bounds**.
3. **Answers are graded against gold facts extracted from the pinned source**,
   not against the model's general knowledge — a memorized-but-outdated
   answer scores as wrong.
4. **Stage 5** (unfamiliar-corpus replication) is the real fix and is named
   as the prerequisite for quoting a *positive* result without a caveat.

Residual risk after mitigation: **high** for families A and C, **low** for
family B. This is stated in `RESULTS.md` and in any sentence that cites A or C.

### 9.2 Judge bias, including bias toward verbose contexts

An LLM judge can favour longer, more confident, more docstring-flavoured
answers — exactly the answers the V0 (original) variant is most likely to
produce. Controls: variant-blind judging with a constant V0 reference (§5.3.1),
opaque answer ids, independent absolute grading, explicit "ignore length and
style" instruction, a length-adjusted re-analysis that can veto a directional
claim, judge self-consistency measurement, and the human-agreement gate that
can disqualify judge-based metrics entirely. Two of the five metrics avoid the
judge altogether.

Residual risk: **moderate**. The design cannot rule out a bias shared between
the judge and the answering model (both are Claude Opus 5 in one arm); a
mitigation would be a second judge from a different vendor, which is
deliberately not in scope here — it is noted as a known gap.

### 9.3 Summarization's structural advantage for V0

In family A, the V0 excerpt contains the docstring, and the gold answer was
written with the docstring in view. V0 can effectively copy; V2 must
reconstruct. This makes a V0 > V2 result on family A **partly tautological**.
It is kept in the design because the *magnitude* is the interesting quantity
("how much of the docstring's content survives in the model's reading of the
code alone"), but three things are done about it: the rubric scores
functional correctness rather than paraphrase similarity; a verbatim-copy
detector flags answers whose longest common substring with the docstring
exceeds 20 words, reported per variant; and the short-docstring control
stratum provides a within-family comparison where the advantage is small.
Family A's result is never quoted as the headline.

### 9.4 Small-n and multiplicity

Twelve primary tests at n = 80 detect dz ≈ 0.32 and accuracy differences of
≈ 15pp per cell. Small true effects will be missed, and this is stated
numerically wherever a null is reported (a null is always reported *with* its
MDE, never as "no effect"). Holm across the primary family and BH across the
exploratory set control the error rate, but the exploratory set is large and
its findings are explicitly not conclusions.

### 9.5 Generalization

The result is: *these four repositories, these three task families, these two
models, these window sizes, this month's model snapshots.* It is not "LLM
quality under compression". Specific known gaps:

* Single-function windows, not whole repositories — the realistic use case
  (a whole tree in the context window) has both a larger dose and a
  larger attention-dilution effect; direction of the difference is unknown.
* Two languages, both statically parseable by TokenPress's current backends.
* One vendor's models. A model family with different pretraining data or
  different comment-sensitivity could behave differently.
* Snapshot drift: model behaviour changes over time; the recorded snapshot ids
  are what make the result checkable, not what make it permanent.

### 9.6 Other threats, briefly

* **Prompt sensitivity.** One template per family. Mitigation: a 10%
  paraphrase-robustness subsample re-run with a second, semantically
  equivalent template; if the contrast direction flips, the family's result
  is downgraded to inconclusive.
* **Line-number asymmetry** in bug localization (§3.2) — handled by making the
  primary metric line-free.
* **Format-violation confound.** If one variant makes models break the output
  format more often, that inflates its error rate for a reason unrelated to
  comprehension. Violation rates are reported per variant, and if they differ
  by more than 10pp the affected contrast is reported both raw and with
  violations excluded.
* **Generation non-determinism** on `claude-opus-5` (§4.2) — measured, not
  assumed away.
* **Item-generator bugs** are the single most dangerous silent failure mode
  (a wrong gold answer looks exactly like a model error). Mitigations: the
  30-item hand-check of `answer_locus` labels, maintainer review of all 80
  family-A golds, unit tests on the exact-match normalizer, and manual
  inspection of 10 mutants per language before stage 2.

---

## 10. What this unblocks

The results replace the following text, which exists in three places today.

**1. `README.md` (currently lines 80–85), verbatim:**

> **Stripped prose is context the model no longer sees.** Comments, docstrings
> and annotations are information an LLM could have used to answer questions
> about the code, and every strip flag deletes some of it. Whether — and how
> much — that degrades the quality of a model's answers has **not been measured
> yet**. Until it is, treat the aggressive flags as a cost/quality trade-off you
> are choosing, not as free savings.

Replacement shape (numbers filled from `summary.json`; wording chosen by the
§6.4 decision table):

> **Stripped prose is context the model no longer sees.** Comments, docstrings
> and annotations are information an LLM could have used to answer questions
> about the code, and every strip flag deletes some of it. This has now been
> measured: on {N} code-understanding items across {corpora}, the aggressive
> flags changed answer quality by {Δ} ({95% CI}) for {model} — see
> [benchmarks/quality-eval/RESULTS.md](benchmarks/quality-eval/RESULTS.md) for
> the design, the per-task breakdown, and the limits of the claim.

**2. `benchmarks/SHOWCASE.md` (currently lines 159–164), verbatim:**

> **Token savings are not free context.** Every comment and docstring stripped
> above is prose the model can no longer read. Whether that degrades the
> quality of a model's answers on real code tasks — and by how much — is
> **unmeasured**; no experiment on this project has tested it. The numbers on
> this page quantify the cost saving only. They say nothing about the quality
> trade-off on the other side of it.

Replacement shape:

> **Token savings are not free context.** Every comment and docstring stripped
> above is prose the model can no longer read. The quality side of that
> trade-off has been measured on a {N}-item, three-variant paired experiment
> across {corpora} and {models}: {one-sentence result with CI}. The
> methodology, the pre-registered analysis plan, and the contamination and
> generalization caveats are in
> [quality-eval/RESULTS.md](quality-eval/RESULTS.md). The numbers on this page
> still quantify the cost saving only.

**3. `benchmarks/RESULTS.md`** gains a new top-level section, "Downstream
quality evaluation", linking to `quality-eval/RESULTS.md` and carrying the
headline table. Its existing "Scope and caveats" bullet —

> * These runs verify **default settings only**. The aggressive settings above
>   (`--py-strip-comments`, `--py-strip-annotations`, `--rs-strip-doc-comments`)
>   are knowingly lossy and are not covered by this harness — stripping doc
>   comments would delete the doc tests being compared.

— stays exactly as it is: it is about *behavioral* verification, which this
experiment does not touch. The new section is additive.

**4. `ROADMAP.md`** P3's "Downstream quality evaluation" item is checked off
by the orchestrator once the run completes, with the result summarized in the
same honest form as every other completed item — including if the result is
unfavourable or inconclusive.

**Not unblocked by this experiment:** any claim about whole-repository
prompts, about languages other than Python and Rust, about models other than
those listed in §4.2, or about any private tokenizer's savings (§4.1). None
of those may be inferred from these results.
