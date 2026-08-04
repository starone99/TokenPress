//! The Java comment hazard surface — which is almost empty — and the one file
//! shape that may not be touched at all.
//!
//! Everything here composes into a single
//! [`tokenpress_treesitter::emit::CommentPolicy`] ([`comment_policy`]): the
//! three language-specific decisions the engine's comment stripper takes.
//! Nothing in this module parses; it reads comment bytes and the tree the
//! engine already produced.
//!
//! This is deliberately **smaller than `tokenpress-go`'s** policy, and the
//! difference is not a simplification of the same problem — it is a different
//! problem. Go has no pragma syntax, so every directive rides in a comment;
//! `javac` reads nothing out of a comment at all. Measured, javac 21.0.10:
//! every `strip_comments` output of all 833 corpus files (500 apache/
//! commons-lang 3.17.0 files plus 333 sources-jar files) passes the parse
//! gate, and commons-lang3's own 11,720-test suite is green over a fully
//! comment-stripped tree. So of the three callbacks only one is non-trivial.
//!
//! # Why the keep predicate keeps nothing
//!
//! [`is_semantic_comment`] answers `false` for every comment, because no
//! comment is a compiler input. What *does* read comments in the wild is
//! third-party tooling — `NOSONAR`, `NOPMD`, `//CHECKSTYLE:OFF`,
//! `@formatter:off`, `$NON-NLS-1$`. Whether `--java-strip-comments` should
//! keep those is an **open user decision tracked in the ROADMAP**, not a
//! settled one: `javac` is indifferent, so no measurement can settle it. The
//! predicate as it stands **deletes them along with everything else**, and
//! `third_party_tool_comments_are_deleted_like_any_other` below pins that
//! behaviour so the decision, when it lands, has to be made by changing a
//! test rather than by noticing a diff.
//!
//! # Why the prologue is empty
//!
//! Go's prologue exists because a build constraint's *blank line* is part of
//! its meaning. Java has no file-header construct of any kind that survives
//! into the compiler: there is no build-constraint syntax, and
//! `package-info.java`'s package Javadoc needs no verbatim region because
//! adjacency is preserved for free — a whitespace run that held a newline
//! still emits one, so an annotation stays on the line above its `package`
//! clause. [`no_prologue`] is therefore the constant empty range the ROADMAP
//! writes as `|_, _| 0..0`, named rather than inlined so it can carry this
//! paragraph and be asserted directly.
//!
//! # Why there is no promotion rule
//!
//! Go pins directive comments to their column because `//line` and
//! `//go:generate` are directives *only* at column 0, so collapsing
//! indentation can create one. Java has no column-sensitive comment syntax at
//! all — the same measurement above is the evidence: with every comment
//! deleted and every line re-indented, all 833 files still pass the parse
//! gate and commons-lang3's suite still passes. So there is no
//! `is_promotable_directive` analogue here and this crate uses the engine's
//! plain [`strip_comments`](tokenpress_treesitter::emit::strip_comments)
//! rather than the `_pinned` sibling.
//!
//! # The one thing Java does need: the escape bail-out
//!
//! `javac` decodes `\uXXXX` escapes **before** lexing (JLS 3.3) and
//! tree-sitter-java does not. That asymmetry is Java's only
//! silent-corruption class, and it is the analogue of Go's cgo bail-out:
//! [`has_escape_hazard`] leaves the whole file alone. See its documentation
//! for the reproduction and for why nothing narrower is possible.

use std::ops::Range;

use tokenpress_treesitter::emit::CommentPolicy;
use tokenpress_treesitter::parser::{Node, Tree};

/// The comment node kinds this module scans, the same two
/// [`crate::config::java_config`] configures.
///
/// Named here rather than read out of a
/// [`LanguageConfig`](tokenpress_treesitter::parser::LanguageConfig) because
/// the bail-out is handed a tree and a source, not a config — the engine's
/// callback contract.
/// `the_scanned_kinds_are_the_configured_comment_kinds` below asserts the two
/// lists against each other so they cannot drift.
const COMMENT_KINDS: [&str; 2] = ["line_comment", "block_comment"];

