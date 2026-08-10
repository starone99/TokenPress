//! Project configuration file (`tokenpress.toml`).
//!
//! This module only turns the file into a typed value. Locating the file and
//! merging it with the command-line arguments happens elsewhere.
use std::path::{Path,PathBuf};use serde::Deserialize;
/// Everything a `tokenpress.toml` can express. Every field is optional: a
/// missing key means "not configured", never a default. Unknown keys are
/// rejected rather than ignored, so a typo fails loudly instead of silently
/// doing nothing.
#[derive(Debug,Deserialize,PartialEq)]#[serde(deny_unknown_fields)]pub struct FileConfig{
/// Free-form tokenizer spec; validated where the CLI validates `--tokenizer`.
pub tokenizer:Option<String>,pub verify:Option<ConfigVerify>,pub python:Option<PythonConfig>,pub rust:Option<RustConfig>,pub javascript:Option<JavaScriptConfig>,#[cfg(feature="ruby")]pub ruby:Option<RubyConfig>,
/// Without the `ruby` cargo feature the table stays in the schema but can
/// no longer be satisfied — see `NoRubySupport`.
#[cfg(not(feature="ruby"))]pub ruby:Option<NoRubySupport>,#[cfg(feature="go")]pub go:Option<GoConfig>,
/// The `go` cargo feature is switchable exactly as `ruby` is, so its table
/// gets the same treatment — see `NoGoSupport`.
#[cfg(not(feature="go"))]pub go:Option<NoGoSupport>,#[cfg(feature="java")]pub java:Option<JavaConfig>,
/// The `java` cargo feature is switchable exactly as `ruby` and `go` are,
/// so its table gets the same treatment — see `NoJavaSupport`.
#[cfg(not(feature="java"))]pub java:Option<NoJavaSupport>,#[cfg(feature="csharp")]pub csharp:Option<CSharpConfig>,
/// The `csharp` cargo feature is switchable exactly as `ruby`, `go` and
/// `java` are, so its table gets the same treatment — see
/// `NoCSharpSupport`.
#[cfg(not(feature="csharp"))]pub csharp:Option<NoCSharpSupport>,}
/// `[python]` table.
#[derive(Debug,Deserialize,PartialEq)]#[serde(deny_unknown_fields)]pub struct PythonConfig{pub strip_comments:Option<bool>,pub strip_docstrings:Option<bool>,pub strip_annotations:Option<bool>,
/// Positively named: `merge_imports = false` is the config spelling of the
/// command line's `--py-no-merge-imports`.
pub merge_imports:Option<bool>,}
/// `[rust]` table.
#[derive(Debug,Deserialize,PartialEq)]#[serde(deny_unknown_fields)]pub struct RustConfig{pub strip_doc_comments:Option<bool>,}
/// `[javascript]` table. Named after `JsFormatter::language()`, and it covers
/// TypeScript too — the JS and TS dialects share one backend and one option
/// set.
#[derive(Debug,Deserialize,PartialEq)]#[serde(deny_unknown_fields)]pub struct JavaScriptConfig{pub strip_comments:Option<bool>,}
/// `[ruby]` table. Named after `RubyFormatter::language()`, and it covers every
/// path that backend claims — the `.rb`/`.rake`/`.gemspec`/`.ru` extensions as
/// well as the extensionless `Gemfile` and `Rakefile`.
#[cfg(feature="ruby")]#[derive(Debug,Deserialize,PartialEq)]#[serde(deny_unknown_fields)]pub struct RubyConfig{pub strip_comments:Option<bool>,}
/// Stand-in for the `[ruby]` table in a build without the `ruby` cargo
/// feature: it never deserializes. Keeping the key in the schema and failing
/// on it names the reason — dropping the field instead would let
/// `deny_unknown_fields` blame the user's spelling for a property of the
/// build, and accepting it would silently ignore a configured option.
#[cfg(not(feature="ruby"))]#[derive(Debug,PartialEq)]pub struct NoRubySupport;#[cfg(not(feature="ruby"))]impl<'de>Deserialize<'de>for NoRubySupport{fn deserialize<D:serde::Deserializer<'de>>(_:D)->Result<Self,D::Error>{Err(serde::de::Error::custom("this tokenpress was built without the `ruby` feature, so the \
             [ruby] table has nothing to configure",))}}
