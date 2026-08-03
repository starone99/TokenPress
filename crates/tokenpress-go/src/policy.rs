//! The Go comment hazard surface: which comments carry meaning, which head
//! of the file has to survive verbatim, and which files may not be touched
//! at all.
//!
//! Everything here composes into one [`tokenpress_treesitter::emit::CommentPolicy`]
//! ([`comment_policy`]) plus one column-0 pinning predicate
//! ([`is_promotable_directive`]) — the two language-specific inputs the
//! engine's comment stripper takes. Nothing in this module parses; it reads
//! comment bytes and the tree the engine already produced.
//!
//! # Why a comment is not decoration in Go
//!
//! Go has no pragma syntax. Every compiler, linker, `go` subcommand and cgo
//! instruction rides in a comment, so deleting a comment can change the
//! program — and the engine's equivalence artifact is comment-blind by
//! construction, so it can never report the change. This module is the whole
//! defence, and each rule below is pinned by a test that cites what measured
//! it against the go1.24.7 toolchain in the measurement container.
//!
//! # The three kinds of hazard
//!
//! 1. **Deletion.** A directive that disappears takes its effect with it,
//!    usually silently: dropping `//go:embed data.txt` leaves a variable that
//!    compiles and is empty (measured: `go run` prints `[]` instead of
//!    `[hello]`). [`is_semantic_comment`] is the keep predicate.
//! 2. **Promotion.** Collapsing indentation moves a comment to column 0, and
//!    some directives are only directives at column 0. [`is_directive_comment`]
//!    answers the shape question; [`is_promotable_directive`] adds the
//!    position that `is_directive_comment` cannot see. See below.
//! 3. **Reformatting the head of the file.** A build constraint's *blank
//!    line* is part of its meaning, and the whitespace policy would collapse
//!    it. [`build_constraint_prologue`] answers the region that has to be
//!    reproduced byte for byte.
//!
//! On top of those, cgo is not a hazard that can be narrowed: the comment
//! before `import "C"` **is** C source and is compiled. [`imports_c`] is the
//! whole-file bail-out.
//!
//! # The column-0 split
//!
//! `//line` and `//go:generate` are read as directives only when the `//`
//! starts a line, so an indented one is an ordinary comment and moving it to
//! column 0 changes the program. The two halves of that rule live apart on
//! purpose:
//!
//! - [`is_directive_comment`] is handed a comment's **own bytes** — it is
//!   the [`CommentPolicy`] keep
//!   callback's view of the world — and mirrors the Go toolchain's
//!   `isDirective` exactly. It cannot see where the comment sits, so it
//!   deliberately does not try to;
//! - [`is_promotable_directive`] is handed the **source and the span**, which
//!   is the engine's pinning-hook view
//!   ([`rewrite_pinned`](tokenpress_treesitter::emit::rewrite_pinned) /
//!   [`strip_comments_pinned`](tokenpress_treesitter::emit::strip_comments_pinned)
//!   take `FnMut(Span) -> bool`, and the source is captured by the closure at
//!   the call site). It is the only place the column is known, so it is the
//!   only place the column rule can live.
//!
//! Measured, go1.24.7: `go/scanner/scanner.go` honours a `//line` directive
//! only when `offs == s.lineOffset`, and `cmd/compile/internal/syntax/parser.go`
//! only when `col == colbase` — two independent implementations of the same
//! rule. `cmd/go/internal/generate/generate.go`'s `isGoGenerate` is stricter
//! still: it tests `bytes.HasPrefix(line, "//go:generate ")` against a whole
//! line, so any indentation at all hides the directive.
//!
//! The pin is applied to every `//`-form directive rather than only to the
//! position-sensitive ones. Over-applying costs one space; under-applying
//! costs a program.
//!
//! # Block comments
//!
//! `/*line …*/` **is** a directive, and unlike `//line` it is honoured at any
//! column (`lit[1] == '*' || offs == s.lineOffset` in `go/scanner`;
//! `col == colbase || msg[1] == '*'` in the compiler's parser). It therefore
//! has to be kept, but it can never be *promoted*, so it needs no pin.
//!
//! There is no `/*go:…*/` form and this module does not invent one.
//! `cmd/compile/internal/syntax/scanner.go`'s `fullComment` recognises
//! exactly one block prefix, `"line "`, and measurement agrees:
//! `/*go:embed data.txt*/` above a `var` yields an empty string, and
//! `/*go:build ignoreme*/` does not keep the file out of `GoFiles`. A
//! block-form `go:` comment is decoration and is deleted like any other.

