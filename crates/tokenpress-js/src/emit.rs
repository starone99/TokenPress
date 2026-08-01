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

use oxc_codegen::{Codegen, CodegenOptions, CommentOptions};

use crate::parser::Program;

/// Renders `program` with minimal whitespace.
///
/// `strip_comments` selects whether comments survive at all; see the
/// module-level comment reality note for what "survive" can mean.
pub fn emit(program: &Program<'_>, strip_comments: bool) -> String {
    let comments = if strip_comments {
        CommentOptions::disabled()
    } else {
        CommentOptions::default()
    };
    let options = CodegenOptions {
        minify: true,
        comments,
        ..CodegenOptions::default()
    };
    Codegen::new().with_options(options).build(program).code
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{self, Arena};
    use std::path::Path;

    fn render(name: &str, source: &str, strip_comments: bool) -> String {
        let allocator = Arena::default();
        let program = parser::parse(&allocator, Path::new(name), source).unwrap();
        emit(&program, strip_comments)
    }

    fn keep(name: &str, source: &str) -> String {
        render(name, source, false)
    }

    #[test]
    fn minifies_a_function() {
        let source = "function add( a , b ) {\n    const sum = a + b;\n    return sum;\n}\n";
        assert_eq!(
            keep("a.js", source),
            "function add(a,b){const sum=a+b;return sum}"
        );
    }

    #[test]
    fn minifies_a_class() {
        let source = "class Point {\n    constructor( x , y ) {\n        this.x = x;\n        this.y = y;\n    }\n\n    get length() {\n        return this.x;\n    }\n}\n";
        assert_eq!(
            keep("a.js", source),
            "class Point{constructor(x,y){this.x=x;this.y=y}get length(){return this.x}}"
        );
    }

    #[test]
    fn preserves_template_literal_whitespace() {
        let source = "const t = `a   b\n   c ${ x }   d`;\n";
        assert_eq!(keep("a.js", source), "const t=`a   b\n   c ${x}   d`;");
    }

    #[test]
    fn minifies_typescript_interface() {
        let source = "interface Shape {\n    name : string ;\n    size ?: number ;\n}\n";
        assert_eq!(
            keep("a.ts", source),
            "interface Shape{name:string;size?:number;}"
        );
    }

    #[test]
    fn minifies_typescript_enum() {
        let source = "enum Color {\n    Red = 1 ,\n    Green ,\n}\n";
        assert_eq!(keep("a.ts", source), "enum Color{Red=1,Green}");
    }

    #[test]
    fn minifies_typescript_namespace() {
        let source = "namespace Outer {\n    export const a : number = 1 ;\n}\n";
        assert_eq!(
            keep("a.ts", source),
            "namespace Outer{export const a:number=1}"
        );
    }

    #[test]
    fn minifies_declare_module() {
        let source = "declare module \"pkg\" {\n    export function f( x : number ) : void ;\n}\n";
        assert_eq!(
            keep("a.d.ts", source),
            "declare module \"pkg\"{export function f(x:number):void;}"
        );
    }

    #[test]
    fn asi_hazards_survive_a_round_trip() {
        let source = "const a = 1;\nconst b = 2;\n(function () { })();\n";
        let out = keep("a.js", source);
        assert_eq!(out, "const a=1;const b=2;(function(){})();");
        let allocator = Arena::default();
        parser::parse(&allocator, Path::new("a.js"), &out).unwrap();
    }

    #[test]
    fn leading_statement_comment_is_kept_and_stripped() {
        let source = "// note\nconst a = 1;\n";
        assert_eq!(keep("a.js", source), "// note\nconst a=1;");
        assert_eq!(render("a.js", source, true), "const a=1;");
    }

    #[test]
    fn trailing_comments_are_dropped_even_when_kept() {
        // Documented oxc_codegen behaviour, not a TokenPress choice: only
        // leading statement-level comments survive.
        let source = "function f(a, b) {\n    return a + b; // tail\n}\n";
        assert_eq!(keep("a.js", source), "function f(a,b){return a+b}");
    }
}