/// `[go]` table. Named after `GoFormatter::language()`, and it covers the one
/// extension that backend claims: `.go`.
#[cfg(feature="go")]#[derive(Debug,Deserialize,PartialEq)]#[serde(deny_unknown_fields)]pub struct GoConfig{pub strip_comments:Option<bool>,}
/// Stand-in for the `[go]` table in a build without the `go` cargo feature,
/// for the same reason `NoRubySupport` is one: naming the missing feature
/// beats blaming the user's spelling or silently ignoring the option.
#[cfg(not(feature="go"))]#[derive(Debug,PartialEq)]pub struct NoGoSupport;#[cfg(not(feature="go"))]impl<'de>Deserialize<'de>for NoGoSupport{fn deserialize<D:serde::Deserializer<'de>>(_:D)->Result<Self,D::Error>{Err(serde::de::Error::custom("this tokenpress was built without the `go` feature, so the \
             [go] table has nothing to configure",))}}
/// `[java]` table. Named after `JavaFormatter::language()`, and it covers the
/// one extension that backend claims: `.java`. Distinct from `[javascript]`,
/// which is the JS/TS backend's table — the two names are close but the
/// backends share nothing, and `deny_unknown_fields` keeps a confusion of the
/// two from being silently accepted.
#[cfg(feature="java")]#[derive(Debug,Deserialize,PartialEq)]#[serde(deny_unknown_fields)]pub struct JavaConfig{pub strip_comments:Option<bool>,}
/// Stand-in for the `[java]` table in a build without the `java` cargo
/// feature, for the same reason `NoRubySupport` and `NoGoSupport` are ones:
/// naming the missing feature beats blaming the user's spelling or silently
/// ignoring the option.
#[cfg(not(feature="java"))]#[derive(Debug,PartialEq)]pub struct NoJavaSupport;#[cfg(not(feature="java"))]impl<'de>Deserialize<'de>for NoJavaSupport{fn deserialize<D:serde::Deserializer<'de>>(_:D)->Result<Self,D::Error>{Err(serde::de::Error::custom("this tokenpress was built without the `java` feature, so the \
             [java] table has nothing to configure",))}}
/// `[csharp]` table. Named after `CSharpFormatter::language()`, which is
/// `"csharp"` and not `"c#"` — the language key stays in the character set a
/// TOML bare key and a command-line flag can both spell. It covers the one
/// extension that backend claims: `.cs`.
#[cfg(feature="csharp")]#[derive(Debug,Deserialize,PartialEq)]#[serde(deny_unknown_fields)]pub struct CSharpConfig{pub strip_comments:Option<bool>,}
/// Stand-in for the `[csharp]` table in a build without the `csharp` cargo
/// feature, for the same reason `NoRubySupport`, `NoGoSupport` and
/// `NoJavaSupport` are ones: naming the missing feature beats blaming the
/// user's spelling or silently ignoring the option.
#[cfg(not(feature="csharp"))]#[derive(Debug,PartialEq)]pub struct NoCSharpSupport;#[cfg(not(feature="csharp"))]impl<'de>Deserialize<'de>for NoCSharpSupport{fn deserialize<D:serde::Deserializer<'de>>(_:D)->Result<Self,D::Error>{Err(serde::de::Error::custom("this tokenpress was built without the `csharp` feature, so the \
             [csharp] table has nothing to configure",))}}
