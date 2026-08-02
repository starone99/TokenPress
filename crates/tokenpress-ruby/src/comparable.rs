//! The AST-equivalence artifact: a deterministic, location-independent
//! rendering of a Ruby parse, so that two sources can be compared for
//! structural equivalence.
//!
//! This is what makes emitter iteration safe — `src/verify.rs` (a later step)
//! compares [`comparable`] of the input against [`comparable`] of the output
//! and refuses to write anything whose artifact moved.
//!
//! # Why not `Debug`, and why the `unsafe` block
//!
//! prism's nodes implement no `PartialEq`, and their `Debug` output is
//! unusable as an artifact: it prints raw pointer addresses, and `IntegerNode`
//! carries no printable value, so `x = 1` and `x = 2` render identically once
//! addresses are normalized. prism ships exactly the artifact needed —
//! `pm_prettyprint` — and the C sources for it *are* compiled into the
//! `libprism.a` that `ruby-prism-sys` links (nothing defines
//! `PRISM_EXCLUDE_PRETTYPRINT`), but `pm_prettyprint`, `pm_buffer_init` and
//! `pm_buffer_free` are **not** in that crate's bindgen allowlist, which
//! covers only the 12 functions it considers "public consumption".
//!
//! This module therefore declares those three symbols itself, alongside a
//! `#[repr(C)]` mirror of `pm_buffer_t`. **That is a deliberate, tested
//! coupling to `ruby-prism-sys`'s vendored prism build**: if a future version
//! stops compiling `prettyprint.c`, changes the layout of `pm_buffer_t`, or
//! changes those signatures, this module fails to link or misbehaves — which
//! is why `ruby-prism-sys` is pinned exactly (`=1.9.0`) as a *direct*
//! dependency in `Cargo.toml` rather than being picked up transitively. Every
//! other prism entry point used here (`pm_parser_init`, `pm_parse`,
//! `pm_node_destroy`, `pm_parser_free`, `pm_parser_t`, `pm_node_t`) comes
//! from `ruby-prism-sys`'s own declarations; nothing is hand-declared that
//! the -sys crate already provides.
//!
//! The safe `ruby-prism` crate is still off limits outside
//! [`crate::parser`] (CLAUDE.md): the safe-crate call this module needs (the
//! parse-error gate and the magic-comment list) goes through
//! [`crate::parser::parse`] and its re-exported types.
//!
//! # What the artifact contains
//!
//! ```text
//! magic frozen_string_literal=true
//! ---
//! @ ProgramNode (location: )
//! +-- locals: [:x]
//! ...
//! ```
//!
//! 1. A **magic-comment prelude**, one `magic key=value` line per recognized,
//!    semantic magic comment, in source order (see below).
//! 2. A `---` separator.
//! 3. The `pm_prettyprint` tree with every source-location coordinate tuple
//!    removed, and every non-ASCII byte escaped as `\xNN`.
//!
//! ## Coordinate-tuple stripping
//!
//! `pm_prettyprint` writes locations as `(line,column)-(line,column)` in
//! exactly two places: after `(location: ` on a `@ NodeName` header line, and
//! as the first thing in a `+-- some_loc: ` field value (there followed by
//! ` = "source text"`). Formatting-only edits move those numbers, so they are
//! removed.
//!
//! The stripper is **position-anchored**, not a global search-and-replace: it
//! only removes a tuple that sits at the very start of a field value (the
//! text right after the first `": "` on the line). A global replace would
//! also eat the contents of `x = "(1,2)-(3,4)"`, which appear in
//! `content_loc`/`unescaped` values and are load-bearing — that case is
//! pinned in the tests.
//!
//! The `= "source text"` half of a location field is **kept**. It is what
//! makes the artifact reject `a - b` vs `a -b` and `"a"` vs `'a'`, and it is
//! also the source of the two known over-refusal classes (below).
//!
//! ## Non-ASCII escaping
//!
//! `pm_prettyprint` escapes source slices itself (bytes `>= 0x7F` become
//! `\xNN`), but constant names — `+-- name: :ほげ`, `+-- locals: [:ほげ]` —
//! are written raw, so the buffer is not guaranteed to be ASCII and, for a
//! non-UTF-8 source encoding, not guaranteed to be UTF-8 either. Every byte
//! `>= 0x80` is therefore escaped as `\xNN` on the way into the artifact.
//! That is lossless (identifiers cannot contain a backslash, so the escape
//! cannot collide with prism's own) and removes any need for a lossy
//! conversion, which could have made two different byte strings compare
//! equal. Non-UTF-8 sources are accepted exactly as [`crate::parser::parse`]
//! accepts them.
//!
//! ## Magic comments
//!
//! `# frozen_string_literal: true` is semantic — it changes string
//! mutability. prism reflects it in `StringFlags: frozen` / `mutable`, so the
//! prettyprint artifact catches it **whenever the file contains a string
//! literal**; a file with no strings renders identically with and without the
//! comment (verified). The prelude closes that gap.
//!
//! Only *semantic* keys are included. `ParseResult::magic_comments()` is a
//! purely lexical scan: it reports **any** `# key: value` comment anywhere in
//! the file, including `# note: hi` in the middle of a method body (verified).
//! Including all of them would make the artifact reject nearly every
//! comment-stripping edit, so the prelude is limited to
//! [`SEMANTIC_MAGIC_COMMENT_KEYS`] — the keys that change how the program is
//! interpreted. Keys are lower-cased (Ruby matches them case-insensitively);
//! values are escaped like everything else.
//!
//! # Known holes and over-refusals
//!
//! - **Comments are invisible** (except the semantic magic comments above).
//!   Adding or removing a comment does not change the artifact, so comment
//!   loss is *not* caught here — comment policy belongs to the emitter, the
//!   same division of labour as `tokenpress-js`.
//! - **Over-refusal: location slices that span reformatted code.** Because
//!   the `= "source text"` half is kept, an edit *inside* a location slice
//!   that covers more than one token is reported as a difference even when
//!   it is semantically inert. The two measured classes are a `<<~` heredoc's
//!   terminator/body indentation and a multi-line index call's `message_loc`
//!   (`a[1,\n2]`), both pinned in the tests. This direction is safe: it
//!   refuses a good output, it never accepts a bad one.

