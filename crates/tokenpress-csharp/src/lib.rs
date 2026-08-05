//! TokenPress for C# — the C#-specific half of the tree-sitter backend.
//!
//! Pipeline: parse ([`config`] names the grammar,
//! [`tokenpress_treesitter::parser`] drives it) → whitespace-minimal re-emit
//! over the source bytes under the comment policy ([`policy`]) →
//! verification ([`tokenpress_treesitter::verify`]) → token accounting. The
//! path is only used to decide whether this backend claims the file
//! ([`paths`]); the grammar has no dialect, filepath or version selector, so
//! [`CSharpFormatter::format`] never reads it.
//!
//! # The crate split
//!
//! `tokenpress-treesitter` is the grammar-agnostic engine: one tree-sitter
//! runtime, the parse gate, the equivalence artifact, the protected-span
//! model, the whitespace rewriter and the comment stripper, none of which
//! knows which language it is looking at. **This** crate holds what cannot be
//! generic: the grammar configuration ([`config`]), the path set the backend
//! claims ([`paths`]), the comment hazard surface ([`policy`]) and the
//! [`tokenpress_core::Formatter`] implementation that composes them. A
//! grammar reaches the engine as configuration, not as a dependency, so
//! `tree-sitter-c-sharp` is named in exactly one place — [`config`] — and
//! this crate never names the `tree-sitter` runtime at all.
//!
//! # What C# brings that Go and Java did not
//!
//! Two differences shape everything above this layer, and both are already
//! visible in [`config`]. **One comment kind**, not Java's two: `//`,
//! `/* … */` and `///` XML documentation comments are all the node kind
//! `comment`, so a policy that wants to treat XML doc specially cannot key on
//! the kind and has to read the comment's leading bytes. And **five**
//! protected literal kinds, not Java's two, because C# spells a string five
//! ways — ordinary, verbatim, raw, interpolated, and the character literal —
//! and the grammar gives four of them a node kind of their own.
//!
//! The one constraint that has no analogue in either earlier backend is the
//! **preprocessor line rule**: `#if`, `#region`, `#nullable` and their
//! relatives must each begin a line, so a directive dragged onto the line
//! before it stops being a directive. That is one of the two hazards behind
//! `newline_sensitive = true` — see [`config`] for the measurement — and
//! [`policy`] keeps it that way, as well as carrying the three constructs
//! where the grammar and a real C# compiler disagree about where a comment
//! ends.
//!
//! # Whitespace reality
//!
//! Minimization rewrites the gaps between protected spans and nothing else: a
//! whitespace run that contained a `\n` becomes exactly one `\n`, every other
//! run becomes exactly one space, and the file's leading run is dropped. The
//! emitter therefore **never joins two lines and never introduces one**.
//! C# has no automatic semicolon insertion and would tolerate joining
//! syntactically, but line structure is preserved all the same, for the two
//! reasons [`config`] measured: a `//` comment ends at the newline after it,
//! and a preprocessor directive must begin its line. Indentation, trailing
//! whitespace, blank lines and every alignment column are parts of runs that
//! already carried their newline, and CRLF normalises to LF. A raw string's
//! interior is not whitespace between spans — it *is* a span — so its
//! significant indentation is copied verbatim, and so are the bytes of a
//! verbatim, interpolated or UTF-8-suffixed literal.
//!
//! # Comment reality
//!
//! Nothing is dropped behind the caller's back: at the default settings
//! **every comment survives, byte for byte**.
//! [`CSharpOptions::strip_comments`] is the opt-in that deletes them, and what
//! it deletes includes **XML documentation comments** — a `///` block is an
//! ordinary `comment` node to this grammar, with no second kind to spare it,
//! so the API documentation of a stripped file goes with the rest of its
//! comments. That is the flag working, not a caveat about it: it is where the
//! difference between the two rows below comes from, it is asked for
//! explicitly, and at the default settings not one byte of documentation is
//! dropped. Whether the flag should carve `///` out is an open decision
//! tracked in the ROADMAP; [`policy`] is where such a carve-out would live,
//! and why it would have to be a text-prefix test rather than a node kind.
//!
//! `csc` reads nothing out of a comment under the invocation C5's gate is
//! designed around, so unlike Go — where a comment carries build constraints,
//! compiler directives and the cgo preamble — there is no keep-list a correct
//! output depends on and no verbatim prologue. C#'s hazards are a different
//! shape: the grammar and the compiler can disagree about where a comment
//! *ends* (a comment spanning a `#if`/`#endif` pair, or a `//` comment
//! carrying one of the line terminators C# has and the grammar ignores), and
//! blanking a comment can *invent* a directive by promoting a `#` to the
//! start of its line. A file carrying any of the three is left byte for byte
//! identical at **both** settings and reports no savings —
//! [`policy::has_comment_boundary_hazard`], the analogue of Go's cgo bail-out
//! and Java's escape bail-out.
//!
//! So **some C# files are returned unchanged by design**. That is a documented
//! consequence and not a parse failure: such a file parses clean — which is
//! exactly the problem, since the parse gate cannot see the hazard — and it is
//! returned as `Ok` with zero savings rather than as an [`Error`]. See
//! [`policy`] for the reproductions and for what measured each decision.
//!
//! # No pinned emitter, and why
//!
//! Go composes the engine's
//! [`strip_comments_pinned`](tokenpress_treesitter::emit::strip_comments_pinned)
//! at both settings, because `//go:generate` is a directive only at column 0
//! and collapsing a comment's indentation would promote it. C# **does** have a
//! column rule — a preprocessor directive must begin its line — but it is a
//! rule about the `#`, not about a comment: no C# comment means anything
//! different for arriving at column 0, so there is no comment span to pin.
//! This backend therefore uses the plain
//! [`strip_comments`](tokenpress_treesitter::emit::strip_comments), Java's
//! composition, and **both** settings run one path that differs only in the
//! keep predicate.
//!
//! The two ways the directive rule could still be broken are covered
//! elsewhere and at both settings, which is what makes that safe: *losing* the
//! newline that starts a directive's line cannot happen because
//! `newline_sensitive = true`, and *inventing* a directive by blanking the
//! comment that shielded it is clause 3 of the boundary bail-out, which
//! refuses the whole file.
//!
//! # Measured savings
//!
//! Measured here with this backend over the JamesNK/Newtonsoft.Json working
//! tree at `4f73e74` (945 `.cs` files; 65 refused at the parse gate and 1
//! refused by the equivalence check, leaving **879** written, 5,188,596 bytes
//! in), with re-parse plus equivalence enforced and refusals never written:
//!
//! | setting | bytes | o200k |
//! | --- | --- | --- |
//! | comments kept (default) | **−18.43 %** | **−8.68 %** |
//! | `strip_comments` | **−43.34 %** | **−33.64 %** |
//!
//! The byte figure of the first row reproduces C1's exactly, on the same tree.
//! **0** of the 879 files were returned unchanged, at either setting: the
//! boundary bail-out never fired, which is C2's 0-of-880 measurement seen from
//! the formatter — and carries C2's caveat, that this corpus holds 26,883
//! comments but not one `/* … */` block, so the bail-out's first clause had
//! nothing to bite on. One corpus is not a distribution: C6(a) owes a second
//! one, chosen to contain delimited comments.
//!
//! # Encoding is refused upstream
//!
//! C# source has no single fixed encoding — `csc /codepage` exists, and a file
//! with no BOM is read as the code page it names — so a non-UTF-8 file can be
//! legal C# to a project configured for it. (Read from the compiler's
//! documented options, not run: this container has no C# toolchain.) Because
//! [`tokenpress_core::Formatter::format`] takes `&str` and the CLI reads with
//! `std::fs::read_to_string`, such a file never reaches this backend at all —
//! it is **refused at read time**, the Java and Ruby situation rather than the
//! Go one, where UTF-8 is in the specification. It costs nothing measurable in
//! practice — 0 of the 945 corpus files are non-UTF-8 — and emission stays on
//! bytes all the same, because that is the engine's model.
//!
//! # Verification
//!
//! `Reparse` re-parses the output; `AstEquiv` compares the comparable
//! artifacts of input and output (which re-parses the output too, so no
//! separate re-parse is needed). `External` is **folded into the `AstEquiv`
//! arm** until C5 ships the `csc` gate, which is what the Python, Rust, Go and
//! Java backends each did before their own external checker existed — so no
//! level is a promise this crate cannot keep, and no arm is unreachable.
//! Output that fails is discarded with [`Error::Verification`] and never
//! returned.

