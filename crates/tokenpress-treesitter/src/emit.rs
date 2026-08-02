//! The emitter's foundation: which bytes of a source may never be touched, and
//! the rewriter that every emitter policy plugs into.
//!
//! Nothing here knows which language it is looking at. Every byte range this
//! module protects is derived from the node kinds a
//! [`LanguageConfig`](crate::parser::LanguageConfig) was built with, so the
//! same code serves Go, Java, C# and PHP.
//!
//! # The protected-span model
//!
//! tree-sitter is a parser, not a code generator: there is no AST printer to
//! render a canonical form with. The emitter therefore works on the **source
//! bytes**. It splits the file into [`Span`]s, whose bytes are copied out
//! verbatim, and the *gaps* between them, which are the only thing a policy
//! may rewrite. Anything not proven safe stays protected, so the failure mode
//! of a missed construct is a missed saving rather than a corrupted file.
//!
//! A span carries a [`SpanKind`] because the two reasons a byte range is
//! copied verbatim are not the same reason:
//!
//! - [`SpanKind::Protected`] — a string, raw string or character literal,
//!   whose bytes *are* the program. No policy may ever touch them.
//! - [`SpanKind::Comment`] — a comment. Its bytes are copied verbatim by the
//!   whitespace policy, but a comment policy may choose to delete the whole
//!   span instead, which is why the classification survives collection rather
//!   than being flattened away.
//!
//! # Why the sort order is `(start, Reverse(end))`
//!
//! [`collect_spans`] sorts and merges, so overlapping and touching ranges
//! become one — a comment inside a protected region, a literal inside a
//! protected prologue. Merging keeps the kind of the span that *starts* the
//! merged run, so the sort order decides the classification of the result, and
//! getting it wrong is silent.
//!
//! Spans that come from a tree are either disjoint or properly nested, so when
//! two of them share a start byte, one **contains** the other and the
//! containing one is the one with the greater end. Sorting by
//! `(start, Reverse(end))` puts that outer span first, and the merged run
//! inherits its kind — which is what makes a protected region that begins with
//! a comment (a Go build-constraint prologue: `//go:build …` at byte 0 inside
//! a region that must be copied verbatim) come out as
//! [`SpanKind::Protected`].
//!
//! Sorting by `(start, end)` instead inverts exactly that case: the nested
//! comment sorts first, the merged run is classified [`SpanKind::Comment`],
//! and a comment policy then deletes the whole outer region — its literals and
//! its code along with it. That is not a hypothetical; it is the bug the
//! prototype hit, and it is pinned by a test in this module.
//!
//! The tie-break between two spans with the *same* range is
//! [`SpanKind::Protected`] first, so a kind named in both of a config's lists
//! ends up protected rather than deletable. Protection is the safe direction.
//!
//! # Policy stages
//!
//! [`rewrite`] takes the gap policy as a parameter. [`keep`] is the identity
//! one, so [`identity`] reproduces its input byte for byte — the whole
//! pipeline with the policy seam left empty. The whitespace policy and the
//! comment policy plug into that seam later; leaving it empty here is what
//! makes span collection testable on its own, with no policy to attribute a
//! difference to.

use std::cmp::Reverse;
use std::ops::Range;

use tokenpress_core::Result;

use crate::parser::{self, LanguageConfig, Node, Tree};

/// Why a byte range is reproduced verbatim.
///
/// The declaration order is load-bearing: it is the tie-break between two
/// spans covering the same range, and `Protected` comes first so protection
/// wins. See the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SpanKind {
    /// Bytes that are the program: a literal whose content no policy may
    /// touch.
    Protected,
    /// A comment. Copied verbatim by a whitespace policy, and the only kind a
    /// comment policy is allowed to delete.
    Comment,
}

/// A byte range of the source that has to be reproduced verbatim, and why.
///
/// `Copy` and free of any `Drop` glue: a span is two offsets and a tag, so it
/// is passed and stored by value everywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// First byte of the range.
    pub start: usize,
    /// One past the last byte of the range.
    pub end: usize,
    /// What the range is.
    pub kind: SpanKind,
}

impl Span {
    /// A span over `start..end`.
    pub fn new(start: usize, end: usize, kind: SpanKind) -> Self {
        Self { start, end, kind }
    }

    /// The range, for slicing the source it was collected from.
    pub fn range(&self) -> Range<usize> {
        self.start..self.end
    }
}

