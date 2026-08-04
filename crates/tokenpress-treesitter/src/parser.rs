//! The only module allowed to touch `tree_sitter` APIs. The runtime is
//! pinned exactly (see Cargo.toml) because it is coupled to the generated
//! parser ABI of every grammar it drives, exactly as CLAUDE.md requires for
//! the ruff pin in `tokenpress-python`, the oxc pin in `tokenpress-js` and
//! the prism pin in `tokenpress-ruby`. Every `tree_sitter` type the rest of
//! the workspace needs is re-exported from here, so no other module — in
//! this crate or in a per-language crate — has to name `tree_sitter`.
//!
//! # Grammars are configuration, not dependencies
//!
//! A [`Language`] is a runtime value and node kinds are plain strings, so one
//! engine can drive every grammar. [`LanguageConfig`] is that configuration:
//! the grammar plus the node kinds this engine has to treat specially. Its
//! constructor validates **every** kind name against the grammar through
//! [`Language::id_for_node_kind`], which returns `0` for a kind the grammar
//! does not have — so a typo (or a kind borrowed from a sibling language,
//! like Java's `text_block` in a Go config) is a construction error naming
//! the offending kind, not a literal that silently never matches a node.
//! Comment and protected kinds are *named* kinds, so the lookup passes
//! `named = true`.
//!
//! # No borrow rule
//!
//! Unlike `ruby-prism`'s `ParseResult<'_>` and oxc's arena, a tree-sitter
//! [`Tree`] is **owned and does not borrow the source**: `Node<'tree>`
//! borrows the tree, not the byte buffer. [`parse`] therefore returns an
//! owned `Tree`, and parse → emit → verify need *not* be squeezed into one
//! function scope. Do not copy the single-scope workaround the two earlier
//! backends need.
//!
//! # The error gate is `has_error()`, and nothing else
//!
//! tree-sitter is error-recovering: it never fails, it produces a tree with
//! `ERROR` and `MISSING` nodes in it. So the rejection predicate had to be
//! established empirically rather than read off an API. It is
//! `tree.root_node().has_error()` alone: validated on a 24-case matrix and
//! by scanning all 7,117 Go 1.24.7 stdlib files, in which every tree with
//! `has_error() == false` contains no `is_error()` node and no `is_missing()`
//! node — `is_missing` is subsumed, confirmed directly by `f(1, 2 }`, which
//! recovers with a `MISSING` node, no `ERROR` node, and
//! `has_error() == true`. Checking `is_missing` separately would therefore
//! add a branch that cannot fire.
//!
//! Note that this gate is a *syntax* gate: like every other backend's, it is
//! not a type checker, and a language whose own tooling is stricter than its
//! grammar (Go's `gofmt` rejects sources this accepts) needs the external
//! check to run on the original first.

use tokenpress_core::{Error, Result};

pub use tree_sitter::{Language, Node, Tree, LANGUAGE_VERSION, MIN_COMPATIBLE_LANGUAGE_VERSION};

/// A node kind name that the configured grammar does not have.
///
/// Deliberately *not* a [`tokenpress_core::Error`]: no variant of the shared
/// error describes a configuration mistake, and this one is a programming
/// error caught by a per-language crate's own tests — it never reaches a
/// user. `tokenpress-cli`'s `ConfigError` is the same pattern.
#[derive(Debug, thiserror::Error)]
#[error("node kind not in this grammar: {0}")]
pub struct UnknownNodeKind(String);

/// A grammar plus the node kinds this engine treats specially.
#[derive(Debug)]
pub struct LanguageConfig {
    language: Language,
    comment_kinds: Vec<&'static str>,
    protected_kinds: Vec<&'static str>,
    newline_sensitive: bool,
}

