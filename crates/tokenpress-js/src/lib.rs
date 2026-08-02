//! TokenPress for JavaScript/TypeScript.
//!
//! Pipeline: parse the dialect implied by the file's extension (`parser`) →
//! whitespace-minimal re-render (`emit`) → verification (`verify`, plus
//! `external` at [`VerifyLevel::External`]) → token accounting.
//!
//! All dialects `parser::parse` can map from a path are accepted end to end,
//! JSX and TSX included. [`VerifyLevel::External`] hands the output to the
//! language's own toolchain (`tsc --noEmit`, falling back to `node --check`);
//! see [`external`] for what that covers and what it requires on PATH.
//!
//! # JSX reality
//!
//! Whitespace inside JSX element children is semantically significant, so it
//! is emitted **verbatim** — a `.jsx`/`.tsx` file saves tokens only on the
//! JavaScript around its markup. The one JSX construct the comment policy
//! touches is a comment-only expression container: with `strip_comments` it
//! becomes `{}`, which is valid JSX and renders identically.
//!
//! # Comment reality
//!
//! Emitted output is **not comment-preserving**: the whitespace-minimal emit
//! keeps only leading statement-level comments (plus jsdoc, annotation and
//! legal comments) — trailing and inline comments are always dropped, even
//! with `strip_comments: false`. That is a property of the code generator,
//! not a choice made here; see [`emit`] for the full statement, and note that
//! [`verify`] cannot catch comment loss because its canonical form is
//! comment-free by construction.

pub mod emit;
pub mod external;
pub mod parser;
pub mod verify;

use std::path::Path;

use tokenpress_core::{FormatOptions, FormatResult, Formatter, VerifyLevel};

pub use tokenpress_core::{Error, Result};

/// JavaScript/TypeScript-specific choices.
#[derive(Clone, Debug, Default)]
pub struct JsOptions {
    /// JSO1: drop comments entirely. The default (`false`) keeps them —
    /// comments are context for LLMs, so stripping is the opt-in. What
    /// "keeping" can mean is limited by the comment reality documented at the
    /// crate level.
    pub strip_comments: bool,
}

pub struct JsFormatter {
    options: JsOptions,
}

impl JsFormatter {
    pub fn new(options: JsOptions) -> Self {
        Self { options }
    }
}

impl Default for JsFormatter {
    fn default() -> Self {
        Self::new(JsOptions::default())
    }
}