use std::mem::MaybeUninit;

use ruby_prism_sys::{
    pm_node_destroy, pm_node_t, pm_parse, pm_parser_free, pm_parser_init, pm_parser_t,
};
use tokenpress_core::Result;

/// Magic-comment keys that change how Ruby interprets the program, and so
/// belong in the artifact. Everything else `magic_comments()` reports is a
/// plain comment that happens to look like `key: value`.
const SEMANTIC_MAGIC_COMMENT_KEYS: [&str; 4] = [
    "coding",
    "encoding",
    "frozen_string_literal",
    "shareable_constant_value",
];

/// The `pm_buffer_t` layout, mirrored because `ruby-prism-sys` does not
/// expose it. See the module doc for the coupling this creates.
#[repr(C)]
struct Buffer {
    length: usize,
    capacity: usize,
    value: *mut std::os::raw::c_char,
}

// Symbols that live in `libprism.a` but are outside `ruby-prism-sys`'s
// bindgen allowlist. Signatures copied from `prism/util/pm_buffer.h` and
// `prism/prettyprint.h` of the vendored prism 1.9.0.
unsafe extern "C" {
    fn pm_buffer_init(buffer: *mut Buffer) -> bool;
    fn pm_buffer_free(buffer: *mut Buffer);
    fn pm_prettyprint(
        output_buffer: *mut Buffer,
        parser: *const pm_parser_t,
        node: *const pm_node_t,
    );
}

