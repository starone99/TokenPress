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
//! # Merging is for nesting, so it needs a shared byte
//!
//! [`collect_spans`] sorts and merges, so ranges that **share a byte** become
//! one — a comment inside a protected region, a literal inside a protected
//! prologue. Merging keeps the kind of the span that *starts* the merged run,
//! which is what makes it the right answer for nesting and the wrong one for
//! anything else: it *reclassifies* the span it absorbs.
//!
//! Two spans that merely **touch** — one ending exactly at the byte the next
//! starts on — therefore stay two spans. They nest in neither direction, so
//! there is nothing to resolve, and joining them would hand the second one's
//! bytes to the first one's policy. A block comment ending where a string
//! literal begins (`x := /*c*/"lit"`) is exactly that shape: as one merged
//! `Comment` span the literal is deleted along with the comment, which the
//! equivalence check refuses and `reparse` does not. Nothing downstream can
//! tell two touching spans from one: the gap between them is empty, so the gap
//! policy emits nothing for it, and each span is copied — or blanked — over
//! the same bytes either way. The rule is "share a byte" rather than "share a
//! byte or a kind" for the same reason: making it conditional on the kind
//! would buy a consolidation nobody reads, at the price of a case in the rule.
//!
//! # Why the sort order is `(start, Reverse(end))`
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
//! comments-kept configuration. [`strip_comments`] is the second stage and
//! takes the same gap policy parameter for the same reason, so
//! [`strip_comments_source`] is the comments-stripped configuration.
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
//!
//! # The comment policy
//!
//! Comment stripping cannot be a gap policy: a comment is *collected into* the
//! spans, so by the time a gap policy runs it is one of the bytes that policy
//! is forbidden to touch. It is instead a second classification of the same
//! spans. [`strip_comments_plan`] partitions the merged span set into a
//! [`StripPlan`]'s two halves — what survives verbatim, and the comments that
//! do not — and [`strip_comments`] applies it before [`rewrite`] ever sees the
//! file.
//!
//! ## Deletion is blanking
//!
//! A deleted comment's bytes are not removed, they are **overwritten with
//! whitespace of the same length**: every byte becomes a space, except a `\n`,
//! which stays one. Three properties fall out of that, and each one replaces a
//! special case:
//!
//! - the bytes on either side of a vanished comment reach the gap policy as
//!   **one** gap, so `\n  // note\n` is a single whitespace run and
//!   [`minimize`] collapses it to the one `\n` it carried: a comment-only line
//!   leaves no blank line behind, with no rule about lines anywhere in this
//!   module;
//! - **every newline of the file survives deletion**, including the ones
//!   *inside* a deleted comment. Go's scanner reads a `/* … \n … */` as a line
//!   break (a general comment containing newlines "acts like a newline"), so
//!   dropping those bytes outright could delete a statement terminator that
//!   the equivalence artifact — comment-blind by construction — could never
//!   report. Keeping them means the T4 argument carries over unchanged: the
//!   emitter never joins two lines and never introduces one;
//! - a comment is at least one byte wide, so what replaces it is at least one
//!   whitespace byte: two tokens that a comment separated stay separated,
//!   `x := 1 /* c */ + 2` becomes `x := 1 + 2` and never `x := 1+2`.
//!
//! Blanking is also **offset-preserving**, which is what makes the column-0
//! hook compose: [`strip_comments_pinned`] hands its predicate spans that
//! still index the *original* source, so a predicate that decides from a
//! comment's bytes — the shape a directive rule has — needs no coordinate map.
//!
//! The one visible residue is at end of file: only the file's *leading*
//! whitespace run is dropped, so a comment that ended a file with no final
//! newline leaves the single space its run collapses to. One byte, pinned by a
//! test rather than special-cased.
//!
//! ## The three language-specific decisions
//!
//! Which comments carry meaning is language knowledge, so it arrives as a
//! [`CommentPolicy`]: a keep predicate over a comment's own bytes, a verbatim
//! prologue region, and a whole-file bail-out. Both region and bail-out are
//! decided from the tree and the source, because that is what the rules they
//! stand for need (a Go build-constraint block ends where the package clause
//! begins; a cgo file is one that imports `"C"`).
//!
//! Neither the prologue nor the bail-out is a flag this module acts on. They
//! are expressed in the model that already exists:
//!
//! - the **prologue** enters the span set as a synthetic
//!   [`SpanKind::Protected`] span and goes through the same merge as
//!   everything else, which is why it is a plan-level entry point rather than
//!   a second span source ([`collect_spans`] stays the only one, `merge` stays
//!   private). Its bytes are therefore reproduced **verbatim, blank lines
//!   included** — protecting it from deletion alone would not be enough,
//!   because the whitespace policy collapses the blank line after a
//!   `//go:build` line and that blank line is part of the constraint's
//!   meaning. Neither loss is visible to the equivalence artifact, so the
//!   region is the only defence there is;
//! - the **bail-out** yields the plan that protects the whole file. There is
//!   then no gap to rewrite and nothing that could be promoted to column 0, so
//!   the output is the input byte for byte under *any* gap policy and *any*
//!   pinning predicate.
//!
//! A comment that overlaps the prologue is kept whatever the keep predicate
//! says. That is deliberately not left to the merge: the merged run inherits
//! the kind of the span that *starts* it, so a comment beginning before the
//! region hands the whole run to the comment policy — the same inversion the
//! sort order guards against, in the one place the sort order cannot reach.

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
/// precondition [`rewrite`] is documented to need. Ranges that share a byte
/// are merged, keeping the kind of the outermost one; ranges that only touch
/// stay separate. See the module docs for why both halves of that are
/// load-bearing.
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

