//! The emitter's foundation: which bytes of a Ruby source may never be
//! touched, and the rewriter that every emitter policy plugs into.
//!
//! # The protected-span model
//!
//! Ruby has no whitespace-insensitive normal form to print back out, and
//! prism is a parser, not a code generator — there is nothing to render an
//! AST with. The emitter therefore works on the **source bytes**: it splits
//! the file into *protected spans*, whose bytes are copied out verbatim, and
//! the *gaps* between them, which are the only thing a policy may rewrite.
//! Anything not proven safe stays protected, so the failure mode of a missed
//! construct is a missed saving rather than a corrupted file.
//!
//! A span is protected when its bytes are load-bearing:
//!
//! - every string, symbol, regexp and xstring literal, in all of its
//!   spellings — `'a'`, `"a"`, `?a`, `%q()`, `%Q()`, `:a`, `:"a"`, `%s()`,
//!   `/re/`, `%r{}`, a regexp in condition position, backticks, `%x()` —
//!   **including** interpolations: `"a#{ b }c"` is protected whole. The code
//!   inside `#{}` is rewritable in principle, but nesting a rewriter inside a
//!   literal buys little and risks much;
//! - `%w`/`%i`/`%W`/`%I` word lists as a whole, because the whitespace
//!   *between* their elements is the element separator — protecting only the
//!   elements would leave that separator in a rewritable gap;
//! - heredoc bodies and terminators (below);
//! - comments and embdocs, from [`ParseResult::comments`]. They are invisible
//!   to the AST, and an embdoc's `=begin`/`=end` must stay at column 0;
//! - the `__END__` data section, from [`ParseResult::data_loc`], which is
//!   likewise invisible to the AST.
//!
//! Spans are returned sorted and merged, so a comment inside an interpolation
//! (`"a#{1 # c\n}b"`) or a heredoc opened inside another heredoc's body ends
//! up as one range rather than two overlapping ones.
//!
//! # The heredoc rule
//!
//! A heredoc's node location is only its **opening marker** (`<<~EOS`); the
//! body and terminator sit further down the file, after whatever else shares
//! the marker's line. The protected region is therefore derived, not read:
//!
//! ```text
//! [ end of the line carrying the opening marker .. end of `closing_loc` ]
//! ```
//!
//! It starts *at* the newline that ends the marker line — that newline is
//! what begins the body, so it may not be collapsed away — and ends past the
//! terminator line. Everything before it on the marker line (`x = `,
//! `.strip`, a second heredoc's marker, `) do`) stays in a gap and stays
//! rewritable. Two heredocs opened on one line derive two overlapping regions
//! that merge into one, which is exactly right: the region between the first
//! terminator and the second is the second heredoc's body.
//!
//! The rule covers every form uniformly — `<<EOS`, `<<-EOS`, `<<~EOS`,
//! `<<~'EOS'`, `<<"EOS"`, `` <<~`EOS` `` — because the only thing it reads
//! from the opening delimiter is that it starts with `<<`.
//!
//! # Policy stages
//!
//! This module ships **no emitter policy**. [`rewrite`] takes the policy as a
//! parameter and [`keep`] is the identity one, so [`identity`] reproduces its
//! input byte for byte. The whitespace policy and the `strip_comments` policy
//! land in later sub-tasks as new gap policies; nothing about span collection
//! has to change for them.

use std::ops::Range;

use tokenpress_core::Result;

use crate::parser::{self, Location, Node, ParseResult, Visit};

/// A byte range of the source that has to be reproduced verbatim.
pub type Span = Range<usize>;

/// Collects every byte range of `parsed`'s source that must survive verbatim.
///
/// The result is sorted, non-overlapping and within the source — the
/// precondition [`rewrite`] is documented to need. Spans that touch or
/// overlap are merged: the gap between two touching spans is empty, so
/// keeping them apart would only hand the policy nothing to do.
pub fn protected_spans(parsed: &ParseResult<'_>) -> Vec<Span> {
    let mut collector = Collector {
        source: parsed.source(),
        spans: Vec::new(),
    };
    collector.visit(&parsed.node());
    let mut spans = collector.spans;

    // Comments and embdocs are not in the tree at all.
    spans.extend(parsed.comments().map(|comment| span(&comment.location())));
    // Neither is anything after `__END__`.
    if let Some(data) = parsed.data_loc() {
        spans.push(span(&data));
    }
    merge(spans)
}

