//! The AST-equivalence artifact: a deterministic, formatting-independent
//! rendering of a tree-sitter parse, so that two sources can be compared for
//! structural equivalence.
//!
//! This is what makes emitter iteration safe, and it deliberately lands
//! *before* any emitter: a later `src/verify.rs` compares [`comparable`] of
//! the input against [`comparable`] of the output and refuses to write
//! anything whose artifact moved.
//!
//! # Why not `to_sexp()`
//!
//! tree-sitter ships a tree renderer, and it is unusable for this. It prints
//! **named nodes only, with no leaf text**, and in the Go grammar operators
//! and keywords are anonymous children — so `var x = 1` / `var x = 2`,
//! `a + b` / `a - b`, `"a"` / `"b"` and `16` / `0x10` all render to
//! byte-identical s-expressions (measured). Conversely it *is* sensitive to
//! comments, which is backwards for a verifier that has to accept comment
//! removal. Both failure directions are pinned in this module's tests.
//!
//! # What the artifact contains
//!
//! A pre-order walk over **every** node, named and anonymous, skipping the
//! nodes whose kind is in [`LanguageConfig::comment_kinds`]:
//!
//! - a leaf renders as `(kind text)` — one raw space, then the leaf's source
//!   bytes;
//! - a **zero-width** leaf renders as `(kind)`: it has no text, and the space
//!   alone would be the only thing separating it from an interior node whose
//!   children were all comments — which is a distinction this artifact
//!   deliberately does not make (see the over-refusal section);
//! - an interior node renders as `(kind` followed by its children and `)`,
//!   with no separator, because every child starts with `(`;
//! - a missing node renders as `(kind \<MISSING>)`. Missing nodes are
//!   zero-width too, so the marker is checked first and survives.
//!
//! ```text
//! (source_file(var_declaration(var var)(var_spec(identifier x)…)))
//! ```
//!
//! Both the kind and the leaf text are escaped, so leaf text can never forge
//! structure: `\` becomes `\\`, `(` and `)` become `\(` and `\)`, a space
//! becomes `\s`, and every byte outside ASCII becomes `\xNN`. That makes the
//! artifact ASCII whatever the source encoding, and it makes the shapes above
//! unforgeable — after a kind comes either a raw space (a leaf with text, or
//! a missing node), a `(` (an interior node) or a `)` (a zero-width leaf, or
//! an interior node whose children were all comments). The [`MISSING`] marker
//! starts with a *raw* backslash, which escaped text can never produce.
//!
//! # Over-refusal
//!
//! `tokenpress-ruby`'s artifact keeps prism's recorded source slices, some of
//! which span more than one token (a multi-line `message_loc`, a `<<~`
//! heredoc body), so reformatting *inside* such a slice is reported as a
//! difference even when it is inert. Nothing analogous exists here: **every
//! leaf of a tree-sitter tree is a token**, so no captured text can span
//! rewritten whitespace, and inter-token whitespace is not captured at all.
//! Measured over the 7,065 parseable Go 1.24.7 stdlib files, in both comment
//! configurations: **0** equivalence refusals.
//!
//! That measurement was once written up as "there is no known over-refusal
//! class", and the absolute was wrong: one class existed and the Go stdlib
//! simply has no file in it. A **comment-only** source strips to an empty
//! one, and the two rendered differently — the comment-only root took the
//! interior-node path and its one child was skipped, giving `(source_file)`,
//! while the empty root took the leaf path and emitted a separator before its
//! empty text, giving `(source_file )`. One space, and the correct output was
//! refused by all three tree-sitter backends under their strip flags
//! (CsvHelper carries 1 such file in 461). The leaf path now emits nothing
//! after the kind for a zero-width, non-missing node, which is what makes the
//! two shapes meet. No other over-refusal class is known, and the claim is
//! kept relative to what has been measured rather than absolute.
//!
//! The flip side is the deliberate blind spot: the artifact ignores comments
//! *by construction*, so every language semantic that lives in a comment (Go's
//! `//go:` directives, `// +build`, cgo prologues) is the emitter's
//! responsibility and can never be caught here.

use tokenpress_core::Result;

use crate::parser::{LanguageConfig, Node};