pub mod config;
pub mod paths;
pub mod policy;

use std::path::Path;

use tokenpress_core::{FormatOptions, FormatResult, Formatter, VerifyLevel};
use tokenpress_treesitter::emit::{self, CommentPolicy};
use tokenpress_treesitter::{parser, verify};

pub use tokenpress_core::{Error, Result};

/// C#-specific choices.
#[derive(Clone, Debug, Default)]
pub struct CSharpOptions {
    /// CSO1: drop comments. The default (`false`) keeps every one of them
    /// verbatim — comments are context for LLMs, so stripping is the opt-in.
    /// An XML documentation comment (`///`) is a comment like any other to
    /// this grammar and goes with them; see the crate docs.
    pub strip_comments: bool,
}

pub struct CSharpFormatter {
    options: CSharpOptions,
}

impl CSharpFormatter {
    pub fn new(options: CSharpOptions) -> Self {
        Self { options }
    }
}

impl Default for CSharpFormatter {
    fn default() -> Self {
        Self::new(CSharpOptions::default())
    }
}

/// The keep predicate of the comments-kept configuration.
///
/// A named function rather than a closure so it has the same
/// `fn(&[u8]) -> bool` type as [`policy::is_semantic_comment`], which is what
/// lets both configurations be one nameable [`policy::CSharpCommentPolicy`].
fn keep_every_comment(_bytes: &[u8]) -> bool {
    true
}

