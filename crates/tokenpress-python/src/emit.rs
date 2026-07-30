//! Token-stream renderer: re-renders the lexed token stream with minimal
//! whitespace. Implements PY01 (space minimization), PY02 (1-char indent),
//! PY03 (blank-line/continuation removal), PY08 (backslash joins are implicit)
//! and PYO1 (comment strip/keep).

use crate::parser::{Tok, TokenKind};
use crate::PythonOptions;

/// Two-character sequences that would lex as one operator if glued.
/// A separating space is kept between adjacent tokens forming one of these.
const GLUE_PAIRS: &[(char, char)] = &[
    ('*', '*'),
    ('*', '='),
    ('/', '/'),
    ('/', '='),
    ('<', '<'),
    ('<', '='),
    ('>', '>'),
    ('>', '='),
    ('=', '='),
    ('!', '='),
    ('-', '>'),
    (':', '='),
    ('+', '='),
    ('-', '='),
    ('%', '='),
    ('@', '='),
    ('&', '='),
    ('|', '='),
    ('^', '='),
];

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whether removing all whitespace between `prev` and `next` would change
/// how the pair lexes.
fn needs_space(prev: &Tok<'_>, next: &Tok<'_>) -> bool {
    let p = prev.text.chars().next_back().unwrap_or('\0');
    let n = next.text.chars().next().unwrap_or('\0');
    if is_word(p) && is_word(n) {
        return true;
    }
    if GLUE_PAIRS.contains(&(p, n)) {
        return true;
    }
    // `1.real` lexes as a float; `1 .real` is the attribute access.
    if matches!(
        prev.kind,
        TokenKind::Int | TokenKind::Float | TokenKind::Complex
    ) && n == '.'
    {
        return true;
    }
    // Two same-quote strings glued can change quoting: `"" "x"` → `"""x` opens
    // a triple-quoted string (implicit concatenation).
    if matches!(p, '"' | '\'') && n == p {
        return true;
    }
    // An identifier glued onto a quote could form a string prefix (`r"x"`).
    prev.kind == TokenKind::Name && matches!(n, '"' | '\'')
}

pub fn render(tokens: &[Tok<'_>], options: &PythonOptions) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    // Last token emitted on the current line; None right after a line break.
    let mut prev: Option<&Tok<'_>> = None;
    // Standalone comments waiting for the next real token, so they are
    // indented at that token's block level (PYO1 keep mode).
    let mut pending_comments: Vec<&str> = Vec::new();

    for tok in tokens {
        match tok.kind {
            TokenKind::Indent => depth += 1,
            TokenKind::Dedent => depth = depth.saturating_sub(1),
            TokenKind::NonLogicalNewline | TokenKind::EndOfFile => {}
            TokenKind::Newline => {
                // `prev` is None right after an emitted comment, which
                // already broke the line — avoid a double newline.
                if prev.is_some() {
                    out.push('\n');
                }
                prev = None;
            }
            TokenKind::Comment => {
                if !options.strip_comments {
                    match prev {
                        Some(_) => {
                            // An inline comment swallows everything after it
                            // on the line, so always break the line here.
                            // Inside brackets the break is a valid
                            // continuation line.
                            out.push(' ');
                            out.push_str(tok.text);
                            out.push('\n');
                            prev = None;
                        }
                        None => pending_comments.push(tok.text),
                    }
                }
            }
            _ => {
                for comment in pending_comments.drain(..) {
                    out.push_str(&" ".repeat(depth));
                    out.push_str(comment);
                    out.push('\n');
                }
                match prev {
                    None => out.push_str(&" ".repeat(depth)),
                    Some(p) => {
                        if needs_space(p, tok) {
                            out.push(' ');
                        }
                    }
                }
                out.push_str(tok.text);
                prev = Some(tok);
            }
        }
    }
    for comment in pending_comments.drain(..) {
        out.push_str(&" ".repeat(depth));
        out.push_str(comment);
        out.push('\n');
    }
    while out.ends_with('\n') {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    fn tok(kind: TokenKind, text: &str) -> Tok<'_> {
        Tok {
            kind,
            text,
            range: crate::parser::TextRange::empty(0.into()),
        }
    }

    #[test]
    fn adjacent_words_need_a_space() {
        assert!(needs_space(
            &tok(TokenKind::Return, "return"),
            &tok(TokenKind::Int, "1")
        ));
        assert!(!needs_space(
            &tok(TokenKind::Name, "f"),
            &tok(TokenKind::Lpar, "(")
        ));
    }

    #[test]
    fn glueable_operator_pairs_need_a_space() {
        assert!(needs_space(
            &tok(TokenKind::Colon, ":"),
            &tok(TokenKind::Equal, "=")
        ));
        assert!(!needs_space(
            &tok(TokenKind::Semi, ";"),
            &tok(TokenKind::Rbrace, "}")
        ));
    }

    #[test]
    fn integer_followed_by_dot_needs_a_space() {
        assert!(needs_space(
            &tok(TokenKind::Int, "1"),
            &tok(TokenKind::Dot, ".")
        ));
    }

    #[test]
    fn implicit_string_concatenation_keeps_same_quote_strings_apart() {
        // `"" "x"` glued would open a triple-quoted string.
        let source = "expected = (\n    \"\"\n    \"Rick: hi\"\n    \" Morty: hello\"\n)\n";
        let parsed = parser::parse(source).unwrap();
        let out = render(&parsed.tokens(source), &PythonOptions::default());
        assert_eq!(out, "expected=(\"\" \"Rick: hi\" \" Morty: hello\")");
        assert!(parser::parse(&out).is_ok());
        // Different quote styles cannot merge and stay glued.
        assert!(!needs_space(
            &tok(TokenKind::String, "\"a\""),
            &tok(TokenKind::String, "'b'")
        ));
    }

    #[test]
    fn name_followed_by_quote_needs_a_space() {
        // A variable named `r` glued onto a string would form a raw string.
        assert!(needs_space(
            &tok(TokenKind::Name, "r"),
            &tok(TokenKind::String, "\"x\"")
        ));
    }

    #[test]
    fn empty_token_text_never_needs_a_space() {
        assert!(!needs_space(
            &tok(TokenKind::FStringMiddle, ""),
            &tok(TokenKind::Lbrace, "{")
        ));
    }

    #[test]
    fn inline_comments_inside_brackets_break_the_line() {
        let source = "result = compute(\n    first,  # the first operand\n    second,\n)\n";
        let parsed = parser::parse(source).unwrap();
        let out = render(&parsed.tokens(source), &PythonOptions::default());
        assert_eq!(out, "result=compute(first, # the first operand\nsecond,)");
        // The output must still parse (the comment must not swallow `second`).
        assert!(parser::parse(&out).is_ok());
    }

    #[test]
    fn comment_only_line_inside_brackets_survives() {
        let source = "items = [\n    # leading note\n    1,\n]\n";
        let parsed = parser::parse(source).unwrap();
        let out = render(&parsed.tokens(source), &PythonOptions::default());
        assert_eq!(out, "items=[ # leading note\n1,]");
        assert!(parser::parse(&out).is_ok());
    }

    #[test]
    fn comment_only_file_flushes_pending_comments_at_eof() {
        let source = "# alpha\n# beta\n";
        let parsed = parser::parse(source).unwrap();
        let keep = PythonOptions::default();
        assert_eq!(render(&parsed.tokens(source), &keep), "# alpha\n# beta");
    }
}