/// The text a missing node renders instead of its (empty) source slice.
///
/// It opens with a raw backslash, and [`push_escaped`] doubles every
/// backslash it copies, so no source text can render as this marker.
const MISSING: &str = "\\<MISSING>";

/// Renders `source` as its canonical, formatting-independent artifact.
///
/// Returns [`tokenpress_core::Error::Parse`] for a source the configured
/// grammar reports errors for — an unparseable input never yields an
/// artifact.
pub fn comparable(config: &LanguageConfig, source: &[u8]) -> Result<String> {
    let tree = crate::parser::parse(config, source)?;
    Ok(render(config, source, tree.root_node()))
}

/// Convenience wrapper: `true` when `a` and `b` have the same artifact.
///
/// Propagates the parse error if *either* side fails to parse.
pub fn equivalent(config: &LanguageConfig, a: &[u8], b: &[u8]) -> Result<bool> {
    Ok(comparable(config, a)? == comparable(config, b)?)
}

/// Walks `root` and returns the artifact.
///
/// Split out from [`comparable`] so the walk can be exercised on a tree the
/// parse gate would have refused; see the missing-node test.
fn render(config: &LanguageConfig, source: &[u8], root: Node) -> String {
    let mut artifact = String::new();
    push_node(config, source, root, &mut artifact);
    artifact
}

/// Appends the rendering of `node` and its subtree to `artifact`.
fn push_node(config: &LanguageConfig, source: &[u8], node: Node, artifact: &mut String) {
    if config
        .comment_kinds()
        .iter()
        .any(|kind| *kind == node.kind())
    {
        return;
    }
    artifact.push('(');
    push_escaped(artifact, node.kind().as_bytes());
    if node.child_count() == 0 {
        if node.is_missing() {
            // A missing node is zero-width, so its source slice would render
            // as nothing at all; the marker keeps it distinguishable. Checked
            // first for exactly that reason — the zero-width case below would
            // otherwise swallow it.
            artifact.push(' ');
            artifact.push_str(MISSING);
        } else if !node.byte_range().is_empty() {
            artifact.push(' ');
            push_escaped(artifact, &source[node.byte_range()]);
        }
        // Otherwise the node is zero-width and not missing: there is no text
        // to emit, and emitting the separator alone would be the only thing
        // distinguishing it from a node whose children were all comments. An
        // empty file's root and a comment-only file's root are the same tree
        // once comments are invisible, so they must render the same.
    } else {
        let mut cursor = node.walk();
        // `children` yields anonymous nodes too — operators, keywords and
        // punctuation are exactly what `to_sexp()` drops.
        for child in node.children(&mut cursor) {
            push_node(config, source, child, artifact);
        }
    }
    artifact.push(')');
}

