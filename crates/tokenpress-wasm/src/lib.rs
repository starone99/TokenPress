//! WebAssembly bindings for TokenPress.
//!
//! The Python and Rust formatters are exposed. Each `#[wasm_bindgen]` export
//! ([`format_python_json`], [`format_rust_json`]) is a JSON-in/JSON-out
//! delegation; every decision lives in the plain functions below, so the whole
//! crate is exercised — and covered — by ordinary host tests.
//!
//! The core invariant is preserved across the boundary: when a formatter
//! refuses output (parse failure, or output that fails verification) the
//! caller receives a structured `{"kind", "message"}` error and no code at
//! all. Partial or unverified output is never returned.
//!
//! Rust output carries the documented MVP caveats of `tokenpress-rust`
//! (non-doc `//` comments are dropped, macro-body whitespace is minimized).
//! The library reports no warnings, so neither does this boundary; callers
//! state the caveats themselves.

use std::path::Path;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use tokenpress_core::{Error, FormatOptions, FormatResult, Formatter, TokenizerKind};
use tokenpress_python::{PythonFormatter, PythonOptions};
use tokenpress_rust::{RustFormatter, RustOptions};
use wasm_bindgen::prelude::wasm_bindgen;

/// The tokenizers every successful result is priced against, in report order.
/// Both vocabularies are embedded in the binary, so neither needs I/O.
const REPORTED_TOKENIZERS: [(&str, TokenizerKind); 2] = [
    ("o200k_base", TokenizerKind::O200kBase),
    ("cl100k_base", TokenizerKind::Cl100kBase),
];

/// Python formatting flags accepted at the boundary.
///
/// Deserialized from a JSON object; every field is optional and falls back to
/// the library default (`merge_imports` on, all stripping off). Unknown fields
/// are rejected so a misspelled flag fails loudly instead of being ignored.
#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WasmPythonOptions {
    pub strip_comments: bool,
    pub strip_docstrings: bool,
    pub strip_annotations: bool,
    pub merge_imports: bool,
}

impl Default for WasmPythonOptions {
    fn default() -> Self {
        let PythonOptions {
            strip_comments,
            strip_docstrings,
            strip_annotations,
            merge_imports,
        } = PythonOptions::default();
        Self {
            strip_comments,
            strip_docstrings,
            strip_annotations,
            merge_imports,
        }
    }
}

impl From<&WasmPythonOptions> for PythonOptions {
    fn from(options: &WasmPythonOptions) -> Self {
        Self {
            strip_comments: options.strip_comments,
            strip_docstrings: options.strip_docstrings,
            strip_annotations: options.strip_annotations,
            merge_imports: options.merge_imports,
        }
    }
}

/// Rust formatting flags accepted at the boundary.
///
/// Deserialized like [`WasmPythonOptions`]: every field optional, unknown
/// fields rejected.
#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WasmRustOptions {
    pub strip_doc_comments: bool,
}

impl Default for WasmRustOptions {
    fn default() -> Self {
        let RustOptions { strip_doc_comments } = RustOptions::default();
        Self { strip_doc_comments }
    }
}

impl From<&WasmRustOptions> for RustOptions {
    fn from(options: &WasmRustOptions) -> Self {
        Self {
            strip_doc_comments: options.strip_doc_comments,
        }
    }
}

/// What one tokenizer charges for the input and for the formatted output.
#[derive(Clone, Debug, PartialEq)]
pub struct WasmTokenStats {
    /// Tokenizer name, e.g. `"o200k_base"`.
    pub tokenizer: &'static str,
    pub original: usize,
    pub formatted: usize,
    pub saved: usize,
    /// Fraction of the input's tokens saved, in `0.0..=1.0`; zero-token input
    /// saves 0.0.
    pub saving_ratio: f64,
}

impl WasmTokenStats {
    /// Prices one source/output pair with one tokenizer.
    ///
    /// The counting and the saved/ratio arithmetic both come from the core
    /// crate, so the boundary cannot drift from what the CLI reports.
    fn measure(
        tokenizer: &'static str,
        kind: &TokenizerKind,
        source: &str,
        code: &str,
    ) -> Result<Self, WasmError> {
        let counter = kind.load()?;
        // `code` is irrelevant to the accounting helpers, so it is not cloned
        // into this scratch result.
        let counted = FormatResult {
            code: String::new(),
            original_tokens: counter.count(source),
            formatted_tokens: counter.count(code),
        };
        Ok(Self {
            tokenizer,
            original: counted.original_tokens,
            formatted: counted.formatted_tokens,
            saved: counted.tokens_saved(),
            saving_ratio: counted.saving_ratio(),
        })
    }

