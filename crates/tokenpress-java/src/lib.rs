//! TokenPress for Java — the Java-specific half of the tree-sitter backend.
//!
//! Pipeline: parse ([`config`] names the grammar,
//! [`tokenpress_treesitter::parser`] drives it) → whitespace-minimal re-emit
//! over the source bytes under the comment policy ([`policy`]) →
//! verification ([`tokenpress_treesitter::verify`]) → token accounting. The
//! path is only used to decide whether this backend claims the file
//! ([`paths`]); the grammar has no dialect, filepath or version selector, so
//! [`JavaFormatter::format`] never reads it.
//!
//! # The crate split
//!
//! `tokenpress-treesitter` is the grammar-agnostic engine: one tree-sitter
//! runtime, the parse gate, the equivalence artifact, the protected-span
//! model, the whitespace rewriter and the comment stripper, none of which
//! knows which language it is looking at. **This** crate holds what cannot be
//! generic: the grammar configuration ([`config`]), the path set the backend
//! claims ([`paths`]), the comment hazard surface ([`policy`]) and the
//! [`tokenpress_core::Formatter`] implementation that composes them.
//!
//! [`config`] answers *what the grammar is*: which node kinds are comments,
//! which are literals whose bytes may never be touched, whether a newline can
//! change meaning. [`policy`] answers *what the language does with a comment*.
//! A grammar reaches the engine as configuration, not as a dependency, so
//! `tree-sitter-java` is named in exactly one place — [`config`] — and this
//! crate never names the `tree-sitter` runtime at all.
//!
//! # Whitespace reality
//!
//! Minimization rewrites the gaps between protected spans and nothing else: a
//! whitespace run that contained a `\n` becomes exactly one `\n`, every other
//! run becomes exactly one space, and the file's leading run is dropped. The
//! emitter therefore **never joins two lines and never introduces one**.
//! Java has no automatic semicolon insertion and would tolerate joining, but
//! line structure is preserved all the same because a `//` comment ends at
//! the newline after it: collapsing that newline to a space would put the
//! rest of the line inside the comment. Indentation, trailing whitespace,
//! blank lines and every alignment column are parts of runs that already
//! carried their newline, and CRLF normalises to LF. A text block's interior
//! is not whitespace between spans — it *is* a span — so its significant
//! indentation is copied verbatim; see [`config`] for why one
//! `string_literal` kind covers it.
//!
//! # Comment reality
//!
//! Nothing is dropped behind the caller's back: at the default settings
//! **every comment survives, byte for byte**. [`JavaOptions::strip_comments`]
//! is the opt-in that deletes them, and what it deletes includes **Javadoc**
//! — a `/** … */` block is an ordinary `block_comment` to the grammar, so
//! the public API documentation of a stripped file goes with the rest of its
//! comments. That is the flag working, not a caveat about it: it is where
//! most of the 49 % below comes from, it is asked for explicitly, and at the
//! default settings not one byte of documentation is dropped.
//!
//! `javac` reads nothing out of a comment, so unlike Go — where a comment
//! carries build constraints, compiler directives and the cgo preamble —
//! there is no keep-list a correct output depends on, no column-0 promotion
//! rule and no verbatim prologue. Java's one hazard is the opposite shape:
//! `javac` decodes `\uXXXX` **before** lexing (JLS 3.3) and tree-sitter-java
//! does not, so a comment carrying an escape that decodes to a comment
//! terminator has live code inside its node. That file is left byte for byte
//! identical at **both** settings and reports no savings —
//! [`policy::has_escape_hazard`], the analogue of Go's cgo bail-out. See
//! [`policy`] for what measured each decision.
//!
//! # The simplification over Go
//!
//! Because Java has no column-sensitive comment syntax, this backend uses
//! the engine's plain
//! [`strip_comments`](tokenpress_treesitter::emit::strip_comments) rather
//! than its `_pinned` sibling, and **both** `strip_comments` settings run one
//! path that differs only in the keep predicate — the private
//! `comment_policy` below is the whole of the difference.
//! Go needs two compositions and a pinning predicate at both settings,
//! because `//go:generate` is a directive only at column 0 and a prologue's
//! blank line is part of a build constraint's meaning. Neither exists here,
//! so there is nothing for the default settings to defend beyond the
//! bail-out they share.
//!
//! # Measured savings
//!
//! Measured with this backend over apache/commons-lang 3.17.0 (500 `.java`
//! files, 7,358,712 bytes), with re-parse plus equivalence enforced and
//! refusals never written:
//!
//! | setting | bytes | o200k |
//! | --- | --- | --- |
//! | comments kept (default) | **−11.03 %** | **−6.15 %** |
//! | `strip_comments` | **−49.05 %** | **−45.51 %** |
//!
//! 500/500 formatted in both configurations, **0** parse refusals and **0**
//! equivalence refusals, with all 1,000 written outputs accepted by `javac`'s
//! parse gate — and commons-lang3's own 11,720-test suite stays green over
//! the formatted tree at both settings. A second corpus (the gson 2.11.0 and
//! commons-lang3 3.17.0 sources jars, 333 files / 4,338,597 bytes) gives
//! −8.01 % / −4.66 % and −70.76 % / −70.39 %, 333/333 with zero refusals.
//! The stripping figures are far larger than Go's because Java is
//! Javadoc-dense — 5,711 `/**` occurrences across 332 of those 333 files —
//! and a sources jar is doc-heavier still than a working repository, which is
//! why the two corpora differ so much.
//!
//! # Encoding is refused upstream
//!
//! Java source has no fixed encoding: `javac`'s is `-encoding`-configurable
//! and defaults to the platform charset, so a Latin-1 file is legal Java to a
//! project configured for it (measured: a lone `0xE9` byte is `error:
//! unmappable character (0xE9) for encoding UTF-8` by default and exit 0
//! under `-encoding ISO-8859-1`). Because
//! [`tokenpress_core::Formatter::format`] takes `&str` and the CLI reads with
//! `std::fs::read_to_string`, such a file never reaches this backend at all —
//! it is **refused at read time**. That is the Ruby situation, not the Go
//! one: Go source is UTF-8 by specification and the contract costs that
//! backend nothing, whereas here it is a real restriction. It costs nothing
//! measurable in practice — 0 of the 833 corpus files are non-UTF-8 — and
//! emission stays on bytes all the same, because that is the engine's model.
//!
//! # Verification
//!
//! `Reparse` re-parses the output; `AstEquiv` compares the comparable
//! artifacts of input and output (which re-parses the output too, so no
//! separate re-parse is needed). `External` runs the `AstEquiv` check and then
//! hands the output to `javac` itself, stopped after its parse phase
//! (`javac -XDshould-stop.ifNoError=PARSE`), which must be on PATH; see
//! [`external`] for what that covers, what it requires, and why the gate
//! self-tests before it is trusted. Output that fails is discarded with
//! [`Error::Verification`] and never returned.
pub mod config;pub mod external;pub mod paths;pub mod policy;use std::path::Path;use tokenpress_core::{FormatOptions,FormatResult,Formatter,VerifyLevel};use tokenpress_treesitter::emit::{self,CommentPolicy};use tokenpress_treesitter::{parser,verify};pub use tokenpress_core::{Error,Result};
/// Java-specific choices.
#[derive(Clone,Debug,Default)]pub struct JavaOptions{
/// JVO1: drop comments. The default (`false`) keeps every one of them
/// verbatim — comments are context for LLMs, so stripping is the opt-in.
/// Javadoc is a comment like any other and goes with them; see the crate
/// docs.
pub strip_comments:bool,}pub struct JavaFormatter{options:JavaOptions,}impl JavaFormatter{pub fn new(options:JavaOptions)->Self{Self{options}}}impl Default for JavaFormatter{fn default()->Self{Self::new(JavaOptions::default())}}
/// The keep predicate of the comments-kept configuration.
///
/// A named function rather than a closure so it has the same
/// `fn(&[u8]) -> bool` type as [`policy::is_semantic_comment`], which is what
/// lets both configurations be one nameable [`policy::JavaCommentPolicy`].
fn keep_every_comment(_bytes:&[u8])->bool{true}
/// The policy for a given `strip_comments` setting.
///
/// The two configurations differ **only** in the keep predicate, and that is
/// the whole of Java's simplification over Go: the prologue is the constant
/// empty range and the escape bail-out is a rule about a grammar
/// disagreement rather than about deleting anything, so both are shared
/// unchanged. There is no second composition and no pinning predicate,
/// because Java has no column-sensitive comment syntax for indentation
/// collapse to create.
///
/// The `strip_comments` arm is exactly [`policy::comment_policy`], assembled
/// here from the same three callbacks so the differing one is visible in one
/// place; `the_stripping_setting_is_the_policy_modules_own_policy` pins that
/// the two cannot drift apart.
fn comment_policy(strip_comments:bool)->policy::JavaCommentPolicy{let keep_comment=if strip_comments{policy::is_semantic_comment}else{keep_every_comment as fn(&[u8])->bool};CommentPolicy::new(keep_comment,policy::no_prologue,policy::has_escape_hazard)}impl Formatter for JavaFormatter{fn language(&self)->&'static str{"java"}fn supports(&self,path:&Path)->bool{paths::supports_path(path)}
/// `path` is not read: the grammar has no dialect, filepath or version
/// selector, so the source is the whole input — `module-info.java` and
/// `package-info.java` included. It stays in the signature because the
/// trait's other implementations need it.
fn format(&self,_path:&Path,source:&str,options:&FormatOptions)->Result<FormatResult>{let bytes=source.as_bytes();let config=config::java_config();let tree=parser::parse(&config,bytes)?;let plan=emit::strip_comments_plan(&config,&tree,bytes,&comment_policy(self.options.strip_comments),);let emitted=emit::strip_comments(bytes,&plan,emit::minimize(&config));let code=String::from_utf8_lossy(&emitted).into_owned();match options.verify{VerifyLevel::Reparse=>{verify::reparse(&config,code.as_bytes())?;}VerifyLevel::AstEquiv=>{verify::equivalent(&config,bytes,code.as_bytes())?;}VerifyLevel::External=>{verify::equivalent(&config,bytes,code.as_bytes())?;external::check(source,&code)?;}}let tokenizer=options.tokenizer.load()?;Ok(FormatResult{original_tokens:tokenizer.count(source),formatted_tokens:tokenizer.count(&code),code,})}}#[cfg(test)]mod tests{use super::*;use std::path::Path;use tokenpress_core::{FormatOptions,Formatter,TokenizerKind,VerifyLevel};fn fmt(source:&str)->String{fmt_with(source,JavaOptions::default())}fn stripped(source:&str)->String{fmt_with(source,JavaOptions{strip_comments:true,},)}fn fmt_with(source:&str,options:JavaOptions)->String{JavaFormatter::new(options).format(Path::new("A.java"),source,&FormatOptions::default()).unwrap().code}#[test]fn language_is_java(){assert_eq!(JavaFormatter::default().language(),"java");}#[test]fn supports_the_java_paths(){let f=JavaFormatter::default();for name in["A.java","src/main/java/org/example/Thing.java","module-info.java",]{assert!(f.supports(Path::new(name)),"{name} should be supported");}for name in["a.go","a.rb","A.JAVA","A.class","A.jsh","java"]{assert!(!f.supports(Path::new(name)),"{name} should be rejected");}}#[test]fn jv01_minimizes_whitespace(){let source="/**\n * Doc.\n */\npublic class A {\n    /** The field. */\n    private int x = 1;\n\n    public int x() {\n        return x;\n    }\n}\n";assert_eq!(fmt(source),"/**\n * Doc.\n */\npublic class A {\n/** The field. */\nprivate int x = 1;\npublic int x() {\nreturn x;\n}\n}\n");}#[test]fn jvo1_defaults_to_keeping_comments(){assert!(!JavaOptions::default().strip_comments);}#[test]fn a_text_block_survives_byte_for_byte_at_both_settings(){let source="class A {\n    // note\n    String s = \"\"\"\n        one\n          two\n        \"\"\";\n}\n";let interior="\"\"\"\n        one\n          two\n        \"\"\"";let kept=fmt(source);assert!(kept.contains(interior),"{kept:?}");assert_eq!(kept,"class A {\n// note\nString s = \"\"\"\n        one\n          two\n        \"\"\";\n}\n");let stripped=stripped(source);assert!(stripped.contains(interior),"{stripped:?}");assert_eq!(stripped,"class A {\nString s = \"\"\"\n        one\n          two\n        \"\"\";\n}\n");}#[test]fn jv01_collapses_alignment_and_blank_lines(){let source="class A {\n    int    a   = 1;\n\n    int    bbb = 2;\n}\n";assert_eq!(fmt(source),"class A {\nint a = 1;\nint bbb = 2;\n}\n");}#[test]fn jv01_normalizes_crlf(){let source="class A {\r\n\r\n    int a = 1;\r\n}\r\n";assert_eq!(fmt(source),"class A {\nint a = 1;\n}\n");}#[test]fn jv01_drops_the_leading_run_of_the_file(){assert_eq!(fmt("\n\n   class  A  {}\n"),"class A {}\n");}#[test]fn jvo1_keeps_every_comment_by_default(){let source="class A {\n    // leading\n    int x = 1; // trailing\n    /* inline */ int y = 2;\n}\n";assert_eq!(fmt(source),"class A {\n// leading\nint x = 1; // trailing\n/* inline */ int y = 2;\n}\n");}#[test]fn jvo1_strips_comments_on_request(){let source="class A {\n    // leading\n    int x = 1; // trailing\n    /* inline */ int y = 2;\n}\n";assert_eq!(stripped(source),"class A {\nint x = 1;\nint y = 2;\n}\n");}#[test]fn a_comment_only_file_is_emptied_rather_than_refused(){let source="// only a comment\n";assert_eq!(stripped(source),"");let r=JavaFormatter::new(JavaOptions{strip_comments:true,}).format(Path::new("A.java"),source,&FormatOptions::default()).unwrap();assert_eq!(r.formatted_tokens,0);}#[test]fn jvo1_deletes_javadoc_when_stripping(){let source="/**\n * The class.\n */\npublic class A {\n    /** The method. */\n    public void m() {}\n}\n";assert_eq!(stripped(source),"public class A {\npublic void m() {}\n}\n");assert_eq!(fmt(source),"/**\n * The class.\n */\npublic class A {\n/** The method. */\npublic void m() {}\n}\n");}#[test]fn an_indented_comment_is_emitted_at_column_0(){let source="class A {\n    void m() {\n        // note\n        int x = 1;\n    }\n}\n";assert_eq!(fmt(source),"class A {\nvoid m() {\n// note\nint x = 1;\n}\n}\n");assert_eq!(stripped(source),"class A {\nvoid m() {\nint x = 1;\n}\n}\n");}#[test]fn the_stripping_setting_is_the_policy_modules_own_policy(){let source=b"/** Doc. */\nclass A {\n// note\nint x = 1; /* trailing */\n}\n";let config=config::java_config();let theirs=emit::strip_comments_source(&config,source,&policy::comment_policy()).unwrap();let ours=emit::strip_comments_source(&config,source,&comment_policy(true)).unwrap();assert_eq!(ours,theirs);}#[test]fn a_comment_byte_adjacent_to_a_literal_takes_only_itself(){let source="class A {\n    void m() {\n        System.out.println(/*c*/\"lit\");\n    }\n}\n";assert_eq!(stripped(source),"class A {\nvoid m() {\nSystem.out.println( \"lit\");\n}\n}\n");let after="class A {\n    void m() {\n        System.out.println(\"lit\"/*c*/);\n    }\n}\n";assert_eq!(stripped(after),"class A {\nvoid m() {\nSystem.out.println(\"lit\" );\n}\n}\n");}#[test]fn an_escape_hazard_file_is_left_byte_identical_at_both_settings(){let source="class A {\nint x = 1; // c \\u000A int y = 2;\nint z = 3;\n}\n";assert_eq!(fmt(source),source);assert_eq!(stripped(source),source);}#[test]fn an_escape_hazard_file_reports_no_savings(){let source="class A {\nint x = 1; // c \\u000A int y = 2;\nint z = 3;\n}\n";let r=JavaFormatter::default().format(Path::new("A.java"),source,&FormatOptions::default()).unwrap();assert_eq!(r.original_tokens,r.formatted_tokens);assert_eq!(r.tokens_saved(),0);}#[test]fn token_counts_are_reported(){let source="public class A {\n    /** Doc. */\n    public int x() {\n        return 1;\n    }\n}\n";let r=JavaFormatter::default().format(Path::new("A.java"),source,&FormatOptions::default()).unwrap();assert!(r.original_tokens>0);assert!(r.formatted_tokens>0);assert!(r.formatted_tokens<r.original_tokens);assert!(r.tokens_saved()>0);}#[test]fn the_path_is_not_read(){let r=JavaFormatter::default().format(Path::new("notes.txt"),"class  A  {}\n",&FormatOptions::default(),).unwrap();assert_eq!(r.code,"class A {}\n");}#[test]fn reparse_only_level_also_passes(){let opts=FormatOptions{verify:VerifyLevel::Reparse,..FormatOptions::default()};let r=JavaFormatter::default().format(Path::new("A.java"),"class  A  {}\n",&opts).unwrap();assert_eq!(r.code,"class A {}\n");}#[test]fn ast_equiv_is_the_default_level(){assert_eq!(FormatOptions::default().verify,VerifyLevel::AstEquiv);let opts=FormatOptions{verify:VerifyLevel::AstEquiv,..FormatOptions::default()};let r=JavaFormatter::default().format(Path::new("A.java"),"class  A  {}\n",&opts).unwrap();assert_eq!(r.code,"class A {}\n");}fn external()->FormatOptions{FormatOptions{verify:VerifyLevel::External,..FormatOptions::default()}}#[test]fn external_level_runs_javac_on_top_of_the_built_in_check(){let r=JavaFormatter::default().format(Path::new("A.java"),"public class A {\n\n    int f(int a) {\n        return a;\n    }\n}\n",&external(),).unwrap();assert_eq!(r.code,"public class A {\nint f(int a) {\nreturn a;\n}\n}\n");}#[test]fn external_level_does_not_blame_input_javac_already_rejects(){let r=JavaFormatter::default().format(Path::new("A.java"),"class A {\n    long x = 99999999999;\n}\n",&external(),).unwrap();assert_eq!(r.code,"class A {\nlong x = 99999999999;\n}\n");}#[test]fn parse_errors_are_reported(){let err=JavaFormatter::default().format(Path::new("Broken.java"),"class A {\n    void m() {\n",&FormatOptions::default(),).unwrap_err();assert!(matches!(err,Error::Parse(_)),"{err}");assert!(err.to_string().starts_with("parse error: syntax error at byte "),"{err}");}#[test]fn formatting_is_idempotent(){let sources=["class A {\n    String s = \"\"\"\n        one\n          two\n        \"\"\";\n}\n","/**\n * Doc.\n */\npublic class A {\n    /** The field. */\n    private int x = 1;\n}\n","public record Point(int x, int y) {\n    public Point {\n        // note\n        assert x >= 0;\n    }\n}\n","class A {\n    int m(int k) {\n        return switch (k) {\n            case 1 -> 10;\n            default -> 0;\n        };\n    }\n}\n","/** Module docs. */\nmodule org.example {\n    requires java.base;\n    exports org.example.api;\n}\n","class A {\nint x = 1; // c \\u000A int y = 2;\nint z = 3;\n}\n",];for source in sources{let once=fmt(source);assert_eq!(fmt(&once),once,"not idempotent for {source:?}");let once_stripped=stripped(source);assert_eq!(stripped(&once_stripped),once_stripped,"not idempotent when stripping for {source:?}");}}#[test]fn tokenizer_choice_is_respected(){let opts=FormatOptions{tokenizer:TokenizerKind::Cl100kBase,..FormatOptions::default()};let r=JavaFormatter::default().format(Path::new("A.java"),"class  A  {}\n",&opts).unwrap();assert!(r.original_tokens>0);}}