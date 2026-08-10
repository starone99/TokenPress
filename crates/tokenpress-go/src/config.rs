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
pub fn go_config()->LanguageConfig{LanguageConfig::new(tree_sitter_go::LANGUAGE.into(),vec!["comment"],vec!["interpreted_string_literal","raw_string_literal","rune_literal",],true,).expect("every configured kind is a named node kind of the pinned tree-sitter-go grammar")}#[cfg(test)]mod tests{use super::*;use tokenpress_treesitter::parser::{parse,Language,LANGUAGE_VERSION,MIN_COMPATIBLE_LANGUAGE_VERSION,};use tokenpress_treesitter::Error;
/// The grammar as the engine sees it.
fn go()->Language{tree_sitter_go::LANGUAGE.into()}#[test]fn the_grammar_abi_is_inside_the_runtime_window(){let abi=go().abi_version();assert!((MIN_COMPATIBLE_LANGUAGE_VERSION..=LANGUAGE_VERSION).contains(&abi),"tree-sitter-go reports ABI {abi}, outside [{MIN_COMPATIBLE_LANGUAGE_VERSION}, {LANGUAGE_VERSION}]");assert_eq!(go_config().language().abi_version(),abi);}#[test]fn the_configuration_is_the_go_one(){let config=go_config();assert_eq!(config.comment_kinds(),["comment"]);assert_eq!(config.protected_kinds(),["interpreted_string_literal","raw_string_literal","rune_literal"]);assert!(config.newline_sensitive());}#[test]fn every_configured_kind_is_validated_against_the_grammar(){let config=go_config();for&kind in config.comment_kinds(){assert!(LanguageConfig::new(go(),vec![kind],vec![],true).is_ok(),"{kind} should be a named kind of the grammar");}for&kind in config.protected_kinds(){assert!(LanguageConfig::new(go(),vec![],vec![kind],true).is_ok(),"{kind} should be a named kind of the grammar");}let err=LanguageConfig::new(go(),vec![],vec!["text_block"],true).unwrap_err();assert_eq!(err.to_string(),"node kind not in this grammar: text_block");}#[test]fn the_configured_kinds_are_the_kinds_the_grammar_emits(){let tree=parse(&go_config(),b"package main\n\n// c\nvar a = \"x\"\nvar b = `y`\nvar r = 'z'\n",).unwrap();let sexp=tree.root_node().to_sexp();for kind in["comment","interpreted_string_literal","raw_string_literal","rune_literal",]{assert!(sexp.contains(kind),"{kind} missing from {sexp}");}}#[test]fn a_source_with_no_package_clause_parses(){let tree=parse(&go_config(),b"func f() int { return 1 }\n").unwrap();assert_eq!(tree.root_node().kind(),"source_file");}#[test]fn an_empty_file_parses(){let tree=parse(&go_config(),b"").unwrap();assert_eq!(tree.root_node().kind(),"source_file");assert_eq!(tree.root_node().child_count(),0);}#[test]fn a_nul_byte_inside_a_string_literal_is_a_parse_error(){for source in[b"package main\n\nvar s = \"a\x00b\"\n".to_vec(),b"package main\n\nvar s = `a\x00b`\n".to_vec(),b"package main\n\nvar r = '\x00'\n".to_vec(),]{let err=parse(&go_config(),&source).unwrap_err();assert!(matches!(err,Error::Parse(_)),"{err}");}}#[test]fn a_nul_byte_after_a_complete_statement_parses(){let tree=parse(&go_config(),b"package main\x00\n").unwrap();assert_eq!(tree.root_node().kind(),"source_file");}}