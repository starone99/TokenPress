//! WebAssembly bindings for TokenPress.
//!
//! Only the Python formatter is exposed. The single `#[wasm_bindgen]` export
//! ([`format_python_json`]) is a JSON-in/JSON-out delegation; every decision
//! lives in the plain functions below, so the whole crate is exercised — and
//! covered — by ordinary host tests.
//!
//! The core invariant is preserved across the boundary: when the formatter
//! refuses output (parse failure, or output that fails verification) the
//! caller receives a structured `{"kind", "message"}` error and no code at
//! all. Partial or unverified output is never returned.

use serde::Deserialize;
use serde_json::json;
use tokenpress_core::{Error, FormatOptions, Formatter};
use tokenpress_python::{PythonFormatter, PythonOptions};
use wasm_bindgen::prelude::wasm_bindgen;

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

/// A successful formatting run: verified output plus whether it differs from
/// the input.
#[derive(Clone, Debug, PartialEq)]
pub struct WasmFormatOutput {
    pub code: String,
    pub changed: bool,
}

impl WasmFormatOutput {
    /// Renders as `{"changed": bool, "code": string}`.
    pub fn to_json(&self) -> String {
        json!({ "code": self.code, "changed": self.changed }).to_string()
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
fn parse_options(options_json: &str) -> Result<WasmPythonOptions, WasmError> {
    serde_json::from_str(options_json).map_err(|err| WasmError::new("options", err.to_string()))
}

/// Formats Python source with the given flags.
///
/// Returns [`WasmError`] — and no code — whenever the formatter refuses the
/// result, so unverified output cannot reach the caller.
pub fn format_python(
    source: &str,
    options: &WasmPythonOptions,
) -> Result<WasmFormatOutput, WasmError> {
    let result = PythonFormatter::new(options.into()).format(source, &FormatOptions::default())?;
    Ok(WasmFormatOutput {
        changed: result.code != source,
        code: result.code,
    })
}

/// Formats Python source. This is the only export reachable from JavaScript.
///
/// `options_json` is a JSON object with the optional boolean flags
/// `strip_comments`, `strip_docstrings`, `strip_annotations` and
/// `merge_imports`; pass `"{}"` for the defaults.
///
/// Resolves to `{"changed": bool, "code": string}`, or rejects with
/// `{"kind": string, "message": string}` where `kind` is one of `options`,
/// `parse`, `verification`, `unsupported_language`, `unknown_tokenizer` or
/// `io`.
#[wasm_bindgen(js_name = formatPython)]
pub fn format_python_json(source: &str, options_json: &str) -> Result<String, String> {
    match parse_options(options_json).and_then(|options| format_python(source, &options)) {
        Ok(output) => Ok(output.to_json()),
        Err(err) => Err(err.to_json()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokenpress_core::Error;

    fn format(source: &str, options: WasmPythonOptions) -> WasmFormatOutput {
        format_python(source, &options).expect("formatting succeeds")
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
        assert_eq!(json, "{\"changed\":true,\"code\":\"import os,sys\"}");
    }

    #[test]
    fn json_boundary_reads_option_flags() {
        let json = format_python_json(
            "def f():\n    \"\"\"Doc.\"\"\"\n    return 1\n",
            "{\"strip_docstrings\":true}",
        )
        .expect("formatting succeeds");
        assert_eq!(json, "{\"changed\":true,\"code\":\"def f():\\n return 1\"}");
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
        let output = WasmFormatOutput {
            code: "x=1".into(),
            changed: true,
        };
        assert_eq!(output.clone(), output);
        assert!(format!("{output:?}").contains("changed"));
        let error = WasmError {
            kind: "parse".into(),
            message: "bad".into(),
        };
        assert_eq!(error.clone(), error);
        assert!(format!("{error:?}").contains("parse"));
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