/// Collects every byte range of `tree` that must survive verbatim: the nodes
/// whose kind is in the config's protected list, and the nodes whose kind is
/// in its comment list.
///
/// The result is sorted, non-overlapping and within the source — the
/// precondition [`rewrite`] is documented to need. Ranges that overlap or
/// touch are merged, keeping the kind of the outermost one; see the module
/// docs for why that ordering is the load-bearing part.
pub fn collect_spans(config: &LanguageConfig, tree: &Tree) -> Vec<Span> {
    let mut spans = Vec::new();
    push_spans(config, tree.root_node(), &mut spans);
    merge(spans)
}

/// Rebuilds `source`, copying `spans` verbatim and passing every gap between
/// them — the whole of the rest of the file — through `gap_policy`.
///
/// `spans` must be sorted, non-overlapping and within `source`, which is what
/// [`collect_spans`] returns; that is the module's only precondition and it is
/// pinned by a property test over both collected and generated span sets.
pub fn rewrite(
    source: &[u8],
    spans: &[Span],
    mut gap_policy: impl FnMut(&[u8]) -> Vec<u8>,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(source.len());
    let mut cursor = 0;
    for span in spans {
        out.extend_from_slice(&gap_policy(&source[cursor..span.start]));
        out.extend_from_slice(&source[span.range()]);
        cursor = span.end;
    }
    out.extend_from_slice(&gap_policy(&source[cursor..]));
    out
}

/// The identity gap policy: the bytes between spans pass through unchanged.
pub fn keep(gap: &[u8]) -> Vec<u8> {
    gap.to_vec()
}

/// Parses `source`, collects its spans and rewrites it with [`keep`],
/// reproducing the input byte for byte.
///
/// This is the whole pipeline with the policy seam left empty: it exists so
/// span collection is exercised end to end, and it stays an identity once the
/// policy stages land.
///
/// Returns [`tokenpress_core::Error::Parse`] for a source the configured
/// grammar reports errors for.
pub fn identity(config: &LanguageConfig, source: &[u8]) -> Result<Vec<u8>> {
    let tree = parser::parse(config, source)?;
    let spans = collect_spans(config, &tree);
    Ok(rewrite(source, &spans, keep))
}

/// Records `node`'s span when its kind is configured, then descends.
///
/// The walk never stops early: a protected node's children are visited like
/// any others, and whatever they contribute is absorbed by the merge. One code
/// path, no nesting special case.
fn push_spans(config: &LanguageConfig, node: Node, spans: &mut Vec<Span>) {
    if let Some(kind) = classify(config, node.kind()) {
        spans.push(Span::new(node.start_byte(), node.end_byte(), kind));
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        push_spans(config, child, spans);
    }
}

/// What the config says about a node kind, if anything.
///
/// A kind named in both lists is protected: a policy that cannot delete a
/// comment only loses a saving, while one that deletes a literal loses the
/// program.
fn classify(config: &LanguageConfig, kind: &str) -> Option<SpanKind> {
    if config.protected_kinds().contains(&kind) {
        Some(SpanKind::Protected)
    } else if config.comment_kinds().contains(&kind) {
        Some(SpanKind::Comment)
    } else {
        None
    }
}

