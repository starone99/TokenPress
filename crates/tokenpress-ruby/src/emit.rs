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
//!   to the AST, and an embdoc's `=begin`/`=end` must stay at column 0. This
//!   is the one entry a policy may opt out of; see the comment policy below;
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
//! [`rewrite`] takes the gap policy as a parameter. [`keep`] is the identity
//! one, so [`identity`] reproduces its input byte for byte; [`minimize`] is
//! the whitespace policy, so [`minimize_source`] is the comments-kept
//! configuration. Comment stripping is *not* a gap policy — it is a
//! classification of the spans themselves, [`strip_comments_plan`], applied by
//! [`strip_comments`]; [`strip_comments_source`] composes it with [`minimize`]
//! and is the comments-stripped configuration.
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
//!
//! # The comment policy
//!
//! Comment stripping cannot be a gap policy. Comments are *collected into* the
//! protected spans, so by the time a gap policy runs, a comment is one of the
//! bytes it is forbidden to touch. It is instead a second classification of
//! the same spans — [`strip_comments_plan`] sorts every comment into the ones
//! that stay protected and the ones that are deleted outright — and
//! [`strip_comments`] removes the deleted ones before handing what is left to
//! [`rewrite`], so the bytes on either side of a vanished comment arrive at
//! the gap policy as **one** gap rather than two.
//!
//! A comment survives when either of these holds:
//!
//! - it starts inside the **magic-comment window** (below);
//! - it overlaps a protected code span, which means it is not a comment at all
//!   but text inside a literal — `"a#{1 # c\n}b"`, a heredoc body, the
//!   `__END__` data section. Deleting those bytes would edit the program.
//!
//! Nothing else is preserved, and the `__END__` section in particular is never
//! deleted: it is program-readable data through `DATA`, not commentary. It is
//! collected with the code spans, not with the comments, so no comment policy
//! can reach it.
//!
//! ## The magic-comment window
//!
//! Magic comments — `# frozen_string_literal: true`, `# encoding: …`,
//! `# shareable_constant_value: …` — change how the program is interpreted,
//! and Ruby only honours them ahead of the code. Everything before the **first
//! code token** is therefore kept, whatever it says.
//!
//! The window's end is read from the root node's location, which is exactly
//! its statements' and so starts at the first byte of code however many
//! comments and blank lines precede it. It is deliberately *not* read from
//! [`ParseResult::magic_comments`]: that is a purely lexical scan which
//! reports any `# key: value` line anywhere in the file (`crate::comparable`
//! documents the same trap), so it cannot say where the window ends.
//!
//! Consequences worth stating:
//!
//! - the **shebang** needs no special case. It sits at byte 0, so no window
//!   can exclude it;
//! - a file with **no code at all** — only comments, or only comments and an
//!   `__END__` section — has no first code token, so the window is the whole
//!   file and every comment survives;
//! - blank lines inside the window change nothing, and the window ends at the
//!   first code *token*, not at the first line that carries code: a comment
//!   trailing that first statement is already outside it;
//! - **embdocs in the window are kept too**. An `=begin` block cannot carry a
//!   magic comment, so deleting one would be safe, but the window is defined
//!   on position alone rather than on comment kind — one rule, no
//!   kind-dependent edge cases, and the cost is bounded by the leading licence
//!   block of a file.
//!
//! ## The line a comment leaves empty
//!
//! A deleted span is more than the comment: it also takes the horizontal
//! whitespace in front of it, and — when that whitespace is all that shares
//! its line — the line terminator that ends it. A line that held only a
//! comment therefore vanishes completely instead of leaving a blank line
//! behind, including where [`minimize`] could not have cleaned it up (a
//! comment line right after a heredoc terminator opens its gap, and a gap's
//! first newline is always kept). A trailing comment leaves its code line
//! otherwise untouched: `x = 1  # note` becomes `x = 1`.
//!
//! An inline comment's span swallows the `\r` of a CRLF terminator, so it is
//! handed back before either decision is taken — a surviving line keeps its
//! CRLF, a deleted line takes both bytes with it.
//!
//! ## Composition
//!
//! Deletion leaves whitespace behind — the indentation of a comment line that
//! also carried code, the blank lines that used to separate comment blocks —
//! so [`strip_comments_source`] runs [`minimize`] over the result. The two
//! stages compose in one direction only: stripping first, then minimizing.
//! [`strip_comments`] takes the gap policy as a parameter like [`rewrite`]
//! does, so the deletion half stays testable on its own with [`keep`].
//!
//! ## The over-refusal this policy adds
//!
//! Because `magic_comments()` is lexical, `crate::comparable`'s prelude
//! records a semantic-looking `# encoding: …` line *wherever* it appears —
//! including inside prose documentation deep in a file. Deleting that comment
//! changes the artifact, and the verifier refuses the file. Measured over the
//! 996 `.rb` files of the ruby 3.3.6 stdlib: 992 stripped and equivalent, 0
//! parse failures, 4 refusals (2 of them this new class: `csv.rb` and
//! `erb.rb`), **−45.8 % bytes**, and every written output also passed
//! `ruby -c`.
use std::ops::Range;use tokenpress_core::Result;use crate::parser::{self,Location,Node,ParseResult,Visit};
/// A byte range of the source that has to be reproduced verbatim.
pub type Span=Range<usize>;
/// Collects every byte range of `parsed`'s source that must survive verbatim.
///
/// The result is sorted, non-overlapping and within the source — the
/// precondition [`rewrite`] is documented to need. Spans that touch or
/// overlap are merged: the gap between two touching spans is empty, so
/// keeping them apart would only hand the policy nothing to do.
pub fn protected_spans(parsed:&ParseResult<'_>)->Vec<Span>{let mut spans=code_spans(parsed);spans.extend(parsed.comments().map(|comment|span(&comment.location())));merge(spans)}
/// The protected spans a comment policy may choose about: everything
/// [`protected_spans`] returns except the comments, sorted and merged.
fn code_spans(parsed:&ParseResult<'_>)->Vec<Span>{let mut collector=Collector{source:parsed.source(),spans:Vec::new(),};collector.visit(&parsed.node());let mut spans=collector.spans;if let Some(data)=parsed.data_loc(){spans.push(span(&data));}merge(spans)}
/// Rebuilds `source`, copying `spans` verbatim and passing every gap between
/// them — the whole of the rest of the file — through `policy`.
///
/// `spans` must be sorted, non-overlapping and within `source`, which is what
/// [`protected_spans`] returns.
pub fn rewrite(source:&[u8],spans:&[Span],mut policy:impl FnMut(&[u8])->Vec<u8>)->Vec<u8>{let mut out=Vec::with_capacity(source.len());let mut cursor=0;for span in spans{out.extend_from_slice(&policy(&source[cursor..span.start]));out.extend_from_slice(&source[span.clone()]);cursor=span.end;}out.extend_from_slice(&policy(&source[cursor..]));out}
/// The identity gap policy: the bytes between protected spans pass through
/// unchanged. The policy stages replace it; see the module docs.
pub fn keep(gap:&[u8])->Vec<u8>{gap.to_vec()}
/// Parses `source`, collects its protected spans and rewrites it with
/// [`keep`], reproducing the input byte for byte.
///
/// This is the whole pipeline with the policy seam left empty: it exists so
/// span collection is exercised end to end, and it stays an identity even
/// once the policy stages land.
///
/// Returns [`tokenpress_core::Error::Parse`] for a source prism reports
/// errors for.
pub fn identity(source:&[u8])->Result<Vec<u8>>{let parsed=parser::parse(source)?;let spans=protected_spans(&parsed);Ok(rewrite(source,&spans,keep))}
/// The whitespace-minimizing gap policy: indentation and trailing whitespace
/// go, blank-line runs collapse to one newline, every other
/// horizontal-whitespace run collapses to one space.
///
/// See the module docs for the rules and for why the start of a gap counts as
/// mid-line.
pub fn minimize(gap:&[u8])->Vec<u8>{let mut out=Vec::with_capacity(gap.len());let mut line_start=false;let mut index=0;while index<gap.len(){let rest=&gap[index..];if rest[0]==b'\\'{let unit=rest.len().min(2);out.extend_from_slice(&rest[..unit]);index+=unit;line_start=false;}else if let Some(width)=newline_len(rest){if!line_start{out.extend_from_slice(&rest[..width]);line_start=true;}index+=width;}else if is_horizontal(rest[0]){index+=rest.iter().take_while(|byte|is_horizontal(**byte)).count();if!line_start&&newline_len(&gap[index..]).is_none(){out.push(b' ');}}else{out.push(rest[0]);index+=1;line_start=false;}}out}
/// Parses `source`, collects its protected spans and rewrites it with
/// [`minimize`]: the whitespace-minimal emitter, comments kept.
///
/// Returns [`tokenpress_core::Error::Parse`] for a source prism reports
/// errors for.
pub fn minimize_source(source:&[u8])->Result<Vec<u8>>{let parsed=parser::parse(source)?;let spans=protected_spans(&parsed);Ok(rewrite(source,&spans,minimize))}
/// What a comment-stripping run does with the bytes it has classified: which
/// spans survive verbatim, and which vanish.
///
/// Both lists are sorted and non-overlapping, and no `deleted` span overlaps a
/// `protected` one — the precondition [`strip_comments`] is documented to
/// need. [`strip_comments_plan`] produces plans that satisfy it.
#[derive(Debug,PartialEq,Eq)]pub struct StripPlan{
/// Byte ranges copied verbatim: what [`protected_spans`] returns, minus
/// the comments that `deleted` removes.
pub protected:Vec<Span>,
/// Byte ranges removed outright: a comment or embdoc, the horizontal
/// whitespace in front of it, and the line terminator that ends it when
/// nothing else shares its line.
pub deleted:Vec<Span>,}
/// Classifies `parsed`'s source for comment stripping: every comment and
/// embdoc is deleted, except the ones in the magic-comment window and the ones
/// that live inside a protected literal.
///
/// See the module docs for the window and for what a deleted span covers.
pub fn strip_comments_plan(parsed:&ParseResult<'_>)->StripPlan{let source=parsed.source();let code=code_spans(parsed);let window_end=magic_comment_window_end(parsed);let mut protected=code.clone();let mut deleted=Vec::new();for comment in parsed.comments(){let comment=span(&comment.location());if comment.start<window_end||code.iter().any(|literal|overlaps(literal,&comment)){protected.push(comment);}else{deleted.push(with_its_line(source,comment));}}StripPlan{protected:merge(protected),deleted:merge(deleted),}}
/// Rebuilds `source` under `plan`: the deleted spans are dropped, the
/// protected spans are copied verbatim, and every gap between them — one
/// contiguous gap across each deletion, never two — goes through `policy`.
///
/// `plan` must satisfy the invariants [`StripPlan`] documents, which is what
/// [`strip_comments_plan`] returns.
pub fn strip_comments(source:&[u8],plan:&StripPlan,policy:impl FnMut(&[u8])->Vec<u8>,)->Vec<u8>{let(kept,spans)=delete(source,plan);rewrite(&kept,&spans,policy)}
/// Parses `source`, plans its comment stripping and rewrites what is left with
/// [`minimize`]: the comments-stripped emitter.
///
/// Returns [`tokenpress_core::Error::Parse`] for a source prism reports
/// errors for.
pub fn strip_comments_source(source:&[u8])->Result<Vec<u8>>{let parsed=parser::parse(source)?;let plan=strip_comments_plan(&parsed);Ok(strip_comments(source,&plan,minimize))}
/// Removes `plan.deleted` from `source` and remaps `plan.protected` onto the
/// bytes that are left.
///
/// A deleted span never overlaps a protected one, so a protected span only
/// ever moves: both of its ends shift by the number of bytes deleted before
/// it.
fn delete(source:&[u8],plan:&StripPlan)->(Vec<u8>,Vec<Span>){let mut kept=Vec::with_capacity(source.len());let mut cursor=0;for span in&plan.deleted{kept.extend_from_slice(&source[cursor..span.start]);cursor=span.end;}kept.extend_from_slice(&source[cursor..]);let mut spans=Vec::with_capacity(plan.protected.len());let mut removed=0;let mut next=0;for span in&plan.protected{while next<plan.deleted.len()&&plan.deleted[next].end<=span.start{removed+=plan.deleted[next].len();next+=1;}spans.push(span.start-removed..span.end-removed);}(kept,spans)}
/// The end of the magic-comment window: the offset of the first code token, or
/// the length of the source when there is no code at all.
///
/// The root node's location is exactly its statements', so it starts at the
/// first byte of code however many comments and blank lines precede it, and it
/// is empty — start equal to end — when the file holds no statement to locate.
fn magic_comment_window_end(parsed:&ParseResult<'_>)->usize{let program=parsed.node().location();if program.start_offset()==program.end_offset(){parsed.source().len()}else{program.start_offset()}}
/// Grows a comment's span over the bytes deleting it would otherwise leave
/// behind: the horizontal whitespace in front of it and, when that whitespace
/// is all that shares its line, the line terminator that ends it.
///
/// The backward scan cannot walk into a protected span: every one of them ends
/// with a delimiter, a terminator line or the end of the file, never with the
/// horizontal whitespace this would have to cross.
fn with_its_line(source:&[u8],comment:Span)->Span{let mut start=comment.start;while start>0&&is_horizontal(source[start-1]){start-=1;}let mut end=comment.end;if source[..end].ends_with(b"\r")&&source[end..].starts_with(b"\n"){end-=1;}if start==0||source[start-1]==b'\n'{end+=newline_len(&source[end..]).unwrap_or(0);}start..end}
/// Whether two spans share at least one byte.
fn overlaps(left:&Span,right:&Span)->bool{left.start<right.end&&right.start<left.end}
/// The whitespace a run is made of. Only spaces and tabs: every other byte
/// Ruby happens to accept as whitespace is rare enough that leaving it alone
/// costs nothing and guessing about it could cost correctness.
fn is_horizontal(byte:u8)->bool{byte==b' '||byte==b'\t'}
/// Width of the line terminator starting `bytes`, or `None` when `bytes` does
/// not start with one. A lone `\r` is not a terminator here — see the module
/// docs.
fn newline_len(bytes:&[u8])->Option<usize>{match bytes{[b'\n',..]=>Some(1),[b'\r',b'\n',..]=>Some(2),_=>None,}}
/// The `Visit` pass that records literal spans. It is generic over node kind
/// — the two `*_node_enter` hooks see *every* node — so a construct is
/// classified by what its own location and delimiters say, never by where in
/// the tree it was found.
struct Collector<'pr>{source:&'pr[u8],spans:Vec<Span>,}impl<'pr>Collector<'pr>{fn record(&mut self,node:&Node<'pr>){if let Some(node)=node.as_string_node(){self.spans.push(span(&node.location()));self.heredoc(node.opening_loc(),node.closing_loc());}else if let Some(node)=node.as_interpolated_string_node(){self.spans.push(span(&node.location()));self.heredoc(node.opening_loc(),node.closing_loc());}else if let Some(node)=node.as_x_string_node(){self.spans.push(span(&node.location()));self.heredoc(Some(node.opening_loc()),Some(node.closing_loc()));}else if let Some(node)=node.as_interpolated_x_string_node(){self.spans.push(span(&node.location()));self.heredoc(Some(node.opening_loc()),Some(node.closing_loc()));}else if let Some(node)=node.as_symbol_node(){self.spans.push(span(&node.location()));}else if let Some(node)=node.as_interpolated_symbol_node(){self.spans.push(span(&node.location()));}else if let Some(node)=node.as_regular_expression_node(){self.spans.push(span(&node.location()));}else if let Some(node)=node.as_interpolated_regular_expression_node(){self.spans.push(span(&node.location()));}else if let Some(node)=node.as_match_last_line_node(){self.spans.push(span(&node.location()));}else if let Some(node)=node.as_interpolated_match_last_line_node(){self.spans.push(span(&node.location()));}else if let Some(node)=node.as_array_node(){if node.opening_loc().is_some_and(|opening|opening.as_slice().starts_with(b"%")){self.spans.push(span(&node.location()));}}}
/// Records a heredoc's body and terminator when `opening` is a heredoc
/// delimiter. See the module docs for the derivation.
fn heredoc(&mut self,opening:Option<Location<'pr>>,closing:Option<Location<'pr>>){let(Some(opening),Some(closing))=(opening,closing)else{return;};if opening.as_slice().starts_with(b"<<"){let after_marker=opening.end_offset();let to_line_end=self.source[after_marker..].iter().take_while(|byte|**byte!=b'\n').count();self.spans.push(after_marker+to_line_end..closing.end_offset());}}}impl<'pr>Visit<'pr>for Collector<'pr>{fn visit_branch_node_enter(&mut self,node:Node<'pr>){self.record(&node);}fn visit_leaf_node_enter(&mut self,node:Node<'pr>){self.record(&node);}}fn span(location:&Location<'_>)->Span{location.start_offset()..location.end_offset()}
/// Sorts `spans` and merges every pair that overlaps or touches.
fn merge(mut spans:Vec<Span>)->Vec<Span>{spans.sort_by_key(|span|span.start);let mut merged:Vec<Span> =Vec::with_capacity(spans.len());for span in spans{match merged.last_mut(){Some(last)if span.start<=last.end=>last.end=last.end.max(span.end),_=>merged.push(span),}}merged}#[cfg(test)]mod tests{use super::*;use tokenpress_core::Error;
/// Every construct whose bytes the emitter may not touch. Byte strings,
/// not `&str`: a Ruby source is a byte sequence and one of these is not
/// valid UTF-8.
const HAZARDS:&[(&str,&[u8])]=&[("plain heredoc",b"x = <<EOS\nbody\nEOS\n"),("dash heredoc",b"x = <<-EOS\n  body\n  EOS\n"),("squiggly heredoc",b"x = <<~EOS\n  body\nEOS\n"),("single quoted heredoc marker",b"x = <<~'EOS'\n  a#{b}\nEOS\n",),("double quoted heredoc marker",b"x = <<\"EOS\"\n  body\nEOS\n",),("two heredocs on one line",b"a = <<~A; b = <<~B\n  one\nA\n  two\nB\n",),("heredoc with interpolation",b"x = <<~EOS\n  a#{1 + 2}b\nEOS\n",),("heredoc with a method call",b"x = <<~EOS.strip\n  body\nEOS\n",),("heredoc with a chained method call",b"x = <<~A.strip.upcase\n  body\nA\n",),("heredoc as an argument",b"p(<<~A, 1)\n  hi\nA\n"),("heredoc argument followed by a block",b"foo(<<~A) do\n  b\nA\n  bar\nend\n",),("heredoc nested in an interpolation",b"a = <<~A\n  #{<<~B}\n  inner\nB\n  rest\nA\n",),("empty heredoc",b"x = <<~A\nA\n"),("backtick heredoc",b"x = <<~`A`\n  echo\nA\n"),("heredoc body that looks like a comment",b"x = <<~A\n  # no\nA\n",),("heredoc body that looks like an embdoc",b"x = <<~A\n=begin\nno\n=end\nA\n",),("comment after a heredoc marker",b"x = <<~A # note\n  body\nA\n",),("heredoc before __END__",b"x = <<~A\n  b\nA\n__END__\ndata\n",),("word list",b"x = %w[a   b]\n"),("symbol list",b"x = %i[a   b]\n"),("interpolated word list",b"x = %W[a#{b}   c]\n"),("interpolated symbol list",b"x = %I[a#{b}   c]\n"),("multi-line word list",b"x = %w[a\n   b]\n"),("empty word list",b"x = %w[]\n"),("percent q string",b"x = %q(a   b)\n"),("percent big q string",b"x = %Q(a   #{b})\n"),("percent regexp",b"x = %r{a   b}i\n"),("plain regexp",b"x = /a   b/\n"),("interpolated regexp",b"x = /a#{b}   c/\n"),("match last line regexp",b"if /a   b/\n  1\nend\n"),("interpolated match last line regexp",b"if /a#{b}   c/\n  1\nend\n",),("backtick xstring",b"x = `echo   hi`\n"),("interpolated backtick xstring",b"x = `echo   #{a}`\n"),("percent x xstring",b"x = %x(echo   hi)\n"),("single quoted string",b"x = 'a  \\n  b'\n"),("double quoted string",b"x = \"a  \\t  b\"\n"),("string interpolation",b"x = \"a#{b}c\"\n"),("adjacent strings over a continuation",b"x = \"a\" \\\n  \"b\"\n",),("character literal",b"c = ?a\n"),("plain symbol",b"x = :sym\n"),("quoted symbol",b"x = :\"quoted   sym\"\n"),("percent s symbol",b"x = %s(w)\n"),("interpolated symbol",b"x = :\"a#{b}c\"\n"),("comment inside an interpolation",b"x = \"a#{1 # c\n}b\"\n"),("comment inside a heredoc interpolation",b"x = <<~A\n  #{1 # c\n  }\nA\n",),("__END__ with data",b"x = 1\n__END__\ndata here\n"),("__END__ without a trailing newline",b"x = 1\n__END__"),("__END__ with nothing after it",b"x = 1\n__END__\n"),("embdoc",b"=begin\nhi\n=end\nx = 1\n"),("shebang and magic comment",b"#!/usr/bin/env ruby\n# frozen_string_literal: true\nx = 1\n",),("inline comments",b"x = 1 # c1\n# c2\ny = 2\n"),("non-utf8 comment",b"# \xe9\nx = 1\n"),("non-utf8 string under a magic comment",b"# encoding: binary\nx = \"\xff\xfe\"\n",),("plain array",b"x = [1,   2]\n"),("implicit array",b"y = 1,   2\n"),("no literals at all",b"def f(a, b)\n  a + b\nend\n"),("bare percent string",b"x = %(a   b)\n"),("empty source",b""),("everything at once",b"#!/usr/bin/env ruby\n\
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
              trailing   data\n",),("crlf line endings",b"x = 1\r\ny = 2\r\n"),("crlf heredoc",b"x = <<~A\r\n  b\r\nA\r\n"),];
/// A length-preserving gap policy: everything outside a protected span is
/// upper-cased. Length preservation is what lets the assertions compare
/// the *same* byte offsets before and after.
fn upcase(gap:&[u8])->Vec<u8>{gap.to_ascii_uppercase()}
/// A gap policy that changes length: runs of spaces collapse to one.
fn collapse_spaces(gap:&[u8])->Vec<u8>{let mut out:Vec<u8> =Vec::new();for byte in gap{if!(*byte==b' '&&out.last()==Some(&b' ')){out.push(*byte);}}out}fn spans_of(source:&[u8])->Vec<Span>{let parsed=parser::parse(source).unwrap();protected_spans(&parsed)}fn rewritten(source:&[u8],policy:impl FnMut(&[u8])->Vec<u8>)->Vec<u8>{rewrite(source,&spans_of(source),policy)}#[test]fn every_hazard_survives_the_identity_rewriter_byte_for_byte(){for(name,source)in HAZARDS{let out=identity(source).unwrap();assert_eq!(out,*source,"{name}");}}#[test]fn every_hazard_keeps_its_protected_bytes_under_a_gap_policy(){for(name,source)in HAZARDS{let spans=spans_of(source);let out=rewritten(source,upcase);assert_eq!(out.len(),source.len(),"{name}");for span in&spans{assert_eq!(out[span.clone()],source[span.clone()],"{name} {span:?}");}}}#[test]fn every_hazard_yields_ordered_disjoint_in_bounds_spans(){for(name,source)in HAZARDS{let spans=spans_of(source);let mut previous=0;for span in&spans{assert!(span.start>=previous,"{name}: {span:?} out of order");assert!(span.start<=span.end,"{name}: {span:?} inverted");assert!(span.end<=source.len(),"{name}: {span:?} out of bounds");previous=span.end;}}}#[test]fn a_heredoc_span_runs_from_the_marker_line_end_to_the_terminator(){let source=b"x = <<~EOS.strip\n  body\nEOS\n";assert_eq!(spans_of(source),vec![4..10,16..28]);assert_eq!(&source[4..10],b"<<~EOS");assert_eq!(&source[16..28],b"\n  body\nEOS\n");}#[test]fn the_code_after_a_heredoc_marker_stays_in_a_gap(){let out=rewritten(b"x = <<~EOS.strip\n  body\nEOS\n",upcase);assert_eq!(out,b"X = <<~EOS.STRIP\n  body\nEOS\n");}#[test]fn two_heredocs_on_one_line_merge_into_one_span(){let source=b"a = <<~A; b = <<~B\n  one\nA\n  two\nB\n";assert_eq!(spans_of(source),vec![4..8,14..35]);assert_eq!(&source[14..35],b"<<~B\n  one\nA\n  two\nB\n");}#[test]fn an_empty_heredoc_body_still_protects_the_terminator(){let source=b"x = <<~A\nA\ny = 2\n";assert_eq!(spans_of(source),vec![4..11]);assert_eq!(&source[4..11],b"<<~A\nA\n");}#[test]fn a_comment_after_a_marker_merges_with_the_heredoc_region(){let source=b"x = <<~A # note\n  body\nA\n";assert_eq!(spans_of(source),vec![4..8,9..25]);assert_eq!(&source[9..25],b"# note\n  body\nA\n");}#[test]fn a_comment_span_is_exactly_the_comment(){let source=b"x = 1 # c1\n# c2\ny = 2\n";assert_eq!(spans_of(source),vec![6..10,11..15]);assert_eq!(&source[6..10],b"# c1");assert_eq!(&source[11..15],b"# c2");}#[test]fn an_embdoc_span_covers_the_whole_block(){let source=b"=begin\nhi\n=end\nx = 1\n";assert_eq!(spans_of(source),vec![0..15]);assert_eq!(&source[0..15],b"=begin\nhi\n=end\n");}#[test]fn the_data_section_is_protected(){let source=b"x = 1\n__END__\ndata here\n";assert_eq!(spans_of(source),vec![6..24]);assert_eq!(&source[6..24],b"__END__\ndata here\n");}#[test]fn a_word_list_is_protected_whole_so_its_separators_survive(){let source=b"x = %w[a   b]\n";assert_eq!(spans_of(source),vec![4..13]);assert_eq!(rewritten(source,collapse_spaces),b"x = %w[a   b]\n");}#[test]fn a_plain_array_is_not_protected(){let source=b"x = [1,   2]\n";assert!(spans_of(source).is_empty());assert_eq!(rewritten(source,collapse_spaces),b"x = [1, 2]\n");}#[test]fn an_implicit_array_is_not_protected(){assert!(spans_of(b"y = 1,   2\n").is_empty());}#[test]fn overlapping_spans_merge(){let source=b"x = \"a#{1 # c\n}b\"\n";assert_eq!(spans_of(source),vec![4..17]);assert_eq!(&source[4..17],b"\"a#{1 # c\n}b\"");}#[test]fn a_gap_policy_reaches_the_code_around_a_literal(){assert_eq!(rewritten(b"x = 'a  b'\n",upcase),b"X = 'a  b'\n");assert_eq!(rewritten(b"def f\n  x = 'a  b'\nend\n",collapse_spaces),b"def f\n x = 'a  b'\nend\n");}#[test]fn keep_returns_the_gap_unchanged(){assert_eq!(keep(b"  a  "),b"  a  ");}#[test]fn rewrite_with_no_spans_is_just_the_policy(){assert_eq!(rewrite(b"a b",&[],upcase),b"A B");}#[test]fn identity_reports_a_parse_error(){let err=identity(b"def ; end").unwrap_err();assert!(matches!(err,Error::Parse(_)),"{err}");}#[test]fn minimize_strips_line_indentation(){assert_eq!(minimize(b"a\n    b\n\tc\n"),b"a\nb\nc\n");}#[test]fn minimize_strips_trailing_whitespace(){assert_eq!(minimize(b"a   \nb\t\n"),b"a\nb\n");}#[test]fn minimize_strips_a_whitespace_run_that_ends_the_gap_after_a_newline(){assert_eq!(minimize(b"a\n   "),b"a\n");}#[test]fn minimize_collapses_blank_line_runs_to_one_newline(){assert_eq!(minimize(b"a\n\n\n\nb\n"),b"a\nb\n");assert_eq!(minimize(b"a\n  \n\t\nb\n"),b"a\nb\n");assert_eq!(minimize(b"\n\n\na\n"),b"\na\n");}#[test]fn minimize_keeps_the_newline_that_ends_a_statement(){assert_eq!(minimize(b"a = 1\nb = 2\n"),b"a = 1\nb = 2\n");}#[test]fn minimize_collapses_intra_line_whitespace_to_one_space(){assert_eq!(minimize(b"a   =\t\t1\n"),b"a = 1\n");}#[test]fn minimize_never_collapses_a_space_run_to_nothing(){assert_eq!(minimize(b"a  -  b\n"),b"a - b\n");assert_eq!(minimize(b"defined?  x\n"),b"defined? x\n");assert_eq!(minimize(b"not  x\n"),b"not x\n");assert_eq!(minimize(b"p   "),b"p ");}#[test]fn minimize_treats_the_start_of_a_gap_as_mid_line(){assert_eq!(minimize(b"    a\n"),b" a\n");}#[test]fn minimize_keeps_a_backslash_and_the_byte_after_it_verbatim(){assert_eq!(minimize(b"a + \\\n  b\n"),b"a + \\\n b\n");assert_eq!(minimize(b"a \\"),b"a \\");}#[test]fn minimize_preserves_crlf_line_endings(){assert_eq!(minimize(b"a = 1\r\nb = 2\r\n"),b"a = 1\r\nb = 2\r\n");assert_eq!(minimize(b"a   \r\n   b\r\n"),b"a\r\nb\r\n");assert_eq!(minimize(b"a\r\n\r\n\r\nb\r\n"),b"a\r\nb\r\n");}#[test]fn minimize_copies_a_carriage_return_that_ends_no_line(){assert_eq!(minimize(b"x = <<~A\r"),b"x = <<~A\r");}#[test]fn minimize_of_an_empty_gap_is_empty(){assert_eq!(minimize(b""),b"");}#[test]fn minimize_source_rewrites_only_the_gaps(){assert_eq!(minimize_source(b"def f(a,   b)\n    a  +  b\nend\n").unwrap(),b"def f(a, b)\na + b\nend\n");}#[test]fn minimize_source_keeps_word_list_separators_but_not_array_spacing(){assert_eq!(minimize_source(b"x = %w[a   b]\n").unwrap(),b"x = %w[a   b]\n");assert_eq!(minimize_source(b"x = [1,   2]\n").unwrap(),b"x = [1, 2]\n");}#[test]fn minimize_source_leaves_a_heredoc_body_alone_while_minimizing_its_marker_line(){assert_eq!(minimize_source(b"x   =   <<~EOS.strip\n  body\nEOS\n").unwrap(),b"x = <<~EOS.strip\n  body\nEOS\n");}#[test]fn minimize_source_keeps_comments_and_the_newline_before_a_column_zero_construct(){assert_eq!(minimize_source(b"x = 1   # c\n\n\n=begin\nd\n=end\n\n\n__END__\ndata\n").unwrap(),b"x = 1 # c\n=begin\nd\n=end\n\n__END__\ndata\n");}#[test]fn minimize_source_reports_a_parse_error(){let err=minimize_source(b"def ; end").unwrap_err();assert!(matches!(err,Error::Parse(_)),"{err}");}#[test]fn every_hazard_stays_equivalent_under_minimization(){for(name,source)in HAZARDS{let out=minimize_source(source).unwrap();let equivalent=crate::comparable::equivalent(source,&out).unwrap();assert!(equivalent,"{name}: {}",String::from_utf8_lossy(&out));}}fn plan_of(source:&[u8])->StripPlan{let parsed=parser::parse(source).unwrap();strip_comments_plan(&parsed)}
/// Comment stripping with the identity gap policy: only the comments and
/// the bytes they leave behind move, so the assertion is byte-exact.
fn stripped(source:&[u8])->Vec<u8>{strip_comments(source,&plan_of(source),keep)}#[test]fn a_plan_deletes_a_comment_with_the_whitespace_in_front_of_it(){let source=b"x = 1 # c\n# d\ny = \"s\" # e\n";let plan=plan_of(source);assert_eq!(plan.protected,vec![18..21]);assert_eq!(plan.deleted,vec![5..9,10..14,21..25]);assert_eq!(&source[5..9],b" # c");assert_eq!(&source[10..14],b"# d\n");assert_eq!(&source[21..25],b" # e");}#[test]fn stripping_a_trailing_comment_leaves_the_code_line_intact(){assert_eq!(stripped(b"x = 1  # note\ny = 2\n"),b"x = 1\ny = 2\n");assert_eq!(stripped(b"x = 1  # note"),b"x = 1");}#[test]fn stripping_a_full_line_comment_removes_the_line_it_leaves_empty(){assert_eq!(stripped(b"x = 1\n  # c\ny = 2\n"),b"x = 1\ny = 2\n");assert_eq!(stripped(b"x = 1\n# a\n# b\ny = 2\n# c\n"),b"x = 1\ny = 2\n");}#[test]fn stripping_a_comment_line_after_a_heredoc_leaves_no_blank_line(){assert_eq!(stripped(b"x = <<~A\nb\nA\n# c\ny = 2\n"),b"x = <<~A\nb\nA\ny = 2\n");}#[test]fn the_shebang_survives_stripping(){assert_eq!(stripped(b"#!/usr/bin/env ruby\nx = 1 # c\n"),b"#!/usr/bin/env ruby\nx = 1\n");}#[test]fn magic_comments_before_the_first_code_token_survive_stripping(){let source=b"#!/usr/bin/env ruby\n\
                       # frozen_string_literal: true\n\
                       # encoding: utf-8\n\
                       x = 1 # gone\n";assert_eq!(stripped(source),b"#!/usr/bin/env ruby\n\
              # frozen_string_literal: true\n\
              # encoding: utf-8\n\
              x = 1\n");}#[test]fn the_window_survives_blank_lines_between_magic_comments(){let source=b"# frozen_string_literal: true\n\n# encoding: utf-8\n\nx = 1 # gone\n";assert_eq!(stripped(source),b"# frozen_string_literal: true\n\n# encoding: utf-8\n\nx = 1\n");}#[test]fn the_window_ends_at_the_first_code_token_not_the_first_line(){let source=b"# frozen_string_literal: true\nx = 1\n# frozen_string_literal: true\n";assert_eq!(stripped(source),b"# frozen_string_literal: true\nx = 1\n");}#[test]fn a_file_of_only_comments_keeps_every_comment(){assert_eq!(stripped(b"# a\n\n# b\n"),b"# a\n\n# b\n");assert_eq!(stripped(b"=begin\nlicense\n=end\n"),b"=begin\nlicense\n=end\n");}#[test]fn an_embdoc_after_the_first_code_token_is_deleted_whole(){assert_eq!(stripped(b"x = 1\n=begin\ndoc\n=end\ny = 2\n"),b"x = 1\ny = 2\n");assert_eq!(stripped(b"x = 1\n=begin\ndoc\n=end"),b"x = 1\n");}#[test]fn an_embdoc_before_the_first_code_token_survives(){assert_eq!(stripped(b"=begin\ndoc\n=end\nx = 1 # gone\n"),b"=begin\ndoc\n=end\nx = 1\n");}#[test]fn the_data_section_survives_stripping_byte_for_byte(){assert_eq!(stripped(b"x = 1 # c\n__END__\n# not a comment\ndata   here\n"),b"x = 1\n__END__\n# not a comment\ndata   here\n");}#[test]fn a_comment_inside_a_literal_is_protected_not_deleted(){assert_eq!(stripped(b"x = \"a#{1 # c\n}b\" # gone\n"),b"x = \"a#{1 # c\n}b\"\n");}#[test]fn a_comment_on_a_heredoc_marker_line_is_deleted(){assert_eq!(stripped(b"x = <<~A # note\n  body\nA\n"),b"x = <<~A\n  body\nA\n");}#[test]fn stripping_keeps_crlf_line_endings(){assert_eq!(stripped(b"x = 1 # c\r\ny = 2\r\n"),b"x = 1\r\ny = 2\r\n");assert_eq!(stripped(b"x = 1\r\n# c\r\ny = 2\r\n"),b"x = 1\r\ny = 2\r\n");}#[test]fn non_utf8_comments_are_stripped_and_kept_like_any_other(){assert_eq!(stripped(b"# \xe9\nx = 1\n"),b"# \xe9\nx = 1\n");assert_eq!(stripped(b"x = 1 # \xe9\n"),b"x = 1\n");assert_eq!(stripped(b"# encoding: binary\nx = \"\xff\xfe\" # c\n"),b"# encoding: binary\nx = \"\xff\xfe\"\n");}#[test]fn strip_comments_source_also_minimizes_the_gaps(){assert_eq!(strip_comments_source(b"def f(a,   b)\n\n  # c\n\n    a  +  b\nend\n").unwrap(),b"def f(a, b)\na + b\nend\n");}#[test]fn strip_comments_source_reports_a_parse_error(){let err=strip_comments_source(b"def ; end").unwrap_err();assert!(matches!(err,Error::Parse(_)),"{err}");}#[test]fn every_hazard_yields_a_disjoint_in_bounds_strip_plan(){for(name,source)in HAZARDS{let plan=plan_of(source);for spans in[&plan.protected,&plan.deleted]{let mut previous=0;for span in spans{assert!(span.start>=previous,"{name}: {span:?} out of order");assert!(span.start<span.end,"{name}: {span:?} empty");assert!(span.end<=source.len(),"{name}: {span:?} out of bounds");previous=span.end;}}for deleted in&plan.deleted{for protected in&plan.protected{assert!(deleted.end<=protected.start||protected.end<=deleted.start,"{name}: {deleted:?} overlaps {protected:?}");}}}}#[test]fn every_hazard_keeps_its_protected_bytes_under_stripping(){for(name,source)in HAZARDS{let plan=plan_of(source);let out=stripped(source);for span in&plan.protected{let bytes=&source[span.clone()];let kept=out.windows(bytes.len()).any(|window|window==bytes);assert!(kept,"{name}: {span:?} lost");}}}#[test]fn every_hazard_stays_equivalent_under_comment_stripping(){for(name,source)in HAZARDS{let out=strip_comments_source(source).unwrap();let equivalent=crate::comparable::equivalent(source,&out).unwrap();assert!(equivalent,"{name}: {}",String::from_utf8_lossy(&out));}}}