use std::ops::Range;

use tokenpress_treesitter::emit::{CommentPolicy, Span, SpanKind};
use tokenpress_treesitter::parser::{Node, Tree};

/// True when a comment has to survive stripping.
///
/// The keep half of the policy: [`CommentPolicy`]'s `keep_comment` callback,
/// handed a comment span's own source bytes (leading `//` or `/*` included,
/// trailing newline not — the grammar's `comment` node stops before it).
///
/// Three families survive:
///
/// - anything the Go toolchain reads as a `//`-form directive
///   ([`is_directive_comment`]), which covers `//go:build`, `//go:generate`,
///   `//go:embed`, `//go:noinline`, `//go:linkname`, `//line`, `//export`
///   (cgo) and `//extern` (gccgo), and third-party ones of the same shape
///   such as `//lint:ignore`;
/// - the block line directive `/*line …*/`, which `isDirective` does not
///   cover because the toolchain only ever asks it about `//` comments;
/// - the legacy `// +build` constraint, which is not directive-shaped at all
///   (no colon) and so is invisible to `isDirective`.
///
/// Everything else is decoration. Note that a `//go:build` line is kept twice
/// over: by this predicate and by [`build_constraint_prologue`], because a
/// keep flag alone would still let the whitespace policy collapse the blank
/// line the legacy form needs.
pub fn is_semantic_comment(bytes: &[u8]) -> bool {
    is_directive_comment(bytes) || bytes.starts_with(b"/*line ") || is_build_constraint(bytes)
}