/// Verification level as spelled in the config file. The variants carry the
/// same lowercase names as the `--verify` values accepted on the command line.
#[derive(Debug,Deserialize,PartialEq)]#[serde(rename_all="lowercase")]pub enum ConfigVerify{Reparse,Ast,External,}
/// Errors raised while loading a config file.
#[derive(Debug,thiserror::Error)]pub enum ConfigError{#[error("cannot read config file {}: {source}",path.display())]Read{path:PathBuf,source:std::io::Error,},
/// The TOML error already carries the offending key and its line/column.
#[error("invalid config file: {0}")]Parse(#[from]toml::de::Error),}impl FileConfig{
/// Parses the contents of a `tokenpress.toml`.
pub fn from_toml_str(text:&str)->Result<Self,ConfigError>{Ok(toml::from_str(text)?)}
/// Reads `path` and parses it. An unreadable file is a `Read` error that
/// names the path; a malformed one is a `Parse` error.
pub fn load(path:&Path)->Result<Self,ConfigError>{let text=std::fs::read_to_string(path).map_err(|source|ConfigError::Read{path:path.to_path_buf(),source,})?;Self::from_toml_str(&text)}}#[cfg(test)]mod tests{use super::*;use std::sync::atomic::{AtomicUsize,Ordering};
/// Unique scratch directory per test, cleaned up on drop.
struct Scratch(PathBuf);impl Scratch{fn new()->Self{static N:AtomicUsize=AtomicUsize::new(0);let dir=std::env::temp_dir().join(format!("tokenpress-config-test-{}-{}",std::process::id(),N.fetch_add(1,Ordering::Relaxed)));std::fs::create_dir_all(&dir).unwrap();Self(dir)}fn file(&self,name:&str,content:&str)->PathBuf{let p=self.0.join(name);std::fs::write(&p,content).unwrap();p}}impl Drop for Scratch{fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}}
/// Parses `text`, expecting success.
fn parse(text:&str)->FileConfig{FileConfig::from_toml_str(text).unwrap()}
/// Parses `text`, expecting failure, and returns the rendered message.
fn parse_err(text:&str)->String{FileConfig::from_toml_str(text).unwrap_err().to_string()}#[test]fn empty_file_leaves_every_field_unset(){let cfg=parse("");assert_eq!(cfg.tokenizer,None);assert_eq!(cfg.verify,None);assert_eq!(cfg.python,None);assert_eq!(cfg.rust,None);assert_eq!(cfg.javascript,None);assert_eq!(cfg.ruby,None);assert_eq!(cfg.go,None);assert_eq!(cfg.java,None);assert_eq!(cfg.csharp,None);}
/// The `[ruby]` table as a build with the `ruby` feature spells it, and as
/// a build without it has to: there the table cannot be satisfied at all,
/// so a test covering the whole schema has to leave it out.
#[cfg(feature="ruby")]const RUBY_TABLE:&str="[ruby]\nstrip_comments = true\n";#[cfg(not(feature="ruby"))]const RUBY_TABLE:&str="";
/// The same for the `[go]` table and the `go` feature.
#[cfg(feature="go")]const GO_TABLE:&str="[go]\nstrip_comments = true\n";#[cfg(not(feature="go"))]const GO_TABLE:&str="";
/// ... and for the `[java]` table and the `java` feature.
#[cfg(feature="java")]const JAVA_TABLE:&str="[java]\nstrip_comments = true\n";#[cfg(not(feature="java"))]const JAVA_TABLE:&str="";
/// ... and for the `[csharp]` table and the `csharp` feature.
#[cfg(feature="csharp")]const CSHARP_TABLE:&str="[csharp]\nstrip_comments = true\n";#[cfg(not(feature="csharp"))]const CSHARP_TABLE:&str="";
/// The same four tables with no keys in them.
#[cfg(feature="ruby")]const EMPTY_RUBY_TABLE:&str="[ruby]\n";#[cfg(not(feature="ruby"))]const EMPTY_RUBY_TABLE:&str="";#[cfg(feature="go")]const EMPTY_GO_TABLE:&str="[go]\n";#[cfg(not(feature="go"))]const EMPTY_GO_TABLE:&str="";#[cfg(feature="java")]const EMPTY_JAVA_TABLE:&str="[java]\n";#[cfg(not(feature="java"))]const EMPTY_JAVA_TABLE:&str="";#[cfg(feature="csharp")]const EMPTY_CSHARP_TABLE:&str="[csharp]\n";#[cfg(not(feature="csharp"))]const EMPTY_CSHARP_TABLE:&str="";#[test]fn full_config_parses_every_field(){let cfg=parse(&format!("tokenizer = \"cl100k_base\"\n\
             verify = \"reparse\"\n\
             [python]\n\
             strip_comments = true\n\
             strip_docstrings = true\n\
             strip_annotations = false\n\
             merge_imports = false\n\
             [rust]\n\
             strip_doc_comments = true\n\
             [javascript]\n\
             strip_comments = true\n\
             {RUBY_TABLE}{GO_TABLE}{JAVA_TABLE}{CSHARP_TABLE}"));assert_eq!(cfg.tokenizer.as_deref(),Some("cl100k_base"));assert_eq!(cfg.verify,Some(ConfigVerify::Reparse));assert_eq!(cfg.python,Some(PythonConfig{strip_comments:Some(true),strip_docstrings:Some(true),strip_annotations:Some(false),merge_imports:Some(false),}));assert_eq!(cfg.rust,Some(RustConfig{strip_doc_comments:Some(true)}));assert_eq!(cfg.javascript,Some(JavaScriptConfig{strip_comments:Some(true)}));#[cfg(feature="ruby")]assert_eq!(cfg.ruby,Some(RubyConfig{strip_comments:Some(true)}));#[cfg(feature="go")]assert_eq!(cfg.go,Some(GoConfig{strip_comments:Some(true)}));#[cfg(feature="java")]assert_eq!(cfg.java,Some(JavaConfig{strip_comments:Some(true)}));#[cfg(feature="csharp")]assert_eq!(cfg.csharp,Some(CSharpConfig{strip_comments:Some(true)}));}#[test]fn partial_config_leaves_the_other_fields_unset(){let cfg=parse("tokenizer = \"o200k_base\"\n[python]\nstrip_comments = true\n");assert_eq!(cfg.tokenizer.as_deref(),Some("o200k_base"));assert_eq!(cfg.verify,None);assert_eq!(cfg.rust,None);assert_eq!(cfg.javascript,None);assert_eq!(cfg.ruby,None);assert_eq!(cfg.go,None);assert_eq!(cfg.java,None);assert_eq!(cfg.csharp,None);let python=cfg.python.unwrap();assert_eq!(python.strip_comments,Some(true));assert_eq!(python.strip_docstrings,None);assert_eq!(python.strip_annotations,None);assert_eq!(python.merge_imports,None);}#[test]fn python_table_alone_parses(){let cfg=parse("[python]\nstrip_annotations = true\nmerge_imports = true\n");assert_eq!(cfg.rust,None);assert_eq!(cfg.javascript,None);assert_eq!(cfg.ruby,None);assert_eq!(cfg.go,None);assert_eq!(cfg.java,None);assert_eq!(cfg.csharp,None);let python=cfg.python.unwrap();assert_eq!(python.strip_annotations,Some(true));assert_eq!(python.merge_imports,Some(true));}#[test]fn rust_table_alone_parses(){let cfg=parse("[rust]\nstrip_doc_comments = false\n");assert_eq!(cfg.python,None);assert_eq!(cfg.javascript,None);assert_eq!(cfg.ruby,None);assert_eq!(cfg.go,None);assert_eq!(cfg.java,None);assert_eq!(cfg.csharp,None);assert_eq!(cfg.rust,Some(RustConfig{strip_doc_comments:Some(false)}));}#[test]fn javascript_table_alone_parses(){let cfg=parse("[javascript]\nstrip_comments = true\n");assert_eq!(cfg.python,None);assert_eq!(cfg.rust,None);assert_eq!(cfg.ruby,None);assert_eq!(cfg.go,None);assert_eq!(cfg.java,None);assert_eq!(cfg.csharp,None);assert_eq!(cfg.javascript,Some(JavaScriptConfig{strip_comments:Some(true)}));}#[cfg(feature="ruby")]#[test]fn ruby_table_alone_parses(){let cfg=parse("[ruby]\nstrip_comments = true\n");assert_eq!(cfg.python,None);assert_eq!(cfg.rust,None);assert_eq!(cfg.javascript,None);assert_eq!(cfg.go,None);assert_eq!(cfg.java,None);assert_eq!(cfg.csharp,None);assert_eq!(cfg.ruby,Some(RubyConfig{strip_comments:Some(true)}));}#[cfg(feature="go")]#[test]fn go_table_alone_parses(){let cfg=parse("[go]\nstrip_comments = true\n");assert_eq!(cfg.python,None);assert_eq!(cfg.rust,None);assert_eq!(cfg.javascript,None);assert_eq!(cfg.ruby,None);assert_eq!(cfg.java,None);assert_eq!(cfg.csharp,None);assert_eq!(cfg.go,Some(GoConfig{strip_comments:Some(true)}));}#[cfg(feature="java")]#[test]fn java_table_alone_parses(){let cfg=parse("[java]\nstrip_comments = true\n");assert_eq!(cfg.python,None);assert_eq!(cfg.rust,None);assert_eq!(cfg.javascript,None);assert_eq!(cfg.ruby,None);assert_eq!(cfg.go,None);assert_eq!(cfg.csharp,None);assert_eq!(cfg.java,Some(JavaConfig{strip_comments:Some(true)}));}#[cfg(feature="csharp")]#[test]fn csharp_table_alone_parses(){let cfg=parse("[csharp]\nstrip_comments = true\n");assert_eq!(cfg.python,None);assert_eq!(cfg.rust,None);assert_eq!(cfg.javascript,None);assert_eq!(cfg.ruby,None);assert_eq!(cfg.go,None);assert_eq!(cfg.java,None);assert_eq!(cfg.csharp,Some(CSharpConfig{strip_comments:Some(true)}));}#[test]fn empty_tables_are_valid_and_leave_their_keys_unset(){let cfg=parse(&format!("[python]\n[rust]\n[javascript]\n\
             {EMPTY_RUBY_TABLE}{EMPTY_GO_TABLE}{EMPTY_JAVA_TABLE}{EMPTY_CSHARP_TABLE}"));assert_eq!(cfg.python,Some(PythonConfig{strip_comments:None,strip_docstrings:None,strip_annotations:None,merge_imports:None,}));assert_eq!(cfg.rust,Some(RustConfig{strip_doc_comments:None}));assert_eq!(cfg.javascript,Some(JavaScriptConfig{strip_comments:None}));#[cfg(feature="ruby")]assert_eq!(cfg.ruby,Some(RubyConfig{strip_comments:None}));#[cfg(feature="go")]assert_eq!(cfg.go,Some(GoConfig{strip_comments:None}));#[cfg(feature="java")]assert_eq!(cfg.java,Some(JavaConfig{strip_comments:None}));#[cfg(feature="csharp")]assert_eq!(cfg.csharp,Some(CSharpConfig{strip_comments:None}));}#[test]fn every_verify_value_is_accepted(){for(text,expected)in[("reparse",ConfigVerify::Reparse),("ast",ConfigVerify::Ast),("external",ConfigVerify::External),]{let cfg=parse(&format!("verify = \"{text}\"\n"));assert_eq!(cfg.verify,Some(expected));}}#[test]fn invalid_verify_value_names_the_value_and_the_alternatives(){let msg=parse_err("verify = \"strict\"\n");assert!(msg.contains("strict"),"{msg}");assert!(msg.contains("reparse"),"{msg}");assert!(msg.contains("ast"),"{msg}");assert!(msg.contains("external"),"{msg}");}#[test]fn unknown_top_level_key_is_an_error(){let msg=parse_err("tokeniser = \"o200k_base\"\n");assert!(msg.contains("tokeniser"),"{msg}");}#[test]fn unknown_python_key_is_an_error(){let msg=parse_err("[python]\nstrip_comment = true\n");assert!(msg.contains("strip_comment"),"{msg}");}#[test]fn unknown_rust_key_is_an_error(){let msg=parse_err("[rust]\nstrip_docs = true\n");assert!(msg.contains("strip_docs"),"{msg}");}#[test]fn unknown_javascript_key_is_an_error(){let msg=parse_err("[javascript]\nstrip_jsdoc = true\n");assert!(msg.contains("strip_jsdoc"),"{msg}");}#[cfg(feature="ruby")]#[test]fn unknown_ruby_key_is_an_error(){let msg=parse_err("[ruby]\nstrip_embdocs = true\n");assert!(msg.contains("strip_embdocs"),"{msg}");}#[cfg(feature="go")]#[test]fn unknown_go_key_is_an_error(){let msg=parse_err("[go]\nstrip_directives = true\n");assert!(msg.contains("strip_directives"),"{msg}");}#[cfg(feature="java")]#[test]fn unknown_java_key_is_an_error(){let msg=parse_err("[java]\nstrip_javadoc = true\n");assert!(msg.contains("strip_javadoc"),"{msg}");}#[cfg(feature="csharp")]#[test]fn unknown_csharp_key_is_an_error(){let msg=parse_err("[csharp]\nstrip_xml_doc = true\n");assert!(msg.contains("strip_xml_doc"),"{msg}");}#[cfg(not(feature="csharp"))]#[test]fn a_csharp_table_names_the_missing_feature_rather_than_being_ignored(){for text in["[csharp]\n","[csharp]\nstrip_comments = true\n","[csharp]\nstrip_xml_doc = true\n",]{let msg=parse_err(text);assert!(msg.contains("built without the `csharp` feature"),"{msg}");}}#[cfg(not(feature="java"))]#[test]fn a_java_table_names_the_missing_feature_rather_than_being_ignored(){for text in["[java]\n","[java]\nstrip_comments = true\n","[java]\nstrip_javadoc = true\n",]{let msg=parse_err(text);assert!(msg.contains("built without the `java` feature"),"{msg}");}}#[cfg(not(feature="go"))]#[test]fn a_go_table_names_the_missing_feature_rather_than_being_ignored(){for text in["[go]\n","[go]\nstrip_comments = true\n","[go]\nstrip_directives = true\n",]{let msg=parse_err(text);assert!(msg.contains("built without the `go` feature"),"{msg}");}}#[cfg(not(feature="ruby"))]#[test]fn a_ruby_table_names_the_missing_feature_rather_than_being_ignored(){for text in["[ruby]\n","[ruby]\nstrip_comments = true\n","[ruby]\nstrip_embdocs = true\n",]{let msg=parse_err(text);assert!(msg.contains("built without the `ruby` feature"),"{msg}");}}#[test]fn the_csharp_table_is_not_spelled_c_sharp(){let msg=parse_err("[\"c#\"]\nstrip_comments = true\n");assert!(msg.contains("c#"),"{msg}");}#[test]fn the_javascript_table_is_not_spelled_js(){let msg=parse_err("[js]\nstrip_comments = true\n");assert!(msg.contains("js"),"{msg}");}#[test]fn wrong_value_type_is_an_error(){let msg=parse_err("[python]\nstrip_comments = \"yes\"\n");assert!(msg.contains("boolean"),"{msg}");let msg=parse_err("tokenizer = 1\n");assert!(msg.contains("string"),"{msg}");}#[test]fn malformed_toml_is_an_error(){let msg=parse_err("tokenizer = \n");assert!(msg.contains("invalid config file"),"{msg}");}#[test]fn load_reads_and_parses_a_file(){let dir=Scratch::new();let path=dir.file("tokenpress.toml","verify = \"ast\"\n");let cfg=FileConfig::load(&path).unwrap();assert_eq!(cfg.verify,Some(ConfigVerify::Ast));}#[test]fn load_reports_an_unreadable_file_with_its_path(){let dir=Scratch::new();let path=dir.0.join("missing.toml");let err=FileConfig::load(&path).unwrap_err();assert!(matches!(err,ConfigError::Read{..}));let msg=err.to_string();assert!(msg.contains("cannot read config file"),"{msg}");assert!(msg.contains("missing.toml"),"{msg}");}#[test]fn load_propagates_parse_errors(){let dir=Scratch::new();let path=dir.file("tokenpress.toml","verify = \"nope\"\n");let err=FileConfig::load(&path).unwrap_err();assert!(matches!(err,ConfigError::Parse(_)));}#[test]fn errors_are_debug_printable(){let err=FileConfig::from_toml_str("verify = \"nope\"\n").unwrap_err();assert!(format!("{err:?}").contains("Parse"));}}