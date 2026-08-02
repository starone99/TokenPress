//! Verification: output that fails any check here is discarded by the caller
//! and never written.
//!
//! Two levels, mirroring the other language crates:
//!
//! - [`reparse`] — the output must parse at all, through
//!   [`crate::parser::parse`], which gates on prism's `errors()` only.
//!   Warnings are not a rejection: valid Ruby routinely produces them.
//! - [`equivalent`] — **comparable-artifact comparison**: the input and the
//!   output are both rendered by [`crate::comparable::comparable`] and the two
//!   artifacts must be equal. prism's nodes implement no `PartialEq` and their
//!   `Debug` is unusable as an artifact, so the prettyprint-derived,
//!   location-independent rendering is what stands in for AST equality here.
//!
//! # Bytes, not `str`
//!
//! Both entry points take `&[u8]`: prism parses bytes and non-UTF-8 Ruby
//! sources are legal (see [`crate::parser`]), and the emitter produces
//! `Vec<u8>`. Refusing non-UTF-8 input belongs at the `Formatter` boundary,
//! whose contract is `&str`, not here.
//!
//! # Owned results
//!
//! A [`crate::parser::ParseResult`] borrows the source it was given, so
//! returning one would tie the caller to this module's parse. Both entry
//! points therefore own their parse and return only owned data — the same
//! deliberate deviation from the Python and Rust crates that
//! `tokenpress-js`'s arena forced, and why [`reparse`] returns `()` where
//! those crates return the re-parsed module.
//!
//! # Why not [`crate::comparable::equivalent`]
//!
//! That helper returns `Result<bool>` and propagates a parse error from
//! *either* side, so a failure cannot be attributed to the output. This
//! module renders the two artifacts separately instead: a parse failure of
//! the **output** is the verifier's own `output failed to re-parse: …`
//! refusal, while a parse failure of the **input** stays a
//! [`tokenpress_core::Error::Parse`] — it is the caller's source, not a
//! candidate this module produced. [`equivalent`] therefore re-parses the
//! output itself, exactly as `tokenpress-js`'s does, so the `Formatter` wiring
//! never needs a separate [`reparse`] call at the equivalence levels:
//! `Reparse` → [`reparse`], `AstEquiv`/`External` → [`equivalent`].
//!
//! # Known over-refusals
//!
//! The artifact keeps the `= "source text"` half of every location field, and
//! its magic-comment prelude is built from a lexical scan. Both are what make
//! it strict, and both cost some real formatting wins. In each class the
//! verifier refuses and the file is left alone — no bad output is ever
//! written:
//!
//! - **Location slices spanning reformatted code.** An edit inside a location
//!   slice that covers more than one token is reported as a difference even
//!   when it is semantically inert. The two measured classes are a `<<~`
//!   squiggly heredoc's terminator/body re-indentation and a multi-line index
//!   call's `message_loc` (`a[1,\n2]`), both pinned below and in
//!   `crate::comparable`.
//! - **The strip-comments class.** `ParseResult::magic_comments` is a purely
//!   lexical scan, so the artifact's prelude records a whitelisted key
//!   *wherever* it lexically appears — including an `# encoding: …` line
//!   buried in prose comments deep in a file. Deleting that comment moves the
//!   artifact, and the verifier refuses the file. See the comment policy in
//!   [`crate::emit`].
//!
//! Comment loss in general is *not* caught here: comments are invisible to
//! the artifact (the semantic magic comments above excepted), so comment
//! policy is enforced by the emitter, not by this module — the same division
//! of labour as `tokenpress-js`.

use crate::comparable;
use crate::parser;
use tokenpress_core::{Error, Result};

/// Weakest level: the output must parse at all.
pub fn reparse(output: &[u8]) -> Result<()> {
    parser::parse(output).map_err(reparse_failure)?;
    Ok(())
}

/// Full check: the output must parse, and its comparable artifact must be
/// identical to `original`'s.
///
/// Returns [`tokenpress_core::Error::Parse`] when `original` itself does not
/// parse; every refusal of the candidate is an
/// [`tokenpress_core::Error::Verification`].
pub fn equivalent(original: &[u8], output: &[u8]) -> Result<()> {
    let expected = comparable::comparable(original)?;
    let actual = comparable::comparable(output).map_err(reparse_failure)?;
    if expected != actual {
        return Err(Error::Verification(
            "output AST differs from input".to_string(),
        ));
    }
    Ok(())
}

