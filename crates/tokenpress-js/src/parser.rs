//! The only module allowed to touch oxc APIs. The oxc crates are pre-1.0
//! with no semver guarantees (pinned exactly in Cargo.toml), so — exactly as
//! CLAUDE.md requires for the ruff pin in `tokenpress-python` — every oxc
//! type and function used by this crate is either used here or re-exported
//! from here, which keeps a future parser swap cheap.
//!
//! # Arena lifetime rule
//!
//! oxc parses into a bump [`Allocator`] that *owns* the AST; the returned
//! [`Program<'a>`] only borrows from it. A parsed program can therefore never
//! outlive its allocator, and an owning `Parsed` struct (as
//! `tokenpress-python` uses) would be self-referential. The allocator is
//! consequently created by the caller and passed in, so the whole
//! parse → emit → verify pipeline runs inside a single function with one
//! local arena that is dropped when that function returns.
use std::path::Path;use oxc_allocator::Allocator;use oxc_parser::Parser;use oxc_span::SourceType;use tokenpress_core::{Error,Result};pub use oxc_allocator::Allocator as Arena;pub use oxc_ast::{ast,ast::Program};pub use oxc_span::SourceType as Dialect;
/// Parses `source` as the dialect implied by `path`'s extension.
///
/// The dialect comes from [`SourceType::from_path`], which accepts
/// `.js`/`.mjs`/`.cjs`/`.jsx` (JavaScript) and
/// `.ts`/`.mts`/`.cts`/`.tsx`/`.d.ts` (TypeScript) and rejects anything else.
///
/// oxc's parser is *error-recovering*: it returns a usable program for many
/// malformed inputs, and `panicked` is only set for unrecoverable failures.
/// Input is therefore accepted only when the parser neither panicked **nor**
/// reported any diagnostic — a top-level `return 1;` is the canonical case
/// that passes the first check and fails the second.
///
/// The returned program borrows from `allocator`; see the module-level arena
/// lifetime rule.
pub fn parse<'a>(allocator:&'a Allocator,path:&Path,source:&'a str)->Result<Program<'a>>{let source_type=SourceType::from_path(path).map_err(|_|Error::UnsupportedLanguage(path.display().to_string()))?;let parsed=Parser::new(allocator,source,source_type).parse();if parsed.panicked||!parsed.diagnostics.is_empty(){let details=parsed.diagnostics.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ");return Err(Error::Parse(format!("{}: {details}",path.display())));}Ok(parsed.program)}#[cfg(test)]mod tests{use super::*;use oxc_allocator::Allocator;use std::path::Path;fn ok(name:&str,source:&str){let allocator=Allocator::default();let program=parse(&allocator,Path::new(name),source).unwrap();assert!(!program.body.is_empty(),"{name} produced an empty program");}#[test]fn parses_plain_javascript(){ok("a.js","const a = 1;\n");}#[test]fn parses_typescript_interface(){ok("a.ts","interface A { b: string }\n");}#[test]fn parses_jsx(){ok("a.jsx","const a = <div className=\"x\">hi</div>;\n");}#[test]fn parses_tsx(){ok("a.tsx","const a = (b: string): JSX.Element => <p>{b}</p>;\n",);}#[test]fn parses_declaration_file(){ok("a.d.ts","declare function f(x: number): void;\n");}#[test]fn parses_module_and_commonjs_extensions(){ok("a.mjs","export const a = 1;\n");ok("a.cjs","module.exports = 1;\n");ok("a.mts","export const a: number = 1;\n");ok("a.cts","const a: number = 1;\n");}#[test]fn rejects_unknown_extension(){let allocator=Allocator::default();let err=parse(&allocator,Path::new("notes.txt"),"const a = 1;\n").unwrap_err();assert_eq!(err.to_string(),"unsupported language for path: notes.txt");}#[test]fn rejects_syntax_error_that_panics_the_parser(){let allocator=Allocator::default();let err=parse(&allocator,Path::new("broken.js"),"function (").unwrap_err();let message=err.to_string();assert!(message.starts_with("parse error: broken.js:"),"{message}");assert!(message.len()>"parse error: broken.js: ".len(),"{message}");}#[test]fn rejects_recoverable_error_without_panic(){let allocator=Allocator::default();let err=parse(&allocator,Path::new("toplevel.js"),"return 1;\n").unwrap_err();let message=err.to_string();assert!(message.starts_with("parse error: toplevel.js:"),"{message}");assert!(message.contains("return"),"{message}");}}