/// The decoded escape values that can end a comment early, and so put live
/// code inside a `line_comment` or `block_comment` node.
///
/// `000A` (LF) and `000D` (CR) are Java's line terminators, and either ends a
/// `//` comment. `002A` (`*`) and `002F` (`/`) are hazardous **individually**
/// rather than only as the pair `\u002A\u002F`, because either half can pair
/// with a literal one: `/* \u002A/ x */` and `/* *\u002F x */` both close
/// early.
///
/// Nothing else belongs here. `\u2028` and `\u2029` are *not* Java line
/// terminators — JLS `LineTerminator` is LF, CR and CRLF — which is why the
/// gson file carrying them formats and still parses.
const HAZARDOUS_ESCAPE_VALUES: [u32; 4] = [0x000A, 0x000D, 0x002A, 0x002F];

/// True when a comment has to survive stripping — which, for Java, is never.
///
/// [`CommentPolicy`]'s `keep_comment` callback, handed a comment span's own
/// source bytes. `javac` reads nothing out of a comment, so unlike Go's
/// predicate this one defends no compiler behaviour and has no keep-list.
///
/// The parameter is unused on purpose rather than the callback being a
/// closure: a named function keeps the policy's type nameable (see
/// [`JavaCommentPolicy`]) and gives the open keep-list decision a single place
/// to land. See the module docs for what that decision is.
pub fn is_semantic_comment(_bytes: &[u8]) -> bool {
    false
}

/// The head of the file that has to be reproduced byte for byte: for Java,
/// nothing.
///
/// [`CommentPolicy`]'s `prologue` callback, and the constant `0..0` — the
/// engine reads an empty range as "there is no such region". See the module
/// docs for why Java has none.
pub fn no_prologue(_tree: &Tree, _source: &[u8]) -> Range<usize> {
    0..0
}

/// True when a comment in `source` carries a unicode escape that `javac` would
/// decode into a comment terminator, so the file must be left byte-identical.
///
/// [`CommentPolicy`]'s `bail_out` callback. `javac` processes `\uXXXX` before
/// lexing (JLS 3.3); tree-sitter-java does not. Where the decoded value ends
/// the comment, the two disagree about what is *code*, and the grammar's
/// answer is the dangerous one.
///
/// Reproduced end to end against javac 21.0.10: in a class whose second line
/// reads `int x = 1; // c \u000A int y = 2;`, the grammar puts `int y = 2;`
/// **inside** the `line_comment` node while javac compiles the field and the
/// program prints `1,2,3`. Under `strip_comments` the emitter blanks that span
/// and the field disappears — and nothing downstream can tell: the re-parse
/// passes, `comparable`/`equivalent` pass by construction because the
/// equivalence artifact is comment-blind, and **the external parse gate
/// accepts both the original and the corrupted output** (exit 0 / exit 0).
/// Only a full compile notices (`cannot find symbol: variable y`, exit 1), and
/// a full compile is not a gate this tool can run — measured, a lone-file
/// `javac -d` accepts only 29 of 333 real files. So the emitter is the only
/// place this can be stopped, and a whole-file bail-out is the only shape that
/// stops it: a narrower rewrite would have to reproduce javac's decoding pass
/// to know where the comment really ends.
///
/// The rule is narrow in *which* escapes count, though, and that narrowness is
/// measured rather than assumed. Comments do carry `\u` escapes in the wild —
/// 5 of 500 commons-lang3 files and 8 of 333 sources-jar files, spelling
/// `\u2192`, `\u0967`-`\u0969`, `\u0020`, `\u0000`, `\u2028`, `\u2029`,
/// `\uffff` and `\u0041` — but **0 of those 833 files** carry one decoding to
/// LF, CR, `*` or `/`. Bailing out on those four values costs
/// nothing measurable, where "any `\u` in a comment" would cost 13 files for
/// nothing.
pub fn has_escape_hazard(tree: &Tree, source: &[u8]) -> bool {
    node_has_escape_hazard(tree.root_node(), source)
}

