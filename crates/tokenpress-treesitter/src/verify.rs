//! Verification: output that fails any check here is discarded by the caller
//! and never written.
//!
//! Two levels, mirroring the other language crates:
//!
//! - [`reparse`] — the output must parse at all, through
//!   [`crate::parser::parse`], whose gate is `has_error()` and nothing else.
//! - [`equivalent`] — **comparable-artifact comparison**: the input and the
//!   output are both rendered by [`crate::comparable::comparable`] and the two
//!   artifacts must be equal. tree-sitter's own `to_sexp()` is unusable for
//!   this (see [`crate::comparable`]), so the hand-rolled artifact is what
//!   stands in for AST equality here.
//!
//! # The configuration comes first
//!
//! This engine is grammar-agnostic, so every entry point takes the
//! [`crate::parser::LanguageConfig`] as its first parameter, exactly as
//! [`crate::comparable::comparable`] and [`crate::emit::minimize_source`] do.
//! Verification is the same code for Go, Java, C# and PHP; only the
//! configuration differs.
//!
//! # Bytes, not `str`
//!
//! Both entry points take `&[u8]`: tree-sitter parses bytes and the emitter
//! produces `Vec<u8>`. Whether a language's sources are UTF-8 by definition is
//! a per-language claim, made at that crate's `Formatter` boundary, not here.
//!
//! # Owned results
//!
//! A [`crate::parser::Tree`] does not borrow its source (see
//! [`crate::parser`]), so returning one would be sound — but both entry points
//! still own their parse and return only owned data, because a caller that
//! wanted the tree would ask [`crate::parser::parse`] for it. That keeps this
//! module's contract to a yes/no, which is why [`reparse`] returns `()` where
//! the Python and Rust crates return the re-parsed module.
//!
//! # Why not [`crate::comparable::equivalent`]
//!
//! That helper returns `Result<bool>` and propagates a parse error from
//! *either* side, so a failure cannot be attributed to the output. This module
//! renders the two artifacts separately instead — the same deliberate
//! deviation `tokenpress-ruby` records: a parse failure of the **output** is
//! the verifier's own `output failed to re-parse: …` refusal, while a parse
//! failure of the **input** stays a [`tokenpress_core::Error::Parse`], because
//! it is the caller's source and not a candidate this module produced.
//! [`equivalent`] therefore re-parses the output itself, so the `Formatter`
//! wiring never needs a separate [`reparse`] call at the equivalence levels:
//! `Reparse` → [`reparse`], `AstEquiv`/`External` → [`equivalent`].
//!
//! # Comment changes are invisible here, by construction
//!
//! The comparable artifact skips every node whose kind is in
//! [`crate::parser::LanguageConfig::comment_kinds`], so a comment that changed
//! — or vanished entirely — produces the *same* artifact and passes
//! [`equivalent`]. That is not an oversight to be patched at this level: it is
//! what lets the comment stripper exist at all. Comment policy is consequently
//! verified
//! elsewhere and never here — by the per-language external gate (a comment
//! carrying a build constraint or a `//go:` directive changes what the
//! toolchain does with the file, which is exactly what running the toolchain
//! catches) and by the emitter's own tests. The blindness is a property of the
//! configuration rather than of this code: a [`crate::parser::LanguageConfig`]
//! with no comment kinds makes comments part of the artifact, and then a
//! comment-only change *is* refused. Both directions are pinned below.
//!
//! # Over-refusal
//!
//! Every leaf of a tree-sitter tree is a token, so no captured text spans
//! rewritten whitespace; see [`crate::comparable`] for the measurement over
//! the Go stdlib. This module adds no comparison of its own on top of the
//! artifact, so it inherits that result unchanged — and inherits its
//! over-refusals too. There **was** one class, contrary to what this doc used
//! to claim absolutely: a comment-only source and the empty source it strips
//! to rendered one space apart, so the correct empty output was refused. It is
//! fixed in [`crate::comparable`] and pinned there and in the three backends;
//! no other class is known, which is a statement about what has been measured
//! and not a proof.
use crate::comparable;use crate::parser::{self,LanguageConfig};use tokenpress_core::{Error,Result};
/// Weakest level: the output must parse at all under `config`.
///
/// This is a *syntax* gate, exactly as strict as [`crate::parser::parse`] and
/// no stricter: a language whose own tooling refuses sources the grammar
/// accepts needs the external check as well.
pub fn reparse(config:&LanguageConfig,output:&[u8])->Result<()>{parser::parse(config,output).map_err(reparse_failure)?;Ok(())}
/// Full check: the output must parse, and its comparable artifact must be
/// identical to `original`'s.
///
/// Returns [`tokenpress_core::Error::Parse`] when `original` itself does not
/// parse; every refusal of the candidate is a
/// [`tokenpress_core::Error::Verification`].
pub fn equivalent(config:&LanguageConfig,original:&[u8],output:&[u8])->Result<()>{let expected=comparable::comparable(config,original)?;let actual=comparable::comparable(config,output).map_err(reparse_failure)?;if expected!=actual{return Err(Error::Verification("output AST differs from input".to_string(),));}Ok(())}
/// Restates a parse failure of the *output* as the verifier's refusal.
fn reparse_failure(error:Error)->Error{Error::Verification(format!("output failed to re-parse: {error}"))}#[cfg(test)]mod tests{use super::*;use crate::emit::{self,CommentPolicy};use crate::parser::{Language,Tree};use std::ops::Range;
/// The dev-dependency grammar, converted from its `LanguageFn`.
fn go()->Language{tree_sitter_go::LANGUAGE.into()}
/// The configuration the first consumer ships.
fn go_config()->LanguageConfig{LanguageConfig::new(go(),vec!["comment"],vec!["interpreted_string_literal","raw_string_literal","rune_literal",],true,).unwrap()}
/// The stripping policy's type. Plain `fn` pointers rather than closures,
/// as in `crate::emit`'s tests.
type StripPolicy=CommentPolicy<fn(&[u8])->bool,fn(&Tree,&[u8])->Range<usize>,fn(&Tree,&[u8])->bool,>;fn never_keep(_comment:&[u8])->bool{false}fn no_prologue(_tree:&Tree,_source:&[u8])->Range<usize>{0..0}fn never_bail(_tree:&Tree,_source:&[u8])->bool{false}
/// The most aggressive policy there is: every comment goes. The real Go
/// predicates are G2; verification does not depend on which comments a
/// policy keeps.
fn delete_every_comment()->StripPolicy{CommentPolicy::new(never_keep,no_prologue,never_bail)}
/// Real Go snippets, each with something the emitters have to get right.
const CORPUS:&[(&str,&[u8])]=&[("empty source",b""),("package clause only",b"package main\n"),("a function with an expression",b"package main\n\nfunc f(a int) int { return a + 1 }\n",),("strings and runes",b"package main\n\nfunc f() { g(\"a  b\", `raw\n\n  body`, 'q') }\n",),("line and block comments",b"package main\n\n// note\n/* block\n   more */\nfunc f() {} // trailing\n",),("a build constraint and a directive",b"//go:build ignore\n\npackage main\n\n//go:generate echo hi\nfunc f() {}\n",),("a struct with tags",b"package main\n\ntype T struct {\n\tA    int    `json:\"a\"`\n\tBcde string `json:\"b\"`\n}\n",),("imports and a method",b"package main\n\nimport (\n\t\"fmt\"\n\t\"os\"\n)\n\nfunc (t *T) f() {\n\tfmt.Fprintln(os.Stdout, \"hi\")\n}\n",),("generics and a channel",b"package main\n\nfunc f[T comparable](c <-chan T) {}\n",),("non-ascii identifiers and text","package main\n\nvar \u{307B} = \"\u{3042}  \u{3044}\"\n".as_bytes(),),];#[test]fn reparse_accepts_valid_and_rejects_invalid_output(){let config=go_config();assert!(reparse(&config,b"package main\n\nfunc f() {}\n").is_ok());let err=reparse(&config,b"package main\n\nfunc f( {\n").unwrap_err();assert!(matches!(err,Error::Verification(_)),"{err}");assert!(err.to_string().starts_with("verification failed: output failed to re-parse: "),"{err}");}#[test]fn reparse_is_a_syntax_gate_and_nothing_more(){let config=go_config();assert!(reparse(&config,b"func f() {}\n").is_ok());assert!(reparse(&config,b"").is_ok());}#[test]fn a_minimized_source_is_equivalent_to_its_input(){let config=go_config();let source=b"package main\n\nfunc f(a int, b int) int {\n\treturn a + b\n}\n";let output=emit::minimize_source(&config,source).unwrap();assert!(output.len()<source.len());assert!(reparse(&config,&output).is_ok());assert!(equivalent(&config,source,&output).is_ok());}#[test]fn a_stripped_source_is_equivalent_to_its_input(){let config=go_config();let source=b"package main\n\n// leading note\nfunc f() {} // trailing note\n";let output=emit::strip_comments_source(&config,source,&delete_every_comment()).unwrap();assert!(!output.windows(4).any(|window|window==b"note"));assert!(reparse(&config,&output).is_ok());assert!(equivalent(&config,source,&output).is_ok());}#[test]fn semantic_changes_are_rejected(){let config=go_config();for(name,original,output)in[("literal value",&b"var x = 1\n"[..],&b"var x = 2\n"[..]),("operator",b"func f() int { return 1 + 2 }\n",b"func f() int { return 1 - 2 }\n",),("identifier",b"func f() {}\n",b"func g() {}\n"),("quote style",b"var x = \"a\"\n",b"var x = `a`\n"),]{let err=equivalent(&config,original,output).unwrap_err();assert!(matches!(err,Error::Verification(_)),"{name}: {err}");assert_eq!(err.to_string(),"verification failed: output AST differs from input","{name}");}}#[test]fn unparsable_output_is_rejected_by_the_equivalence_check(){let config=go_config();let err=equivalent(&config,b"package main\n",b"func f( {\n").unwrap_err();assert!(matches!(err,Error::Verification(_)),"{err}");assert!(err.to_string().starts_with("verification failed: output failed to re-parse: "),"{err}");}#[test]fn an_unparsable_input_is_a_parse_error_not_a_refusal(){let config=go_config();let err=equivalent(&config,b"func f( {\n",b"package main\n").unwrap_err();assert!(matches!(err,Error::Parse(_)),"{err}");assert!(err.to_string().starts_with("parse error: "),"{err}");}#[test]fn comment_only_changes_are_invisible_by_construction(){let config=go_config();let source=b"package main\n\n// one\nfunc f() {}\n";assert!(equivalent(&config,source,b"package main\n\n// two\nfunc f() {}\n").is_ok());assert!(equivalent(&config,source,b"package main\n\nfunc f() {}\n").is_ok());let stripped=emit::strip_comments_source(&config,source,&delete_every_comment()).unwrap();assert!(equivalent(&config,source,&stripped).is_ok());assert!(equivalent(&config,b"//go:build ignore\n\npackage main\n",b"package main\n").is_ok());}#[test]fn a_configuration_without_comment_kinds_refuses_a_comment_change(){let config=LanguageConfig::new(go(),vec![],vec![],true).unwrap();let err=equivalent(&config,b"package main\n\n// one\nfunc f() {}\n",b"package main\n\n// two\nfunc f() {}\n",).unwrap_err();assert_eq!(err.to_string(),"verification failed: output AST differs from input");}#[test]fn every_corpus_source_survives_both_emitters(){let config=go_config();let policy=delete_every_comment();for(name,source)in CORPUS{for(emitter,output)in[("minimize",emit::minimize_source(&config,source).unwrap()),("strip comments",emit::strip_comments_source(&config,source,&policy).unwrap(),),]{let name=format!("{emitter}: {name}");let rendered=String::from_utf8_lossy(&output);assert!(reparse(&config,&output).is_ok(),"{name}: {rendered}");assert!(equivalent(&config,source,&output).is_ok(),"{name}: {rendered}");}}}#[test]fn verification_is_on_bytes_not_on_text(){let config=go_config();let source="package main\n\nvar \u{307B} = \"\u{3042}\"\n".as_bytes();let output=emit::minimize_source(&config,source).unwrap();assert!(equivalent(&config,source,&output).is_ok());assert!(equivalent(&config,source,"package main\n\nvar \u{307B} = \"\u{3044}\"\n".as_bytes()).is_err());}}