//! Verification: output that fails any check here is discarded by the caller
//! and never written.
//!
//! Two levels, mirroring the other language crates:
//!
//! - [`reparse`] — the output must parse at all, through
//!   [`crate::parser::parse`], which accepts only a parse that neither
//!   panicked nor produced a diagnostic.
//! - [`equivalent`] — **canonical re-emit comparison**: input and output are
//!   both rendered in the full-minify canonical form
//!   ([`crate::emit::canonical`]) and the two strings must be equal. oxc has
//!   no comparable-AST helper (the counterpart of
//!   `ruff_python_ast::comparable` used by `tokenpress-python`) and raw AST
//!   equality would compare spans, which formatting legitimately moves, so a
//!   canonical re-emit is the feasible substitute.
//!
//! # Known hole
//!
//! Canonicalization **erases comments**: the canonical form is comment-free
//! by construction, so a comment lost between input and output produces
//! identical canonical strings and passes [`equivalent`]. Comment loss is
//! therefore *not* caught by this module — comment policy is enforced by the
//! emitter (see [`crate::emit`]), not by the verifier.
//!
//! # Arena
//!
//! Re-parsing the output needs an [`Arena`], and the resulting program only
//! borrows from it (see the arena lifetime rule in [`crate::parser`]). Both
//! entry points therefore own a local arena and return only owned data, which
//! keeps verification free of lifetime coupling to the caller; this is why
//! [`reparse`] returns `()` where the Python and Rust crates return the
//! re-parsed module.
use std::path::Path;use crate::emit;use crate::parser::{self,Arena,Program};use tokenpress_core::{Error,Result};
/// Weakest level: the output must parse at all, as the dialect implied by
/// `path`.
pub fn reparse(path:&Path,code:&str)->Result<()>{let allocator=Arena::default();parse_output(&allocator,path,code)?;Ok(())}
/// Full check: the output must parse and must be canonically identical to
/// `original`.
pub fn equivalent(original:&Program<'_>,path:&Path,code:&str)->Result<()>{let allocator=Arena::default();let reparsed=parse_output(&allocator,path,code)?;if emit::canonical(original)!=emit::canonical(&reparsed){return Err(Error::Verification("output AST differs from input".to_string(),));}Ok(())}fn parse_output<'a>(allocator:&'a Arena,path:&Path,code:&'a str)->Result<Program<'a>>{parser::parse(allocator,path,code).map_err(|e|Error::Verification(format!("output failed to re-parse: {e}")))}#[cfg(test)]mod tests{use super::*;use crate::parser::{self,Arena};
/// Parses `source` and runs `equivalent` against `code`, both under the
/// dialect implied by `name`.
fn check(name:&str,source:&str,code:&str)->Result<()>{let allocator=Arena::default();let path=Path::new(name);let program=parser::parse(&allocator,path,source).unwrap();equivalent(&program,path,code)}#[test]fn reparse_accepts_valid_and_rejects_invalid_output(){assert!(reparse(Path::new("a.js"),"const a=1;").is_ok());let err=reparse(Path::new("a.js"),"function (").unwrap_err();assert!(err.to_string().contains("failed to re-parse"),"{err}");}#[test]fn reparse_rejects_recoverable_errors_too(){let err=reparse(Path::new("a.js"),"return 1;").unwrap_err();assert!(err.to_string().contains("failed to re-parse"),"{err}");}#[test]fn formatting_only_differences_are_equivalent(){assert!(check("a.js","function add( a , b ) {\n    return a + b;\n}\n","function add(a,b){return a+b}",).is_ok());}#[test]fn semantic_change_is_rejected(){let err=check("a.js","const a = 1 + 2;\n","const a=1-2;").unwrap_err();assert!(err.to_string().contains("differs from input"),"{err}");}#[test]fn dropped_typescript_annotation_is_rejected(){let err=check("a.ts","const a: number = 1;\n","const a=1;").unwrap_err();assert!(err.to_string().contains("differs from input"),"{err}");}#[test]fn unparsable_output_is_rejected_by_the_equivalence_check(){let err=check("a.js","const a = 1;\n","function (").unwrap_err();assert!(err.to_string().contains("failed to re-parse"),"{err}");}#[test]fn comment_loss_is_not_caught_by_the_verifier(){assert!(check("a.js","// note\nconst a = 1;\n","const a=1;").is_ok());}#[test]fn jsx_formatting_only_differences_are_equivalent(){assert!(check("a.jsx","const el = <>\n  <div className=\"box\" id={ id } { ...rest }>hi  there</div>\n</>;\n","const el=<>\n  <div className=\"box\" id={id}{...rest}>hi  there</div>\n</>;",).is_ok());}#[test]fn a_changed_jsx_attribute_is_rejected(){let err=check("a.jsx","const a = <div id={ x } />;\n","const a=<div id={y}/>;",).unwrap_err();assert!(err.to_string().contains("differs from input"),"{err}");}#[test]fn compressed_jsx_text_is_rejected(){let err=check("a.jsx","const a = <div>a  b</div>;\n","const a=<div>a b</div>;",).unwrap_err();assert!(err.to_string().contains("differs from input"),"{err}");}#[test]fn emptying_a_comment_only_container_passes_the_equivalence_check(){assert!(check("a.jsx","const a = <div>{/* c */}</div>;\n","const a=<div>{}</div>;").is_ok());}#[test]fn tsx_reparse_rejects_invalid_jsx_output(){let err=reparse(Path::new("a.tsx"),"const a=<div>hi;").unwrap_err();assert!(err.to_string().contains("failed to re-parse"),"{err}");}}