/// Rebuilds `source`, copying `spans` verbatim and passing every gap between
/// them — the whole of the rest of the file — through `policy`.
///
/// `spans` must be sorted, non-overlapping and within `source`, which is what
/// [`protected_spans`] returns.
pub fn rewrite(source: &[u8], spans: &[Span], mut policy: impl FnMut(&[u8]) -> Vec<u8>) -> Vec<u8> {
    let mut out = Vec::with_capacity(source.len());
    let mut cursor = 0;
    for span in spans {
        out.extend_from_slice(&policy(&source[cursor..span.start]));
        out.extend_from_slice(&source[span.clone()]);
        cursor = span.end;
    }
    out.extend_from_slice(&policy(&source[cursor..]));
    out
}

/// The identity gap policy: the bytes between protected spans pass through
/// unchanged. The policy stages replace it; see the module docs.
pub fn keep(gap: &[u8]) -> Vec<u8> {
    gap.to_vec()
}

/// Parses `source`, collects its protected spans and rewrites it with
/// [`keep`], reproducing the input byte for byte.
///
/// This is the whole pipeline with the policy seam left empty: it exists so
/// span collection is exercised end to end, and it stays an identity even
/// once the policy stages land.
///
/// Returns [`tokenpress_core::Error::Parse`] for a source prism reports
/// errors for.
pub fn identity(source: &[u8]) -> Result<Vec<u8>> {
    let parsed = parser::parse(source)?;
    let spans = protected_spans(&parsed);
    Ok(rewrite(source, &spans, keep))
}

/// The `Visit` pass that records literal spans. It is generic over node kind
/// — the two `*_node_enter` hooks see *every* node — so a construct is
/// classified by what its own location and delimiters say, never by where in
/// the tree it was found.
struct Collector<'pr> {
    source: &'pr [u8],
    spans: Vec<Span>,
}

impl<'pr> Collector<'pr> {
    fn record(&mut self, node: &Node<'pr>) {
        if let Some(node) = node.as_string_node() {
            self.spans.push(span(&node.location()));
            self.heredoc(node.opening_loc(), node.closing_loc());
        } else if let Some(node) = node.as_interpolated_string_node() {
            self.spans.push(span(&node.location()));
            self.heredoc(node.opening_loc(), node.closing_loc());
        } else if let Some(node) = node.as_x_string_node() {
            self.spans.push(span(&node.location()));
            self.heredoc(Some(node.opening_loc()), Some(node.closing_loc()));
        } else if let Some(node) = node.as_interpolated_x_string_node() {
            self.spans.push(span(&node.location()));
            self.heredoc(Some(node.opening_loc()), Some(node.closing_loc()));
        } else if let Some(node) = node.as_symbol_node() {
            self.spans.push(span(&node.location()));
        } else if let Some(node) = node.as_interpolated_symbol_node() {
            self.spans.push(span(&node.location()));
        } else if let Some(node) = node.as_regular_expression_node() {
            self.spans.push(span(&node.location()));
        } else if let Some(node) = node.as_interpolated_regular_expression_node() {
            self.spans.push(span(&node.location()));
        } else if let Some(node) = node.as_match_last_line_node() {
            self.spans.push(span(&node.location()));
        } else if let Some(node) = node.as_interpolated_match_last_line_node() {
            self.spans.push(span(&node.location()));
        } else if let Some(node) = node.as_array_node() {
            // Only the `%w`/`%i`/`%W`/`%I` spelling, whose separators are
            // whitespace. A `[1, 2]` array opens with `[` and an implicit
            // `1, 2` array has no opening delimiter at all; both are ordinary
            // rewritable code.
            if node
                .opening_loc()
                .is_some_and(|opening| opening.as_slice().starts_with(b"%"))
            {
                self.spans.push(span(&node.location()));
            }
        }
    }

    /// Records a heredoc's body and terminator when `opening` is a heredoc
    /// delimiter. See the module docs for the derivation.
    fn heredoc(&mut self, opening: Option<Location<'pr>>, closing: Option<Location<'pr>>) {
        // A literal with no delimiters (a `%w` element) or no closing one
        // (`?a`) is never a heredoc.
        let (Some(opening), Some(closing)) = (opening, closing) else {
            return;
        };
        if opening.as_slice().starts_with(b"<<") {
            let after_marker = opening.end_offset();
            // Distance to the newline that ends the marker's line. Counting
            // rather than searching keeps this total: a marker line with no
            // newline left in the file would simply run to the end, and no
            // arm exists that could not be taken.
            let to_line_end = self.source[after_marker..]
                .iter()
                .take_while(|byte| **byte != b'\n')
                .count();
            self.spans
                .push(after_marker + to_line_end..closing.end_offset());
        }
    }
}

