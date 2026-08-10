//! TokenPress for Ruby — the Ruby backend, built on `ruby-prism`.
//!
//! Pipeline: parse ([`parser`]) → whitespace-minimal re-emit over the source
//! bytes ([`emit`]) → verification ([`verify`], plus [`external`] at
//! [`VerifyLevel::External`]) → token accounting. The path is only used to
//! decide whether this backend claims the file ([`paths`]); prism has no
//! dialect, filepath or version selector, so [`RubyFormatter::format`] never
//! reads it.
//!
//! # Comment reality
//!
//! Unlike `tokenpress-js`, nothing is dropped behind the caller's back: the
//! emitter rewrites the whitespace *between* protected source spans and
//! copies everything else verbatim, so at the default settings **every
//! comment survives, byte for byte** — leading, trailing and inline alike,
//! embdocs included.
//!
//! `RubyOptions::strip_comments` is the opt-in that deletes them: every
//! comment and embdoc goes, except the ones inside the magic-comment window
//! (everything before the first code token — so the shebang and
//! `# frozen_string_literal: true` always survive) and the ones that are not
//! comments at all but text inside a string, a heredoc body or the `__END__`
//! data section. See [`emit`] for the whole policy.
//!
//! # Whitespace reality
//!
//! Minimization removes indentation, trailing whitespace and blank lines, and
//! collapses every other run of spaces and tabs to **exactly one space** —
//! never to zero, because `a - b` is a subtraction while `a -b` is a call
//! with a unary-minus argument. Newlines are statement terminators in Ruby
//! and are kept, so the output has the line structure of the input with the
//! formatting whitespace gone. The savings come from indentation and blank
//! lines, not from gluing tokens together; see [`emit`] for why elision is
//! not attempted.
//!
//! # Non-UTF-8 sources are refused, by the contract
//!
//! Ruby sources need not be valid UTF-8 — an `# encoding:` magic comment
//! picks how literals are read, and both [`parser`] and [`verify`] work on
//! `&[u8]` and accept such files. [`tokenpress_core::Formatter::format`],
//! however, takes `&str`, so a non-UTF-8 file can never reach this formatter:
//! whoever reads it from disk refuses it first. This crate therefore never
//! sees, and never has to decide about, invalid UTF-8.
//!
//! # Verification
//!
//! `Reparse` re-parses the output; `AstEquiv` compares the comparable
//! artifacts of input and output (which re-parses the output too, so no
//! separate re-parse is needed). `External` runs the `AstEquiv` check and
//! then hands the output to Ruby itself (`ruby -c`), which must be on PATH;
//! see [`external`] for what that covers and what it requires. Output that
//! fails is discarded with [`Error::Verification`] and never returned.
//!
//! # Parser boundary
//!
//! `ruby-prism` is pinned **exactly** (`=1.9.0`): it is pre-1.0 with no
//! semver guarantees and declares no `rust-version`, so there is not even an
//! MSRV signal to read and any upgrade is a toolchain decision. Every
//! `ruby_prism` type and function this crate uses is therefore used inside
//! [`parser`] or re-exported from it — no other module may name `ruby_prism`,
//! exactly as CLAUDE.md requires for the ruff pin in `tokenpress-python` and
//! the oxc pin in `tokenpress-js`.
//!
//! A parsed result borrows the source bytes it was handed and cannot be
//! wrapped in an owning struct, so parse, emit and verify all run inside
//! [`RubyFormatter::format`]; see [`parser`].
//!
//! # Build prerequisite
//!
//! `ruby-prism-sys` compiles the vendored prism C sources and generates its
//! bindings with bindgen, so building this crate needs a C compiler **and**
//! libclang. Ruby itself is not needed at build time.
pub mod comparable;pub mod emit;pub mod external;pub mod parser;pub mod paths;pub mod verify;use std::path::Path;use tokenpress_core::{FormatOptions,FormatResult,Formatter,VerifyLevel};pub use tokenpress_core::{Error,Result};
/// Ruby-specific choices.
#[derive(Clone,Debug,Default)]pub struct RubyOptions{
/// RBO1: drop comments and embdocs. The default (`false`) keeps every one
/// of them verbatim — comments are context for LLMs, so stripping is the
/// opt-in. The magic-comment window is preserved either way; see the
/// comment policy at the crate level.
pub strip_comments:bool,}pub struct RubyFormatter{options:RubyOptions,}impl RubyFormatter{pub fn new(options:RubyOptions)->Self{Self{options}}}impl Default for RubyFormatter{fn default()->Self{Self::new(RubyOptions::default())}}impl Formatter for RubyFormatter{fn language(&self)->&'static str{"ruby"}fn supports(&self,path:&Path)->bool{paths::supports_path(path)}
/// `path` is not read: prism has no dialect, filepath or version
/// selector, so the source is the whole input. It stays in the signature
/// because the trait's other implementations need it.
fn format(&self,_path:&Path,source:&str,options:&FormatOptions)->Result<FormatResult>{let bytes=source.as_bytes();let parsed=parser::parse(bytes)?;let emitted=if self.options.strip_comments{let plan=emit::strip_comments_plan(&parsed);emit::strip_comments(bytes,&plan,emit::minimize)}else{let spans=emit::protected_spans(&parsed);emit::rewrite(bytes,&spans,emit::minimize)};let code=String::from_utf8_lossy(&emitted).into_owned();match options.verify{VerifyLevel::Reparse=>{verify::reparse(code.as_bytes())?;}VerifyLevel::AstEquiv=>{verify::equivalent(bytes,code.as_bytes())?;}VerifyLevel::External=>{verify::equivalent(bytes,code.as_bytes())?;external::check(source,&code)?;}}let tokenizer=options.tokenizer.load()?;Ok(FormatResult{original_tokens:tokenizer.count(source),formatted_tokens:tokenizer.count(&code),code,})}}#[cfg(test)]mod tests{use super::*;use tokenpress_core::TokenizerKind;fn fmt(source:&str)->String{fmt_with(source,RubyOptions::default())}fn fmt_with(source:&str,options:RubyOptions)->String{RubyFormatter::new(options).format(Path::new("a.rb"),source,&FormatOptions::default()).unwrap().code}#[test]fn language_is_ruby(){assert_eq!(RubyFormatter::default().language(),"ruby");}#[test]fn supports_the_ruby_paths(){let f=RubyFormatter::default();for name in["a.rb","tasks.rake","tokenpress.gemspec","config.ru","Gemfile","sub/dir/Rakefile",]{assert!(f.supports(Path::new(name)),"{name} should be supported");}for name in["a.py","a.rs","a.txt","gemfile","Gemfile.lock","rb"]{assert!(!f.supports(Path::new(name)),"{name} should be rejected");}}#[test]fn rb01_minimizes_whitespace(){let source="def add(a, b)\n    sum  =  a + b\n\n\n    sum\nend\n";assert_eq!(fmt(source),"def add(a, b)\nsum = a + b\nsum\nend\n");}#[test]fn token_counts_are_reported(){let source="def add(a, b)\n    sum = a + b\n    sum\nend\n";let r=RubyFormatter::default().format(Path::new("a.rb"),source,&FormatOptions::default()).unwrap();assert!(r.original_tokens>0);assert!(r.formatted_tokens>0);assert!(r.formatted_tokens<r.original_tokens);assert!(r.tokens_saved()>0);}#[test]fn the_path_is_not_read(){let r=RubyFormatter::default().format(Path::new("notes.txt"),"x  =  1\n",&FormatOptions::default(),).unwrap();assert_eq!(r.code,"x = 1\n");}#[test]fn rbo1_defaults_to_keeping_comments(){assert!(!RubyOptions::default().strip_comments);}#[test]fn rbo1_keeps_every_comment_by_default(){let source="=begin\nblock\n=end\n# leading\nx  =  1  # trailing\n";assert_eq!(fmt(source),"=begin\nblock\n=end\n# leading\nx = 1 # trailing\n");}#[test]fn rbo1_strips_comments_on_request(){let source="x  =  1  # trailing\n=begin\nblock\n=end\n# leading\ny = 2\n";assert_eq!(fmt(source),"x = 1 # trailing\n=begin\nblock\n=end\n# leading\ny = 2\n");assert_eq!(fmt_with(source,RubyOptions{strip_comments:true}),"x = 1\ny = 2\n");}#[test]fn rbo1_keeps_the_magic_comment_window_when_stripping(){let source="#!/usr/bin/env ruby\n# frozen_string_literal: true\n\n# licence\ndef f(a)\n  a  # tail\nend\n";assert_eq!(fmt_with(source,RubyOptions{strip_comments:true}),"#!/usr/bin/env ruby\n# frozen_string_literal: true\n# licence\ndef f(a)\na\nend\n");}#[test]fn reparse_only_level_also_passes(){let opts=FormatOptions{verify:VerifyLevel::Reparse,..FormatOptions::default()};let r=RubyFormatter::default().format(Path::new("a.rb"),"x  =  1\n",&opts).unwrap();assert_eq!(r.code,"x = 1\n");}#[test]fn ast_equiv_is_the_default_level(){let opts=FormatOptions{verify:VerifyLevel::AstEquiv,..FormatOptions::default()};let r=RubyFormatter::default().format(Path::new("a.rb"),"x  =  1 + 2\n",&opts).unwrap();assert_eq!(r.code,"x = 1 + 2\n");}fn external()->FormatOptions{FormatOptions{verify:VerifyLevel::External,..FormatOptions::default()}}#[test]fn external_level_adds_the_ruby_syntax_check(){let r=RubyFormatter::default().format(Path::new("a.rb"),"x  =  1 + 2\n",&external()).unwrap();assert_eq!(r.code,"x = 1 + 2\n");let err=RubyFormatter::default().format(Path::new("a.rb"),"a[1,\n  2]\n",&external()).unwrap_err();assert!(matches!(err,Error::Verification(_)),"{err}");}#[test]fn external_level_does_not_blame_input_ruby_already_rejects(){let r=RubyFormatter::default().format(Path::new("a.rb"),"x  =  /[z-a]/\n",&external()).unwrap();assert_eq!(r.code,"x = /[z-a]/\n");}#[test]fn parse_errors_are_reported(){let err=RubyFormatter::default().format(Path::new("broken.rb"),"def ; end",&FormatOptions::default(),).unwrap_err();assert!(matches!(err,Error::Parse(_)),"{err}");assert!(err.to_string().starts_with("parse error: byte "),"{err}");}#[test]fn a_multiline_index_call_is_a_known_over_refusal(){let err=RubyFormatter::default().format(Path::new("a.rb"),"a[1,\n  2]\n",&FormatOptions::default()).unwrap_err();assert_eq!(err.to_string(),"verification failed: output AST differs from input");}#[test]fn formatting_is_idempotent(){let sources=["def add(a, b)\n    sum = a + b\n    sum\nend\n","# note\nx  =  1\n","class A\n  def b(c)\n    s = \"a#{ c } b\"\n    xs = %w[a   b]\n    xs.each do |x|\n      p x   # note\n    end\n\n\n    s\n  end\nend\n","x = <<~A\n    hi\n  A\ny  =  2\n","x = 1\n__END__\n  data  here\n",];for source in sources{let once=fmt(source);assert_eq!(fmt(&once),once,"not idempotent for {source:?}");let stripped=fmt_with(source,RubyOptions{strip_comments:true,},);assert_eq!(fmt_with(&stripped,RubyOptions{strip_comments:true}),stripped,"not idempotent when stripping for {source:?}");}}#[test]fn tokenizer_choice_is_respected(){let opts=FormatOptions{tokenizer:TokenizerKind::Cl100kBase,..FormatOptions::default()};let r=RubyFormatter::default().format(Path::new("a.rb"),"x  =  1\n",&opts).unwrap();assert!(r.original_tokens>0);}}