impl LanguageConfig {
    /// Builds a configuration, rejecting any kind name the grammar does not
    /// have.
    ///
    /// `comment_kinds` are the kinds the equivalence artifact ignores and the
    /// comment policy acts on; `protected_kinds` are the kinds whose bytes
    /// are copied verbatim (string and character literals, in every language
    /// so far). `newline_sensitive` says whether the emitter has to preserve
    /// the source's line structure. Automatic semicolon insertion (Go) makes
    /// it true, but it is not the only thing that does: **Java sets it true
    /// as well**, even though it is a brace-and-semicolon language with no
    /// ASI, because the `false` branch collapses the newline *after* a
    /// `line_comment` to a space and the comment then swallows the line below
    /// it. Measured at Java's default settings (comments kept), `false`
    /// refuses 247 of 500 apache/commons-lang 3.17.0 files. So the flag is
    /// about line structure, not about ASI, and a language answers it by
    /// measurement rather than by family.
    pub fn new(
        language: Language,
        comment_kinds: Vec<&'static str>,
        protected_kinds: Vec<&'static str>,
        newline_sensitive: bool,
    ) -> std::result::Result<Self, UnknownNodeKind> {
        for kind in comment_kinds.iter().chain(protected_kinds.iter()) {
            // Both lists hold *named* kinds; `id_for_node_kind` answers 0 for
            // a kind the grammar does not have.
            if language.id_for_node_kind(kind, true) == 0 {
                return Err(UnknownNodeKind((*kind).to_string()));
            }
        }
        Ok(Self {
            language,
            comment_kinds,
            protected_kinds,
            newline_sensitive,
        })
    }

    /// The grammar this configuration was built against.
    pub fn language(&self) -> &Language {
        &self.language
    }

    /// The comment node kinds, validated against the grammar.
    pub fn comment_kinds(&self) -> &[&'static str] {
        &self.comment_kinds
    }

    /// The node kinds whose bytes must survive verbatim.
    pub fn protected_kinds(&self) -> &[&'static str] {
        &self.protected_kinds
    }

    /// Whether a newline in the source can change the meaning of the program.
    pub fn newline_sensitive(&self) -> bool {
        self.newline_sensitive
    }
}

/// Parses `source` with the configured grammar.
///
/// Rejects exactly what `root_node().has_error()` rejects; see the module
/// doc for why that predicate, and only that predicate, is the gate. The
/// returned [`Tree`] is owned and outlives `source`.
pub fn parse(config: &LanguageConfig, source: &[u8]) -> Result<Tree> {
    let tree = parse_tree(config, source);
    if tree.root_node().has_error() {
        return Err(Error::Parse(format!(
            "syntax error at byte {}",
            deepest_error_offset(tree.root_node())
        )));
    }
    Ok(tree)
}

/// The parse itself, without the gate.
///
/// tree-sitter never fails, so this always yields a tree — possibly one
/// carrying `ERROR` or `MISSING` nodes, which is precisely what [`parse`]
/// refuses to hand out.
fn parse_tree(config: &LanguageConfig, source: &[u8]) -> Tree {
    let mut parser = tree_sitter::Parser::new();
    // Fails only when the grammar's ABI is outside the pinned runtime's
    // window, which is a version-pin mistake rather than a runtime condition:
    // every grammar pin ships an `abi_version()` test (see below, and the
    // per-language crates) whose whole job is to catch it at `cargo test`
    // time.
    parser
        .set_language(&config.language)
        .expect("the grammar's ABI is inside the pinned runtime's window");
    // `None` means no language was set or the parse was cancelled: the
    // language is set immediately above, and neither a timeout nor a
    // cancellation flag is ever configured, so there is no such tree.
    parser
        .parse(source, None)
        .expect("a language is set and the parse is never cancelled")
}

/// The ungated parse, for this crate's tests only.
///
/// `src/comparable.rs` renders a `<MISSING>` marker so its walk is total, but
/// [`parse`] gates on `has_error()` and every `MISSING` node sets it, so that
/// branch is unreachable through the public API. Rather than weaken the gate
/// or leave the branch untested, the tests reach it through here — and
/// nothing outside `cfg(test)` can.
#[cfg(test)]
pub(crate) fn parse_ungated(config: &LanguageConfig, source: &[u8]) -> Tree {
    parse_tree(config, source)
}