/// The three language-specific decisions comment stripping is made of.
///
/// They are callbacks, and they are parameters of [`strip_comments_plan`]
/// rather than fields of a [`LanguageConfig`], because a config is a validated
/// description of a *grammar*: kind names checked against it, `Debug`, no type
/// parameters, shared by [`crate::parser::parse`] and
/// [`crate::comparable::comparable`], neither of which has any use for a
/// policy. Hanging three closures off it would put three type parameters into
/// every signature that mentions one. They are generic rather than
/// `Box<dyn Fn>` because nothing here stores a policy for later.
///
/// The callbacks are `Fn`, not `FnMut`: each one is a decision about the bytes
/// it is handed, taken an unspecified number of times and in an unspecified
/// order.
pub struct CommentPolicy<Keep, Prologue, BailOut> {
    keep_comment: Keep,
    prologue: Prologue,
    bail_out: BailOut,
}

impl<Keep, Prologue, BailOut> CommentPolicy<Keep, Prologue, BailOut>
where
    Keep: Fn(&[u8]) -> bool,
    Prologue: Fn(&Tree, &[u8]) -> Range<usize>,
    BailOut: Fn(&Tree, &[u8]) -> bool,
{
    /// Builds a policy from its three decisions.
    ///
    /// - `keep_comment` is handed a comment span's own source bytes and
    ///   answers whether it has to survive;
    /// - `prologue` answers the byte range at the head of the file whose bytes
    ///   are reproduced verbatim — blank lines included — or an empty range
    ///   when the language has no such region;
    /// - `bail_out` answers whether the file must be left alone entirely.
    ///
    /// See the module docs for what each one is for and what it may not do.
    pub fn new(keep_comment: Keep, prologue: Prologue, bail_out: BailOut) -> Self {
        Self {
            keep_comment,
            prologue,
            bail_out,
        }
    }
}

/// What a comment-stripping run does with the bytes it has classified: which
/// spans survive verbatim, and which are blanked out.
///
/// The two lists are a partition of one merged span set, so each is sorted and
/// non-overlapping, no `deleted` span overlaps a `protected` one, and both are
/// within the source — the precondition [`strip_comments`] is documented to
/// need. [`strip_comments_plan`] produces plans that satisfy it.
#[derive(Debug, PartialEq, Eq)]
pub struct StripPlan {
    /// The byte ranges copied verbatim: every protected span, plus the
    /// comments the policy keeps, plus the prologue region.
    pub protected: Vec<Span>,
    /// The comment spans that do not survive.
    pub deleted: Vec<Span>,
}

/// Classifies `source` for comment stripping under `policy`.
///
/// Every comment span is deleted except the ones the policy keeps and the ones
/// that share a byte with the prologue region; a policy that bails out yields
/// the plan that protects the whole file. See the module docs.
pub fn strip_comments_plan<Keep, Prologue, BailOut>(
    config: &LanguageConfig,
    tree: &Tree,
    source: &[u8],
    policy: &CommentPolicy<Keep, Prologue, BailOut>,
) -> StripPlan
where
    Keep: Fn(&[u8]) -> bool,
    Prologue: Fn(&Tree, &[u8]) -> Range<usize>,
    BailOut: Fn(&Tree, &[u8]) -> bool,
{
    if (policy.bail_out)(tree, source) {
        // One span over the whole file: every byte is copied verbatim and
        // there is no gap left for a policy to rewrite, so the output is the
        // input whatever else is applied to it.
        return StripPlan {
            protected: vec![Span::new(0, source.len(), SpanKind::Protected)],
            deleted: Vec::new(),
        };
    }

    let mut spans = collect_spans(config, tree);
    let prologue = (policy.prologue)(tree, source);
    if !prologue.is_empty() {
        // The prologue's own entry point into the span set: it is not a node
        // kind, so collection cannot produce it, and it has to go through the
        // same merge as everything else.
        spans.push(Span::new(prologue.start, prologue.end, SpanKind::Protected));
        spans = merge(spans);
    }

    let mut protected = Vec::new();
    let mut deleted = Vec::new();
    for span in spans {
        // The overlap test, not the merged kind, is what protects the
        // prologue: a comment that starts before it decides the merged run's
        // kind, so the run can reach here classified `Comment`.
        if span.kind == SpanKind::Comment
            && !overlaps(&prologue, span)
            && !(policy.keep_comment)(&source[span.range()])
        {
            deleted.push(span);
        } else {
            protected.push(span);
        }
    }
    StripPlan { protected, deleted }
}