/// Sorts `spans` and merges every pair that overlaps or touches, keeping the
/// kind of the span that starts the merged run.
///
/// See the module docs for why the sort key is `(start, Reverse(end), kind)`.
fn merge(mut spans: Vec<Span>) -> Vec<Span> {
    spans.sort_by_key(|span| (span.start, Reverse(span.end), span.kind));
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
    use crate::parser::Language;
    use tokenpress_core::Error;

    /// The dev-dependency grammar, converted from its `LanguageFn`.
    fn go() -> Language {
        tree_sitter_go::LANGUAGE.into()
    }

    /// The configuration the first consumer ships.
    fn go_config() -> LanguageConfig {
        LanguageConfig::new(
            go(),
            vec!["comment"],
            vec![
                "interpreted_string_literal",
                "raw_string_literal",
                "rune_literal",
            ],
            true,
        )
        .unwrap()
    }

    /// Sources whose bytes the identity rewriter has to reproduce exactly.
    /// Byte strings rather than `&str`, because a source is a byte sequence.
    const CORPUS: &[(&str, &[u8])] = &[
        ("empty source", b""),
        ("package clause only", b"package main\n"),
        (
            "no literals at all",
            b"package main\n\nfunc f(a int) int { return a + 1 }\n",
        ),
        (
            "interpreted string",
            b"package main\n\nfunc f() { g(\"a  b\") }\n",
        ),
        (
            "raw string over several lines",
            b"package main\n\nvar s = `a\n\n  b`\n",
        ),
        ("rune literal", b"package main\n\nvar r = 'q'\n"),
        ("line comment", b"package main\n\n// note\nfunc f() {}\n"),
        (
            "block comment",
            b"package main\n\n/* note\n   more */\nfunc f() {}\n",
        ),
        (
            "comment text inside a string",
            b"package main\n\nvar s = \"// not a comment\"\n",
        ),
        (
            "string text inside a comment",
            b"package main\n\n// var s = \"not a string\"\nfunc f() {}\n",
        ),
        (
            "modern build constraint",
            b"//go:build ignore\n\npackage main\n",
        ),
        (
            "legacy build constraint",
            b"// +build ignore\n\npackage main\n",
        ),
        (
            "cgo prologue",
            b"package main\n\n/*\n#include <stdio.h>\n*/\nimport \"C\"\n",
        ),
        (
            "struct tag",
            b"package main\n\ntype T struct {\n\tA int `json:\"a\"`\n}\n",
        ),
        ("crlf line endings", b"package main\r\n\r\nfunc f() {}\r\n"),
        (
            "non-ascii identifier and string",
            "package main\n\nvar \u{307B} = \"\u{3042}  \u{3044}\"\n".as_bytes(),
        ),
        (
            "directive comment",
            b"package main\n\n//go:generate echo hi\nfunc f() {}\n",
        ),
        (
            "trailing comment with no final newline",
            b"package main\n\nfunc f() {} // note",
        ),
        (
            "everything at once",
            b"//go:build ignore\n\
              \n\
              // package doc\n\
              package main\n\
              \n\
              import \"C\"\n\
              \n\
              /* block */\n\
              func f() {\n\
              \tvar s = `raw\n\n  body`\n\
              \tg(s, \"a  b\", 'q') // trailing\n\
              }\n",
        ),
    ];

    /// A length-preserving gap policy, so the assertions can compare the same
    /// byte offsets before and after.
    fn upcase(gap: &[u8]) -> Vec<u8> {
        gap.to_ascii_uppercase()
    }

    fn spans_of(config: &LanguageConfig, source: &[u8]) -> Vec<Span> {
        let tree = parser::parse(config, source).unwrap();
        collect_spans(config, &tree)
    }

    /// The precondition [`rewrite`] documents.
    fn assert_sorted_disjoint_in_bounds(spans: &[Span], len: usize, name: &str) {
        let mut previous = 0;
        for span in spans {
            assert!(span.start >= previous, "{name}: {span:?} out of order");
            assert!(span.start <= span.end, "{name}: {span:?} inverted");
            assert!(span.end <= len, "{name}: {span:?} out of bounds");
            previous = span.end;
        }
    }

    /// A deterministic generator, so a failing case is reproducible and the
    /// suite never flakes. Numerical Recipes' LCG constants; the high bits are
    /// the usable ones.
    struct Rng(u64);

    impl Rng {
        fn new(seed: u64) -> Self {
            Self(seed)
        }

        fn next_u64(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0 >> 33
        }

        /// A value in `0..bound`.
        fn below(&mut self, bound: usize) -> usize {
            (self.next_u64() % bound as u64) as usize
        }
    }

    /// Statement-shaped fragments a generated source is assembled from. Every
    /// one of them is a complete statement or an extra, so any sequence of
    /// them parses.
    const FRAGMENTS: &[&str] = &[
        "x := 1",
        "y := \"a  b\"",
        "z := `raw\n\n  string`",
        "r := 'q'",
        "// line comment",
        "/* block\n   comment */",
        "g(1, 2)",
        "s := \"// not a comment\"",
        "_ = x",
    ];

    /// Separators, all of which contain a newline: Go inserts semicolons at
    /// line ends, so two statements may not share a line.
    const SEPARATORS: &[&str] = &["\n", "\n\n", "\n\t", "\n    ", "\n\t\t\n", "\n\r\n"];

    fn generated_source(rng: &mut Rng) -> Vec<u8> {
        let mut source = String::from("package main\n\nfunc f() {");
        for _ in 0..=rng.below(8) {
            source.push_str(SEPARATORS[rng.below(SEPARATORS.len())]);
            source.push_str(FRAGMENTS[rng.below(FRAGMENTS.len())]);
        }
        source.push_str("\n}\n");
        source.into_bytes()
    }

    fn generated_bytes(rng: &mut Rng, len: usize) -> Vec<u8> {
        (0..len).map(|_| rng.below(256) as u8).collect()
    }

    /// A span set satisfying [`rewrite`]'s precondition by construction: each
    /// span starts at or after the cursor, is non-empty, ends within `len`,
    /// and leaves at least one byte before the next one.
    fn generated_spans(rng: &mut Rng, len: usize) -> Vec<Span> {
        let mut spans = Vec::new();
        let mut cursor = 0;
        while cursor < len {
            let start = (cursor + rng.below(4)).min(len - 1);
            let end = (start + 1 + rng.below(5)).min(len);
            let kind = if rng.below(2) == 0 {
                SpanKind::Protected
            } else {
                SpanKind::Comment
            };
            spans.push(Span::new(start, end, kind));
            cursor = end + 1;
        }
        spans
    }

    #[test]
    fn a_span_is_a_range_and_a_reason() {
        let span = Span::new(2, 5, SpanKind::Comment);
        assert_eq!(span.range(), 2..5);
        assert_eq!(span.kind, SpanKind::Comment);
        // `Copy`, so the original is still usable after the move.
        let copy = span;
        assert_eq!(copy, span);
        assert_eq!(
            format!("{span:?}"),
            "Span { start: 2, end: 5, kind: Comment }"
        );
    }

    #[test]
    fn collection_classifies_literals_and_comments() {
        let config = go_config();
        let source = b"package main\n\nfunc f() {\n\tg(\"a  b\") // note\n}\n";
        let spans = spans_of(&config, source);
        assert_eq!(spans.len(), 2, "{spans:?}");
        assert_eq!(spans[0].kind, SpanKind::Protected);
        assert_eq!(&source[spans[0].range()], b"\"a  b\"");
        assert_eq!(spans[1].kind, SpanKind::Comment);
        assert_eq!(&source[spans[1].range()], b"// note");
    }

    #[test]
    fn collection_ignores_every_other_kind() {
        let config = go_config();
        assert!(spans_of(&config, b"package main\n\nvar x = 1\n").is_empty());
    }

    #[test]
    fn collection_follows_the_config_not_the_language() {
        // Nothing is configured, so nothing is protected — the engine has no
        // opinion of its own about Go.
        let config = LanguageConfig::new(go(), vec![], vec![], true).unwrap();
        assert!(spans_of(&config, b"package main\n\n// note\nvar s = \"a\"\n").is_empty());
    }

    #[test]
    fn a_kind_in_both_lists_is_protected() {
        let config = LanguageConfig::new(go(), vec!["comment"], vec!["comment"], true).unwrap();
        let spans = spans_of(&config, b"package main\n\n// note\n");
        assert_eq!(spans.len(), 1, "{spans:?}");
        assert_eq!(spans[0].kind, SpanKind::Protected);
    }

    #[test]
    fn an_outer_span_wins_over_a_comment_nested_inside_it() {
        // The ordering bug, pinned. A protected region that *begins* with a
        // comment — a Go build-constraint prologue is exactly this shape — is
        // modelled here by protecting the whole file. Both spans start at byte
        // 0, so only the end decides which sorts first: under
        // `(start, Reverse(end))` the outer one does and the merged run is
        // Protected, under `(start, end)` the comment does and the merged run
        // is classified Comment, which hands the whole region to the comment
        // policy to delete.
        let config = LanguageConfig::new(go(), vec!["comment"], vec!["source_file"], true).unwrap();
        let source = b"//go:build ignore\n\npackage main\n";
        let spans = spans_of(&config, source);
        assert_eq!(spans.len(), 1, "{spans:?}");
        assert_eq!(spans[0].range(), 0..source.len());
        assert_eq!(
            spans[0].kind,
            SpanKind::Protected,
            "the outer span has to decide the kind"
        );
    }

    #[test]
    fn merging_keeps_the_outermost_kind_whatever_the_input_order() {
        // The same inversion at the unit level, with the nested span offered
        // first so insertion order cannot be what saves it.
        let merged = merge(vec![
            Span::new(0, 17, SpanKind::Comment),
            Span::new(0, 32, SpanKind::Protected),
        ]);
        assert_eq!(merged, vec![Span::new(0, 32, SpanKind::Protected)]);
    }

    #[test]
    fn merging_breaks_a_tie_towards_protection() {
        // Two spans over the same range: the kind is the tie-break, and
        // `Protected` sorts first.
        let merged = merge(vec![
            Span::new(3, 9, SpanKind::Comment),
            Span::new(3, 9, SpanKind::Protected),
        ]);
        assert_eq!(merged, vec![Span::new(3, 9, SpanKind::Protected)]);
    }

    #[test]
    fn merging_joins_overlapping_and_touching_spans_and_leaves_disjoint_ones_alone() {
        let merged = merge(vec![
            Span::new(10, 14, SpanKind::Comment),
            Span::new(0, 4, SpanKind::Protected),
            Span::new(2, 6, SpanKind::Comment),
            Span::new(6, 8, SpanKind::Protected),
        ]);
        assert_eq!(
            merged,
            vec![
                Span::new(0, 8, SpanKind::Protected),
                Span::new(10, 14, SpanKind::Comment),
            ]
        );
    }

    #[test]
    fn a_comment_inside_a_protected_literal_is_absorbed() {
        // Not a comment at all — bytes of a raw string that look like one.
        let config = go_config();
        let source = b"package main\n\nvar s = `a // b\nc`\n";
        let spans = spans_of(&config, source);
        assert_eq!(spans.len(), 1, "{spans:?}");
        assert_eq!(spans[0].kind, SpanKind::Protected);
        assert_eq!(&source[spans[0].range()], b"`a // b\nc`");
    }

    #[test]
    fn every_corpus_source_survives_the_identity_rewriter_byte_for_byte() {
        let config = go_config();
        for (name, source) in CORPUS {
            assert_eq!(identity(&config, source).unwrap(), *source, "{name}");
        }
    }

    #[test]
    fn every_corpus_source_yields_spans_that_satisfy_the_precondition() {
        let config = go_config();
        for (name, source) in CORPUS {
            assert_sorted_disjoint_in_bounds(&spans_of(&config, source), source.len(), name);
        }
    }

    #[test]
    fn every_corpus_source_keeps_its_protected_bytes_under_a_gap_policy() {
        let config = go_config();
        for (name, source) in CORPUS {
            let spans = spans_of(&config, source);
            let out = rewrite(source, &spans, upcase);
            // `upcase` preserves length, so the spans still index the same
            // bytes in the output.
            assert_eq!(out.len(), source.len(), "{name}");
            for span in &spans {
                assert_eq!(out[span.range()], source[span.range()], "{name} {span:?}");
            }
        }
    }

    #[test]
    fn generated_sources_survive_the_identity_rewriter_and_satisfy_the_precondition() {
        let config = go_config();
        let mut rng = Rng::new(0x5eed_0001);
        for case in 0..256 {
            let source = generated_source(&mut rng);
            let name = format!("generated {case}");
            assert_sorted_disjoint_in_bounds(&spans_of(&config, &source), source.len(), &name);
            let out = identity(&config, &source).unwrap();
            assert_eq!(out, source, "{name}: {}", String::from_utf8_lossy(&source));
        }
    }

    #[test]
    fn the_identity_policy_reproduces_any_bytes_under_any_valid_span_set() {
        // The property in its strongest form: arbitrary bytes, arbitrary spans
        // satisfying the precondition, no grammar involved at all.
        let mut rng = Rng::new(0x5eed_0002);
        for case in 0..256 {
            let len = rng.below(64);
            let source = generated_bytes(&mut rng, len);
            let spans = generated_spans(&mut rng, len);
            assert_sorted_disjoint_in_bounds(&spans, len, "generated");
            assert_eq!(rewrite(&source, &spans, keep), source, "case {case}");
        }
    }

    #[test]
    fn rewrite_hands_every_gap_and_only_the_gaps_to_the_policy() {
        let config = go_config();
        let source = b"package main\n\nfunc f() {\n\tg(\"a  b\") // note\n}\n";
        assert_eq!(
            rewrite(source, &spans_of(&config, source), upcase),
            b"PACKAGE MAIN\n\nFUNC F() {\n\tG(\"a  b\") // note\n}\n"
        );
    }

    #[test]
    fn rewrite_with_no_spans_is_just_the_policy() {
        assert_eq!(rewrite(b"a b", &[], upcase), b"A B");
    }

    #[test]
    fn rewrite_calls_the_policy_once_per_gap_including_the_empty_ones() {
        // A span at byte 0 and a span at the end still leave an empty gap on
        // each side, so a stateful policy sees a call for them.
        let mut gaps: Vec<Vec<u8>> = Vec::new();
        let out = rewrite(
            b"ab c",
            &[
                Span::new(0, 1, SpanKind::Protected),
                Span::new(1, 2, SpanKind::Comment),
            ],
            |gap| {
                gaps.push(gap.to_vec());
                gap.to_vec()
            },
        );
        assert_eq!(out, b"ab c");
        assert_eq!(gaps, [b"".to_vec(), b"".to_vec(), b" c".to_vec()]);
    }

    #[test]
    fn keep_returns_the_gap_unchanged() {
        assert_eq!(keep(b"  a  "), b"  a  ");
    }

    #[test]
    fn identity_reports_a_parse_error() {
        let err = identity(&go_config(), b"package main\n\nfunc f( {\n").unwrap_err();
        assert!(matches!(err, Error::Parse(_)), "{err}");
    }
}