/// Appends `bytes` to `artifact`, escaping everything that could be mistaken
/// for structure and everything outside ASCII.
fn push_escaped(artifact: &mut String, bytes: &[u8]) {
    for byte in bytes {
        match *byte {
            b'\\' => artifact.push_str("\\\\"),
            b'(' => artifact.push_str("\\("),
            b')' => artifact.push_str("\\)"),
            b' ' => artifact.push_str("\\s"),
            ascii if ascii.is_ascii() => artifact.push(char::from(ascii)),
            other => artifact.push_str(&format!("\\x{other:02X}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{Language, LanguageConfig};
    use tokenpress_core::Error;

    /// The dev-dependency grammar, converted from its `LanguageFn`.
    fn go() -> Language {
        tree_sitter_go::LANGUAGE.into()
    }

    fn go_config() -> LanguageConfig {
        LanguageConfig::new(
            go(),
            vec!["comment"],
            vec![
                "interpreted_string_literal",
                "raw_string_literal",
                "rune_literal",
            ],
            true,
        )
        .unwrap()
    }

    /// Pairs that differ only in formatting or in comment text: the artifact
    /// must be identical. Twelve of the probe's 38 cases.
    const EQUIVALENT: &[(&str, &str, &str)] = &[
        (
            "indentation",
            "func f() {\n\tx := 1\n}\n",
            "func f() {\nx := 1\n}\n",
        ),
        (
            "blank lines",
            "func f() {\n\n\n\tx := 1\n}\n",
            "func f() {\n\tx := 1\n}\n",
        ),
        (
            "trailing whitespace",
            "func f() {\n\tx := 1   \n}\n",
            "func f() {\n\tx := 1\n}\n",
        ),
        (
            "gofmt column alignment",
            "type T struct {\n\tA    int\n\tBcde string\n}\n",
            "type T struct {\n\tA int\n\tBcde string\n}\n",
        ),
        (
            "tabs vs spaces",
            "func f() {\n\tx := 1\n}\n",
            "func f() {\n    x := 1\n}\n",
        ),
        (
            "CRLF vs LF",
            "package main\r\n\r\nfunc f() {}\r\n",
            "package main\n\nfunc f() {}\n",
        ),
        (
            "comment text",
            "func f() {\n\t// one\n}\n",
            "func f() {\n\t// two\n}\n",
        ),
        (
            "comment removal",
            "func f() {\n\t// one\n\tx := 1\n}\n",
            "func f() {\n\tx := 1\n}\n",
        ),
        (
            "operator spacing",
            "func f() int { return 1 + 2 }\n",
            "func f() int { return 1+2 }\n",
        ),
        ("final newline", "package main", "package main\n"),
        (
            "leading blank lines",
            "\n\npackage main\n",
            "package main\n",
        ),
        (
            "space after a comma",
            "func f() { g(1, 2) }\n",
            "func f() { g(1,2) }\n",
        ),
    ];

    /// Pairs that must never be accepted as equivalent. Twenty-six of the
    /// probe's 38 cases.
    const DIFFERENT: &[(&str, &str, &str)] = &[
        ("identifier", "func f() {}\n", "func g() {}\n"),
        ("literal value", "var x = 1\n", "var x = 2\n"),
        // The base is spelled in the token text, so the artifact keeps it
        // even though both literals denote 16.
        ("integer base", "var x = 16\n", "var x = 0x10\n"),
        (
            "digit separators",
            "var x = 1000000\n",
            "var x = 1_000_000\n",
        ),
        ("hex digit case", "var x = 0xFF\n", "var x = 0xff\n"),
        ("float spelling", "var x = 1.0\n", "var x = 1.\n"),
        (
            "operator",
            "func f() int { return 1 + 2 }\n",
            "func f() int { return 1 - 2 }\n",
        ),
        (
            "comparison direction",
            "func f(a int, b int) bool { return a < b }\n",
            "func f(a int, b int) bool { return b > a }\n",
        ),
        ("string contents", "var x = \"a\"\n", "var x = \"b\"\n"),
        ("quote style", "var x = \"a\"\n", "var x = `a`\n"),
        ("escape spelling", "var x = \"\\x41\"\n", "var x = \"A\"\n"),
        ("rune", "var x = 'a'\n", "var x = 'b'\n"),
        (
            "struct tag",
            "type T struct {\n\tA int `json:\"a\"`\n}\n",
            "type T struct {\n\tA int `json:\"b\"`\n}\n",
        ),
        (
            "receiver pointer",
            "func (t T) f() {}\n",
            "func (t *T) f() {}\n",
        ),
        (
            "keyword",
            "func f(c bool) { for c {\n} }\n",
            "func f(c bool) { if c {\n} }\n",
        ),
        (
            "argument order",
            "func f(a int, b int) { g(a, b) }\n",
            "func f(a int, b int) { g(b, a) }\n",
        ),
        (
            "go statement",
            "func f() { g() }\n",
            "func f() { go g() }\n",
        ),
        (
            "semicolon vs newline",
            "func f() { a(); b() }\n",
            "func f() {\na()\nb()\n}\n",
        ),
        // The Go gotcha automatic semicolon insertion exists for: a newline
        // after `return` ends the statement.
        (
            "return newline",
            "func f() int {\n\treturn g()\n}\n",
            "func f() int {\n\treturn\n\tg()\n}\n",
        ),
        (
            "trailing comma",
            "var x = []int{1, 2}\n",
            "var x = []int{1, 2,}\n",
        ),
        (
            "label",
            "func f() {\n\tfor {\n\t\tbreak\n\t}\n}\n",
            "func f() {\nL:\n\tfor {\n\t\tbreak L\n\t}\n}\n",
        ),
        ("import path", "import \"a\"\n", "import \"b\"\n"),
        ("import alias", "import \"a\"\n", "import x \"a\"\n"),
        (
            "generic constraint",
            "func f[T any]() {}\n",
            "func f[T comparable]() {}\n",
        ),
        (
            "channel direction",
            "func f(c chan int) {}\n",
            "func f(c <-chan int) {}\n",
        ),
        // A blank line inside a raw string is program data, not formatting.
        (
            "raw string blank line",
            "var x = `a\n\nb`\n",
            "var x = `a\nb`\n",
        ),
    ];

    #[test]
    fn formatting_and_comment_only_changes_compare_equal() {
        let config = go_config();
        for (name, left, right) in EQUIVALENT {
            let left = comparable(&config, left.as_bytes()).unwrap();
            let right = comparable(&config, right.as_bytes()).unwrap();
            assert_eq!(left, right, "{name}: expected equal");
        }
    }

    #[test]
    fn semantic_changes_compare_different() {
        let config = go_config();
        for (name, left, right) in DIFFERENT {
            let left = comparable(&config, left.as_bytes()).unwrap();
            let right = comparable(&config, right.as_bytes()).unwrap();
            assert_ne!(left, right, "{name}: expected different");
        }
    }

    #[test]
    fn the_matrix_covers_the_probes_thirty_eight_cases() {
        assert_eq!(EQUIVALENT.len() + DIFFERENT.len(), 38);
    }

    #[test]
    fn the_to_sexp_counter_examples_are_distinguished() {
        // These four pairs are exactly why this module hand-rolls a walk
        // instead of calling tree-sitter's `to_sexp()`: that renderer prints
        // *named* nodes only, with no leaf text, and Go's operators are
        // anonymous children — so it produces byte-identical s-expressions
        // for every pair below. `to_sexp()` is deliberately not called here;
        // naming it would mean naming `tree_sitter` outside `parser.rs`.
        let config = go_config();
        for (left, right) in [
            ("var x = 1\n", "var x = 2\n"),
            (
                "func f(a int, b int) int { return a + b }\n",
                "func f(a int, b int) int { return a - b }\n",
            ),
            ("var x = \"a\"\n", "var x = \"b\"\n"),
            ("var x = 16\n", "var x = 0x10\n"),
        ] {
            assert!(!equivalent(&config, left.as_bytes(), right.as_bytes()).unwrap());
        }
    }

    #[test]
    fn a_leaf_renders_its_kind_and_its_text() {
        let config = go_config();
        let artifact = comparable(&config, b"var x = 1\n").unwrap();
        assert!(
            artifact.starts_with("(source_file(var_declaration(var var)"),
            "{artifact}"
        );
        assert!(artifact.contains("(identifier x)"), "{artifact}");
        assert!(artifact.contains("(int_literal 1)"), "{artifact}");
        // An interior node is `(kind` + children + `)`, with no separator: a
        // child always starts with `(`, so the two shapes cannot collide.
        assert!(artifact.contains("(var_spec(identifier x)"), "{artifact}");
    }

    #[test]
    fn comment_nodes_are_skipped_entirely() {
        let config = go_config();
        let artifact = comparable(&config, b"// hi\npackage main\n").unwrap();
        assert!(!artifact.contains("comment"), "{artifact}");
        assert!(!artifact.contains("hi"), "{artifact}");
    }

    #[test]
    fn a_configuration_without_comment_kinds_keeps_comments() {
        // Comment blindness is configuration, not a hard-coded kind list.
        let config = LanguageConfig::new(go(), vec![], vec![], true).unwrap();
        let artifact = comparable(&config, b"// hi\npackage main\n").unwrap();
        assert!(artifact.contains("(comment //\\shi)"), "{artifact}");
    }

    #[test]
    fn leaf_text_cannot_forge_structure() {
        let config = go_config();
        // A raw string whose bytes spell a complete node rendering. Escaped,
        // it can never be read as one.
        let forged = comparable(&config, "var x = `)(int_literal 2)(`\n".as_bytes()).unwrap();
        assert!(forged.contains("\\)\\(int_literal\\s2\\)\\("), "{forged}");
        assert_ne!(forged, comparable(&config, b"var x = 2\n").unwrap());
        // A backslash is doubled, so it cannot start the MISSING marker
        // either.
        let backslash = comparable(&config, "var x = `\\<MISSING>`\n".as_bytes()).unwrap();
        assert!(backslash.contains("\\\\<MISSING>"), "{backslash}");
        assert!(
            !backslash.contains("(raw_string_literal \\<MISSING>)"),
            "{backslash}"
        );
    }

    #[test]
    fn every_escape_is_reversible() {
        let mut out = String::new();
        push_escaped(&mut out, "\\()a \u{3042}".as_bytes());
        assert_eq!(out, "\\\\\\(\\)a\\s\\xE3\\x81\\x82");
    }

    #[test]
    fn the_artifact_is_ascii_whatever_the_source() {
        let config = go_config();
        let artifact = comparable(&config, "var \u{307B}\u{3052} = 1\n".as_bytes()).unwrap();
        assert!(artifact.is_ascii(), "{artifact}");
        assert!(
            artifact.contains("(identifier \\xE3\\x81\\xBB\\xE3\\x81\\x92)"),
            "{artifact}"
        );
    }

    #[test]
    fn a_missing_node_renders_a_marker() {
        // Not reachable through `comparable`: `parser::parse` gates on
        // `has_error()`, which every MISSING node sets. The marker exists so
        // the walk is total, and this test reaches it the only way the crate
        // allows — the test-only ungated parse in `parser.rs`.
        let config = go_config();
        let source = b"package main\n\nfunc f() { g(1, 2 }\n";
        let tree = crate::parser::parse_ungated(&config, source);
        assert!(tree.root_node().has_error());
        let artifact = render(&config, source, tree.root_node());
        assert!(artifact.contains("\\<MISSING>"), "{artifact}");
        // And the gated entry point still refuses it.
        assert!(matches!(
            comparable(&config, source).unwrap_err(),
            Error::Parse(_)
        ));
    }

    #[test]
    fn an_empty_source_has_an_artifact() {
        let config = go_config();
        assert_eq!(comparable(&config, b"").unwrap(), "(source_file)");
    }

    #[test]
    fn a_comment_only_source_is_equivalent_to_an_empty_one() {
        // The over-refusal class this artifact was once claimed not to have:
        // stripping every comment out of a comment-only file yields an empty
        // file, and the two must compare equal or the correct output is
        // refused. The comment-only root has one child, all of it skipped, so
        // it renders as an interior node with no children; the empty root has
        // no children at all. Both are `(source_file)`.
        let config = go_config();
        assert_eq!(
            comparable(&config, b"// only a comment\n").unwrap(),
            "(source_file)"
        );
        assert!(equivalent(&config, b"// only a comment\n", b"").unwrap());
    }

    #[test]
    fn a_zero_width_leaf_renders_no_separator() {
        // The leaf branch's third case: a node with no children whose byte
        // range is empty and which is not MISSING. It emits nothing after the
        // kind, so it cannot be told apart from an interior node whose
        // children were all comments — which is exactly the comment
        // invisibility the artifact is built for.
        let config = go_config();
        assert_eq!(
            comparable(&config, b"\n\n// a\n/* b */\n").unwrap(),
            "(source_file)"
        );
        assert_eq!(comparable(&config, b"   \n").unwrap(), "(source_file)");
    }

    #[test]
    fn a_parse_error_is_reported_not_rendered() {
        let config = go_config();
        let err = comparable(&config, b"func f( {\n").unwrap_err();
        assert!(matches!(err, Error::Parse(_)), "{err}");
        assert!(err.to_string().starts_with("parse error: "), "{err}");
    }

    #[test]
    fn equivalent_propagates_a_parse_error_from_either_side() {
        let config = go_config();
        assert!(matches!(
            equivalent(&config, b"func f( {\n", b"package main\n").unwrap_err(),
            Error::Parse(_)
        ));
        assert!(matches!(
            equivalent(&config, b"package main\n", b"func f( {\n").unwrap_err(),
            Error::Parse(_)
        ));
    }

    #[test]
    fn the_artifact_is_deterministic() {
        let config = go_config();
        let source = b"package main\n\nfunc f(a int) int { return a + 1 }\n";
        assert_eq!(
            comparable(&config, source).unwrap(),
            comparable(&config, source).unwrap()
        );
    }
}
