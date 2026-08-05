//! TokenPress for Go — the Go-specific half of the tree-sitter backend.
//!
//! Pipeline: parse ([`config`] names the grammar,
//! [`tokenpress_treesitter::parser`] drives it) → whitespace-minimal re-emit
//! over the source bytes under the comment policy ([`policy`]) →
//! verification ([`tokenpress_treesitter::verify`]) → token accounting. The
//! path is only used to decide whether this backend claims the file
//! ([`paths`]); the grammar has no dialect, filepath or version selector, so
//! [`GoFormatter::format`] never reads it.
//!
//! # The crate split
//!
//! `tokenpress-treesitter` is the grammar-agnostic engine: one tree-sitter
//! runtime, the parse gate, the equivalence artifact, the protected-span
//! model, the whitespace rewriter and the comment stripper, none of which
//! knows which language it is looking at. **This** crate holds what cannot be
//! generic: the grammar configuration ([`config`]), the path set the backend
//! claims ([`paths`]), the comment hazard surface ([`policy`]) and the
//! [`tokenpress_core::Formatter`] implementation that composes them.
//!
//! [`config`] answers *what the grammar is*: which node kinds are comments,
//! which are literals whose bytes may never be touched, whether a newline can
//! change meaning. [`policy`] answers *what the language does with a comment*.
//! A grammar reaches the engine as configuration, not as a dependency, so
//! `tree-sitter-go` is named in exactly one place — [`config`] — and this
//! crate never names the `tree-sitter` runtime at all.
//!
//! # Whitespace reality
//!
//! Minimization rewrites the gaps between protected spans and nothing else: a
//! whitespace run that contained a `\n` becomes exactly one `\n`, every other
//! run becomes exactly one space, and the file's leading run is dropped. The
//! emitter therefore **never joins two lines and never introduces one**, so
//! Go's automatic semicolon insertion is preserved by construction rather
//! than by case analysis — joining is never safe in Go (`a := 1` and `b := 2`
//! on one line is a parse error, a newline after `return` changes meaning),
//! so `;`-joining is not attempted. Indentation, trailing whitespace, blank
//! lines and gofmt's alignment columns are all parts of runs that already
//! carried their newline, and CRLF normalises to LF.
//!
//! # Comment reality
//!
//! Nothing is dropped behind the caller's back: at the default settings
//! **every comment survives, byte for byte**. [`GoOptions::strip_comments`]
//! is the opt-in that deletes them — but Go has no pragma syntax, so a
//! comment is where build constraints, compiler and linker directives,
//! `go generate` commands, `go:embed` bindings and the whole cgo preamble
//! live, and the equivalence artifact is comment-blind by construction and
//! can never report their loss. Four rules defend against that, and three of
//! them apply at **both** settings because they are whitespace rules, not
//! deletion rules:
//!
//! - **deletion** — `policy::is_semantic_comment` keeps every `//`-form
//!   directive, `/*line …*/` and the legacy `// +build` constraint;
//! - **promotion** (both settings) — `//line` and `//go:generate` are read as
//!   directives only at column 0, so an *indented* directive-shaped comment
//!   is emitted with one space in front of it and can never be promoted;
//! - **the build-constraint prologue** (both settings) — when the region
//!   before the `package_clause` carries a constraint, that whole region is
//!   reproduced verbatim, blank lines included, because a legacy
//!   `// +build` line needs the blank line after it (measured with `go list`
//!   on go1.24.7: losing it moves the file from `IgnoredGoFiles` into
//!   `GoFiles`);
//! - **cgo** (both settings) — a file that imports `"C"` is left byte for
//!   byte identical, so it also reports no savings at all.
//!
//! See [`policy`] for what measured each rule.
//!
//! # Measured savings
//!
//! Measured with this backend over the Go 1.24.7 standard library
//! (`/usr/local/go/src`, 7,117 `.go` files, 79,005,123 bytes), with re-parse
//! plus equivalence enforced and refusals never written:
//!
//! | setting | bytes | o200k |
//! | --- | --- | --- |
//! | comments kept (default) | **−9.00 %** | **−7.17 %** |
//! | `strip_comments` | **−32.13 %** | **−23.61 %** |
//!
//! 7,065 files formatted in both configurations, 52 refused at parse — all
//! deliberately-invalid compiler/type-checker `testdata` — with **0**
//! re-parse failures and **0** equivalence refusals — every leaf of the
//! artifact is a token, so no captured text can span rewritten whitespace.
//! That is a measurement over this corpus and not a proof: one over-refusal
//! class did exist (a comment-only file stripping to an empty one was
//! refused), and no stdlib file is comment-only, which is why the figure
//! above never saw it. It is fixed in [`tokenpress_treesitter::comparable`].
//!
//! The savings from protecting a build-constraint prologue at the *default*
//! settings, where no comment is being deleted and only the blank line is at
//! stake, cost **3,382 bytes** of the 79 MB — 0.004 % of the corpus, for a
//! rule without which 42 stdlib files change how they build.
//!
//! `gofmt -e` (which is what [`VerifyLevel::External`] runs, see [`external`])
//! accepts every output in both configurations except five, and those five are
//! exactly the files whose *originals* `gofmt -e` already rejects — which is
//! why the external gate has to check the original first.
//!
//! # Encoding is a non-issue
//!
//! Go source is **UTF-8 by specification**, so the `&str` contract of
//! [`tokenpress_core::Formatter::format`] costs this backend nothing — unlike
//! Ruby, where an `# encoding:` magic comment makes a non-UTF-8 source legal
//! and the contract refuses it. Emission stays on bytes all the same, because
//! that is the engine's model.
//!
//! # Verification
//!
//! `Reparse` re-parses the output; `AstEquiv` compares the comparable
//! artifacts of input and output (which re-parses the output too, so no
//! separate re-parse is needed). `External` runs the `AstEquiv` check and
//! then hands the output to the Go toolchain itself (`gofmt -e`), which must
//! be on PATH; see [`external`] for what that covers and what it requires.
//! Output that fails is discarded with [`Error::Verification`] and never
//! returned.

