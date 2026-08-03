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
//! pipeline with the policy seam left empty, which is what makes span
//! collection testable on its own, with no policy to attribute a difference
//! to. [`minimize`] is the whitespace policy, so [`minimize_source`] is the
//! comments-kept configuration. The comment policy plugs into the same seam
//! later.
//!
//! # The whitespace policy
//!
//! [`minimize`] rewrites the gaps and nothing else, by one rule per
//! whitespace run:
//!
//! - a run containing a `\n` collapses to exactly one `\n`;
//! - any other run collapses to exactly one space — never to zero, because
//!   `a - b` and `a -b` are not the same program in every language and the
//!   engine does not know which language it is looking at;
//! - the **leading** run of the file is dropped entirely;
//! - a byte that is not whitespace is copied verbatim, so a BOM (or anything
//!   else a grammar leaves outside its nodes) survives untouched.
//!
//! Whitespace is the ASCII set, so a `\r` and the `\n` after it are one run
//! and CRLF normalises to LF. Indentation and trailing whitespace are not
//! special cases: they are parts of a run that already contains the newline
//! they sit against, and the run's one byte is that newline.
//!
//! For a `newline_sensitive` config the policy therefore **never joins two
//! lines and never introduces a line** — every emitted `\n` comes from a run
//! that already held one, and every run that held one still emits one. Go's
//! automatic semicolon insertion is preserved by construction rather than by
//! case analysis, and the property is pinned by an equivalence test over the
//! Go corpus and over generated sources.
//!
//! When `newline_sensitive` is `false` (the Java/C#/PHP shape) a newline is
//! formatting like any other whitespace and every run collapses to a space.
//! That branch is only sound for a language whose end-of-line comments are
//! handled by the comment policy — a `//` comment whose newline became a
//! space swallows the line after it — which is the per-language crate's
//! decision to make, not this module's.
//!
//! The policy is stateful across gaps, because "the leading run of the
//! **file**" is not something a single gap can recognise. It is built per
//! rewrite by [`minimize`] rather than being a free function like [`keep`],
//! and the state is one flag: only the closure's first call can see byte 0,
//! since every later one is preceded by a gap or by a span.
//!
//! # The column-0 pinning hook
//!
//! Collapsing indentation moves code left, and in some languages column 0 is
//! meaningful: Go reads `//line` and `//go:generate` as directives only when
//! the comment starts a line, so an *indented* directive-shaped comment that
//! gets promoted to column 0 changes the program — silently, because the
//! equivalence artifact ignores comments by construction.
//!
//! [`rewrite_pinned`] is the generic guard: the caller supplies a predicate
//! over spans, and when a span it answers `true` for would be emitted right
//! after a `\n`, one space goes out first. The rewriter therefore never
//! promotes such a span, whatever the gap policy did — the condition is read
//! off the bytes already emitted, not off the policy. [`rewrite`] is the same
//! function with a predicate that never fires.
//!
//! Which spans those are is language knowledge, so this crate ships no
//! predicate; the Go one arrives with the Go backend. Two edges belong to
//! that predicate rather than to the hook:
//!
//! - a span at byte 0 is never pinned, because nothing has been emitted for
//!   it to follow. It was already at column 0 in the source, so there is no
//!   promotion to undo — and prepending a byte there would corrupt a byte-0
//!   build-constraint prologue;
//! - a span that reaches output column 0 because the file's leading run was
//!   dropped is likewise not pinned, for the same reason: there is no
//!   emitted `\n` in front of it. A language whose prologue is
//!   position-sensitive protects it as a span instead.

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
pub fn rewrite(source: &[u8], spans: &[Span], gap_policy: impl FnMut(&[u8]) -> Vec<u8>) -> Vec<u8> {
    rewrite_pinned(source, spans, gap_policy, |_| false)
}

