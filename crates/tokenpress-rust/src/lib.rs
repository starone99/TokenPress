//! TokenPress for Rust: token-minimizing formatter.
//!
//! Pipeline: `syn` parse → optional doc-attribute stripping → token-stream
//! re-render with minimal whitespace (`emit`) → verification (re-parse +
//! canonical token comparison, `verify`) → token accounting.
//! Transform rules are documented in `docs/transforms/rust.md` (RS**/RSO**).
//!
//! Known MVP limits (see the transform reference): regular `//` comments are
//! dropped by the parser and whitespace inside macro bodies is minimized,
//! which can change the output of whitespace-sensitive macros (`stringify!`).

mod emit;
mod verify;

use std::path::Path;

use quote::ToTokens;
use tokenpress_core::{Error, FormatOptions, FormatResult, Formatter, Result, VerifyLevel};

/// Rust-specific choices. See `docs/transforms/rust.md` §2.
#[derive(Clone, Debug, Default)]
pub struct RustOptions {
    /// RSO1: drop `///`//`//!` (= `#[doc = "..."]`) attributes. Default keeps
    /// them, re-emitted in line-comment form when that round-trips exactly.
    pub strip_doc_comments: bool,
}

#[derive(Default)]
pub struct RustFormatter {
    options: RustOptions,
}

impl RustFormatter {
    pub fn new(options: RustOptions) -> Self {
        Self { options }
    }
}

impl Formatter for RustFormatter {
    fn language(&self) -> &'static str {
        "rust"
    }

    fn supports(&self, path: &Path) -> bool {
        path.extension().is_some_and(|e| e == "rs")
    }

    fn format(&self, source: &str, options: &FormatOptions) -> Result<FormatResult> {
        let mut file = syn::parse_file(source).map_err(|e| Error::Parse(e.to_string()))?;
        if self.options.strip_doc_comments {
            file = syn::parse2(emit::strip_doc_attrs(file.to_token_stream()))
                .expect("removing attributes preserves syntax");
        }
        let code = emit::render(&file);
        match options.verify {
            VerifyLevel::Reparse => {
                verify::reparse(&code)?;
            }
            // External (`rustc --emit=metadata`) is not wired up yet.
            VerifyLevel::AstEquiv | VerifyLevel::External => {
                verify::equivalent(&file, &code)?;
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

    fn fmt(source: &str) -> String {
        RustFormatter::default()
            .format(source, &FormatOptions::default())
            .unwrap()
            .code
    }

    #[test]
    fn rs01_rs02_minimize_whitespace_and_newlines() {
        let src = "fn add(a: i32, b: i32) -> i32 {\n    let sum = a + b;\n\n    sum\n}\n";
        assert_eq!(fmt(src), "fn add(a:i32,b:i32)->i32{let sum=a+b;sum}");
    }

    #[test]
    fn regular_comments_are_always_dropped() {
        assert_eq!(fmt("// gone\nfn f() {} // also gone\n"), "fn f(){}");
    }

    #[test]
    fn doc_comments_are_kept_by_default() {
        assert_eq!(fmt("/// Adds.\npub fn f() {}\n"), "/// Adds.\npub fn f(){}");
    }

    #[test]
    fn rso1_strips_doc_comments_on_request() {
        let formatter = RustFormatter::new(RustOptions {
            strip_doc_comments: true,
        });
        let r = formatter
            .format("/// Adds.\npub fn f() {}\n", &FormatOptions::default())
            .unwrap();
        assert_eq!(r.code, "pub fn f(){}");
    }

    #[test]
    fn structs_enums_and_impls_roundtrip() {
        let src = "#[derive(Debug, Clone)]\npub struct P { x: u8, y: u8 }\n\nimpl P {\n    pub fn sum(&self) -> u8 { self.x + self.y }\n}\n";
        assert_eq!(
            fmt(src),
            "#[derive(Debug,Clone)]pub struct P{x:u8,y:u8}impl P{pub fn sum(&self)->u8{self.x+self.y}}"
        );
    }

    #[test]
    fn parse_errors_are_reported() {
        let err = RustFormatter::default()
            .format("fn f( {", &FormatOptions::default())
            .unwrap_err();
        assert!(err.to_string().starts_with("parse error:"));
    }

    #[test]
    fn reparse_only_level_also_passes() {
        let opts = FormatOptions {
            verify: VerifyLevel::Reparse,
            ..FormatOptions::default()
        };
        let r = RustFormatter::default()
            .format("fn f() {}\n", &opts)
            .unwrap();
        assert_eq!(r.code, "fn f(){}");
    }

    #[test]
    fn external_level_currently_behaves_like_ast_equiv() {
        let opts = FormatOptions {
            verify: VerifyLevel::External,
            ..FormatOptions::default()
        };
        let r = RustFormatter::default()
            .format("fn f() {}\n", &opts)
            .unwrap();
        assert_eq!(r.code, "fn f(){}");
    }

    #[test]
    fn formatting_reduces_token_count() {
        let src = "fn add(a: i32, b: i32) -> i32 {\n    let sum = a + b;\n    sum\n}\n";
        let r = RustFormatter::default()
            .format(src, &FormatOptions::default())
            .unwrap();
        assert!(r.formatted_tokens < r.original_tokens);
    }

    #[test]
    fn formatting_is_idempotent() {
        let sources = [
            "fn f<'a>(x: &'a str) -> &'a str { x }\n",
            "/// Doc.\npub struct S { pub v: Vec<Vec<u8>> }\n",
            "fn main() { println!(\"{} x\", 1); }\n",
        ];
        for src in sources {
            let once = fmt(src);
            assert_eq!(fmt(&once), once, "not idempotent for {src:?}");
        }
    }

    #[test]
    fn supports_only_rs_extension() {
        let f = RustFormatter::default();
        assert!(f.supports(Path::new("a.rs")));
        assert!(!f.supports(Path::new("a.py")));
        assert_eq!(f.language(), "rust");
    }
}