    /// Renders as `{"formatted", "original", "saved", "saving_ratio"}`.
    fn to_value(&self) -> Value {
        json!({
            "original": self.original,
            "formatted": self.formatted,
            "saved": self.saved,
            "saving_ratio": self.saving_ratio,
        })
    }
}

/// A successful formatting run: verified output, whether it differs from the
/// input, and what it costs under each embedded tokenizer.
#[derive(Clone, Debug, PartialEq)]
pub struct WasmFormatOutput {
    pub code: String,
    pub changed: bool,
    pub tokens: Vec<WasmTokenStats>,
}

impl WasmFormatOutput {
    /// Renders as `{"changed": bool, "code": string, "tokens": {<tokenizer>:
    /// {"original", "formatted", "saved", "saving_ratio"}}}`.
    pub fn to_json(&self) -> String {
        let tokens: Map<String, Value> = self
            .tokens
            .iter()
            .map(|stats| (stats.tokenizer.to_string(), stats.to_value()))
            .collect();
        json!({ "code": self.code, "changed": self.changed, "tokens": tokens }).to_string()
    }
}

/// A structured failure: a machine-readable `kind` plus a human-readable
/// `message`. Never carries code.
#[derive(Clone, Debug, PartialEq)]
pub struct WasmError {
    pub kind: String,
    pub message: String,
}

impl WasmError {
    fn new(kind: &str, message: impl Into<String>) -> Self {
        Self {
            kind: kind.to_string(),
            message: message.into(),
        }
    }

    /// Renders as `{"kind": string, "message": string}`.
    pub fn to_json(&self) -> String {
        json!({ "kind": self.kind, "message": self.message }).to_string()
    }
}

impl From<Error> for WasmError {
    fn from(err: Error) -> Self {
        Self::new(error_kind(&err), err.to_string())
    }
}

/// Stable, machine-readable name for each core error.
fn error_kind(err: &Error) -> &'static str {
    match err {
        Error::Parse(_) => "parse",
        Error::Verification(_) => "verification",
        Error::UnsupportedLanguage(_) => "unsupported_language",
        Error::UnknownTokenizer(_) => "unknown_tokenizer",
        Error::Io(_) => "io",
    }
}

/// Parses the JSON options object.
fn parse_options<T: DeserializeOwned>(options_json: &str) -> Result<T, WasmError> {
    serde_json::from_str(options_json).map_err(|err| WasmError::new("options", err.to_string()))
}

/// Runs one formatter and prices the result against every reported tokenizer.
///
/// `path` is synthetic — there is no file system behind this boundary. It only
/// tells the formatter which dialect to apply, so each caller passes it right
/// where it picks the formatter and the two cannot disagree.
///
/// Returns [`WasmError`] — and no code — whenever the formatter refuses the
/// result, so unverified output cannot reach the caller.
fn run(
    formatter: &dyn Formatter,
    path: &Path,
    source: &str,
) -> Result<WasmFormatOutput, WasmError> {
    let result = formatter.format(path, source, &FormatOptions::default())?;
    let tokens = REPORTED_TOKENIZERS
        .iter()
        .map(|(name, kind)| WasmTokenStats::measure(name, kind, source, &result.code))
        .collect::<Result<Vec<_>, WasmError>>()?;
    Ok(WasmFormatOutput {
        changed: result.code != source,
        code: result.code,
        tokens,
    })
}

/// Formats Python source with the given flags.
pub fn format_python(
    source: &str,
    options: &WasmPythonOptions,
) -> Result<WasmFormatOutput, WasmError> {
    run(
        &PythonFormatter::new(options.into()),
        Path::new("input.py"),
        source,
    )
}

/// Formats Rust source with the given flags.
pub fn format_rust(source: &str, options: &WasmRustOptions) -> Result<WasmFormatOutput, WasmError> {
    run(
        &RustFormatter::new(options.into()),
        Path::new("input.rs"),
        source,
    )
}

/// Renders either outcome as the JSON the JavaScript side sees.
fn to_json_result(outcome: Result<WasmFormatOutput, WasmError>) -> Result<String, String> {
    match outcome {
        Ok(output) => Ok(output.to_json()),
        Err(err) => Err(err.to_json()),
    }
}

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
#[wasm_bindgen(js_name = formatPython)]
pub fn format_python_json(source: &str, options_json: &str) -> Result<String, String> {
    to_json_result(parse_options(options_json).and_then(|options| format_python(source, &options)))
}

