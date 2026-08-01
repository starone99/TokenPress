//! TokenPress for Python: token-minimizing formatter.
//!
//! Pipeline: lex/parse → transform passes (import merging, optional docstring
//! and annotation stripping) → token-stream re-render with minimal whitespace
//! (`emit`) → verification (`verify`) → token accounting. Transform rules are
//! documented in `docs/transforms/python.md` (PY**/PYO** rule IDs).

mod emit;
mod parser;
mod passes;
mod verify;

use std::path::Path;

use tokenpress_core::{FormatOptions, FormatResult, Formatter, Result, VerifyLevel};

/// Python-specific choices. See `docs/transforms/python.md` §2.
#[derive(Clone, Debug)]
pub struct PythonOptions {
    /// PYO1: drop `#` comments. Default keeps them — comments are context
    /// for LLMs; stripping is the opt-in.
    pub strip_comments: bool,
    /// PYO2: drop docstrings (opt-in). This empties `__doc__`, so `help()`,
    /// `doctest` and docstring-driven tooling lose their input.
    pub strip_docstrings: bool,
    /// PYO3: drop type annotations (opt-in). This changes `__annotations__`
    /// and breaks dataclass/pydantic/FastAPI-style runtime introspection.
    pub strip_annotations: bool,
    /// PY09: merge adjacent import statements (default on — adjacency keeps
    /// side-effect order, so behavior is preserved).
    pub merge_imports: bool,
}

impl Default for PythonOptions {
    fn default() -> Self {
        Self {
            strip_comments: false,
            strip_docstrings: false,
            strip_annotations: false,
            merge_imports: true,
        }
    }
}

pub struct PythonFormatter {
    options: PythonOptions,
}

impl PythonFormatter {
    pub fn new(options: PythonOptions) -> Self {
        Self { options }
    }
}

impl Default for PythonFormatter {
    fn default() -> Self {
        Self::new(PythonOptions::default())
    }
}