/// The policy for a given `strip_comments` setting.
///
/// The two configurations differ **only** in the keep predicate: the prologue
/// is the constant empty range and the boundary bail-out is a rule about a
/// grammar/compiler disagreement rather than about deleting anything, so both
/// are shared unchanged — and the bail-out firing at the kept setting too is
/// what makes a hazardous file byte-identical either way.
///
/// The `strip_comments` arm is exactly [`policy::comment_policy`], assembled
/// here from the same three callbacks so the differing one is visible in one
/// place; `the_stripping_setting_is_the_policy_modules_own_policy` pins that
/// the two cannot drift apart.
fn comment_policy(strip_comments: bool) -> policy::CSharpCommentPolicy {
    let keep_comment = if strip_comments {
        policy::is_semantic_comment
    } else {
        keep_every_comment as fn(&[u8]) -> bool
    };
    CommentPolicy::new(
        keep_comment,
        policy::no_prologue,
        policy::has_comment_boundary_hazard,
    )
}

impl Formatter for CSharpFormatter {
    fn language(&self) -> &'static str {
        "csharp"
    }

    fn supports(&self, path: &Path) -> bool {
        paths::supports_path(path)
    }

    /// `path` is not read: the grammar has no dialect, filepath or version
    /// selector, so the source is the whole input — a `*.Designer.cs` or a
    /// `*.g.cs` included. It stays in the signature because the trait's other
    /// implementations need it.
    fn format(&self, _path: &Path, source: &str, options: &FormatOptions) -> Result<FormatResult> {
        let bytes = source.as_bytes();
        let config = config::csharp_config();
        let tree = parser::parse(&config, bytes)?;
        let plan = emit::strip_comments_plan(
            &config,
            &tree,
            bytes,
            &comment_policy(self.options.strip_comments),
        );
        // The plain emitter, not `strip_comments_pinned`. C# does have a
        // column rule — a preprocessor directive must begin its line — but it
        // is a rule about the `#`, not about a comment: no comment means
        // anything different for arriving at column 0, so there is nothing to
        // pin a comment span against. The two ways the rule could still be
        // broken are handled elsewhere and at *both* comment settings: losing
        // the newline before a directive cannot happen because
        // `newline_sensitive = true` (see [`config`]), and *inventing* a
        // directive by blanking the comment that shielded it is clause 3 of
        // [`policy::has_comment_boundary_hazard`], which bails the whole file
        // out. The plan is built here rather than through the engine's
        // one-call `strip_comments_source` so the tree is parsed once and not
        // again.
        let emitted = emit::strip_comments(bytes, &plan, emit::minimize(&config));
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
            // `reparse` call is needed at either level. `External` is folded
            // in here until C5 ships `csc`'s diagnostic-multiset gate — the
            // Python, Rust, Go and Java precedent for a level that has no
            // external checker yet, and what keeps every arm reachable.
            VerifyLevel::AstEquiv | VerifyLevel::External => {
                verify::equivalent(&config, bytes, code.as_bytes())?;
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
        fmt_with(source, CSharpOptions::default())
    }

    fn stripped(source: &str) -> String {
        fmt_with(
            source,
            CSharpOptions {
                strip_comments: true,
            },
        )
    }

    fn fmt_with(source: &str, options: CSharpOptions) -> String {
        CSharpFormatter::new(options)
            .format(Path::new("A.cs"), source, &FormatOptions::default())
            .unwrap()
            .code
    }

    #[test]
    fn language_is_csharp() {
        // "csharp", not "c#": the name is a CLI language key and a config
        // table name, so it stays in the character set both can spell.
        assert_eq!(CSharpFormatter::default().language(), "csharp");
    }

    #[test]
    fn supports_the_csharp_paths() {
        // The decision itself lives in `paths`, which owns its own table of
        // cases; this only pins that the formatter delegates to it.
        let f = CSharpFormatter::default();
        for name in ["A.cs", "src/Newtonsoft.Json/Linq/JToken.cs", "A.Designer.cs"] {
            assert!(f.supports(Path::new(name)), "{name} should be supported");
        }
        for name in ["a.go", "A.java", "A.CS", "A.csx", "A.csproj", "cs"] {
            assert!(!f.supports(Path::new(name)), "{name} should be rejected");
        }
    }

    #[test]
    fn cs01_minimizes_whitespace() {
        let source = "/// <summary>The class.</summary>\npublic class A\n{\n    /// <summary>The field.</summary>\n    private int x = 1;\n\n    public int X() => x;\n}\n";
        assert_eq!(
            fmt(source),
            "/// <summary>The class.</summary>\npublic class A\n{\n/// <summary>The field.</summary>\nprivate int x = 1;\npublic int X() => x;\n}\n"
        );
    }

    #[test]
    fn cso1_defaults_to_keeping_comments() {
        assert!(!CSharpOptions::default().strip_comments);
    }

    #[test]
    fn a_raw_string_literal_survives_byte_for_byte_at_both_settings() {
        let source = "class A\n{\n    // note\n    string s = \"\"\"\n        one\n          two\n        \"\"\";\n}\n";
        let interior = "\"\"\"\n        one\n          two\n        \"\"\"";
        let kept = fmt(source);
        assert!(kept.contains(interior), "{kept:?}");
        assert_eq!(
            kept,
            "class A\n{\n// note\nstring s = \"\"\"\n        one\n          two\n        \"\"\";\n}\n"
        );
        let stripped = stripped(source);
        assert!(stripped.contains(interior), "{stripped:?}");
        assert_eq!(
            stripped,
            "class A\n{\nstring s = \"\"\"\n        one\n          two\n        \"\"\";\n}\n"
        );
    }

    #[test]
    fn cs01_collapses_alignment_and_blank_lines() {
        // Every run that held a newline emits one newline, every other run
        // one space: indentation, alignment columns and blank lines are all
        // pure savings.
        let source = "class A\n{\n    int    a   = 1;\n\n    int    bbb = 2;\n}\n";
        assert_eq!(fmt(source), "class A\n{\nint a = 1;\nint bbb = 2;\n}\n");
    }

    #[test]
    fn cs01_normalizes_crlf() {
        // A `\r` and the `\n` after it are one whitespace run, so CRLF
        // collapses to the single `\n` it carries — and the newline a
        // directive's line needs is one of those, whichever way the file was
        // checked out.
        let source = "class A\r\n{\r\n\r\n    int a = 1;\r\n}\r\n";
        assert_eq!(fmt(source), "class A\n{\nint a = 1;\n}\n");
    }

    #[test]
    fn cs01_drops_the_leading_run_of_the_file() {
        assert_eq!(fmt("\n\n   class  A  {}\n"), "class A {}\n");
    }

    #[test]
    fn cso1_keeps_every_comment_by_default() {
        let source = "class A\n{\n    // leading\n    int x = 1; // trailing\n    /* inline */ int y = 2;\n}\n";
        assert_eq!(
            fmt(source),
            "class A\n{\n// leading\nint x = 1; // trailing\n/* inline */ int y = 2;\n}\n"
        );
    }

    #[test]
    fn cso1_strips_comments_on_request() {
        // Blanking is length-preserving, so a comment-only line leaves the one
        // newline its run carried and no blank line behind it.
        let source = "class A\n{\n    // leading\n    int x = 1; // trailing\n    /* inline */ int y = 2;\n}\n";
        assert_eq!(stripped(source), "class A\n{\nint x = 1;\nint y = 2;\n}\n");
    }

    #[test]
    fn cso1_deletes_xml_doc_comments_when_stripping() {
        // The documented consequence of the opt-in, pinned where a user would
        // meet it: `///` is an ordinary `comment` to this grammar — there is
        // no second kind to spare it — so the API documentation goes with
        // every other comment. Nothing is dropped at the default settings,
        // which the second half asserts. `policy` records why a carve-out
        // would have to be a text-prefix test, and that the decision is open.
        let source = "/// <summary>The class.</summary>\npublic class A\n{\n    /// <summary>The method.</summary>\n    public void M() {}\n}\n";
        assert_eq!(stripped(source), "public class A\n{\npublic void M() {}\n}\n");
        assert_eq!(
            fmt(source),
            "/// <summary>The class.</summary>\npublic class A\n{\n/// <summary>The method.</summary>\npublic void M() {}\n}\n"
        );
    }

    #[test]
    fn an_indented_comment_is_emitted_at_column_0() {
        // Why there is no `_pinned` composition here, pinned as behaviour
        // rather than as prose. Go has to keep `//go:generate` off column 0,
        // because moving it there makes `go generate` run it. C#'s column rule
        // is about the `#` of a directive and not about a comment, so an
        // indented comment simply loses its indentation like anything else,
        // and both settings run the one path.
        let source = "class A\n{\n    void M()\n    {\n        // note\n        int x = 1;\n    }\n}\n";
        assert_eq!(
            fmt(source),
            "class A\n{\nvoid M()\n{\n// note\nint x = 1;\n}\n}\n"
        );
        assert_eq!(
            stripped(source),
            "class A\n{\nvoid M()\n{\nint x = 1;\n}\n}\n"
        );
    }

    #[test]
    fn a_directive_still_begins_its_line_at_both_settings() {
        // The other half of that decision: the directive rule is defended by
        // `newline_sensitive = true` (the run before the `#` held a newline,
        // so it still emits one) and not by pinning anything, and deleting the
        // comment above a directive cannot pull it up.
        let source = "class A\n{\n    // note\n    #region helpers\n    int x = 1;\n    #endregion\n}\n";
        assert_eq!(
            fmt(source),
            "class A\n{\n// note\n#region helpers\nint x = 1;\n#endregion\n}\n"
        );
        assert_eq!(
            stripped(source),
            "class A\n{\n#region helpers\nint x = 1;\n#endregion\n}\n"
        );
    }

    #[test]
    fn the_stripping_setting_is_the_policy_modules_own_policy() {
        // `comment_policy(true)` is assembled from the same three callbacks
        // as `policy::comment_policy`, so the two are one policy written
        // twice; this pins that they cannot drift apart.
        let source = b"/// <summary>Doc.</summary>\nclass A {\n// note\nint x = 1; /* trailing */\n}\n";
        let config = config::csharp_config();
        let theirs =
            emit::strip_comments_source(&config, source, &policy::comment_policy()).unwrap();
        let ours = emit::strip_comments_source(&config, source, &comment_policy(true)).unwrap();
        assert_eq!(ours, theirs);
    }

    #[test]
    fn a_boundary_hazard_file_is_left_byte_identical_at_both_settings() {
        // C#'s silent-corruption class, reached through the preprocessor
        // rather than through an escape: to the compiler both `#if FALSE`
        // sections are skipped text scanned only for directives, so
        // `int x = 1;` is a field, while the grammar lexes one comment
        // straight through both `#endif`s. The bail-out is one of the two
        // callbacks both settings share, and the plan it yields protects every
        // byte. See `policy::has_comment_boundary_hazard` for the end-to-end
        // reproduction and for the other two clauses.
        let source = "class A {\n#if FALSE\n/*\n#endif\nint x = 1;\n#if FALSE\n*/\n#endif\n}\n";
        assert_eq!(fmt(source), source);
        assert_eq!(stripped(source), source);
    }

    #[test]
    fn a_boundary_hazard_file_reports_no_savings() {
        // The other side of the bail-out, stated where a user would meet it:
        // some C# files are returned unchanged **by design**, and that is not
        // a parse failure — the file parses clean, which is the whole problem.
        let source = "class A {\n#if FALSE\n/*\n#endif\nint x = 1;\n#if FALSE\n*/\n#endif\n}\n";
        let r = CSharpFormatter::default()
            .format(Path::new("A.cs"), source, &FormatOptions::default())
            .unwrap();
        assert_eq!(r.original_tokens, r.formatted_tokens);
        assert_eq!(r.tokens_saved(), 0);
    }

    #[test]
    fn token_counts_are_reported() {
        let source = "public class A\n{\n    /// <summary>Doc.</summary>\n    public int X()\n    {\n        return 1;\n    }\n}\n";
        let r = CSharpFormatter::default()
            .format(Path::new("A.cs"), source, &FormatOptions::default())
            .unwrap();
        assert!(r.original_tokens > 0);
        assert!(r.formatted_tokens > 0);
        assert!(r.formatted_tokens < r.original_tokens);
        assert!(r.tokens_saved() > 0);
    }

    #[test]
    fn the_path_is_not_read() {
        // tree-sitter-c-sharp has no dialect, filepath or version selector, so
        // a supported path is the caller's filter and nothing more — which is
        // also why a generated `*.Designer.cs` needs no special case.
        let r = CSharpFormatter::default()
            .format(
                Path::new("notes.txt"),
                "class  A  {}\n",
                &FormatOptions::default(),
            )
            .unwrap();
        assert_eq!(r.code, "class A {}\n");
    }

    #[test]
    fn reparse_only_level_also_passes() {
        let opts = FormatOptions {
            verify: VerifyLevel::Reparse,
            ..FormatOptions::default()
        };
        let r = CSharpFormatter::default()
            .format(Path::new("A.cs"), "class  A  {}\n", &opts)
            .unwrap();
        assert_eq!(r.code, "class A {}\n");
    }

    #[test]
    fn ast_equiv_is_the_default_level() {
        assert_eq!(FormatOptions::default().verify, VerifyLevel::AstEquiv);
        let opts = FormatOptions {
            verify: VerifyLevel::AstEquiv,
            ..FormatOptions::default()
        };
        let r = CSharpFormatter::default()
            .format(Path::new("A.cs"), "class  A  {}\n", &opts)
            .unwrap();
        assert_eq!(r.code, "class A {}\n");
    }

    #[test]
    fn external_level_currently_behaves_like_ast_equiv() {
        // C5 replaces this with `csc`'s diagnostic-multiset comparison. Until
        // it lands the level runs the built-in equivalence check and nothing
        // else, which is what the Python, Rust, Go and Java backends each did
        // before their own external gate shipped.
        let opts = FormatOptions {
            verify: VerifyLevel::External,
            ..FormatOptions::default()
        };
        let r = CSharpFormatter::default()
            .format(Path::new("A.cs"), "class  A  {}\n", &opts)
            .unwrap();
        assert_eq!(r.code, "class A {}\n");
    }

    #[test]
    fn parse_errors_are_reported() {
        let err = CSharpFormatter::default()
            .format(
                Path::new("Broken.cs"),
                "class A {\n    void M() {\n",
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
    fn a_candidate_that_fails_verification_is_never_returned() {
        // The core invariant, reached by the one input C1's 880-file corpus
        // found: `#region helpers` with trailing spaces puts those spaces
        // inside the `preproc_arg` leaf, so collapsing the run moves the
        // comparable artifact. The output is discarded rather than written.
        let source = "class A\n{\n#region helpers   \n#endregion\n}\n";
        let err = CSharpFormatter::default()
            .format(Path::new("A.cs"), source, &FormatOptions::default())
            .unwrap_err();
        assert!(matches!(err, Error::Verification(_)), "{err}");
    }

    #[test]
    fn formatting_is_idempotent() {
        let sources = [
            // A file-scoped namespace.
            "namespace Example;\n\npublic class A\n{\n    private int x = 1;\n}\n",
            // A `record struct`.
            "public record struct Point(int X, int Y);\n",
            // A raw string literal, whose interior is a protected span.
            "class A\n{\n    string s = \"\"\"\n        one\n          two\n        \"\"\";\n}\n",
            // A `#region` block.
            "class A\n{\n    #region helpers\n    int x = 1;\n    #endregion\n}\n",
            // An `#if`/`#endif` pair, with the arms complete syntactic units.
            "class A\n{\n#if FOO\n    int x = 1;\n#else\n    int x = 2;\n#endif\n}\n",
            // A primary constructor.
            "public class A(int x)\n{\n    public int X => x;\n}\n",
            // An XML-doc'd class.
            "/// <summary>The class.</summary>\npublic class A\n{\n    /// <summary>The field.</summary>\n    private int x = 1;\n}\n",
            // ...and the boundary-hazard file the bail-out reproduces
            // verbatim, which is idempotent by being untouched.
            "class A {\n#if FALSE\n/*\n#endif\nint x = 1;\n#if FALSE\n*/\n#endif\n}\n",
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
        let r = CSharpFormatter::default()
            .format(Path::new("A.cs"), "class  A  {}\n", &opts)
            .unwrap();
        assert!(r.original_tokens > 0);
    }
}
