//! The only module allowed to touch ruff parser APIs. The ruff crates are
//! internal components with no semver guarantees (pinned exactly in
//! Cargo.toml), so keeping all access here makes a future parser swap cheap.

use ruff_python_ast::ModModule;
use ruff_python_parser::{parse_module, Parsed};
use ruff_text_size::Ranged;
use tokenpress_core::{Error, Result};

/// A successfully parsed module, with its token stream.
#[derive(Debug)]
pub struct ParsedModule {
    parsed: Parsed<ModModule>,
}

/// One lexed token: its kind, the exact source text it covers, and its
/// position in the original source (used by transform passes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tok<'a> {
    pub kind: TokenKind,
    pub text: &'a str,
    pub range: TextRange,
}

pub use ruff_python_ast::token::TokenKind;
pub use ruff_python_ast::{self as ast, ModModule as Module};
pub use ruff_text_size::{Ranged as AstRanged, TextRange};

pub fn parse(source: &str) -> Result<ParsedModule> {
    match parse_module(source) {
        Ok(parsed) => Ok(ParsedModule { parsed }),
        Err(err) => Err(Error::Parse(err.to_string())),
    }
}

impl ParsedModule {
    /// Tokens paired with their original source text.
    pub fn tokens<'a>(&self, source: &'a str) -> Vec<Tok<'a>> {
        self.parsed
            .tokens()
            .iter()
            .map(|t| Tok {
                kind: t.kind(),
                text: &source[t.range()],
                range: t.range(),
            })
            .collect()
    }

    /// The parsed module AST (for transform passes).
    pub fn ast(&self) -> &ModModule {
        self.parsed.syntax()
    }

    /// Location-insensitive comparable form of the AST.
    pub fn comparable(&self) -> impl PartialEq + std::fmt::Debug + '_ {
        self.parsed
            .syntax()
            .body
            .iter()
            .map(ruff_python_ast::comparable::ComparableStmt::from)
            .collect::<Vec<_>>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_valid_module() {
        let m = parse("x = 1\n").unwrap();
        let toks = m.tokens("x = 1\n");
        assert!(toks
            .iter()
            .any(|t| t.kind == TokenKind::Name && t.text == "x"));
        assert!(toks.iter().any(|t| t.kind == TokenKind::Newline));
    }

    #[test]
    fn rejects_invalid_syntax() {
        let err = parse("def f(:\n").unwrap_err();
        assert!(err.to_string().starts_with("parse error:"));
    }

    #[test]
    fn comparable_ast_ignores_formatting() {
        let a = parse("x = (1 + 2)\n").unwrap();
        let b = parse("x=1+2").unwrap();
        assert!(a.comparable() == b.comparable());
    }

    #[test]
    fn comparable_ast_detects_semantic_change() {
        let a = parse("x = 1 + 2\n").unwrap();
        let b = parse("x = 1 - 2\n").unwrap();
        assert!(a.comparable() != b.comparable());
    }
}
