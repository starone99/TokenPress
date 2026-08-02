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
//! [`rewrite`] takes the policy as a parameter. [`keep`] is the identity one,
//! so [`identity`] reproduces its input byte for byte; [`minimize`] is the
//! whitespace policy, so [`minimize_source`] is the comments-kept
//! configuration. The `strip_comments` policy lands in a later sub-task as a
//! change to span collection, not to either policy.
//!
//! # The whitespace policy
//!
//! [`minimize`] removes the formatting whitespace of a gap and nothing else:
//!
//! - a horizontal-whitespace run (spaces and tabs) that follows a newline
//!   *this call has emitted* is indentation, and vanishes;
//! - a horizontal-whitespace run that precedes a newline is trailing
//!   whitespace, and vanishes;
//! - every other horizontal-whitespace run collapses to **exactly one
//!   space** — never to zero. That single space is the load-bearing safety
//!   choice of the whole policy: `a - b` is a subtraction while `a -b` is a
//!   call with a unary-minus argument, and `defined? x` and `not x` need
//!   their separator to stay a separator;
//! - a newline that would open a blank line is dropped, so a run of blank
//!   lines collapses to one newline. Every other newline survives, because in
//!   Ruby a newline is a statement terminator.
//!
//! Nothing here joins two non-blank lines, and nothing here deletes the last
//! newline before a protected span — the first newline of a run is always the
//! one kept — so the constructs that must start a line (`=begin`, `__END__`,
//! a heredoc terminator) keep the newline that puts them there.
//!
//! ## One gap at a time, no state between gaps
//!
//! A policy sees a gap, never the protected bytes around it, so it cannot
//! know what the previous protected span ended with. [`minimize`] is
//! therefore stateless between calls, and treats the start of a gap as
//! **mid-line**: leading whitespace there collapses to one space instead of
//! vanishing. That is the conservative direction. A line may start in one
//! gap, run through a protected string and resume in the next — `x = "a" +\n
//! "b"` — and the second gap's leading bytes are then not indentation at all.
//! The cost is one surviving space at the start of the file and after each
//! protected span that ends with a newline (a heredoc region, an embdoc);
//! the benefit is that no gap can be misread as line-leading.
//!
//! ## `\r` and `\`
//!
//! A `\r\n` is treated as one line terminator and reproduced as `\r\n`, so a
//! CRLF file stays a CRLF file; a `\r` that does not end a line is an
//! ordinary byte and is copied. (A CRLF heredoc leaves exactly that: the
//! marker line's `\r` sits in the gap, because the protected region starts at
//! the `\n`.) Rewriting CRLF to LF would save a byte per line, but it edits
//! the source slices that `crate::comparable` keeps for multi-line location
//! fields, so it is left alone.
//!
//! A `\` and the byte after it are copied verbatim as one unit. In a gap a
//! backslash can only be a line continuation, and `\` + `\n` has to stay
//! adjacent: stripping "trailing whitespace" around it, or deleting the
//! newline it holds, would join or split logical lines. The physical line
//! after a continuation is a continuation of the same logical line, so it is
//! mid-line and its leading run collapses to a space rather than vanishing.
//!
//! ## The one over-refusal this policy triggers
//!
//! Reformatting the inside of an index call — `a[1,\n  2]` — moves the source
//! slice `crate::comparable` keeps for its `message_loc`, which spans the
//! whole bracket pair. That is the known over-refusal class already pinned in
//! `comparable.rs`, and the verifier's answer is to leave the file alone; it
//! costs a saving, never correctness. Measured over the 1650 `.rb` files
//! shipped with ruby 3.3.6 it is the *only* source of disagreement, and it
//! hits 6 of them.

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

