//! Verification: output that fails any check here is discarded by the caller
//! and never written. See DESIGN §7.
//!
//! When no transform pass modified the token stream, the output must match
//! the original AST exactly. When a pass did modify it (import merging,
//! annotation stripping), the output is compared against the pass-produced
//! token stream instead, plus an import-preservation check.

use crate::parser::{self, ast, ParsedModule, Tok, TokenKind};
use crate::PythonOptions;
use tokenpress_core::{Error, Result};

/// Weakest level: the output must parse at all.
pub fn reparse(code: &str) -> Result<ParsedModule> {
    parser::parse(code).map_err(|e| Error::Verification(format!("output failed to re-parse: {e}")))
}

/// Full check for the given intended token stream (original or surgical).
pub fn full(
    original: &ParsedModule,
    intended: &[Tok<'_>],
    code: &str,
    options: &PythonOptions,
    surgery_modified: bool,
) -> Result<()> {
    let reparsed = reparse(code)?;
    if !surgery_modified && original.comparable() != reparsed.comparable() {
        return Err(Error::Verification("output AST differs from input".into()));
    }
    let a = canonical(intended, options);
    let b_tokens = reparsed.tokens(code);
    let b = canonical(&b_tokens, options);
    if a != b {
        return Err(Error::Verification(
            "output token stream differs from input".into(),
        ));
    }
    if surgery_modified {
        imports_preserved(original, &reparsed)?;
    }
    Ok(())
}

/// Token sequence with formatting-only differences erased: non-logical
/// newlines and EOF dropped, whitespace token texts ignored, trailing
/// newlines trimmed. Comments are compared only when they are being kept.
fn canonical<'a>(tokens: &[Tok<'a>], options: &PythonOptions) -> Vec<(TokenKind, Option<&'a str>)> {
    let mut seq: Vec<(TokenKind, Option<&'a str>)> = tokens
        .iter()
        .filter(|t| {
            !(matches!(t.kind, TokenKind::NonLogicalNewline | TokenKind::EndOfFile)
                || (t.kind == TokenKind::Comment && options.strip_comments))
        })
        .map(|t| {
            let text = match t.kind {
                TokenKind::Indent | TokenKind::Dedent | TokenKind::Newline => None,
                _ => Some(t.text),
            };
            (t.kind, text)
        })
        .collect();
    while seq.last().is_some_and(|(k, _)| *k == TokenKind::Newline) {
        seq.pop();
    }
    seq
}

/// Every imported (module, name, alias) must survive, in execution order.
fn imports_preserved(original: &ParsedModule, output: &ParsedModule) -> Result<()> {
    if collect_imports(original) != collect_imports(output) {
        return Err(Error::Verification(
            "output imports differ from input".into(),
        ));
    }
    Ok(())
}

#[derive(Default)]
struct ImportCollector {
    items: Vec<String>,
}

impl ast::visitor::Visitor<'_> for ImportCollector {
    fn visit_stmt(&mut self, stmt: &ast::Stmt) {
        match stmt {
            ast::Stmt::Import(imp) => {
                for alias in &imp.names {
                    self.items.push(format!(
                        "import {} as {:?}",
                        alias.name,
                        alias.asname.as_ref().map(|a| a.as_str())
                    ));
                }
            }
            ast::Stmt::ImportFrom(imp) => {
                for alias in &imp.names {
                    self.items.push(format!(
                        "from {}{:?} import {} as {:?}",
                        ".".repeat(imp.level as usize),
                        imp.module.as_ref().map(|m| m.as_str()),
                        alias.name,
                        alias.asname.as_ref().map(|a| a.as_str())
                    ));
                }
            }
            _ => {}
        }
        ast::visitor::walk_stmt(self, stmt);
    }
}