impl Formatter for JsFormatter {
    fn language(&self) -> &'static str {
        "javascript"
    }

    fn supports(&self, path: &Path) -> bool {
        // Exactly the extensions `parser::parse` can map to a dialect, minus
        // none: `.d.ts` comes along via `.ts`.
        path.extension().is_some_and(|e| {
            matches!(
                e.to_str(),
                Some("js" | "mjs" | "cjs" | "jsx" | "ts" | "mts" | "cts" | "tsx")
            )
        })
    }

    fn format(&self, path: &Path, source: &str, options: &FormatOptions) -> Result<FormatResult> {
        // One arena, local to this function: the parsed program borrows from
        // it, so parse, emit and verify all have to run here. See the arena
        // lifetime rule in [`parser`].
        let allocator = parser::Arena::default();
        let program = parser::parse(&allocator, path, source)?;
        let code = emit::emit(&program, self.options.strip_comments);
        match options.verify {
            VerifyLevel::Reparse => {
                verify::reparse(path, &code)?;
            }
            // `equivalent` re-parses the output itself, so no separate
            // `reparse` call is needed at either level.
            VerifyLevel::AstEquiv => {
                verify::equivalent(&program, path, &code)?;
            }
            // External tooling runs *in addition to* the built-in check, and
            // only after it: a candidate the equivalence check already
            // rejected is not worth a process spawn.
            VerifyLevel::External => {
                verify::equivalent(&program, path, &code)?;
                external::check(path, source, &code)?;
            }
        }
        let tokenizer = options.tokenizer.load()?;
        Ok(FormatResult {
            original_tokens: tokenizer.count(source),
            formatted_tokens: tokenizer.count(&code),
            code,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokenpress_core::TokenizerKind;

    fn fmt(name: &str, source: &str) -> String {
        fmt_with(name, source, JsOptions::default())
    }

    fn fmt_with(name: &str, source: &str, options: JsOptions) -> String {
        JsFormatter::new(options)
            .format(Path::new(name), source, &FormatOptions::default())
            .unwrap()
            .code
    }

    #[test]
    fn language_is_javascript() {
        assert_eq!(JsFormatter::default().language(), "javascript");
    }

    #[test]
    fn supports_the_javascript_and_typescript_extensions() {
        let f = JsFormatter::default();
        for name in [
            "a.js", "a.mjs", "a.cjs", "a.jsx", "a.ts", "a.mts", "a.cts", "a.tsx", "a.d.ts",
        ] {
            assert!(f.supports(Path::new(name)), "{name} should be supported");
        }
        for name in ["a.py", "a.rs", "a.txt", "js", "a."] {
            assert!(!f.supports(Path::new(name)), "{name} should be rejected");
        }
    }

    #[test]
    fn js01_minimizes_whitespace() {
        let source = "function add( a , b ) {\n    const sum = a + b;\n    return sum;\n}\n";
        assert_eq!(
            fmt("a.js", source),
            "function add(a,b){const sum=a+b;return sum}"
        );
    }

    #[test]
    fn token_counts_are_reported() {
        let source = "function add( a , b ) {\n    const sum = a + b;\n    return sum;\n}\n";
        let r = JsFormatter::default()
            .format(Path::new("a.js"), source, &FormatOptions::default())
            .unwrap();
        assert!(r.original_tokens > 0);
        assert!(r.formatted_tokens > 0);
        assert!(r.formatted_tokens < r.original_tokens);
        assert!(r.tokens_saved() > 0);
    }

    #[test]
    fn the_path_selects_the_typescript_dialect() {
        let source = "interface Shape {\n    name : string ;\n    size ?: number ;\n}\n";
        assert_eq!(
            fmt("a.ts", source),
            "interface Shape{name:string;size?:number;}"
        );
    }

    #[test]
    fn jso1_keeps_leading_comments_by_default() {
        assert_eq!(
            fmt("a.js", "// note\nconst a = 1;\n"),
            "// note\nconst a=1;"
        );
    }

    #[test]
    fn jso1_strips_comments_on_request() {
        assert_eq!(
            fmt_with(
                "a.js",
                "// note\nconst a = 1;\n",
                JsOptions {
                    strip_comments: true
                }
            ),
            "const a=1;"
        );
    }

    #[test]
    fn trailing_comments_are_dropped_even_when_kept() {
        // Pins the crate-level honesty claim: keeping comments does not keep
        // trailing ones.
        assert_eq!(
            fmt("a.js", "function f(a, b) {\n    return a + b; // tail\n}\n"),
            "function f(a,b){return a+b}"
        );
    }

    #[test]
    fn reparse_only_level_also_passes() {
        let opts = FormatOptions {
            verify: VerifyLevel::Reparse,
            ..FormatOptions::default()
        };
        let r = JsFormatter::default()
            .format(Path::new("a.js"), "const a = 1;\n", &opts)
            .unwrap();
        assert_eq!(r.code, "const a=1;");
    }

    #[test]
    fn ast_equiv_is_the_default_level() {
        let opts = FormatOptions {
            verify: VerifyLevel::AstEquiv,
            ..FormatOptions::default()
        };
        let r = JsFormatter::default()
            .format(Path::new("a.js"), "const a = 1 + 2;\n", &opts)
            .unwrap();
        assert_eq!(r.code, "const a=1+2;");
    }

    #[test]
    fn external_level_adds_the_external_checker() {
        // Runs the real toolchain (`tsc`, else the `node --check` fallback):
        // `.js` is a dialect both accept.
        let opts = FormatOptions {
            verify: VerifyLevel::External,
            ..FormatOptions::default()
        };
        let r = JsFormatter::default()
            .format(Path::new("a.js"), "const a = 1;\n", &opts)
            .unwrap();
        assert_eq!(r.code, "const a=1;");
    }

    #[test]
    fn parse_errors_are_reported() {
        let err = JsFormatter::default()
            .format(
                Path::new("broken.js"),
                "function (",
                &FormatOptions::default(),
            )
            .unwrap_err();
        assert!(
            err.to_string().starts_with("parse error: broken.js:"),
            "{err}"
        );
    }

    #[test]
    fn an_unsupported_extension_reaching_format_is_refused() {
        // `supports` is the caller's filter, but `parser::parse` is the
        // authority: a path it cannot map to a dialect is refused rather than
        // guessed at.
        let err = JsFormatter::default()
            .format(
                Path::new("notes.txt"),
                "const a = 1;\n",
                &FormatOptions::default(),
            )
            .unwrap_err();
        assert_eq!(err.to_string(), "unsupported language for path: notes.txt");
    }

    #[test]
    fn the_path_selects_the_jsx_dialect() {
        // One case for all of it: a fragment, a string attribute, an
        // expression-container attribute, a spread attribute, a nested
        // expression container — and JSX text (`text  here`) whose double
        // space survives while `const x = 1;` next to it is minimized.
        let source = "function App( p ) {\n    const x = 1;\n    return <>\n        <div a=\"1\" b={ x } { ...p }><span>text  here</span>{ x + 1 }</div>\n    </>;\n}\n";
        assert_eq!(
            fmt("a.jsx", source),
            "function App(p){const x=1;return<>\n        <div a=\"1\" b={x}{...p}><span>text  here</span>{x+1}</div>\n    </>}"
        );
    }

    #[test]
    fn the_path_selects_the_tsx_dialect() {
        // TypeScript *and* JSX in one file: an interface, a return-type
        // annotation and a generic function whose `<T>` must be emitted as
        // `<T,>` to stay unambiguous against a JSX element.
        let source = "interface Props {\n    name : string ;\n}\n\nconst Greet = ( p : Props ) : JSX.Element => <span title={ p.name }>Hi, { p.name }!</span>;\n\nfunction List< T >( items : T[] ) {\n    return <ul>{ items.map( ( i ) => <li>{ String( i ) }</li> ) }</ul>;\n}\n";
        assert_eq!(
            fmt("a.tsx", source),
            "interface Props{name:string;}const Greet=(p:Props):JSX.Element=><span title={p.name}>Hi, {p.name}!</span>;function List<T,>(items:T[]){return<ul>{items.map(i=><li>{String(i)}</li>)}</ul>}"
        );
    }

    #[test]
    fn jso1_empties_a_comment_only_container_when_stripping() {
        let source = "const a = <div>{/* c */}</div>;\n";
        // The default keeps it: a JSX expression container's leading comment
        // is one of the classes oxc_codegen preserves.
        assert_eq!(fmt("a.jsx", source), "const a=<div>{/* c */}</div>;");
        // Stripped, the container stays — `{}` is valid JSX and renders
        // exactly like `{/* c */}` did.
        assert_eq!(
            fmt_with(
                "a.jsx",
                source,
                JsOptions {
                    strip_comments: true
                }
            ),
            "const a=<div>{}</div>;"
        );
    }

    #[test]
    fn jsx_trailing_comments_are_dropped_even_when_kept() {
        // Pins the same reality as `trailing_comments_are_dropped_even_when_kept`
        // for JSX: the container's leading comment survives, a trailing one
        // inside a container does not.
        assert_eq!(
            fmt(
                "a.jsx",
                "const a = <div>\n  {/* keep */}\n  {x /* tail */}\n</div>;\n"
            ),
            "const a=<div>\n  {/* keep */}\n  {x}\n</div>;"
        );
    }

    #[test]
    fn invalid_jsx_is_a_parse_error_and_yields_no_output() {
        let err = JsFormatter::default()
            .format(
                Path::new("broken.jsx"),
                "const a = <div>hi;\n",
                &FormatOptions::default(),
            )
            .unwrap_err();
        assert!(matches!(err, Error::Parse(_)), "{err}");
        assert!(
            err.to_string().starts_with("parse error: broken.jsx:"),
            "{err}"
        );
    }

    #[test]
    fn formatting_is_idempotent() {
        let sources = [
            (
                "a.js",
                "class P {\n    constructor( x ) {\n        this.x = x;\n    }\n}\n",
            ),
            ("a.js", "// note\nconst a = 1;\n"),
            ("a.ts", "enum Color {\n    Red = 1 ,\n    Green ,\n}\n"),
            ("a.mjs", "export const a = 1;\n"),
            (
                "a.jsx",
                "const el = <>\n  <div className=\"box\" id={ id } { ...rest }>hi  there</div>\n  {/* c */}\n</>;\n",
            ),
            (
                "a.tsx",
                "const Greet = ( p : { name : string } ) => <span title={ p.name }>Hi, { p.name }!</span>;\n",
            ),
        ];
        for (name, src) in sources {
            let once = fmt(name, src);
            assert_eq!(fmt(name, &once), once, "not idempotent for {src:?}");
        }
    }

    #[test]
    fn tokenizer_choice_is_respected() {
        let opts = FormatOptions {
            tokenizer: TokenizerKind::Cl100kBase,
            ..FormatOptions::default()
        };
        let r = JsFormatter::default()
            .format(Path::new("a.js"), "const a = 1;\n", &opts)
            .unwrap();
        assert!(r.original_tokens > 0);
    }
}
