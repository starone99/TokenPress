//! Source-level transform passes that run on the token stream before the
//! minimal-whitespace render:
//!
//! - PY09 `merge_imports` (default on): joins *adjacent* `import` /
//!   `from m import` statements. Adjacency keeps module-init side-effect
//!   order identical, so this is always behavior-preserving.
//! - PYO3 `strip_annotations` (opt-in): removes type annotations. This *does*
//!   change `__annotations__` and breaks dataclass/pydantic-style runtime
//!   introspection, which is why it is not a default.
//!
//! Passes return `(tokens, modified)`; when a pass modified the stream, the
//! verifier compares the output against these tokens instead of the original
//! AST (plus an import-preservation check, see `verify`).

use crate::parser::{ast, AstRanged, Module, TextRange, Tok, TokenKind};

/// PY09 - merge adjacent import statements.
///
/// Statements merge only when nothing but blank lines separates them, both
/// are the same flavor (`import` / `from <same module> import`), and neither
/// uses parentheses, `*`, or `;`. Comments and any other statement break the
/// run, so kept comments never move relative to their imports.
pub fn merge_imports<'a>(tokens: &[Tok<'a>]) -> (Vec<Tok<'a>>, bool) {
    #[derive(PartialEq)]
    enum Kind {
        Plain,
        From(String),
    }
    struct Import {
        kind: Kind,
        /// Number of leading tokens forming the `import` / `from m import`
        /// prefix that a merged continuation drops.
        prefix: usize,
    }

    // Classifies the statement starting at `start`; the returned end index
    // points at its terminating Newline.
    fn scan(tokens: &[Tok<'_>], start: usize) -> Option<(Import, usize)> {
        let mut import_kw = None;
        let mut end = None;
        for (offset, tok) in tokens[start..].iter().enumerate() {
            match tok.kind {
                TokenKind::Newline => {
                    end = Some(start + offset);
                    break;
                }
                // A trailing comment would swallow everything appended after
                // it, so a commented import never merges.
                TokenKind::Lpar
                | TokenKind::Star
                | TokenKind::Semi
                | TokenKind::Comment
                | TokenKind::EndOfFile => {
                    return None;
                }
                TokenKind::Import if offset > 0 => import_kw = Some(start + offset),
                _ => {}
            }
        }
        let end = end?;
        let import = if tokens[start].kind == TokenKind::Import {
            Import {
                kind: Kind::Plain,
                prefix: 1,
            }
        } else {
            let import_kw = import_kw?;
            let module: String = tokens[start + 1..import_kw]
                .iter()
                .filter(|t| t.kind != TokenKind::NonLogicalNewline)
                .map(|t| t.text)
                .collect();
            Import {
                kind: Kind::From(module),
                prefix: import_kw - start + 1,
            }
        };
        Some((import, end))
    }

    let mut out: Vec<Tok<'a>> = Vec::new();
    let mut modified = false;
    // The withheld Newline of the previous import statement, still open for
    // merging with the next one.
    let mut pending: Option<(Kind, Tok<'a>)> = None;
    let mut at_line_start = true;
    let mut i = 0;
    while i < tokens.len() {
        let tok = &tokens[i];
        // Blank lines are transparent for merging (the renderer drops them),
        // and the token after one is a line start - `import`/`from` are
        // keywords, so a false positive inside brackets is impossible.
        if tok.kind == TokenKind::NonLogicalNewline {
            out.push(tok.clone());
            at_line_start = true;
            i += 1;
            continue;
        }
        let import = if at_line_start && matches!(tok.kind, TokenKind::Import | TokenKind::From) {
            scan(tokens, i)
        } else {
            None
        };
        match import {
            Some((stmt, end)) => {
                match pending.take() {
                    Some((kind, _newline)) if kind == stmt.kind => {
                        // Continue the previous statement: `, names...`
                        modified = true;
                        out.push(Tok {
                            kind: TokenKind::Comma,
                            text: ",",
                            range: tok.range,
                        });
                    }
                    Some((_, newline)) => {
                        out.push(newline);
                        out.extend(tokens[i..i + stmt.prefix].iter().cloned());
                    }
                    None => out.extend(tokens[i..i + stmt.prefix].iter().cloned()),
                }
                out.extend(tokens[i + stmt.prefix..end].iter().cloned());
                pending = Some((stmt.kind, tokens[end].clone()));
                at_line_start = true;
                i = end + 1;
            }
            None => {
                if let Some((_, newline)) = pending.take() {
                    out.push(newline);
                }
                at_line_start = matches!(
                    tok.kind,
                    TokenKind::Newline | TokenKind::Indent | TokenKind::Dedent
                );
                out.push(tok.clone());
                i += 1;
            }
        }
    }
    if let Some((_, newline)) = pending {
        out.push(newline);
    }
    (out, modified)
}

/// Spans of annotation syntax collected from the AST.
#[derive(Default)]
struct AnnotationSpans {
    /// `: <expr>` (parameters, annotated assignments with a value) - deleted.
    deletes: Vec<TextRange>,
    /// Return annotations - deleted together with the preceding `->`.
    returns: Vec<TextRange>,
    /// Bare `target: ann` statements - replaced by `pass`.
    replaced_stmts: Vec<TextRange>,
}

impl ast::visitor::Visitor<'_> for AnnotationSpans {
    fn visit_stmt(&mut self, stmt: &ast::Stmt) {
        match stmt {
            ast::Stmt::AnnAssign(a) => {
                if a.value.is_some() {
                    self.deletes
                        .push(TextRange::new(a.target.end(), a.annotation.end()));
                } else {
                    self.replaced_stmts.push(a.range());
                }
            }
            ast::Stmt::FunctionDef(f) => {
                for p in f.parameters.iter() {
                    if let Some(annotation) = p.annotation() {
                        self.deletes
                            .push(TextRange::new(p.name().end(), annotation.end()));
                    }
                }
                if let Some(returns) = &f.returns {
                    self.returns.push(returns.range());
                }
            }
            _ => {}
        }
        ast::visitor::walk_stmt(self, stmt);
    }
}

/// PYO3 - strip type annotations, using AST spans to cut tokens.
pub fn strip_annotations<'a>(tokens: &[Tok<'a>], module: &Module) -> (Vec<Tok<'a>>, bool) {
    use ast::visitor::Visitor;
    let mut spans = AnnotationSpans::default();
    for stmt in &module.body {
        spans.visit_stmt(stmt);
    }

    // Extend each return-annotation span backwards over the `->` token.
    let mut deletes = spans.deletes;
    for ret in spans.returns {
        let first = tokens
            .iter()
            .position(|t| t.range.start() >= ret.start() && t.kind != TokenKind::NonLogicalNewline)
            .expect("return annotation has tokens");
        if first > 0 && tokens[first - 1].kind == TokenKind::Rarrow {
            deletes.push(TextRange::new(tokens[first - 1].range.start(), ret.end()));
        }
    }

    let modified = !(deletes.is_empty() && spans.replaced_stmts.is_empty());
    if !modified {
        return (tokens.to_vec(), false);
    }
    let inside = |t: &Tok<'_>, spans: &[TextRange]| {
        spans
            .iter()
            .any(|s| t.range.start() >= s.start() && t.range.end() <= s.end())
    };
    let mut out: Vec<Tok<'a>> = Vec::new();
    let mut replaced_span: Option<TextRange> = None;
    for tok in tokens {
        let replacement = spans
            .replaced_stmts
            .iter()
            .find(|s| tok.range.start() >= s.start() && tok.range.end() <= s.end())
            .copied();
        if let Some(span) = replacement {
            // The first token of each bare `x: ann` statement becomes `pass`.
            if replaced_span != Some(span) {
                replaced_span = Some(span);
                out.push(Tok {
                    kind: TokenKind::Pass,
                    text: "pass",
                    range: tok.range,
                });
            }
        } else if !inside(tok, &deletes) {
            out.push(tok.clone());
        }
    }
    (out, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    fn merge(source: &str) -> (String, bool) {
        let parsed = parser::parse(source).unwrap();
        let (tokens, modified) = merge_imports(&parsed.tokens(source));
        (
            crate::emit::render(&tokens, source, &crate::PythonOptions::default()),
            modified,
        )
    }

    fn strip(source: &str) -> (String, bool) {
        let parsed = parser::parse(source).unwrap();
        let (tokens, modified) = strip_annotations(&parsed.tokens(source), parsed.ast());
        (
            crate::emit::render(&tokens, source, &crate::PythonOptions::default()),
            modified,
        )
    }

    #[test]
    fn adjacent_plain_imports_merge() {
        let (code, modified) = merge("import os\nimport sys\nimport re\n");
        assert_eq!(code, "import os,sys,re");
        assert!(modified);
    }

    #[test]
    fn blank_lines_do_not_stop_a_merge() {
        assert_eq!(merge("import os\n\n\nimport sys\n").0, "import os,sys");
    }

    #[test]
    fn from_imports_merge_only_within_the_same_module() {
        let (code, _) = merge("from a import b\nfrom a import c as d\nfrom e import f\n");
        assert_eq!(code, "from a import b,c as d\nfrom e import f");
    }

    #[test]
    fn plain_and_from_imports_do_not_merge_together() {
        assert_eq!(
            merge("import os\nfrom re import match\n").0,
            "import os\nfrom re import match"
        );
    }

    #[test]
    fn relative_import_levels_are_distinct_modules() {
        let (code, _) = merge("from . import a\nfrom .. import b\n");
        assert_eq!(code, "from.import a\nfrom..import b");
    }

    #[test]
    fn comments_block_merging() {
        let (code, modified) = merge("import os\n# group two\nimport sys\n");
        assert_eq!(code, "import os\n# group two\nimport sys");
        assert!(!modified);
    }

    #[test]
    fn trailing_comments_block_merging() {
        let (code, modified) = merge("import os  # core\nimport sys\n");
        assert_eq!(code, "import os # core\nimport sys");
        assert!(!modified);
    }

    #[test]
    fn other_statements_block_merging() {
        assert_eq!(
            merge("import os\nx = 1\nimport sys\n").0,
            "import os\nx=1\nimport sys"
        );
    }

    #[test]
    fn parenthesized_star_and_semicolon_imports_are_left_alone() {
        let (code, modified) = merge("from a import (b)\nfrom a import c\n");
        assert_eq!(code, "from a import(b)\nfrom a import c");
        assert!(!modified);
        assert!(!merge("from a import *\nfrom a import b\n").1);
        assert!(!merge("import os;import sys\nimport re\n").1);
    }

    #[test]
    fn imports_inside_a_function_merge_at_that_level() {
        assert_eq!(
            merge("def f():\n    import os\n    import sys\n    return os\n").0,
            "def f():\n import os,sys\n return os"
        );
    }

    #[test]
    fn import_as_the_last_line_without_newline_is_untouched() {
        // The implicit final Newline still terminates the statement.
        assert_eq!(merge("import os\nimport sys").0, "import os,sys");
    }

    #[test]
    fn strip_removes_parameter_and_return_annotations() {
        let (code, modified) =
            strip("def f(x: int, *args: str, y: float = 1.0, **kw: int) -> int:\n    return x\n");
        assert_eq!(code, "def f(x,*args,y=1.0,**kw):\n return x");
        assert!(modified);
    }

    #[test]
    fn strip_rewrites_annotated_assignments() {
        assert_eq!(strip("x: int = 1\n").0, "x=1");
        assert_eq!(strip("x: dict[str, int] = {}\n").0, "x={}");
    }

    #[test]
    fn bare_annotations_become_pass() {
        assert_eq!(
            strip("class C:\n    x: int\n    y: str\n").0,
            "class C:\n pass\n pass"
        );
    }

    #[test]
    fn multiline_return_annotations_are_removed() {
        let (code, _) =
            strip("def f(\n    x,\n) -> dict[\n    str,\n    int,\n]:\n    return {}\n");
        assert_eq!(code, "def f(x,):\n return{}");
    }

    #[test]
    fn unannotated_code_is_not_modified() {
        let (code, modified) = strip("def f(x):\n    return x\n");
        assert_eq!(code, "def f(x):\n return x");
        assert!(!modified);
    }

    #[test]
    fn nested_functions_and_methods_are_stripped_too() {
        let (code, _) = strip("class C:\n    def m(self, x: int) -> None:\n        def g(y: str) -> str:\n            return y\n        return g\n");
        assert_eq!(
            code,
            "class C:\n def m(self,x):\n  def g(y):\n   return y\n  return g"
        );
    }
}