/// The whitespace-minimizing gap policy: indentation and trailing whitespace
/// go, blank-line runs collapse to one newline, every other
/// horizontal-whitespace run collapses to one space.
///
/// See the module docs for the rules and for why the start of a gap counts as
/// mid-line.
pub fn minimize(gap: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(gap.len());
    // Set only by a newline *this call* has emitted. A gap starts mid-line as
    // far as the policy can tell, so it starts `false`.
    let mut line_start = false;
    let mut index = 0;

    while index < gap.len() {
        let rest = &gap[index..];
        if rest[0] == b'\\' {
            // A continuation: the backslash and whatever it holds move
            // together, and the logical line carries on past them.
            let unit = rest.len().min(2);
            out.extend_from_slice(&rest[..unit]);
            index += unit;
            line_start = false;
        } else if let Some(width) = newline_len(rest) {
            // The first newline of a run terminates a statement; the ones
            // after it only open blank lines.
            if !line_start {
                out.extend_from_slice(&rest[..width]);
                line_start = true;
            }
            index += width;
        } else if is_horizontal(rest[0]) {
            index += rest.iter().take_while(|byte| is_horizontal(**byte)).count();
            // Indentation and trailing whitespace vanish. Anything else is a
            // separator, and a separator is one space.
            if !line_start && newline_len(&gap[index..]).is_none() {
                out.push(b' ');
            }
        } else {
            out.push(rest[0]);
            index += 1;
            line_start = false;
        }
    }
    out
}

/// Parses `source`, collects its protected spans and rewrites it with
/// [`minimize`]: the whitespace-minimal emitter, comments kept.
///
/// Returns [`tokenpress_core::Error::Parse`] for a source prism reports
/// errors for.
pub fn minimize_source(source: &[u8]) -> Result<Vec<u8>> {
    let parsed = parser::parse(source)?;
    let spans = protected_spans(&parsed);
    Ok(rewrite(source, &spans, minimize))
}

/// The whitespace a run is made of. Only spaces and tabs: every other byte
/// Ruby happens to accept as whitespace is rare enough that leaving it alone
/// costs nothing and guessing about it could cost correctness.
fn is_horizontal(byte: u8) -> bool {
    byte == b' ' || byte == b'\t'
}