fn collect_imports(parsed: &ParsedModule) -> Vec<String> {
    use ast::visitor::Visitor;
    let mut collector = ImportCollector::default();
    for stmt in &parsed.ast().body {
        collector.visit_stmt(stmt);
    }
    collector.items
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> PythonOptions {
        PythonOptions::default()
    }

    fn check(source: &str, code: &str) -> Result<()> {
        let parsed = parser::parse(source).unwrap();
        let tokens = parsed.tokens(source);
        full(&parsed, &tokens, code, &opts(), false)
    }

    #[test]
    fn reparse_accepts_valid_and_rejects_invalid_output() {
        assert!(reparse("x=1").is_ok());
        let err = reparse("def f(:").err().unwrap();
        assert!(err.to_string().contains("failed to re-parse"));
    }

    #[test]
    fn identical_programs_are_equivalent() {
        assert!(check("x = 1\ny = 2\n", "x=1\ny=2").is_ok());
    }

    #[test]
    fn semantic_change_is_caught_by_ast_comparison() {
        let err = check("x = 1\n", "x=2").unwrap_err();
        assert!(err.to_string().contains("AST differs"));
    }

    #[test]
    fn token_change_is_caught_even_when_ast_matches() {
        // Redundant parens are invisible in the AST but not in the tokens.
        let err = check("x = 1\n", "x=(1)").unwrap_err();
        assert!(err.to_string().contains("token stream differs"));
    }

    #[test]
    fn unparsable_output_is_rejected() {
        let err = check("x = 1\n", "def f(:").unwrap_err();
        assert!(err.to_string().contains("failed to re-parse"));
    }

    #[test]
    fn kept_comments_participate_in_the_comparison() {
        let src = "x = 1  # note\n";
        let parsed = parser::parse(src).unwrap();
        let tokens = parsed.tokens(src);
        assert!(full(&parsed, &tokens, "x=1 # note", &opts(), false).is_ok());
        let err = full(&parsed, &tokens, "x=1", &opts(), false).unwrap_err();
        assert!(err.to_string().contains("token stream differs"));
    }

    #[test]
    fn stripped_comments_are_ignored_in_the_comparison() {
        let strip = PythonOptions {
            strip_comments: true,
            ..PythonOptions::default()
        };
        let src = "x = 1  # note\n";
        let parsed = parser::parse(src).unwrap();
        let tokens = parsed.tokens(src);
        assert!(full(&parsed, &tokens, "x=1", &strip, false).is_ok());
    }

    #[test]
    fn surgery_mode_compares_against_the_surgical_tokens() {
        // Simulate an import merge: intended tokens come from the merged form.
        let src = "import os\nimport sys\n";
        let parsed = parser::parse(src).unwrap();
        let (merged, modified) = crate::passes::merge_imports(&parsed.tokens(src));
        assert!(modified);
        assert!(full(&parsed, &merged, "import os,sys", &opts(), true).is_ok());
    }

    #[test]
    fn surgery_mode_still_requires_imports_to_survive() {
        let src = "import os\nimport sys\n";
        let parsed = parser::parse(src).unwrap();
        // Pretend a buggy pass dropped `sys`: intended tokens match the
        // (wrong) output, so only the import check can catch it.
        let broken = parser::parse("import os\n").unwrap();
        let broken_tokens = broken.tokens("import os\n");
        let err = full(&parsed, &broken_tokens, "import os", &opts(), true).unwrap_err();
        assert!(err.to_string().contains("imports differ"));
    }

    #[test]
    fn import_collection_covers_aliases_levels_and_nesting() {
        let a = parser::parse("import a as b\nfrom ..c import d as e\ndef f():\n    import g\n")
            .unwrap();
        let b = parser::parse("import a as b\nfrom ..c import d as e\ndef f():\n    import g\n")
            .unwrap();
        assert!(imports_preserved(&a, &b).is_ok());
        let c =
            parser::parse("import a\nfrom ..c import d as e\ndef f():\n    import g\n").unwrap();
        assert!(imports_preserved(&a, &c).is_err());
    }
}
