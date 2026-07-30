//! Verification: output that fails any check here is discarded by the caller
//! and never written. See DESIGN §7.

use crate::parser::{self, ParsedModule, TokenKind};
use crate::PythonOptions;
use tokenpress_core::{Error, Result};

/// Weakest level: the output must parse at all.
pub fn reparse(code: &str) -> Result<ParsedModule> {
    parser::parse(code).map_err(|e| Error::Verification(format!("output failed to re-parse: {e}")))
}

/// Full check: the output must parse, have an identical AST, and carry the
/// same token stream (modulo whitespace and, when stripping, comments).
pub fn equivalent(
    source: &str,
    original: &ParsedModule,
    code: &str,
    options: &PythonOptions,
) -> Result<()> {
    let reparsed = reparse(code)?;
    if original.comparable() != reparsed.comparable() {
        return Err(Error::Verification("output AST differs from input".into()));
    }
    let a = canonical_tokens(source, original, options);
    let b = canonical_tokens(code, &reparsed, options);
    if a != b {
        return Err(Error::Verification(
            "output token stream differs from input".into(),
        ));
    }
    Ok(())
}

/// Token sequence with formatting-only differences erased: non-logical
/// newlines and EOF dropped, whitespace token texts ignored, trailing
/// newlines trimmed. Comments are compared only when they are being kept.
fn canonical_tokens<'a>(
    source: &'a str,
    parsed: &ParsedModule,
    options: &PythonOptions,
) -> Vec<(TokenKind, Option<&'a str>)> {
    let mut seq: Vec<(TokenKind, Option<&'a str>)> = parsed
        .tokens(source)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> PythonOptions {
        PythonOptions::default()
    }

    #[test]
    fn reparse_accepts_valid_and_rejects_invalid_output() {
        assert!(reparse("x=1").is_ok());
        let err = reparse("def f(:").unwrap_err();
        assert!(err.to_string().contains("failed to re-parse"));
    }

    #[test]
    fn identical_programs_are_equivalent() {
        let src = "x = 1\ny = 2\n";
        let parsed = parser::parse(src).unwrap();
        assert!(equivalent(src, &parsed, "x=1\ny=2", &opts()).is_ok());
    }

    #[test]
    fn semantic_change_is_caught_by_ast_comparison() {
        let src = "x = 1\n";
        let parsed = parser::parse(src).unwrap();
        let err = equivalent(src, &parsed, "x=2", &opts()).unwrap_err();
        assert!(err.to_string().contains("AST differs"));
    }

    #[test]
    fn token_change_is_caught_even_when_ast_matches() {
        // Redundant parens are invisible in the AST but not in the tokens.
        let src = "x = 1\n";
        let parsed = parser::parse(src).unwrap();
        let err = equivalent(src, &parsed, "x=(1)", &opts()).unwrap_err();
        assert!(err.to_string().contains("token stream differs"));
    }

    #[test]
    fn unparsable_output_is_rejected() {
        let src = "x = 1\n";
        let parsed = parser::parse(src).unwrap();
        let err = equivalent(src, &parsed, "def f(:", &opts()).unwrap_err();
        assert!(err.to_string().contains("failed to re-parse"));
    }

    #[test]
    fn kept_comments_participate_in_the_comparison() {
        let keep = PythonOptions {
            strip_comments: false,
        };
        let src = "x = 1  # note\n";
        let parsed = parser::parse(src).unwrap();
        assert!(equivalent(src, &parsed, "x=1 # note", &keep).is_ok());
        let err = equivalent(src, &parsed, "x=1", &keep).unwrap_err();
        assert!(err.to_string().contains("token stream differs"));
    }
}