/// The assembled Java comment policy.
///
/// The three decisions are plain functions with nothing to capture, so the
/// policy's type parameters are **function pointers** rather than closures.
/// That is what makes this type nameable at all: an `impl Fn` triple would
/// force every caller that wants to hold a policy — a struct field, a helper's
/// parameter — into an `impl Trait` chain it cannot write down.
pub type JavaCommentPolicy =
    CommentPolicy<fn(&[u8]) -> bool, fn(&Tree, &[u8]) -> Range<usize>, fn(&Tree, &[u8]) -> bool>;

/// The Java comment policy: the one call the formatter makes.
///
/// Building one is three function-pointer stores, so callers construct a
/// policy per operation rather than sharing one, exactly as they do with
/// [`crate::config::java_config`].
pub fn comment_policy() -> JavaCommentPolicy {
    CommentPolicy::new(is_semantic_comment, no_prologue, has_escape_hazard)
}

/// Whether `node`'s subtree holds a comment carrying a hazardous escape.
///
/// Comments are `extra` nodes and can appear anywhere, so the walk is the
/// whole tree rather than the top level. A comment node has no children of
/// interest, so answering for one ends the descent.
fn node_has_escape_hazard(node: Node, source: &[u8]) -> bool {
    if COMMENT_KINDS.contains(&node.kind()) {
        return holds_hazardous_escape(&source[node.byte_range()]);
    }
    let mut cursor = node.walk();
    let mut children = node.children(&mut cursor);
    children.any(|child| node_has_escape_hazard(child, source))
}

/// Whether a comment's own bytes carry an eligible escape decoding to a
/// comment terminator.
///
/// Eligibility is JLS 3.3's: a `\` preceded by an **even** number of
/// backslashes, since an odd count means the backslash is itself escaped and
/// the `\u` is two ordinary characters. Only the last backslash of a run can
/// be followed by a `u`, so a run opens an escape exactly when its length is
/// odd.
///
/// Scanning the comment's own bytes is exactly right rather than merely
/// convenient: a comment begins with `//` or `/*`, so no backslash run inside
/// one can reach back past its start and the parity is always fully visible
/// here.
fn holds_hazardous_escape(bytes: &[u8]) -> bool {
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'\\' {
            index += 1;
            continue;
        }
        let run = bytes[index..]
            .iter()
            .take_while(|byte| **byte == b'\\')
            .count();
        if run % 2 == 1
            && escape_value(&bytes[index + run..])
                .is_some_and(|value| HAZARDOUS_ESCAPE_VALUES.contains(&value))
        {
            return true;
        }
        index += run;
    }
    false
}