/// Renders `source` as its canonical, location-independent artifact.
///
/// Returns [`tokenpress_core::Error::Parse`] for a source prism reports
/// errors for — an unparseable input never yields an artifact.
pub fn comparable(source: &[u8]) -> Result<String> {
    // The parse-error gate, and the magic-comment list, both come from the
    // safe crate through `parser::parse` — it is the only module allowed to
    // name `ruby_prism`, and reusing it keeps the error text identical to the
    // rest of the crate. prism is re-run below through the raw API because
    // `pm_prettyprint` needs the `pm_parser_t` that the safe wrapper owns
    // privately.
    let parsed = crate::parser::parse(source)?;

    let mut artifact = String::new();
    for magic in parsed.magic_comments() {
        let mut key = String::new();
        push_escaped(&mut key, magic.key());
        key.make_ascii_lowercase();
        if SEMANTIC_MAGIC_COMMENT_KEYS.contains(&key.as_str()) {
            artifact.push_str("magic ");
            artifact.push_str(&key);
            artifact.push('=');
            push_escaped(&mut artifact, magic.value());
            artifact.push('\n');
        }
    }
    artifact.push_str("---\n");

    for line in prettyprint(source).split_inclusive(|byte| *byte == b'\n') {
        push_escaped(&mut artifact, &strip_location(line));
    }
    Ok(artifact)
}

/// Convenience wrapper: `true` when `a` and `b` have the same artifact.
///
/// Propagates the parse error if *either* side fails to parse.
pub fn equivalent(a: &[u8], b: &[u8]) -> Result<bool> {
    Ok(comparable(a)? == comparable(b)?)
}

/// Runs prism over `source` and returns the raw `pm_prettyprint` bytes.
///
/// The caller has already established through [`crate::parser::parse`] that
/// `source` parses; a source with errors still yields a tree here, it is just
/// never asked for.
fn prettyprint(source: &[u8]) -> Vec<u8> {
    // The parser struct is several kilobytes, so it is boxed rather than held
    // on the stack — the same shape `ruby_prism::parse` uses.
    let uninit = Box::into_raw(Box::new(MaybeUninit::<pm_parser_t>::uninit()));

    // SAFETY: `uninit` is a live, correctly aligned allocation for one
    // `pm_parser_t`, which `pm_parser_init` fully initializes before anything
    // reads it. `source` outlives every prism call below because prism only
    // borrows it for the duration of the parse. `buffer` is likewise
    // initialized by `pm_buffer_init` before use, and holds a
    // `pm_buffer_init`-allocated block of `length` readable bytes at `value`.
    // Each resource is released exactly once, in the order prism documents:
    // the buffer, then the node, then the parser.
    unsafe {
        pm_parser_init(
            (*uninit).as_mut_ptr(),
            source.as_ptr(),
            source.len(),
            std::ptr::null(),
        );
        let parser: *mut pm_parser_t = (*uninit).assume_init_mut();
        let node = pm_parse(parser);

        let mut buffer = MaybeUninit::<Buffer>::uninit();
        // `pm_buffer_init` only reports failure when its initial `malloc`
        // fails, i.e. the process is out of memory; there is nothing useful
        // to report back through `Result` at that point.
        assert!(
            pm_buffer_init(buffer.as_mut_ptr()),
            "prism buffer allocation"
        );
        let mut buffer = buffer.assume_init();
        pm_prettyprint(&mut buffer, parser, node);
        let rendered =
            std::slice::from_raw_parts(buffer.value.cast::<u8>(), buffer.length).to_vec();

        pm_buffer_free(&mut buffer);
        pm_node_destroy(parser, node);
        pm_parser_free(parser);
        drop(Box::from_raw(uninit));
        rendered
    }
}

/// Removes the source-location coordinate tuple from one prettyprint line.
///
/// A tuple only counts when it starts the line's *value* — the text right
/// after the first `": "` — which is where `pm_prettyprint` puts one and
/// where nothing else can look like one.
fn strip_location(line: &[u8]) -> Vec<u8> {
    if let Some(separator) = line.windows(2).position(|pair| pair == b": ") {
        let value = separator + 2;
        if let Some(length) = coordinate_tuple_len(&line[value..]) {
            return [&line[..value], &line[value + length..]].concat();
        }
    }
    line.to_vec()
}