impl<'pr> Visit<'pr> for Collector<'pr> {
    fn visit_branch_node_enter(&mut self, node: Node<'pr>) {
        self.record(&node);
    }

    fn visit_leaf_node_enter(&mut self, node: Node<'pr>) {
        self.record(&node);
    }
}

fn span(location: &Location<'_>) -> Span {
    location.start_offset()..location.end_offset()
}

/// Sorts `spans` and merges every pair that overlaps or touches.
fn merge(mut spans: Vec<Span>) -> Vec<Span> {
    spans.sort_by_key(|span| span.start);
    let mut merged: Vec<Span> = Vec::with_capacity(spans.len());
    for span in spans {
        match merged.last_mut() {
            Some(last) if span.start <= last.end => last.end = last.end.max(span.end),
            _ => merged.push(span),
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokenpress_core::Error;

    /// Every construct whose bytes the emitter may not touch. Byte strings,
    /// not `&str`: a Ruby source is a byte sequence and one of these is not
    /// valid UTF-8.
    const HAZARDS: &[(&str, &[u8])] = &[
        ("plain heredoc", b"x = <<EOS\nbody\nEOS\n"),
        ("dash heredoc", b"x = <<-EOS\n  body\n  EOS\n"),
        ("squiggly heredoc", b"x = <<~EOS\n  body\nEOS\n"),
        (
            "single quoted heredoc marker",
            b"x = <<~'EOS'\n  a#{b}\nEOS\n",
        ),
        (
            "double quoted heredoc marker",
            b"x = <<\"EOS\"\n  body\nEOS\n",
        ),
        (
            "two heredocs on one line",
            b"a = <<~A; b = <<~B\n  one\nA\n  two\nB\n",
        ),
        (
            "heredoc with interpolation",
            b"x = <<~EOS\n  a#{1 + 2}b\nEOS\n",
        ),
        (
            "heredoc with a method call",
            b"x = <<~EOS.strip\n  body\nEOS\n",
        ),
        (
            "heredoc with a chained method call",
            b"x = <<~A.strip.upcase\n  body\nA\n",
        ),
        ("heredoc as an argument", b"p(<<~A, 1)\n  hi\nA\n"),
        (
            "heredoc argument followed by a block",
            b"foo(<<~A) do\n  b\nA\n  bar\nend\n",
        ),
        (
            "heredoc nested in an interpolation",
            b"a = <<~A\n  #{<<~B}\n  inner\nB\n  rest\nA\n",
        ),
        ("empty heredoc", b"x = <<~A\nA\n"),
        ("backtick heredoc", b"x = <<~`A`\n  echo\nA\n"),
        (
            "heredoc body that looks like a comment",
            b"x = <<~A\n  # no\nA\n",
        ),
        (
            "heredoc body that looks like an embdoc",
            b"x = <<~A\n=begin\nno\n=end\nA\n",
        ),
        (
            "comment after a heredoc marker",
            b"x = <<~A # note\n  body\nA\n",
        ),
        (
            "heredoc before __END__",
            b"x = <<~A\n  b\nA\n__END__\ndata\n",
        ),
        ("word list", b"x = %w[a   b]\n"),
        ("symbol list", b"x = %i[a   b]\n"),
        ("interpolated word list", b"x = %W[a#{b}   c]\n"),
        ("interpolated symbol list", b"x = %I[a#{b}   c]\n"),
        ("multi-line word list", b"x = %w[a\n   b]\n"),
        ("empty word list", b"x = %w[]\n"),
        ("percent q string", b"x = %q(a   b)\n"),
        ("percent big q string", b"x = %Q(a   #{b})\n"),
        ("percent regexp", b"x = %r{a   b}i\n"),
        ("plain regexp", b"x = /a   b/\n"),
        ("interpolated regexp", b"x = /a#{b}   c/\n"),
        ("match last line regexp", b"if /a   b/\n  1\nend\n"),
        (
            "interpolated match last line regexp",
            b"if /a#{b}   c/\n  1\nend\n",
        ),
        ("backtick xstring", b"x = `echo   hi`\n"),
        ("interpolated backtick xstring", b"x = `echo   #{a}`\n"),
        ("percent x xstring", b"x = %x(echo   hi)\n"),
        ("single quoted string", b"x = 'a  \\n  b'\n"),
        ("double quoted string", b"x = \"a  \\t  b\"\n"),
        ("string interpolation", b"x = \"a#{b}c\"\n"),
        (
            "adjacent strings over a continuation",
            b"x = \"a\" \\\n  \"b\"\n",
        ),
        ("character literal", b"c = ?a\n"),
        ("plain symbol", b"x = :sym\n"),
        ("quoted symbol", b"x = :\"quoted   sym\"\n"),
        ("percent s symbol", b"x = %s(w)\n"),
        ("interpolated symbol", b"x = :\"a#{b}c\"\n"),
        ("comment inside an interpolation", b"x = \"a#{1 # c\n}b\"\n"),
        (
            "comment inside a heredoc interpolation",
            b"x = <<~A\n  #{1 # c\n  }\nA\n",
        ),
        ("__END__ with data", b"x = 1\n__END__\ndata here\n"),
        ("__END__ without a trailing newline", b"x = 1\n__END__"),
        ("__END__ with nothing after it", b"x = 1\n__END__\n"),
        ("embdoc", b"=begin\nhi\n=end\nx = 1\n"),
        (
            "shebang and magic comment",
            b"#!/usr/bin/env ruby\n# frozen_string_literal: true\nx = 1\n",
        ),
        ("inline comments", b"x = 1 # c1\n# c2\ny = 2\n"),
        ("non-utf8 comment", b"# \xe9\nx = 1\n"),
        (
            "non-utf8 string under a magic comment",
            b"# encoding: binary\nx = \"\xff\xfe\"\n",
        ),
        ("plain array", b"x = [1,   2]\n"),
        ("implicit array", b"y = 1,   2\n"),
        ("no literals at all", b"def f(a, b)\n  a + b\nend\n"),
        ("bare percent string", b"x = %(a   b)\n"),
        ("empty source", b""),
        (
            "everything at once",
            b"#!/usr/bin/env ruby\n\
              # frozen_string_literal: true\n\
              =begin\n\
              doc\n\
              =end\n\
              class A\n\
                WORDS = %w[a   b]\n\
                def run(x) # trailing\n\
                  puts <<~SQL.strip, /re   x/, :\"a#{x}\", `echo   hi`\n\
                    select   *\n\
                  SQL\n\
                end\n\
              end\n\
              __END__\n\
              trailing   data\n",
        ),
        ("crlf line endings", b"x = 1\r\ny = 2\r\n"),
        ("crlf heredoc", b"x = <<~A\r\n  b\r\nA\r\n"),
    ];

    /// A length-preserving gap policy: everything outside a protected span is
    /// upper-cased. Length preservation is what lets the assertions compare
    /// the *same* byte offsets before and after.
    fn upcase(gap: &[u8]) -> Vec<u8> {
        gap.to_ascii_uppercase()
    }

    /// A gap policy that changes length: runs of spaces collapse to one.
    fn collapse_spaces(gap: &[u8]) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        for byte in gap {
            if !(*byte == b' ' && out.last() == Some(&b' ')) {
                out.push(*byte);
            }
        }
        out
    }

    fn spans_of(source: &[u8]) -> Vec<Span> {
        let parsed = parser::parse(source).unwrap();
        protected_spans(&parsed)
    }

    fn rewritten(source: &[u8], policy: impl FnMut(&[u8]) -> Vec<u8>) -> Vec<u8> {
        rewrite(source, &spans_of(source), policy)
    }

    #[test]
    fn every_hazard_survives_the_identity_rewriter_byte_for_byte() {
        for (name, source) in HAZARDS {
            let out = identity(source).unwrap();
            assert_eq!(out, *source, "{name}");
        }
    }

    #[test]
    fn every_hazard_keeps_its_protected_bytes_under_a_gap_policy() {
        for (name, source) in HAZARDS {
            let spans = spans_of(source);
            let out = rewritten(source, upcase);
            // `upcase` preserves length, so the protected spans still index
            // the same bytes in the output.
            assert_eq!(out.len(), source.len(), "{name}");
            for span in &spans {
                assert_eq!(out[span.clone()], source[span.clone()], "{name} {span:?}");
            }
        }
    }

    #[test]
    fn every_hazard_yields_ordered_disjoint_in_bounds_spans() {
        for (name, source) in HAZARDS {
            let spans = spans_of(source);
            let mut previous = 0;
            for span in &spans {
                assert!(span.start >= previous, "{name}: {span:?} out of order");
                assert!(span.start <= span.end, "{name}: {span:?} inverted");
                assert!(span.end <= source.len(), "{name}: {span:?} out of bounds");
                previous = span.end;
            }
        }
    }

    #[test]
    fn a_heredoc_span_runs_from_the_marker_line_end_to_the_terminator() {
        let source = b"x = <<~EOS.strip\n  body\nEOS\n";
        assert_eq!(spans_of(source), vec![4..10, 16..28]);
        // The marker itself, then the body *and* the terminator line.
        assert_eq!(&source[4..10], b"<<~EOS");
        assert_eq!(&source[16..28], b"\n  body\nEOS\n");
    }

    #[test]
    fn the_code_after_a_heredoc_marker_stays_in_a_gap() {
        // `.strip` sits between the marker and the body, so it is rewritable.
        let out = rewritten(b"x = <<~EOS.strip\n  body\nEOS\n", upcase);
        assert_eq!(out, b"X = <<~EOS.STRIP\n  body\nEOS\n");
    }

    #[test]
    fn two_heredocs_on_one_line_merge_into_one_span() {
        let source = b"a = <<~A; b = <<~B\n  one\nA\n  two\nB\n";
        // `<<~A`, then the second marker joined to everything from the end of
        // the marker line through the last terminator.
        assert_eq!(spans_of(source), vec![4..8, 14..35]);
        assert_eq!(&source[14..35], b"<<~B\n  one\nA\n  two\nB\n");
    }

    #[test]
    fn an_empty_heredoc_body_still_protects_the_terminator() {
        // The marker ends where the derived region starts, so the two touch
        // and merge; `y = 2` after the terminator stays a gap.
        let source = b"x = <<~A\nA\ny = 2\n";
        assert_eq!(spans_of(source), vec![4..11]);
        assert_eq!(&source[4..11], b"<<~A\nA\n");
    }

    #[test]
    fn a_comment_after_a_marker_merges_with_the_heredoc_region() {
        // The comment ends where the marker line does, which is where the
        // derived region begins.
        let source = b"x = <<~A # note\n  body\nA\n";
        assert_eq!(spans_of(source), vec![4..8, 9..25]);
        assert_eq!(&source[9..25], b"# note\n  body\nA\n");
    }

    #[test]
    fn a_comment_span_is_exactly_the_comment() {
        let source = b"x = 1 # c1\n# c2\ny = 2\n";
        assert_eq!(spans_of(source), vec![6..10, 11..15]);
        assert_eq!(&source[6..10], b"# c1");
        assert_eq!(&source[11..15], b"# c2");
    }

    #[test]
    fn an_embdoc_span_covers_the_whole_block() {
        let source = b"=begin\nhi\n=end\nx = 1\n";
        assert_eq!(spans_of(source), vec![0..15]);
        assert_eq!(&source[0..15], b"=begin\nhi\n=end\n");
    }

    #[test]
    fn the_data_section_is_protected() {
        let source = b"x = 1\n__END__\ndata here\n";
        assert_eq!(spans_of(source), vec![6..24]);
        assert_eq!(&source[6..24], b"__END__\ndata here\n");
    }

    #[test]
    fn a_word_list_is_protected_whole_so_its_separators_survive() {
        // The elements alone would leave the separating whitespace in a gap,
        // and that whitespace *is* the separator.
        let source = b"x = %w[a   b]\n";
        assert_eq!(spans_of(source), vec![4..13]);
        assert_eq!(rewritten(source, collapse_spaces), b"x = %w[a   b]\n");
    }

    #[test]
    fn a_plain_array_is_not_protected() {
        let source = b"x = [1,   2]\n";
        assert!(spans_of(source).is_empty());
        assert_eq!(rewritten(source, collapse_spaces), b"x = [1, 2]\n");
    }

    #[test]
    fn an_implicit_array_is_not_protected() {
        // No opening delimiter at all, so the `%` test has nothing to read.
        assert!(spans_of(b"y = 1,   2\n").is_empty());
    }

    #[test]
    fn overlapping_spans_merge() {
        // The comment lives inside the string literal's span.
        let source = b"x = \"a#{1 # c\n}b\"\n";
        assert_eq!(spans_of(source), vec![4..17]);
        assert_eq!(&source[4..17], b"\"a#{1 # c\n}b\"");
    }

    #[test]
    fn a_gap_policy_reaches_the_code_around_a_literal() {
        assert_eq!(rewritten(b"x = 'a  b'\n", upcase), b"X = 'a  b'\n");
        assert_eq!(
            rewritten(b"def f\n  x = 'a  b'\nend\n", collapse_spaces),
            b"def f\n x = 'a  b'\nend\n"
        );
    }

    #[test]
    fn keep_returns_the_gap_unchanged() {
        assert_eq!(keep(b"  a  "), b"  a  ");
    }

    #[test]
    fn rewrite_with_no_spans_is_just_the_policy() {
        assert_eq!(rewrite(b"a b", &[], upcase), b"A B");
    }

    #[test]
    fn identity_reports_a_parse_error() {
        let err = identity(b"def ; end").unwrap_err();
        assert!(matches!(err, Error::Parse(_)), "{err}");
    }
}
