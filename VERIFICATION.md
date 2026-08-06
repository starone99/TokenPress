# Verification, and what is never touched

The project's core invariant is that output failing verification is never
written. This is what that check is, and what the formatter will not do to
your code even when it passes.


Identifiers, string/number literals, decorators/attributes, the token sequence
inside macro invocations, import order — anything that carries meaning for an
LLM or affects behavior. In Python, comments, docstrings and annotations are
kept by default and only removed by explicit opt-in — and every strip flag
loses information: `--py-strip-docstrings` removes the leading string literal
of a module, class or function body (other string expressions are untouched),
which empties `__doc__`.

Four documented exceptions — one that applies to every backend, two in Rust,
one in JavaScript/TypeScript. These are the scope limits on the "preserving
behavior" claim at the top of this page.

**Line numbers are never preserved, by any backend, at any settings.** Deleting
blank lines and re-flowing whitespace is the core of what TokenPress does, so
every line below a removal moves — in Python, Ruby, Go, Java and C# exactly as
in Rust and JS/TS. No flag turns this off. Code whose behavior depends on physical line
numbers can therefore change behavior after formatting: Ruby `__LINE__` and
`caller`, Rust `line!()` and `std::panic::Location`, Python `inspect` and
traceback line numbers, JavaScript `Error.stack`, Go `runtime.Caller` (a
`//line` directive is the one case that is protected, because it is a comment
the toolchain reads), Java stack-trace line numbers, C# stack-trace line
numbers and `CallerLineNumberAttribute`, and any test that asserts on a
traceback or a stack trace.

Format-time verification cannot detect this **by construction**: the canonical
forms the re-parse/equivalence check compares are location-independent, which
is what makes them usable as an equality stand-in at all, so a moved line is
the same token in the same position before and after. `--verify external` does
not help either — `tsc`, `ruby -c`, `gofmt -e`, `javac`'s parse gate and the
`csc` diagnostic comparison all stop before anything runs, and none of them
compares positions. The
layer that does catch it is running a corpus's own upstream test suite against
the formatted copy (`benchmarks/verify-upstream.sh`), and it has: on 2026-08-02
the rack v3.2.6 target came back **DIVERGED** on one test,
`Rack::Builder::parse_file` "sets `__LINE__` correctly" — TokenPress deletes the
blank line above the code in `test/builder/line.ru`, so `__LINE__` reads `2`
where the test asserts `3`. Reproduced byte-identically on repeat runs — not a
flake. That rewrite saved **zero tokens** (35 before, 35 after at
`o200k_base`: a blank line and a plain newline each cost one token). Full triage in
[benchmarks/RESULTS.md](benchmarks/RESULTS.md). The limitation is documented
rather than mitigated: if your code, your tests or your tooling depend on line
numbers, TokenPress output is not a drop-in replacement for the original —
keep it.

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

**Trailing and expression-position JS/TS comments are dropped.** Regardless of
`--js-strip-comments`, the JS/TS emitter keeps only leading statement-level
comments plus jsdoc, annotation (`#__PURE__`) and legal (`//!`, `/*!`,
`@license`, `@preserve`) comments. Everything else — a `// tail` after a
statement, a comment between arguments — is lost, and the verifier cannot see
it because its canonical form is comment-free. If a JS/TS file's inline
comments matter to you, keep the original.

