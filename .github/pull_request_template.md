## Summary

<!-- What changes, and *why*. Link any related issue. -->

## Checklist

See [CONTRIBUTING.md](../CONTRIBUTING.md) for the full rules.

- [ ] **TDD**: a failing test was written first for any new behavior; when
      behavior changed intentionally, the test changed first
- [ ] `cargo fmt` was **not** run — TokenPress formats this tree, and rustfmt
      would revert it (see CONTRIBUTING, "Code style")
- [ ] `cargo clippy --workspace --all-targets -- -D warnings -A clippy::possible_missing_else`
      is clean
- [ ] `cargo test --workspace` passes
- [ ] `./scripts/coverage.sh` (or `.\scripts\coverage.ps1`) passes at 100% line
      coverage
- [ ] No unreachable defensive code added to satisfy the coverage gate
- [ ] Everything committed — code comments, docs, commit messages, this
      description — is in **English**