/// Restates a parse failure of the *output* as the verifier's refusal.
fn reparse_failure(error: Error) -> Error {
    Error::Verification(format!("output failed to re-parse: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emit;

    #[test]
    fn reparse_accepts_valid_and_rejects_invalid_output() {
        assert!(reparse(b"def f(a, b)\n  a + b\nend\n").is_ok());
        let err = reparse(b"def ; end").unwrap_err();
        assert!(matches!(err, Error::Verification(_)), "{err}");
        assert!(
            err.to_string()
                .starts_with("verification failed: output failed to re-parse: "),
            "{err}"
        );
    }

    #[test]
    fn reparse_does_not_gate_on_warnings() {
        // The parser boundary gates on `errors()` only, and verification
        // inherits exactly that: an assignment used as a condition warns but
        // is valid Ruby, and refusing it would refuse a working file.
        assert!(reparse(b"a = 0\nif a = 1\nend\n").is_ok());
    }

    #[test]
    fn reparse_accepts_non_utf8_output() {
        assert!(reparse(b"# encoding: binary\nx = \"\xff\xfe\"\n").is_ok());
    }

    #[test]
    fn a_minimized_hazard_source_is_equivalent_to_its_input() {
        // The real pipeline shape: whatever `minimize_source` produces for a
        // source full of protected constructs has to survive verification.
        let source = b"class A\n  def b(c)\n    s = \"a#{ c } b\"\n    xs = %w[a   b]\n    xs.each do |x|\n      p x   # note\n    end\n    s\n  end\nend\n";
        let output = emit::minimize_source(source).unwrap();
        assert!(output.len() < source.len());
        assert!(equivalent(source, &output).is_ok());
    }

    #[test]
    fn a_stripped_source_is_equivalent_to_its_input() {
        let source =
            b"# frozen_string_literal: true\n# leading note\nx = 1  # trailing note\ny = 2\n";
        let output = emit::strip_comments_source(source).unwrap();
        assert!(equivalent(source, &output).is_ok());
    }

    #[test]
    fn semantic_changes_are_rejected() {
        // A couple of the `comparable` DIFFERENT rows, restated at the
        // verifier level: `a -b` is a call with a unary-minus argument, not a
        // subtraction.
        for (name, original, output) in [
            ("integer value", &b"x = 1\n"[..], &b"x = 2\n"[..]),
            ("unary argument vs binary operator", b"a - b\n", b"a -b\n"),
            ("quote style", b"x = \"a\"\n", b"x = 'a'\n"),
        ] {
            let err = equivalent(original, output).unwrap_err();
            assert!(matches!(err, Error::Verification(_)), "{name}: {err}");
            assert_eq!(
                err.to_string(),
                "verification failed: output AST differs from input",
                "{name}"
            );
        }
    }

    #[test]
    fn unparsable_output_is_rejected_by_the_equivalence_check() {
        // `equivalent` re-parses the output itself, so the caller never needs
        // a separate `reparse` at this level.
        let err = equivalent(b"x = 1\n", b"1 +").unwrap_err();
        assert!(matches!(err, Error::Verification(_)), "{err}");
        assert!(
            err.to_string()
                .starts_with("verification failed: output failed to re-parse: "),
            "{err}"
        );
    }

    #[test]
    fn an_unparsable_input_is_a_parse_error_not_a_refusal() {
        // The input is the caller's source, not a candidate this module
        // produced: it is reported as what it is.
        let err = equivalent(b"def ; end", b"x = 1\n").unwrap_err();
        assert!(matches!(err, Error::Parse(_)), "{err}");
    }

    #[test]
    fn non_utf8_output_is_verified_on_its_bytes() {
        let source = b"# encoding: binary\nx  =  \"\xff\xfe\"\n";
        let output = emit::minimize_source(source).unwrap();
        assert!(equivalent(source, &output).is_ok());
        // Two different byte strings must not pass as equivalent.
        assert!(equivalent(source, b"# encoding: binary\nx = \"\xfe\xff\"\n").is_err());
    }

    #[test]
    fn a_squiggly_heredoc_reindent_is_a_known_over_refusal() {
        // Known over-refusal class 1: the terminator and body indentation live
        // inside `closing_loc`/`content_loc` slices, which the artifact keeps.
        // The safe direction — a good output is refused, a bad one never
        // accepted.
        let err = equivalent(b"x = <<~A\n    hi\n  A\n", b"x = <<~A\n  hi\nA\n").unwrap_err();
        assert_eq!(
            err.to_string(),
            "verification failed: output AST differs from input"
        );
    }

    #[test]
    fn a_multiline_index_call_is_a_known_over_refusal() {
        // Known over-refusal class 1 again: `message_loc` for `[]` spans the
        // whole bracket pair, so joining its arguments moves the slice.
        assert!(equivalent(b"a[1,\n2]\n", b"a[1, 2]\n").is_err());
    }

    #[test]
    fn deleting_a_buried_magic_looking_comment_is_a_known_over_refusal() {
        // Known over-refusal class 2: `magic_comments()` is lexical, so the
        // artifact's prelude records this `# encoding:` line even though it
        // sits after the first code token and Ruby ignores it there.
        // `strip_comments_source` deletes it, and the verifier refuses.
        let source = b"x = 1\n# encoding: binary\ny = 2\n";
        let output = emit::strip_comments_source(source).unwrap();
        assert!(!output.windows(8).any(|w| w == b"encoding"));
        let err = equivalent(source, &output).unwrap_err();
        assert_eq!(
            err.to_string(),
            "verification failed: output AST differs from input"
        );
    }
}
