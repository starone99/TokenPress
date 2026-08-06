//! The Java grammar as engine configuration — the only module that names
//! `tree_sitter_java`.
//!
//! This crate's grammar boundary, and the analogue of the sibling backends'
//! `parser.rs` under CLAUDE.md's confinement rule (ruff in
//! `tokenpress-python`, oxc in `tokenpress-js`, prism in `tokenpress-ruby`).
//! It is called `config` rather than `parser` because no parsing happens
//! here: a grammar is a runtime value the engine takes as configuration, so
//! all this module does is name the kinds and hand
//! `tree_sitter_java::LANGUAGE` to the engine. Parsing itself stays in
//! `tokenpress_treesitter::parser`.

use tokenpress_treesitter::parser::LanguageConfig;

/// The Java configuration the engine drives.
///
/// - comment kinds `line_comment` and `block_comment` — Java has **two**,
///   the first structural difference from Go's single `comment`. Javadoc is
///   not a third kind: a `/** … */` block is an ordinary `block_comment`.
/// - protected kinds `string_literal`, `character_literal` — every kind
///   whose bytes carry meaning of their own and must be copied verbatim.
///   One kind covers both ordinary strings and `"""` text blocks, because
///   **there is no `text_block` node kind**: a text block parses to a
///   `string_literal` wrapping `multiline_string_fragment` children, so the
///   engine's whole-node span protection keeps a text block's significant
///   indentation with no new rule. `string_fragment`, `escape_sequence` and
///   `string_interpolation` are all *children* of `string_literal` and need
///   not be listed; the engine's walk-and-merge absorbs them.
/// - newline-sensitive. Java has no automatic semicolon insertion, but the
///   flag does not mean "has ASI" — it means "does the emitter preserve line
///   structure", and Java needs it preserved because the `false` branch
///   collapses the newline *after* a `line_comment` to a space, so the
///   comment swallows the line below it. Measured at the default settings
///   (comments kept): 247 of 500 apache/commons-lang 3.17.0 files are then
///   refused by `verify::equivalent`. `false` also buys nothing — under
///   `strip_comments` the two branches emit byte-identical output, and on
///   tokens `false` is marginally worse (o200k −45.41 % against −45.51 %).
///
/// Building the configuration is cheap (a `Language` handle plus two small
/// vectors), so callers construct one per operation rather than sharing one.
pub fn java_config() -> LanguageConfig {
    LanguageConfig::new(
        // The grammar crate exports a `LanguageFn`; the engine's re-exported
        // `Language` is what `LanguageConfig` wants, and the conversion is
        // the only reason this crate depends on a grammar at all.
        tree_sitter_java::LANGUAGE.into(),
        vec!["line_comment", "block_comment"],
        vec!["string_literal", "character_literal"],
        true,
    )
    // `LanguageConfig::new` rejects a kind name the grammar does not have.
    // This list is fixed and correct, so the error cannot occur — and rather
    // than propagate a `Result` no caller could ever act on, the constructor
    // is unwrapped here, exactly as `tokenpress-go` does and as the engine
    // unwraps `set_language` for its sibling impossibility (the ABI window).
    // What keeps that honest is the same thing: tests below that assert each
    // kind against the grammar, so a grammar bump that renames one fails
    // `cargo test` rather than a user's run.
    .expect("every configured kind is a named node kind of the pinned tree-sitter-java grammar")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokenpress_treesitter::parser::{
        parse, Language, LANGUAGE_VERSION, MIN_COMPATIBLE_LANGUAGE_VERSION,
    };
    use tokenpress_treesitter::Error;

    /// The grammar as the engine sees it.
    fn java() -> Language {
        tree_sitter_java::LANGUAGE.into()
    }

    #[test]
    fn the_grammar_abi_is_inside_the_runtime_window() {
        let abi = java().abi_version();
        assert!(
            (MIN_COMPATIBLE_LANGUAGE_VERSION..=LANGUAGE_VERSION).contains(&abi),
            "tree-sitter-java reports ABI {abi}, outside [{MIN_COMPATIBLE_LANGUAGE_VERSION}, {LANGUAGE_VERSION}]"
        );
        assert_eq!(java_config().language().abi_version(), abi);
    }

    #[test]
    fn the_configuration_is_the_java_one() {
        let config = java_config();
        assert_eq!(config.comment_kinds(), ["line_comment", "block_comment"]);
        assert_eq!(
            config.protected_kinds(),
            ["string_literal", "character_literal"]
        );
        assert!(config.newline_sensitive());
    }

    #[test]
    fn every_configured_kind_is_validated_against_the_grammar() {
        let config = java_config();
        for &kind in config.comment_kinds() {
            assert!(
                LanguageConfig::new(java(), vec![kind], vec![], true).is_ok(),
                "{kind} should be a named kind of the grammar"
            );
        }
        for &kind in config.protected_kinds() {
            assert!(
                LanguageConfig::new(java(), vec![], vec![kind], true).is_ok(),
                "{kind} should be a named kind of the grammar"
            );
        }
        let err = LanguageConfig::new(java(), vec![], vec!["not_a_java_kind"], true).unwrap_err();
        assert_eq!(
            err.to_string(),
            "node kind not in this grammar: not_a_java_kind"
        );
    }

    #[test]
    fn text_block_is_refused_by_name_while_string_literal_is_accepted() {
        let err = LanguageConfig::new(java(), vec![], vec!["text_block"], true).unwrap_err();
        assert_eq!(err.to_string(), "node kind not in this grammar: text_block");
        assert!(LanguageConfig::new(java(), vec![], vec!["string_literal"], true).is_ok());
    }

    #[test]
    fn gos_comment_kind_is_refused() {
        let err = LanguageConfig::new(java(), vec!["comment"], vec![], true).unwrap_err();
        assert_eq!(err.to_string(), "node kind not in this grammar: comment");
    }

    #[test]
    fn the_configured_kinds_are_the_kinds_the_grammar_emits() {
        let tree = parse(
            &java_config(),
            b"/** doc */\nclass A {\n  // line\n  /* block */\n  String s = \"x\";\n  char c = 'y';\n}\n",
        )
        .unwrap();
        let sexp = tree.root_node().to_sexp();
        for kind in [
            "line_comment",
            "block_comment",
            "string_literal",
            "character_literal",
        ] {
            assert!(sexp.contains(kind), "{kind} missing from {sexp}");
        }
    }

    #[test]
    fn a_text_block_is_a_string_literal_with_multiline_string_fragment_children() {
        let tree = parse(
            &java_config(),
            b"class A {\n  String s = \"\"\"\n      one\n        two\n      \"\"\";\n}\n",
        )
        .unwrap();
        let sexp = tree.root_node().to_sexp();
        assert!(sexp.contains("string_literal"), "{sexp}");
        assert!(sexp.contains("multiline_string_fragment"), "{sexp}");
        assert!(!sexp.contains("text_block"), "{sexp}");
    }

    #[test]
    fn a_javadoc_block_is_a_block_comment() {
        let tree = parse(&java_config(), b"/**\n * Doc.\n */\nclass A {}\n").unwrap();
        let sexp = tree.root_node().to_sexp();
        assert!(sexp.contains("block_comment"), "{sexp}");
        for kind in ["javadoc", "doc_comment"] {
            assert!(
                !sexp.contains(kind),
                "{kind} unexpectedly present in {sexp}"
            );
        }
    }

    // --- Divergence pins -------------------------------------------------
    //
    // The engine's gate is a *syntax* gate, and Java's own toolchain does not
    // draw the line in the same place. These are the shapes for which the
    // external check (`crate::external`) has to run over the **original**
    // first: if it ran only on the output, a file this backend accepts and
    // reproduces faithfully would be reported as broken by `javac`.
    //
    // Which of them `javac` actually refuses was re-measured here against the
    // container's javac 21.0.10, rather than assumed — the four are easy to
    // group together as "divergence rows" without saying which side each
    // falls on:
    //
    // | source                          | tree-sitter | javac 21.0.10 |
    // | ------------------------------- | ----------- | ------------- |
    // | empty file                      | parses      | **exit 0**    |
    // | comments only                   | parses      | **exit 0**    |
    // | `int x = 1;` with no class      | parses      | exit 1        |
    // | unnamed variable `int _ = 1;`   | parses      | exit 1        |
    // | `STR."…"` template              | parses      | exit 1        |
    //
    // So the first two rows are **not** divergences — `javac` accepts an
    // empty and a comments-only compilation unit and writes no class file for
    // either. They are pinned anyway because it is easy to assume the
    // opposite (`gofmt` really does reject both, which is where the
    // assumption comes from). The last three are real, and all three are the
    // *version-skew* family: `javac` names them preview features disabled by
    // default, including the bare statement, which it reads as an unnamed
    // class rather than as a stray statement. An `--enable-preview` or
    // `--release` guess would only move the boundary, never remove it, which
    // is exactly why the policy is to check the original instead.

    #[test]
    fn an_empty_file_parses() {
        let tree = parse(&java_config(), b"").unwrap();
        assert_eq!(tree.root_node().kind(), "program");
        assert_eq!(tree.root_node().child_count(), 0);
    }

    #[test]
    fn a_comments_only_file_parses() {
        let tree = parse(&java_config(), b"// only a comment\n/* and a block */\n").unwrap();
        assert_eq!(tree.root_node().kind(), "program");
    }

    #[test]
    fn a_bare_statement_with_no_class_parses() {
        let tree = parse(&java_config(), b"int x = 1;\n").unwrap();
        assert_eq!(tree.root_node().kind(), "program");
    }

    #[test]
    fn preview_syntax_parses() {
        // An unnamed variable and a `STR."…"` string template: both are
        // clean trees here and both are `exit 1` to javac 21.
        for source in [
            &b"class A { void m() { int _ = 1; } }\n"[..],
            &b"class A { String s = STR.\"x\\{1}y\"; }\n"[..],
        ] {
            let tree = parse(&java_config(), source).unwrap();
            assert_eq!(tree.root_node().kind(), "program");
        }
    }

    #[test]
    fn a_nul_byte_between_tokens_is_a_parse_error() {
        // The Go/Java asymmetry, and the reason this is pinned separately
        // from the literal case below: `tree-sitter-go` *swallows* a NUL that
        // lands where the grammar is already willing to stop, so
        // `package main\0\n` parses there. `tree-sitter-java` refuses it.
        let err = parse(&java_config(), b"class A {}\x00\n").unwrap_err();
        assert!(matches!(err, Error::Parse(_)), "{err}");
    }

    #[test]
    fn a_nul_byte_inside_a_string_literal_is_a_parse_error() {
        // Refused here as well, so no NUL in either position ever reaches
        // the emitter — a grammar bump that started accepting one would hand
        // it bytes the toolchain will not read.
        let err = parse(&java_config(), b"class A { String s = \"a\x00b\"; }\n").unwrap_err();
        assert!(matches!(err, Error::Parse(_)), "{err}");
    }
}
