//! Source-level transform passes that run on the token stream before the
//! minimal-whitespace render:
//!
//! - PY09 `merge_imports` (default on): joins *adjacent* `import` /
//!   `from m import` statements. Adjacency keeps module-init side-effect
//!   order identical, so this is always behavior-preserving.
//! - PYO2 `strip_docstrings` (opt-in): removes the leading string literal of a
//!   module, class or function body. This *does* empty `__doc__`, breaking
//!   `help()`, doctests and docstring-driven tooling, which is why it is not a
//!   default.
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

/// Spans of docstring statements collected from the AST.
#[derive(Default)]
struct DocstringSpans {
    /// Docstrings whose body keeps other statements (or is the module body,
    /// which may legally end up empty) - deleted.
    deletes: Vec<TextRange>,
    /// Docstrings that are the only statement of a class/function body -
    /// replaced by `pass` so the body stays syntactically valid.
    replaced_stmts: Vec<TextRange>,
}

impl DocstringSpans {
    /// Records the docstring of `body`, if it has one. A docstring is a
    /// string-literal expression statement in *first* position; a later one is
    /// an ordinary expression statement. `needs_pass` is false for the module
    /// body, the only body that may legally become empty.
    ///
    /// Implicit concatenation (`"a" "b"`) parses as one string literal and
    /// still sets `__doc__`, so it is a docstring; f-strings and byte strings
    /// are separate AST nodes that do not set `__doc__`, so they are kept.
    fn scan(&mut self, body: &[ast::Stmt], needs_pass: bool) {
        let Some(ast::Stmt::Expr(stmt)) = body.first() else {
            return;
        };
        if !matches!(*stmt.value, ast::Expr::StringLiteral(_)) {
            return;
        }
        if needs_pass && body.len() == 1 {
            self.replaced_stmts.push(stmt.range());
        } else {
            self.deletes.push(stmt.range());
        }
    }
}

impl ast::visitor::Visitor<'_> for DocstringSpans {
    fn visit_stmt(&mut self, stmt: &ast::Stmt) {
        match stmt {
            ast::Stmt::FunctionDef(f) => self.scan(&f.body, true),
            ast::Stmt::ClassDef(c) => self.scan(&c.body, true),
            _ => {}
        }
        ast::visitor::walk_stmt(self, stmt);
    }
}

/// PYO2 - strip docstrings, using AST spans to cut tokens.
pub fn strip_docstrings<'a>(tokens: &[Tok<'a>], module: &Module) -> (Vec<Tok<'a>>, bool) {
    use ast::visitor::Visitor;
    let mut spans = DocstringSpans::default();
    spans.scan(&module.body, false);
    for stmt in &module.body {
        spans.visit_stmt(stmt);
    }
    let modified = !(spans.deletes.is_empty() && spans.replaced_stmts.is_empty());
    if !modified {
        return (tokens.to_vec(), false);
    }

    let inside =
        |t: &Tok<'_>, s: &TextRange| t.range.start() >= s.start() && t.range.end() <= s.end();
    let mut dropped = vec![false; tokens.len()];
    for span in &spans.deletes {
        // The statement separator goes with the statement: a stray `;` is a
        // syntax error, and a stray Newline makes the emitted stream differ
        // from the verifier's re-lexed one. It is the first `Newline` / `;`
        // past the statement; only a trailing comment can sit in between.
        let cut_end = tokens
            .iter()
            .find(|t| {
                t.range.start() >= span.end()
                    && matches!(t.kind, TokenKind::Newline | TokenKind::Semi)
            })
            .map_or(span.end(), |t| t.range.end());
        for (i, tok) in tokens.iter().enumerate() {
            // A comment on the docstring's line is kept: it becomes a
            // comment-only line (unless PYO1 drops it at render time).
            if tok.range.start() >= span.start()
                && tok.range.start() < cut_end
                && tok.kind != TokenKind::Comment
            {
                dropped[i] = true;
            }
        }
    }

    let mut out: Vec<Tok<'a>> = Vec::new();
    let mut replaced_span: Option<TextRange> = None;
    for (i, tok) in tokens.iter().enumerate() {
        let replacement = spans
            .replaced_stmts
            .iter()
            .find(|s| inside(tok, s))
            .copied();
        if let Some(span) = replacement {
            // The first token of a sole docstring becomes `pass`.
            if replaced_span != Some(span) {
                replaced_span = Some(span);
                out.push(Tok {
                    kind: TokenKind::Pass,
                    text: "pass",
                    range: tok.range,
                });
            }
        } else if !dropped[i] {
            out.push(tok.clone());
        }
    }
    settle_indents(&mut out);
    (out, true)
}

