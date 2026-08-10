//! The only module allowed to touch `ruby-prism` APIs. The crate is pre-1.0
//! with no semver guarantees and — unlike oxc — declares no `rust-version` at
//! all, so it is pinned exactly in Cargo.toml. Exactly as CLAUDE.md requires
//! for the ruff pin in `tokenpress-python`, every `ruby_prism` type used by
//! this crate is either used here or re-exported from here, which keeps a
//! future parser swap cheap.
//!
//! # Borrow rule
//!
//! [`ruby_prism::parse`] returns a [`ParseResult<'_>`] that *borrows* the
//! source bytes it was given, so an owning wrapper struct (as
//! `tokenpress-python`'s `ParsedModule` is) cannot be written without making
//! the crate self-referential. Like `tokenpress-js`'s arena, the parse result
//! therefore stays a local of the function that parses, emits and verifies.
//!
//! # One dialect
//!
//! [`ruby_prism::parse`] takes no options at all: there is no Ruby-version,
//! filepath or encoding selector. Everything is parsed as the pinned prism
//! version's Ruby, which may accept syntax an older MRI rejects.
//!
//! # Bytes, not `str`
//!
//! prism parses `&[u8]`, and non-UTF-8 Ruby sources are legal — an
//! `# encoding:` magic comment selects how literals are read, and comments
//! may hold arbitrary bytes regardless. This module accepts them; any refusal
//! of non-UTF-8 input belongs at the `Formatter` boundary, whose contract is
//! `&str`. What it does *not* accept is a byte that is invalid in the source's
//! declared (by default UTF-8) encoding: that is a Ruby syntax error and is
//! reported as one.
use tokenpress_core::{Error,Result};pub use ruby_prism::{Comment,CommentType,Location,Node,ParseResult,Visit};
/// Parses `source` as Ruby.
///
/// prism has no `panicked` flag: a broken parse still yields a root node, so
/// the gate is the error list. Warnings are deliberately **not** a rejection
/// — valid Ruby routinely produces them (an unused variable, `if a = 1`), and
/// rejecting on them would refuse working files.
///
/// The returned result borrows `source`; see the module-level borrow rule.
pub fn parse(source:&[u8])->Result<ParseResult<'_>>{let parsed=ruby_prism::parse(source);let details=parsed.errors().map(|e|format!("byte {}: {}",e.location().start_offset(),e.message())).collect::<Vec<_>>();if!details.is_empty(){return Err(Error::Parse(details.join("; ")));}Ok(parsed)}#[cfg(test)]mod tests{use super::*;#[test]fn parses_valid_ruby(){let parsed=parse(b"def f(a, b)\n  a + b\nend\n").unwrap();assert_eq!(parsed.errors().count(),0);assert!(matches!(parsed.node(),Node::ProgramNode{..}));}#[test]fn warnings_do_not_reject(){let parsed=parse(b"a = 0\nif a = 1\nend\n").unwrap();assert!(parsed.warnings().count()>0);}#[test]fn an_unused_variable_also_only_warns(){let parsed=parse(b"def f\n  unused = 1\n  2\nend\n").unwrap();assert!(parsed.warnings().count()>0);}#[test]fn rejects_a_syntax_error(){let err=parse(b"def ; end").unwrap_err();assert!(matches!(err,Error::Parse(_)),"{err}");let message=err.to_string();assert!(message.starts_with("parse error: byte "),"{message}");assert!(message.len()>"parse error: byte 0: ".len(),"{message}");}#[test]fn rejects_an_incomplete_expression(){let err=parse(b"1 +").unwrap_err();assert!(matches!(err,Error::Parse(_)),"{err}");}#[test]fn accepts_non_utf8_sources(){for source in[b"# encoding: binary\nx = \"\xff\xfe\"\n".to_vec(),b"# encoding: shift_jis\nx = \"\x82\xa0\"\n".to_vec(),b"# \xe9\nx = 1\n".to_vec(),]{let parsed=parse(&source).unwrap();assert_eq!(parsed.errors().count(),0);}}#[test]fn a_stray_non_utf8_byte_without_a_magic_comment_is_a_parse_error(){let err=parse(b"x = \"\xff\xfe\"\n").unwrap_err();assert!(matches!(err,Error::Parse(_)),"{err}");}#[test]fn the_result_borrows_the_source(){let source=b"x = 1\n";let parsed=parse(source).unwrap();assert_eq!(parsed.source(),source);}}