/// [`rewrite`] plus the column-0 pinning hook: a span `never_starts_a_line`
/// answers `true` for is never emitted at the start of a line.
///
/// Everything [`rewrite`] documents holds here — same precondition, same
/// verbatim spans, same one call per gap. The only addition is that when the
/// bytes emitted so far end in a `\n`, the span about to be copied would start
/// at column 0, and the predicate answers `true` for it, one space goes out
/// first. See the module docs for what the hook is for and where its edges
/// are.
pub fn rewrite_pinned(
    source: &[u8],
    spans: &[Span],
    mut gap_policy: impl FnMut(&[u8]) -> Vec<u8>,
    mut never_starts_a_line: impl FnMut(Span) -> bool,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(source.len());
    let mut cursor = 0;
    for span in spans {
        out.extend_from_slice(&gap_policy(&source[cursor..span.start]));
        // The span is about to land at column 0 exactly when the last byte
        // emitted is a newline. Asking the predicate only then keeps it a
        // question about *promotion* rather than about the span itself.
        if out.last() == Some(&b'\n') && never_starts_a_line(*span) {
            out.push(b' ');
        }
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

/// The whitespace policy: builds the gap policy that reduces every whitespace
/// run of a file to the single byte that carries its meaning.
///
/// Stateful across gaps by construction — it has to be, because "the leading
/// whitespace run of the *file*" is not something a single gap can recognise —
/// so it is a closure built per rewrite, not a free function like [`keep`].
/// See the module docs for the rules and for why they are safe.
pub fn minimize(config: &LanguageConfig) -> impl FnMut(&[u8]) -> Vec<u8> {
    let newline_sensitive = config.newline_sensitive();
    // The file's leading run is the run at byte 0, so it can only ever be the
    // one this closure sees first: every later call is preceded by a gap or by
    // a span, and after either of those nothing is at the start of the file
    // any more.
    let mut at_file_start = true;
    move |gap: &[u8]| {
        let mut out = Vec::with_capacity(gap.len());
        let mut leading = at_file_start;
        at_file_start = false;
        let mut index = 0;
        while index < gap.len() {
            if gap[index].is_ascii_whitespace() {
                let width = whitespace_run(&gap[index..]);
                if !leading {
                    out.push(collapsed(&gap[index..index + width], newline_sensitive));
                }
                index += width;
            } else {
                // A byte that is not whitespace is not formatting: a BOM, or
                // anything else a grammar leaves outside its nodes.
                out.push(gap[index]);
                index += 1;
            }
            leading = false;
        }
        out
    }
}

/// Parses `source`, collects its spans and rewrites it with [`minimize`].
///
/// The whitespace policy in its plain configuration: comments kept, no
/// column-0 pinning. A backend that needs pinning composes [`minimize`] with
/// [`rewrite_pinned`] itself, because the predicate is language knowledge this
/// crate does not have.
///
/// Returns [`tokenpress_core::Error::Parse`] for a source the configured
/// grammar reports errors for.
pub fn minimize_source(config: &LanguageConfig, source: &[u8]) -> Result<Vec<u8>> {
    let tree = parser::parse(config, source)?;
    let spans = collect_spans(config, &tree);
    Ok(rewrite(source, &spans, minimize(config)))
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

/// The length of the whitespace run `bytes` starts with, in bytes.
///
/// "Whitespace" is the ASCII set — space, tab, newline, carriage return and
/// form feed — so a CR and the LF after it belong to the same run, which is
/// what makes CRLF collapse to LF rather than to a lone CR. A byte outside
/// that set (a vertical tab, a BOM) is not whitespace and is copied verbatim.
fn whitespace_run(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .take_while(|byte| byte.is_ascii_whitespace())
        .count()
}

/// The single byte a whitespace run collapses to.
///
/// A run that contains a newline *is* a line break, and for a
/// `newline_sensitive` language a line break is a statement terminator, so it
/// comes out as one `\n`. Everything else is a separator, and a separator is
/// one space — never zero, because `a - b` and `a -b` are not the same
/// program in every language, and no engine-level rule can tell which
/// language it is looking at.
fn collapsed(run: &[u8], newline_sensitive: bool) -> u8 {
    if newline_sensitive && run.contains(&b'\n') {
        b'\n'
    } else {
        b' '
    }
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
    use crate::comparable::equivalent;
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

    // ---- the whitespace policy ----------------------------------------

    /// A Java/C#/PHP-shaped configuration: same grammar, newlines carry no
    /// meaning. The kind lists are irrelevant to the whitespace policy, so
    /// they stay Go's.
    fn newline_insensitive_config() -> LanguageConfig {
        LanguageConfig::new(
            go(),
            vec!["comment"],
            vec![
                "interpreted_string_literal",
                "raw_string_literal",
                "rune_literal",
            ],
            false,
        )
        .unwrap()
    }

    /// The policy applied to a gap that is *not* the file's first: the empty
    /// first call stands for a span sitting at byte 0.
    fn minimized_gap(config: &LanguageConfig, gap: &[u8]) -> Vec<u8> {
        let mut policy = minimize(config);
        assert!(policy(b"").is_empty(), "an empty gap yields nothing");
        policy(gap)
    }

    /// The policy applied to the file's first gap.
    fn minimized_leading_gap(config: &LanguageConfig, gap: &[u8]) -> Vec<u8> {
        minimize(config)(gap)
    }

    /// `(name, gap, minimized)` for every gap but the file's first, under a
    /// newline-sensitive config.
    const GAPS: &[(&str, &[u8], &[u8])] = &[
        ("a newline run becomes one newline", b"\n\n\n", b"\n"),
        ("a space run becomes one space", b"     ", b" "),
        ("a tab run becomes one space", b"\t\t", b" "),
        ("a mixed horizontal run becomes one space", b" \t \t", b" "),
        ("indentation joins the newline run", b"\n\t\t", b"\n"),
        ("trailing whitespace joins the newline run", b"   \n", b"\n"),
        ("a blank-line run collapses", b"\n   \n\t\n", b"\n"),
        ("crlf normalises to lf", b"\r\n", b"\n"),
        (
            "a run of crlfs normalises to one lf",
            b"\r\n\r\n\r\n",
            b"\n",
        ),
        ("a lone carriage return carries no newline", b"\r\r", b" "),
        ("a form feed is whitespace", b"\x0c", b" "),
        (
            "a form feed beside a newline joins the run",
            b"\x0c\n",
            b"\n",
        ),
        (
            "intra-line runs collapse one by one",
            b" a  b   c ",
            b" a b c ",
        ),
        ("a gap without whitespace is verbatim", b"x=1", b"x=1"),
        ("an empty gap stays empty", b"", b""),
        (
            "a bom is copied verbatim",
            "\u{feff}  x".as_bytes(),
            "\u{feff} x".as_bytes(),
        ),
        (
            "a vertical tab is not ascii whitespace, so it is verbatim",
            b"\x0b",
            b"\x0b",
        ),
    ];

    /// `(name, gap, minimized)` for the file's *first* gap: its leading run is
    /// the one run that is dropped rather than collapsed.
    const LEADING_GAPS: &[(&str, &[u8], &[u8])] = &[
        ("a leading space run is dropped", b"   x", b"x"),
        ("a leading newline run is dropped", b"\n\n\nx", b"x"),
        ("a leading crlf run is dropped", b"\r\n\r\nx", b"x"),
        ("only the *leading* run is dropped", b"  x  y", b"x y"),
        ("a whitespace-only file yields nothing", b" \n\t \n", b""),
        ("an empty file yields nothing", b"", b""),
        ("a file starting with code is not special", b"x  y", b"x y"),
        (
            "a leading bom is not whitespace, so nothing is dropped",
            "\u{feff}  x".as_bytes(),
            "\u{feff} x".as_bytes(),
        ),
    ];

    /// `(name, gap, minimized)` under a newline-*insensitive* config: a
    /// newline is formatting like any other whitespace, so every run collapses
    /// to one space.
    const INSENSITIVE_GAPS: &[(&str, &[u8], &[u8])] = &[
        ("a newline run becomes one space", b"\n\n", b" "),
        ("crlf becomes one space", b"\r\n", b" "),
        ("a space run still becomes one space", b"  ", b" "),
        ("indentation still becomes one space", b"\n\t", b" "),
        ("code between runs is still verbatim", b" a\nb ", b" a b "),
    ];

    #[test]
    fn every_whitespace_run_collapses_to_the_byte_that_carries_its_meaning() {
        let config = go_config();
        for (name, gap, expected) in GAPS {
            assert_eq!(minimized_gap(&config, gap), *expected, "{name}");
        }
    }

    #[test]
    fn the_files_leading_whitespace_run_is_dropped_entirely() {
        let config = go_config();
        for (name, gap, expected) in LEADING_GAPS {
            assert_eq!(minimized_leading_gap(&config, gap), *expected, "{name}");
        }
    }

    #[test]
    fn a_newline_insensitive_config_collapses_a_newline_run_to_a_space() {
        let config = newline_insensitive_config();
        for (name, gap, expected) in INSENSITIVE_GAPS {
            assert_eq!(minimized_gap(&config, gap), *expected, "{name}");
        }
    }

    #[test]
    fn a_newline_insensitive_config_still_drops_the_leading_run() {
        // The two rules are independent: the `newline_sensitive` branch
        // decides *which* byte a run collapses to, never whether the file's
        // leading run survives.
        let config = newline_insensitive_config();
        assert_eq!(minimized_leading_gap(&config, b"\n\n x"), b"x");
    }

    #[test]
    fn a_whitespace_run_at_the_end_of_the_file_collapses_like_any_other() {
        // Only the *leading* run is dropped; the trailing one collapses, so a
        // file ending in a newline keeps exactly one.
        let config = go_config();
        assert_eq!(minimized_gap(&config, b"x\n\n\n"), b"x\n");
        assert_eq!(minimized_gap(&config, b"x   "), b"x ");
    }

    #[test]
    fn only_the_first_gap_can_hold_the_files_leading_run() {
        // A span at byte 0 leaves an empty first gap, and the run after that
        // span is ordinary: dropping it would join the span to the next line,
        // which for a `newline_sensitive` language deletes a statement
        // terminator.
        let config = go_config();
        let out = rewrite(
            b"// c\n\n\tx",
            &[Span::new(0, 4, SpanKind::Comment)],
            minimize(&config),
        );
        assert_eq!(out, b"// c\nx");
    }

    #[test]
    fn minimize_source_rewrites_a_go_source_byte_for_byte() {
        let config = go_config();
        assert_eq!(
            minimize_source(
                &config,
                b"package main\n\nfunc f() {\n\tg(\"a  b\") // note\n}\n"
            )
            .unwrap(),
            b"package main\nfunc f() {\ng(\"a  b\") // note\n}\n"
        );
    }

    #[test]
    fn minimize_source_drops_the_files_leading_blank_lines() {
        let config = go_config();
        assert_eq!(
            minimize_source(&config, b"\n\n\npackage main\n").unwrap(),
            b"package main\n"
        );
    }

    #[test]
    fn minimize_source_normalises_crlf_to_lf() {
        let config = go_config();
        assert_eq!(
            minimize_source(&config, b"package main\r\n\r\nfunc f() {}\r\n").unwrap(),
            b"package main\nfunc f() {}\n"
        );
    }

    #[test]
    fn minimize_source_never_touches_a_protected_literals_own_whitespace() {
        // The blank line inside the raw string is program data; the blank line
        // outside it is formatting.
        let config = go_config();
        assert_eq!(
            minimize_source(&config, b"package main\n\nvar s = `a\n\n  b`\n").unwrap(),
            b"package main\nvar s = `a\n\n  b`\n"
        );
    }

    #[test]
    fn minimize_source_handles_an_empty_and_a_whitespace_only_file() {
        let config = go_config();
        assert_eq!(minimize_source(&config, b"").unwrap(), b"");
        assert_eq!(minimize_source(&config, b"  \n\t\n").unwrap(), b"");
    }

    #[test]
    fn minimize_source_reports_a_parse_error() {
        let err = minimize_source(&go_config(), b"package main\n\nfunc f( {\n").unwrap_err();
        assert!(matches!(err, Error::Parse(_)), "{err}");
    }

    #[test]
    fn every_corpus_source_minimizes_to_an_equivalent_source() {
        let config = go_config();
        for (name, source) in CORPUS {
            let out = minimize_source(&config, source).unwrap();
            let rendered = String::from_utf8_lossy(&out);
            assert!(
                equivalent(&config, source, &out).unwrap(),
                "{name}: {rendered}"
            );
            // A run of n bytes becomes at most one byte and the leading run
            // becomes none, so the policy can only ever shrink a file.
            assert!(out.len() <= source.len(), "{name}");
            // And it is a fixed point: everything it emits is already minimal.
            assert_eq!(minimize_source(&config, &out).unwrap(), out, "{name}");
        }
    }

    #[test]
    fn generated_sources_minimize_to_equivalent_sources() {
        let config = go_config();
        let mut rng = Rng::new(0x5eed_0003);
        for case in 0..256 {
            let source = generated_source(&mut rng);
            let out = minimize_source(&config, &source).unwrap();
            let name = format!("generated {case}: {}", String::from_utf8_lossy(&source));
            assert!(equivalent(&config, &source, &out).unwrap(), "{name}");
            assert!(out.len() <= source.len(), "{name}");
            // Never introduces a line: the policy emits a `\n` only for a run
            // that already contained one.
            let lines = |bytes: &[u8]| bytes.iter().filter(|byte| **byte == b'\n').count();
            assert!(lines(&out) <= lines(&source), "{name}");
        }
    }

    #[test]
    fn generated_sources_stay_equivalent_with_every_comment_pinned() {
        // The same property with the hook engaged, under a stand-in predicate
        // of the shape G2 will supply: no comment may start a line.
        let config = go_config();
        let mut rng = Rng::new(0x5eed_0004);
        for case in 0..256 {
            let source = generated_source(&mut rng);
            let spans = spans_of(&config, &source);
            let out = rewrite_pinned(&source, &spans, minimize(&config), |span| {
                span.kind == SpanKind::Comment
            });
            let name = format!("generated {case}: {}", String::from_utf8_lossy(&source));
            let rendered = String::from_utf8_lossy(&out);
            assert!(equivalent(&config, &source, &out).unwrap(), "{name}");
            // No pinned span survives at column 0.
            assert!(!out.starts_with(b"//"), "{name}");
            assert!(
                !out.windows(3).any(|window| window == b"\n//"),
                "{name}: {rendered}"
            );
        }
    }

    // ---- the column-0 pinning hook -------------------------------------

    #[test]
    fn pinning_keeps_a_pinned_span_off_column_zero() {
        let config = go_config();
        let source = b"x\n// c";
        let spans = [Span::new(2, 6, SpanKind::Comment)];
        assert_eq!(
            rewrite_pinned(source, &spans, minimize(&config), |_| true),
            b"x\n // c"
        );
    }

    #[test]
    fn pinning_leaves_a_span_that_is_already_mid_line_alone() {
        let config = go_config();
        let source = b"x // c";
        let spans = [Span::new(2, 6, SpanKind::Comment)];
        assert_eq!(
            rewrite_pinned(source, &spans, minimize(&config), |_| true),
            b"x // c"
        );
    }

    #[test]
    fn a_predicate_that_answers_false_leaves_every_span_where_it_is() {
        let config = go_config();
        let source = b"x\n// c";
        let spans = [Span::new(2, 6, SpanKind::Comment)];
        assert_eq!(
            rewrite_pinned(source, &spans, minimize(&config), |_| false),
            b"x\n// c"
        );
    }

    #[test]
    fn pinning_never_fires_for_a_span_at_byte_zero() {
        // Nothing has been emitted, so there is no `\n` to follow and no
        // promotion to undo: the span was already where it is. Prepending a
        // byte here would corrupt a byte-0 prologue, which is why the hook
        // fires on an emitted newline rather than on "column 0" as such.
        let config = go_config();
        let source = b"// c\nx";
        let spans = [Span::new(0, 4, SpanKind::Comment)];
        assert_eq!(
            rewrite_pinned(source, &spans, minimize(&config), |_| true),
            b"// c\nx"
        );
    }

    #[test]
    fn the_predicate_is_asked_only_where_the_hook_could_fire() {
        // Three spans: one at byte 0, one after a newline, one mid-line. Only
        // the middle one is a candidate, so only it reaches the predicate.
        let config = go_config();
        let source = b"a\nb c";
        let spans = [
            Span::new(0, 1, SpanKind::Protected),
            Span::new(2, 3, SpanKind::Protected),
            Span::new(4, 5, SpanKind::Comment),
        ];
        let mut asked = Vec::new();
        let out = rewrite_pinned(source, &spans, minimize(&config), |span| {
            asked.push(span);
            true
        });
        assert_eq!(asked, [Span::new(2, 3, SpanKind::Protected)]);
        assert_eq!(out, b"a\n b c");
    }

    #[test]
    fn pinning_fires_once_per_pinned_span() {
        let config = go_config();
        let source = b"// a\n// b\n// c";
        let spans = [
            Span::new(0, 4, SpanKind::Comment),
            Span::new(5, 9, SpanKind::Comment),
            Span::new(10, 14, SpanKind::Comment),
        ];
        assert_eq!(
            rewrite_pinned(source, &spans, minimize(&config), |_| true),
            b"// a\n // b\n // c"
        );
    }

    #[test]
    fn pinning_is_independent_of_the_gap_policy() {
        // The hook reads the bytes emitted so far, not the policy: `keep`
        // leaves the newline in place and the pin fires just the same.
        let source = b"x\n\n// c";
        let spans = [Span::new(3, 7, SpanKind::Comment)];
        assert_eq!(
            rewrite_pinned(source, &spans, keep, |_| true),
            b"x\n\n // c"
        );
    }

    #[test]
    fn rewrite_is_the_unpinned_rewriter() {
        let config = go_config();
        for (name, source) in CORPUS {
            let spans = spans_of(&config, source);
            assert_eq!(
                rewrite(source, &spans, minimize(&config)),
                rewrite_pinned(source, &spans, minimize(&config), |_| false),
                "{name}"
            );
        }
    }
}