/// The value of the unicode escape whose backslash directly precedes
/// `after_backslash`, if that is what these bytes are.
///
/// JLS 3.3 spells the tail as `UnicodeMarker HexDigit HexDigit HexDigit
/// HexDigit`, where `UnicodeMarker` is one or more `u`s — so `\uu000A` is the
/// same escape as `\u000A`. Anything else is not an escape and yields `None`:
/// no marker at all (a Windows path in a comment), fewer than four bytes left,
/// or a non-hex digit among them (`\uZZZZ`, which javac itself rejects with
/// `illegal unicode escape`).
fn escape_value(after_backslash: &[u8]) -> Option<u32> {
    let markers = after_backslash
        .iter()
        .take_while(|byte| **byte == b'u')
        .count();
    if markers == 0 {
        return None;
    }
    let digits = after_backslash.get(markers..markers + 4)?;
    let mut value = 0;
    for digit in digits {
        value = value * 16 + char::from(*digit).to_digit(16)?;
    }
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::java_config;
    use tokenpress_treesitter::emit::strip_comments_source;
    use tokenpress_treesitter::parser::parse;

    /// The comments-stripped emitter as J3 will assemble it — the engine's
    /// own end-to-end entry point, with no `_pinned` variant because Java has
    /// no column-0 rule.
    fn stripped(source: &[u8]) -> Vec<u8> {
        strip_comments_source(&java_config(), source, &comment_policy()).unwrap()
    }

    /// The **comments-kept** setting as J3 will assemble it: the same three
    /// callbacks with the keep predicate answering yes to everything, which is
    /// the only thing that differs between Java's two settings.
    fn kept(source: &[u8]) -> Vec<u8> {
        let policy = CommentPolicy::new(keeps_every_comment, no_prologue, has_escape_hazard);
        strip_comments_source(&java_config(), source, &policy).unwrap()
    }

    /// The stripping emitter with the bail-out disarmed: what the output would
    /// be if [`has_escape_hazard`] did not exist.
    fn stripped_without_the_bail_out(source: &[u8]) -> Vec<u8> {
        let policy = CommentPolicy::new(is_semantic_comment, no_prologue, never_bails_out);
        strip_comments_source(&java_config(), source, &policy).unwrap()
    }

    fn keeps_every_comment(_bytes: &[u8]) -> bool {
        true
    }

    fn never_bails_out(_tree: &Tree, _source: &[u8]) -> bool {
        false
    }

    /// Whether the bail-out fires for a source the engine accepts.
    fn bails_out(source: &[u8]) -> bool {
        let config = java_config();
        let tree = parse(&config, source).unwrap();
        has_escape_hazard(&tree, source)
    }

    // --- The escape bail-out -----------------------------------------------

    #[test]
    fn the_escape_reproduction_is_left_byte_identical() {
        // The one silent-corruption class Java has, reproduced end to end
        // against javac 21.0.10. `javac` decodes `\uXXXX` **before** lexing
        // (JLS 3.3) and tree-sitter-java does not, so in this file the grammar
        // puts `int y = 2;` *inside* the `line_comment` node while javac reads
        // it as a field declaration and the program prints `1,2,3`.
        //
        // Without the bail-out the emitter blanks that span and the statement
        // disappears — and every check this backend has says the output is
        // fine: the re-parse passes, `comparable`/`equivalent` pass by
        // construction because the equivalence artifact is comment-blind, and
        // **the external parse gate accepts both the original and the
        // corrupted output** (exit 0 / exit 0). Only a full compile notices
        // (`cannot find symbol: variable y`, exit 1), and a full compile is
        // not a gate this tool can run. A whole-file bail-out is the only
        // defence there is.
        let source = b"class A {\nint x = 1; // c \\u000A int y = 2;\nint z = 3;\n}\n";
        assert!(bails_out(source));
        assert_eq!(stripped(source), source);

        // Non-vacuous: with the bail-out disarmed the field really does vanish.
        assert_eq!(
            stripped_without_the_bail_out(source),
            &b"class A {\nint x = 1;\nint z = 3;\n}\n"[..]
        );
    }

    #[test]
    fn a_file_that_trips_the_bail_out_is_byte_identical_at_both_settings() {
        // The bail-out is one of the two callbacks the two settings share, so
        // it fires whether comments are being kept or stripped, and the plan
        // it produces protects every byte in the file. Both settings therefore
        // reproduce the input exactly.
        let source = b"class A {\nint x = 1; // c \\u000A int y = 2;\nint z = 3;\n}\n";
        assert_eq!(kept(source), source);
        assert_eq!(stripped(source), source);
    }

    #[test]
    fn the_narrowness_table_holds() {
        // Prevalence, measured over the two corpora: comments really do carry
        // `\u` escapes — 5 of 500 apache/commons-lang 3.17.0 files and 8 of
        // 333 sources-jar files — spelling `\u2192`, `\u0967`-`\u0969`,
        // `\u0020`, `\u0000`, `\u2028`, `\u2029`, `\uffff` and `\u0041`. But
        // **0 of those 833 files** carry one decoding to LF, CR, `*` or `/`.
        // So the narrow rule is free, where "any `\u` in a comment" would cost
        // 13 files for nothing. `\u2028`/`\u2029` are deliberately not
        // hazardous: JLS `LineTerminator` is LF, CR and CRLF only, which is
        // why the gson file carrying them formats and still parses.
        for (escape, expected) in [
            ("\\u2192", false),
            ("\\u0020", false),
            ("\\u0000", false),
            ("\\u2028", false),
            ("\\u2029", false),
            ("\\u000A", true),
            ("\\u000D", true),
            ("\\u002A", true),
            ("\\u002F", true),
            // Hex digits are case-insensitive, so the lower-case spelling is
            // the same escape.
            ("\\u000a", true),
            // The multi-`u` form is legal Java: `UnicodeMarker` is `u {u}`.
            // Measured: `// c \uu000A int y = 2; }` exits 0 because the escape
            // ended the comment, where the control `// c uu000A int y = 2; }`
            // exits 1 because the `}` stayed inside it.
            ("\\uu000A", true),
            ("\\uuuu000A", true),
            // Parity: the backslash is itself escaped, so this is the two
            // characters `\` and `u`, not an escape.
            ("\\\\u000A", false),
        ] {
            let source = format!("class A {{ // note {escape}\n}}\n");
            assert_eq!(bails_out(source.as_bytes()), expected, "{escape}");
        }
    }

    #[test]
    fn a_block_comment_is_scanned_too() {
        // Both of Java's comment kinds are in scope. `\u002A` and `\u002F` are
        // hazardous individually rather than only as the pair `\u002A\u002F`,
        // because either half can pair with a literal one: `/* \u002A/ x */`
        // and `/* *\u002F x */` both close early.
        assert!(bails_out(b"class A { /* c \\u002A/ int y = 2; */ }\n"));
        assert!(bails_out(b"class A { /* c *\\u002F int y = 2; */ }\n"));
        assert!(!bails_out(b"class A { /* c \\u0041 */ }\n"));
    }

    #[test]
    fn backslash_parity_counts_the_whole_run() {
        // An eligible backslash is one preceded by an **even** number of
        // backslashes, so only the last backslash of a run can open an escape
        // and only when the run has odd length.
        assert!(bails_out(b"class A { // c \\u000A\n}\n"));
        assert!(!bails_out(b"class A { // c \\\\u000A\n}\n"));
        assert!(bails_out(b"class A { // c \\\\\\u000A\n}\n"));
        assert!(!bails_out(b"class A { // c \\\\\\\\u000A\n}\n"));
    }

    #[test]
    fn a_malformed_escape_is_not_a_hazard() {
        // No `u` marker at all.
        assert!(!bails_out(b"class A { // a windows path C:\\temp\n}\n"));
        // A marker with non-hex digits — `error: illegal unicode escape` to
        // javac, and nothing this rule has to defend against.
        assert!(!bails_out(b"class A { // c \\uZZZZ\n}\n"));
        // Fewer than four digits before the comment ends.
        assert!(!bails_out(b"class A { // c \\u00\n}\n"));
        // ...and a trailing backslash with nothing after it at all.
        assert!(!bails_out(b"class A { // c \\\n}\n"));
    }

    #[test]
    fn an_escape_outside_a_comment_does_not_bail_out() {
        // The rule reads comments and nothing else. A hazardous *value* in a
        // string literal is ordinary Java — `"\u002A"` is the one-character
        // string `"*"` to javac — and the file formats normally.
        let source = b"class A {\nString s = \"\\u002A\"; // note\n}\n";
        assert!(!bails_out(source));
        assert_eq!(stripped(source), &b"class A {\nString s = \"\\u002A\";\n}\n"[..]);
    }

    #[test]
    fn the_bail_out_reaches_a_comment_nested_in_a_method_body() {
        // The walk descends the whole tree, not just the top level.
        assert!(bails_out(
            b"class A {\nvoid m() {\nint x = 1; // c \\u000A int y = 2;\n}\n}\n"
        ));
    }

    #[test]
    fn a_file_with_no_escape_at_all_does_not_bail_out() {
        assert!(!bails_out(b"class A {\n// plain\n/* plain */\n}\n"));
        assert!(!bails_out(b""));
    }

    // --- The keep predicate ------------------------------------------------

    #[test]
    fn no_java_comment_is_semantic() {
        // `javac` reads nothing out of a comment: every `strip_comments`
        // output of all 833 corpus files passes the parse gate, and
        // commons-lang3's own 11,720-test suite is green over a fully
        // comment-stripped tree. So there is no correctness keep-list.
        for comment in [
            &b"// an ordinary note"[..],
            b"/** Javadoc. */",
            b"/* a block comment */",
            b"//",
        ] {
            let text = String::from_utf8_lossy(comment);
            assert!(!is_semantic_comment(comment), "{text}");
        }
    }

    #[test]
    fn third_party_tool_comments_are_deleted_like_any_other() {
        // **This test pins an open decision, not a settled one.** Whether
        // `--java-strip-comments` keeps third-party tool comments or deletes
        // them with everything else is a user decision tracked as a
        // `[blocked]` item in the ROADMAP; `javac` is indifferent, so no
        // measurement can settle it. The current predicate deletes them, and
        // this test records that. **If the decision comes back "keep", this is
        // the test to change first**, per the repository's TDD rule — change
        // the test, watch it fail, then turn the predicate into a prefix list
        // like `tokenpress-go`'s.
        //
        // Measured prevalence in commons-lang3 3.17.0: `NOSONAR` in 2 files,
        // `NOPMD` in 1, `@formatter:off`/`on` in 4 files (12 occurrences),
        // `$NON-NLS` in 1. `@SuppressWarnings` is an annotation, not a
        // comment, and is untouched by any comment rule.
        for comment in [
            &b"// NOSONAR"[..],
            b"// NOPMD",
            b"//CHECKSTYLE:OFF",
            b"// @formatter:off",
            b"//$NON-NLS-1$",
        ] {
            let text = String::from_utf8_lossy(comment);
            assert!(!is_semantic_comment(comment), "{text}");
        }
        let source = b"class A {\nint x = 1; // NOSONAR\n}\n";
        assert_eq!(stripped(source), &b"class A {\nint x = 1;\n}\n"[..]);
    }

    // --- The empty prologue ------------------------------------------------

    #[test]
    fn the_prologue_is_always_empty() {
        // Java has no build-constraint header and no `package-info` special
        // case: a package Javadoc keeps its adjacency for free, because a
        // whitespace run that held a newline still emits one.
        let config = java_config();
        for source in [
            &b""[..],
            b"class A {}\n",
            b"/** Package docs. */\n@Deprecated\npackage a;\n",
        ] {
            let tree = parse(&config, source).unwrap();
            assert_eq!(no_prologue(&tree, source), 0..0);
        }
    }

    #[test]
    fn a_package_info_keeps_its_annotation_adjacent_to_the_package_clause() {
        // The prologue is empty and nothing is lost by it: the Javadoc goes,
        // and the annotation stays on the line above the clause it applies to.
        assert_eq!(
            stripped(b"/** Package docs. */\n@Deprecated\npackage a;\n"),
            &b"@Deprecated\npackage a;\n"[..]
        );
    }

    // --- The assembled policy ----------------------------------------------

    #[test]
    fn the_policy_deletes_every_comment_and_keeps_the_code() {
        let source =
            b"/** Doc. */\nclass A {\n// note\nint x = 1; /* trailing */\nString s = \"// not a comment\";\n}\n";
        assert_eq!(
            stripped(source),
            &b"class A {\nint x = 1;\nString s = \"// not a comment\";\n}\n"[..]
        );
        // ...and the same policy at the kept setting changes nothing but the
        // whitespace.
        assert_eq!(
            kept(source),
            &b"/** Doc. */\nclass A {\n// note\nint x = 1; /* trailing */\nString s = \"// not a comment\";\n}\n"[..]
        );
    }

    #[test]
    fn the_scanned_kinds_are_the_configured_comment_kinds() {
        // The bail-out names the two kinds itself rather than reading them out
        // of a `LanguageConfig` it is not given. This is what keeps the two
        // lists from drifting apart.
        assert_eq!(java_config().comment_kinds(), COMMENT_KINDS);
    }
}