/// Rebuilds `source` under `plan`: the deleted spans become whitespace of the
/// same length, so the bytes on either side of a vanished comment reach the gap
/// policy as **one** gap, and the protected spans are copied verbatim.
///
/// `plan` must satisfy the invariants [`StripPlan`] documents, which is what
/// [`strip_comments_plan`] returns.
pub fn strip_comments(
    source: &[u8],
    plan: &StripPlan,
    gap_policy: impl FnMut(&[u8]) -> Vec<u8>,
) -> Vec<u8> {
    strip_comments_pinned(source, plan, gap_policy, |_| false)
}

/// [`strip_comments`] plus the column-0 pinning hook, exactly as
/// [`rewrite_pinned`] is [`rewrite`] plus that hook.
///
/// Blanking is length-preserving, so the spans the predicate is handed index
/// `source` itself: a predicate that reads a span's bytes out of the original
/// source — which is the shape a directive rule needs — composes with
/// stripping without a coordinate map.
pub fn strip_comments_pinned(
    source: &[u8],
    plan: &StripPlan,
    gap_policy: impl FnMut(&[u8]) -> Vec<u8>,
    never_starts_a_line: impl FnMut(Span) -> bool,
) -> Vec<u8> {
    rewrite_pinned(
        &blanked(source, &plan.deleted),
        &plan.protected,
        gap_policy,
        never_starts_a_line,
    )
}