/// Width of the line terminator starting `bytes`, or `None` when `bytes` does
/// not start with one. A lone `\r` is not a terminator here — see the module
/// docs.
fn newline_len(bytes: &[u8]) -> Option<usize> {
    match bytes {
        [b'\n', ..] => Some(1),
        [b'\r', b'\n', ..] => Some(2),
        _ => None,
    }
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

    #[test]
    fn minimize_strips_line_indentation() {
        assert_eq!(minimize(b"a\n    b\n\tc\n"), b"a\nb\nc\n");
    }

    #[test]
    fn minimize_strips_trailing_whitespace() {
        assert_eq!(minimize(b"a   \nb\t\n"), b"a\nb\n");
    }

    #[test]
    fn minimize_strips_a_whitespace_run_that_ends_the_gap_after_a_newline() {
        // Indentation with nothing after it is still indentation.
        assert_eq!(minimize(b"a\n   "), b"a\n");
    }

    #[test]
    fn minimize_collapses_blank_line_runs_to_one_newline() {
        assert_eq!(minimize(b"a\n\n\n\nb\n"), b"a\nb\n");
        // A line holding only whitespace is blank too.
        assert_eq!(minimize(b"a\n  \n\t\nb\n"), b"a\nb\n");
        // A gap that opens with blank lines keeps the first newline: the
        // policy cannot see what came before it.
        assert_eq!(minimize(b"\n\n\na\n"), b"\na\n");
    }

    #[test]
    fn minimize_keeps_the_newline_that_ends_a_statement() {
        // Newlines are statement terminators; only *blank* lines go.
        assert_eq!(minimize(b"a = 1\nb = 2\n"), b"a = 1\nb = 2\n");
    }

    #[test]
    fn minimize_collapses_intra_line_whitespace_to_one_space() {
        assert_eq!(minimize(b"a   =\t\t1\n"), b"a = 1\n");
    }

    #[test]
    fn minimize_never_collapses_a_space_run_to_nothing() {
        // The load-bearing choice: one space, never zero. `a -b` is a call
        // with a unary-minus argument, `a - b` a subtraction.
        assert_eq!(minimize(b"a  -  b\n"), b"a - b\n");
        assert_eq!(minimize(b"defined?  x\n"), b"defined? x\n");
        assert_eq!(minimize(b"not  x\n"), b"not x\n");
        // A run that ends the gap is mid-line as far as the policy knows, so
        // it also keeps its space — the next bytes may be a protected span.
        assert_eq!(minimize(b"p   "), b"p ");
    }

    #[test]
    fn minimize_treats_the_start_of_a_gap_as_mid_line() {
        // Stateless per gap: without a newline of its own to key on, leading
        // whitespace is an intra-line run, not indentation.
        assert_eq!(minimize(b"    a\n"), b" a\n");
    }

    #[test]
    fn minimize_keeps_a_backslash_and_the_byte_after_it_verbatim() {
        // `\` + `\n` is one indivisible line continuation, and the physical
        // line after it is mid-logical-line, so its leading run collapses to
        // a space rather than vanishing.
        assert_eq!(minimize(b"a + \\\n  b\n"), b"a + \\\n b\n");
        // A backslash that ends the gap has nothing to protect.
        assert_eq!(minimize(b"a \\"), b"a \\");
    }

    #[test]
    fn minimize_preserves_crlf_line_endings() {
        assert_eq!(minimize(b"a = 1\r\nb = 2\r\n"), b"a = 1\r\nb = 2\r\n");
        assert_eq!(minimize(b"a   \r\n   b\r\n"), b"a\r\nb\r\n");
        assert_eq!(minimize(b"a\r\n\r\n\r\nb\r\n"), b"a\r\nb\r\n");
    }

    #[test]
    fn minimize_copies_a_carriage_return_that_ends_no_line() {
        // The byte a CRLF heredoc leaves in the gap before its body.
        assert_eq!(minimize(b"x = <<~A\r"), b"x = <<~A\r");
    }

    #[test]
    fn minimize_of_an_empty_gap_is_empty() {
        assert_eq!(minimize(b""), b"");
    }

    #[test]
    fn minimize_source_rewrites_only_the_gaps() {
        assert_eq!(
            minimize_source(b"def f(a,   b)\n    a  +  b\nend\n").unwrap(),
            b"def f(a, b)\na + b\nend\n"
        );
    }

    #[test]
    fn minimize_source_keeps_word_list_separators_but_not_array_spacing() {
        // The stage-1 seam, now under the real policy.
        assert_eq!(
            minimize_source(b"x = %w[a   b]\n").unwrap(),
            b"x = %w[a   b]\n"
        );
        assert_eq!(minimize_source(b"x = [1,   2]\n").unwrap(), b"x = [1, 2]\n");
    }

    #[test]
    fn minimize_source_leaves_a_heredoc_body_alone_while_minimizing_its_marker_line() {
        assert_eq!(
            minimize_source(b"x   =   <<~EOS.strip\n  body\nEOS\n").unwrap(),
            b"x = <<~EOS.strip\n  body\nEOS\n"
        );
    }

    #[test]
    fn minimize_source_keeps_comments_and_the_newline_before_a_column_zero_construct() {
        // `=begin` and `__END__` must stay at column 0, so the newline that
        // puts them there is the one blank-run collapsing keeps. The blank
        // run after the embdoc collapses to two newlines rather than one:
        // the embdoc span ends *with* a newline, which the following gap
        // cannot see, so its own first newline still counts as a terminator.
        assert_eq!(
            minimize_source(b"x = 1   # c\n\n\n=begin\nd\n=end\n\n\n__END__\ndata\n").unwrap(),
            b"x = 1 # c\n=begin\nd\n=end\n\n__END__\ndata\n"
        );
    }

    #[test]
    fn minimize_source_reports_a_parse_error() {
        let err = minimize_source(b"def ; end").unwrap_err();
        assert!(matches!(err, Error::Parse(_)), "{err}");
    }

    #[test]
    fn every_hazard_stays_equivalent_under_minimization() {
        for (name, source) in HAZARDS {
            let out = minimize_source(source).unwrap();
            let equivalent = crate::comparable::equivalent(source, &out).unwrap();
            assert!(equivalent, "{name}: {}", String::from_utf8_lossy(&out));
        }
    }
}