/// Length in bytes of a `(line,column)-(line,column)` tuple at the start of
/// `value`, or `None` when `value` does not start with one.
///
/// Line and column are printed unsigned in practice: prism only emits a
/// negative line number when the caller sets a negative `start_line` through
/// `pm_options_t`, and this module always passes null options.
fn coordinate_tuple_len(value: &[u8]) -> Option<usize> {
    // The literal bytes of the tuple, in order. Every one of them except a
    // closing paren and the joining dash is followed by at least one digit.
    const SHAPE: &[u8] = b"(,)-(,)";

    let mut index = 0;
    for expected in SHAPE {
        if value.get(index) != Some(expected) {
            return None;
        }
        index += 1;
        if !matches!(*expected, b')' | b'-') {
            let digits = value[index..]
                .iter()
                .take_while(|byte| byte.is_ascii_digit())
                .count();
            if digits == 0 {
                return None;
            }
            index += digits;
        }
    }
    Some(index)
}

/// Appends `bytes` to `out`, escaping everything outside ASCII as `\xNN`.
fn push_escaped(out: &mut String, bytes: &[u8]) {
    for byte in bytes {
        if byte.is_ascii() {
            out.push(char::from(*byte));
        } else {
            out.push_str(&format!("\\x{byte:02X}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokenpress_core::Error;

    /// Pairs that differ only in formatting: the artifact must be identical.
    const EQUIVALENT: &[(&str, &str, &str)] = &[
        ("whitespace only", "x  =   1\n", "x = 1\n"),
        ("newline vs semicolon", "a = 1\nb = 2\n", "a = 1; b = 2\n"),
        (
            "indentation removed",
            "def f\n    1\nend\n",
            "def f\n1\nend\n",
        ),
        ("blank line runs", "a = 1\n\n\n\nb = 2\n", "a = 1\nb = 2\n"),
        ("trailing whitespace", "x = 1   \n", "x = 1\n"),
        ("final newline", "x = 1", "x = 1\n"),
        ("joined argument list", "foo(1,\n  2)\n", "foo(1, 2)\n"),
        (
            "joined parameter list",
            "def f(a,\nb)\nend\n",
            "def f(a, b)\nend\n",
        ),
        ("hash label spacing", "f(a: 1)\n", "f(a:1)\n"),
        ("word list spacing", "%w[a   b]\n", "%w[a b]\n"),
        ("symbol list spacing", "%i[a   b]\n", "%i[a b]\n"),
        ("operator spacing", "1 + 2\n", "1+2\n"),
        ("comment added", "x = 1\n", "# note\nx = 1\n"),
        (
            "comment inside a body",
            "def f\n  # hi\n  1\nend\n",
            "def f\n  1\nend\n",
        ),
        ("embdoc removed", "=begin\nhi\n=end\nx = 1\n", "x = 1\n"),
        (
            "brace block reflowed",
            "xs.each { |x| p x }\n",
            "xs.each { |x|\n  p x\n}\n",
        ),
        (
            "do block reflowed",
            "xs.each do |x|\n  p x\nend\n",
            "xs.each do |x| p x end\n",
        ),
        (
            "class body reflowed",
            "class A\n  def b; 1; end\nend\n",
            "class A\ndef b\n1\nend\nend\n",
        ),
        (
            "module body reflowed",
            "module M\n  X = 1\nend\n",
            "module M\nX = 1\nend\n",
        ),
        (
            "rescue clause reflowed",
            "begin\n  a\nrescue => e\n  b\nend\n",
            "begin\na\nrescue => e\nb\nend\n",
        ),
    ];

    /// Pairs that must never be accepted as equivalent.
    const DIFFERENT: &[(&str, &str, &str)] = &[
        ("integer value", "x = 1\n", "x = 2\n"),
        // The flag-blindness canary: both are the integer 16. Only a
        // flag-aware artifact separates them.
        ("integer base flag", "x = 0x10\n", "x = 16\n"),
        ("binary integer base flag", "x = 0b10\n", "x = 2\n"),
        ("integer vs float", "x = 1\n", "x = 1.0\n"),
        ("different operator", "1 + 2\n", "1 - 2\n"),
        // `a -b` is a call with a unary-minus argument, not a subtraction.
        ("unary argument vs binary operator", "a - b\n", "a -b\n"),
        // `foo (1..2)` wraps the argument in a ParenthesesNode.
        ("parenthesized argument", "foo(1..2)\n", "foo (1..2)\n"),
        ("string vs symbol", "x = \"a\"\n", "x = :a\n"),
        ("string contents", "x = \"a\"\n", "x = 'b'\n"),
        ("quote style", "x = \"a\"\n", "x = 'a'\n"),
        (
            "interpolated string contents",
            "x = \"a#{1}b\"\n",
            "x = \"a#{1}c\"\n",
        ),
        ("regexp contents", "x = /a/\n", "x = /b/\n"),
        ("word list vs symbol list", "%w[a b]\n", "%i[a b]\n"),
        ("method rename", "def a; end\n", "def b; end\n"),
        ("added statement", "a = 1\n", "a = 1\nb = 2\n"),
        ("removed statement", "a = 1\nb = 2\n", "a = 1\n"),
        ("heredoc contents", "x = <<A\nhi\nA\n", "x = <<A\nho\nA\n"),
        ("safe navigation flag", "a&.b\n", "a.b\n"),
        ("range exclusivity", "1..2\n", "1...2\n"),
        // The `then` keyword is a recorded location, so this is a real token
        // difference rather than pure formatting.
        ("then keyword", "if a\n  b\nend\n", "if a then b end\n"),
        // `def a = 1` and `def a; 1; end` are different prism trees:
        // `equal_loc` is set for the endless form, `end_keyword_loc` for the
        // other. Verified against prism 1.9.0.
        (
            "endless method definition",
            "def a = 1\n",
            "def a; 1; end\n",
        ),
        // The frozen-string magic comment reaches StringFlags…
        (
            "frozen string literal with strings",
            "# frozen_string_literal: true\nx = \"a\"\n",
            "x = \"a\"\n",
        ),
        // …and the prelude covers the case where there is no string to flag.
        (
            "frozen string literal without strings",
            "# frozen_string_literal: true\nx = 1\n",
            "x = 1\n",
        ),
        (
            "frozen string literal true vs false",
            "# frozen_string_literal: true\nx = 1\n",
            "# frozen_string_literal: false\nx = 1\n",
        ),
        (
            "encoding magic comment",
            "# encoding: binary\nx = 1\n",
            "x = 1\n",
        ),
        (
            "shareable constant value magic comment",
            "# shareable_constant_value: literal\nX = [1]\n",
            "X = [1]\n",
        ),
        // The stripper must not reach inside a string literal that happens to
        // look like a pair of coordinate tuples.
        (
            "string that looks like a location",
            "x = \"(1,2)-(3,4)\"\n",
            "x = \"(9,9)-(9,9)\"\n",
        ),
    ];

    #[test]
    fn formatting_only_changes_compare_equal() {
        // The artifacts are rendered up front rather than in the assertion
        // message so that a failure prints them — and so that no line of this
        // test only runs when it fails.
        for (name, left, right) in EQUIVALENT {
            let left = comparable(left.as_bytes()).unwrap();
            let right = comparable(right.as_bytes()).unwrap();
            assert_eq!(left, right, "{name}: expected equal");
        }
    }

    #[test]
    fn semantic_changes_compare_different() {
        for (name, left, right) in DIFFERENT {
            let left = comparable(left.as_bytes()).unwrap();
            let right = comparable(right.as_bytes()).unwrap();
            assert_ne!(left, right, "{name}: expected different");
        }
    }

    #[test]
    fn a_squiggly_heredoc_reindent_is_an_over_refusal() {
        // Known over-refusal: the terminator and body indentation live inside
        // `closing_loc`/`content_loc` slices, which the artifact keeps.
        assert!(!equivalent(b"x = <<~A\n    hi\n  A\n", b"x = <<~A\n  hi\nA\n").unwrap());
    }

    #[test]
    fn a_multiline_index_call_is_an_over_refusal() {
        // Known over-refusal: `message_loc` for `[]` spans the whole bracket
        // pair, so joining its arguments moves the recorded source slice.
        assert!(!equivalent(b"a[1,\n2]\n", b"a[1, 2]\n").unwrap());
    }

    #[test]
    fn coordinate_tuples_are_stripped() {
        let artifact = comparable(b"x = 1\n").unwrap();
        assert!(
            artifact.contains("@ ProgramNode (location: )"),
            "{artifact}"
        );
        assert!(!artifact.contains("(1,0)"), "{artifact}");
        assert!(
            artifact.contains("+-- operator_loc:  = \"=\""),
            "{artifact}"
        );
    }

    #[test]
    fn the_magic_comment_prelude_lists_only_semantic_keys() {
        let artifact = comparable(b"# frozen_string_literal: true\n# note: hi\nx = 1\n").unwrap();
        let (prelude, _) = artifact.split_once("\n---\n").unwrap();
        assert_eq!(prelude, "magic frozen_string_literal=true");
    }

    #[test]
    fn the_magic_comment_prelude_is_empty_when_there_are_none() {
        let artifact = comparable(b"# note: hi\nx = 1\n").unwrap();
        assert!(artifact.starts_with("---\n"), "{artifact}");
    }

    #[test]
    fn magic_comment_keys_are_matched_case_insensitively() {
        // Ruby matches magic-comment keys case-insensitively, so the artifact
        // must not treat a differently-cased spelling as a plain comment.
        let artifact = comparable(b"# Frozen_String_Literal: true\nx = 1\n").unwrap();
        assert!(
            artifact.starts_with("magic frozen_string_literal=true\n"),
            "{artifact}"
        );
    }

    #[test]
    fn non_ascii_bytes_are_escaped() {
        // `+-- name: :ほげ` is written raw by prism; the artifact escapes it,
        // so the result is always ASCII whatever the source encoding.
        let artifact = comparable("def ほげ; end\n".as_bytes()).unwrap();
        assert!(artifact.is_ascii(), "{artifact}");
        assert!(
            artifact.contains("+-- name: :\\xE3\\x81\\xBB\\xE3\\x81\\x92"),
            "{artifact}"
        );
    }

    #[test]
    fn non_utf8_sources_are_accepted_and_distinguished() {
        // parser.rs accepts these; so must this module, and two different
        // byte strings must not collapse onto the same artifact.
        let left = b"# encoding: binary\nx = \"\xff\"\n".to_vec();
        let right = b"# encoding: binary\nx = \"\xfe\"\n".to_vec();
        assert!(comparable(&left).unwrap().is_ascii());
        assert!(!equivalent(&left, &right).unwrap());
    }

    #[test]
    fn a_parse_error_is_reported_not_rendered() {
        let err = comparable(b"def ; end").unwrap_err();
        assert!(matches!(err, Error::Parse(_)), "{err}");
    }

    #[test]
    fn equivalent_propagates_a_parse_error_from_either_side() {
        assert!(matches!(
            equivalent(b"def ; end", b"x = 1\n").unwrap_err(),
            Error::Parse(_)
        ));
        assert!(matches!(
            equivalent(b"x = 1\n", b"1 +").unwrap_err(),
            Error::Parse(_)
        ));
    }

    #[test]
    fn an_empty_source_has_an_artifact() {
        assert!(equivalent(b"", b"\n\n").unwrap());
        assert!(comparable(b"").unwrap().contains("@ ProgramNode"));
    }

    #[test]
    fn the_artifact_is_deterministic() {
        let source = b"class A\n  def b(c) = c + 1\nend\n";
        assert_eq!(comparable(source).unwrap(), comparable(source).unwrap());
    }
}