/// Parses `source`, plans its comment stripping under `policy` and rewrites
/// what is left with [`minimize`]: the comments-stripped emitter.
///
/// Returns [`tokenpress_core::Error::Parse`] for a source the configured
/// grammar reports errors for.
pub fn strip_comments_source<Keep, Prologue, BailOut>(
    config: &LanguageConfig,
    source: &[u8],
    policy: &CommentPolicy<Keep, Prologue, BailOut>,
) -> Result<Vec<u8>>
where
    Keep: Fn(&[u8]) -> bool,
    Prologue: Fn(&Tree, &[u8]) -> Range<usize>,
    BailOut: Fn(&Tree, &[u8]) -> bool,
{
    let tree = parser::parse(config, source)?;
    let plan = strip_comments_plan(config, &tree, source, policy);
    Ok(strip_comments(source, &plan, minimize(config)))
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

/// `source` with every deleted span turned into whitespace of the same length:
/// each of its bytes becomes a space, except a `\n`, which stays one.
///
/// Length-preserving, so every offset in the plan still indexes the result,
/// and newline-preserving, so a deleted comment still ends the lines it
/// ended. See the module docs for why both properties are load-bearing.
fn blanked(source: &[u8], deleted: &[Span]) -> Vec<u8> {
    let mut out = source.to_vec();
    for span in deleted {
        for byte in &mut out[span.range()] {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
    }
    out
}

/// Whether `span` shares a byte with `region`.
///
/// An empty region shares nothing with anything, which is what makes "this
/// language has no prologue" the same code path as "this file has none".
fn overlaps(region: &Range<usize>, span: Span) -> bool {
    region.start < span.end && span.start < region.end
}

/// Sorts `spans` and merges every pair that **shares a byte**, keeping the
/// kind of the span that starts the merged run. Two spans that merely touch —
/// one ending exactly where the next begins — stay two spans.
///
/// The strict `<` is the whole rule: merging exists for nesting, and a nested
/// span shares bytes with the one containing it. A touch shares none, so
/// joining the two only reclassifies the second under the first's kind, which
/// is how a block comment ending where a literal begins used to swallow it.
/// See the module docs.
fn merge(mut spans: Vec<Span>) -> Vec<Span> {
    spans.sort_by_key(|span| (span.start, Reverse(span.end), span.kind));
    let mut merged: Vec<Span> = Vec::with_capacity(spans.len());
    for span in spans {
        match merged.last_mut() {
            Some(last) if span.start < last.end => last.end = last.end.max(span.end),
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
    use std::cell::RefCell;
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
    fn merging_joins_spans_that_share_a_byte_and_leaves_touching_and_disjoint_ones_alone() {
        // `0..4` and `2..6` share bytes 2 and 3, so they are one run and the
        // earlier span's kind decides it. `6..8` starts exactly where that run
        // ends: it shares no byte with it and stays its own span, as does the
        // disjoint `10..14`.
        let merged = merge(vec![
            Span::new(10, 14, SpanKind::Comment),
            Span::new(0, 4, SpanKind::Protected),
            Span::new(2, 6, SpanKind::Comment),
            Span::new(6, 8, SpanKind::Protected),
        ]);
        assert_eq!(
            merged,
            vec![
                Span::new(0, 6, SpanKind::Protected),
                Span::new(6, 8, SpanKind::Protected),
                Span::new(10, 14, SpanKind::Comment),
            ]
        );
    }

    #[test]
    fn merging_leaves_two_touching_spans_of_the_same_kind_alone_as_well() {
        // The rule is "share a byte", not "share a byte or a kind": two
        // touching comments stay two spans. Nothing downstream can tell the
        // difference — each is copied or blanked over the same bytes either
        // way — and making the merge conditional on the kind would buy a
        // consolidation nobody reads, at the price of a rule with a case in
        // it. Pinned so that neither `<=` nor a kind-conditional variant can
        // come back unnoticed.
        assert_eq!(
            merge(vec![
                Span::new(0, 5, SpanKind::Comment),
                Span::new(5, 9, SpanKind::Comment),
            ]),
            vec![
                Span::new(0, 5, SpanKind::Comment),
                Span::new(5, 9, SpanKind::Comment),
            ]
        );
        assert_eq!(
            merge(vec![
                Span::new(0, 5, SpanKind::Protected),
                Span::new(5, 9, SpanKind::Protected),
            ]),
            vec![
                Span::new(0, 5, SpanKind::Protected),
                Span::new(5, 9, SpanKind::Protected),
            ]
        );
    }

    #[test]
    fn merging_absorbs_an_empty_span_that_falls_inside_another() {
        // Collection cannot produce an empty span — the parse gate rejects
        // every tree with a `MISSING` node, and no configured kind is a
        // zero-width token — and the prologue's own entry point is guarded by
        // `is_empty`. `merge` is reachable from the tests directly, though, so
        // what it does with one is pinned rather than assumed: an empty span
        // strictly inside another shares a byte position with it and is
        // absorbed, and one at another's end shares nothing and survives as
        // the zero bytes it is.
        assert_eq!(
            merge(vec![
                Span::new(0, 4, SpanKind::Protected),
                Span::new(2, 2, SpanKind::Comment),
            ]),
            vec![Span::new(0, 4, SpanKind::Protected)]
        );
        let boundary = merge(vec![
            Span::new(0, 4, SpanKind::Protected),
            Span::new(4, 4, SpanKind::Comment),
        ]);
        assert_eq!(
            boundary,
            vec![
                Span::new(0, 4, SpanKind::Protected),
                Span::new(4, 4, SpanKind::Comment),
            ]
        );
        // Still the precondition `rewrite` documents, and still the identity.
        assert_sorted_disjoint_in_bounds(&boundary, 4, "empty span at a boundary");
        assert_eq!(rewrite(b"abcd", &boundary, keep), b"abcd");
    }

    #[test]
    fn a_comment_byte_adjacent_to_a_literal_stays_its_own_span() {
        // The over-refusal class this rule exists for: `/*c*/` ends exactly
        // where `"lit"` begins. Merging them would hand the literal to the
        // comment policy — the merged run keeps the *earlier* kind — and
        // deleting a comment would delete a string literal with it.
        let config = go_config();
        let source = b"package main\n\nfunc f() { g(/*c*/\"lit\") }\n";
        let spans = spans_of(&config, source);
        assert_eq!(texts(source, &spans), vec![&b"/*c*/"[..], &b"\"lit\""[..]]);
        assert_eq!(spans[0].kind, SpanKind::Comment);
        assert_eq!(spans[1].kind, SpanKind::Protected);
        assert_eq!(spans[0].end, spans[1].start, "the two have to be touching");
    }

    #[test]
    fn a_comment_byte_adjacent_after_a_literal_stays_its_own_span_too() {
        // The mirror image, which merging classified `Protected` and so kept:
        // a lost saving rather than a lost literal, and gone for the same
        // reason.
        let config = go_config();
        let source = b"package main\n\nfunc f() { g(\"lit\"/*c*/) }\n";
        let spans = spans_of(&config, source);
        assert_eq!(texts(source, &spans), vec![&b"\"lit\""[..], &b"/*c*/"[..]]);
        assert_eq!(spans[0].kind, SpanKind::Protected);
        assert_eq!(spans[1].kind, SpanKind::Comment);
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

    // ---- the comment policy --------------------------------------------

    /// The stand-in policies' type. Plain `fn` pointers rather than closures,
    /// so a helper can name the policy it is handed; the real Go ones (G2)
    /// capture nothing either.
    type StandInPolicy = CommentPolicy<
        fn(&[u8]) -> bool,
        fn(&Tree, &[u8]) -> Range<usize>,
        fn(&Tree, &[u8]) -> bool,
    >;

    /// The keep predicate that keeps nothing.
    fn never_keep(_comment: &[u8]) -> bool {
        false
    }

    /// A keep predicate of the shape G2 will supply: a marked comment is
    /// semantic and survives.
    fn keep_marked(comment: &[u8]) -> bool {
        comment.starts_with(b"//keep")
    }

    /// The prologue of a language that has none.
    fn no_prologue(_tree: &Tree, _source: &[u8]) -> Range<usize> {
        0..0
    }

    /// A prologue of the shape G2 will supply: everything before the file's
    /// `package_clause`, which is `0..0` for a file that starts with one.
    fn prologue_before_package(tree: &Tree, _source: &[u8]) -> Range<usize> {
        0..package_offset(tree)
    }

    /// A deliberately misaligned prologue: it starts three bytes into the
    /// file, so on a file that opens with a comment it starts *inside* one.
    fn prologue_from_byte_three(tree: &Tree, _source: &[u8]) -> Range<usize> {
        3..package_offset(tree)
    }

    /// The start byte of the file's package clause, or 0 when it has none.
    fn package_offset(tree: &Tree) -> usize {
        let root = tree.root_node();
        let mut cursor = root.walk();
        let clause = root
            .children(&mut cursor)
            .find(|child| child.kind() == "package_clause");
        clause.map_or(0, |clause| clause.start_byte())
    }

    /// The bail-out of a language that never needs one.
    fn never_bail(_tree: &Tree, _source: &[u8]) -> bool {
        false
    }

    /// A bail-out of the shape G2 will supply: a cgo file is left alone.
    fn bail_on_import_c(_tree: &Tree, source: &[u8]) -> bool {
        source
            .windows(IMPORT_C.len())
            .any(|window| window == IMPORT_C)
    }

    const IMPORT_C: &[u8] = b"import \"C\"";

    /// The most aggressive policy: every comment goes, nothing is special.
    fn delete_every_comment() -> StandInPolicy {
        CommentPolicy::new(never_keep, no_prologue, never_bail)
    }

    /// All three callbacks doing something.
    fn stand_in_policy() -> StandInPolicy {
        CommentPolicy::new(keep_marked, prologue_before_package, bail_on_import_c)
    }

    fn plan_for(config: &LanguageConfig, source: &[u8], policy: &StandInPolicy) -> StripPlan {
        let tree = parser::parse(config, source).unwrap();
        strip_comments_plan(config, &tree, source, policy)
    }

    fn stripped(config: &LanguageConfig, source: &[u8], policy: &StandInPolicy) -> Vec<u8> {
        strip_comments_source(config, source, policy).unwrap()
    }

    /// The source bytes of each span, so an assertion reads as the text it is
    /// about rather than as offsets.
    fn texts<'a>(source: &'a [u8], spans: &[Span]) -> Vec<&'a [u8]> {
        spans.iter().map(|span| &source[span.range()]).collect()
    }

    /// The precondition [`strip_comments`] documents: two sorted, disjoint,
    /// in-bounds lists whose union is sorted and disjoint too.
    fn assert_plan_is_well_formed(plan: &StripPlan, len: usize, name: &str) {
        assert_sorted_disjoint_in_bounds(&plan.protected, len, name);
        assert_sorted_disjoint_in_bounds(&plan.deleted, len, name);
        let mut all: Vec<Span> = plan
            .protected
            .iter()
            .chain(plan.deleted.iter())
            .copied()
            .collect();
        all.sort_by_key(|span| span.start);
        assert_sorted_disjoint_in_bounds(&all, len, name);
    }

    #[test]
    fn a_plan_partitions_the_spans_into_the_kept_and_the_deleted() {
        let config = go_config();
        let source = b"package main\n\n// note\n//keep me\nfunc f() { g(\"a  b\") }\n";
        let plan = plan_for(&config, source, &stand_in_policy());
        assert_eq!(
            texts(source, &plan.protected),
            [&b"//keep me"[..], b"\"a  b\""]
        );
        assert_eq!(texts(source, &plan.deleted), [&b"// note"[..]]);
    }

    #[test]
    fn a_source_without_comments_deletes_nothing() {
        let config = go_config();
        let source = b"package main\n\nvar s = \"a\"\n";
        let plan = plan_for(&config, source, &delete_every_comment());
        assert!(plan.deleted.is_empty(), "{plan:?}");
        assert_eq!(texts(source, &plan.protected), [&b"\"a\""[..]]);
    }

    #[test]
    fn the_keep_predicate_sees_every_comments_own_bytes_and_nothing_else() {
        let config = go_config();
        let source = b"package main\n\n// a\n/* b */\nvar s = `c // d`\n";
        let seen = RefCell::new(Vec::new());
        let policy = CommentPolicy::new(
            |comment: &[u8]| {
                seen.borrow_mut().push(comment.to_vec());
                false
            },
            no_prologue,
            never_bail,
        );
        let tree = parser::parse(&config, source).unwrap();
        let plan = strip_comments_plan(&config, &tree, source, &policy);
        // The bytes that look like a comment inside the raw string are a
        // protected literal, so the policy is never even asked about them.
        assert_eq!(seen.into_inner(), [b"// a".to_vec(), b"/* b */".to_vec()]);
        assert_eq!(texts(source, &plan.deleted), [&b"// a"[..], b"/* b */"]);
        assert_eq!(texts(source, &plan.protected), [&b"`c // d`"[..]]);
    }

    #[test]
    fn the_prologue_region_is_reproduced_verbatim_blank_line_included() {
        let config = go_config();
        let source = b"//go:build ignore\n\npackage main\n";
        // Without a prologue the constraint is deleted and the blank line that
        // separates it from the package clause collapses with it. Both are
        // equivalence-clean and both are fatal to a build constraint, which is
        // why the prologue is a *verbatim* region rather than a keep rule.
        assert_eq!(
            stripped(&config, source, &delete_every_comment()),
            b"package main\n"
        );
        assert_eq!(stripped(&config, source, &stand_in_policy()), source);
    }

    #[test]
    fn a_comment_the_prologue_covers_is_never_deleted() {
        let config = go_config();
        let source = b"// note\npackage main\n";
        let plan = plan_for(&config, source, &stand_in_policy());
        // The prologue starts first and reaches further, so the merged run is
        // its: protected, not a comment any more.
        assert_eq!(plan.protected, [Span::new(0, 8, SpanKind::Protected)]);
        assert!(plan.deleted.is_empty(), "{plan:?}");
        assert_eq!(stripped(&config, source, &stand_in_policy()), source);
    }

    #[test]
    fn a_prologue_the_merge_classifies_as_a_comment_is_still_never_deleted() {
        // A prologue that starts *inside* a comment: the comment starts first,
        // so the merged run inherits *its* kind and the sort order cannot
        // save the region. The overlap rule is what does.
        let config = go_config();
        let source = b"// note\npackage main\n";
        let policy: StandInPolicy =
            CommentPolicy::new(never_keep, prologue_from_byte_three, never_bail);
        let plan = plan_for(&config, source, &policy);
        assert_eq!(plan.protected, [Span::new(0, 8, SpanKind::Comment)]);
        assert!(plan.deleted.is_empty(), "{plan:?}");
        assert_eq!(stripped(&config, source, &policy), source);
    }

    #[test]
    fn a_comment_that_only_touches_the_prologue_is_outside_it() {
        // The other side of the same rule, and the one place where dropping
        // merge-on-touch is visible beyond the literal case: here the comment
        // ends on the byte the region starts on. It shares no byte with the
        // region, so it is not merged into it and the region's own overlap
        // test — which has always been a strict one — answers `false`. A
        // language whose prologue has to cover a comment says so by returning
        // a region that covers it; Go's cannot reach this shape at all, since
        // its region is `0 .. package_clause.start_byte()` and no comment can
        // end at byte 0 or begin at the `p` of `package`.
        let config = go_config();
        let source = b"//a\npackage main\n";
        let policy: StandInPolicy =
            CommentPolicy::new(never_keep, prologue_from_byte_three, never_bail);
        let plan = plan_for(&config, source, &policy);
        assert_eq!(plan.deleted, [Span::new(0, 3, SpanKind::Comment)]);
        assert_eq!(plan.protected, [Span::new(3, 4, SpanKind::Protected)]);
        // The region's own byte — the newline — is still reproduced verbatim.
        assert_eq!(stripped(&config, source, &policy), b"\npackage main\n");
    }

    #[test]
    fn an_empty_prologue_region_contributes_no_span() {
        let config = go_config();
        // The package clause is at byte 0, so the stand-in prologue is `0..0`.
        let plan = plan_for(&config, b"package main\n", &stand_in_policy());
        assert_eq!(
            plan,
            StripPlan {
                protected: Vec::new(),
                deleted: Vec::new(),
            }
        );
    }

    #[test]
    fn a_bail_out_protects_the_whole_file_and_reproduces_it_byte_for_byte() {
        let config = go_config();
        let source = b"package main\n\n/*\n#include <stdio.h>\n*/\nimport \"C\"\n";
        let plan = plan_for(&config, source, &stand_in_policy());
        assert_eq!(
            plan,
            StripPlan {
                protected: vec![Span::new(0, source.len(), SpanKind::Protected)],
                deleted: Vec::new(),
            }
        );
        assert_eq!(stripped(&config, source, &stand_in_policy()), source);
        // Whatever the gap policy, and even with every span pinned: the file
        // is one span, so there is no gap to rewrite and nothing that could be
        // promoted to column 0.
        assert_eq!(strip_comments(source, &plan, keep), source);
        assert_eq!(
            strip_comments_pinned(source, &plan, minimize(&config), |_| true),
            source
        );
        // The hazard it exists for: without it the cgo preamble is deleted,
        // and the equivalence artifact cannot see the difference.
        let deleted = stripped(&config, source, &delete_every_comment());
        assert_ne!(deleted, source);
        assert!(equivalent(&config, source, &deleted).unwrap());
    }

    #[test]
    fn deletion_blanks_a_comments_bytes_and_keeps_the_newlines_they_carried() {
        // The deletion half on its own: a hand-built plan, no grammar.
        let source = b"a /* x\ny */ b";
        let plan = StripPlan {
            protected: Vec::new(),
            deleted: vec![Span::new(2, 11, SpanKind::Comment)],
        };
        assert_eq!(strip_comments(source, &plan, keep), b"a     \n     b");
        // The blanked bytes joined the runs on either side, so the whole thing
        // is one run — and it still carries the newline the comment held.
        let config = go_config();
        assert_eq!(strip_comments(source, &plan, minimize(&config)), b"a\nb");
    }

    #[test]
    fn a_deleted_block_comment_still_ends_the_line_it_ended() {
        let config = go_config();
        let source = b"package main\n\nfunc f() {\n\tx := 1\n\t/* a\n\t   b */\n\t_ = x\n}\n";
        assert_eq!(
            stripped(&config, source, &delete_every_comment()),
            b"package main\nfunc f() {\nx := 1\n_ = x\n}\n"
        );
    }

    #[test]
    fn a_comment_only_line_leaves_no_blank_line_behind() {
        let config = go_config();
        assert_eq!(
            stripped(
                &config,
                b"package main\n\n// note\nfunc f() {}\n",
                &delete_every_comment()
            ),
            b"package main\nfunc f() {}\n"
        );
    }

    #[test]
    fn adjacent_comment_lines_vanish_together() {
        let config = go_config();
        assert_eq!(
            stripped(
                &config,
                b"package main\n\n// a\n// b\nfunc f() {}\n",
                &delete_every_comment()
            ),
            b"package main\nfunc f() {}\n"
        );
    }

    #[test]
    fn a_comment_at_byte_zero_vanishes_with_the_files_leading_run() {
        let config = go_config();
        assert_eq!(
            stripped(&config, b"// note\npackage main\n", &delete_every_comment()),
            b"package main\n"
        );
    }

    #[test]
    fn a_comment_at_the_end_of_a_file_leaves_the_space_its_run_collapses_to() {
        // Only the file's *leading* whitespace run is dropped, so the run the
        // blanked comment joined comes out as the one space every horizontal
        // run comes out as. One byte, pinned rather than special-cased.
        let config = go_config();
        assert_eq!(
            stripped(
                &config,
                b"package main\n\nfunc f() {} // note",
                &delete_every_comment()
            ),
            b"package main\nfunc f() {} "
        );
    }

    #[test]
    fn a_deleted_comment_between_two_tokens_still_separates_them() {
        let config = go_config();
        let source = b"package main\n\nfunc f() {\n\tx := 1 /* c */ + 2\n\t_ = x\n}\n";
        let out = stripped(&config, source, &delete_every_comment());
        assert_eq!(out, b"package main\nfunc f() {\nx := 1 + 2\n_ = x\n}\n");
        assert!(equivalent(&config, source, &out).unwrap());
    }

    #[test]
    fn a_comment_byte_adjacent_to_a_literal_is_deleted_and_the_literal_survives() {
        // The end-to-end shape of the over-refusal: before the merge rule
        // required a shared byte, `/*c*/"lit"` came out as one deletable
        // `Comment` span and the literal went with the comment. At
        // `--verify ast` that was a refusal; at `--verify reparse` it was a
        // written file with a string literal missing from it.
        let config = go_config();
        let source =
            b"package main\n\nimport \"fmt\"\n\nfunc f() {\n\tfmt.Println(/*c*/\"lit\")\n}\n";
        let out = stripped(&config, source, &delete_every_comment());
        assert_eq!(
            out,
            b"package main\nimport \"fmt\"\nfunc f() {\nfmt.Println( \"lit\")\n}\n"
        );
        assert!(equivalent(&config, source, &out).unwrap());
    }

    #[test]
    fn a_comment_byte_adjacent_after_a_literal_is_deleted_as_well() {
        // The mirror image was kept, because the merged run inherited the
        // literal's kind. It is a comment again, so it goes.
        let config = go_config();
        let source =
            b"package main\n\nimport \"fmt\"\n\nfunc f() {\n\tfmt.Println(\"lit\"/*c*/)\n}\n";
        let out = stripped(&config, source, &delete_every_comment());
        assert_eq!(
            out,
            b"package main\nimport \"fmt\"\nfunc f() {\nfmt.Println(\"lit\" )\n}\n"
        );
        assert!(equivalent(&config, source, &out).unwrap());
    }

    #[test]
    fn comment_bytes_inside_a_literal_still_survive_stripping() {
        // The case that must not regress: containment is a shared byte, so it
        // still merges and the outer kind still wins. These `//` bytes are the
        // program, and stripping every comment has to leave them alone.
        let config = go_config();
        let source = b"package main\n\nvar s = `a // b\nc`\nvar t = \"// not a comment\"\n";
        let out = stripped(&config, source, &delete_every_comment());
        assert_eq!(
            out,
            b"package main\nvar s = `a // b\nc`\nvar t = \"// not a comment\"\n"
        );
        assert!(equivalent(&config, source, &out).unwrap());
    }

    #[test]
    fn a_newline_insensitive_config_strips_the_same_spans() {
        // The plan does not depend on newline sensitivity at all; the gap
        // policy it feeds does, and this is the Java/C#/PHP branch of it.
        let source = b"package main\n\n// note\nfunc f() {}\n";
        assert_eq!(
            stripped(&go_config(), source, &delete_every_comment()),
            b"package main\nfunc f() {}\n"
        );
        assert_eq!(
            stripped(
                &newline_insensitive_config(),
                source,
                &delete_every_comment()
            ),
            b"package main func f() {} "
        );
    }

    #[test]
    fn a_surviving_comment_can_still_be_kept_off_column_zero() {
        // Stripping composes with the T4 hook, and the predicate reads the
        // surviving comment's bytes out of the *original* source — which is
        // sound only because blanking preserves every offset.
        let config = go_config();
        let source =
            b"package main\n\nfunc f() {\n\t// note\n\t//keep:directive\n\tx := 1\n\t_ = x\n}\n";
        let policy: StandInPolicy = CommentPolicy::new(keep_marked, no_prologue, never_bail);
        let plan = plan_for(&config, source, &policy);
        let out = strip_comments_pinned(source, &plan, minimize(&config), |span| {
            source[span.range()].starts_with(b"//keep")
        });
        assert_eq!(
            out,
            b"package main\nfunc f() {\n //keep:directive\nx := 1\n_ = x\n}\n"
        );
        assert!(equivalent(&config, source, &out).unwrap());
    }

    #[test]
    fn strip_comments_is_the_unpinned_stripper() {
        let config = go_config();
        for (name, source) in CORPUS {
            let plan = plan_for(&config, source, &stand_in_policy());
            assert_eq!(
                strip_comments(source, &plan, minimize(&config)),
                strip_comments_pinned(source, &plan, minimize(&config), |_| false),
                "{name}"
            );
        }
    }

    #[test]
    fn strip_comments_source_reports_a_parse_error() {
        let err = strip_comments_source(
            &go_config(),
            b"package main\n\nfunc f( {\n",
            &delete_every_comment(),
        )
        .unwrap_err();
        assert!(matches!(err, Error::Parse(_)), "{err}");
    }

    #[test]
    fn every_corpus_source_strips_to_an_equivalent_source() {
        let config = go_config();
        for (policy_name, policy) in [
            ("delete every comment", delete_every_comment()),
            ("the full stand-in policy", stand_in_policy()),
        ] {
            for (name, source) in CORPUS {
                let name = format!("{policy_name}: {name}");
                assert_plan_is_well_formed(
                    &plan_for(&config, source, &policy),
                    source.len(),
                    &name,
                );
                let out = strip_comments_source(&config, source, &policy).unwrap();
                let rendered = String::from_utf8_lossy(&out);
                // The artifact skips comment kinds, so removing a comment is
                // invisible to it — and so is keeping one.
                assert!(
                    equivalent(&config, source, &out).unwrap(),
                    "{name}: {rendered}"
                );
                // Blanking is length-preserving and the gap policy only ever
                // shrinks, so stripping cannot grow a file.
                assert!(out.len() <= source.len(), "{name}");
                // And it is a fixed point.
                assert_eq!(
                    strip_comments_source(&config, &out, &policy).unwrap(),
                    out,
                    "{name}"
                );
            }
        }
    }

    #[test]
    fn generated_sources_strip_to_equivalent_sources() {
        let config = go_config();
        let policy = delete_every_comment();
        let mut rng = Rng::new(0x5eed_0005);
        for case in 0..256 {
            let source = generated_source(&mut rng);
            let out = strip_comments_source(&config, &source, &policy).unwrap();
            let name = format!("generated {case}: {}", String::from_utf8_lossy(&source));
            assert!(equivalent(&config, &source, &out).unwrap(), "{name}");
            assert!(out.len() <= source.len(), "{name}");
            // Never introduces a line: a blanked comment keeps the newlines it
            // held and the gap policy emits one `\n` per run that had one.
            let lines = |bytes: &[u8]| bytes.iter().filter(|byte| **byte == b'\n').count();
            assert!(lines(&out) <= lines(&source), "{name}");
            // Nothing that survives is a comment any more.
            let rendered = String::from_utf8_lossy(&out);
            assert!(
                spans_of(&config, &out)
                    .iter()
                    .all(|span| span.kind != SpanKind::Comment),
                "{name}: {rendered}"
            );
            assert_eq!(
                strip_comments_source(&config, &out, &policy).unwrap(),
                out,
                "{name}"
            );
        }
    }
}
