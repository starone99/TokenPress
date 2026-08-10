//! Whitespace-minimal renderer over `oxc_codegen`.
//!
//! This module is the **sole** `oxc_codegen` access point, exactly as
//! [`crate::parser`] is the sole `oxc_parser`/`oxc_ast`/`oxc_span` access
//! point: the oxc crates are pre-1.0 with no semver guarantees (pinned
//! exactly in `Cargo.toml`), so every code-generation API used by this crate
//! is confined here and a future codegen swap stays a one-file change.
//!
//! # Comment reality
//!
//! `oxc_codegen` preserves only *leading, statement-level* comments, plus
//! jsdoc (`/** */`), annotation (`#__PURE__`, webpack/vite/coverage) and
//! legal (`//!`, `/*!`, `@license`, `@preserve`) comments. **Trailing
//! comments and comments in expression position are dropped**, even with
//! `strip_comments == false`. That is a property of the code generator, not
//! a choice made here; the tests below pin it so the limitation is never
//! silently claimed away.
//!
//! # JSX reality
//!
//! JSX text is emitted **verbatim**: whitespace inside element children is
//! semantically significant, so `oxc_codegen` never compresses it and a
//! `.jsx`/`.tsx` file saves tokens only on the JavaScript around its markup.
//! The single comment class the JSX dialect adds is a comment-only expression
//! container: `{/* c */}` survives at the default settings and collapses to
//! `{}` when comments are stripped — valid JSX that renders identically. In
//! the `.tsx` dialect a bare type-parameter list would be ambiguous with a JSX
//! element, so `<T>` is emitted as `<T,>`. All three are pinned below.
use oxc_codegen::{Codegen,CodegenOptions,CommentOptions};use crate::parser::Program;
/// Renders `program` with minimal whitespace.
///
/// `strip_comments` selects whether comments survive at all; see the
/// module-level comment reality note for what "survive" can mean.
pub fn emit(program:&Program<'_>,strip_comments:bool)->String{let comments=if strip_comments{CommentOptions::disabled()}else{CommentOptions::default()};render(program,CodegenOptions{minify:true,comments,..CodegenOptions::default()},)}
/// Renders `program` in the canonical full-minify form used by
/// [`crate::verify`] to compare two programs for equivalence.
///
/// Deliberately built from [`CodegenOptions::minify()`] rather than from
/// [`emit`]: the canonical form must stay a fixed, comment-free normal form
/// even if the comment policy of [`emit`] changes. Comments are erased by
/// construction here, which is exactly why the verifier cannot see comment
/// loss — see the note in [`crate::verify`].
pub(crate)fn canonical(program:&Program<'_>)->String{render(program,CodegenOptions::minify())}fn render(program:&Program<'_>,options:CodegenOptions)->String{Codegen::new().with_options(options).build(program).code}#[cfg(test)]mod tests{use super::*;use crate::parser::{self,Arena};use std::path::Path;fn render(name:&str,source:&str,strip_comments:bool)->String{let allocator=Arena::default();let program=parser::parse(&allocator,Path::new(name),source).unwrap();emit(&program,strip_comments)}fn keep(name:&str,source:&str)->String{render(name,source,false)}#[test]fn minifies_a_function(){let source="function add( a , b ) {\n    const sum = a + b;\n    return sum;\n}\n";assert_eq!(keep("a.js",source),"function add(a,b){const sum=a+b;return sum}");}#[test]fn minifies_a_class(){let source="class Point {\n    constructor( x , y ) {\n        this.x = x;\n        this.y = y;\n    }\n\n    get length() {\n        return this.x;\n    }\n}\n";assert_eq!(keep("a.js",source),"class Point{constructor(x,y){this.x=x;this.y=y}get length(){return this.x}}");}#[test]fn preserves_template_literal_whitespace(){let source="const t = `a   b\n   c ${ x }   d`;\n";assert_eq!(keep("a.js",source),"const t=`a   b\n   c ${x}   d`;");}#[test]fn minifies_typescript_interface(){let source="interface Shape {\n    name : string ;\n    size ?: number ;\n}\n";assert_eq!(keep("a.ts",source),"interface Shape{name:string;size?:number;}");}#[test]fn minifies_typescript_enum(){let source="enum Color {\n    Red = 1 ,\n    Green ,\n}\n";assert_eq!(keep("a.ts",source),"enum Color{Red=1,Green}");}#[test]fn minifies_typescript_namespace(){let source="namespace Outer {\n    export const a : number = 1 ;\n}\n";assert_eq!(keep("a.ts",source),"namespace Outer{export const a:number=1}");}#[test]fn minifies_declare_module(){let source="declare module \"pkg\" {\n    export function f( x : number ) : void ;\n}\n";assert_eq!(keep("a.d.ts",source),"declare module \"pkg\"{export function f(x:number):void;}");}#[test]fn asi_hazards_survive_a_round_trip(){let source="const a = 1;\nconst b = 2;\n(function () { })();\n";let out=keep("a.js",source);assert_eq!(out,"const a=1;const b=2;(function(){})();");let allocator=Arena::default();parser::parse(&allocator,Path::new("a.js"),&out).unwrap();}#[test]fn leading_statement_comment_is_kept_and_stripped(){let source="// note\nconst a = 1;\n";assert_eq!(keep("a.js",source),"// note\nconst a=1;");assert_eq!(render("a.js",source,true),"const a=1;");}#[test]fn trailing_comments_are_dropped_even_when_kept(){let source="function f(a, b) {\n    return a + b; // tail\n}\n";assert_eq!(keep("a.js",source),"function f(a,b){return a+b}");}#[test]fn jsx_text_is_verbatim_while_the_code_around_it_is_minified(){let source="function App() {\n    const x = 1;\n    return <div a=\"1\" b={ x } { ...p }><span>text  here</span>{ x + 1 }</div>;\n}\n";assert_eq!(keep("a.jsx",source),"function App(){const x=1;return<div a=\"1\" b={x}{...p}><span>text  here</span>{x+1}</div>}");}#[test]fn jsx_fragments_and_nested_containers_round_trip(){let source="const el = <>\n  <p>a  b</p>\n  { items.map( ( i ) => <li>{ i }</li> ) }\n</>;\n";assert_eq!(keep("a.jsx",source),"const el=<>\n  <p>a  b</p>\n  {items.map(i=><li>{i}</li>)}\n</>;");}#[test]fn jsx_whitespace_only_children_survive(){assert_eq!(keep("a.jsx","const a = <div>   </div>;\n"),"const a=<div>   </div>;");assert_eq!(keep("a.jsx","const a = <div>\n\n  x\n\n</div>;\n"),"const a=<div>\n\n  x\n\n</div>;");}#[test]fn a_comment_only_expression_container_becomes_empty_when_stripped(){let source="const a = <div>{/* c */}</div>;\n";assert_eq!(keep("a.jsx",source),"const a=<div>{/* c */}</div>;");assert_eq!(render("a.jsx",source,true),"const a=<div>{}</div>;");}#[test]fn a_trailing_comment_inside_a_container_is_dropped_even_when_kept(){let source="const a = <div>\n  {/* inner */}\n  {x /* tail */}\n</div>;\n";assert_eq!(keep("a.jsx",source),"const a=<div>\n  {/* inner */}\n  {x}\n</div>;");}#[test]fn minifies_a_typed_tsx_component(){let source="interface Props {\n    name : string ;\n}\n\nconst Greet = ( p : Props ) : JSX.Element => <span title={ p.name }>Hi, { p.name }!</span>;\n";assert_eq!(keep("a.tsx",source),"interface Props{name:string;}const Greet=(p:Props):JSX.Element=><span title={p.name}>Hi, {p.name}!</span>;");}#[test]fn tsx_type_parameters_keep_a_disambiguating_trailing_comma(){let source="function List< T >( items : T[] ) {\n    return <ul>{ items.map( ( i ) => <li>{ String( i ) }</li> ) }</ul>;\n}\n";assert_eq!(keep("a.tsx",source),"function List<T,>(items:T[]){return<ul>{items.map(i=><li>{String(i)}</li>)}</ul>}");}}