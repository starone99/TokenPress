//! The Go grammar as engine configuration — the only module that names
//! `tree_sitter_go`.
//!
//! This crate's grammar boundary, and the analogue of the sibling backends'
//! `parser.rs` under CLAUDE.md's confinement rule (ruff in
//! `tokenpress-python`, oxc in `tokenpress-js`, prism in `tokenpress-ruby`).
//! It is called `config` rather than `parser` because no parsing happens
//! here: a grammar is a runtime value the engine takes as configuration, so
//! all this module does is name the kinds and hand
//! `tree_sitter_go::LANGUAGE` to the engine. Parsing itself stays in
//! `tokenpress_treesitter::parser`.

use tokenpress_treesitter::parser::LanguageConfig;

/// The Go configuration the engine drives.
///
/// - comment kind `comment` — Go has one, covering `//` and `/* */` alike.
/// - protected kinds `interpreted_string_literal`, `raw_string_literal`,
///   `rune_literal` — every kind whose bytes carry meaning of their own and
///   must be copied verbatim.
/// - newline-sensitive, because Go's automatic semicolon insertion makes a
///   newline a statement terminator.
///
/// Building the configuration is cheap (a `Language` handle plus two small
/// vectors), so callers construct one per operation rather than sharing one.
pub fn go_config() -> LanguageConfig {
    LanguageConfig::new(
        // The grammar crate exports a `LanguageFn`; the engine's re-exported
        // `Language` is what `LanguageConfig` wants, and the conversion is
        // the only reason this crate depends on a grammar at all.
        tree_sitter_go::LANGUAGE.into(),
        vec!["comment"],
        vec![
            "interpreted_string_literal",
            "raw_string_literal",
            "rune_literal",
        ],
        true,
    )
    // `LanguageConfig::new` rejects a kind name the grammar does not have.
    // This list is fixed and correct, so the error cannot occur — and rather
    // than propagate a `Result` no caller could ever act on, the constructor
    // is unwrapped here, exactly as the engine unwraps `set_language` for its
    // sibling impossibility (the ABI window). What keeps that honest is the
    // same thing: tests below that assert each kind against the grammar, so a
    // grammar bump that renames one fails `cargo test` rather than a user's
    // run.
    .expect("every configured kind is a named node kind of the pinned tree-sitter-go grammar")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokenpress_treesitter::parser::{
        parse, Language, LANGUAGE_VERSION, MIN_COMPATIBLE_LANGUAGE_VERSION,
    };
    use tokenpress_treesitter::Error;

    /// The grammar as the engine sees it.
    fn go() -> Language {
        tree_sitter_go::LANGUAGE.into()
    }

    #[test]
    fn the_grammar_abi_is_inside_the_runtime_window() {
        // Every grammar pin owes this test: the runtime accepts a window of
        // ABI versions and grammar crates do not move in lockstep with it.
        // Asserting the window rather than a number survives a deliberate
        // bump while still catching an incompatible one.
        let abi = go().abi_version();
        assert!(
            (MIN_COMPATIBLE_LANGUAGE_VERSION..=LANGUAGE_VERSION).contains(&abi),
            "tree-sitter-go reports ABI {abi}, outside [{MIN_COMPATIBLE_LANGUAGE_VERSION}, {LANGUAGE_VERSION}]"
        );
        // And the configuration really carries that grammar, not another.
        assert_eq!(go_config().language().abi_version(), abi);
    }

    #[test]
    fn the_configuration_is_the_go_one() {
        let config = go_config();
        assert_eq!(config.comment_kinds(), ["comment"]);
        assert_eq!(
            config.protected_kinds(),
            [
                "interpreted_string_literal",
                "raw_string_literal",
                "rune_literal"
            ]
        );
        // Automatic semicolon insertion: a newline can terminate a statement.
        assert!(config.newline_sensitive());
    }

    #[test]
    fn every_configured_kind_is_validated_against_the_grammar() {
        // The constructor is the validation, so exercise it kind by kind:
        // each of the four names on its own has to be accepted...
        let config = go_config();
        for &kind in config.comment_kinds() {
            assert!(
                LanguageConfig::new(go(), vec![kind], vec![], true).is_ok(),
                "{kind} should be a named kind of the grammar"
            );
        }
        for &kind in config.protected_kinds() {
            assert!(
                LanguageConfig::new(go(), vec![], vec![kind], true).is_ok(),
                "{kind} should be a named kind of the grammar"
            );
        }
        // ...and a name the grammar does not have has to be refused, or the
        // check above would prove nothing.
        let err = LanguageConfig::new(go(), vec![], vec!["text_block"], true).unwrap_err();
        assert_eq!(err.to_string(), "node kind not in this grammar: text_block");
    }

    #[test]
    fn the_configured_kinds_are_the_kinds_the_grammar_emits() {
        // Existing in the grammar is not the same as being produced for the
        // syntax this backend cares about, so pin the node kinds a real
        // source yields.
        let tree = parse(
            &go_config(),
            b"package main\n\n// c\nvar a = \"x\"\nvar b = `y`\nvar r = 'z'\n",
        )
        .unwrap();
        let sexp = tree.root_node().to_sexp();
        for kind in [
            "comment",
            "interpreted_string_literal",
            "raw_string_literal",
            "rune_literal",
        ] {
            assert!(sexp.contains(kind), "{kind} missing from {sexp}");
        }
    }

    // --- Divergence pins -------------------------------------------------
    //
    // The engine's gate is a *syntax* gate, and Go's own tooling is stricter
    // than its grammar. These are the cases that prove the external check
    // (G5) has to run on the **original** first: if it ran only on the
    // output, a file this backend accepts and reproduces faithfully would be
    // reported as broken by `gofmt`.

    #[test]
    fn a_source_with_no_package_clause_parses() {
        // `gofmt` rejects this; the grammar does not.
        let tree = parse(&go_config(), b"func f() int { return 1 }\n").unwrap();
        assert_eq!(tree.root_node().kind(), "source_file");
    }

    #[test]
    fn an_empty_file_parses() {
        let tree = parse(&go_config(), b"").unwrap();
        assert_eq!(tree.root_node().kind(), "source_file");
        assert_eq!(tree.root_node().child_count(), 0);
    }

    #[test]
    fn a_nul_byte_inside_a_string_literal_is_a_parse_error() {
        // Measured, under tree-sitter-go 0.25.0 and against the toolchain:
        // an embedded NUL byte is *rejected* in both string kinds and in a
        // rune literal, and `gofmt -e` rejects the same three sources with
        // "illegal character NUL" (go1.24.7) — the implementation restriction
        // the Go spec allows. This is therefore **not** a divergence; it is
        // pinned because it is easy to assume the opposite, and because a
        // grammar bump that started accepting NUL would hand the emitter
        // bytes no Go compiler will read.
        for source in [
            b"package main\n\nvar s = \"a\x00b\"\n".to_vec(),
            b"package main\n\nvar s = `a\x00b`\n".to_vec(),
            b"package main\n\nvar r = '\x00'\n".to_vec(),
        ] {
            let err = parse(&go_config(), &source).unwrap_err();
            assert!(matches!(err, Error::Parse(_)), "{err}");
        }
    }

    #[test]
    fn a_nul_byte_after_a_complete_statement_parses() {
        // The asymmetry the NUL pin above needs beside it: a NUL that lands
        // where the grammar is already willing to stop is swallowed, so this
        // parses — while `gofmt -e` refuses it ("expected ';', found
        // 'ILLEGAL'"). It is a divergence of the same family as the two
        // above, and the likely origin of the older "embedded NUL parses"
        // claim: it holds for a NUL *between* tokens, not inside a literal.
        let tree = parse(&go_config(), b"package main\x00\n").unwrap();
        assert_eq!(tree.root_node().kind(), "source_file");
    }
}