/// The start byte of the innermost node whose subtree carries the error.
///
/// Descends through erroring children for as long as there is one; the node
/// it stops on is the most precise location the tree offers. Only ever
/// called on a node that already reports `has_error()`.
fn deepest_error_offset(root: Node) -> usize {
    let mut node = root;
    loop {
        let mut cursor = node.walk();
        let erroring = node.children(&mut cursor).find(|child| child.has_error());
        match erroring {
            Some(child) => node = child,
            None => return node.start_byte(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn parses_a_valid_source() {
        let tree = parse(
            &go_config(),
            b"package main\n\nfunc f(a int) int { return a }\n",
        )
        .unwrap();
        assert_eq!(tree.root_node().kind(), "source_file");
        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn rejects_a_syntax_error() {
        let err = parse(&go_config(), b"package main\n\nfunc f( {\n").unwrap_err();
        assert!(matches!(err, Error::Parse(_)), "{err}");
        let message = err.to_string();
        // The offset has to be the *inner* one the descent found, not the
        // root's 0.
        assert!(
            message.starts_with("parse error: syntax error at byte "),
            "{message}"
        );
        assert!(!message.ends_with("byte 0"), "{message}");
    }

    #[test]
    fn a_missing_node_alone_is_rejected_by_the_same_gate() {
        // `has_error()` subsumes `is_missing()`: this recovers with a MISSING
        // node rather than an ERROR node, and is still refused. Pinning it
        // here is why the gate needs no second predicate.
        let err = parse(&go_config(), b"package main\n\nfunc f() { g(1, 2 }\n").unwrap_err();
        assert!(matches!(err, Error::Parse(_)), "{err}");
    }

    #[test]
    fn an_error_at_the_root_reports_the_root_offset() {
        // A source whose only defect is at the very start: the descent stops
        // immediately, exercising the loop's exit on its first pass.
        let err = parse(&go_config(), b"}").unwrap_err();
        assert_eq!(err.to_string(), "parse error: syntax error at byte 0");
    }

    #[test]
    fn an_unknown_comment_kind_is_rejected_by_name() {
        // `text_block` is a Java kind, not a Go one — the exact class of
        // mistake this validation exists for.
        let err =
            LanguageConfig::new(go(), vec!["comment", "text_block"], vec![], true).unwrap_err();
        assert_eq!(err.to_string(), "node kind not in this grammar: text_block");
        assert!(format!("{err:?}").contains("text_block"), "{err:?}");
    }

    #[test]
    fn an_unknown_protected_kind_is_rejected_by_name() {
        let err = LanguageConfig::new(go(), vec![], vec!["text_block"], true).unwrap_err();
        assert_eq!(err.to_string(), "node kind not in this grammar: text_block");
    }

    #[test]
    fn an_anonymous_kind_is_not_accepted_as_a_named_one() {
        // `{` exists in the Go grammar, but only as an anonymous node, and
        // both kind lists are named-node lists.
        let err = LanguageConfig::new(go(), vec![], vec!["{"], true).unwrap_err();
        assert_eq!(err.to_string(), "node kind not in this grammar: {");
    }

    #[test]
    fn the_grammar_abi_is_inside_the_runtime_window() {
        // Every grammar pin needs this test: the runtime accepts a window of
        // ABI versions, and grammar crates do not move in lockstep with it.
        // Asserting the window, not a number, is what makes it survive a
        // deliberate version bump while still catching an incompatible one.
        let abi = go().abi_version();
        assert!(
            (MIN_COMPATIBLE_LANGUAGE_VERSION..=LANGUAGE_VERSION).contains(&abi),
            "tree-sitter-go reports ABI {abi}, outside [{MIN_COMPATIBLE_LANGUAGE_VERSION}, {LANGUAGE_VERSION}]"
        );
    }

    #[test]
    fn the_config_exposes_what_it_was_built_with() {
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
        assert!(config.newline_sensitive());
        assert_eq!(config.language().abi_version(), go().abi_version());
        assert!(format!("{config:?}").starts_with("LanguageConfig"));
    }

    #[test]
    fn empty_kind_lists_and_newline_insensitivity_are_configurations_too() {
        // The Java/C#/PHP shape: nothing to protect yet, newlines carry no
        // meaning. Neither is a special case in the engine.
        let config = LanguageConfig::new(go(), vec![], vec![], false).unwrap();
        assert!(config.comment_kinds().is_empty());
        assert!(config.protected_kinds().is_empty());
        assert!(!config.newline_sensitive());
        // Still a working parser: the kind lists are policy, not parsing.
        assert!(parse(&config, b"package main\n").is_ok());
    }

    #[test]
    fn the_tree_does_not_borrow_the_source() {
        // Pins the API-shape claim in the module doc: the source buffer is
        // dropped here, and the tree is still usable.
        let tree = {
            let source = b"package main\n".to_vec();
            parse(&go_config(), &source).unwrap()
        };
        assert_eq!(tree.root_node().kind(), "source_file");
    }
}
