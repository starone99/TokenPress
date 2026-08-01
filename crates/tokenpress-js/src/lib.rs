//! TokenPress for JavaScript/TypeScript — **experimental**.
//!
//! Pipeline: parse the dialect implied by the file's extension (`parser`) →
//! whitespace-minimal re-render (`emit`) → verification (`verify`) → token
//! accounting.
//!
//! JS/TS support is not yet wired into the CLI, and JSX/TSX are deliberately
//! not accepted by [`JsFormatter::supports`] yet (see the note there), so
//! nothing here should be treated as a supported language backend.
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
        // `.d.ts` comes along via `.ts`. `.jsx`/`.tsx` are deliberately
        // absent even though `parser::parse` accepts them: the emitter is not
        // yet validated for JSX, so enabling those dialects end to end is a
        // later sub-task.
        path.extension().is_some_and(|e| {
            matches!(
                e.to_str(),
                Some("js" | "mjs" | "cjs" | "ts" | "mts" | "cts")
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
            // External tooling (`tsc --noEmit` / `node --check`) is not wired
            // up yet; both levels run the strongest built-in check.
            // `equivalent` re-parses the output itself, so no separate
            // `reparse` call is needed here.
            VerifyLevel::AstEquiv | VerifyLevel::External => {
                verify::equivalent(&program, path, &code)?;
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
        for name in ["a.js", "a.mjs", "a.cjs", "a.ts", "a.mts", "a.cts", "a.d.ts"] {
            assert!(f.supports(Path::new(name)), "{name} should be supported");
        }
        // `.jsx`/`.tsx` parse fine but are not enabled end to end yet.
        for name in ["a.py", "a.rs", "a.jsx", "a.tsx", "a.txt", "js", "a."] {
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
    fn external_level_currently_behaves_like_ast_equiv() {
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
    fn formatting_is_idempotent() {
        let sources = [
            (
                "a.js",
                "class P {\n    constructor( x ) {\n        this.x = x;\n    }\n}\n",
            ),
            ("a.js", "// note\nconst a = 1;\n"),
            ("a.ts", "enum Color {\n    Red = 1 ,\n    Green ,\n}\n"),
            ("a.mjs", "export const a = 1;\n"),
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