/// Formats Rust source.
///
/// `options_json` is a JSON object with the optional boolean flag
/// `strip_doc_comments`; pass `"{}"` for the defaults. Resolves and rejects
/// exactly like [`format_python_json`].
#[wasm_bindgen(js_name = formatRust)]
pub fn format_rust_json(source: &str, options_json: &str) -> Result<String, String> {
    to_json_result(parse_options(options_json).and_then(|options| format_rust(source, &options)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokenpress_core::Error;

    fn format(source: &str, options: WasmPythonOptions) -> WasmFormatOutput {
        format_python(source, &options).expect("formatting succeeds")
    }

    fn format_rs(source: &str, options: WasmRustOptions) -> WasmFormatOutput {
        format_rust(source, &options).expect("formatting succeeds")
    }

    fn stats<'a>(output: &'a WasmFormatOutput, tokenizer: &str) -> &'a WasmTokenStats {
        output
            .tokens
            .iter()
            .find(|stats| stats.tokenizer == tokenizer)
            .expect("tokenizer is reported")
    }

    fn parsed(json: &str) -> serde_json::Value {
        serde_json::from_str(json).expect("the boundary emits valid JSON")
    }

    #[test]
    fn default_options_minimize_whitespace_and_merge_imports() {
        let out = format(
            "import os\nimport sys\n\nx = f(a, b)\n",
            WasmPythonOptions::default(),
        );
        assert_eq!(out.code, "import os,sys\nx=f(a,b)");
        assert!(out.changed);
    }

    #[test]
    fn already_minimal_source_is_reported_as_unchanged() {
        let out = format("x=1", WasmPythonOptions::default());
        assert_eq!(out.code, "x=1");
        assert!(!out.changed);
    }

    #[test]
    fn strip_docstrings_toggles_docstring_removal() {
        let source = "def f():\n    \"\"\"Doc.\"\"\"\n    return 1\n";
        assert_eq!(
            format(source, WasmPythonOptions::default()).code,
            "def f():\n \"\"\"Doc.\"\"\"\n return 1"
        );
        assert_eq!(
            format(
                source,
                WasmPythonOptions {
                    strip_docstrings: true,
                    ..WasmPythonOptions::default()
                }
            )
            .code,
            "def f():\n return 1"
        );
    }

    #[test]
    fn strip_comments_toggles_comment_removal() {
        let source = "# top\nx = 1\n";
        assert_eq!(
            format(source, WasmPythonOptions::default()).code,
            "# top\nx=1"
        );
        assert_eq!(
            format(
                source,
                WasmPythonOptions {
                    strip_comments: true,
                    ..WasmPythonOptions::default()
                }
            )
            .code,
            "x=1"
        );
    }

    #[test]
    fn strip_annotations_toggles_annotation_removal() {
        let source = "def f(x: int = 1) -> int:\n    return x\n";
        assert_eq!(
            format(source, WasmPythonOptions::default()).code,
            "def f(x:int=1)->int:\n return x"
        );
        assert_eq!(
            format(
                source,
                WasmPythonOptions {
                    strip_annotations: true,
                    ..WasmPythonOptions::default()
                }
            )
            .code,
            "def f(x=1):\n return x"
        );
    }

    #[test]
    fn invalid_python_returns_a_structured_error_and_no_output() {
        let err = format_python("def f(:\n", &WasmPythonOptions::default())
            .expect_err("invalid Python cannot be formatted");
        assert_eq!(err.kind, "parse");
        assert!(!err.message.is_empty());
        // The whole point: a refusal carries no code field at all, so no
        // unverified output can leak through the boundary.
        assert_eq!(
            err.to_json(),
            format!(
                "{{\"kind\":\"parse\",\"message\":{}}}",
                serde_json::Value::from(err.message.clone())
            )
        );
    }

    #[test]
    fn every_core_error_maps_to_a_kind() {
        let cases = [
            (Error::Parse("x".into()), "parse"),
            (Error::Verification("x".into()), "verification"),
            (
                Error::UnsupportedLanguage("x".into()),
                "unsupported_language",
            ),
            (Error::UnknownTokenizer("x".into()), "unknown_tokenizer"),
            (Error::Io(std::io::Error::other("x")), "io"),
        ];
        for (error, kind) in cases {
            let message = error.to_string();
            let wasm_error = WasmError::from(error);
            assert_eq!(wasm_error.kind, kind);
            assert_eq!(wasm_error.message, message);
        }
    }

    #[test]
    fn json_boundary_returns_code_and_changed_flag() {
        let json =
            format_python_json("import os\nimport sys\n", "{}").expect("formatting succeeds");
        let value = parsed(&json);
        assert_eq!(value["changed"], serde_json::json!(true));
        assert_eq!(value["code"], serde_json::json!("import os,sys"));
    }

    #[test]
    fn json_boundary_reads_option_flags() {
        let json = format_python_json(
            "def f():\n    \"\"\"Doc.\"\"\"\n    return 1\n",
            "{\"strip_docstrings\":true}",
        )
        .expect("formatting succeeds");
        let value = parsed(&json);
        assert_eq!(value["changed"], serde_json::json!(true));
        assert_eq!(value["code"], serde_json::json!("def f():\n return 1"));
    }

    #[test]
    fn json_boundary_reports_per_tokenizer_token_stats() {
        for json in [
            format_python_json("import os\nimport sys\n", "{}").expect("formatting succeeds"),
            format_rust_json("fn f() {\n    let x = 1;\n}\n", "{}").expect("formatting succeeds"),
        ] {
            let value = parsed(&json);
            for name in ["o200k_base", "cl100k_base"] {
                let entry = &value["tokens"][name];
                let original = entry["original"].as_u64().expect("original count");
                let formatted = entry["formatted"].as_u64().expect("formatted count");
                let saved = entry["saved"].as_u64().expect("saved count");
                let ratio = entry["saving_ratio"].as_f64().expect("saving ratio");
                assert!(formatted < original, "{json}");
                assert_eq!(saved, original - formatted, "{json}");
                assert!(ratio > 0.0 && ratio < 1.0, "{json}");
            }
        }
    }

    #[test]
    fn json_boundary_reports_failures_as_structured_json() {
        let err = format_python_json("def f(:\n", "{}").expect_err("invalid Python is refused");
        assert!(
            err.starts_with("{\"kind\":\"parse\",\"message\":\""),
            "{err}"
        );
    }

    #[test]
    fn json_boundary_rejects_malformed_option_objects() {
        for options in ["", "{\"nope\":true}", "{\"merge_imports\":\"yes\"}"] {
            let err = format_python_json("x=1", options).expect_err("bad options are refused");
            assert!(
                err.starts_with("{\"kind\":\"options\",\"message\":\""),
                "{err}"
            );
        }
    }

    #[test]
    fn boundary_types_are_cloneable_and_debuggable() {
        let options = WasmPythonOptions::default();
        assert!(format!("{:?}", options.clone()).contains("merge_imports"));
        let rust_options = WasmRustOptions::default();
        assert!(format!("{:?}", rust_options.clone()).contains("strip_doc_comments"));
        let output = WasmFormatOutput {
            code: "x=1".into(),
            changed: true,
            tokens: vec![WasmTokenStats {
                tokenizer: "o200k_base",
                original: 4,
                formatted: 3,
                saved: 1,
                saving_ratio: 0.25,
            }],
        };
        assert_eq!(output.clone(), output);
        assert!(format!("{output:?}").contains("changed"));
        assert!(format!("{:?}", output.tokens[0].clone()).contains("saving_ratio"));
        let error = WasmError {
            kind: "parse".into(),
            message: "bad".into(),
        };
        assert_eq!(error.clone(), error);
        assert!(format!("{error:?}").contains("parse"));
    }

    #[test]
    fn rust_default_options_minimize_whitespace_and_keep_doc_comments() {
        let out = format_rs(
            "/// Adds.\npub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",
            WasmRustOptions::default(),
        );
        assert_eq!(out.code, "/// Adds.\npub fn add(a:i32,b:i32)->i32{a+b}");
        assert!(out.changed);
    }

    #[test]
    fn rust_already_minimal_source_is_reported_as_unchanged() {
        let out = format_rs("fn f(){}", WasmRustOptions::default());
        assert_eq!(out.code, "fn f(){}");
        assert!(!out.changed);
    }

    #[test]
    fn rust_strip_doc_comments_toggles_doc_removal() {
        let source = "/// Adds.\npub fn f() {}\n";
        assert_eq!(
            format_rs(source, WasmRustOptions::default()).code,
            "/// Adds.\npub fn f(){}"
        );
        assert_eq!(
            format_rs(
                source,
                WasmRustOptions {
                    strip_doc_comments: true,
                }
            )
            .code,
            "pub fn f(){}"
        );
    }

    #[test]
    fn invalid_rust_returns_a_structured_error_and_no_output() {
        let err = format_rust("fn f( {", &WasmRustOptions::default())
            .expect_err("invalid Rust cannot be formatted");
        assert_eq!(err.kind, "parse");
        assert!(!err.message.is_empty());
        // A refusal carries no code and no token stats — nothing unverified
        // crosses the boundary.
        assert_eq!(
            err.to_json(),
            format!(
                "{{\"kind\":\"parse\",\"message\":{}}}",
                serde_json::Value::from(err.message.clone())
            )
        );
    }

    #[test]
    fn json_boundary_formats_rust() {
        let json = format_rust_json("pub fn f() {\n    let x = 1;\n}\n", "{}")
            .expect("formatting succeeds");
        let value = parsed(&json);
        assert_eq!(value["changed"], serde_json::json!(true));
        assert_eq!(value["code"], serde_json::json!("pub fn f(){let x=1;}"));
    }

    #[test]
    fn json_boundary_reads_rust_option_flags() {
        let json = format_rust_json(
            "/// Adds.\npub fn f() {}\n",
            "{\"strip_doc_comments\":true}",
        )
        .expect("formatting succeeds");
        assert_eq!(parsed(&json)["code"], serde_json::json!("pub fn f(){}"));
    }

    #[test]
    fn json_boundary_reports_rust_failures_as_structured_json() {
        let err = format_rust_json("fn f( {", "{}").expect_err("invalid Rust is refused");
        assert!(
            err.starts_with("{\"kind\":\"parse\",\"message\":\""),
            "{err}"
        );
    }

    #[test]
    fn json_boundary_rejects_malformed_rust_option_objects() {
        for options in ["", "{\"nope\":true}", "{\"strip_doc_comments\":\"yes\"}"] {
            let err = format_rust_json("fn f(){}", options).expect_err("bad options are refused");
            assert!(
                err.starts_with("{\"kind\":\"options\",\"message\":\""),
                "{err}"
            );
        }
    }

    #[test]
    fn both_embedded_tokenizers_are_reported_in_a_stable_order() {
        for out in [
            format("x = f(a, b)\n", WasmPythonOptions::default()),
            format_rs("fn f() { g(a, b); }\n", WasmRustOptions::default()),
        ] {
            let names: Vec<_> = out.tokens.iter().map(|stats| stats.tokenizer).collect();
            assert_eq!(names, ["o200k_base", "cl100k_base"]);
        }
    }

    #[test]
    fn token_counts_match_the_tokenizer_api() {
        let python = "import os\nimport sys\n\nx = f(a, b)\n";
        let rust = "fn add(a: i32, b: i32) -> i32 {\n    let sum = a + b;\n    sum\n}\n";
        for (source, out) in [
            (python, format(python, WasmPythonOptions::default())),
            (rust, format_rs(rust, WasmRustOptions::default())),
        ] {
            for (name, kind) in [
                ("o200k_base", TokenizerKind::O200kBase),
                ("cl100k_base", TokenizerKind::Cl100kBase),
            ] {
                let tokenizer = kind.load().expect("embedded tokenizer loads");
                let stats = stats(&out, name);
                assert_eq!(stats.original, tokenizer.count(source));
                assert_eq!(stats.formatted, tokenizer.count(&out.code));
                assert_eq!(stats.saved, stats.original - stats.formatted);
                let expected = stats.saved as f64 / stats.original as f64;
                assert!((stats.saving_ratio - expected).abs() < f64::EPSILON);
                assert!(stats.formatted < stats.original);
            }
        }
    }

    #[test]
    fn zero_token_input_reports_a_zero_saving_ratio() {
        for out in [
            format("", WasmPythonOptions::default()),
            format_rs("", WasmRustOptions::default()),
        ] {
            assert_eq!(out.code, "");
            assert!(!out.changed);
            assert_eq!(out.tokens.len(), 2);
            for stats in &out.tokens {
                assert_eq!(
                    (stats.original, stats.formatted, stats.saved),
                    (0, 0, 0),
                    "{:?}",
                    stats.tokenizer
                );
                assert_eq!(stats.saving_ratio, 0.0);
            }
        }
    }

    #[test]
    fn merge_imports_can_be_turned_off() {
        let source = "import os\nimport sys\n";
        assert_eq!(
            format(source, WasmPythonOptions::default()).code,
            "import os,sys"
        );
        assert_eq!(
            format(
                source,
                WasmPythonOptions {
                    merge_imports: false,
                    ..WasmPythonOptions::default()
                }
            )
            .code,
            "import os\nimport sys"
        );
    }
}