/// Moves each `Indent` behind the comment-only lines that now open its block.
///
/// A lexer emits `Indent` at the block's first *logical* line, so comments
/// preceding it come first in the stream. Deleting a docstring can leave
/// comments in front of that first logical line; the verifier re-lexes the
/// output, so the pass has to put the `Indent` where the lexer will. Streams
/// that kept their first statement are untouched: an `Indent` is only ever
/// followed by comments once something before them was removed.
fn settle_indents(tokens: &mut [Tok<'_>]) {
    let mut i = 0;
    while i < tokens.len() {
        let mut end = i + 1;
        let mut comments = false;
        if tokens[i].kind == TokenKind::Indent {
            while end < tokens.len()
                && matches!(
                    tokens[end].kind,
                    TokenKind::Comment | TokenKind::NonLogicalNewline
                )
            {
                comments |= tokens[end].kind == TokenKind::Comment;
                end += 1;
            }
        }
        if comments {
            tokens[i..end].rotate_left(1);
        }
        i = end;
    }
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

    /// Kinds of the stripped token stream, for order assertions.
    fn docstring_kinds(source: &str) -> Vec<TokenKind> {
        let parsed = parser::parse(source).unwrap();
        let (tokens, _) = strip_docstrings(&parsed.tokens(source), parsed.ast());
        tokens.iter().map(|t| t.kind).collect()
    }

    fn docstrings(source: &str) -> (String, bool) {
        let parsed = parser::parse(source).unwrap();
        let (tokens, modified) = strip_docstrings(&parsed.tokens(source), parsed.ast());
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
    fn module_docstrings_are_removed() {
        let (code, modified) = docstrings("\"\"\"Doc.\"\"\"\nx = 1\n");
        assert_eq!(code, "x=1");
        assert!(modified);
    }

    #[test]
    fn a_module_of_only_a_docstring_becomes_empty() {
        // An empty module is legal Python, so no `pass` is needed here.
        assert_eq!(docstrings("\"\"\"Doc.\"\"\"\n").0, "");
    }

    #[test]
    fn function_and_class_docstrings_are_removed() {
        assert_eq!(
            docstrings("def f():\n    \"\"\"Doc.\"\"\"\n    return 1\n").0,
            "def f():\n return 1"
        );
        assert_eq!(
            docstrings("async def f():\n    \"\"\"Doc.\"\"\"\n    return 1\n").0,
            "async def f():\n return 1"
        );
        assert_eq!(
            docstrings(
                "class C:\n    \"\"\"Doc.\"\"\"\n    def m(self):\n        \"\"\"Method.\"\"\"\n        return 1\n"
            )
            .0,
            "class C:\n def m(self):\n  return 1"
        );
    }

    #[test]
    fn docstrings_of_nested_definitions_are_removed() {
        let (code, _) =
            docstrings("if flag:\n    def f():\n        \"\"\"Doc.\"\"\"\n        return 1\n");
        assert_eq!(code, "if flag:\n def f():\n  return 1");
    }

    #[test]
    fn a_sole_docstring_body_becomes_pass() {
        assert_eq!(
            docstrings("def f():\n    \"\"\"Doc.\"\"\"\n").0,
            "def f():\n pass"
        );
        assert_eq!(
            docstrings("class C:\n    \"\"\"Doc.\"\"\"\n").0,
            "class C:\n pass"
        );
        // Same body, written on the header line.
        assert_eq!(docstrings("def f(): \"\"\"Doc.\"\"\"\n").0, "def f():pass");
    }

    #[test]
    fn a_comment_after_a_sole_docstring_survives() {
        assert_eq!(
            docstrings("def f():\n    \"\"\"Doc.\"\"\"\n    # note\n").0,
            "def f():\n pass\n# note"
        );
    }

    #[test]
    fn a_trailing_comment_on_the_docstring_line_survives() {
        // The docstring statement including its Newline is gone, so the
        // comment becomes a comment-only line.
        assert_eq!(
            docstrings("\"\"\"Doc.\"\"\"  # note\nx = 1\n").0,
            "# note\nx=1"
        );
    }

    #[test]
    fn comments_starting_a_body_move_ahead_of_the_block_indent() {
        // With the docstring gone the comments start the body, and comment-only
        // lines are lexed *before* the block's Indent - the pass must order them
        // that way too, or the verifier sees a different stream than the output
        // lexes to.
        for (source, expected) in [
            (
                "def f():\n    \"\"\"Doc.\"\"\"  # note\n    return 1\n",
                "def f():\n # note\n return 1",
            ),
            (
                "class C:\n    \"\"\"Doc.\"\"\"\n\n    # note\n    x = 1\n",
                "class C:\n # note\n x=1",
            ),
        ] {
            assert_eq!(docstrings(source).0, expected);
            let kinds = docstring_kinds(source);
            let indent = kinds.iter().position(|k| *k == TokenKind::Indent).unwrap();
            let comment = kinds.iter().position(|k| *k == TokenKind::Comment).unwrap();
            assert!(
                comment < indent,
                "comment must precede Indent in {source:?}"
            );
        }
        // A block whose first surviving token is real keeps its Indent.
        let kinds = docstring_kinds("class C:\n    \"\"\"Doc.\"\"\"\n    x = 1\n");
        let indent = kinds.iter().position(|k| *k == TokenKind::Indent).unwrap();
        assert_eq!(kinds[indent + 1], TokenKind::Name);
    }

    #[test]
    fn a_semicolon_terminated_docstring_is_removed_with_its_separator() {
        assert_eq!(docstrings("\"\"\"Doc.\"\"\";x = 1\n").0, "x=1");
    }

    #[test]
    fn implicitly_concatenated_and_parenthesized_docstrings_are_removed() {
        // CPython sets `__doc__` for both forms, so both are docstrings.
        assert_eq!(
            docstrings("def f():\n    \"a\" \"b\"\n    return 1\n").0,
            "def f():\n return 1"
        );
        assert_eq!(
            docstrings("def f():\n    (\"Doc.\")\n    return 1\n").0,
            "def f():\n return 1"
        );
    }

    #[test]
    fn fstrings_and_byte_strings_are_not_docstrings() {
        // Neither sets `__doc__` in CPython, so both are kept.
        let (code, modified) = docstrings("def f():\n    f\"{x}\"\n    return 1\n");
        assert_eq!(code, "def f():\n f\"{x}\"\n return 1");
        assert!(!modified);
        let (code, modified) = docstrings("def f():\n    b\"doc\"\n    return 1\n");
        assert_eq!(code, "def f():\n b\"doc\"\n return 1");
        assert!(!modified);
    }

    #[test]
    fn string_statements_that_are_not_first_are_kept() {
        let (code, modified) = docstrings("def f():\n    x = 1\n    \"note\"\n");
        assert_eq!(code, "def f():\n x=1\n \"note\"");
        assert!(!modified);
        // A block that is not a module/class/function body has no docstring.
        let (code, modified) = docstrings("if flag:\n    \"note\"\n");
        assert_eq!(code, "if flag:\n \"note\"");
        assert!(!modified);
    }

    #[test]
    fn sources_without_docstrings_are_not_modified() {
        let (code, modified) = docstrings("def f():\n    return 1\n");
        assert_eq!(code, "def f():\n return 1");
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