impl Formatter for PythonFormatter {
    fn language(&self) -> &'static str {
        "python"
    }

    fn supports(&self, path: &Path) -> bool {
        path.extension().is_some_and(|e| e == "py")
    }

    fn format(&self, source: &str, options: &FormatOptions) -> Result<FormatResult> {
        let parsed = parser::parse(source)?;
        let mut tokens = parsed.tokens(source);
        let mut modified = false;
        // Docstrings first: removing a whole statement can empty a body, which
        // the annotation pass then sees as already-`pass`-terminated.
        if self.options.strip_docstrings {
            let (stripped, m) = passes::strip_docstrings(&tokens, parsed.ast());
            tokens = stripped;
            modified |= m;
        }
        if self.options.strip_annotations {
            let (stripped, m) = passes::strip_annotations(&tokens, parsed.ast());
            tokens = stripped;
            modified |= m;
        }
        if self.options.merge_imports {
            let (merged, m) = passes::merge_imports(&tokens);
            tokens = merged;
            modified |= m;
        }
        let code = emit::render(&tokens, source, &self.options);
        match options.verify {
            VerifyLevel::Reparse => {
                verify::reparse(&code)?;
            }
            // External tooling (py_compile) is not wired up yet; both levels
            // run the strongest built-in check.
            VerifyLevel::AstEquiv | VerifyLevel::External => {
                verify::full(&parsed, &tokens, &code, &self.options, modified)?;
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

    fn fmt(source: &str) -> String {
        PythonFormatter::default()
            .format(source, &FormatOptions::default())
            .unwrap()
            .code
    }

    fn fmt_with(source: &str, options: PythonOptions) -> String {
        PythonFormatter::new(options)
            .format(source, &FormatOptions::default())
            .unwrap()
            .code
    }

    #[test]
    fn py01_removes_spaces_around_operators() {
        assert_eq!(fmt("x = f(a, b) + 1\n"), "x=f(a,b)+1");
    }

    #[test]
    fn py02_indents_one_char_per_level() {
        assert_eq!(
            fmt("def f(x):\n    if x:\n        return 1\n"),
            "def f(x):\n if x:\n  return 1"
        );
    }

    #[test]
    fn py03_drops_blank_lines() {
        assert_eq!(fmt("a = 1\n\n\nb = 2\n"), "a=1\nb=2");
    }

    #[test]
    fn py03_joins_bracketed_continuation_lines() {
        assert_eq!(fmt("f(\n    1,\n    2,\n)\n"), "f(1,2,)");
    }

    #[test]
    fn py08_backslash_continuation_is_joined() {
        assert_eq!(fmt("x = 1 + \\\n    2\n"), "x=1+2");
    }

    #[test]
    fn py09_merges_adjacent_imports_by_default() {
        assert_eq!(
            fmt("import os\nimport sys\n\nfrom a import b\nfrom a import c\nx = 1\n"),
            "import os,sys\nfrom a import b,c\nx=1"
        );
    }

    #[test]
    fn py09_can_be_disabled() {
        let out = fmt_with(
            "import os\nimport sys\n",
            PythonOptions {
                merge_imports: false,
                ..PythonOptions::default()
            },
        );
        assert_eq!(out, "import os\nimport sys");
    }

    #[test]
    fn pyo1_keeps_comments_by_default() {
        assert_eq!(
            fmt("# top\nx = 1  # trailing\ny = 2\n"),
            "# top\nx=1 # trailing\ny=2"
        );
    }

    #[test]
    fn pyo1_strips_comments_on_request() {
        let out = fmt_with(
            "# top\nx = 1  # trailing\n",
            PythonOptions {
                strip_comments: true,
                ..PythonOptions::default()
            },
        );
        assert_eq!(out, "x=1");
    }

    #[test]
    fn keeps_indented_comment_at_its_block_level() {
        assert_eq!(
            fmt("def f():\n    # doc\n    return 1\n"),
            "def f():\n # doc\n return 1"
        );
    }

    #[test]
    fn pyo3_strips_annotations_on_request() {
        let strip = PythonOptions {
            strip_annotations: true,
            ..PythonOptions::default()
        };
        assert_eq!(
            fmt_with("def f(x: int = 1) -> int:\n    return x\n", strip.clone()),
            "def f(x=1):\n return x"
        );
        assert_eq!(fmt_with("total: int = 0\n", strip.clone()), "total=0");
        assert_eq!(fmt_with("class C:\n    x: int\n", strip), "class C:\n pass");
    }

    #[test]
    fn pyo2_strips_docstrings_on_request() {
        let strip = PythonOptions {
            strip_docstrings: true,
            ..PythonOptions::default()
        };
        assert_eq!(
            fmt_with(
                "\"\"\"Module doc.\"\"\"\ndef f():\n    \"\"\"Doc.\"\"\"\n    return 1\n",
                strip.clone()
            ),
            "def f():\n return 1"
        );
        assert_eq!(
            fmt_with("class C:\n    \"\"\"Doc.\"\"\"\n", strip),
            "class C:\n pass"
        );
    }

    #[test]
    fn docstrings_with_trailing_comments_still_verify() {
        // Regression: the surviving comment starts the body, which changes
        // where the lexer puts the block's Indent. Verification refuses the
        // output unless the pass produces the same order.
        let strip = PythonOptions {
            strip_docstrings: true,
            ..PythonOptions::default()
        };
        assert_eq!(
            fmt_with(
                "def f():  # header\n    \"\"\"Doc.\"\"\"  # note\n    return 1\n",
                strip.clone()
            ),
            "def f(): # header\n # note\n return 1"
        );
        assert_eq!(
            fmt_with(
                "class C:\n    \"\"\"Doc.\"\"\"  # note\n    x = 1\n",
                strip.clone()
            ),
            "class C:\n # note\n x=1"
        );
        // Comment lines *after* the docstring start the body just the same.
        assert_eq!(
            fmt_with(
                "class C:\n    \"\"\"Doc.\"\"\"\n\n    # a note\n    # and another\n    x = 1\n",
                strip
            ),
            "class C:\n # a note\n # and another\n x=1"
        );
    }

    #[test]
    fn docstring_stripped_output_passes_verification() {
        // Removing docstrings changes the AST, so the pass must be reconciled
        // with verification rather than tripping it.
        let src = "\"\"\"Module doc.\"\"\"\nimport os\nclass C:\n    \"\"\"Doc.\"\"\"\n    def m(self):\n        \"\"\"Method.\"\"\"\n        return os\n";
        for verify in [VerifyLevel::AstEquiv, VerifyLevel::External] {
            let options = FormatOptions {
                verify,
                ..FormatOptions::default()
            };
            let result = PythonFormatter::new(PythonOptions {
                strip_docstrings: true,
                ..PythonOptions::default()
            })
            .format(src, &options);
            assert_eq!(
                result.unwrap().code,
                "import os\nclass C:\n def m(self):\n  return os"
            );
        }
    }

    #[test]
    fn annotations_are_kept_by_default() {
        assert_eq!(
            fmt("def f(x: int = 1) -> int:\n    return x\n"),
            "def f(x:int=1)->int:\n return x"
        );
        assert_eq!(fmt("if (n := 10) > 5:\n    pass\n"), "if(n:=10)>5:\n pass");
    }

    #[test]
    fn keyword_boundaries_keep_a_separating_space() {
        assert_eq!(
            fmt("import os\nreturn_value = not True\n"),
            "import os\nreturn_value=not True"
        );
        assert_eq!(fmt("x = 1 if flag else 2\n"), "x=1 if flag else 2");
    }

    #[test]
    fn fstring_interiors_are_verbatim() {
        // Interpolation whitespace can be semantic (`f"{x = }"` echoes it
        // into the output string), so f-strings are never minimized inside.
        assert_eq!(fmt("y = f\"a{x + 1}b\"\n"), "y=f\"a{x + 1}b\"");
        assert_eq!(fmt("msg = f\"{value = }\"\n"), "msg=f\"{value = }\"");
        assert_eq!(
            fmt("z = f\"outer {f'inner {a + b}'} end\"\n"),
            "z=f\"outer {f'inner {a + b}'} end\""
        );
    }

    #[test]
    fn triple_quoted_strings_keep_their_newlines() {
        let src = "s = \"\"\"line1\nline2\"\"\"\n";
        assert_eq!(fmt(src), "s=\"\"\"line1\nline2\"\"\"");
    }

    #[test]
    fn docstrings_are_kept() {
        assert_eq!(
            fmt("def f():\n    \"\"\"Doc.\"\"\"\n    return 1\n"),
            "def f():\n \"\"\"Doc.\"\"\"\n return 1"
        );
    }

    #[test]
    fn empty_and_comment_only_sources() {
        assert_eq!(fmt(""), "");
        assert_eq!(fmt("# only a comment\n"), "# only a comment");
    }

    #[test]
    fn crlf_input_is_normalized() {
        assert_eq!(fmt("a = 1\r\nb = 2\r\n"), "a=1\nb=2");
    }

    #[test]
    fn parse_errors_are_reported() {
        let err = PythonFormatter::default()
            .format("def f(:\n", &FormatOptions::default())
            .unwrap_err();
        assert!(err.to_string().starts_with("parse error:"));
    }

    #[test]
    fn reparse_only_level_also_passes() {
        let opts = FormatOptions {
            verify: VerifyLevel::Reparse,
            ..FormatOptions::default()
        };
        let r = PythonFormatter::default().format("x = 1\n", &opts).unwrap();
        assert_eq!(r.code, "x=1");
    }

    #[test]
    fn external_level_currently_behaves_like_ast_equiv() {
        let opts = FormatOptions {
            verify: VerifyLevel::External,
            ..FormatOptions::default()
        };
        let r = PythonFormatter::default().format("x = 1\n", &opts).unwrap();
        assert_eq!(r.code, "x=1");
    }

    #[test]
    fn formatting_reduces_token_count() {
        let src = "def add(a, b):\n    result = a + b\n    return result\n";
        let r = PythonFormatter::default()
            .format(src, &FormatOptions::default())
            .unwrap();
        assert!(r.formatted_tokens < r.original_tokens);
        assert!(r.tokens_saved() > 0);
    }

    #[test]
    fn formatting_is_idempotent() {
        let sources = [
            "def f(x):\n    if x:\n        return 1\n    return 0\n",
            "class A:\n    \"\"\"Doc.\"\"\"\n\n    def m(self):\n        return [\n            1,\n            2,\n        ]\n",
            "x = 1 if flag else 2\n",
            "import os\nimport sys\nfrom a import b\nfrom a import c\n",
            "# comment stays\nx = 1  # here too\n",
        ];
        for src in sources {
            let once = fmt(src);
            assert_eq!(fmt(&once), once, "not idempotent for {src:?}");
        }
    }

    #[test]
    fn stripping_passes_are_idempotent_too() {
        let strip = PythonOptions {
            strip_comments: true,
            strip_docstrings: false,
            strip_annotations: true,
            merge_imports: true,
        };
        let src = "import os\nimport re\n# gone\ndef f(x: int) -> int:\n    return x\n";
        let once = fmt_with(src, strip.clone());
        assert_eq!(fmt_with(&once, strip.clone()), once);
        assert_eq!(once, "import os,re\ndef f(x):\n return x");
    }

    #[test]
    fn all_stripping_passes_together_are_idempotent() {
        let strip = PythonOptions {
            strip_comments: true,
            strip_docstrings: true,
            strip_annotations: true,
            merge_imports: true,
        };
        let src = "\"\"\"Module doc.\"\"\"\nimport os\nimport re\n# gone\ndef f(x: int) -> int:\n    \"\"\"Doc.\"\"\"\n    return x\nclass C:\n    \"\"\"Doc.\"\"\"\n";
        let once = fmt_with(src, strip.clone());
        assert_eq!(once, "import os,re\ndef f(x):\n return x\nclass C:\n pass");
        assert_eq!(fmt_with(&once, strip.clone()), once);
    }

    #[test]
    fn supports_only_py_extension() {
        let f = PythonFormatter::default();
        assert!(f.supports(Path::new("a.py")));
        assert!(!f.supports(Path::new("a.rs")));
        assert!(!f.supports(Path::new("py")));
        assert_eq!(f.language(), "python");
    }

    #[test]
    fn tokenizer_choice_is_respected() {
        let opts = FormatOptions {
            tokenizer: TokenizerKind::Cl100kBase,
            ..FormatOptions::default()
        };
        let r = PythonFormatter::default().format("x = 1\n", &opts).unwrap();
        assert!(r.original_tokens > 0);
    }
}
