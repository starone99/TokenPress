//! WebAssembly bindings for TokenPress.
//!
//! The Python, Rust, JavaScript/TypeScript, Go, Java and C# formatters are
//! exposed. Each `#[wasm_bindgen]` export ([`format_python_json`],
//! [`format_rust_json`], [`format_js_json`], [`format_go_json`],
//! [`format_java_json`], [`format_csharp_json`]) is a JSON-in/JSON-out
//! delegation; every decision lives in the plain functions below, so the whole
//! crate is exercised — and covered — by ordinary host tests.
//!
//! The core invariant is preserved across the boundary: when a formatter
//! refuses output (parse failure, or output that fails verification) the
//! caller receives a structured `{"kind", "message"}` error and no code at
//! all. Partial or unverified output is never returned.
//!
//! Verification here is always *internal* ([`VERIFY`]): a wasm module cannot
//! spawn processes, so the external level — which hands the output to the
//! language's own toolchain — is never selected.
//!
//! Rust output carries the documented MVP caveats of `tokenpress-rust`
//! (non-doc `//` comments are dropped, macro-body whitespace is minimized),
//! and JavaScript/TypeScript output those of `tokenpress-js` (trailing and
//! expression-position comments are dropped, JSX text is never compressed).
//! The library reports no warnings, so neither does this boundary; callers
//! state the caveats themselves.
//!
//! Go's caveat set is a different shape: at the default settings it is
//! **context-lossless** — every comment survives byte for byte, trailing and
//! inline ones included — and [`WasmGoOptions::strip_comments`] is the lossy
//! opt-in. Even then the comments the Go toolchain reads as instructions are
//! kept: `//go:` directives, `line` directives (in both their `//` and block
//! comment forms) and build constraints (`//go:build` and the legacy
//! `// +build`). Three rules hold at *both* settings, because
//! they are whitespace rules rather than deletion rules: an indented
//! directive-shaped comment is never moved to column 0 (where the toolchain
//! would start obeying it), a build-constraint prologue is reproduced verbatim
//! (blank lines included), and a file that imports `"C"` is left byte for byte
//! identical and so reports no savings at all.
//!
//! Java's caveat set is the same shape as Go's with **less to defend**. The
//! defaults are context-lossless in the same way — every comment survives
//! byte for byte, whitespace only is minimized — and
//! [`WasmJavaOptions::strip_comments`] is the lossy opt-in. But `javac` reads
//! nothing out of a comment, so there is no keep-list: the opt-in deletes
//! **every** comment including Javadoc, which is an ordinary block comment to
//! the grammar. There is likewise no promotion rule and no verbatim prologue,
//! because Java has no column-sensitive comment syntax and no file-header
//! construct that reaches the compiler. One unconditional rule remains, the
//! analogue of Go's cgo bail-out: `javac` decodes a `\uXXXX` escape before
//! lexing (JLS 3.3) and tree-sitter-java does not, so a file where that
//! asymmetry could bite is left byte for byte identical at both settings and
//! reports no savings at all.
//!
//! C#'s caveat set is Java's shape with **one more thing to lose**. The
//! defaults are context-lossless in the same way — with
//! [`WasmCSharpOptions::strip_comments`] off, every comment survives byte for
//! byte and only whitespace is minimized — and the flag is the lossy opt-in,
//! with no keep-list, because the compiler reads nothing out of a comment
//! under the invocation the CLI's external level uses. What it deletes
//! includes **XML documentation comments**: C# has a single comment node kind,
//! so a documentation comment written with three leading slashes is
//! indistinguishable by kind from an ordinary line comment and goes with the
//! rest. The unconditional rule, the analogue of Go's cgo bail-out and Java's
//! escape bail-out, is about where a comment *ends*: a comment crossing a
//! conditional-compilation boundary, one carrying a line terminator the
//! grammar does not honour (U+0085, U+2028, U+2029), or one shielding a
//! preprocessor directive from the start of its line leaves the file byte for
//! byte identical at both settings, with no savings at all.
use std::path::Path;use serde::de::DeserializeOwned;use serde::Deserialize;use serde_json::{json,Map,Value};use tokenpress_core::{Error,FormatOptions,FormatResult,Formatter,TokenizerKind,VerifyLevel};use tokenpress_csharp::{CSharpFormatter,CSharpOptions};use tokenpress_go::{GoFormatter,GoOptions};use tokenpress_java::{JavaFormatter,JavaOptions};use tokenpress_js::{JsFormatter,JsOptions};use tokenpress_python::{PythonFormatter,PythonOptions};use tokenpress_rust::{RustFormatter,RustOptions};use wasm_bindgen::prelude::wasm_bindgen;
/// The tokenizers every successful result is priced against, in report order.
/// Both vocabularies are embedded in the binary, so neither needs I/O.
const REPORTED_TOKENIZERS:[(&str,TokenizerKind);2]=[("o200k_base",TokenizerKind::O200kBase),("cl100k_base",TokenizerKind::Cl100kBase),];
/// The verification level every export runs at, spelled out rather than left
/// to `FormatOptions::default()`.
///
/// It must never become [`VerifyLevel::External`]: that level runs the
/// language's own toolchain in a child process (`tsc --noEmit`, falling back
/// to `node --check`, for JavaScript/TypeScript; `gofmt -e` for Go; `javac`
/// stopped after its parse phase for Java; Roslyn's `csc`, reached through
/// `dotnet`, for C#), and a wasm module cannot spawn processes — every call
/// would fail.
/// [`VerifyLevel::AstEquiv`] is the strongest level that is purely
/// in-process: the output is re-parsed and compared with the input for
/// equivalence.
const VERIFY:VerifyLevel=VerifyLevel::AstEquiv;
/// Python formatting flags accepted at the boundary.
///
/// Deserialized from a JSON object; every field is optional and falls back to
/// the library default (`merge_imports` on, all stripping off). Unknown fields
/// are rejected so a misspelled flag fails loudly instead of being ignored.
#[derive(Clone,Debug,Deserialize)]#[serde(default,deny_unknown_fields)]pub struct WasmPythonOptions{pub strip_comments:bool,pub strip_docstrings:bool,pub strip_annotations:bool,pub merge_imports:bool,}impl Default for WasmPythonOptions{fn default()->Self{let PythonOptions{strip_comments,strip_docstrings,strip_annotations,merge_imports,}=PythonOptions::default();Self{strip_comments,strip_docstrings,strip_annotations,merge_imports,}}}impl From<&WasmPythonOptions>for PythonOptions{fn from(options:&WasmPythonOptions)->Self{Self{strip_comments:options.strip_comments,strip_docstrings:options.strip_docstrings,strip_annotations:options.strip_annotations,merge_imports:options.merge_imports,}}}
/// Rust formatting flags accepted at the boundary.
///
/// Deserialized like [`WasmPythonOptions`]: every field optional, unknown
/// fields rejected.
#[derive(Clone,Debug,Deserialize)]#[serde(default,deny_unknown_fields)]pub struct WasmRustOptions{pub strip_doc_comments:bool,}impl Default for WasmRustOptions{fn default()->Self{let RustOptions{strip_doc_comments}=RustOptions::default();Self{strip_doc_comments}}}impl From<&WasmRustOptions>for RustOptions{fn from(options:&WasmRustOptions)->Self{Self{strip_doc_comments:options.strip_doc_comments,}}}
/// Which JavaScript/TypeScript dialect the source is parsed and re-emitted as.
///
/// `tokenpress-js` takes its dialect from the file's extension, and there is
/// no file system behind this boundary, so the caller names the dialect and it
/// is mapped to a synthetic path here. The variants are exactly the extensions
/// the formatter accepts, so module kind (`mjs` = ESM, `cjs` = script) is
/// selectable as well as the four syntax dialects.
///
/// Deserialized from a lowercase string; an unrecognised name is rejected with
/// a message naming every accepted value, matching the `deny_unknown_fields`
/// strictness of the options objects.
#[derive(Clone,Copy,Debug,Default,Deserialize,PartialEq)]#[serde(rename_all="lowercase")]pub enum WasmJsDialect{
/// Plain JavaScript — the default, so `"{}"` means `.js` and TypeScript or
/// JSX syntax has to be asked for explicitly rather than guessed at.
#[default]Js,Mjs,Cjs,Jsx,Ts,Mts,Cts,Tsx,}impl WasmJsDialect{
/// The synthetic path that selects this dialect. Never touched on disk.
fn path(self)->&'static Path{Path::new(match self{Self::Js=>"input.js",Self::Mjs=>"input.mjs",Self::Cjs=>"input.cjs",Self::Jsx=>"input.jsx",Self::Ts=>"input.ts",Self::Mts=>"input.mts",Self::Cts=>"input.cts",Self::Tsx=>"input.tsx",})}}
/// JavaScript/TypeScript formatting flags accepted at the boundary.
///
/// Deserialized like [`WasmPythonOptions`]: every field optional, unknown
/// fields rejected. `dialect` is the boundary's own field — it has no library
/// counterpart because the library reads the dialect from the path.
#[derive(Clone,Debug,Deserialize)]#[serde(default,deny_unknown_fields)]pub struct WasmJsOptions{pub strip_comments:bool,pub dialect:WasmJsDialect,}impl Default for WasmJsOptions{fn default()->Self{let JsOptions{strip_comments}=JsOptions::default();Self{strip_comments,dialect:WasmJsDialect::default(),}}}impl From<&WasmJsOptions>for JsOptions{fn from(options:&WasmJsOptions)->Self{Self{strip_comments:options.strip_comments,}}}
/// Go formatting flags accepted at the boundary.
///
/// Deserialized like [`WasmPythonOptions`]: every field optional, unknown
/// fields rejected.
#[derive(Clone,Debug,Deserialize)]#[serde(default,deny_unknown_fields)]pub struct WasmGoOptions{pub strip_comments:bool,}impl Default for WasmGoOptions{fn default()->Self{let GoOptions{strip_comments}=GoOptions::default();Self{strip_comments}}}impl From<&WasmGoOptions>for GoOptions{fn from(options:&WasmGoOptions)->Self{Self{strip_comments:options.strip_comments,}}}
/// Java formatting flags accepted at the boundary.
///
/// Deserialized like [`WasmPythonOptions`]: every field optional, unknown
/// fields rejected.
#[derive(Clone,Debug,Deserialize)]#[serde(default,deny_unknown_fields)]pub struct WasmJavaOptions{pub strip_comments:bool,}impl Default for WasmJavaOptions{fn default()->Self{let JavaOptions{strip_comments}=JavaOptions::default();Self{strip_comments}}}impl From<&WasmJavaOptions>for JavaOptions{fn from(options:&WasmJavaOptions)->Self{Self{strip_comments:options.strip_comments,}}}
/// C# formatting flags accepted at the boundary.
///
/// Deserialized like [`WasmPythonOptions`]: every field optional, unknown
/// fields rejected.
#[derive(Clone,Debug,Deserialize)]#[serde(default,deny_unknown_fields)]pub struct WasmCSharpOptions{pub strip_comments:bool,}impl Default for WasmCSharpOptions{fn default()->Self{let CSharpOptions{strip_comments}=CSharpOptions::default();Self{strip_comments}}}impl From<&WasmCSharpOptions>for CSharpOptions{fn from(options:&WasmCSharpOptions)->Self{Self{strip_comments:options.strip_comments,}}}
/// What one tokenizer charges for the input and for the formatted output.
#[derive(Clone,Debug,PartialEq)]pub struct WasmTokenStats{
/// Tokenizer name, e.g. `"o200k_base"`.
pub tokenizer:&'static str,pub original:usize,pub formatted:usize,pub saved:usize,
/// Fraction of the input's tokens saved, in `0.0..=1.0`; zero-token input
/// saves 0.0.
pub saving_ratio:f64,}impl WasmTokenStats{
/// Prices one source/output pair with one tokenizer.
///
/// The counting and the saved/ratio arithmetic both come from the core
/// crate, so the boundary cannot drift from what the CLI reports.
fn measure(tokenizer:&'static str,kind:&TokenizerKind,source:&str,code:&str,)->Result<Self,WasmError>{let counter=kind.load()?;let counted=FormatResult{code:String::new(),original_tokens:counter.count(source),formatted_tokens:counter.count(code),};Ok(Self{tokenizer,original:counted.original_tokens,formatted:counted.formatted_tokens,saved:counted.tokens_saved(),saving_ratio:counted.saving_ratio(),})}
/// Renders as `{"formatted", "original", "saved", "saving_ratio"}`.
fn to_value(&self)->Value{json!({"original":self.original,"formatted":self.formatted,"saved":self.saved,"saving_ratio":self.saving_ratio,})}}
/// A successful formatting run: verified output, whether it differs from the
/// input, and what it costs under each embedded tokenizer.
#[derive(Clone,Debug,PartialEq)]pub struct WasmFormatOutput{pub code:String,pub changed:bool,pub tokens:Vec<WasmTokenStats>,}impl WasmFormatOutput{
/// Renders as `{"changed": bool, "code": string, "tokens": {<tokenizer>:
/// {"original", "formatted", "saved", "saving_ratio"}}}`.
pub fn to_json(&self)->String{let tokens:Map<String,Value> =self.tokens.iter().map(|stats|(stats.tokenizer.to_string(),stats.to_value())).collect();json!({"code":self.code,"changed":self.changed,"tokens":tokens}).to_string()}}
/// A structured failure: a machine-readable `kind` plus a human-readable
/// `message`. Never carries code.
#[derive(Clone,Debug,PartialEq)]pub struct WasmError{pub kind:String,pub message:String,}impl WasmError{fn new(kind:&str,message:impl Into<String>)->Self{Self{kind:kind.to_string(),message:message.into(),}}
/// Renders as `{"kind": string, "message": string}`.
pub fn to_json(&self)->String{json!({"kind":self.kind,"message":self.message}).to_string()}}impl From<Error>for WasmError{fn from(err:Error)->Self{Self::new(error_kind(&err),err.to_string())}}
/// Stable, machine-readable name for each core error.
fn error_kind(err:&Error)->&'static str{match err{Error::Parse(_)=>"parse",Error::Verification(_)=>"verification",Error::UnsupportedLanguage(_)=>"unsupported_language",Error::UnknownTokenizer(_)=>"unknown_tokenizer",Error::Io(_)=>"io",}}
/// Parses the JSON options object.
fn parse_options<T:DeserializeOwned>(options_json:&str)->Result<T,WasmError>{serde_json::from_str(options_json).map_err(|err|WasmError::new("options",err.to_string()))}
/// Runs one formatter and prices the result against every reported tokenizer.
///
/// `path` is synthetic — there is no file system behind this boundary. It only
/// tells the formatter which dialect to apply, so each caller passes it right
/// where it picks the formatter and the two cannot disagree.
///
/// Returns [`WasmError`] — and no code — whenever the formatter refuses the
/// result, so unverified output cannot reach the caller.
fn run(formatter:&dyn Formatter,path:&Path,source:&str,)->Result<WasmFormatOutput,WasmError>{let options=FormatOptions{verify:VERIFY,..FormatOptions::default()};let result=formatter.format(path,source,&options)?;let tokens=REPORTED_TOKENIZERS.iter().map(|(name,kind)|WasmTokenStats::measure(name,kind,source,&result.code)).collect::<Result<Vec<_>,WasmError>>()?;Ok(WasmFormatOutput{changed:result.code!=source,code:result.code,tokens,})}
/// Formats Python source with the given flags.
pub fn format_python(source:&str,options:&WasmPythonOptions,)->Result<WasmFormatOutput,WasmError>{run(&PythonFormatter::new(options.into()),Path::new("input.py"),source,)}
/// Formats Rust source with the given flags.
pub fn format_rust(source:&str,options:&WasmRustOptions)->Result<WasmFormatOutput,WasmError>{run(&RustFormatter::new(options.into()),Path::new("input.rs"),source,)}
/// Formats JavaScript/TypeScript source with the given flags.
///
/// The dialect in `options` picks the synthetic path, so the caller cannot
/// select a dialect the formatter would not apply.
pub fn format_js(source:&str,options:&WasmJsOptions)->Result<WasmFormatOutput,WasmError>{run(&JsFormatter::new(options.into()),options.dialect.path(),source,)}
/// Formats Go source with the given flags.
pub fn format_go(source:&str,options:&WasmGoOptions)->Result<WasmFormatOutput,WasmError>{run(&GoFormatter::new(options.into()),Path::new("input.go"),source,)}
/// Formats Java source with the given flags.
pub fn format_java(source:&str,options:&WasmJavaOptions)->Result<WasmFormatOutput,WasmError>{run(&JavaFormatter::new(options.into()),Path::new("Input.java"),source,)}
/// Formats C# source with the given flags.
pub fn format_csharp(source:&str,options:&WasmCSharpOptions,)->Result<WasmFormatOutput,WasmError>{run(&CSharpFormatter::new(options.into()),Path::new("Input.cs"),source,)}
/// Renders either outcome as the JSON the JavaScript side sees.
fn to_json_result(outcome:Result<WasmFormatOutput,WasmError>)->Result<String,String>{match outcome{Ok(output)=>Ok(output.to_json()),Err(err)=>Err(err.to_json()),}}
/// Formats Python source.
///
/// `options_json` is a JSON object with the optional boolean flags
/// `strip_comments`, `strip_docstrings`, `strip_annotations` and
/// `merge_imports`; pass `"{}"` for the defaults.
///
/// Resolves to `{"changed": bool, "code": string, "tokens": {...}}` (see
/// [`WasmFormatOutput::to_json`]), or rejects with `{"kind": string,
/// "message": string}` where `kind` is one of `options`, `parse`,
/// `verification`, `unsupported_language`, `unknown_tokenizer` or `io`.
#[wasm_bindgen(js_name=formatPython)]pub fn format_python_json(source:&str,options_json:&str)->Result<String,String>{to_json_result(parse_options(options_json).and_then(|options|format_python(source,&options)))}
/// Formats Rust source.
///
/// `options_json` is a JSON object with the optional boolean flag
/// `strip_doc_comments`; pass `"{}"` for the defaults. Resolves and rejects
/// exactly like [`format_python_json`].
#[wasm_bindgen(js_name=formatRust)]pub fn format_rust_json(source:&str,options_json:&str)->Result<String,String>{to_json_result(parse_options(options_json).and_then(|options|format_rust(source,&options)))}
/// Formats JavaScript/TypeScript source.
///
/// `options_json` is a JSON object with the optional boolean flag
/// `strip_comments` and the optional string `dialect`, one of `js`, `mjs`,
/// `cjs`, `jsx`, `ts`, `mts`, `cts` or `tsx`; pass `"{}"` for the defaults
/// (`js`, comments kept). An unknown dialect is refused with `kind`
/// `options`, like any other malformed flag. Resolves and rejects exactly
/// like [`format_python_json`].
///
/// Verification is **internal only** — re-parse plus canonical re-emit
/// equivalence. A wasm module cannot spawn processes, so the external level
/// (`tsc --noEmit` / `node --check`, which the CLI offers as
/// `--verify external`) is not available here and is never used; see
/// [`VERIFY`].
///
/// Comments: output is not comment-preserving. Only leading statement-level,
/// jsdoc, annotation and legal comments survive; trailing and
/// expression-position comments are always dropped, and `strip_comments`
/// removes everything except legal and annotation comments. JSX text is
/// emitted verbatim, so `.jsx`/`.tsx` saves tokens only around the markup.
#[wasm_bindgen(js_name=formatJs)]pub fn format_js_json(source:&str,options_json:&str)->Result<String,String>{to_json_result(parse_options(options_json).and_then(|options|format_js(source,&options)))}
/// Formats Go source.
///
/// `options_json` is a JSON object with the optional boolean flag
/// `strip_comments`; pass `"{}"` for the defaults. Resolves and rejects
/// exactly like [`format_python_json`].
///
/// Verification is **internal only** — re-parse plus AST equivalence. A wasm
/// module cannot spawn processes, so the external level (`gofmt -e`, which the
/// CLI offers as `--verify external`) is not available here and is never used;
/// see [`VERIFY`].
///
/// Comments: at the defaults the output is comment-preserving — every comment
/// survives byte for byte. `strip_comments` is the lossy opt-in, and even then
/// the comments the Go toolchain reads as instructions are kept: `//go:`
/// directives, `line` directives (in both their `//` and block comment forms)
/// and build constraints (`//go:build` and the legacy `// +build`). A file
/// that imports `"C"` is left byte for byte identical at either setting, so it
/// reports no savings at all.
///
/// The block comment form is spelled out in words rather than shown, on
/// purpose: `wasm-bindgen` copies this text into a JSDoc block in the
/// generated glue, and a literal comment terminator inside it would close that
/// block early and emit unparsable JavaScript. See the guard test.
#[wasm_bindgen(js_name=formatGo)]pub fn format_go_json(source:&str,options_json:&str)->Result<String,String>{to_json_result(parse_options(options_json).and_then(|options|format_go(source,&options)))}
/// Formats Java source.
///
/// `options_json` is a JSON object with the optional boolean flag
/// `strip_comments`; pass `"{}"` for the defaults. Resolves and rejects
/// exactly like [`format_python_json`].
///
/// Verification is **internal only** — re-parse plus AST equivalence. A wasm
/// module cannot spawn processes, so the external level (`javac` stopped
/// after its parse phase, which the CLI offers as `--verify external`) is not
/// available here and is never used; see [`VERIFY`].
///
/// Comments: at the defaults the output is comment-preserving — every comment
/// survives byte for byte, trailing and inline ones included, and only
/// whitespace is minimized. `strip_comments` is the lossy opt-in, and unlike
/// Go's it keeps nothing back: `javac` reads nothing out of a comment, so
/// there is no keep-list, and the Javadoc blocks a file carries are deleted
/// with every other comment. Whichever setting is chosen, a file whose
/// comments carry a unicode escape that could decode into a comment marker is
/// left byte for byte identical and reports no savings at all.
///
/// The Javadoc and block comment markers are spelled out in words rather than
/// shown, on purpose: `wasm-bindgen` copies this text into a JSDoc block in
/// the generated glue, and a literal comment terminator inside it would close
/// that block early and emit unparsable JavaScript. See the guard test.
#[wasm_bindgen(js_name=formatJava)]pub fn format_java_json(source:&str,options_json:&str)->Result<String,String>{to_json_result(parse_options(options_json).and_then(|options|format_java(source,&options)))}
/// Formats C# source.
///
/// `options_json` is a JSON object with the optional boolean flag
/// `strip_comments`; pass `"{}"` for the defaults. Resolves and rejects
/// exactly like [`format_python_json`].
///
/// Verification is **internal only** — re-parse plus AST equivalence. A wasm
/// module cannot spawn processes, so the external level (Roslyn's own
/// compiler, reached through the dotnet driver, which the CLI offers as
/// `--verify external`) is not available here and is never used; see
/// [`VERIFY`].
///
/// Comments: at the defaults the output is comment-preserving — every comment
/// survives byte for byte, trailing and inline ones included, and only
/// whitespace is minimized. `strip_comments` is the lossy opt-in, and like
/// Java's it keeps nothing back — with one thing more to lose. C# has a
/// **single** comment node kind, so an XML documentation comment, written with
/// three leading slashes, is indistinguishable by kind from an ordinary line
/// comment and is deleted with the rest: the API documentation of a stripped
/// file goes with its comments. Whichever setting is chosen, a file where the
/// grammar and a real compiler could disagree about where a comment ends — a
/// comment crossing a conditional-compilation boundary, one carrying a line
/// terminator the grammar does not honour, or one shielding a directive from
/// the start of its line — is left byte for byte identical and reports no
/// savings at all.
///
/// The doc comment marker and the block comment markers are spelled out in
/// words rather than shown, on purpose: `wasm-bindgen` copies this text into a
/// JSDoc block in the generated glue, and a literal comment terminator inside
/// it would close that block early and emit unparsable JavaScript. See the
/// guard test.
#[wasm_bindgen(js_name=formatCSharp)]pub fn format_csharp_json(source:&str,options_json:&str)->Result<String,String>{to_json_result(parse_options(options_json).and_then(|options|format_csharp(source,&options)))}#[cfg(test)]mod tests{use super::*;use tokenpress_core::Error;fn format(source:&str,options:WasmPythonOptions)->WasmFormatOutput{format_python(source,&options).expect("formatting succeeds")}fn format_rs(source:&str,options:WasmRustOptions)->WasmFormatOutput{format_rust(source,&options).expect("formatting succeeds")}fn format_js_ok(source:&str,options:WasmJsOptions)->WasmFormatOutput{format_js(source,&options).expect("formatting succeeds")}fn format_go_ok(source:&str,options:WasmGoOptions)->WasmFormatOutput{format_go(source,&options).expect("formatting succeeds")}fn format_java_ok(source:&str,options:WasmJavaOptions)->WasmFormatOutput{format_java(source,&options).expect("formatting succeeds")}fn format_csharp_ok(source:&str,options:WasmCSharpOptions)->WasmFormatOutput{format_csharp(source,&options).expect("formatting succeeds")}fn js_dialect(dialect:WasmJsDialect)->WasmJsOptions{WasmJsOptions{dialect,..WasmJsOptions::default()}}fn stats<'a>(output:&'a WasmFormatOutput,tokenizer:&str)->&'a WasmTokenStats{output.tokens.iter().find(|stats|stats.tokenizer==tokenizer).expect("tokenizer is reported")}fn parsed(json:&str)->serde_json::Value{serde_json::from_str(json).expect("the boundary emits valid JSON")}#[test]fn default_options_minimize_whitespace_and_merge_imports(){let out=format("import os\nimport sys\n\nx = f(a, b)\n",WasmPythonOptions::default(),);assert_eq!(out.code,"import os,sys\nx=f(a,b)");assert!(out.changed);}#[test]fn already_minimal_source_is_reported_as_unchanged(){let out=format("x=1",WasmPythonOptions::default());assert_eq!(out.code,"x=1");assert!(!out.changed);}#[test]fn strip_docstrings_toggles_docstring_removal(){let source="def f():\n    \"\"\"Doc.\"\"\"\n    return 1\n";assert_eq!(format(source,WasmPythonOptions::default()).code,"def f():\n \"\"\"Doc.\"\"\"\n return 1");assert_eq!(format(source,WasmPythonOptions{strip_docstrings:true,..WasmPythonOptions::default()}).code,"def f():\n return 1");}#[test]fn strip_comments_toggles_comment_removal(){let source="# top\nx = 1\n";assert_eq!(format(source,WasmPythonOptions::default()).code,"# top\nx=1");assert_eq!(format(source,WasmPythonOptions{strip_comments:true,..WasmPythonOptions::default()}).code,"x=1");}#[test]fn strip_annotations_toggles_annotation_removal(){let source="def f(x: int = 1) -> int:\n    return x\n";assert_eq!(format(source,WasmPythonOptions::default()).code,"def f(x:int=1)->int:\n return x");assert_eq!(format(source,WasmPythonOptions{strip_annotations:true,..WasmPythonOptions::default()}).code,"def f(x=1):\n return x");}#[test]fn invalid_python_returns_a_structured_error_and_no_output(){let err=format_python("def f(:\n",&WasmPythonOptions::default()).expect_err("invalid Python cannot be formatted");assert_eq!(err.kind,"parse");assert!(!err.message.is_empty());assert_eq!(err.to_json(),format!("{{\"kind\":\"parse\",\"message\":{}}}",serde_json::Value::from(err.message.clone())));}#[test]fn every_core_error_maps_to_a_kind(){let cases=[(Error::Parse("x".into()),"parse"),(Error::Verification("x".into()),"verification"),(Error::UnsupportedLanguage("x".into()),"unsupported_language",),(Error::UnknownTokenizer("x".into()),"unknown_tokenizer"),(Error::Io(std::io::Error::other("x")),"io"),];for(error,kind)in cases{let message=error.to_string();let wasm_error=WasmError::from(error);assert_eq!(wasm_error.kind,kind);assert_eq!(wasm_error.message,message);}}#[test]fn json_boundary_returns_code_and_changed_flag(){let json=format_python_json("import os\nimport sys\n","{}").expect("formatting succeeds");let value=parsed(&json);assert_eq!(value["changed"],serde_json::json!(true));assert_eq!(value["code"],serde_json::json!("import os,sys"));}#[test]fn json_boundary_reads_option_flags(){let json=format_python_json("def f():\n    \"\"\"Doc.\"\"\"\n    return 1\n","{\"strip_docstrings\":true}",).expect("formatting succeeds");let value=parsed(&json);assert_eq!(value["changed"],serde_json::json!(true));assert_eq!(value["code"],serde_json::json!("def f():\n return 1"));}#[test]fn json_boundary_reports_per_tokenizer_token_stats(){for json in[format_python_json("import os\nimport sys\n","{}").expect("formatting succeeds"),format_rust_json("fn f() {\n    let x = 1;\n}\n","{}").expect("formatting succeeds"),format_js_json("function f() {\n    const x = 1;\n    return x;\n}\n","{}").expect("formatting succeeds"),format_go_json("package main\n\nfunc f() {\n    x := 1\n    _ = x\n}\n","{}",).expect("formatting succeeds"),format_java_json("class A {\n    int f() {\n        return 1;\n    }\n}\n","{}",).expect("formatting succeeds"),format_csharp_json("class A\n{\n    int F()\n    {\n        return 1;\n    }\n}\n","{}",).expect("formatting succeeds"),]{let value=parsed(&json);for name in["o200k_base","cl100k_base"]{let entry=&value["tokens"][name];let original=entry["original"].as_u64().expect("original count");let formatted=entry["formatted"].as_u64().expect("formatted count");let saved=entry["saved"].as_u64().expect("saved count");let ratio=entry["saving_ratio"].as_f64().expect("saving ratio");assert!(formatted<original,"{json}");assert_eq!(saved,original-formatted,"{json}");assert!(ratio>0.0&&ratio<1.0,"{json}");}}}#[test]fn json_boundary_reports_failures_as_structured_json(){let err=format_python_json("def f(:\n","{}").expect_err("invalid Python is refused");assert!(err.starts_with("{\"kind\":\"parse\",\"message\":\""),"{err}");}#[test]fn json_boundary_rejects_malformed_option_objects(){for options in["","{\"nope\":true}","{\"merge_imports\":\"yes\"}"]{let err=format_python_json("x=1",options).expect_err("bad options are refused");assert!(err.starts_with("{\"kind\":\"options\",\"message\":\""),"{err}");}}#[test]fn boundary_types_are_cloneable_and_debuggable(){let options=WasmPythonOptions::default();assert!(format!("{:?}",options.clone()).contains("merge_imports"));let rust_options=WasmRustOptions::default();assert!(format!("{:?}",rust_options.clone()).contains("strip_doc_comments"));let js_options=WasmJsOptions::default();let debugged=format!("{:?}",js_options.clone());assert!(debugged.contains("strip_comments"),"{debugged}");assert!(debugged.contains("Js"),"{debugged}");assert_eq!(js_options.dialect,WasmJsDialect::default());let go_options=WasmGoOptions::default();assert!(format!("{:?}",go_options.clone()).contains("strip_comments"));assert!(!go_options.strip_comments);let java_options=WasmJavaOptions::default();assert!(format!("{:?}",java_options.clone()).contains("strip_comments"));assert!(!java_options.strip_comments);let csharp_options=WasmCSharpOptions::default();assert!(format!("{:?}",csharp_options.clone()).contains("strip_comments"));assert!(!csharp_options.strip_comments);let output=WasmFormatOutput{code:"x=1".into(),changed:true,tokens:vec![WasmTokenStats{tokenizer:"o200k_base",original:4,formatted:3,saved:1,saving_ratio:0.25,}],};assert_eq!(output.clone(),output);assert!(format!("{output:?}").contains("changed"));assert!(format!("{:?}",output.tokens[0].clone()).contains("saving_ratio"));let error=WasmError{kind:"parse".into(),message:"bad".into(),};assert_eq!(error.clone(),error);assert!(format!("{error:?}").contains("parse"));}#[test]fn rust_default_options_minimize_whitespace_and_keep_doc_comments(){let out=format_rs("/// Adds.\npub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",WasmRustOptions::default(),);assert_eq!(out.code,"/// Adds.\npub fn add(a:i32,b:i32)->i32{a+b}");assert!(out.changed);}#[test]fn rust_already_minimal_source_is_reported_as_unchanged(){let out=format_rs("fn f(){}",WasmRustOptions::default());assert_eq!(out.code,"fn f(){}");assert!(!out.changed);}#[test]fn rust_strip_doc_comments_toggles_doc_removal(){let source="/// Adds.\npub fn f() {}\n";assert_eq!(format_rs(source,WasmRustOptions::default()).code,"/// Adds.\npub fn f(){}");assert_eq!(format_rs(source,WasmRustOptions{strip_doc_comments:true,}).code,"pub fn f(){}");}#[test]fn invalid_rust_returns_a_structured_error_and_no_output(){let err=format_rust("fn f( {",&WasmRustOptions::default()).expect_err("invalid Rust cannot be formatted");assert_eq!(err.kind,"parse");assert!(!err.message.is_empty());assert_eq!(err.to_json(),format!("{{\"kind\":\"parse\",\"message\":{}}}",serde_json::Value::from(err.message.clone())));}#[test]fn json_boundary_formats_rust(){let json=format_rust_json("pub fn f() {\n    let x = 1;\n}\n","{}").expect("formatting succeeds");let value=parsed(&json);assert_eq!(value["changed"],serde_json::json!(true));assert_eq!(value["code"],serde_json::json!("pub fn f(){let x=1;}"));}#[test]fn json_boundary_reads_rust_option_flags(){let json=format_rust_json("/// Adds.\npub fn f() {}\n","{\"strip_doc_comments\":true}",).expect("formatting succeeds");assert_eq!(parsed(&json)["code"],serde_json::json!("pub fn f(){}"));}#[test]fn json_boundary_reports_rust_failures_as_structured_json(){let err=format_rust_json("fn f( {","{}").expect_err("invalid Rust is refused");assert!(err.starts_with("{\"kind\":\"parse\",\"message\":\""),"{err}");}#[test]fn json_boundary_rejects_malformed_rust_option_objects(){for options in["","{\"nope\":true}","{\"strip_doc_comments\":\"yes\"}"]{let err=format_rust_json("fn f(){}",options).expect_err("bad options are refused");assert!(err.starts_with("{\"kind\":\"options\",\"message\":\""),"{err}");}}#[test]fn both_embedded_tokenizers_are_reported_in_a_stable_order(){for out in[format("x = f(a, b)\n",WasmPythonOptions::default()),format_rs("fn f() { g(a, b); }\n",WasmRustOptions::default()),format_js_ok("function f() { g(a, b); }\n",WasmJsOptions::default()),format_go_ok("package main\n\nfunc f() {\n    g(a, b)\n}\n",WasmGoOptions::default(),),format_java_ok("class A {\n    void f() {\n        g(a, b);\n    }\n}\n",WasmJavaOptions::default(),),format_csharp_ok("class A\n{\n    void F()\n    {\n        G(a, b);\n    }\n}\n",WasmCSharpOptions::default(),),]{let names:Vec<_> =out.tokens.iter().map(|stats|stats.tokenizer).collect();assert_eq!(names,["o200k_base","cl100k_base"]);}}#[test]fn token_counts_match_the_tokenizer_api(){let python="import os\nimport sys\n\nx = f(a, b)\n";let rust="fn add(a: i32, b: i32) -> i32 {\n    let sum = a + b;\n    sum\n}\n";let js="function add(a, b) {\n    const sum = a + b;\n    return sum;\n}\n";let go="package main\n\nfunc add(a int, b int) int {\n    sum := a + b\n    return sum\n}\n";let java="class A {\n    int add(int a, int b) {\n        int sum = a + b;\n        return sum;\n    }\n}\n";let csharp="class A\n{\n    int Add(int a, int b)\n    {\n        int sum = a + b;\n        return sum;\n    }\n}\n";for(source,out)in[(python,format(python,WasmPythonOptions::default())),(rust,format_rs(rust,WasmRustOptions::default())),(js,format_js_ok(js,WasmJsOptions::default())),(go,format_go_ok(go,WasmGoOptions::default())),(java,format_java_ok(java,WasmJavaOptions::default())),(csharp,format_csharp_ok(csharp,WasmCSharpOptions::default()),),]{for(name,kind)in[("o200k_base",TokenizerKind::O200kBase),("cl100k_base",TokenizerKind::Cl100kBase),]{let tokenizer=kind.load().expect("embedded tokenizer loads");let stats=stats(&out,name);assert_eq!(stats.original,tokenizer.count(source));assert_eq!(stats.formatted,tokenizer.count(&out.code));assert_eq!(stats.saved,stats.original-stats.formatted);let expected=stats.saved as f64/stats.original as f64;assert!((stats.saving_ratio-expected).abs()<f64::EPSILON);assert!(stats.formatted<stats.original);}}}#[test]fn zero_token_input_reports_a_zero_saving_ratio(){for out in[format("",WasmPythonOptions::default()),format_rs("",WasmRustOptions::default()),format_js_ok("",WasmJsOptions::default()),format_go_ok("",WasmGoOptions::default()),format_java_ok("",WasmJavaOptions::default()),format_csharp_ok("",WasmCSharpOptions::default()),]{assert_eq!(out.code,"");assert!(!out.changed);assert_eq!(out.tokens.len(),2);for stats in&out.tokens{assert_eq!((stats.original,stats.formatted,stats.saved),(0,0,0),"{:?}",stats.tokenizer);assert_eq!(stats.saving_ratio,0.0);}}}#[test]fn merge_imports_can_be_turned_off(){let source="import os\nimport sys\n";assert_eq!(format(source,WasmPythonOptions::default()).code,"import os,sys");assert_eq!(format(source,WasmPythonOptions{merge_imports:false,..WasmPythonOptions::default()}).code,"import os\nimport sys");}#[test]fn wasm_verifies_internally_and_never_externally(){assert_eq!(VERIFY,VerifyLevel::AstEquiv);assert_ne!(VERIFY,VerifyLevel::External);}#[test]fn js_default_options_minimize_whitespace_and_keep_leading_comments(){let out=format_js_ok("// note\nfunction add( a , b ) {\n    return a + b;\n}\n",WasmJsOptions::default(),);assert_eq!(out.code,"// note\nfunction add(a,b){return a+b}");assert!(out.changed);}#[test]fn js_already_minimal_source_is_reported_as_unchanged(){let out=format_js_ok("const a=1;",WasmJsOptions::default());assert_eq!(out.code,"const a=1;");assert!(!out.changed);}#[test]fn js_strip_comments_toggles_comment_removal(){let source="// note\nconst a = 1;\n";assert_eq!(format_js_ok(source,WasmJsOptions::default()).code,"// note\nconst a=1;");assert_eq!(format_js_ok(source,WasmJsOptions{strip_comments:true,..WasmJsOptions::default()}).code,"const a=1;");}#[test]fn js_trailing_comments_are_dropped_even_when_kept(){assert_eq!(format_js_ok("function f(a, b) {\n    return a + b; // tail\n}\n",WasmJsOptions::default()).code,"function f(a,b){return a+b}");}#[test]fn every_js_dialect_maps_to_its_synthetic_path(){let cases=[(WasmJsDialect::Js,"input.js"),(WasmJsDialect::Mjs,"input.mjs"),(WasmJsDialect::Cjs,"input.cjs"),(WasmJsDialect::Jsx,"input.jsx"),(WasmJsDialect::Ts,"input.ts"),(WasmJsDialect::Mts,"input.mts"),(WasmJsDialect::Cts,"input.cts"),(WasmJsDialect::Tsx,"input.tsx"),];for(dialect,path)in cases{assert_eq!(dialect.path(),Path::new(path),"{dialect:?}");}}#[test]fn every_js_dialect_formats_its_own_syntax(){let cases=[(WasmJsDialect::Js,"const a = 1;\n","const a=1;"),(WasmJsDialect::Mjs,"export const a = 1;\n","export const a=1;",),(WasmJsDialect::Cjs,"const a = require( \"b\" );\n","const a=require(\"b\");",),(WasmJsDialect::Jsx,"const a = <div>hi</div>;\n","const a=<div>hi</div>;",),(WasmJsDialect::Ts,"let x : number = 1;\n","let x:number=1;",),(WasmJsDialect::Mts,"export let x : number = 1;\n","export let x:number=1;",),(WasmJsDialect::Cts,"let x : number = 1;\n","let x:number=1;",),(WasmJsDialect::Tsx,"const a : JSX.Element = <div>hi</div>;\n","const a:JSX.Element=<div>hi</div>;",),];for(dialect,source,expected)in cases{assert_eq!(format_js_ok(source,js_dialect(dialect)).code,expected,"{dialect:?}");}}#[test]fn the_default_js_dialect_is_plain_javascript(){let err=format_js("let x: number = 1;\n",&WasmJsOptions::default()).expect_err("TypeScript is not plain JavaScript");assert_eq!(err.kind,"parse");assert!(err.message.contains("input.js"),"{}",err.message);assert_eq!(format_js_ok("let x: number = 1;\n",js_dialect(WasmJsDialect::Ts)).code,"let x:number=1;");}#[test]fn invalid_js_returns_a_structured_error_and_no_output(){let err=format_js("function (",&WasmJsOptions::default()).expect_err("invalid JavaScript cannot be formatted");assert_eq!(err.kind,"parse");assert!(!err.message.is_empty());assert_eq!(err.to_json(),format!("{{\"kind\":\"parse\",\"message\":{}}}",serde_json::Value::from(err.message.clone())));}#[test]fn json_boundary_formats_js(){let json=format_js_json("function f( a ) {\n    return a;\n}\n","{}").expect("formatting succeeds");let value=parsed(&json);assert_eq!(value["changed"],serde_json::json!(true));assert_eq!(value["code"],serde_json::json!("function f(a){return a}"));}#[test]fn json_boundary_reads_js_option_flags(){let json=format_js_json("// note\ninterface Shape {\n    name : string ;\n}\n","{\"dialect\":\"ts\",\"strip_comments\":true}",).expect("formatting succeeds");assert_eq!(parsed(&json)["code"],serde_json::json!("interface Shape{name:string;}"));}#[test]fn json_boundary_reports_js_failures_as_structured_json(){let err=format_js_json("function (","{}").expect_err("invalid JavaScript is refused");assert!(err.starts_with("{\"kind\":\"parse\",\"message\":\""),"{err}");}#[test]fn json_boundary_rejects_malformed_js_option_objects(){for options in["","{\"nope\":true}","{\"strip_comments\":\"yes\"}","{\"dialect\":\"coffee\"}","{\"dialect\":true}",]{let err=format_js_json("const a=1;",options).expect_err("bad options are refused");assert!(err.starts_with("{\"kind\":\"options\",\"message\":\""),"{err}");}}#[test]fn an_unknown_js_dialect_names_the_accepted_values(){let err=format_js_json("const a=1;","{\"dialect\":\"coffee\"}").expect_err("an unknown dialect is refused");for name in["js","mjs","cjs","jsx","ts","mts","cts","tsx"]{assert!(err.contains(&format!("`{name}`")),"{err}");}}
/// The 1-based numbers of the doc-comment lines in `source` that contain a
/// block-comment terminator.
fn doc_lines_closing_a_comment_block(source:&str)->Vec<usize>{let mut lines=Vec::new();for(index,line)in source.lines().enumerate(){let trimmed=line.trim_start();let is_doc=trimmed.starts_with("///")||trimmed.starts_with("//!");if is_doc&&trimmed.contains("*/"){lines.push(index+1);}}lines}#[test]fn the_jsdoc_guard_finds_a_doc_comment_that_closes_the_block(){let source="//! a */ b\n/// c\n    /// d */\n// e */\nlet f = \"*/\";\n";assert_eq!(doc_lines_closing_a_comment_block(source),vec![1,3]);}#[test]fn no_doc_comment_in_this_file_closes_the_generated_jsdoc_block(){assert_eq!(doc_lines_closing_a_comment_block(include_str!("lib.rs")),Vec::<usize>::new());}#[test]fn go_default_options_minimize_whitespace_and_keep_every_comment(){let out=format_go_ok("package main\n\n// note\nfunc main()  {\n\n    x := 1\n    _ = x // tail\n}\n",WasmGoOptions::default(),);assert_eq!(out.code,"package main\n// note\nfunc main() {\nx := 1\n_ = x // tail\n}\n");assert!(out.changed);}#[test]fn go_already_minimal_source_is_reported_as_unchanged(){let out=format_go_ok("package main\nfunc f(){}\n",WasmGoOptions::default());assert_eq!(out.code,"package main\nfunc f(){}\n");assert!(!out.changed);}#[test]fn go_strip_comments_toggles_comment_removal(){let source="package main\n\n// note\nfunc f() {}\n";assert_eq!(format_go_ok(source,WasmGoOptions::default()).code,"package main\n// note\nfunc f() {}\n");assert_eq!(format_go_ok(source,WasmGoOptions{strip_comments:true}).code,"package main\nfunc f() {}\n");}#[test]fn go_strip_comments_keeps_toolchain_directives(){let source="//go:build linux\n\npackage main\n\n//go:generate echo hi\nfunc f() {}\n";assert_eq!(format_go_ok(source,WasmGoOptions{strip_comments:true}).code,"//go:build linux\n\npackage main\n//go:generate echo hi\nfunc f() {}\n");}#[test]fn invalid_go_returns_a_structured_error_and_no_output(){let err=format_go("package main\nfunc {{{",&WasmGoOptions::default()).expect_err("invalid Go cannot be formatted");assert_eq!(err.kind,"parse");assert!(!err.message.is_empty());assert_eq!(err.to_json(),format!("{{\"kind\":\"parse\",\"message\":{}}}",serde_json::Value::from(err.message.clone())));}#[test]fn json_boundary_formats_go(){let json=format_go_json("package main\n\nfunc f()  {}\n","{}").expect("formatting succeeds");let value=parsed(&json);assert_eq!(value["changed"],serde_json::json!(true));assert_eq!(value["code"],serde_json::json!("package main\nfunc f() {}\n"));}#[test]fn json_boundary_reads_go_option_flags(){let json=format_go_json("package main\n\n// note\nfunc f() {}\n","{\"strip_comments\":true}",).expect("formatting succeeds");assert_eq!(parsed(&json)["code"],serde_json::json!("package main\nfunc f() {}\n"));}#[test]fn json_boundary_reports_go_failures_as_structured_json(){let err=format_go_json("package main\nfunc {{{","{}").expect_err("invalid Go is refused");assert!(err.starts_with("{\"kind\":\"parse\",\"message\":\""),"{err}");}#[test]fn json_boundary_rejects_malformed_go_option_objects(){for options in["","{\"nope\":true}","{\"strip_comments\":\"yes\"}"]{let err=format_go_json("package main\nfunc f(){}\n",options).expect_err("bad options are refused");assert!(err.starts_with("{\"kind\":\"options\",\"message\":\""),"{err}");}}#[test]fn java_default_options_minimize_whitespace_and_keep_every_comment(){let out=format_java_ok("class A {\n    // leading\n    int x = 1; // trailing\n\n    /* inline */ int y = 2;\n}\n",WasmJavaOptions::default(),);assert_eq!(out.code,"class A {\n// leading\nint x = 1; // trailing\n/* inline */ int y = 2;\n}\n");assert!(out.changed);}#[test]fn java_already_minimal_source_is_reported_as_unchanged(){let out=format_java_ok("class A {\nint x = 1;\n}\n",WasmJavaOptions::default());assert_eq!(out.code,"class A {\nint x = 1;\n}\n");assert!(!out.changed);}#[test]fn java_strip_comments_toggles_comment_removal(){let source="class A {\n    // note\n    int x = 1;\n}\n";assert_eq!(format_java_ok(source,WasmJavaOptions::default()).code,"class A {\n// note\nint x = 1;\n}\n");assert_eq!(format_java_ok(source,WasmJavaOptions{strip_comments:true}).code,"class A {\nint x = 1;\n}\n");}#[test]fn java_strip_comments_deletes_javadoc_with_every_other_comment(){let source="/**\n * The class.\n */\nclass A {\n    /** The field. */\n    int x = 1;\n}\n";assert_eq!(format_java_ok(source,WasmJavaOptions::default()).code,"/**\n * The class.\n */\nclass A {\n/** The field. */\nint x = 1;\n}\n");assert_eq!(format_java_ok(source,WasmJavaOptions{strip_comments:true}).code,"class A {\nint x = 1;\n}\n");}#[test]fn a_java_escape_hazard_file_is_untouched_and_reports_no_savings(){let source="class A {\nint x = 1; // c \\u000A int y = 2;\nint z = 3;\n}\n";for options in[WasmJavaOptions::default(),WasmJavaOptions{strip_comments:true,},]{let out=format_java_ok(source,options);assert_eq!(out.code,source);assert!(!out.changed);for stats in&out.tokens{assert_eq!(stats.saved,0);assert_eq!(stats.saving_ratio,0.0);}}}#[test]fn invalid_java_returns_a_structured_error_and_no_output(){let err=format_java("class A {{{",&WasmJavaOptions::default()).expect_err("invalid Java cannot be formatted");assert_eq!(err.kind,"parse");assert!(!err.message.is_empty());assert_eq!(err.to_json(),format!("{{\"kind\":\"parse\",\"message\":{}}}",serde_json::Value::from(err.message.clone())));}#[test]fn json_boundary_formats_java(){let json=format_java_json("class  A  {\n    int x = 1;\n}\n","{}").expect("formatting succeeds");let value=parsed(&json);assert_eq!(value["changed"],serde_json::json!(true));assert_eq!(value["code"],serde_json::json!("class A {\nint x = 1;\n}\n"));}#[test]fn json_boundary_reads_java_option_flags(){let json=format_java_json("class A {\n    // note\n    int x = 1;\n}\n","{\"strip_comments\":true}",).expect("formatting succeeds");assert_eq!(parsed(&json)["code"],serde_json::json!("class A {\nint x = 1;\n}\n"));}#[test]fn json_boundary_reports_java_failures_as_structured_json(){let err=format_java_json("class A {{{","{}").expect_err("invalid Java is refused");assert!(err.starts_with("{\"kind\":\"parse\",\"message\":\""),"{err}");}#[test]fn json_boundary_rejects_malformed_java_option_objects(){for options in["","{\"nope\":true}","{\"strip_comments\":\"yes\"}"]{let err=format_java_json("class A {}\n",options).expect_err("bad options are refused");assert!(err.starts_with("{\"kind\":\"options\",\"message\":\""),"{err}");}}#[test]fn csharp_default_options_minimize_whitespace_and_keep_every_comment(){let out=format_csharp_ok("class A\n{\n    // leading\n    int x = 1; // trailing\n\n    /* inline */ int y = 2;\n}\n",WasmCSharpOptions::default(),);assert_eq!(out.code,"class A\n{\n// leading\nint x = 1; // trailing\n/* inline */ int y = 2;\n}\n");assert!(out.changed);}#[test]fn csharp_already_minimal_source_is_reported_as_unchanged(){let out=format_csharp_ok("class A\n{\nint x = 1;\n}\n",WasmCSharpOptions::default());assert_eq!(out.code,"class A\n{\nint x = 1;\n}\n");assert!(!out.changed);}#[test]fn csharp_strip_comments_toggles_comment_removal(){let source="class A\n{\n    // note\n    int x = 1;\n}\n";assert_eq!(format_csharp_ok(source,WasmCSharpOptions::default()).code,"class A\n{\n// note\nint x = 1;\n}\n");assert_eq!(format_csharp_ok(source,WasmCSharpOptions{strip_comments:true}).code,"class A\n{\nint x = 1;\n}\n");}#[test]fn csharp_strip_comments_deletes_xml_documentation_with_every_other_comment(){let source="/// <summary>The class.</summary>\npublic class A\n{\n    /// <summary>The method.</summary>\n    public void M() {}\n}\n";assert_eq!(format_csharp_ok(source,WasmCSharpOptions::default()).code,"/// <summary>The class.</summary>\npublic class A\n{\n/// <summary>The method.</summary>\npublic void M() {}\n}\n");assert_eq!(format_csharp_ok(source,WasmCSharpOptions{strip_comments:true}).code,"public class A\n{\npublic void M() {}\n}\n");}#[test]fn a_csharp_comment_boundary_hazard_file_is_untouched_and_reports_no_savings(){let source="class A {\n#if FALSE\n/*\n#endif\nint x = 1;\n#if FALSE\n*/\n#endif\n}\n";for options in[WasmCSharpOptions::default(),WasmCSharpOptions{strip_comments:true,},]{let out=format_csharp_ok(source,options);assert_eq!(out.code,source);assert!(!out.changed);for stats in&out.tokens{assert_eq!(stats.saved,0);assert_eq!(stats.saving_ratio,0.0);}}}#[test]fn invalid_csharp_returns_a_structured_error_and_no_output(){let err=format_csharp("class A {\n    void M() {\n",&WasmCSharpOptions::default()).expect_err("invalid C# cannot be formatted");assert_eq!(err.kind,"parse");assert!(!err.message.is_empty());assert_eq!(err.to_json(),format!("{{\"kind\":\"parse\",\"message\":{}}}",serde_json::Value::from(err.message.clone())));}#[test]fn json_boundary_formats_csharp(){let json=format_csharp_json("class  A\n{\n    int x = 1;\n}\n","{}").expect("formatting succeeds");let value=parsed(&json);assert_eq!(value["changed"],serde_json::json!(true));assert_eq!(value["code"],serde_json::json!("class A\n{\nint x = 1;\n}\n"));}#[test]fn json_boundary_reads_csharp_option_flags(){let json=format_csharp_json("class A\n{\n    // note\n    int x = 1;\n}\n","{\"strip_comments\":true}",).expect("formatting succeeds");assert_eq!(parsed(&json)["code"],serde_json::json!("class A\n{\nint x = 1;\n}\n"));}#[test]fn json_boundary_reports_csharp_failures_as_structured_json(){let err=format_csharp_json("class A {\n    void M() {\n","{}").expect_err("invalid C# is refused");assert!(err.starts_with("{\"kind\":\"parse\",\"message\":\""),"{err}");}#[test]fn json_boundary_rejects_malformed_csharp_option_objects(){for options in["","{\"nope\":true}","{\"strip_comments\":\"yes\"}"]{let err=format_csharp_json("class A {}\n",options).expect_err("bad options are refused");assert!(err.starts_with("{\"kind\":\"options\",\"message\":\""),"{err}");}}}