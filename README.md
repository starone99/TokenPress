# TokenPress

> A token-aware formatter for Python and Rust that minimizes LLM token usage while preserving behavior.

TokenPress is a token-aware source code formatter for LLMs. Unlike a minifier that
shrinks characters, TokenPress optimizes against an actual LLM tokenizer
(`o200k_base`, `cl100k_base`, ...) — the output is the equivalent program that
costs the fewest input tokens.

```text
minimize  tokenizer.encode(transformed_code)
s.t.      the transformed code parses, compiles, and behaves identically
```

## Status

Design phase.

## Planned usage

```bash
tokenpress format app.py
tokenpress format src/main.rs --tokenizer o200k_base
tokenpress check .        # CI: exit 1 if anything would change
tokenpress diff app.py
tokenpress stats .
```

```text
app.py        4,821 → 3,476 tokens  (-27.9%)
```

## Layout

Cargo workspace with a single distributed binary:

| Crate | Role |
|---|---|
| `tokenpress-core` | Formatter/Tokenizer traits, options, results, errors |
| `tokenpress-python` | Python parse → transform → emit → verify |
| `tokenpress-rust` | Rust parse → transform → emit → verify |
| `tokenpress-cli` | The `tokenpress` binary: discovery, language detection, commands |