/// True when `bytes` is a `//`-form comment the Go toolchain reads as a
/// directive, ignoring where it sits.
///
/// A faithful port of `isDirective` as read in
/// `/usr/local/go/src/go/ast/ast.go:162` (go1.24.7), which
/// `/usr/local/go/src/go/printer/comment.go:113` duplicates verbatim and
/// labels "This code is also in go/ast". It is unexported in both, so the
/// name `ast.IsDirective` names the rule rather than an API. With the leading
/// `//` removed, a comment is a directive when it
///
/// - starts with `line `, `extern ` or `export ` — note the required trailing
///   space, so a bare `//line` is not one; or
/// - matches `[a-z0-9]+:[a-z0-9]`, measured against the **first** colon: the
///   colon may not be at index 0, at least one byte must follow it, and every
///   byte before it plus the single byte after it must be a lowercase ASCII
///   letter or digit.
///
/// A block comment is never a directive here, because the toolchain never
/// asks: `go/ast`'s caller strips `//` first and `go/printer` only calls it
/// from its `//`-comment branch. `/*line …*/` is handled by
/// [`is_semantic_comment`] instead.
pub fn is_directive_comment(bytes: &[u8]) -> bool {
    let Some(text) = bytes.strip_prefix(b"//") else {
        return false;
    };
    if text.starts_with(b"line ") || text.starts_with(b"extern ") || text.starts_with(b"export ") {
        return true;
    }
    let Some(colon) = text.iter().position(|byte| *byte == b':') else {
        return false;
    };
    if colon == 0 || colon + 1 >= text.len() {
        return false;
    }
    // Go walks `i <= colon+1` and skips `i == colon`, so the inspected window
    // is every byte before the colon plus the one directly after it.
    text[..=colon + 1]
        .iter()
        .enumerate()
        .all(|(index, byte)| index == colon || byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

/// True when emitting `span` at the start of a line would turn an ordinary
/// comment into a directive.
///
/// The Go instance of the engine's `never_starts_a_line` predicate; a call
/// site partially applies it, `|span| is_promotable_directive(source, span)`.
/// Blanking is length-preserving, so `span` indexes `source` itself whether
/// or not comments have been stripped.
///
/// True for a comment that is directive-shaped and is **not** already at the
/// start of a line in the source — the only case where the rewriter's
/// collapsing of indentation could change the program. A comment already at
/// column 0 is already a directive and must stay where it is, and a comment
/// at byte 0 is at column 0 by definition.
///
/// The span kind is tested first even though a Go literal's bytes can never
/// begin with `//`: the predicate is offered *every* span, and deciding a
/// protected literal's fate by pattern-matching its bytes would be wrong in
/// principle even where it happens to be right in practice.
pub fn is_promotable_directive(source: &[u8], span: Span) -> bool {
    span.kind == SpanKind::Comment
        && !starts_a_line(source, span.start)
        && is_directive_comment(&source[span.range()])
}

/// The head of the file that has to be reproduced byte for byte, or an empty
/// range when there is none.
///
/// [`CommentPolicy`]'s `prologue` callback. The region is
/// `[0 .. package_clause.start_byte())` — everything before the package
/// clause — and it is claimed only when that region carries a build
/// constraint of either generation.
///
/// Protecting the *region* rather than the constraint comment is the whole
/// point. `go/build`'s `parseFileHeader` accepts a legacy `// +build` line
/// only when a blank line separates the header from the package clause, and
/// the whitespace policy collapses blank lines; measured with `go list` on
/// go1.24.7, losing that one blank line moves the file out of
/// `IgnoredGoFiles` and into `GoFiles`, which is a build the user did not ask
/// for. A keep flag on the comment cannot defend the whitespace around it.
///
/// The region therefore also swallows whatever else sits in the header — a
/// second constraint line, a licence banner, the package doc comment. That is
/// a deliberate lost saving on the small minority of files that carry a build
/// constraint, in exchange for a rule with no seams in it.
///
/// Both anchor failures measured in G1 yield an empty range rather than a
/// panic: a source with **no package clause** parses, and an **empty file**
/// parses with zero children, so there is no node to anchor on in either
/// case.
pub fn build_constraint_prologue(tree: &Tree, source: &[u8]) -> Range<usize> {
    let Some(end) = package_clause_start(tree) else {
        return 0..0;
    };
    if source[..end]
        .split(|byte| *byte == b'\n')
        .any(is_build_constraint)
    {
        0..end
    } else {
        0..0
    }
}

/// True when the file imports `"C"` and must therefore be left byte-identical.
///
/// [`CommentPolicy`]'s `bail_out` callback. In a cgo file the comment
/// immediately preceding `import "C"` is the **preamble**: it is C source and
/// it is compiled. Measured on go1.24.7, deleting it turns a building program
/// into `could not determine what C.hello refers to`, and no comment-blind
/// equivalence check could have seen the difference. Worse, `#cgo` lines in
/// that preamble carry compiler and linker flags, so even reflowing it is not
/// safe. There is no narrower rule worth having, so the whole file is left
/// alone.
///
/// Detection is structural: an `import_spec` node whose own bytes contain the
/// import path, in either the single form (`import "C"`) or the grouped one
/// (`import ( … "C" … )`). Because it reads import specs rather than string
/// literals, a `"C"` used as an ordinary value and an import path that merely
/// ends in `/C` are both left alone.
///
/// Both quotations of the path count. Measured: `` import `C` `` is reported
/// by `go list` in `CgoFiles` — `go/build` unquotes the path before comparing
/// — even though `cmd/cgo` then fails to resolve `C.` references in it. It is
/// a cgo file to the toolchain, so it is one here.
pub fn imports_c(tree: &Tree, source: &[u8]) -> bool {
    node_imports_c(tree.root_node(), source)
}

/// The assembled Go comment policy.
///
/// The three decisions are plain functions with nothing to capture, so the
/// policy's type parameters are **function pointers** rather than closures.
/// That is what makes this type nameable at all: an `impl Fn` triple would
/// force every caller that wants to hold a policy — a struct field, a helper's
/// parameter — into an `impl Trait` chain it cannot write down.
pub type GoCommentPolicy =
    CommentPolicy<fn(&[u8]) -> bool, fn(&Tree, &[u8]) -> Range<usize>, fn(&Tree, &[u8]) -> bool>;

/// The Go comment policy: the one call the formatter makes.
///
/// Building one is three function-pointer stores, so callers construct a
/// policy per operation rather than sharing one, exactly as they do with
/// [`crate::config::go_config`].
pub fn comment_policy() -> GoCommentPolicy {
    CommentPolicy::new(is_semantic_comment, build_constraint_prologue, imports_c)
}

/// True when `line` is a build constraint of either generation.
///
/// Used both on a whole line of the file header and on a line comment's own
/// bytes, which is sound because a `//` comment is exactly one line.
///
/// `go/build`'s `parseFileHeader` trims each header line before testing it,
/// so an **indented** constraint counts — measured: `\t//go:build ignoreme`
/// still keeps the file out of `GoFiles`. And `splitPlusBuild` notes that
/// "the space is optional", so `//+build` is a constraint too.
///
/// This is deliberately looser than `constraint.IsGoBuild` /
/// `constraint.IsPlusBuild`: it tests the prefix and not the expression that
/// follows. Answering `true` for something the toolchain would ignore keeps a
/// comment or protects a header that did not need it, which costs bytes; the
/// other direction costs a build.
fn is_build_constraint(line: &[u8]) -> bool {
    let line = line.trim_ascii();
    let Some(after_slashes) = line.strip_prefix(b"//") else {
        return false;
    };
    line.starts_with(b"//go:build") || after_slashes.trim_ascii_start().starts_with(b"+build")
}

/// Whether the byte at `start` is the first of its line.
///
/// Byte 0 is, by definition: there is no preceding newline because there is
/// no preceding byte.
fn starts_a_line(source: &[u8], start: usize) -> bool {
    start == 0 || source[start - 1] == b'\n'
}

/// The start byte of the file's package clause, if it has one.
///
/// A `package_clause` is a top-level declaration in the grammar, so it is a
/// direct child of `source_file` — and the engine's parse gate rejects any
/// tree carrying an `ERROR` node, so it cannot be buried inside one.
fn package_clause_start(tree: &Tree) -> Option<usize> {
    let root = tree.root_node();
    let mut cursor = root.walk();
    // Bound to a local rather than tail-returned: the iterator borrows
    // `cursor`, and a temporary in tail position outlives it.
    let mut children = root.children(&mut cursor);
    children
        .find(|child| child.kind() == "package_clause")
        .map(|clause| clause.start_byte())
}

/// Whether `node`'s subtree contains an import of `"C"`.
///
/// An `import_spec`'s children are its optional local name and its path, and
/// no spelling of a name is textually `"C"` or `` `C` `` — a name is a bare
/// identifier, a `.` or a `_`. Testing every child is therefore exactly as
/// precise as reading the `path` field, without an `Option` whose empty case
/// the grammar cannot produce.
fn node_imports_c(node: Node, source: &[u8]) -> bool {
    let mut cursor = node.walk();
    let mut children = node.children(&mut cursor);
    if node.kind() == "import_spec" {
        children.any(|child| is_c_import_path(&source[child.byte_range()]))
    } else {
        children.any(|child| node_imports_c(child, source))
    }
}

/// Whether these are the bytes of an import path naming the cgo pseudo-package.
fn is_c_import_path(bytes: &[u8]) -> bool {
    bytes == b"\"C\"" || bytes == b"`C`"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::go_config;
    use tokenpress_treesitter::emit::{
        minimize, strip_comments_pinned, strip_comments_plan, strip_comments_source,
    };
    use tokenpress_treesitter::parser::parse;

    /// The comments-stripped emitter as G3 will assemble it, minus the
    /// column-0 pin — the engine's own end-to-end entry point.
    fn stripped(source: &[u8]) -> Vec<u8> {
        strip_comments_source(&go_config(), source, &comment_policy()).unwrap()
    }

    /// The whole composition, pin included: what G3 has to build.
    fn stripped_and_pinned(source: &[u8]) -> Vec<u8> {
        let config = go_config();
        let tree = parse(&config, source).unwrap();
        let plan = strip_comments_plan(&config, &tree, source, &comment_policy());
        strip_comments_pinned(source, &plan, minimize(&config), |span| {
            is_promotable_directive(source, span)
        })
    }

    /// The prologue range for a source the engine accepts.
    fn prologue(source: &[u8]) -> Range<usize> {
        let config = go_config();
        let tree = parse(&config, source).unwrap();
        build_constraint_prologue(&tree, source)
    }

    /// Whether the bail-out fires for a source the engine accepts.
    fn bails_out(source: &[u8]) -> bool {
        let config = go_config();
        let tree = parse(&config, source).unwrap();
        imports_c(&tree, source)
    }

    // --- ast.IsDirective ---------------------------------------------------

    #[test]
    fn the_go_toolchains_own_is_directive_table_holds() {
        // Copied from `isDirectiveTests` in
        // `/usr/local/go/src/go/ast/ast_test.go:55` (go1.24.7), with the `//`
        // the toolchain strips before calling `isDirective` put back on.
        for (text, expected) in [
            ("abc", false),
            ("go:inline", true),
            ("Go:inline", false),
            ("go:Inline", false),
            (":inline", false),
            ("lint:ignore", true),
            ("lint:1234", true),
            ("1234:lint", true),
            ("go: inline", false),
            ("go:", false),
            ("go:*", false),
            ("go:x*", true),
            ("export foo", true),
            ("extern foo", true),
            ("expert foo", false),
        ] {
            let comment = format!("//{text}");
            assert_eq!(
                is_directive_comment(comment.as_bytes()),
                expected,
                "{comment}"
            );
        }
    }

    #[test]
    fn the_directive_prefixes_need_their_trailing_space() {
        // `strings.HasPrefix(c, "line ")`, so a bare `//line` is not a line
        // directive — and it has no colon either, so nothing else catches it.
        assert!(!is_directive_comment(b"//line"));
        assert!(!is_directive_comment(b"//export"));
        assert!(!is_directive_comment(b"//extern"));
        assert!(is_directive_comment(b"//line foo.go:1"));
        // `//line:1` is not the line directive, but it *is* the
        // `[a-z0-9]+:[a-z0-9]` shape, which is exactly what Go answers.
        assert!(is_directive_comment(b"//line:1"));
    }

    #[test]
    fn only_the_first_colon_decides() {
        // `strings.Index(c, ":")` finds the first one, so everything after
        // the byte following it is unconstrained.
        assert!(is_directive_comment(b"//go:generate echo A:B:C"));
        assert!(is_directive_comment(b"//a:b:c"));
        // ...but a byte before the *first* colon still has to be in the set.
        assert!(!is_directive_comment(b"//go generate:x"));
    }

    #[test]
    fn a_block_comment_is_never_a_line_directive() {
        // `isDirective` is only ever asked about `//` comments: `go/ast`
        // strips the slashes first and `go/printer` calls it from its
        // `//`-comment branch only.
        assert!(!is_directive_comment(b"/*go:inline*/"));
        assert!(!is_directive_comment(b"/*line foo.go:1*/"));
        assert!(!is_directive_comment(b"/* not a comment marker at all */"));
        assert!(!is_directive_comment(b"go:inline"));
    }

    // --- The keep predicate ------------------------------------------------

    #[test]
    fn every_directive_family_is_semantic() {
        for comment in [
            &b"//go:build linux"[..],
            b"//go:generate stringer -type=T",
            b"//go:embed data.txt",
            b"//go:noinline",
            b"//go:linkname localname target",
            b"//line foo.go:12:3",
            b"//export GoCallback",
            b"//extern gccgo_symbol",
            b"//lint:ignore SA1000 reason",
        ] {
            // Rendered eagerly, not as a lazy assertion message: an argument
            // only evaluated on failure is a line the coverage gate never
            // sees.
            let text = String::from_utf8_lossy(comment);
            assert!(is_semantic_comment(comment), "{text}");
        }
    }

    #[test]
    fn the_block_line_directive_is_semantic_but_no_block_go_directive_is() {
        // `/*line ` is honoured at any column — `go/scanner/scanner.go` gates
        // on `lit[1] == '*' || offs == s.lineOffset` — so it has to survive.
        assert!(is_semantic_comment(b"/*line foo.go:12*/"));
        // There is no `/*go:` form. `fullComment` in
        // `cmd/compile/internal/syntax/scanner.go` recognises the single
        // prefix `"line "`, and measurement agrees: with go1.24.7,
        // `/*go:embed data.txt*/` above a `var` leaves it empty, and
        // `/*go:build ignoreme*/` does not keep the file out of `GoFiles`.
        assert!(!is_semantic_comment(b"/*go:embed data.txt*/"));
        assert!(!is_semantic_comment(b"/*go:build ignoreme*/"));
        // And the space is part of the prefix.
        assert!(!is_semantic_comment(b"/*linefoo.go:12*/"));
    }

    #[test]
    fn the_legacy_build_constraint_is_semantic() {
        // `// +build` has no colon, so `isDirective` says no; it is kept by
        // the constraint rule instead.
        assert!(!is_directive_comment(b"// +build linux"));
        assert!(is_semantic_comment(b"// +build linux"));
        // `splitPlusBuild` in `go/build/constraint/expr.go`: "the space is
        // optional".
        assert!(is_semantic_comment(b"//+build linux"));
    }

    #[test]
    fn an_ordinary_comment_is_not_semantic() {
        for comment in [
            &b"// Package main does things."[..],
            b"//",
            b"// TODO: fix this",
            b"/* a block comment */",
            b"// go:generate not-a-directive",
        ] {
            let text = String::from_utf8_lossy(comment);
            assert!(!is_semantic_comment(comment), "{text}");
        }
    }

    // --- The column-0 split ------------------------------------------------

    #[test]
    fn an_indented_line_directive_is_promotable_and_a_column_zero_one_is_not() {
        let source = b"package main\n\nfunc f() {\n\t//line filename:line:col\n\tprintln(1)\n}\n";
        let start = source
            .windows(2)
            .position(|pair| pair == b"//")
            .expect("the comment is there");
        let span = Span::new(start, start + 24, SpanKind::Comment);
        assert_eq!(&source[span.range()], b"//line filename:line:col");
        assert!(is_promotable_directive(source, span));

        // The same comment one byte earlier, where the tab is not: at column
        // 0 it already *is* a directive and may not be moved.
        let at_column_zero = Span::new(start - 1, start + 23, SpanKind::Comment);
        assert!(!is_promotable_directive(
            b"package main\n\nfunc f() {\n//line filename:line:col\n\tprintln(1)\n}\n",
            at_column_zero
        ));
    }

    #[test]
    fn the_indented_line_directive_from_go_scanner_survives_at_column_one() {
        // The real thing: `/usr/local/go/src/go/scanner/scanner.go` lines 263
        // and 272 carry indented `//line filename:line:col` comments, written
        // as documentation of the directive's own syntax. Measured with
        // go1.24.7 in this container: with the comment indented, `gofmt -e`
        // exits 0; moved to column 0 it exits 2 with
        // `invalid line number: col`. Promotion is therefore an
        // external-gate failure, not a subtlety.
        let source = b"package main\n\nfunc f() {\n\t//line filename:line:col\n\tprintln(1)\n}\n";
        assert_eq!(
            stripped_and_pinned(source),
            b"package main\nfunc f() {\n //line filename:line:col\nprintln(1)\n}\n"
        );
        // Without the pin the rewriter promotes it, which is exactly the
        // output `gofmt -e` refuses. The pin is the only thing between the
        // two.
        assert_eq!(
            stripped(source),
            b"package main\nfunc f() {\n//line filename:line:col\nprintln(1)\n}\n"
        );
    }

    #[test]
    fn an_indented_go_generate_is_not_promoted() {
        // The silent one. `cmd/go/internal/generate/generate.go`'s
        // `isGoGenerate` tests `bytes.HasPrefix(line, "//go:generate ")` on a
        // whole line, so indentation hides the directive completely.
        // Measured with go1.24.7: `go generate -n ./...` over a package with
        // an indented `//go:generate echo INDENTED-RAN` inside a function and
        // a column-0 `//go:generate echo COL0-RAN` at top level prints only
        // `echo COL0-RAN`. Promoting the indented one would make `go
        // generate` run a command the author never registered — and no
        // compiler, and no equivalence check, would say a word.
        let source = b"package main\n\nfunc main() {\n\t//go:generate echo ran\n\tprintln(1)\n}\n";
        assert_eq!(
            stripped_and_pinned(source),
            b"package main\nfunc main() {\n //go:generate echo ran\nprintln(1)\n}\n"
        );
    }

    #[test]
    fn a_column_zero_directive_keeps_its_column() {
        // The other half: a directive that is already a directive must not
        // acquire a space either.
        let source = b"package main\n\n//go:generate echo ran\nfunc main() {}\n";
        assert_eq!(
            stripped_and_pinned(source),
            b"package main\n//go:generate echo ran\nfunc main() {}\n"
        );
    }

    #[test]
    fn a_protected_span_and_an_ordinary_comment_are_never_promotable() {
        let source = b"package main\n\nvar s = \"//go:generate x\"\n";
        let start = source
            .windows(1)
            .position(|byte| byte == b"\"")
            .expect("the literal is there");
        // A string literal's bytes begin with a quote, so the shape test
        // would answer no anyway — the kind test is what makes that an
        // argument rather than a coincidence.
        assert!(!is_promotable_directive(
            source,
            Span::new(start, source.len() - 1, SpanKind::Protected)
        ));
        // And a plain indented comment is not directive-shaped.
        let plain = b"package main\n\nfunc f() {\n\t// just a note\n}\n";
        let note = plain
            .windows(2)
            .position(|pair| pair == b"//")
            .expect("the comment is there");
        assert!(!is_promotable_directive(
            plain,
            Span::new(note, note + 14, SpanKind::Comment)
        ));
    }

    // --- The verbatim prologue ---------------------------------------------

    #[test]
    fn a_go_build_header_is_reproduced_verbatim_including_its_blank_line() {
        let source = b"//go:build linux && amd64\n\npackage a\n\nfunc f() {}\n";
        assert_eq!(prologue(source), 0..27);
        assert_eq!(
            stripped(source),
            b"//go:build linux && amd64\n\npackage a\nfunc f() {}\n"
        );
    }

    #[test]
    fn the_legacy_constraints_blank_line_survives() {
        // Measured with go1.24.7, two packages differing only in that blank
        // line: with it, `go list` reports `GoFiles=[x.go] Ignored=[y.go]`;
        // without it, `GoFiles=[x.go y.go] Ignored=[]`. The blank line is the
        // constraint. The whitespace policy collapses blank lines, so the
        // verbatim region — not the keep flag — is what defends it.
        let source = b"// +build ignoreme\n\npackage a\n\nfunc f() {}\n";
        assert_eq!(prologue(source), 0..20);
        assert_eq!(
            stripped(source),
            b"// +build ignoreme\n\npackage a\nfunc f() {}\n"
        );
    }

    #[test]
    fn the_region_covers_the_whole_header_not_just_the_constraint() {
        // Both constraint generations plus the package doc comment: all of it
        // is before the package clause, so all of it is reproduced verbatim.
        // The doc comment surviving a `--strip-comments` run is the price of
        // a rule with no seams.
        let source = b"//go:build linux\n// +build linux\n\n// Package a does things.\npackage a\n\nfunc f() {}\n";
        assert_eq!(prologue(source), 0..60);
        assert_eq!(
            stripped(source),
            &b"//go:build linux\n// +build linux\n\n// Package a does things.\npackage a\nfunc f() {}\n"[..]
        );
    }

    #[test]
    fn an_indented_constraint_still_claims_the_header() {
        // `parseFileHeader` trims each line before testing it. Measured:
        // `\t//go:build ignoreme` still keeps the file out of `GoFiles`.
        assert_eq!(prologue(b"\t//go:build linux\n\npackage a\n"), 0..19);
    }

    #[test]
    fn a_header_without_a_constraint_claims_nothing() {
        let source = b"// Package a does things.\npackage a\n\nfunc f() {}\n";
        assert_eq!(prologue(source), 0..0);
        // ...so the doc comment is an ordinary comment and goes.
        assert_eq!(stripped(source), b"package a\nfunc f() {}\n");
    }

    #[test]
    fn a_source_with_no_package_clause_has_an_empty_prologue() {
        // Measured in G1: this parses, so the anchor is simply absent and the
        // answer has to be defined rather than unwrapped.
        assert_eq!(prologue(b"func f() int { return 1 }\n"), 0..0);
        // Even with a header that would otherwise claim it.
        assert_eq!(prologue(b"//go:build linux\n\nfunc f() {}\n"), 0..0);
    }

    #[test]
    fn an_empty_source_has_an_empty_prologue() {
        // Measured in G1: an empty file parses with zero children.
        assert_eq!(prologue(b""), 0..0);
        assert_eq!(stripped(b""), b"");
    }

    // --- The cgo bail-out --------------------------------------------------

    #[test]
    fn a_cgo_file_is_left_byte_identical() {
        // Measured with go1.24.7: with the preamble the package builds;
        // delete those four comment lines and `go build` fails with
        // `could not determine what C.hello refers to`. The preamble is C
        // source, and the equivalence artifact cannot see comments at all.
        let source = b"package main\n\n/*\n#include <stdio.h>\nstatic void hello(void) { printf(\"hi\\n\"); }\n*/\nimport \"C\"\n\nfunc main() { C.hello() }\n";
        assert!(bails_out(source));
        assert_eq!(stripped(source), source);
        // The bail-out also survives the pinning composition, because the
        // plan protects every byte and leaves no gap.
        assert_eq!(stripped_and_pinned(source), source);
    }

    #[test]
    fn a_grouped_import_of_c_bails_out() {
        // Measured: this form builds as cgo under go1.24.7.
        let source = b"package main\n\nimport (\n\t\"fmt\"\n\t/*\n\t   #include <stdio.h>\n\t*/\n\t\"C\"\n)\n\nfunc main() { fmt.Println(\"x\"); C.hello() }\n";
        assert!(bails_out(source));
        assert_eq!(stripped(source), source);
    }

    #[test]
    fn a_back_quoted_import_of_c_bails_out() {
        // Measured: `go list` puts this file in `CgoFiles` — `go/build`
        // unquotes the path before comparing — even though `cmd/cgo` then
        // reports `cannot find import "C"`. It is a cgo file to the
        // toolchain, so it is one here.
        assert!(bails_out(b"package main\n\nimport `C`\n\nfunc main() {}\n"));
    }

    #[test]
    fn a_named_import_of_c_bails_out() {
        assert!(bails_out(
            b"package main\n\nimport _ \"C\"\n\nfunc main() {}\n"
        ));
    }

    #[test]
    fn a_c_that_is_not_an_import_of_c_does_not_bail_out() {
        // A `"C"` used as a value, an import path that merely ends in `/C`,
        // and one that merely starts with `C`.
        let source = b"package main\n\nimport (\n\t\"fmt\"\n\t\"example.com/C\"\n\t\"Csv\"\n)\n\n// note\nvar c = \"C\"\n\nfunc main() { fmt.Println(c, C.X, Csv.Y) }\n";
        assert!(!bails_out(source));
        // ...and stripping really did happen, so the assertion above is not
        // vacuous.
        assert!(!stripped(source).windows(6).any(|w| w == b"// not"));
    }

    // --- go:embed ----------------------------------------------------------

    #[test]
    fn the_embed_directive_survives_and_stays_attached_to_its_var() {
        // Measured with go1.24.7: with the directive, `go run` prints
        // `[hello]`; delete it and the program still compiles and prints
        // `[]`. Silent corruption of exactly the kind the equivalence
        // artifact is blind to.
        //
        // Also measured, refining the usual "must be adjacent" folklore: a
        // blank line between the directive and the `var` is tolerated, but a
        // declaration in between is not (`go:embed cannot apply to var of
        // type int`). The emitter never reorders declarations, so what has to
        // hold here is only that the directive survives in place.
        let source = b"package main\n\nimport _ \"embed\"\n\n//go:embed data.txt\nvar s string\n\nfunc main() { println(s) }\n";
        assert_eq!(
            stripped(source),
            &b"package main\nimport _ \"embed\"\n//go:embed data.txt\nvar s string\nfunc main() { println(s) }\n"[..]
        );
    }

    #[test]
    fn deleting_a_comment_between_the_directive_and_its_var_does_not_separate_them() {
        // Blanking is length-preserving and keeps every newline, so the
        // deleted note leaves the directive on the line directly above the
        // declaration rather than pushing it away.
        let source =
            b"package main\n\nimport _ \"embed\"\n\n//go:embed data.txt\n// a note\nvar s string\n";
        assert_eq!(
            stripped(source),
            &b"package main\nimport _ \"embed\"\n//go:embed data.txt\nvar s string\n"[..]
        );
    }

    // --- The assembled policy ----------------------------------------------

    #[test]
    fn the_policy_is_the_three_decisions_in_one_call() {
        // One source exercising all three at once: a claimed header, a kept
        // directive deeper in the file, and a deleted ordinary comment.
        let source = b"//go:build linux\n\npackage a\n\n// dropped\n//go:noinline\nfunc f() {}\n";
        assert_eq!(
            stripped(source),
            &b"//go:build linux\n\npackage a\n//go:noinline\nfunc f() {}\n"[..]
        );
    }
}