pub mod config;
pub mod external;
pub mod paths;
pub mod policy;

use std::path::Path;

use tokenpress_core::{FormatOptions, FormatResult, Formatter, VerifyLevel};
use tokenpress_treesitter::emit::{self, CommentPolicy};
use tokenpress_treesitter::{parser, verify};

pub use tokenpress_core::{Error, Result};

/// Go-specific choices.
#[derive(Clone, Debug, Default)]
pub struct GoOptions {
    /// GOO1: drop comments. The default (`false`) keeps every one of them
    /// verbatim — comments are context for LLMs, so stripping is the opt-in.
    /// The comments that carry meaning to the Go toolchain survive either
    /// way; see the comment policy at the crate level.
    pub strip_comments: bool,
}

pub struct GoFormatter {
    options: GoOptions,
}

impl GoFormatter {
    pub fn new(options: GoOptions) -> Self {
        Self { options }
    }
}

impl Default for GoFormatter {
    fn default() -> Self {
        Self::new(GoOptions::default())
    }
}

/// The keep predicate of the comments-kept configuration.
///
/// A named function rather than a closure so it has the same
/// `fn(&[u8]) -> bool` type as [`policy::is_semantic_comment`], which is what
/// lets both configurations be one nameable [`policy::GoCommentPolicy`].
fn keep_every_comment(_bytes: &[u8]) -> bool {
    true
}

/// The policy for a given `strip_comments` setting.
///
/// The two configurations differ **only** in the keep predicate. The prologue
/// and the cgo bail-out are the same at both settings, because neither is a
/// rule about deleting comments: the blank line a legacy `// +build` needs is
/// destroyed by the whitespace policy alone, and a cgo file has to come out
/// byte-identical whatever was asked of it.
fn comment_policy(strip_comments: bool) -> policy::GoCommentPolicy {
    let keep_comment = if strip_comments {
        policy::is_semantic_comment
    } else {
        keep_every_comment as fn(&[u8]) -> bool
    };
    CommentPolicy::new(
        keep_comment,
        policy::build_constraint_prologue,
        policy::imports_c,
    )
}

