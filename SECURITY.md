# Security Policy

## Reporting a vulnerability

Report privately through GitHub's
[**Report a vulnerability**](https://github.com/starone99/TokenPress/security/advisories/new)
form. It opens a draft advisory only the maintainers can see.

Please do not open a public issue for a vulnerability. Everything else —
crashes, wrong output, refusals — belongs in a public issue, and is more
useful there.

Expect a first response within 7 days. If a report is confirmed, the fix and
the advisory are published together; if it is not, you get the reasoning
rather than silence. There is no bounty.

Useful in a report: the input file (or the smallest fragment that still shows
it), the exact command line, the version (`tokenpress --version`), and what
you expected instead.

## Supported versions

This project is pre-1.0 and has no long-term support branches. Fixes go to the
default branch and into the next release; older tags are not patched.

## Threat model

TokenPress is a command-line formatter. It reads source files, and with
`format` it **overwrites them in place**. That is the capability worth
thinking about, so this section says what it does and does not do with it.

**It writes only what it verified.** Output that fails re-parse and
input/output equivalence is never written — the file is left byte-identical
instead. This is the project's core invariant, it is not conditional on
flags, and a bug that breaks it is a security bug rather than a correctness
bug: `format` is normally run across a whole tree at once, and silent
corruption at that scale is not something a reviewer catches.

**It runs external programs, and only when asked.** `--verify external` hands
output to the language's own toolchain — `ruby -c`, `gofmt -e`, `javac`,
Roslyn `csc`, `tsc`/`node --check`. Those are resolved on `PATH` and executed
as subprocesses. On a machine where `PATH` is attacker-controlled, so is what
runs. Nothing is executed at any other verification level, and no formatted
code is ever executed.

**It does not use the network, and never has.** No telemetry, no update check,
no remote tokenizer fetch. `--tokenizer hf:…` and `kimi:…` read a file you
already downloaded. The benchmark harness in `benchmarks/` does download
corpora and tokenizer files, but it is a development tool and is not part of
the CLI or of any release archive.

**Untrusted input is in scope.** Formatting a hostile source file should
produce a refusal or correct output, never a crash that corrupts a file,
an escape from the paths given on the command line, or unbounded resource use
that a reasonable input would not explain. Reports in that shape are wanted.

## Release integrity

Release archives are built by the tagged run of
[`.github/workflows/release.yml`](.github/workflows/release.yml) and published
with a `SHA256SUMS` file. `install.sh` and `install.ps1` verify against it, and
a missing `SHA256SUMS` or a mismatch aborts the install rather than
downgrading to a warning. The pre-commit hook and the GitHub Action install
through `install.sh` and inherit that.

One gap, stated rather than papered over: `install.sh` computes the digest
with `sha256sum` or `shasum`, and on a host that has neither it warns on
stderr and installs anyway. `install.ps1` has no such path — `Get-FileHash` is
built in.

The archives are not signed. If you need provenance stronger than the
checksum, build from source: the tag is the only input, and `Cargo.lock` is
committed.
