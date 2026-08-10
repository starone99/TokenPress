//! The C# grammar as engine configuration — the only module that names
//! `tree_sitter_c_sharp`.
//!
//! This crate's grammar boundary, and the analogue of the sibling backends'
//! `parser.rs` under CLAUDE.md's confinement rule (ruff in
//! `tokenpress-python`, oxc in `tokenpress-js`, prism in `tokenpress-ruby`).
//! It is called `config` rather than `parser` because no parsing happens
//! here: a grammar is a runtime value the engine takes as configuration, so
//! all this module does is name the kinds and hand
//! `tree_sitter_c_sharp::LANGUAGE` to the engine. Parsing itself stays in
//! `tokenpress_treesitter::parser`.
//!
//! # What was measured here rather than assumed
//!
//! Every kind name below was checked against `id_for_node_kind(k, true)`
//! before it was written down, the way Java's `text_block` guess was. Two of
//! the answers are not the ones a reader of the C# specification would
//! predict, and both are pinned by tests:
//!
//! - **UTF-8 literals have no kind of their own.** `"hi"u8` is a plain
//!   `string_literal`, `@"hi"u8` a `verbatim_string_literal` and
//!   `"""hi"""u8` a `raw_string_literal`; there is no
//!   `utf8_string_literal` (the name is refused by the constructor). The
//!   `u8` suffix sits *inside* the literal's byte span — it is an anonymous
//!   token of the literal node, not a sibling — so protecting the literal
//!   protects the suffix with it and no fourth kind is needed. This is the
//!   C# echo of Java's `string_literal`-covers-text-blocks finding, in the
//!   opposite direction: Java had one kind where two were expected, C# has
//!   three kinds where six were expected.
//! - **All three interpolated quotings are one kind.** `$"…"`, `$@"…"` and
//!   `$"""…"""` are every one of them an `interpolated_string_expression` —
//!   the interpolation, not the quoting, decides the kind, so
//!   `$@"…"` is *not* a `verbatim_string_literal`.
//!
//! # The grammar defect this pin carries
//!
//! `tree-sitter-c-sharp 0.23.5` cannot parse a slice pattern with a
//! designation: `[var a, .. var r]` produces an `ERROR` node at the
//! `.. var r`, while the bare `[var a, ..]` parses clean. Under the core
//! invariant that is the behaviour we want and did not have to build — a
//! file using that form is refused at the parse gate and never rewritten.
//! `a_slice_pattern_with_a_designation_is_refused_while_the_bare_form_parses`
//! pins it so a future grammar bump that fixes it is *noticed* rather than
//! quietly widening what TokenPress accepts.
//!
//! A second, much more common refusal has the same shape and the same
//! verdict: a `#if` block whose two arms are not each a complete syntactic
//! unit — a directive inside a type parameter list, or one that wraps the
//! `else` of an `if`/`else` — is an `ERROR`. Measured over the corpus below,
//! that is 65 of 945 files, all of them refused rather than mangled.
use tokenpress_treesitter::parser::LanguageConfig;
/// The C# configuration the engine drives.
///
/// - comment kind `comment` — C# has **one**, and that is the structural
///   difference from Java, which has two. `//`, `/* … */` and `///` XML
///   documentation comments are all the same node kind, so a policy that
///   wants to treat XML doc specially **cannot key on the node kind** and
///   has to read the comment's leading bytes.
/// - protected kinds `string_literal`, `verbatim_string_literal`,
///   `raw_string_literal`, `interpolated_string_expression`,
///   `character_literal` — every kind whose bytes carry meaning of their own
///   and must be copied verbatim. Five, against Java's two, because C#
///   spells a string four ways and the grammar gives each a kind; see the
///   module doc for the two surprises in that list. `string_literal_content`,
///   `raw_string_content`, `string_content`, `escape_sequence` and
///   `interpolation` are all *children* of one of these and need not be
///   listed; the engine's walk-and-merge absorbs them. Protecting the whole
///   `interpolated_string_expression` — rather than only its
///   `string_content` leaves — costs the whitespace inside an interpolation
///   hole and buys the raw-interpolated case for free, which is the same
///   trade Go and Java make by protecting whole literals.
/// - newline-sensitive. C# is a brace-and-semicolon language with no
///   automatic semicolon insertion, which is exactly the family the engine's
///   doc comment warns is a trap: the flag does not mean "has ASI", it means
///   "does the emitter preserve line structure". C# needs it preserved for
///   **two** independent reasons, where Java had one — a `//` comment
///   swallows whatever the `false` branch joins onto its line, *and* a
///   preprocessor directive dragged onto the previous line stops being a
///   directive at all. Measured over the JamesNK/Newtonsoft.Json `master`
///   working tree (945 `.cs` files, 7,114,672 bytes; 65 refused at the parse
///   gate for the `#if` reason above, leaving 880 that reach the emitter):
///   `false` is refused by `verify::equivalent` for **872 of those 880**
///   files, `true` for **1** — Java's 247/500 was mild by comparison. The
///   one refusal under `true` is not the flag's doing: a `#region helpers`
///   with trailing spaces puts those spaces inside the `preproc_arg` leaf, so
///   collapsing them moves the comparable artifact and the output is
///   discarded. Safe, not silent, and one file in 880. The 879 that survive
///   are 5,188,596 bytes in and 4,232,226 out, **−18.43 %**, with comments
///   kept — the whole of C1's savings evidence, and not a substitute for the
///   corpus run C6(a) owes.
///
/// Building the configuration is cheap (a `Language` handle plus two small
/// vectors), so callers construct one per operation rather than sharing one.
pub fn csharp_config()->LanguageConfig{LanguageConfig::new(tree_sitter_c_sharp::LANGUAGE.into(),vec!["comment"],vec!["string_literal","verbatim_string_literal","raw_string_literal","interpolated_string_expression","character_literal",],true,).expect("every configured kind is a named node kind of the pinned tree-sitter-c-sharp grammar")}#[cfg(test)]mod tests{use super::*;use tokenpress_treesitter::parser::{parse,Language,LANGUAGE_VERSION,MIN_COMPATIBLE_LANGUAGE_VERSION,};use tokenpress_treesitter::{emit,verify};
/// The grammar as the engine sees it.
fn csharp()->Language{tree_sitter_c_sharp::LANGUAGE.into()}
/// The s-expression of a source that has to parse cleanly.
fn sexp(source:&[u8])->String{parse(&csharp_config(),source).unwrap().root_node().to_sexp()}#[test]fn the_grammar_abi_is_inside_the_runtime_window(){let abi=csharp().abi_version();assert!((MIN_COMPATIBLE_LANGUAGE_VERSION..=LANGUAGE_VERSION).contains(&abi),"tree-sitter-c-sharp reports ABI {abi}, outside [{MIN_COMPATIBLE_LANGUAGE_VERSION}, {LANGUAGE_VERSION}]");assert_eq!(csharp_config().language().abi_version(),abi);}#[test]fn the_configuration_is_the_csharp_one(){let config=csharp_config();assert_eq!(config.comment_kinds(),["comment"]);assert_eq!(config.protected_kinds(),["string_literal","verbatim_string_literal","raw_string_literal","interpolated_string_expression","character_literal"]);assert!(config.newline_sensitive());}#[test]fn every_configured_kind_is_validated_against_the_grammar(){let config=csharp_config();for&kind in config.comment_kinds(){assert!(LanguageConfig::new(csharp(),vec![kind],vec![],true).is_ok(),"{kind} should be a named kind of the grammar");}for&kind in config.protected_kinds(){assert!(LanguageConfig::new(csharp(),vec![],vec![kind],true).is_ok(),"{kind} should be a named kind of the grammar");}let err=LanguageConfig::new(csharp(),vec![],vec!["not_a_csharp_kind"],true).unwrap_err();assert_eq!(err.to_string(),"node kind not in this grammar: not_a_csharp_kind");}#[test]fn javas_two_comment_kinds_are_refused_while_the_single_kind_is_accepted(){for kind in["line_comment","block_comment"]{let err=LanguageConfig::new(csharp(),vec![kind],vec![],true).unwrap_err();assert_eq!(err.to_string(),format!("node kind not in this grammar: {kind}"));}assert!(LanguageConfig::new(csharp(),vec!["comment"],vec![],true).is_ok());}#[test]fn xml_doc_line_and_block_comments_are_all_the_one_comment_kind(){let sexp=sexp(b"/// <summary>Doc.</summary>\n// line\n/* block */\nclass A {}\n");assert_eq!(sexp.matches("(comment)").count(),3,"{sexp}");for absent in["documentation_comment","line_comment","block_comment"]{assert!(!sexp.contains(absent),"{absent} unexpectedly in {sexp}");}}#[test]fn each_string_form_has_the_kind_the_configuration_names(){for(source,kind)in[(&b"class A { string s = \"hi\"; }"[..],"string_literal"),(b"class A { string s = @\"hi\"; }","verbatim_string_literal",),(b"class A { string s = \"\"\"hi\"\"\"; }","raw_string_literal",),(b"class A { string s = $\"a{1}b\"; }","interpolated_string_expression",),(b"class A { char c = 'y'; }","character_literal"),]{let sexp=sexp(source);assert!(sexp.contains(kind),"{kind} missing from {sexp}");}}#[test]fn a_utf8_literal_is_its_unsuffixed_kind_with_the_suffix_inside_the_span(){for(source,kind)in[(&b"class A { System.ReadOnlySpan<byte> s => \"hi\"u8; }"[..],"string_literal",),(b"class A { System.ReadOnlySpan<byte> s => @\"hi\"u8; }","verbatim_string_literal",),(b"class A { System.ReadOnlySpan<byte> s => \"\"\"hi\"\"\"u8; }","raw_string_literal",),]{let sexp=sexp(source);assert!(sexp.contains(kind),"{kind} missing from {sexp}");}for absent in["utf8_string_literal","utf8_verbatim_string_literal","utf8_raw_string_literal",]{let err=LanguageConfig::new(csharp(),vec![],vec![absent],true).unwrap_err();assert_eq!(err.to_string(),format!("node kind not in this grammar: {absent}"));}let source=b"class A { System.ReadOnlySpan<byte> s => \"hi\"u8; }";assert_eq!(emit::minimize_source(&csharp_config(),source).unwrap(),b"class A { System.ReadOnlySpan<byte> s => \"hi\"u8; }".to_vec());}#[test]fn all_three_interpolated_quotings_are_one_kind(){for source in[&b"class A { string s = $\"a{1}b\"; }"[..],b"class A { string s = $@\"a{1}b\"; }",b"class A { string s = $\"\"\"a{1}b\"\"\"; }",]{let sexp=sexp(source);assert!(sexp.contains("interpolated_string_expression"),"{sexp}");for absent in["verbatim_string_literal","raw_string_literal"]{assert!(!sexp.contains(absent),"{absent} unexpectedly in {sexp}");}}}#[test]fn a_raw_string_interior_survives_byte_for_byte(){let source=b"class A {\n    string s = \"\"\"\n        one\n          two\n        \"\"\";\n}\n";let out=emit::minimize_source(&csharp_config(),source).unwrap();let out=String::from_utf8(out).unwrap();assert!(out.contains("\"\"\"\n        one\n          two\n        \"\"\""),"{out:?}");assert_eq!(out,"class A {\nstring s = \"\"\"\n        one\n          two\n        \"\"\";\n}\n");}#[test]fn both_newline_hazards_are_real_and_the_flag_is_what_defends_them(){let joining=LanguageConfig::new(csharp_config().language().clone(),vec!["comment"],vec![],false,).unwrap();for source in[&b"class A {\n    // note\n    int x = 1;\n}\n"[..],b"class A {\n    int x = 1;\n#if FOO\n    int y = 2;\n#endif\n}\n",]{let joined=emit::minimize_source(&joining,source).unwrap();assert!(verify::equivalent(&joining,source,&joined).is_err(),"the `false` branch should be refused for {source:?}");let kept=emit::minimize_source(&csharp_config(),source).unwrap();verify::equivalent(&csharp_config(),source,&kept).unwrap();}}#[test]fn a_directive_argument_with_trailing_spaces_is_refused_not_rewritten(){let config=csharp_config();let trailing=&b"class A {\n#region helpers   \n#endregion\n}\n"[..];let out=emit::minimize_source(&config,trailing).unwrap();assert!(verify::equivalent(&config,trailing,&out).is_err());let clean=&b"class A {\n#region helpers\n#endregion\n}\n"[..];let out=emit::minimize_source(&config,clean).unwrap();verify::equivalent(&config,clean,&out).unwrap();}#[test]fn a_slice_pattern_with_a_designation_is_refused_while_the_bare_form_parses(){let config=csharp_config();let bare=b"class A { int m(int[] xs) => xs switch { [var a, ..] => a, _ => 0 }; }\n";assert!(parse(&config,bare).is_ok());let designated=b"class A { int m(int[] xs) => xs switch { [var a, .. var r] => a, _ => 0 }; }\n";let err=parse(&config,designated).unwrap_err();assert!(err.to_string().starts_with("parse error: syntax error at byte "),"{err}");}#[test]fn preprocessor_directives_parse(){for source in[&b"#pragma warning disable 618\nclass A {}\n#pragma warning restore 618\n"[..],b"#define FOO\nclass A {}\n",b"#nullable enable\nclass A {}\n",b"#line 1 \"A.cs\"\nclass A {}\n",b"#warning careful\nclass A {}\n",b"class A {\n#if FOO\n    int x = 1;\n#endif\n}\n",b"class A {\n    void m() {\n        #region helpers\n        int x = 1;\n        #endregion\n    }\n}\n",]{assert!(parse(&csharp_config(),source).is_ok(),"{source:?}");}}#[test]fn a_conditional_block_that_breaks_syntactic_structure_is_refused(){let config=csharp_config();for source in[&b"interface I<\n#if X\nout\n#endif\n T> {}\n"[..],b"class A { void m() { if (true) { }\n#if X\n else { }\n#endif\n } }\n",]{assert!(parse(&config,source).is_err(),"{source:?}");}}}