impl Formatter for GoFormatter {
    fn language(&self) -> &'static str {
        "go"
    }

    fn supports(&self, path: &Path) -> bool {
        paths::supports_path(path)
    }

    /// `path` is not read: the grammar has no dialect, filepath or version
    /// selector, so the source is the whole input. It stays in the signature
    /// because the trait's other implementations need it.
    fn format(&self, _path: &Path, source: &str, options: &FormatOptions) -> Result<FormatResult> {
        let bytes = source.as_bytes();
        let config = config::go_config();
        let tree = parser::parse(&config, bytes)?;
        let plan = emit::strip_comments_plan(
            &config,
            &tree,
            bytes,
            &comment_policy(self.options.strip_comments),
        );
        // The engine's one-call emitters take neither the pinning predicate
        // nor a keep-everything policy, so the path is assembled here: plan,
        // then rewrite with the whitespace policy *and* the column-0 pin.
        // Blanking is length-preserving, so the spans the predicate is handed
        // still index `bytes` and no coordinate map is needed.
        let emitted = emit::strip_comments_pinned(bytes, &plan, emit::minimize(&config), |span| {
            policy::is_promotable_directive(bytes, span)
        });
        // The emitter only ever copies input bytes and blanks whole spans or
        // collapses runs of ASCII whitespace, so valid UTF-8 in yields valid
        // UTF-8 out and this conversion is lossless. Spelling it `_lossy`
        // keeps that invariant from becoming an error branch no input can
        // reach — and costs nothing even if it were ever wrong, because
        // verification runs on the converted bytes below: a substitution
        // would move the AST and be refused rather than written.
        let code = String::from_utf8_lossy(&emitted).into_owned();
        match options.verify {
            VerifyLevel::Reparse => {
                verify::reparse(&config, code.as_bytes())?;
            }
            // `equivalent` re-parses the output itself, so no separate
            // `reparse` call is needed at either level.
            VerifyLevel::AstEquiv => {
                verify::equivalent(&config, bytes, code.as_bytes())?;
            }
            // External tooling runs *in addition to* the built-in check, and
            // only after it: a candidate the equivalence check already
            // rejected is not worth a process spawn.
            VerifyLevel::External => {
                verify::equivalent(&config, bytes, code.as_bytes())?;
                external::check(source, &code)?;
            }
        }
        let tokenizer = options.tokenizer.load()?;
        Ok(FormatResult {
            original_tokens: tokenizer.count(source),
            formatted_tokens: tokenizer.count(&code),
            code,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tokenpress_core::{FormatOptions, Formatter, TokenizerKind, VerifyLevel};

    fn fmt(source: &str) -> String {
        fmt_with(source, GoOptions::default())
    }

    fn stripped(source: &str) -> String {
        fmt_with(
            source,
            GoOptions {
                strip_comments: true,
            },
        )
    }

    fn fmt_with(source: &str, options: GoOptions) -> String {
        GoFormatter::new(options)
            .format(Path::new("a.go"), source, &FormatOptions::default())
            .unwrap()
            .code
    }

    #[test]
    fn language_is_go() {
        assert_eq!(GoFormatter::default().language(), "go");
    }

    #[test]
    fn supports_the_go_paths() {
        // The decision itself lives in `paths`, which owns its own table of
        // cases; this only pins that the formatter delegates to it.
        let f = GoFormatter::default();
        for name in ["a.go", "main.go", "internal/deep/thing_test.go"] {
            assert!(f.supports(Path::new(name)), "{name} should be supported");
        }
        for name in ["a.rb", "a.py", "go.mod", "go", "a.GO"] {
            assert!(!f.supports(Path::new(name)), "{name} should be rejected");
        }
    }

    #[test]
    fn go01_minimizes_whitespace() {
        let source = "package main\n\nimport \"fmt\"\n\nfunc main() {\n\tfmt.Println(\"hi\")\n}\n";
        assert_eq!(
            fmt(source),
            "package main\nimport \"fmt\"\nfunc main() {\nfmt.Println(\"hi\")\n}\n"
        );
    }

    #[test]
    fn go01_collapses_gofmt_alignment_columns() {
        // gofmt aligns a `var` block's `=` signs with runs of spaces that are
        // pure formatting, and they are pure savings here.
        let source = "package main\n\nvar (\n\ta   = 1\n\tbbb = 2\n)\n";
        assert_eq!(fmt(source), "package main\nvar (\na = 1\nbbb = 2\n)\n");
    }

    #[test]
    fn go01_normalizes_crlf() {
        // A `\r` and the `\n` after it are one whitespace run, so CRLF
        // collapses to the single `\n` it carries.
        let source = "package main\r\n\r\nfunc f() {}\r\n";
        assert_eq!(fmt(source), "package main\nfunc f() {}\n");
    }

    #[test]
    fn go01_drops_the_leading_run_of_a_source_with_no_package_clause() {
        // Measured in G1: a source with no package clause parses, so the
        // backend formats it rather than refusing it, and the prologue lookup
        // has no node to anchor on.
        assert_eq!(fmt("\tfunc  f()  {}\n"), "func f() {}\n");
    }

    #[test]
    fn token_counts_are_reported() {
        let source = "package main\n\nfunc main() {\n\tx := 1\n\t_ = x\n}\n";
        let r = GoFormatter::default()
            .format(Path::new("a.go"), source, &FormatOptions::default())
            .unwrap();
        assert!(r.original_tokens > 0);
        assert!(r.formatted_tokens > 0);
        assert!(r.formatted_tokens < r.original_tokens);
        assert!(r.tokens_saved() > 0);
    }

    #[test]
    fn the_path_is_not_read() {
        // tree-sitter-go has no dialect, filepath or version selector, so a
        // supported path is the caller's filter and nothing more.
        let r = GoFormatter::default()
            .format(
                Path::new("notes.txt"),
                "package  main\n",
                &FormatOptions::default(),
            )
            .unwrap();
        assert_eq!(r.code, "package main\n");
    }

    #[test]
    fn goo1_defaults_to_keeping_comments() {
        assert!(!GoOptions::default().strip_comments);
    }

    #[test]
    fn goo1_keeps_every_comment_by_default() {
        let source = "package main\n\n// leading\nfunc main() {\n\tx := 1 // trailing\n\t/* inline */ _ = x\n}\n";
        assert_eq!(
            fmt(source),
            "package main\n// leading\nfunc main() {\nx := 1 // trailing\n/* inline */ _ = x\n}\n"
        );
    }

    #[test]
    fn goo1_strips_comments_on_request() {
        // Blanking is length-preserving, so a comment-only line leaves the one
        // newline its run carried and no blank line behind it.
        let source = "package main\n\n// leading\nfunc main() {\n\tx := 1 // trailing\n\t/* inline */ _ = x\n}\n";
        assert_eq!(
            stripped(source),
            "package main\nfunc main() {\nx := 1\n_ = x\n}\n"
        );
    }

    #[test]
    fn a_comment_only_file_is_emptied_rather_than_refused() {
        // C6(a): a file whose entire content is comments strips down to
        // nothing, and the equivalence check used to reject the empty result
        // because the artifact of a root with only comment children differed
        // from the artifact of an empty root by one space. The empty file is
        // the correct answer, so it must verify and be returned.
        let source = "// only a comment\n";
        assert_eq!(stripped(source), "");
        let r = GoFormatter::new(GoOptions {
            strip_comments: true,
        })
        .format(Path::new("a.go"), source, &FormatOptions::default())
        .unwrap();
        assert_eq!(r.formatted_tokens, 0);
    }

    #[test]
    fn goo1_keeps_semantic_comments_when_stripping() {
        // The keep predicate is `policy::is_semantic_comment`; this pins that
        // the formatter hands the policy to the engine at all.
        let source =
            "package main\n\n//go:generate echo hi\nfunc f() {}\n\n//line a.go:10\nfunc g() {\n\t// noise\n}\n";
        assert_eq!(
            stripped(source),
            "package main\n//go:generate echo hi\nfunc f() {}\n//line a.go:10\nfunc g() {\n}\n"
        );
    }

    #[test]
    fn an_indented_directive_is_never_promoted_to_column_0() {
        // Go hazard 1, and the reason both compositions are pinned: an
        // indented `//go:generate` is an ordinary comment, and moving it to
        // column 0 makes `go generate` run it. Invisible to re-parse and to
        // the comment-blind equivalence artifact, so the emitter is the only
        // defence — at the default settings just as much as when stripping.
        let source =
            "package main\n\nfunc main() {\n\t//go:generate echo PROMOTED\n\tx := 1\n\t_ = x\n}\n";
        let expected =
            "package main\nfunc main() {\n //go:generate echo PROMOTED\nx := 1\n_ = x\n}\n";
        assert_eq!(fmt(source), expected);
        assert_eq!(stripped(source), expected);
    }

    #[test]
    fn a_column_0_directive_is_not_indented_by_the_pin() {
        // The pin is a promotion guard, not a rewrite: a directive that was
        // already at the start of a line stays exactly where it was.
        let source = "package main\n\nfunc f() {}\n\n//go:noinline\nfunc g() {}\n";
        let expected = "package main\nfunc f() {}\n//go:noinline\nfunc g() {}\n";
        assert_eq!(fmt(source), expected);
        assert_eq!(stripped(source), expected);
    }

    #[test]
    fn the_build_constraint_prologue_survives_verbatim_at_both_settings() {
        // Go hazard 2: `go/build`'s `parseFileHeader` accepts a legacy
        // `// +build` line only when a blank line separates the header from
        // the package clause, and the whitespace policy collapses blank lines.
        // Measured again here with `go list` on go1.24.7: losing that blank
        // line moves the file from `IgnoredGoFiles` into `GoFiles`. Nothing
        // about that depends on comment stripping, so the prologue is
        // protected at the default settings too.
        let source =
            "//go:build ignore\n// +build ignore\n\npackage main\n\nfunc main() {\n\tx  :=  1\n\t_ = x\n}\n";
        let expected =
            "//go:build ignore\n// +build ignore\n\npackage main\nfunc main() {\nx := 1\n_ = x\n}\n";
        assert_eq!(fmt(source), expected);
        assert_eq!(stripped(source), expected);
    }

    #[test]
    fn a_header_without_a_build_constraint_is_minimized_normally() {
        // The prologue is claimed only when the region before the package
        // clause carries a constraint; an ordinary licence banner is not one,
        // so its blank line goes like any other.
        let source = "// licence\n// notes\n\npackage main\n\nfunc f() {}\n";
        assert_eq!(
            fmt(source),
            "// licence\n// notes\npackage main\nfunc f() {}\n"
        );
        assert_eq!(stripped(source), "package main\nfunc f() {}\n");
    }

    #[test]
    fn cgo_files_are_left_byte_identical_at_both_settings() {
        // The comment above `import "C"` is C source and is compiled, and its
        // `#cgo` lines carry linker flags, so there is no narrower rule worth
        // having: the whole file is left alone. The bail-out yields a plan
        // that protects every byte, so the output is the input under any gap
        // policy and any pinning predicate.
        let source = "package main\n\n/*\n#include <stdio.h>\n*/\nimport \"C\"\n\nfunc main() {\n\tC.puts(C.CString(\"hi\"))\n}\n";
        assert_eq!(fmt(source), source);
        assert_eq!(stripped(source), source);
    }

    #[test]
    fn cgo_files_report_no_savings() {
        // The other side of the bail-out, stated where a user would meet it.
        let source = "package main\n\n/*\n#include <stdio.h>\n*/\nimport \"C\"\n\nfunc main() {\n\tC.puts(C.CString(\"hi\"))\n}\n";
        let r = GoFormatter::default()
            .format(Path::new("a.go"), source, &FormatOptions::default())
            .unwrap();
        assert_eq!(r.original_tokens, r.formatted_tokens);
        assert_eq!(r.tokens_saved(), 0);
    }

    #[test]
    fn reparse_only_level_also_passes() {
        let opts = FormatOptions {
            verify: VerifyLevel::Reparse,
            ..FormatOptions::default()
        };
        let r = GoFormatter::default()
            .format(Path::new("a.go"), "package  main\n", &opts)
            .unwrap();
        assert_eq!(r.code, "package main\n");
    }

    #[test]
    fn ast_equiv_is_the_default_level() {
        assert_eq!(FormatOptions::default().verify, VerifyLevel::AstEquiv);
        let opts = FormatOptions {
            verify: VerifyLevel::AstEquiv,
            ..FormatOptions::default()
        };
        let r = GoFormatter::default()
            .format(Path::new("a.go"), "package  main\n", &opts)
            .unwrap();
        assert_eq!(r.code, "package main\n");
    }

    fn external() -> FormatOptions {
        FormatOptions {
            verify: VerifyLevel::External,
            ..FormatOptions::default()
        }
    }

    #[test]
    fn external_level_runs_gofmt_on_top_of_the_built_in_check() {
        // Spawns the real toolchain (`gofmt -e`) after the equivalence check.
        // `gofmt` agreeing here is the expected result and not a weak
        // assertion: it agreed with **every** output this backend produced
        // over the go1.24.7 standard library, in both comment configurations
        // (see [`external`]). The level is an addition to the built-in check,
        // never a substitute for it.
        let r = GoFormatter::default()
            .format(
                Path::new("a.go"),
                "package  main\n\nfunc  main()  {}\n",
                &external(),
            )
            .unwrap();
        assert_eq!(r.code, "package main\nfunc main() {}\n");
    }

    #[test]
    fn external_level_does_not_blame_input_gofmt_already_rejects() {
        // A source file carrying only comments is a clean tree-sitter parse
        // and an "expected 'package'" error to `go/parser`. The external
        // level is then satisfied by the built-in equivalence check alone, so
        // the file is still formatted instead of the run failing on the
        // user's own input.
        let r = GoFormatter::default()
            .format(Path::new("a.go"), "\n\n// note\n\n", &external())
            .unwrap();
        assert_eq!(r.code, "// note\n");
    }

    #[test]
    fn parse_errors_are_reported() {
        let err = GoFormatter::default()
            .format(
                Path::new("broken.go"),
                "package main\n\nfunc main() {\n",
                &FormatOptions::default(),
            )
            .unwrap_err();
        assert!(matches!(err, Error::Parse(_)), "{err}");
        assert!(
            err.to_string()
                .starts_with("parse error: syntax error at byte "),
            "{err}"
        );
    }

    #[test]
    fn formatting_is_idempotent() {
        let sources = [
            "package main\n\nimport \"fmt\"\n\nfunc main() {\n\tfmt.Println(\"hi\")\n}\n",
            "//go:build ignore\n// +build ignore\n\npackage main\n\nfunc main() {}\n",
            "package main\n\nfunc main() {\n\t//go:generate echo PROMOTED\n\tx := 1\n\t_ = x\n}\n",
            "package main\n\n/*\n#include <stdio.h>\n*/\nimport \"C\"\n\nfunc main() {}\n",
            "package main\n\nvar s = `raw\n  string\n`\n\n// note\nfunc f() {}\n",
        ];
        for source in sources {
            let once = fmt(source);
            assert_eq!(fmt(&once), once, "not idempotent for {source:?}");
            let once_stripped = stripped(source);
            assert_eq!(
                stripped(&once_stripped),
                once_stripped,
                "not idempotent when stripping for {source:?}"
            );
        }
    }

    #[test]
    fn tokenizer_choice_is_respected() {
        let opts = FormatOptions {
            tokenizer: TokenizerKind::Cl100kBase,
            ..FormatOptions::default()
        };
        let r = GoFormatter::default()
            .format(Path::new("a.go"), "package  main\n", &opts)
            .unwrap();
        assert!(r.original_tokens > 0);
    }
}
