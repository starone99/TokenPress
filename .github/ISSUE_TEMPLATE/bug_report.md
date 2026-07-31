---
name: Bug report
about: Report incorrect output, a crash, or a verification failure
title: ''
labels: bug
---

<!--
Core invariant: output that fails verification (re-parse + equivalence) is
never written. If TokenPress reports a verification or parse failure for your
file, that is a bug in TokenPress worth reporting here — please do not work
around it by disabling or lowering verification.
-->

## Environment

- **tokenpress version** (`tokenpress --version`):
- **OS / version**:

## Command line

The exact command you ran:

```bash
tokenpress ...
```

- **Was `--verify` passed?** yes / no (default is `ast`)
- **If yes, which level?** `reparse` / `ast` / `external`

## Minimal input

The smallest source file that reproduces the issue (please reduce it as far as
you can, and say which language backend it hits — python / rust):

```python
# or ```rust
```

## Expected output

## Actual output

Include the full stdout/stderr, including any `warning:` or `error:` lines.

```text
```

## Additional context

Anything else relevant — non-default flags (`--py-strip-comments`,
`--py-strip-annotations`, `--py-no-merge-imports`,
`--rs-strip-doc-comments`), a non-default `--tokenizer`, or whether the input
came from a larger corpus.
